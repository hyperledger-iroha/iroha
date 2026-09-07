#!/usr/bin/env python3
"""Verify observer-signed KAGEMUSHA V1 physical-device evidence.

The verifier accepts canonical JSON only.  It never imports or executes code from
the candidate under qualification.  Collection is deliberately out of process:
an OEM or laboratory collector emits this closed transcript, and trusted
observers approve its canonical digest with detached Ed25519 signatures.
"""

from __future__ import annotations

import argparse
import hashlib
import sys
from pathlib import Path
from typing import Any, Mapping, NoReturn, Sequence

# The release projector injects its already authenticated module when loading
# these policy-pinned source bytes. Standalone use loads the local verifier.
if "_trusted_release" in globals():
    release = globals()["_trusted_release"]
else:
    import verify_kagemusha_v1_release_evidence as release


TRANSCRIPT_SCHEMA = "iroha.kagemusha_v1.physical_device_transcript"
APPROVAL_SCHEMA = "iroha.kagemusha_v1.physical_device_transcript_approval"
REPORT_SCHEMA = "iroha.kagemusha_v1.hardware_profile_qualification_report"
PHYSICAL_VERIFIER_ID = "physical-device-verifier"
SCHEMA_VERSION = 1
EVENT_HASH_DOMAIN = b"iroha:kagemusha:v1:physical-device-event"
APPROVAL_DOMAIN = b"iroha:kagemusha:v1:physical-device-approval"
ZERO_DIGEST = "0" * 64
MAX_EVIDENCE_BYTES = 16 * 1024 * 1024
MAX_EVENTS = 20_000
MAX_RSS_BYTES = 128 * 1024 * 1024
MAX_LATENCY_MS = 30_000
MAX_THERMAL_LATENCY_MS = 10_000
MIN_THERMAL_FOLDS = 1_000
MIN_THERMAL_DURATION_MS = 60_000
U128_MAX = (1 << 128) - 1
U64_MAX = (1 << 64) - 1
SENDER_EVIDENCE_DOMAIN = b"iroha:kagemusha:v1:physical-sender-evidence\0"
SENDER_PARSER_SCHEMA = "iroha.kagemusha_v1.sender_release_command_projection"
SENDER_PARSER_ID = "native-sender-command-parser"
SENDER_PARSER_SOURCE_ID = "native-sender-command-parser-source"
SENDER_PARSER_PURPOSE = "sender_release_structure"
SENDER_COMMAND_MAX_BYTES = 16 * 1024
SENDER_PARSER_SOURCE = Path(__file__).resolve().parents[1] / "crates/connect_norito_bridge/src/kagemusha_sender_release_evidence.rs"
SENDER_PROJECTION_DIGEST_FIELDS = (
    "command_sha256", "context_sha256", "envelope_sha256", "certificate_sha256",
    "terminal_receipt_sha256", "hardware_authorization_sha256", "operation_id",
    "inputs_digest", "preparation_id", "candidate_digest", "release_id", "outcome_id",
    "transition_nullifier", "envelope_digest", "terminal_receipt_digest", "authorization_id",
    "authorization_key_reference", "core_authorization_key_reference",
    "prepared_one_use_authorization_digest", "outbox_reservation_commitment",
    "hardware_one_use_nonce", "commit_certificate_digest", "network_id", "lane_commitment",
    "asset_id", "asset_incarnation", "suite_id", "vk_digest", "hardware_profile_id",
    "credential_id", "hardware_epoch_id", "device_key_reference", "hardware_policy_id",
    "commit_evidence_commitment",
)
SENDER_PROJECTION_FIELDS = (
    "schema", "schema_version", "operation_kind", "authorization_purpose", "receipt_kind",
    "structural_only", "operation", "protocol_version", "policy_epoch",
    "hardware_epoch_generation", "asset_scale", "artifact_manifest_digest",
    "commit_evidence_source", "payment_committed_at_ms", "request_sha256",
    "request_start_ms", "request_end_ms",
    *SENDER_PROJECTION_DIGEST_FIELDS,
)
SENDER_CASES = (
    "before_issuance", "trusted_valid", "lease_credential_straddle",
    "lease_empty", "lease_zero",
    "lease_before_credential", "lease_end",
    "trusted_before_expiry", "credential_expiry", "credential_after",
)
SENDER_OPERATIONS = ("send_split", "redeem_split")
SENDER_POSITIVE_CASES = frozenset({"trusted_valid", "lease_end", "trusted_before_expiry"})
SENDER_SNAPSHOT_FIELDS = (
    "state", "counter", "epoch", "authorization_counter", "lease_counter",
    "release_counter", "journal_revision", "outbox_revision", "outbox_digest",
)
CREDENTIAL_FIELDS = (
    "version", "credential_id", "network_id", "hardware_profile_id", "suite_id",
    "firmware_policy_digest", "policy_epoch", "lane_commitment", "hardware_epoch_id",
    "hardware_epoch_generation", "device_public_key", "device_key_reference",
    "issued_at_ms", "expires_at_ms", "governance_signature",
)
PHYSICAL_CHECKS = (
    "airplane_mode",
    "restart",
    "power_loss",
    "clock_rollback",
    "backup_restore_rejection",
    "memory_and_latency",
    "thermal_folding",
    "no_software_fallback",
)
PHYSICAL_ENDPOINT_KINDS = frozenset({"physical_secure_element"})
PHYSICAL_PLATFORM_CLASSES = frozenset(
    {
        "android_oem_service",
        "apple_oem_service",
        "dedicated_secure_element",
        "other_qualified",
    }
)
PHYSICAL_TRANSPORTS = frozenset({"secure_service", "usb", "nfc"})


class PhysicalDeviceEvidenceError(ValueError):
    """Raised when physical-device evidence is not valid qualification proof."""


def _fail(message: str) -> NoReturn:
    raise PhysicalDeviceEvidenceError(message)


def _exact_fields(value: Any, fields: Sequence[str], label: str) -> Mapping[str, Any]:
    if not isinstance(value, dict):
        _fail(f"{label} must be an object")
    expected = set(fields)
    actual = set(value)
    missing = sorted(expected - actual)
    unknown = sorted(actual - expected)
    if missing:
        _fail(f"{label} is missing fields: {', '.join(missing)}")
    if unknown:
        _fail(f"{label} has unknown fields: {', '.join(unknown)}")
    return value


def _array(value: Any, label: str) -> list[Any]:
    if not isinstance(value, list):
        _fail(f"{label} must be an array")
    return value


def _string(value: Any, label: str, *, allowed: frozenset[str] | None = None) -> str:
    if not isinstance(value, str) or not value:
        _fail(f"{label} must be a non-empty string")
    if len(value) > 128:
        _fail(f"{label} is too long")
    if allowed is not None and value not in allowed:
        _fail(f"{label} is not an allowed value")
    return value


def _digest(value: Any, label: str, *, allow_zero: bool = False) -> str:
    if not isinstance(value, str) or len(value) != 64:
        _fail(f"{label} must be a 32-byte lowercase hexadecimal digest")
    try:
        raw = bytes.fromhex(value)
    except ValueError:
        _fail(f"{label} must be a 32-byte lowercase hexadecimal digest")
    if value != raw.hex():
        _fail(f"{label} must use canonical lowercase hexadecimal")
    if not allow_zero and value == ZERO_DIGEST:
        _fail(f"{label} must not be the zero digest")
    return value


def _signature(value: Any, label: str) -> bytes:
    if not isinstance(value, str) or len(value) != 128:
        _fail(f"{label} must be a 64-byte lowercase hexadecimal signature")
    try:
        raw = bytes.fromhex(value)
    except ValueError:
        _fail(f"{label} must be a 64-byte lowercase hexadecimal signature")
    if value != raw.hex():
        _fail(f"{label} must use canonical lowercase hexadecimal")
    return raw


def _integer(value: Any, label: str, *, minimum: int = 0, maximum: int = U128_MAX) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        _fail(f"{label} must be an integer")
    if value < minimum or value > maximum:
        _fail(f"{label} must be between {minimum} and {maximum}")
    return value


def _boolean(value: Any, label: str) -> bool:
    if not isinstance(value, bool):
        _fail(f"{label} must be a boolean")
    return value


