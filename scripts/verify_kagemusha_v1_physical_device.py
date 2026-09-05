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
PHYSICAL_CHECKS = (
    "airplane_mode",
    "restart",
    "power_loss",
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


# TODO: The V1 release report/command schema currently retains only the small
# derived report below and admits only that report as the verification-command
# input.  It therefore cannot carry this transcript, its OEM attestation, or its
# observer-policy binding into the final release closure.  This verifier checks
# the observer-signed transcript's internal consistency; it must not be treated
# as closing that external attestation/provenance gap until the release schema is
# extended.
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
        ("run_id", "candidate_digest", "artifact_set_digest", "started_at_ms", "ended_at_ms"),
        "run",
    )
    for field in ("run_id", "candidate_digest", "artifact_set_digest"):
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
        "backup_snapshot",
        "advance_state",
        "backup_restore_attempt",
        "epoch_rollover",
        "counter_rollover",
        "thermal_start",
    ]
    suffix = [
        "thermal_end",
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
    run: Mapping[str, Any],
    events: Sequence[Mapping[str, Any]],
    metrics: Sequence[tuple[int, int]],
    thermal_start: int,
    thermal_end: int,
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
    if top["schema"] != TRANSCRIPT_SCHEMA or top["schema_version"] != SCHEMA_VERSION:
        _fail("transcript schema or schema_version is unsupported")
    profile = _validate_profile(top["profile"])
    endpoint = _validate_endpoint(top["endpoint"], profile)
    run = _validate_run(top["run"])
    events, metrics = _validate_events(top["events"], run)
    thermal_start, thermal_end = _expect_sequence(events)
    _derive_checks(run, events, metrics, thermal_start, thermal_end)
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