def _sha256(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


# Keep this stable profile preimage independent of the full transcript. The
# release manifest binds the raw transcript, OEM attestation and verifier report
# beside this report; including their hashes here would create a profile cycle.
def _report(provider_id: str, policy_epoch: int, run_id: str) -> dict[str, Any]:
    return {
        "schema": REPORT_SCHEMA,
        "schema_version": SCHEMA_VERSION,
        "verification_id": f"physical-{run_id}",
        "provider_id": provider_id,
        "policy_epoch": policy_epoch,
        "physical_checks": list(PHYSICAL_CHECKS),
        "passed": True,
    }


def _require_policy_admission(policy: release.TrustedObserverPolicy) -> None:
    """Require the observer policy to pin this exact verifier and report type."""

    try:
        source_info, _ = release.stable_read_path(
            Path(__file__), max_size=MAX_EVIDENCE_BYTES
        )
    except (
        release.KagemushaEvidenceError,
        release.ReleaseArtifactError,
        OSError,
        ValueError,
    ) as exc:
        _fail(f"failed to authenticate this physical-device verifier: {exc}")
    trusted = policy.verifiers.get(PHYSICAL_VERIFIER_ID)
    if (
        trusted is None
        or trusted.sha256 != source_info.sha256
        or REPORT_SCHEMA not in trusted.report_schemas
    ):
        _fail(
            "observer policy does not admit this physical-device verifier hash "
            "for the hardware qualification report schema"
        )


def _validate_profile(value: Any) -> Mapping[str, Any]:
    profile = _exact_fields(
        value,
        (
            "hardware_profile_id",
            "provider_id",
            "hardware_policy_id",
            "qualification_report_digest",
            "policy_epoch",
            "capability_mask",
        ),
        "profile",
    )
    for field in (
        "hardware_profile_id",
        "provider_id",
        "hardware_policy_id",
        "qualification_report_digest",
    ):
        _digest(profile[field], f"profile.{field}")
    _integer(profile["policy_epoch"], "profile.policy_epoch", minimum=1, maximum=(1 << 64) - 1)
    if _integer(profile["capability_mask"], "profile.capability_mask", maximum=0xFFFF) != 0xFFFF:
        _fail("profile.capability_mask must contain every KAGEMUSHA V1 capability bit")
    return profile


def _validate_endpoint(value: Any, profile: Mapping[str, Any]) -> Mapping[str, Any]:
    endpoint = _exact_fields(
        value,
        (
            "kind",
            "platform_class",
            "transport",
            "device_id",
            "product_id",
            "firmware_digest",
            "os_build_digest",
            "attestation_digest",
            "hardware_profile_id",
            "hardware_policy_id",
            "qualification_report_digest",
            "hardware_backed",
            "software_fallback",
            "production_build",
        ),
        "endpoint",
    )
    _string(endpoint["kind"], "endpoint.kind", allowed=PHYSICAL_ENDPOINT_KINDS)
    _string(
        endpoint["platform_class"],
        "endpoint.platform_class",
        allowed=PHYSICAL_PLATFORM_CLASSES,
    )
    _string(endpoint["transport"], "endpoint.transport", allowed=PHYSICAL_TRANSPORTS)
    for field in (
        "device_id",
        "product_id",
        "firmware_digest",
        "os_build_digest",
        "attestation_digest",
        "hardware_profile_id",
        "hardware_policy_id",
        "qualification_report_digest",
    ):
        _digest(endpoint[field], f"endpoint.{field}")
    for field in ("hardware_profile_id", "hardware_policy_id", "qualification_report_digest"):
        if endpoint[field] != profile[field]:
            _fail(f"endpoint.{field} does not match profile.{field}")
    if not _boolean(endpoint["hardware_backed"], "endpoint.hardware_backed"):
        _fail("endpoint must be hardware-backed")
    if _boolean(endpoint["software_fallback"], "endpoint.software_fallback"):
        _fail("endpoint advertises a software fallback")
    if not _boolean(endpoint["production_build"], "endpoint.production_build"):
        _fail("endpoint must use a production build")
    return endpoint


def _validate_run(value: Any) -> Mapping[str, Any]:
    run = _exact_fields(
        value,
        ("run_id", "candidate_digest", "candidate_context_digest", "artifact_set_digest", "started_at_ms", "ended_at_ms"),
        "run",
    )
    for field in ("run_id", "candidate_digest", "candidate_context_digest", "artifact_set_digest"):
        _digest(run[field], f"run.{field}")
    started = _integer(run["started_at_ms"], "run.started_at_ms", minimum=1, maximum=(1 << 64) - 1)
    ended = _integer(run["ended_at_ms"], "run.ended_at_ms", minimum=1, maximum=(1 << 64) - 1)
    if ended <= started:
        _fail("run.ended_at_ms must be after run.started_at_ms")
    return run


TRANSITION_FIELDS = (
    "operation_id",
    "predecessor",
    "successor",
    "counter_before",
    "counter_after",
    "epoch_before",
    "epoch_after",
    "artifact_sha256",
    "canonical_bytes_sha256",
    "result",
    "latency_ms",
    "rss_bytes",
    "energy_millijoules",
    "hardware_counter_before",
    "hardware_counter_after",
)
TRANSITION_KINDS = frozenset(
    {
        "prepare",
        "recover_prepare",
        "prove",
        "recover_prove",
        "candidate_persisted",
        "commit",
        "recover_commit",
        "advance_state",
        "epoch_rollover",
        "counter_rollover",
        "thermal_fold",
    }
)
CONTROL_PAIR_FIELDS = ("control_id", "boot_id")
EVENT_DATA_FIELDS: dict[str, tuple[str, ...]] = {
    "sender_validity_context": (
        "run_id", "candidate_context_digest", "artifact_set_digest", "device_id",
        "hardware_policy_id", "hardware_profile", "credentials", "vk_digest",
    ),
    "sender_admission_attempt": (
        "case", "operation_kind", "operation_id", "credential_id", "boot_id",
        "command_sha256", "preparation_sha256", "candidate_sha256", "request_sha256",
        "request_start_ms", "request_end_ms", "reservation_sha256",
        "reservation_start_ms", "reservation_end_ms", "source", "trusted_time_ms",
        "lease_start_ms", "lease_end_ms", "time_evidence_sha256", "before",
        "commit_evidence_commitment",
    ),
    "sender_admission_result": (
        "operation_id", "attempt_event_hash", "boot_id", "response_sha256", "after",
        "decision_trusted_time_ms", "result", "certificate_sha256", "envelope_sha256", "hardware_signature",
    ),
    "sender_historical_recovery": (
        "operation_id", "original_result_hash", "control_id", "prior_boot_id", "boot_id",
        "trusted_time_ms", "time_evidence_sha256", "before", "after", "replies",
        "receipt_kind", "receipt_sha256", "release_authorization_sha256", "hardware_signature",
        "native_parser_report", "service_release_observation",
    ),
    "run_start": ("boot_id", "initial_state", "counter", "epoch"),
    "airplane_mode_enabled": ("control_id",),
    "airplane_mode_disabled": ("control_id",),
    "network_probe": ("control_id", "tx_bytes", "rx_bytes", "result"),
    "operation_probe": (
        "operation",
        "request_id",
        "command_sha256",
        "response_sha256",
        "result",
        "latency_ms",
        "rss_bytes",
    ),
    "restart_begin": CONTROL_PAIR_FIELDS,
    "restart_end": ("control_id", "prior_boot_id", "new_boot_id"),
    "power_loss_begin": CONTROL_PAIR_FIELDS,
    "power_loss_end": ("control_id", "prior_boot_id", "new_boot_id"),
    "second_successor_rejected": (
        "operation_id",
        "predecessor",
        "attempted_successor",
        "committed_successor",
        "observed_state",
        "result",
    ),
    "stale_predecessor_rejected": (
        "operation_id",
        "predecessor",
        "observed_state",
        "result",
    ),
    "inbox_stage": (
        "credit_id",
        "canonical_bytes_sha256",
        "receipt_sha256",
        "inbox_revision",
        "result",
        "latency_ms",
        "rss_bytes",
    ),
    "inbox_recover": (
        "credit_id",
        "canonical_bytes_sha256",
        "receipt_sha256",
        "inbox_revision",
        "result",
        "latency_ms",
        "rss_bytes",
    ),
    "outbox_install": (
        "operation_id",
        "canonical_bytes_sha256",
        "certificate_sha256",
        "outbox_revision",
        "result",
        "latency_ms",
        "rss_bytes",
    ),
    "outbox_recover": (
        "operation_id",
        "canonical_bytes_sha256",
        "certificate_sha256",
        "outbox_revision",
        "result",
        "latency_ms",
        "rss_bytes",
    ),
    "backup_snapshot": ("control_id", "state", "counter", "epoch", "snapshot_sha256"),
    "clock_rollback_begin": (
        "control_id", "boot_id", "state", "counter", "epoch",
        "host_time_ms", "trusted_time_ms", "request_expires_at_ms", "request_sha256",
    ),
    "clock_rollback_applied": (
        "control_id", "boot_id", "host_time_ms", "trusted_time_ms",
    ),
    "expired_request_rejected": (
        "control_id", "boot_id", "operation_id", "request_sha256",
        "authoritative_state", "counter", "epoch", "host_time_ms", "trusted_time_ms",
        "result",
    ),
    "clock_rollback_end": (
        "control_id", "boot_id", "host_time_ms", "trusted_time_ms",
    ),
    "backup_restore_attempt": (
        "control_id",
        "snapshot_sha256",
        "snapshot_state",
        "authoritative_state",
        "counter",
        "epoch",
        "result",
    ),
    "thermal_start": ("control_id", "sensor_digest"),
    "thermal_end": ("control_id", "sensor_digest"),
    "software_fallback_probe": (
        "control_id",
        "requested_backend",
        "observed_state",
        "result",
    ),
    "run_end": ("boot_id", "final_state", "counter", "epoch"),
}
for _transition_kind in TRANSITION_KINDS:
    EVENT_DATA_FIELDS[_transition_kind] = TRANSITION_FIELDS


def _validate_metric_data(data: Mapping[str, Any], label: str, metrics: list[tuple[int, int]]) -> None:
    latency = _integer(data["latency_ms"], f"{label}.latency_ms", minimum=1, maximum=(1 << 32) - 1)
    rss = _integer(data["rss_bytes"], f"{label}.rss_bytes", minimum=1, maximum=(1 << 64) - 1)
    metrics.append((latency, rss))


def _validate_transition_data(
    data: Mapping[str, Any], label: str, kind: str, metrics: list[tuple[int, int]]
) -> None:
    for field in ("operation_id", "predecessor", "successor", "artifact_sha256", "canonical_bytes_sha256"):
        _digest(data[field], f"{label}.{field}")
    if data["predecessor"] == data["successor"]:
        _fail(f"{label} must have distinct predecessor and successor states")
    before = _integer(data["counter_before"], f"{label}.counter_before")
    after = _integer(data["counter_after"], f"{label}.counter_after")
    epoch_before = _integer(data["epoch_before"], f"{label}.epoch_before", minimum=1, maximum=(1 << 64) - 1)
    epoch_after = _integer(data["epoch_after"], f"{label}.epoch_after", minimum=1, maximum=(1 << 64) - 1)
    if data["result"] != "success":
        _fail(f"{label}.result must be success")
    _validate_metric_data(data, label, metrics)
    energy = _integer(data["energy_millijoules"], f"{label}.energy_millijoules", maximum=(1 << 64) - 1)
    hardware_before = _integer(data["hardware_counter_before"], f"{label}.hardware_counter_before")
    hardware_after = _integer(data["hardware_counter_after"], f"{label}.hardware_counter_after")
    if kind == "thermal_fold":
        if energy == 0:
            _fail(f"{label}.energy_millijoules must be positive")
    elif energy != 0:
        _fail(f"{label}.energy_millijoules must be zero outside the thermal segment")
    if kind == "epoch_rollover":
        if epoch_after != epoch_before + 1 or after != before + 1 or hardware_before or hardware_after:
            _fail(f"{label} must advance one logical counter and exactly one epoch")
    elif kind == "counter_rollover":
        if (
            after != before + 1
            or hardware_before != U128_MAX
            or hardware_after != 1
            or epoch_after != epoch_before + 1
        ):
            _fail(f"{label} must roll an exhausted counter into exactly the next epoch")
    else:
        if hardware_before or hardware_after:
            _fail(f"{label} must not claim a hardware-counter rollover")
        if after != before + 1 or epoch_after != epoch_before:
            _fail(f"{label} must describe an exact-next transition")


def _validate_event_data(
    kind: str, value: Any, label: str, metrics: list[tuple[int, int]]
) -> Mapping[str, Any]:
    fields = EVENT_DATA_FIELDS.get(kind)
    if fields is None:
        _fail(f"{label} has unsupported kind {kind!r}")
    data = _exact_fields(value, fields, f"{label}.data")
    # The sender segment has nested closed records and context-dependent authority;
    # validate those together after checking the enclosing hash-chain bytes.
    if kind.startswith("sender_"):
        return data
    if kind in TRANSITION_KINDS:
        _validate_transition_data(data, f"{label}.data", kind, metrics)
    elif kind == "run_start":
        _digest(data["boot_id"], f"{label}.data.boot_id")
        _digest(data["initial_state"], f"{label}.data.initial_state")
        _integer(data["counter"], f"{label}.data.counter")
        _integer(data["epoch"], f"{label}.data.epoch", minimum=1, maximum=(1 << 64) - 1)
    elif kind in {"airplane_mode_enabled", "airplane_mode_disabled"}:
        _digest(data["control_id"], f"{label}.data.control_id")
    elif kind == "network_probe":
        _digest(data["control_id"], f"{label}.data.control_id")
        _integer(data["tx_bytes"], f"{label}.data.tx_bytes", maximum=(1 << 64) - 1)
        _integer(data["rx_bytes"], f"{label}.data.rx_bytes", maximum=(1 << 64) - 1)
        if data["result"] != "isolated":
            _fail(f"{label}.data.result must be isolated")
    elif kind == "operation_probe":
        _integer(data["operation"], f"{label}.data.operation", minimum=1, maximum=22)
        for field in ("request_id", "command_sha256", "response_sha256"):
            _digest(data[field], f"{label}.data.{field}")
        if data["result"] != "authenticated":
            _fail(f"{label}.data.result must be authenticated")
        _validate_metric_data(data, f"{label}.data", metrics)
    elif kind in {"restart_begin", "power_loss_begin"}:
        _digest(data["control_id"], f"{label}.data.control_id")
        _digest(data["boot_id"], f"{label}.data.boot_id")
    elif kind in {"restart_end", "power_loss_end"}:
        for field in ("control_id", "prior_boot_id", "new_boot_id"):
            _digest(data[field], f"{label}.data.{field}")
        if data["prior_boot_id"] == data["new_boot_id"]:
            _fail(f"{label} must observe a new hardware boot identifier")
    elif kind == "second_successor_rejected":
        for field in (
            "operation_id",
            "predecessor",
            "attempted_successor",
            "committed_successor",
            "observed_state",
        ):
            _digest(data[field], f"{label}.data.{field}")
        if data["result"] != "rejected":
            _fail(f"{label}.data.result must be rejected")
    elif kind == "stale_predecessor_rejected":
        for field in ("operation_id", "predecessor", "observed_state"):
            _digest(data[field], f"{label}.data.{field}")
        if data["result"] != "rejected":
            _fail(f"{label}.data.result must be rejected")
    elif kind in {"inbox_stage", "inbox_recover"}:
        for field in ("credit_id", "canonical_bytes_sha256", "receipt_sha256"):
            _digest(data[field], f"{label}.data.{field}")
        _integer(data["inbox_revision"], f"{label}.data.inbox_revision", minimum=1)
        if data["result"] != "durable":
            _fail(f"{label}.data.result must be durable")
        _validate_metric_data(data, f"{label}.data", metrics)
    elif kind in {"outbox_install", "outbox_recover"}:
        for field in ("operation_id", "canonical_bytes_sha256", "certificate_sha256"):
            _digest(data[field], f"{label}.data.{field}")
        _integer(data["outbox_revision"], f"{label}.data.outbox_revision", minimum=1)
        if data["result"] != "durable":
            _fail(f"{label}.data.result must be durable")
        _validate_metric_data(data, f"{label}.data", metrics)
    elif kind in {
        "clock_rollback_begin", "clock_rollback_applied",
        "expired_request_rejected", "clock_rollback_end",
    }:
        for field in ("control_id", "boot_id"):
            _digest(data[field], f"{label}.data.{field}")
        for field in ("host_time_ms", "trusted_time_ms"):
            _integer(data[field], f"{label}.data.{field}", minimum=1, maximum=(1 << 64) - 1)
        if kind in {"clock_rollback_begin", "expired_request_rejected"}:
            _digest(data["request_sha256"], f"{label}.data.request_sha256")
            _integer(data["counter"], f"{label}.data.counter")
            _integer(data["epoch"], f"{label}.data.epoch", minimum=1, maximum=(1 << 64) - 1)
        if kind == "clock_rollback_begin":
            _digest(data["state"], f"{label}.data.state")
            _integer(data["request_expires_at_ms"], f"{label}.data.request_expires_at_ms", minimum=1, maximum=(1 << 64) - 1)
        elif kind == "expired_request_rejected":
            _digest(data["operation_id"], f"{label}.data.operation_id")
            _digest(data["authoritative_state"], f"{label}.data.authoritative_state")
            if data["result"] != "expired_request_rejected":
                _fail(f"{label}.data.result must be expired_request_rejected")
    elif kind == "backup_snapshot":
        for field in ("control_id", "state", "snapshot_sha256"):
            _digest(data[field], f"{label}.data.{field}")
        _integer(data["counter"], f"{label}.data.counter")
        _integer(data["epoch"], f"{label}.data.epoch", minimum=1, maximum=(1 << 64) - 1)
    elif kind == "backup_restore_attempt":
        for field in ("control_id", "snapshot_sha256", "snapshot_state", "authoritative_state"):
            _digest(data[field], f"{label}.data.{field}")
        _integer(data["counter"], f"{label}.data.counter")
        _integer(data["epoch"], f"{label}.data.epoch", minimum=1, maximum=(1 << 64) - 1)
        if data["result"] != "rollback_rejected":
            _fail(f"{label}.data.result must be rollback_rejected")
    elif kind in {"thermal_start", "thermal_end"}:
        _digest(data["control_id"], f"{label}.data.control_id")
        _digest(data["sensor_digest"], f"{label}.data.sensor_digest")
    elif kind == "software_fallback_probe":
        _digest(data["control_id"], f"{label}.data.control_id")
        _digest(data["observed_state"], f"{label}.data.observed_state")
        if data["requested_backend"] != "software" or data["result"] != "rejected":
            _fail(f"{label} must prove rejection of the software backend")
    elif kind == "run_end":
        _digest(data["boot_id"], f"{label}.data.boot_id")
        _digest(data["final_state"], f"{label}.data.final_state")
        _integer(data["counter"], f"{label}.data.counter")
        _integer(data["epoch"], f"{label}.data.epoch", minimum=1, maximum=(1 << 64) - 1)
    return data


def _validate_events(
    value: Any, run: Mapping[str, Any]
) -> tuple[list[Mapping[str, Any]], list[tuple[int, int]]]:
    raw_events = _array(value, "events")
    if not raw_events or len(raw_events) > MAX_EVENTS:
        _fail(f"events must contain between 1 and {MAX_EVENTS} entries")
    events: list[Mapping[str, Any]] = []
    metrics: list[tuple[int, int]] = []
    previous_hash = ZERO_DIGEST
    seen_hashes: set[str] = set()
    previous_time = int(run["started_at_ms"])
    for index, raw in enumerate(raw_events):
        label = f"events[{index}]"
        event = _exact_fields(raw, ("index", "kind", "observed_at_ms", "previous_hash", "data", "event_hash"), label)
        if _integer(event["index"], f"{label}.index", maximum=MAX_EVENTS) != index:
            _fail(f"{label}.index must be the contiguous canonical event index")
        kind = _string(event["kind"], f"{label}.kind")
        observed = _integer(
            event["observed_at_ms"], f"{label}.observed_at_ms", minimum=1, maximum=(1 << 64) - 1
        )
        if observed < previous_time or observed > run["ended_at_ms"]:
            _fail(f"{label}.observed_at_ms is outside the monotonic run interval")
        previous_time = observed
        if _digest(event["previous_hash"], f"{label}.previous_hash", allow_zero=True) != previous_hash:
            _fail(f"{label}.previous_hash does not extend the canonical hash chain")
        event_hash = _digest(event["event_hash"], f"{label}.event_hash")
        unhashed = {key: event[key] for key in ("index", "kind", "observed_at_ms", "previous_hash", "data")}
        expected_hash = _sha256(EVENT_HASH_DOMAIN + b"\0" + release.canonical_json_bytes(unhashed))
        if event_hash != expected_hash:
            _fail(f"{label}.event_hash does not match its canonical event bytes")
        if event_hash in seen_hashes:
            _fail(f"{label}.event_hash replays an earlier event")
        seen_hashes.add(event_hash)
        previous_hash = event_hash
        _validate_event_data(kind, event["data"], label, metrics)
        events.append(event)
    return events, metrics


def _sender_sequence() -> list[str]:
    """Return the mandatory V1 sender-admission and historical-recovery segment."""
    return ["sender_validity_context"] + [
        kind for _case in SENDER_CASES for _operation in SENDER_OPERATIONS
        for kind in ("sender_admission_attempt", "sender_admission_result")
    ] + ["sender_historical_recovery"] * (len(SENDER_POSITIVE_CASES) * len(SENDER_OPERATIONS))


def _credential_payload(credential: Mapping[str, Any]) -> bytes:
    """Encode the exact existing Rust compact credential ID preimage payload."""
    c = credential
    return release._norito_struct(
        release._u16(c["version"]), bytes.fromhex(c["network_id"]),
        bytes.fromhex(c["hardware_profile_id"]), bytes.fromhex(c["suite_id"]),
        bytes.fromhex(c["firmware_policy_digest"]), release._u64(c["policy_epoch"]),
        bytes.fromhex(c["lane_commitment"]), bytes.fromhex(c["hardware_epoch_id"]),
        release._u64(c["hardware_epoch_generation"]), bytes.fromhex(c["device_public_key"]),
        bytes.fromhex(c["device_key_reference"]), release._u64(c["issued_at_ms"]),
        release._u64(c["expires_at_ms"]),
    )


def credential_identity(credential: Mapping[str, Any]) -> str:
    """Reconstruct Rust's SHA-bound canonical credential identity."""
    return release._rust_digest(
        b"iroha:kagemusha:v1:hardware-credential-id",
        "iroha.kagemusha.v1.hardware-credential-id-preimage", _credential_payload(credential),
    )


def credential_signing_bytes(credential: Mapping[str, Any]) -> bytes:
    """Reconstruct the existing issuer-signed Norito credential subject."""
    domain = b"iroha:kagemusha:v1:hardware-credential-signing"
    return release._norito_frame(
        "iroha.kagemusha.v1.hardware-credential-signing-preimage",
        release._norito_struct(
            release._u64(len(domain)) + domain,
            bytes.fromhex(credential["credential_id"]), _credential_payload(credential),
        ),
    )


def sender_evidence_signing_bytes(context_hash: str, kind: str, data: Mapping[str, Any]) -> bytes:
    """Bind a hardware observation to the exact observer-approved context and event kind.

    This evidence-only subject is not a device command or commit authorization.
    The retained device authenticator supplements the threshold observer chain.
    """
    subject = {
        "context_event_hash": context_hash, "kind": kind,
        "data": {key: value for key, value in data.items() if key != "hardware_signature"},
    }
    encoded = release.canonical_json_bytes(subject)
    return SENDER_EVIDENCE_DOMAIN + len(encoded).to_bytes(8, "little") + encoded


def _sender_context(
    event: Mapping[str, Any], document: Mapping[str, Any],
) -> tuple[Mapping[str, Any], list[Mapping[str, Any]]]:
    data = event["data"]
    _digest(data["vk_digest"], "sender context VK digest")
    for field in ("run_id", "candidate_context_digest", "artifact_set_digest"):
        if data[field] != document["run"][field]:
            _fail("sender context substitutes the authenticated run or release")
    for field, expected in (
        ("device_id", document["endpoint"]["device_id"]),
        ("hardware_policy_id", document["profile"]["hardware_policy_id"]),
    ):
        if data[field] != expected:
            _fail("sender context substitutes the authenticated device or policy")
    profile = _exact_fields(data["hardware_profile"], release._HARDWARE_PROFILE_FIELDS, "sender profile")
    try:
        identity = release.rust_hardware_profile_id(profile)
    except (ValueError, OverflowError) as exc:
        _fail(f"invalid sender profile: {exc}")
    for field in ("hardware_profile_id", "provider_id", "qualification_report_digest", "policy_epoch", "capability_mask"):
        if profile[field] != document["profile"][field]:
            _fail("sender profile differs from the exact qualified profile")
    if identity != profile["hardware_profile_id"] or profile["platform_class"] != document["endpoint"]["platform_class"]:
        _fail("sender profile canonical identity or platform mismatch")
    start = _integer(profile["valid_from_ms"], "sender profile.valid_from_ms", maximum=U64_MAX)
    end = _integer(profile["expires_at_ms"], "sender profile.expires_at_ms", minimum=1, maximum=U64_MAX)
    run = document["run"]
    if not (start <= run["started_at_ms"] < run["ended_at_ms"] <= end):
        _fail("sender qualification run must remain inside the active governed profile")
    credentials = _array(data["credentials"], "sender credentials")
    if len(credentials) != 1:
        _fail("sender context requires exactly one early-expiring qualified credential")
    for raw in credentials:
        c = _exact_fields(raw, CREDENTIAL_FIELDS, "sender credential")
        for field in (
            "credential_id", "network_id", "hardware_profile_id", "suite_id",
            "firmware_policy_digest", "lane_commitment", "hardware_epoch_id", "device_key_reference",
        ):
            _digest(c[field], f"sender credential.{field}")
        _integer(c["version"], "sender credential.version", minimum=1, maximum=1)
        for field in ("policy_epoch", "hardware_epoch_generation", "issued_at_ms", "expires_at_ms"):
            _integer(c[field], f"sender credential.{field}", maximum=U64_MAX)
        try:
            key = release._device_public_key(c["device_public_key"], "sender device key")
            issuer = release._device_public_key(profile["governance_credential_public_key"], "sender profile issuer")
        except ValueError as exc:
            _fail(str(exc))
        if (
            c["hardware_profile_id"] != identity
            or c["policy_epoch"] != profile["policy_epoch"]
            or c["firmware_policy_digest"] != profile["firmware_policy_digest"]
            or release._suite_commitment(c["suite_id"]) != profile["allowed_suite_commitment"]
            or not (start <= c["issued_at_ms"] < c["expires_at_ms"] <= end)
            or c["device_key_reference"] != _sha256(b"iroha:kagemusha:v1:device-key-reference\0" + key)
            or credential_identity(c) != c["credential_id"]
        ):
            _fail("sender credential identity, lifetime or exact profile binding mismatch")
        signature = _signature(c["governance_signature"], "sender governance signature")
        if not release._p256_verify(issuer, credential_signing_bytes(c), signature):
            _fail("sender credential issuer signature is invalid")
    if not (credentials[0]["expires_at_ms"] < run["ended_at_ms"] <= end):
        _fail("sender credential does not support expiry testing within the active profile")
    return profile, credentials


def _sender_snapshot(raw: Any) -> Mapping[str, Any]:
    snapshot = _exact_fields(raw, SENDER_SNAPSHOT_FIELDS, "sender authoritative snapshot")
    for field in SENDER_SNAPSHOT_FIELDS:
        if field in {"state", "outbox_digest"}:
            _digest(snapshot[field], f"sender snapshot.{field}")
        else:
            _integer(snapshot[field], f"sender snapshot.{field}", minimum=1 if field == "epoch" else 0)
    return snapshot


def _sender_signature(context: Mapping[str, Any], kind: str, data: Mapping[str, Any], key: str) -> None:
    signature = _signature(data["hardware_signature"], "sender hardware signature")
    if not release._p256_verify(bytes.fromhex(key), sender_evidence_signing_bytes(context["event_hash"], kind, data), signature):
        _fail("sender hardware observation signature is invalid")


def _sender_time_case(case: str, decision: int, issued: int, expires: int) -> bool:
    """Admit realizable physical time intervals with inclusive/exclusive endpoints."""
    if case == "before_issuance":
        return 0 < decision < issued
    if case == "credential_expiry":
        return decision >= expires
    if case == "credential_after":
        return decision > expires
    return issued <= decision < expires


def sender_parser_approval_message(report: Mapping[str, Any], policy: release.TrustedObserverPolicy) -> bytes:
    """Bind an independently observed native parse to its exact evidence role and policy."""
    subject = {
        "observer_policy_sha256": policy.info.sha256,
        "report": {key: value for key, value in report.items() if key != "approvals"},
    }
    return release._approval_message(subject)


def _sender_parser_report(
    raw: Any, document: Mapping[str, Any], context: Mapping[str, Any],
    attempt: Mapping[str, Any], result: Mapping[str, Any], recovery: Mapping[str, Any],
    credential: Mapping[str, Any], policy: release.TrustedObserverPolicy,
) -> Mapping[str, Any]:
    """Authenticate a pinned parser observation; never execute a supplied program.

    This establishes structural public-byte bindings only. The separate service
    observation must attest actual release-authority consumption, including the
    in-process finalized redemption capability when required.
    """
    report = _exact_fields(raw, (
        "schema", "schema_version", "purpose", "verifier_id", "verifier_sha256",
        "source_id", "source_sha256", "run_id", "candidate_context_digest",
        "artifact_set_digest", "device_id", "sender_context_event_hash",
        "command_hex", "projection", "approvals",
    ), "sender parser report")
    _integer(report["schema_version"], "sender parser schema version", minimum=1, maximum=1)
    if (report["schema"], report["schema_version"], report["purpose"], report["verifier_id"], report["source_id"]) != (
        SENDER_PARSER_SCHEMA, 1, SENDER_PARSER_PURPOSE, SENDER_PARSER_ID, SENDER_PARSER_SOURCE_ID,
    ) or isinstance(report["schema_version"], bool):
        _fail("sender parser observation substitutes its schema, purpose or verifier role")
    for identity, field in ((SENDER_PARSER_ID, "verifier_sha256"), (SENDER_PARSER_SOURCE_ID, "source_sha256")):
        trusted = policy.verifiers.get(identity)
        if trusted is None or trusted.sha256 != _digest(report[field], f"sender parser {field}") or SENDER_PARSER_SCHEMA not in trusted.report_schemas:
            _fail("observer policy does not admit the exact native sender parser binary and source")
    try:
        source_info, _ = release.stable_read_path(SENDER_PARSER_SOURCE, max_size=MAX_EVIDENCE_BYTES)
    except (release.KagemushaEvidenceError, OSError, ValueError) as error:
        _fail(f"cannot authenticate native sender parser source: {error}")
    if source_info.sha256 != report["source_sha256"]:
        _fail("native sender parser source differs from the independently pinned source")
    expected_context = {
        **{key: document["run"][key] for key in ("run_id", "candidate_context_digest", "artifact_set_digest")},
        "device_id": document["endpoint"]["device_id"], "sender_context_event_hash": context["event_hash"],
    }
    if any(report[key] != value for key, value in expected_context.items()):
        _fail("sender parser observation substitutes its qualified run or device context")
    command_hex = report["command_hex"]
    if not isinstance(command_hex, str) or not (0 < len(command_hex) <= SENDER_COMMAND_MAX_BYTES * 2):
        _fail("sender parser command must be a bounded canonical payload")
    try:
        command = bytes.fromhex(command_hex)
    except ValueError:
        _fail("sender parser command is not canonical hexadecimal")
    if command.hex() != command_hex:
        _fail("sender parser command is not canonical lowercase hexadecimal")
    projection = _exact_fields(report["projection"], SENDER_PROJECTION_FIELDS, "sender native projection")
    for field in SENDER_PROJECTION_DIGEST_FIELDS:
        _digest(projection[field], f"sender projection.{field}")
    for field, maximum in (("schema_version", 1), ("operation", 12), ("protocol_version", 1), ("policy_epoch", U64_MAX), ("hardware_epoch_generation", U128_MAX), ("asset_scale", 255)):
        _integer(projection[field], f"sender projection.{field}", maximum=maximum)
    if projection["structural_only"] is not True:
        _fail("sender parser output cannot grant release or finalized receipt authority")
    a, r = attempt["data"], result["data"]
    if a["operation_kind"] == "send_split":
        _digest(projection["request_sha256"], "sender projection request")
        for field in ("request_start_ms", "request_end_ms"):
            _integer(projection[field], f"sender projection.{field}", maximum=U64_MAX)
    if projection["payment_committed_at_ms"] is not None:
        _integer(projection["payment_committed_at_ms"], "sender projection payment time", minimum=1, maximum=U64_MAX)
    # This is the loaded release manifest selector, not the candidate artifact
    # set digest. The public parser cannot establish catalog admission for it.
    if a["operation_kind"] == "send_split":
        if projection["artifact_manifest_digest"] is not None:
            _fail("sender payment projection invents a manifest field absent from V1")
    else:
        _digest(projection["artifact_manifest_digest"], "sender redemption manifest selector")
    expected = {
        "schema": SENDER_PARSER_SCHEMA, "schema_version": 1, "operation": 12,
        "protocol_version": 1, "authorization_purpose": "release",
        "operation_kind": a["operation_kind"], "operation_id": a["operation_id"],
        "command_sha256": _sha256(command), "preparation_id": a["preparation_sha256"],
        "candidate_digest": a["candidate_sha256"], "certificate_sha256": r["certificate_sha256"],
        "envelope_sha256": r["envelope_sha256"], "terminal_receipt_sha256": recovery["receipt_sha256"],
        "hardware_authorization_sha256": recovery["release_authorization_sha256"],
        "hardware_policy_id": document["profile"]["hardware_policy_id"],
        "vk_digest": context["data"]["vk_digest"],
        "commit_evidence_source": a["source"],
        "commit_evidence_commitment": a["commit_evidence_commitment"],
        "payment_committed_at_ms": r["decision_trusted_time_ms"] if a["operation_kind"] == "send_split" else None,
        **{key: a[key] if a["operation_kind"] == "send_split" else None for key in ("request_sha256", "request_start_ms", "request_end_ms")},
        **{key: credential[key] for key in (
            "credential_id", "hardware_profile_id", "suite_id", "network_id", "lane_commitment",
            "hardware_epoch_id", "hardware_epoch_generation", "device_key_reference", "policy_epoch",
        )},
        "receipt_kind": "payment_acknowledgement" if a["operation_kind"] == "send_split" else "finalized_redemption_selector",
    }
    if release.canonical_json_bytes({key: projection[key] for key in expected}) != release.canonical_json_bytes(expected):
        _fail("sender native projection substitutes its exact command, receipt, authorization or retained context")
    if projection["authorization_key_reference"] != projection["core_authorization_key_reference"]:
        _fail("sender release authorization key differs from its decoded context")
    approvals = _array(report["approvals"], "sender parser approvals")
    if len(approvals) > len(policy.authorities):
        _fail("sender parser approvals exceed the observer policy")
    message = sender_parser_approval_message(report, policy)
    previous = ""
    verified = 0
    for raw_approval in approvals:
        approval = _exact_fields(raw_approval, ("authority_id", "signature"), "sender parser approval")
        authority_id = _digest(approval["authority_id"], "sender parser observer")
        if authority_id <= previous or authority_id not in policy.authorities:
            _fail("sender parser observers must be trusted and uniquely sorted")
        previous = authority_id
        if not release._ed25519_verify(policy.authorities[authority_id], message, _signature(approval["signature"], "sender parser approval signature")):
            _fail("sender parser observation has an invalid independent observer signature")
        verified += 1
    if verified < policy.threshold:
        _fail("sender parser observation does not meet the independent observer threshold")
    return projection


def _derive_sender_checks(
    document: Mapping[str, Any], events: Sequence[Mapping[str, Any]],
    state: str, counter: int, epoch: int, boot_id: str, policy: release.TrustedObserverPolicy,
) -> tuple[str, int, int, str]:
    """Check atomic nonmutation, bounded successes and historical exact-once recovery."""
    segment = [event for event in events if event["kind"].startswith("sender_")]
    context = segment[0]
    profile, (short,) = _sender_context(context, document)
    run = document["run"]
    prefix = events[:context["index"]]
    previous_time = max(run["started_at_ms"], *(event["data"].get("trusted_time_ms", 0) for event in prefix))
    snapshot = None
    used_operations = {event["data"]["operation_id"] for event in events if not event["kind"].startswith("sender_") and "operation_id" in event["data"]}
    used_digests: set[str] = set()
    committed: list[tuple[Mapping[str, Any], Mapping[str, Any]]] = []
    cursor = 1

    def unique(value: Any, label: str) -> None:
        digest = _digest(value, label)
        if digest in used_digests:
            _fail("sender evidence reuses a command, response, preparation or authority observation")
        used_digests.add(digest)

    for case in SENDER_CASES:
        for operation in SENDER_OPERATIONS:
            attempt, result = segment[cursor:cursor + 2]
            cursor += 2
            a, r = attempt["data"], result["data"]
            credential = short
            if a["case"] != case or a["operation_kind"] != operation or a["credential_id"] != credential["credential_id"]:
                _fail("sender scenario substitutes its required operation, case or exact credential")
            operation_id = _digest(a["operation_id"], "sender operation_id")
            if operation_id in used_operations:
                _fail("sender operation identifier is reused")
            used_operations.add(operation_id)
            for field in ("command_sha256", "preparation_sha256", "candidate_sha256", "reservation_sha256", "time_evidence_sha256", "commit_evidence_commitment"):
                unique(a[field], f"sender {field}")
            unique(r["response_sha256"], "sender response")
            if a["boot_id"] != boot_id or r["boot_id"] != boot_id or r["operation_id"] != operation_id or r["attempt_event_hash"] != attempt["event_hash"]:
                _fail("sender result is detached from its attempt or active hardware boot")
            before, after = _sender_snapshot(a["before"]), _sender_snapshot(r["after"])
            if snapshot is None:
                if (before["state"], before["counter"], before["epoch"]) != (state, counter, epoch):
                    _fail("sender segment is detached from the authoritative state chain")
                if short["hardware_epoch_generation"] != epoch:
                    _fail("sender credential does not bind the authoritative hardware epoch")
                snapshot = before
            if before != snapshot:
                _fail("sender attempt does not continue the authoritative state and counters")
            for field in ("trusted_time_ms", "lease_start_ms", "lease_end_ms", "request_start_ms", "request_end_ms", "reservation_start_ms", "reservation_end_ms"):
                _integer(a[field], f"sender {field}", maximum=U64_MAX)
            sample = a["trusted_time_ms"]
            now = _integer(r["decision_trusted_time_ms"], "sender decision trusted time", minimum=1, maximum=U64_MAX)
            if not (previous_time <= sample <= now <= result["observed_at_ms"] <= run["ended_at_ms"] and sample <= attempt["observed_at_ms"] and profile["valid_from_ms"] <= now < profile["expires_at_ms"]):
                _fail("sender authoritative time is nonmonotonic or outside the active observed run")
            if result["observed_at_ms"] - now > MAX_LATENCY_MS or attempt["observed_at_ms"] - sample > MAX_LATENCY_MS:
                _fail("sender hardware time is a stale observation")
            previous_time = now
            if not (a["reservation_start_ms"] <= now < a["reservation_end_ms"]):
                _fail("sender reservation must remain valid during the admission attempt")
            _digest(a["request_sha256"], "sender request", allow_zero=operation == "redeem_split")
            if operation == "send_split":
                if not (a["request_start_ms"] <= now < a["request_end_ms"] and a["request_end_ms"] - a["request_start_ms"] <= 300_000):
                    _fail("sender request must remain valid during the admission attempt")
            elif (a["request_sha256"], a["request_start_ms"], a["request_end_ms"]) != (ZERO_DIGEST, 0, 0):
                _fail("redemption admission must not substitute a receiver request")
            issued, expires = short["issued_at_ms"], short["expires_at_ms"]
            if not _sender_time_case(case, now, issued, expires):
                _fail("sender case does not exercise its required time boundary")
            is_lease = case.startswith("lease_")
            expected_lease = {
                "lease_credential_straddle": (issued, expires + 1),
                "lease_empty": (issued, issued), "lease_zero": (0, expires),
                "lease_before_credential": (issued - 1, expires),
                "lease_end": (issued, expires),
            }.get(case, (0, 0))
            if a["source"] != ("monotonic_lease" if is_lease else "trusted_time") or (a["lease_start_ms"], a["lease_end_ms"]) != expected_lease:
                _fail("sender case does not exercise the exact whole-lease boundary")
            if case in {"lease_credential_straddle", "lease_before_credential", "lease_end"}:
                if not (a["reservation_start_ms"] <= a["lease_start_ms"] < a["lease_end_ms"] <= a["reservation_end_ms"]):
                    _fail("sender lease case must keep the entire reservation window valid")
                if operation == "send_split" and not (a["request_start_ms"] <= a["lease_start_ms"] < a["lease_end_ms"] <= a["request_end_ms"]):
                    _fail("sender lease case must keep the entire receiver request window valid")
            # A valid sample inside the credential is deliberately insufficient
            # for a lease whose complete window crosses either authenticated bound.
            positive = case in SENDER_POSITIVE_CASES
            if r["result"] != ("committed" if positive else "rejected"):
                _fail("sender admission result does not match the required case")
            for field in ("certificate_sha256", "envelope_sha256"):
                _digest(r[field], f"sender {field}", allow_zero=not positive)
            if positive:
                unique(r["certificate_sha256"], "sender terminal certificate")
                unique(r["envelope_sha256"], "sender terminal envelope")
                for field in SENDER_SNAPSHOT_FIELDS:
                    delta = int(field in {"counter", "authorization_counter", "journal_revision", "outbox_revision"} or (field == "lease_counter" and is_lease))
                    if field in {"state", "outbox_digest"}:
                        if after[field] == before[field]:
                            _fail("sender valid commit must install one successor and terminal outbox entry")
                    elif after[field] != before[field] + delta:
                        _fail("sender valid commit must advance exactly its permitted counters")
                committed.append((attempt, result))
            elif after != before or r["certificate_sha256"] != ZERO_DIGEST or r["envelope_sha256"] != ZERO_DIGEST:
                _fail("rejected sender commit changed authoritative state, counters or outbox")
            _sender_signature(context, result["kind"], r, credential["device_public_key"])
            snapshot = after

    controls = {event["data"]["control_id"] for event in prefix if "control_id" in event["data"]}
    boots = {event["data"][field] for event in prefix for field in ("boot_id", "prior_boot_id", "new_boot_id") if field in event["data"]}
    for attempt, original in committed:
        recovery = segment[cursor]
        cursor += 1
        data = recovery["data"]
        a, r = attempt["data"], original["data"]
        if data["operation_id"] != a["operation_id"] or data["original_result_hash"] != original["event_hash"]:
            _fail("sender recovery substitutes its historical operation or certificate")
        control = _digest(data["control_id"], "sender recovery control")
        new_boot = _digest(data["boot_id"], "sender recovery boot")
        if control in controls or new_boot in boots or data["prior_boot_id"] != boot_id:
            _fail("sender recovery must prove a fresh restart of the active hardware boot")
        controls.add(control)
        boots.add(new_boot)
        boot_id = new_boot
        now = _integer(data["trusted_time_ms"], "sender recovery trusted time", maximum=U64_MAX)
        if not (short["expires_at_ms"] < now < profile["expires_at_ms"] and previous_time <= now <= recovery["observed_at_ms"] <= run["ended_at_ms"] <= profile["expires_at_ms"]):
            _fail("sender historical recovery must occur after credential expiry inside the active profile")
        if recovery["observed_at_ms"] - now > MAX_LATENCY_MS:
            _fail("sender historical recovery uses stale hardware time")
        previous_time = now
        unique(data["time_evidence_sha256"], "sender recovery time evidence")
        before, after = _sender_snapshot(data["before"]), _sender_snapshot(data["after"])
        if before != snapshot:
            _fail("sender recovery does not continue the authoritative state")
        for field in SENDER_SNAPSHOT_FIELDS:
            if field == "outbox_digest":
                if after[field] == before[field]:
                    _fail("sender release must remove its retained outbox entry")
            elif field == "state":
                if after[field] != before[field]:
                    _fail("sender recovery or release recommitted monetary state or authorization counters")
            elif after[field] != before[field] + int(field in {"outbox_revision", "release_counter"}):
                _fail("sender recovery or release recommitted monetary state or authorization counters")
        expected_receipt = "payment_acknowledgement" if a["operation_kind"] == "send_split" else "finalized_redemption_capability"
        if data["receipt_kind"] != expected_receipt:
            _fail("sender release substitutes its exact operation receipt authority")
        unique(data["receipt_sha256"], "sender release receipt")
        unique(data["release_authorization_sha256"], "sender release authorization")
        replies = _array(data["replies"], "sender recovery replies")
        if len(replies) != 5:
            _fail("sender recovery must cover operations 7, 8, 9, 10 and 12")
        for code, reply in zip((7, 8, 9, 10, 12), replies):
            _exact_fields(reply, ("operation", "command_sha256", "response_sha256", "certificate_sha256", "envelope_sha256", "result"), "sender recovery reply")
            if _integer(reply["operation"], "sender recovery operation", maximum=22) != code:
                _fail("sender recovery must cover its exact ordered operation codes")
            for field in ("command_sha256", "response_sha256"):
                if code == 7:
                    expected = a[field] if field == "command_sha256" else r[field]
                    if reply[field] != expected:
                        _fail("sender operation-7 recovery must replay the original exact command and response")
                else:
                    unique(reply[field], "sender recovery reply")
            if reply["certificate_sha256"] != r["certificate_sha256"] or reply["envelope_sha256"] != r["envelope_sha256"] or reply["result"] != ("released" if code == 12 else "recovered"):
                _fail("sender recovery is not byte-identical to its valid historical terminal output")
        projection = _sender_parser_report(data["native_parser_report"], document, context, attempt, original, data, short, policy)
        if replies[-1]["command_sha256"] != projection["command_sha256"]:
            _fail("sender operation-12 reply substitutes its parsed exact canonical command")
        service = _exact_fields(data["service_release_observation"], (
            "operation", "command_sha256", "authorization_purpose", "authorization_id",
            "authorization_key_reference", "release_id", "terminal_receipt_digest",
            "receipt_authority", "result",
        ), "sender service release observation")
        expected_service = {
            **{key: projection[key] for key in (
                "operation", "command_sha256", "authorization_purpose", "authorization_id",
                "authorization_key_reference", "release_id", "terminal_receipt_digest",
            )},
            "receipt_authority": "receiver_acknowledgement_signature_verified" if a["operation_kind"] == "send_split" else "core_finalized_redemption_capability_consumed",
            "result": "release_authorization_consumed",
        }
        if release.canonical_json_bytes(service) != release.canonical_json_bytes(expected_service):
            _fail("sender service observation does not bind the exact consumed release and receipt authority")
        _sender_signature(context, recovery["kind"], data, short["device_public_key"])
        snapshot = after
    assert snapshot is not None
    return snapshot["state"], snapshot["counter"], snapshot["epoch"], boot_id


def _expect_sequence(events: Sequence[Mapping[str, Any]]) -> tuple[int, int]:
    kinds = [event["kind"] for event in events]
    prefix = ["run_start", "airplane_mode_enabled", "network_probe"] + ["operation_probe"] * 22
    middle = [
        "prepare",
        "restart_begin",
        "restart_end",
        "recover_prepare",
        "prove",
        "restart_begin",
        "restart_end",
        "recover_prove",
        "candidate_persisted",
        "commit",
        "second_successor_rejected",
        "stale_predecessor_rejected",
        "power_loss_begin",
        "power_loss_end",
        "recover_commit",
        "inbox_stage",
        "power_loss_begin",
        "power_loss_end",
        "inbox_recover",
        "outbox_install",
        "power_loss_begin",
        "power_loss_end",
        "outbox_recover",
        "clock_rollback_begin",
        "clock_rollback_applied",
        "expired_request_rejected",
        "clock_rollback_end",
        "backup_snapshot",
        "advance_state",
        "backup_restore_attempt",
        "epoch_rollover",
        "counter_rollover",
        "thermal_start",
    ]
    suffix = [
        "thermal_end",
        *_sender_sequence(),
        "software_fallback_probe",
        "network_probe",
        "airplane_mode_disabled",
        "run_end",
    ]
    if kinds[: len(prefix)] != prefix:
        _fail("events are missing the canonical run/airplane/operation-probe prefix")
    middle_start = len(prefix)
    if kinds[middle_start : middle_start + len(middle)] != middle:
        _fail("events are missing a required lifecycle, recovery, durability, or control boundary")
    thermal_start = middle_start + len(middle)
    thermal_end = thermal_start
    while thermal_end < len(kinds) and kinds[thermal_end] == "thermal_fold":
        thermal_end += 1
    if thermal_end - thermal_start < MIN_THERMAL_FOLDS:
        _fail(f"thermal segment must contain at least {MIN_THERMAL_FOLDS} folds")
    if kinds[thermal_end:] != suffix:
        _fail("events are missing the canonical thermal/software/airplane/run suffix")
    probes = events[3:25]
    if [event["data"]["operation"] for event in probes] != list(range(1, 23)):
        _fail("operation probes must cover KAGEMUSHA V1 operations 1 through 22 exactly once")
    return thermal_start, thermal_end


def _semantic_transition(data: Mapping[str, Any]) -> tuple[Any, ...]:
    return tuple(data[field] for field in TRANSITION_FIELDS if field not in {"latency_ms", "rss_bytes", "energy_millijoules"})


def _derive_checks(
    document: Mapping[str, Any],
    run: Mapping[str, Any],
    events: Sequence[Mapping[str, Any]],
    metrics: Sequence[tuple[int, int]],
    thermal_start: int,
    thermal_end: int,
    policy: release.TrustedObserverPolicy,
) -> None:
    by_kind: dict[str, list[Mapping[str, Any]]] = {}
    for event in events:
        by_kind.setdefault(str(event["kind"]), []).append(event)

    start = by_kind["run_start"][0]["data"]
    airplane_on = by_kind["airplane_mode_enabled"][0]["data"]
    airplane_off = by_kind["airplane_mode_disabled"][0]["data"]
    if airplane_on["control_id"] != airplane_off["control_id"]:
        _fail("airplane-mode control boundaries do not match")
    for probe in by_kind["network_probe"]:
        data = probe["data"]
        if data["control_id"] != airplane_on["control_id"] or data["tx_bytes"] or data["rx_bytes"]:
            _fail("airplane-mode network probes must observe zero transmitted and received bytes")

    restart_begins = by_kind["restart_begin"]
    restart_ends = by_kind["restart_end"]
    power_begins = by_kind["power_loss_begin"]
    power_ends = by_kind["power_loss_end"]
    if len(restart_begins) != 2 or len(restart_ends) != 2:
        _fail("prepare and prove recovery each require one restart control cycle")
    if len(power_begins) != 3 or len(power_ends) != 3:
        _fail("commit, inbox, and outbox recovery each require one power-loss control cycle")
    boot_id = start["boot_id"]
    control_ids: set[str] = set()
    for label, begins, ends in (
        ("restart", restart_begins, restart_ends),
        ("power-loss", power_begins, power_ends),
    ):
        for begin_event, end_event in zip(begins, ends):
            begin = begin_event["data"]
            end = end_event["data"]
            if (
                begin["control_id"] in control_ids
                or begin["control_id"] != end["control_id"]
                or begin["boot_id"] != boot_id
                or end["prior_boot_id"] != boot_id
            ):
                _fail(f"{label} control cycle is replayed or not bound to the active hardware boot")
            control_ids.add(begin["control_id"])
            boot_id = end["new_boot_id"]

    for original_kind, recovered_kind in (
        ("prepare", "recover_prepare"),
        ("prove", "recover_prove"),
        ("commit", "recover_commit"),
    ):
        original = by_kind[original_kind][0]["data"]
        recovered = by_kind[recovered_kind][0]["data"]
        if _semantic_transition(original) != _semantic_transition(recovered):
            _fail(f"{recovered_kind} is not byte-identical to {original_kind}")
    lifecycle = [by_kind[kind][0]["data"] for kind in ("prepare", "prove", "candidate_persisted", "commit")]
    planned = lifecycle[0]
    if (
        planned["predecessor"] != start["initial_state"]
        or planned["counter_before"] != start["counter"]
        or planned["epoch_before"] != start["epoch"]
    ):
        _fail("run_start does not bind the prepared predecessor, counter, and epoch")
    for data in lifecycle[1:]:
        for field in (
            "operation_id",
            "predecessor",
            "successor",
            "counter_before",
            "counter_after",
            "epoch_before",
            "epoch_after",
            "canonical_bytes_sha256",
        ):
            if data[field] != planned[field]:
                _fail("prepare/prove/persist/commit recovery does not bind identical canonical bytes")

    candidate = by_kind["candidate_persisted"][0]["data"]
    if candidate["artifact_sha256"] != run["candidate_digest"]:
        _fail("run.candidate_digest does not bind the persisted candidate artifact")

    commit = by_kind["commit"][0]["data"]
    # Rolling back the host clock must not make an expired request spendable.
    # The observer clock remains monotonic; host and hardware clocks are explicit
    # observations so decreasing host time cannot be hidden in event timestamps.
    clock_begin = by_kind["clock_rollback_begin"][0]["data"]
    clock_applied = by_kind["clock_rollback_applied"][0]["data"]
    clock_rejection = by_kind["expired_request_rejected"][0]["data"]
    clock_end = by_kind["clock_rollback_end"][0]["data"]
    clock_kinds = {
        "clock_rollback_begin", "clock_rollback_applied",
        "expired_request_rejected", "clock_rollback_end",
    }
    if (
        any(data["control_id"] != clock_begin["control_id"] or data["boot_id"] != boot_id
            for data in (clock_begin, clock_applied, clock_rejection, clock_end))
        or any(event["kind"] not in clock_kinds
               and event["data"].get("control_id") == clock_begin["control_id"]
               for event in events)
    ):
        _fail("clock-rollback control is replayed or not bound to the active hardware boot")
    expiry = clock_begin["request_expires_at_ms"]
    if not (
        clock_applied["host_time_ms"] <= clock_rejection["host_time_ms"] < expiry
        < clock_begin["host_time_ms"] <= clock_end["host_time_ms"]
        and expiry < clock_begin["trusted_time_ms"] <= clock_applied["trusted_time_ms"]
        <= clock_rejection["trusted_time_ms"] <= clock_end["trusted_time_ms"]
    ):
        _fail("clock-rollback evidence must cross request expiry while trusted time remains monotonic")
    if not (
        clock_begin["state"] == clock_rejection["authoritative_state"] == commit["successor"]
        and clock_begin["counter"] == clock_rejection["counter"] == commit["counter_after"]
        and clock_begin["epoch"] == clock_rejection["epoch"] == commit["epoch_after"]
        and clock_begin["request_sha256"] == clock_rejection["request_sha256"]
    ):
        _fail("clock-rollback rejection must bind the expired request and unchanged authoritative state")
    second = by_kind["second_successor_rejected"][0]["data"]
    stale = by_kind["stale_predecessor_rejected"][0]["data"]
    if not (
        second["predecessor"] == commit["predecessor"]
        and second["committed_successor"] == commit["successor"]
        and second["observed_state"] == commit["successor"]
        and second["attempted_successor"] != commit["successor"]
        and stale["predecessor"] == commit["predecessor"]
        and stale["observed_state"] == commit["successor"]
    ):
        _fail("one-successor or stale-predecessor rejection evidence is inconsistent")

    inbox = by_kind["inbox_stage"][0]["data"]
    inbox_recovered = by_kind["inbox_recover"][0]["data"]
    outbox = by_kind["outbox_install"][0]["data"]
    outbox_recovered = by_kind["outbox_recover"][0]["data"]
    if inbox != inbox_recovered:
        _fail("inbox recovery is not byte-identical and durable")
    if outbox != outbox_recovered:
        _fail("outbox recovery is not byte-identical and durable")
    if (
        outbox["operation_id"] != commit["operation_id"]
        or outbox["canonical_bytes_sha256"] != candidate["canonical_bytes_sha256"]
        or outbox["certificate_sha256"] != commit["artifact_sha256"]
    ):
        _fail(
            "outbox does not bind the committed operation, canonical candidate "
            "envelope, and terminal certificate"
        )

    snapshot = by_kind["backup_snapshot"][0]["data"]
    advance = by_kind["advance_state"][0]["data"]
    restore = by_kind["backup_restore_attempt"][0]["data"]
    if not (
        snapshot["state"] == commit["successor"]
        and snapshot["counter"] == commit["counter_after"]
        and snapshot["epoch"] == commit["epoch_after"]
        and advance["predecessor"] == snapshot["state"]
        and advance["counter_before"] == snapshot["counter"]
        and advance["epoch_before"] == snapshot["epoch"]
        and restore["control_id"] == snapshot["control_id"]
        and restore["snapshot_sha256"] == snapshot["snapshot_sha256"]
        and restore["snapshot_state"] == snapshot["state"]
        and restore["authoritative_state"] == advance["successor"]
        and restore["counter"] == advance["counter_after"]
        and restore["epoch"] == advance["epoch_after"]
    ):
        _fail("backup restore did not preserve the rollback-resistant authoritative state")

    epoch_rollover = by_kind["epoch_rollover"][0]["data"]
    counter_rollover = by_kind["counter_rollover"][0]["data"]
    if (
        epoch_rollover["predecessor"] != advance["successor"]
        or epoch_rollover["counter_before"] != advance["counter_after"]
        or epoch_rollover["epoch_before"] != advance["epoch_after"]
        or counter_rollover["predecessor"] != epoch_rollover["successor"]
        or counter_rollover["counter_before"] != epoch_rollover["counter_after"]
        or counter_rollover["epoch_before"] != epoch_rollover["epoch_after"]
    ):
        _fail("epoch/counter rollover evidence does not extend the authoritative state")

    thermal_controls = (by_kind["thermal_start"][0], by_kind["thermal_end"][0])
    if thermal_controls[0]["data"]["control_id"] != thermal_controls[1]["data"]["control_id"]:
        _fail("thermal control boundaries do not match")
    if thermal_controls[1]["observed_at_ms"] - thermal_controls[0]["observed_at_ms"] < MIN_THERMAL_DURATION_MS:
        _fail(f"thermal folding must be sustained for at least {MIN_THERMAL_DURATION_MS} ms")
    state = counter_rollover["successor"]
    counter = counter_rollover["counter_after"]
    epoch = counter_rollover["epoch_after"]
    operation_ids: set[str] = set()
    for event in events[thermal_start:thermal_end]:
        data = event["data"]
        if data["operation_id"] in operation_ids:
            _fail("thermal segment replays an operation identifier")
        operation_ids.add(data["operation_id"])
        if (
            data["predecessor"] != state
            or data["counter_before"] != counter
            or data["epoch_before"] != epoch
        ):
            _fail("thermal segment does not form an exact-next aggregate state chain")
        state = data["successor"]
        counter = data["counter_after"]
        epoch = data["epoch_after"]
    single_use_operation_ids = [
        clock_rejection["operation_id"],
        second["operation_id"],
        stale["operation_id"],
        advance["operation_id"],
        epoch_rollover["operation_id"],
        counter_rollover["operation_id"],
        *(event["data"]["operation_id"] for event in events[thermal_start:thermal_end]),
    ]
    if (
        planned["operation_id"] in single_use_operation_ids
        or len(single_use_operation_ids) != len(set(single_use_operation_ids))
    ):
        _fail("an operation identifier is reused outside its intended recovery sequence")
    thermal_latencies = sorted(event["data"]["latency_ms"] for event in events[thermal_start:thermal_end])
    p95_thermal = thermal_latencies[(len(thermal_latencies) * 95 + 99) // 100 - 1]
    if p95_thermal > MAX_THERMAL_LATENCY_MS:
        _fail(f"thermal fold p95 latency exceeds {MAX_THERMAL_LATENCY_MS} ms")

    if not metrics:
        _fail("transcript has no resource measurements")
    if max(rss for _, rss in metrics) > MAX_RSS_BYTES:
        _fail(f"process RSS exceeds {MAX_RSS_BYTES} bytes")
    sorted_latencies = sorted(latency for latency, _ in metrics)
    p95_latency = sorted_latencies[(len(sorted_latencies) * 95 + 99) // 100 - 1]
    if p95_latency > MAX_LATENCY_MS:
        _fail(f"operation p95 latency exceeds {MAX_LATENCY_MS} ms")

    state, counter, epoch, boot_id = _derive_sender_checks(
        document, events, state, counter, epoch, boot_id, policy,
    )
    software = by_kind["software_fallback_probe"][0]["data"]
    if software["observed_state"] != state:
        _fail("software fallback probe is not bound to the authoritative state")
    run_end = by_kind["run_end"][0]["data"]
    if (
        run_end["boot_id"] != boot_id
        or run_end["final_state"] != state
        or run_end["counter"] != counter
        or run_end["epoch"] != epoch
    ):
        _fail("run_end does not bind the final physical-device state")


def _approval_subject(
    document: Mapping[str, Any], policy: release.TrustedObserverPolicy
) -> tuple[dict[str, Any], bytes]:
    body = {key: document[key] for key in ("schema", "schema_version", "profile", "endpoint", "run", "events")}
    profile = document["profile"]
    endpoint = document["endpoint"]
    run = document["run"]
    events = document["events"]
    subject = {
        "schema": APPROVAL_SCHEMA,
        "schema_version": SCHEMA_VERSION,
        "body_sha256": _sha256(release.canonical_json_bytes(body)),
        "transcript_head": events[-1]["event_hash"],
        "event_count": len(events),
        "observer_policy_sha256": policy.info.sha256,
        "hardware_profile_id": profile["hardware_profile_id"],
        "provider_id": profile["provider_id"],
        "hardware_policy_id": profile["hardware_policy_id"],
        "device_id": endpoint["device_id"],
        "run_id": run["run_id"],
    }
    canonical = release.canonical_json_bytes(subject)
    message = APPROVAL_DOMAIN + b"\0" + len(canonical).to_bytes(8, "little") + canonical
    return subject, message


def _validate_approvals(
    value: Any, document: Mapping[str, Any], policy: release.TrustedObserverPolicy
) -> None:
    approvals = _array(value, "approvals")
    if len(approvals) > len(policy.authorities):
        _fail("approvals contain more entries than the trusted observer policy")
    _, message = _approval_subject(document, policy)
    authorities = policy.authorities
    previous_id = ""
    verified = 0
    for index, raw in enumerate(approvals):
        label = f"approvals[{index}]"
        approval = _exact_fields(raw, ("authority_id", "signature"), label)
        authority_id = _digest(approval["authority_id"], f"{label}.authority_id")
        if authority_id <= previous_id:
            _fail("approvals must be uniquely sorted by authority_id")
        previous_id = authority_id
        authority = authorities.get(authority_id)
        if authority is None:
            _fail(f"{label} is not a trusted observer authority")
        signature = _signature(approval["signature"], f"{label}.signature")
        if not release._ed25519_verify(authority, message, signature):
            _fail(f"{label} has an invalid detached Ed25519 signature")
        verified += 1
    if verified < policy.threshold:
        _fail(f"observer approvals do not meet threshold {policy.threshold}")


def verify_document(
    document: Mapping[str, Any], policy: release.TrustedObserverPolicy
) -> dict[str, Any]:
    """Validate a decoded canonical transcript and return its derived report."""

    _require_policy_admission(policy)
    top = _exact_fields(
        document,
        ("schema", "schema_version", "profile", "endpoint", "run", "events", "approvals"),
        "transcript",
    )
    _integer(top["schema_version"], "transcript.schema_version", minimum=1, maximum=SCHEMA_VERSION)
    if top["schema"] != TRANSCRIPT_SCHEMA or top["schema_version"] != SCHEMA_VERSION:
        _fail("transcript schema or schema_version is unsupported")
    profile = _validate_profile(top["profile"])
    endpoint = _validate_endpoint(top["endpoint"], profile)
    run = _validate_run(top["run"])
    events, metrics = _validate_events(top["events"], run)
    thermal_start, thermal_end = _expect_sequence(events)
    _derive_checks(top, run, events, metrics, thermal_start, thermal_end, policy)
    _validate_approvals(top["approvals"], top, policy)
    report = _report(profile["provider_id"], profile["policy_epoch"], run["run_id"])
    if _sha256(release.canonical_json_bytes(report)) != profile["qualification_report_digest"]:
        _fail("profile.qualification_report_digest does not bind the derived canonical report")
    return report


def verify_bytes(payload: bytes, policy: release.TrustedObserverPolicy) -> dict[str, Any]:
    """Validate bounded canonical JSON bytes and return the derived report."""

    if not payload or len(payload) > MAX_EVIDENCE_BYTES:
        _fail(f"evidence must contain between 1 and {MAX_EVIDENCE_BYTES} bytes")
    try:
        document = release.load_json_object(payload, "physical-device evidence")
    except (release.KagemushaEvidenceError, release.ReleaseArtifactError, OSError, ValueError) as exc:
        _fail(str(exc))
    if payload != release.canonical_json_bytes(document):
        _fail("physical-device evidence must be canonical JSON without trailing bytes")
    return verify_document(document, policy)


def _load_policy(path: Path, expected_sha256: str) -> release.TrustedObserverPolicy:
    try:
        return release._load_observer_policy(path, expected_sha256)
    except (release.KagemushaEvidenceError, release.ReleaseArtifactError, OSError, ValueError) as exc:
        _fail(str(exc))


def _load_evidence(path: Path, expected_sha256: str) -> bytes:
    _digest(expected_sha256, "--evidence-sha256")
    try:
        info, payload = release.stable_read_path(path, max_size=MAX_EVIDENCE_BYTES)
    except release.KagemushaEvidenceError as exc:
        _fail(str(exc))
    if info.sha256 != expected_sha256:
        _fail("physical-device evidence SHA-256 does not match --evidence-sha256")
    return payload


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", required=True, type=Path)
    parser.add_argument("--evidence-sha256", required=True)
    parser.add_argument("--observer-policy", required=True, type=Path)
    parser.add_argument("--observer-policy-sha256", required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        policy = _load_policy(args.observer_policy, args.observer_policy_sha256)
        payload = _load_evidence(args.evidence, args.evidence_sha256)
        report = verify_bytes(payload, policy)
    except PhysicalDeviceEvidenceError as exc:
        print(f"physical-device qualification rejected: {exc}", file=sys.stderr)
        return 1
    sys.stdout.buffer.write(release.canonical_json_bytes(report))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
