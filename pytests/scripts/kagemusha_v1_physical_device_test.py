"""Regressions for the closed KAGEMUSHA V1 physical-device verifier."""

from __future__ import annotations

import copy
import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path
from typing import Any, Mapping


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = REPOSITORY_ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

import verify_kagemusha_v1_physical_device as physical  # noqa: E402
import verify_kagemusha_v1_release_evidence as release  # noqa: E402


def _digest(label: str) -> str:
    return hashlib.sha256(label.encode("utf-8")).hexdigest()


def _sender_parser_policy_rows() -> list[dict[str, Any]]:
    """Explicit synthetic observer policy; its fake binary hash is never production evidence."""
    return [
        {"id": physical.SENDER_PARSER_ID, "sha256": _digest("test-only-native-parser-binary"), "report_schemas": [physical.SENDER_PARSER_SCHEMA]},
        {"id": physical.SENDER_PARSER_SOURCE_ID, "sha256": hashlib.sha256(physical.SENDER_PARSER_SOURCE.read_bytes()).hexdigest(), "report_schemas": [physical.SENDER_PARSER_SCHEMA]},
    ]


def _sender_native_fixtures(context: Mapping[str, Any]) -> dict[tuple[str, str], Mapping[str, Any]]:
    """Select immutable actual native-parser golden bytes without relabeling their context."""
    path = REPOSITORY_ROOT / "fixtures/offline/kagemusha_sender_release_parser_v1.json"
    golden = json.loads(path.read_text())
    assert golden["schema"] == "iroha.kagemusha_v1.sender_release_parser_test_fixtures"
    for bundle in golden["contexts"].values():
        native_context = bundle["context"]
        if all(native_context[key] == context[key] for key in (
            "hardware_profile", "credentials", "hardware_policy_id", "vk_digest",
        )):
            return {(row["case"], row["operation_kind"]): row for row in bundle["fixtures"]}
    raise AssertionError("no native golden command bundle matches the exact synthetic sender context")


def _keypair(seed: bytes) -> tuple[bytes, bytes]:
    hashed = hashlib.sha512(seed).digest()
    scalar_bytes = bytearray(hashed[:32])
    scalar_bytes[0] &= 248
    scalar_bytes[31] &= 63
    scalar_bytes[31] |= 64
    scalar = int.from_bytes(scalar_bytes, "little")
    public_key = release._ed_encode(release._ed_scalarmult(release._ED_B, scalar))
    return seed, public_key


def _sign(seed: bytes, message: bytes) -> bytes:
    hashed = hashlib.sha512(seed).digest()
    scalar_bytes = bytearray(hashed[:32])
    scalar_bytes[0] &= 248
    scalar_bytes[31] &= 63
    scalar_bytes[31] |= 64
    scalar = int.from_bytes(scalar_bytes, "little")
    public_key = release._ed_encode(release._ed_scalarmult(release._ED_B, scalar))
    nonce = int.from_bytes(hashlib.sha512(hashed[32:] + message).digest(), "little") % release._ED_L
    encoded_r = release._ed_encode(release._ed_scalarmult(release._ED_B, nonce))
    challenge = int.from_bytes(hashlib.sha512(encoded_r + public_key + message).digest(), "little") % release._ED_L
    encoded_s = ((nonce + challenge * scalar) % release._ED_L).to_bytes(32, "little")
    return encoded_r + encoded_s


def _p256_public(scalar: int) -> str:
    """Return a deterministic test-only P-256 public key."""
    x, y = release._p256_multiply(scalar, release._P256_GENERATOR)
    return (b"\x04" + x.to_bytes(32, "big") + y.to_bytes(32, "big")).hex()


def _p256_sign(message: bytes, scalar: int) -> str:
    """Sign synthetic fixtures only; never expose fixture keys to collectors."""
    order = release._P256_ORDER
    nonce = 1 + int.from_bytes(hashlib.sha256(b"synthetic-sender-only" + scalar.to_bytes(32, "big") + message).digest(), "big") % (order - 1)
    r = release._p256_multiply(nonce, release._P256_GENERATOR)[0] % order
    s = (int.from_bytes(hashlib.sha256(message).digest(), "big") + r * scalar) * pow(nonce, -1, order) % order
    assert r and s
    return (r.to_bytes(32, "big") + min(s, order - s).to_bytes(32, "big")).hex()


class _TranscriptBuilder:
    """Build deterministic signed synthetic evidence; never used for qualification."""

    def __init__(self, policy: release.TrustedObserverPolicy, seeds: Mapping[str, bytes]) -> None:
        self.policy = policy
        self.seeds = seeds
        self.events: list[dict[str, Any]] = []
        self.clock = 1_700_000_000_000
        self.previous = physical.ZERO_DIGEST

    def add(self, kind: str, data: Mapping[str, Any], *, step_ms: int = 10) -> None:
        self.clock += step_ms
        unhashed = {
            "index": len(self.events),
            "kind": kind,
            "observed_at_ms": self.clock,
            "previous_hash": self.previous,
            "data": dict(data),
        }
        event_hash = hashlib.sha256(
            physical.EVENT_HASH_DOMAIN + b"\0" + release.canonical_json_bytes(unhashed)
        ).hexdigest()
        event = dict(unhashed)
        event["event_hash"] = event_hash
        self.events.append(event)
        self.previous = event_hash

    @staticmethod
    def transition(
        kind: str,
        operation_id: str,
        predecessor: str,
        successor: str,
        counter_before: int,
        counter_after: int,
        epoch_before: int,
        epoch_after: int,
        canonical_bytes: str,
        artifact: str,
        *,
        energy: int = 0,
        hardware_before: int = 0,
        hardware_after: int = 0,
    ) -> dict[str, Any]:
        del kind
        return {
            "operation_id": operation_id,
            "predecessor": predecessor,
            "successor": successor,
            "counter_before": counter_before,
            "counter_after": counter_after,
            "epoch_before": epoch_before,
            "epoch_after": epoch_after,
            "artifact_sha256": artifact,
            "canonical_bytes_sha256": canonical_bytes,
            "result": "success",
            "latency_ms": 10,
            "rss_bytes": 32 * 1024 * 1024,
            "energy_millijoules": energy,
            "hardware_counter_before": hardware_before,
            "hardware_counter_after": hardware_after,
        }

    def build(self) -> dict[str, Any]:
        provider_id = _digest("provider")
        run_id = _digest("physical-run")
        policy_epoch = 7
        report = physical._report(provider_id, policy_epoch, run_id)
        qualification_digest = hashlib.sha256(release.canonical_json_bytes(report)).hexdigest()
        profile_id = _digest("hardware-profile")
        hardware_policy_id = _digest("hardware-policy")
        boot_1, boot_2, boot_3, boot_4, boot_5, boot_6 = (
            _digest(f"boot-{index}") for index in range(1, 7)
        )
        state_0 = _digest("state-0")
        airplane_control = _digest("airplane-control")
        self.add(
            "run_start",
            {"boot_id": boot_1, "initial_state": state_0, "counter": 10, "epoch": 1},
        )
        self.add("airplane_mode_enabled", {"control_id": airplane_control})
        self.add(
            "network_probe",
            {"control_id": airplane_control, "tx_bytes": 0, "rx_bytes": 0, "result": "isolated"},
        )
        for operation in range(1, 23):
            self.add(
                "operation_probe",
                {
                    "operation": operation,
                    "request_id": _digest(f"probe-request-{operation}"),
                    "command_sha256": _digest(f"probe-command-{operation}"),
                    "response_sha256": _digest(f"probe-response-{operation}"),
                    "result": "authenticated",
                    "latency_ms": 5,
                    "rss_bytes": 24 * 1024 * 1024,
                },
            )

        state_1 = _digest("state-1")
        lifecycle_operation = _digest("lifecycle-operation")
        lifecycle_bytes = _digest("lifecycle-canonical-bytes")
        prepare = self.transition(
            "prepare",
            lifecycle_operation,
            state_0,
            state_1,
            10,
            11,
            1,
            1,
            lifecycle_bytes,
            _digest("prepared-artifact"),
        )
        self.add("prepare", prepare)
        restart_control = _digest("restart-control")
        self.add("restart_begin", {"control_id": restart_control, "boot_id": boot_1})
        self.add(
            "restart_end",
            {"control_id": restart_control, "prior_boot_id": boot_1, "new_boot_id": boot_2},
        )
        self.add("recover_prepare", prepare)
        prove = dict(prepare)
        prove["artifact_sha256"] = _digest("proof-artifact")
        self.add("prove", prove)
        prove_restart_control = _digest("prove-restart-control")
        self.add(
            "restart_begin", {"control_id": prove_restart_control, "boot_id": boot_2}
        )
        self.add(
            "restart_end",
            {
                "control_id": prove_restart_control,
                "prior_boot_id": boot_2,
                "new_boot_id": boot_3,
            },
        )
        self.add("recover_prove", prove)
        candidate = dict(prepare)
        candidate["artifact_sha256"] = _digest("candidate-artifact")
        self.add("candidate_persisted", candidate)
        commit = dict(prepare)
        commit["artifact_sha256"] = _digest("commit-artifact")
        self.add("commit", commit)
        self.add(
            "second_successor_rejected",
            {
                "operation_id": _digest("second-successor-operation"),
                "predecessor": state_0,
                "attempted_successor": _digest("conflicting-state"),
                "committed_successor": state_1,
                "observed_state": state_1,
                "result": "rejected",
            },
        )
        self.add(
            "stale_predecessor_rejected",
            {
                "operation_id": _digest("stale-operation"),
                "predecessor": state_0,
                "observed_state": state_1,
                "result": "rejected",
            },
        )
        power_control = _digest("power-control")
        self.add("power_loss_begin", {"control_id": power_control, "boot_id": boot_3})
        self.add(
            "power_loss_end",
            {"control_id": power_control, "prior_boot_id": boot_3, "new_boot_id": boot_4},
        )
        self.add("recover_commit", commit)

        inbox = {
            "credit_id": _digest("inbox-credit"),
            "canonical_bytes_sha256": _digest("inbox-bytes"),
            "receipt_sha256": _digest("inbox-receipt"),
            "inbox_revision": 1,
            "result": "durable",
            "latency_ms": 5,
            "rss_bytes": 28 * 1024 * 1024,
        }
        self.add("inbox_stage", inbox)
        inbox_power_control = _digest("inbox-power-control")
        self.add(
            "power_loss_begin", {"control_id": inbox_power_control, "boot_id": boot_4}
        )
        self.add(
            "power_loss_end",
            {
                "control_id": inbox_power_control,
                "prior_boot_id": boot_4,
                "new_boot_id": boot_5,
            },
        )
        self.add("inbox_recover", inbox)
        outbox = {
            "operation_id": lifecycle_operation,
            "canonical_bytes_sha256": lifecycle_bytes,
            "certificate_sha256": commit["artifact_sha256"],
            "outbox_revision": 1,
            "result": "durable",
            "latency_ms": 5,
            "rss_bytes": 28 * 1024 * 1024,
        }
        self.add("outbox_install", outbox)
        outbox_power_control = _digest("outbox-power-control")
        self.add(
            "power_loss_begin", {"control_id": outbox_power_control, "boot_id": boot_5}
        )
        self.add(
            "power_loss_end",
            {
                "control_id": outbox_power_control,
                "prior_boot_id": boot_5,
                "new_boot_id": boot_6,
            },
        )
        self.add("outbox_recover", outbox)

        clock_control = _digest("clock-control")
        clock_request = _digest("expired-clock-request")
        self.add(
            "clock_rollback_begin",
            {
                "control_id": clock_control, "boot_id": boot_6,
                "state": state_1, "counter": 11, "epoch": 1,
                "host_time_ms": 20_000, "trusted_time_ms": 20_000,
                "request_expires_at_ms": 15_000, "request_sha256": clock_request,
            },
        )
        self.add(
            "clock_rollback_applied",
            {
                "control_id": clock_control, "boot_id": boot_6,
                "host_time_ms": 10_000, "trusted_time_ms": 20_001,
            },
        )
        self.add(
            "expired_request_rejected",
            {
                "control_id": clock_control, "boot_id": boot_6,
                "operation_id": _digest("expired-clock-operation"),
                "request_sha256": clock_request, "authoritative_state": state_1,
                "counter": 11, "epoch": 1, "host_time_ms": 10_001,
                "trusted_time_ms": 20_002, "result": "expired_request_rejected",
            },
        )
        self.add(
            "clock_rollback_end",
            {
                "control_id": clock_control, "boot_id": boot_6,
                "host_time_ms": 20_003, "trusted_time_ms": 20_003,
            },
        )

        backup_control = _digest("backup-control")
        snapshot = _digest("backup-snapshot")
        self.add(
            "backup_snapshot",
            {
                "control_id": backup_control,
                "state": state_1,
                "counter": 11,
                "epoch": 1,
                "snapshot_sha256": snapshot,
            },
        )
        state_2 = _digest("state-2")
        advance = self.transition(
            "advance_state",
            _digest("advance-operation"),
            state_1,
            state_2,
            11,
            12,
            1,
            1,
            _digest("advance-bytes"),
            _digest("advance-artifact"),
        )
        self.add("advance_state", advance)
        self.add(
            "backup_restore_attempt",
            {
                "control_id": backup_control,
                "snapshot_sha256": snapshot,
                "snapshot_state": state_1,
                "authoritative_state": state_2,
                "counter": 12,
                "epoch": 1,
                "result": "rollback_rejected",
            },
        )
        state_3 = _digest("state-3")
        self.add(
            "epoch_rollover",
            self.transition(
                "epoch_rollover",
                _digest("epoch-operation"),
                state_2,
                state_3,
                12,
                13,
                1,
                2,
                _digest("epoch-bytes"),
                _digest("epoch-artifact"),
            ),
        )
        state_4 = _digest("state-4")
        self.add(
            "counter_rollover",
            self.transition(
                "counter_rollover",
                _digest("counter-operation"),
                state_3,
                state_4,
                13,
                14,
                2,
                3,
                _digest("counter-bytes"),
                _digest("counter-artifact"),
                hardware_before=physical.U128_MAX,
                hardware_after=1,
            ),
        )

        thermal_control = _digest("thermal-control")
        self.add(
            "thermal_start",
            {"control_id": thermal_control, "sensor_digest": _digest("thermal-sensor-start")},
        )
        state, counter = state_4, 14
        for fold in range(physical.MIN_THERMAL_FOLDS):
            successor = _digest(f"thermal-state-{fold}")
            self.add(
                "thermal_fold",
                self.transition(
                    "thermal_fold",
                    _digest(f"thermal-operation-{fold}"),
                    state,
                    successor,
                    counter,
                    counter + 1,
                    3,
                    3,
                    _digest(f"thermal-bytes-{fold}"),
                    _digest(f"thermal-artifact-{fold}"),
                    energy=2,
                ),
                step_ms=61,
            )
            state, counter = successor, counter + 1
        self.add(
            "thermal_end",
            {"control_id": thermal_control, "sensor_digest": _digest("thermal-sensor-end")},
        )
        self.add(
            "software_fallback_probe",
            {
                "control_id": _digest("software-control"),
                "requested_backend": "software",
                "observed_state": state,
                "result": "rejected",
            },
        )
        self.add(
            "network_probe",
            {"control_id": airplane_control, "tx_bytes": 0, "rx_bytes": 0, "result": "isolated"},
        )
        self.add("airplane_mode_disabled", {"control_id": airplane_control})
        self.add(
            "run_end",
            {"boot_id": boot_6, "final_state": state, "counter": counter, "epoch": 3},
        )

        document = {
            "schema": physical.TRANSCRIPT_SCHEMA,
            "schema_version": physical.SCHEMA_VERSION,
            "profile": {
                "hardware_profile_id": profile_id,
                "provider_id": provider_id,
                "hardware_policy_id": hardware_policy_id,
                "qualification_report_digest": qualification_digest,
                "policy_epoch": policy_epoch,
                "capability_mask": 0xFFFF,
            },
            "endpoint": {
                "kind": "physical_secure_element",
                "platform_class": "android_oem_service",
                "transport": "secure_service",
                "device_id": _digest("device"),
                "product_id": _digest("product"),
                "firmware_digest": _digest("firmware"),
                "os_build_digest": _digest("os-build"),
                "attestation_digest": _digest("attestation"),
                "hardware_profile_id": profile_id,
                "hardware_policy_id": hardware_policy_id,
                "qualification_report_digest": qualification_digest,
                "hardware_backed": True,
                "software_fallback": False,
                "production_build": True,
            },
            "run": {
                "run_id": run_id,
                "candidate_digest": candidate["artifact_sha256"],
                "candidate_context_digest": _digest("release-candidate-context"),
                "artifact_set_digest": _digest("artifacts"),
                "started_at_ms": 1_700_000_000_000,
                "ended_at_ms": 1_700_000_200_000,
            },
            "events": self.events,
            "approvals": [],
        }
        self.add_sender_validity(document)
        return document

    def add_sender_validity(
        self, document: dict[str, Any], hardware_profile: Mapping[str, Any] | None = None,
        suite_id: str | None = None, vk_digest: str | None = None,
    ) -> None:
        """Replace the complete synthetic sender segment for the exact release fixture.

        Scalar one is the test governance issuer; scalar two is the test device.
        These signatures exercise authentication, never physical qualification.
        """
        suite_id = suite_id or _digest("sender-suite")
        p = document["profile"]
        profile = dict(hardware_profile) if hardware_profile is not None else {
            "version": 1, "protocol_version": 1, "hardware_profile_id": physical.ZERO_DIGEST,
            "provider_id": p["provider_id"], "platform_class": document["endpoint"]["platform_class"],
            "product_class_digest": _digest("product-class"), "firmware_policy_digest": _digest("firmware-policy"),
            "enrollment_attestation_verifier_digest": _digest("enrollment-verifier"),
            "attestation_trust_roots_digest": _digest("attestation-roots"),
            "allowed_suite_commitment": release._suite_commitment(suite_id),
            "policy_epoch": p["policy_epoch"], "governance_credential_public_key": _p256_public(1),
            "capability_mask": p["capability_mask"], "qualification_report_digest": p["qualification_report_digest"],
            "valid_from_ms": 1, "expires_at_ms": 1_800_000_000_000,
        }
        profile["hardware_profile_id"] = release.rust_hardware_profile_id(profile)
        p["hardware_profile_id"] = profile["hardware_profile_id"]
        document["endpoint"]["hardware_profile_id"] = profile["hardware_profile_id"]
        original = [event for event in document["events"] if not event["kind"].startswith("sender_")]
        boundary = next(index for index, event in enumerate(original) if event["kind"] == "thermal_end") + 1
        tail = copy.deepcopy(original[boundary:])
        self.events = original[:boundary]
        self.previous = self.events[-1]["event_hash"]
        self.clock = self.events[-1]["observed_at_ms"]
        last_fold = next(event["data"] for event in reversed(self.events) if event["kind"] == "thermal_fold")
        boot = next(event["data"]["new_boot_id"] for event in reversed(self.events) if event["kind"] == "power_loss_end")
        issued, expires = self.clock + 1_000, self.clock + 3_000
        credentials = []
        for end in (expires,):
            credential = {
                "version": 1, "credential_id": physical.ZERO_DIGEST, "network_id": _digest("sender-network"),
                "hardware_profile_id": profile["hardware_profile_id"], "suite_id": suite_id,
                "firmware_policy_digest": profile["firmware_policy_digest"], "policy_epoch": profile["policy_epoch"],
                "lane_commitment": _digest("sender-lane"), "hardware_epoch_id": _digest("sender-epoch"),
                "hardware_epoch_generation": last_fold["epoch_after"], "device_public_key": _p256_public(2),
                "device_key_reference": hashlib.sha256(b"iroha:kagemusha:v1:device-key-reference\0" + bytes.fromhex(_p256_public(2))).hexdigest(),
                "issued_at_ms": issued, "expires_at_ms": end, "governance_signature": "00" * 64,
            }
            credential["credential_id"] = physical.credential_identity(credential)
            credential["governance_signature"] = _p256_sign(physical.credential_signing_bytes(credential), 1)
            credentials.append(credential)
        context = {
            **{field: document["run"][field] for field in ("run_id", "candidate_context_digest", "artifact_set_digest")},
            "device_id": document["endpoint"]["device_id"], "hardware_policy_id": p["hardware_policy_id"],
            "hardware_profile": profile, "credentials": credentials,
            "vk_digest": vk_digest or _digest("sender-vk"),
        }
        native_fixtures = _sender_native_fixtures(context)
        self.add("sender_validity_context", context)
        context_hash = self.events[-1]["event_hash"]
        snapshot = {
            "state": last_fold["successor"], "counter": last_fold["counter_after"], "epoch": last_fold["epoch_after"],
            "authorization_counter": 40, "lease_counter": 3, "release_counter": 7,
            "journal_revision": last_fold["counter_after"], "outbox_revision": 10, "outbox_digest": _digest("sender-initial-outbox"),
        }
        positives = []
        for case in physical.SENDER_CASES:
            for operation in physical.SENDER_OPERATIONS:
                label = f"{case}-{operation}"
                if case == "before_issuance":
                    now = self.clock + 10
                elif case == "trusted_before_expiry":
                    now = max(self.clock + 10, expires - 100)
                elif case in {"credential_expiry", "credential_after"}:
                    now = max(self.clock + 10, expires + 10)
                else:
                    now = max(self.clock + 10, issued + 10)
                lease = {
                    "lease_credential_straddle": (issued, expires + 1),
                    "lease_empty": (issued, issued), "lease_zero": (0, expires),
                    "lease_before_credential": (issued - 1, expires),
                    "lease_end": (issued, expires),
                }.get(case, (0, 0))
                is_lease = case.startswith("lease_")
                attempt = {
                    "case": case, "operation_kind": operation, "operation_id": _digest(label + "-operation"),
                    "credential_id": credentials[0]["credential_id"], "boot_id": boot,
                    **{field: _digest(label + field) for field in ("command_sha256", "preparation_sha256", "candidate_sha256", "reservation_sha256", "time_evidence_sha256", "commit_evidence_commitment")},
                    "request_sha256": _digest(label + "-request") if operation == "send_split" else physical.ZERO_DIGEST,
                    "request_start_ms": document["run"]["started_at_ms"] if operation == "send_split" else 0,
                    "request_end_ms": expires + 1000 if operation == "send_split" else 0,
                    "reservation_start_ms": document["run"]["started_at_ms"], "reservation_end_ms": expires + 1000,
                    "source": "monotonic_lease" if is_lease else "trusted_time", "trusted_time_ms": now,
                    "lease_start_ms": lease[0], "lease_end_ms": lease[1], "before": dict(snapshot),
                }
                positive = case in physical.SENDER_POSITIVE_CASES
                native = native_fixtures[(case, operation)] if positive else None
                if positive and operation == "send_split":
                    attempt.update({key: native["projection"][key] for key in ("request_sha256", "request_start_ms", "request_end_ms")})
                self.clock = max(self.clock, now)
                self.add("sender_admission_attempt", attempt)
                attempt_event = self.events[-1]
                if positive:
                    snapshot = dict(snapshot)
                    for field in ("counter", "authorization_counter", "journal_revision", "outbox_revision"):
                        snapshot[field] += 1
                    snapshot["lease_counter"] += int(is_lease)
                    snapshot["state"] = _digest(label + "-successor")
                    snapshot["outbox_digest"] = _digest(label + "-outbox")
                result = {
                    "operation_id": attempt["operation_id"], "attempt_event_hash": attempt_event["event_hash"], "boot_id": boot,
                    "response_sha256": _digest(label + "-response"), "after": dict(snapshot),
                    "decision_trusted_time_ms": now + 5,
                    "result": "committed" if positive else "rejected",
                    "certificate_sha256": native["projection"]["certificate_sha256"] if positive else physical.ZERO_DIGEST,
                    "envelope_sha256": native["projection"]["envelope_sha256"] if positive else physical.ZERO_DIGEST,
                }
                result["hardware_signature"] = _p256_sign(physical.sender_evidence_signing_bytes(context_hash, "sender_admission_result", result), 2)
                self.add("sender_admission_result", result)
                if positive:
                    positives.append((attempt_event, self.events[-1]))
        for index, (attempt_event, result_event) in enumerate(positives):
            a, r = attempt_event["data"], result_event["data"]
            native = native_fixtures[(a["case"], a["operation_kind"])]
            projection = copy.deepcopy(native["projection"])
            label = a["operation_id"] + "-recovery"
            after = dict(snapshot)
            after["release_counter"] += 1
            after["outbox_revision"] += 1
            after["outbox_digest"] = _digest(label + "-released-outbox")
            recovery = {
                "operation_id": a["operation_id"], "original_result_hash": result_event["event_hash"],
                "control_id": _digest(label + "-control"), "prior_boot_id": boot, "boot_id": _digest(label + "-boot"),
                "trusted_time_ms": self.clock + 5, "time_evidence_sha256": _digest(label + "-time"),
                "before": dict(snapshot), "after": dict(after),
                "receipt_kind": "payment_acknowledgement" if a["operation_kind"] == "send_split" else "finalized_redemption_capability",
                "receipt_sha256": projection["terminal_receipt_sha256"], "release_authorization_sha256": projection["hardware_authorization_sha256"],
                "replies": [{
                    "operation": code, "command_sha256": a["command_sha256"] if code == 7 else _digest(label + f"-command-{code}"),
                    "response_sha256": r["response_sha256"] if code == 7 else _digest(label + f"-response-{code}"), "certificate_sha256": r["certificate_sha256"],
                    "envelope_sha256": r["envelope_sha256"], "result": "released" if code == 12 else "recovered",
                } for code in (7, 8, 9, 10, 12)],
            }
            recovery["replies"][-1]["command_sha256"] = projection["command_sha256"]
            report = {
                "schema": physical.SENDER_PARSER_SCHEMA, "schema_version": 1,
                "purpose": physical.SENDER_PARSER_PURPOSE,
                "verifier_id": physical.SENDER_PARSER_ID, "source_id": physical.SENDER_PARSER_SOURCE_ID,
                "verifier_sha256": self.policy.verifiers[physical.SENDER_PARSER_ID].sha256,
                "source_sha256": self.policy.verifiers[physical.SENDER_PARSER_SOURCE_ID].sha256,
                **{key: document["run"][key] for key in ("run_id", "candidate_context_digest", "artifact_set_digest")},
                "device_id": document["endpoint"]["device_id"], "sender_context_event_hash": context_hash,
                "command_hex": native["canonical_sender_command_hex"], "projection": projection,
                "approvals": [],
            }
            self.approve_parser(report)
            recovery["native_parser_report"] = report
            recovery["service_release_observation"] = {
                **{key: projection[key] for key in (
                    "operation", "command_sha256", "authorization_purpose", "authorization_id",
                    "authorization_key_reference", "release_id", "terminal_receipt_digest",
                )},
                "receipt_authority": "receiver_acknowledgement_signature_verified" if a["operation_kind"] == "send_split" else "core_finalized_redemption_capability_consumed",
                "result": "release_authorization_consumed",
            }
            recovery["hardware_signature"] = _p256_sign(physical.sender_evidence_signing_bytes(context_hash, "sender_historical_recovery", recovery), 2)
            self.add("sender_historical_recovery", recovery)
            snapshot, boot = after, recovery["boot_id"]
        for event in tail:
            data = event["data"]
            if event["kind"] == "software_fallback_probe":
                data["observed_state"] = snapshot["state"]
            elif event["kind"] == "run_end":
                data.update({"boot_id": boot, "final_state": snapshot["state"], "counter": snapshot["counter"], "epoch": snapshot["epoch"]})
            self.add(event["kind"], data)
        document["events"] = self.events
        self.approve(document)

    def rechain_sender(self, document: dict[str, Any], *, sign_hardware: bool = True) -> None:
        """Reapprove changed test observations, optionally forging no device authority."""
        previous = physical.ZERO_DIGEST
        replacements: dict[str, str] = {}
        context_hash = ""
        for index, event in enumerate(document["events"]):
            old_hash = event["event_hash"]
            event.update({"index": index, "previous_hash": previous})
            data = event["data"]
            for field in ("attempt_event_hash", "original_result_hash"):
                if field in data:
                    data[field] = replacements.get(data[field], data[field])
            if event["kind"] == "sender_historical_recovery":
                report = data["native_parser_report"]
                report["sender_context_event_hash"] = context_hash
                report.update({key: document["run"][key] for key in ("run_id", "candidate_context_digest", "artifact_set_digest")})
                self.approve_parser(report)
            if sign_hardware and event["kind"] in {"sender_admission_result", "sender_historical_recovery"}:
                data["hardware_signature"] = _p256_sign(physical.sender_evidence_signing_bytes(context_hash, event["kind"], data), 2)
            unhashed = {key: event[key] for key in ("index", "kind", "observed_at_ms", "previous_hash", "data")}
            event["event_hash"] = hashlib.sha256(physical.EVENT_HASH_DOMAIN + b"\0" + release.canonical_json_bytes(unhashed)).hexdigest()
            replacements[old_hash] = event["event_hash"]
            previous = event["event_hash"]
            if event["kind"] == "sender_validity_context":
                context_hash = previous
        self.approve(document)

    def approve_parser(self, report: dict[str, Any]) -> None:
        """Sign synthetic observations over actual native golden output, never execute a candidate."""
        message = physical.sender_parser_approval_message(report, self.policy)
        report["approvals"] = [
            {"authority_id": authority_id, "signature": _sign(self.seeds[authority_id], message).hex()}
            for authority_id in sorted(self.seeds)
        ]

    def approve(self, document: dict[str, Any]) -> None:
        document["approvals"] = []
        _, message = physical._approval_subject(document, self.policy)
        document["approvals"] = [
            {"authority_id": authority_id, "signature": _sign(self.seeds[authority_id], message).hex()}
            for authority_id in sorted(self.seeds)
        ]


class PhysicalDeviceEvidenceTest(unittest.TestCase):
    """Reject adversarial transcripts while accepting one closed synthetic fixture."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.temporary = tempfile.TemporaryDirectory()
        cls.addClassCleanup(cls.temporary.cleanup)
        directory = Path(cls.temporary.name)
        keypairs = [_keypair(bytes([index]) * 32) for index in (11, 29)]
        seeds: dict[str, bytes] = {}
        authority_rows: list[dict[str, str]] = []
        for seed, public_key in keypairs:
            authority_id = hashlib.sha256(release.OBSERVER_AUTHORITY_ID_DOMAIN + public_key).hexdigest()
            seeds[authority_id] = seed
            authority_rows.append(
                {"authority_id": authority_id, "ed25519_public_key": public_key.hex()}
            )
        policy_document = {
            "schema": release.OBSERVER_POLICY_SCHEMA,
            "schema_version": 1,
            "threshold": 2,
            "authorities": sorted(authority_rows, key=lambda row: row["authority_id"]),
            "verifiers": [
                *_sender_parser_policy_rows(),
                {
                    "id": physical.PHYSICAL_VERIFIER_ID,
                    "sha256": hashlib.sha256(
                        Path(physical.__file__).read_bytes()
                    ).hexdigest(),
                    "report_schemas": [physical.REPORT_SCHEMA],
                }
            ],
        }
        policy_payload = release.canonical_json_bytes(policy_document)
        policy_path = directory / "observer-policy.json"
        policy_path.write_bytes(policy_payload)
        cls.policy_path = policy_path.resolve()
        cls.policy_sha256 = hashlib.sha256(policy_payload).hexdigest()
        cls.policy = release._load_observer_policy(
            cls.policy_path, cls.policy_sha256
        )
        cls.seeds = seeds
        cls.builder = _TranscriptBuilder(cls.policy, cls.seeds)
        cls.valid_document = cls.builder.build()

    def fresh(self) -> dict[str, Any]:
        return copy.deepcopy(self.valid_document)

    def verify(self, document: Mapping[str, Any]) -> dict[str, Any]:
        return physical.verify_bytes(release.canonical_json_bytes(document), self.policy)

    def rechain_and_approve(self, document: dict[str, Any]) -> None:
        previous = physical.ZERO_DIGEST
        for index, event in enumerate(document["events"]):
            event["index"] = index
            event["previous_hash"] = previous
            unhashed = {
                key: event[key]
                for key in ("index", "kind", "observed_at_ms", "previous_hash", "data")
            }
            event["event_hash"] = hashlib.sha256(
                physical.EVENT_HASH_DOMAIN + b"\0" + release.canonical_json_bytes(unhashed)
            ).hexdigest()
            previous = event["event_hash"]
        self.builder.approve(document)

    def rechain_sender(self, document: dict[str, Any], *, sign_hardware: bool = True) -> None:
        self.builder.rechain_sender(document, sign_hardware=sign_hardware)

    def sender_attempt(self, document: dict[str, Any], case: str, operation: str = "send_split") -> dict[str, Any]:
        return next(event for event in document["events"] if event["kind"] == "sender_admission_attempt" and event["data"]["case"] == case and event["data"]["operation_kind"] == operation)

    def test_native_golden_raw_objects_match_actual_parser_projection(self) -> None:
        context = next(event["data"] for event in self.valid_document["events"] if event["kind"] == "sender_validity_context")
        for row in _sender_native_fixtures(context).values():
            for raw, field in (
                ("canonical_sender_command_hex", "command_sha256"),
                ("canonical_envelope_hex", "envelope_sha256"),
                ("canonical_certificate_hex", "certificate_sha256"),
                ("canonical_terminal_receipt_hex", "terminal_receipt_sha256"),
                ("canonical_hardware_authorization_hex", "hardware_authorization_sha256"),
            ):
                self.assertEqual(hashlib.sha256(bytes.fromhex(row[raw])).hexdigest(), row["projection"][field])
            self.assertIs(row["projection"]["structural_only"], True)

    def test_sender_parser_rejects_freshly_signed_projection_substitution(self) -> None:
        variants = {
            "credential_id": _digest("substituted-credential"), "operation_kind": "redeem_split",
            "command_sha256": _digest("substituted-command"), "terminal_receipt_sha256": _digest("substituted-receipt"),
            "hardware_authorization_sha256": _digest("substituted-authorization"), "authorization_purpose": "commit",
            "vk_digest": _digest("substituted-vk"), "commit_evidence_commitment": _digest("substituted-time-commitment"),
            "payment_committed_at_ms": 42, "request_end_ms": 42,
            "artifact_manifest_digest": _digest("invented-send-manifest"), "structural_only": False,
        }
        for field, value in variants.items():
            with self.subTest(field=field):
                document = self.fresh()
                recovery = next(event["data"] for event in document["events"] if event["kind"] == "sender_historical_recovery")
                recovery["native_parser_report"]["projection"][field] = value
                self.rechain_sender(document)
                with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, "projection|parser output"):
                    self.verify(document)

    def test_sender_parser_rejects_role_purpose_and_hash_substitutions(self) -> None:
        variants = {
            "schema": physical.REPORT_SCHEMA, "purpose": "hardware_qualification",
            "schema_version": 1.0,
            "verifier_id": physical.PHYSICAL_VERIFIER_ID, "source_id": physical.SENDER_PARSER_ID,
            "source_sha256": _digest("different-parser-source"), "verifier_sha256": _digest("different-parser-binary"),
        }
        for field, value in variants.items():
            with self.subTest(field=field):
                document = self.fresh()
                recovery = next(event["data"] for event in document["events"] if event["kind"] == "sender_historical_recovery")
                recovery["native_parser_report"][field] = value
                self.rechain_sender(document)
                with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, "schema, purpose|exact native sender parser|schema version"):
                    self.verify(document)

    def test_sender_parser_cannot_be_forged_by_device_observation_alone(self) -> None:
        document = self.fresh()
        context = next(event for event in document["events"] if event["kind"] == "sender_validity_context")
        recovery = next(event["data"] for event in document["events"] if event["kind"] == "sender_historical_recovery")
        recovery["native_parser_report"]["approvals"] = []
        recovery["hardware_signature"] = _p256_sign(physical.sender_evidence_signing_bytes(context["event_hash"], "sender_historical_recovery", recovery), 2)
        self.rechain_and_approve(document)
        with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, "independent observer threshold"):
            self.verify(document)

    def test_sender_parser_rejects_changed_raw_command_and_receipt_bytes(self) -> None:
        for field in ("command_hex", "receipt_sha256", "release_authorization_sha256"):
            with self.subTest(field=field):
                document = self.fresh()
                recovery = next(event["data"] for event in document["events"] if event["kind"] == "sender_historical_recovery")
                if field == "command_hex":
                    recovery["native_parser_report"][field] += "00"
                else:
                    recovery[field] = _digest("substituted-" + field)
                self.rechain_sender(document)
                with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, "native projection substitutes"):
                    self.verify(document)

    def test_sender_release_requires_actual_service_receipt_authority_observation(self) -> None:
        for kind, replacement in (("payment_acknowledgement", "public_acknowledgement"), ("finalized_redemption_capability", "finalized_redemption_selector")):
            with self.subTest(kind=kind):
                document = self.fresh()
                recovery = next(event["data"] for event in document["events"] if event["kind"] == "sender_historical_recovery" and event["data"]["receipt_kind"] == kind)
                recovery["service_release_observation"]["receipt_authority"] = replacement
                self.rechain_sender(document)
                with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, "service observation"):
                    self.verify(document)

    def test_sender_request_ttl_is_native_bounded_even_for_rejected_commit(self) -> None:
        document = self.fresh()
        attempt = self.sender_attempt(document, "lease_credential_straddle")["data"]
        attempt["request_end_ms"] = attempt["request_start_ms"] + 300_001
        self.rechain_sender(document)
        with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, "request must remain valid"):
            self.verify(document)

    def test_sender_requires_every_case_for_both_operations(self) -> None:
        for case in physical.SENDER_CASES:
            for operation in physical.SENDER_OPERATIONS:
                with self.subTest(case=case, operation=operation):
                    document = self.fresh()
                    attempt = self.sender_attempt(document, case, operation)
                    document["events"].remove(attempt)
                    self.rechain_sender(document)
                    with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, "suffix"):
                        self.verify(document)

    def test_sender_rejects_every_mutated_rejection_snapshot(self) -> None:
        for field in physical.SENDER_SNAPSHOT_FIELDS:
            with self.subTest(field=field):
                document = self.fresh()
                attempt = self.sender_attempt(document, "credential_expiry")
                result = document["events"][attempt["index"] + 1]["data"]
                value = result["after"][field]
                result["after"][field] = _digest("changed-" + field) if isinstance(value, str) else value + 1
                self.rechain_sender(document)
                with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, "rejected sender commit changed"):
                    self.verify(document)

    def test_sender_rejects_resigned_time_and_lease_endpoint_substitution(self) -> None:
        for case, field in (
            ("trusted_valid", "trusted_time_ms"), ("trusted_before_expiry", "trusted_time_ms"),
            ("credential_expiry", "trusted_time_ms"), ("credential_after", "trusted_time_ms"),
            ("lease_credential_straddle", "lease_end_ms"),
            ("lease_empty", "lease_end_ms"), ("lease_zero", "lease_start_ms"),
            ("lease_end", "lease_end_ms"), ("lease_before_credential", "lease_start_ms"),
        ):
            with self.subTest(case=case, field=field):
                document = self.fresh()
                attempt = self.sender_attempt(document, case)
                data = attempt["data"]
                if field == "trusted_time_ms":
                    context = next(event["data"] for event in document["events"] if event["kind"] == "sender_validity_context")
                    credential = context["credentials"][0]
                    document["events"][attempt["index"] + 1]["data"]["decision_trusted_time_ms"] = credential["issued_at_ms"] if case in {"credential_expiry", "credential_after"} else credential["expires_at_ms"]
                else:
                    data[field] += 1
                self.rechain_sender(document)
                with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, "boundary|nonmonotonic"):
                    self.verify(document)

    def test_sender_time_interval_exact_unit_boundaries(self) -> None:
        for case in ("trusted_valid", "trusted_before_expiry", "lease_credential_straddle"):
            self.assertFalse(physical._sender_time_case(case, 99, 100, 200))
            self.assertTrue(physical._sender_time_case(case, 100, 100, 200))
            self.assertTrue(physical._sender_time_case(case, 199, 100, 200))
            self.assertFalse(physical._sender_time_case(case, 200, 100, 200))
        self.assertFalse(physical._sender_time_case("before_issuance", 0, 100, 200))
        self.assertTrue(physical._sender_time_case("before_issuance", 99, 100, 200))
        self.assertFalse(physical._sender_time_case("before_issuance", 100, 100, 200))
        self.assertFalse(physical._sender_time_case("credential_expiry", 199, 100, 200))
        self.assertTrue(physical._sender_time_case("credential_expiry", 200, 100, 200))
        self.assertFalse(physical._sender_time_case("credential_after", 200, 100, 200))
        self.assertTrue(physical._sender_time_case("credential_after", 201, 100, 200))

    def test_sender_credential_encoding_matches_existing_rust_fixture(self) -> None:
        import json

        fixture = json.loads((REPOSITORY_ROOT / "fixtures/offline/kagemusha_v1.json").read_text())

        def fields(payload: bytes) -> list[bytes]:
            result = []
            offset = 0
            while offset < len(payload):
                length = shift = 0
                while True:
                    byte = payload[offset]
                    offset += 1
                    length |= (byte & 127) << shift
                    shift += 7
                    if byte < 128:
                        break
                result.append(payload[offset:offset + length])
                offset += length
            self.assertEqual(offset, len(payload))
            return result

        request = fields(bytes.fromhex(fixture["payment_request"]["norito_hex"])[40:])
        encoded = next(value for value in request if len(value) == 434)
        raw = fields(encoded)
        self.assertEqual(len(raw), len(physical.CREDENTIAL_FIELDS))
        numeric = {"version", "policy_epoch", "hardware_epoch_generation", "issued_at_ms", "expires_at_ms"}
        credential = {name: int.from_bytes(value, "little") if name in numeric else value.hex() for name, value in zip(physical.CREDENTIAL_FIELDS, raw)}
        self.assertEqual(physical.credential_identity(credential), credential["credential_id"])
        self.assertEqual(len(release._norito_frame("iroha.kagemusha.v1.hardware-credential-id-preimage", physical._credential_payload(credential))), 376)
        # Rust's fixture issuer uses the explicit [8; 32] scalar seed; verify its signature
        # independently of our synthetic signing helper.
        issuer = _p256_public(int.from_bytes(bytes([8]) * 32, "big"))
        self.assertTrue(release._p256_verify(bytes.fromhex(issuer), physical.credential_signing_bytes(credential), bytes.fromhex(credential["governance_signature"])))

    def test_sender_recovery_rejects_prior_prefix_boot_replay(self) -> None:
        document = self.fresh()
        prior_boot = document["events"][0]["data"]["boot_id"]
        recovery = next(event["data"] for event in document["events"] if event["kind"] == "sender_historical_recovery")
        recovery["boot_id"] = prior_boot
        self.rechain_sender(document)
        with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, "fresh restart"):
            self.verify(document)

    def test_sender_carries_authenticated_clock_forward_from_prefix(self) -> None:
        document = self.fresh()
        context = next(event["data"] for event in document["events"] if event["kind"] == "sender_validity_context")
        later = context["credentials"][0]["issued_at_ms"] + 100
        for increment, kind in enumerate(("clock_rollback_begin", "clock_rollback_applied", "expired_request_rejected", "clock_rollback_end")):
            next(event["data"] for event in document["events"] if event["kind"] == kind)["trusted_time_ms"] = later + increment
        self.rechain_sender(document)
        with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, "nonmonotonic"):
            self.verify(document)

    def test_sender_recovery_accepts_exact_cached_op7_response_and_final_boot(self) -> None:
        document = self.fresh()
        results = {event["event_hash"]: event["data"] for event in document["events"] if event["kind"] == "sender_admission_result"}
        recoveries = [event["data"] for event in document["events"] if event["kind"] == "sender_historical_recovery"]
        for recovery in recoveries:
            self.assertEqual(recovery["replies"][0]["response_sha256"], results[recovery["original_result_hash"]]["response_sha256"])
        self.assertEqual(document["events"][-1]["data"]["boot_id"], recoveries[-1]["boot_id"])
        self.verify(document)

    def test_sender_decision_time_cannot_reuse_a_valid_preparation_sample(self) -> None:
        document = self.fresh()
        attempt = self.sender_attempt(document, "trusted_before_expiry")
        context = next(event["data"] for event in document["events"] if event["kind"] == "sender_validity_context")
        expires = context["credentials"][0]["expires_at_ms"]
        document["events"][attempt["index"] + 1]["data"]["decision_trusted_time_ms"] = expires
        self.rechain_sender(document)
        with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, "nonmonotonic|boundary"):
            self.verify(document)

    def test_sender_rejects_untrusted_time_and_masking_expired_request_or_reservation(self) -> None:
        for field, value, error in (
            ("source", "host_time", "whole-lease"),
            ("trusted_time_ms", True, "must be an integer"),
            ("request_end_ms", 1, "request must remain valid"),
            ("reservation_end_ms", 1, "reservation must remain valid"),
        ):
            with self.subTest(field=field):
                document = self.fresh()
                self.sender_attempt(document, "credential_expiry")["data"][field] = value
                self.rechain_sender(document)
                with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, error):
                    self.verify(document)

    def test_sender_rejects_issuer_or_canonical_credential_substitution(self) -> None:
        for field, value in (
            ("governance_signature", "11" * 64), ("credential_id", _digest("other-credential")),
            ("lane_commitment", _digest("other-lane")), ("device_key_reference", _digest("other-key")),
            ("issued_at_ms", True), ("hardware_profile_id", _digest("other-profile")),
        ):
            with self.subTest(field=field):
                document = self.fresh()
                context = next(event["data"] for event in document["events"] if event["kind"] == "sender_validity_context")
                context["credentials"][0][field] = value
                self.rechain_sender(document)
                with self.assertRaises(physical.PhysicalDeviceEvidenceError):
                    self.verify(document)

    def test_sender_rejects_new_observer_approval_without_device_authentication(self) -> None:
        document = self.fresh()
        self.sender_attempt(document, "before_issuance")["data"]["command_sha256"] = _digest("substituted-command")
        self.rechain_sender(document, sign_hardware=False)
        with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, "hardware observation signature"):
            self.verify(document)

    def test_sender_rejects_context_reuse_under_fresh_observer_approval(self) -> None:
        document = self.fresh()
        new_context = _digest("other-release-context")
        document["run"]["candidate_context_digest"] = new_context
        next(event["data"] for event in document["events"] if event["kind"] == "sender_validity_context")["candidate_context_digest"] = new_context
        self.rechain_sender(document, sign_hardware=False)
        with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, "hardware observation signature"):
            self.verify(document)

    def test_sender_rejects_operation_and_command_evidence_reuse(self) -> None:
        for field in ("operation_id", "command_sha256", "candidate_sha256", "time_evidence_sha256", "reservation_sha256"):
            with self.subTest(field=field):
                document = self.fresh()
                first = self.sender_attempt(document, "before_issuance")["data"]
                later = self.sender_attempt(document, "trusted_valid")
                later["data"][field] = first[field]
                if field == "operation_id":
                    document["events"][later["index"] + 1]["data"][field] = first[field]
                self.rechain_sender(document)
                with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, "reused|reuses"):
                    self.verify(document)

    def test_sender_recovery_rejects_new_commit_counters_and_changed_terminal_bytes(self) -> None:
        for field in ("state", "counter", "epoch", "authorization_counter", "lease_counter", "journal_revision", "certificate", "envelope", "time", "boot", "receipt"):
            with self.subTest(field=field):
                document = self.fresh()
                recovery = next(event["data"] for event in document["events"] if event["kind"] == "sender_historical_recovery")
                if field in {"certificate", "envelope"}:
                    recovery["replies"][0][field + "_sha256"] = _digest("changed-" + field)
                elif field == "time":
                    recovery["trusted_time_ms"] = document["run"]["started_at_ms"]
                elif field == "boot":
                    recovery["boot_id"] = recovery["prior_boot_id"]
                elif field == "receipt":
                    recovery["receipt_kind"] = "public_redemption_digest"
                else:
                    value = recovery["after"][field]
                    recovery["after"][field] = _digest("recommitted-state") if isinstance(value, str) else value + 1
                self.rechain_sender(document)
                with self.assertRaises(physical.PhysicalDeviceEvidenceError):
                    self.verify(document)

    def test_accepts_signed_physical_transcript_and_derives_report(self) -> None:
        report = self.verify(self.fresh())
        self.assertEqual(report["schema"], physical.REPORT_SCHEMA)
        self.assertEqual(report["physical_checks"], list(physical.PHYSICAL_CHECKS))
        self.assertIs(report["passed"], True)

    def test_rejects_boolean_transcript_schema_version(self) -> None:
        document = self.fresh()
        document["schema_version"] = True
        self.builder.approve(document)
        with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, "schema_version must be an integer"):
            self.verify(document)

    def test_requires_policy_to_admit_exact_physical_verifier(self) -> None:
        trusted = self.policy.verifiers[physical.PHYSICAL_VERIFIER_ID]
        variants = {
            "missing verifier": {},
            "wrong verifier hash": {
                physical.PHYSICAL_VERIFIER_ID: replace(
                    trusted, sha256=_digest("substituted-physical-verifier")
                )
            },
            "missing report schema": {
                physical.PHYSICAL_VERIFIER_ID: replace(
                    trusted,
                    report_schemas=frozenset(
                        {"iroha.kagemusha_v1.acceptance_case_report"}
                    ),
                )
            },
        }
        for label, verifiers in variants.items():
            with self.subTest(label=label):
                policy = replace(self.policy, verifiers=verifiers)
                with self.assertRaisesRegex(
                    physical.PhysicalDeviceEvidenceError,
                    "does not admit this physical-device verifier hash",
                ):
                    physical.verify_bytes(
                        release.canonical_json_bytes(self.fresh()), policy
                    )

    def test_requires_physical_clock_rollback_check_in_release_report(self) -> None:
        self.assertIn("clock_rollback", physical.PHYSICAL_CHECKS)
        self.assertEqual(physical.PHYSICAL_CHECKS, release.PHYSICAL_PROFILE_CHECKS)

    def test_rejects_each_missing_clock_control_boundary(self) -> None:
        for kind in (
            "clock_rollback_begin", "clock_rollback_applied",
            "expired_request_rejected", "clock_rollback_end",
        ):
            with self.subTest(kind=kind):
                document = self.fresh()
                document["events"] = [event for event in document["events"] if event["kind"] != kind]
                self.rechain_and_approve(document)
                with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, "boundary"):
                    self.verify(document)

    def test_rejects_signed_clock_rollback_substitutions(self) -> None:
        cases = (
            ("clock_rollback_applied", "control_id", _digest("other-control"), "control is replayed"),
            ("clock_rollback_begin", "boot_id", _digest("other-boot"), "active hardware boot"),
            ("clock_rollback_applied", "host_time_ms", 20_000, "cross request expiry"),
            ("expired_request_rejected", "host_time_ms", 15_000, "cross request expiry"),
            ("clock_rollback_end", "host_time_ms", 19_999, "cross request expiry"),
            ("clock_rollback_begin", "trusted_time_ms", 15_000, "trusted time remains monotonic"),
            ("clock_rollback_applied", "trusted_time_ms", 19_999, "trusted time remains monotonic"),
            ("expired_request_rejected", "trusted_time_ms", 20_000, "trusted time remains monotonic"),
            ("clock_rollback_end", "trusted_time_ms", 20_001, "trusted time remains monotonic"),
            ("clock_rollback_begin", "request_expires_at_ms", True, "must be an integer"),
            ("expired_request_rejected", "request_sha256", _digest("other-request"), "unchanged authoritative state"),
            ("expired_request_rejected", "authoritative_state", _digest("other-state"), "unchanged authoritative state"),
            ("expired_request_rejected", "counter", 12, "unchanged authoritative state"),
            ("expired_request_rejected", "epoch", 2, "unchanged authoritative state"),
            ("expired_request_rejected", "result", "success", "must be expired_request_rejected"),
        )
        for kind, field, value, error in cases:
            with self.subTest(kind=kind, field=field, value=value):
                document = self.fresh()
                event = next(event for event in document["events"] if event["kind"] == kind)
                event["data"][field] = value
                self.rechain_and_approve(document)
                with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, error):
                    self.verify(document)

    def test_rejects_reused_clock_control_and_operation(self) -> None:
        for reuse in ("control", "operation"):
            with self.subTest(reuse=reuse):
                document = self.fresh()
                by_kind = {event["kind"]: event["data"] for event in document["events"]}
                if reuse == "control":
                    for kind in (
                        "clock_rollback_begin", "clock_rollback_applied",
                        "expired_request_rejected", "clock_rollback_end",
                    ):
                        by_kind[kind]["control_id"] = by_kind["airplane_mode_enabled"]["control_id"]
                    error = "control is replayed"
                else:
                    by_kind["expired_request_rejected"]["operation_id"] = by_kind["prepare"]["operation_id"]
                    error = "operation identifier is reused"
                self.rechain_and_approve(document)
                with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, error):
                    self.verify(document)

    def test_run_start_must_bind_prepared_state_counter_and_epoch(self) -> None:
        replacements = {
            "initial_state": _digest("substituted-initial-state"),
            "counter": 9,
            "epoch": 2,
        }
        for field, replacement in replacements.items():
            with self.subTest(field=field):
                document = self.fresh()
                start = next(
                    event for event in document["events"] if event["kind"] == "run_start"
                )
                start["data"][field] = replacement
                self.rechain_and_approve(document)
                with self.assertRaisesRegex(
                    physical.PhysicalDeviceEvidenceError,
                    "run_start does not bind",
                ):
                    self.verify(document)

    def test_candidate_commit_and_outbox_are_one_durable_envelope(self) -> None:
        cases = (
            "run candidate digest",
            "persisted candidate digest",
            "persisted operation",
            "commit certificate",
            "outbox operation",
            "outbox canonical bytes",
            "outbox certificate",
        )
        for case in cases:
            with self.subTest(case=case):
                document = self.fresh()
                by_kind = {
                    event["kind"]: event for event in document["events"]
                }
                expected_error = "outbox does not bind"
                if case == "run candidate digest":
                    document["run"]["candidate_digest"] = _digest(
                        "substituted-run-candidate"
                    )
                    expected_error = "run.candidate_digest does not bind"
                elif case == "persisted candidate digest":
                    by_kind["candidate_persisted"]["data"]["artifact_sha256"] = (
                        _digest("substituted-persisted-candidate")
                    )
                    expected_error = "run.candidate_digest does not bind"
                elif case == "persisted operation":
                    by_kind["candidate_persisted"]["data"]["operation_id"] = _digest(
                        "substituted-persisted-operation"
                    )
                    expected_error = "prepare/prove/persist/commit"
                elif case == "commit certificate":
                    substituted = _digest("substituted-commit-certificate")
                    by_kind["commit"]["data"]["artifact_sha256"] = substituted
                    by_kind["recover_commit"]["data"]["artifact_sha256"] = substituted
                elif case == "outbox operation":
                    substituted = _digest("substituted-outbox-operation")
                    by_kind["outbox_install"]["data"]["operation_id"] = substituted
                    by_kind["outbox_recover"]["data"]["operation_id"] = substituted
                elif case == "outbox canonical bytes":
                    substituted = _digest("substituted-outbox-envelope")
                    by_kind["outbox_install"]["data"][
                        "canonical_bytes_sha256"
                    ] = substituted
                    by_kind["outbox_recover"]["data"][
                        "canonical_bytes_sha256"
                    ] = substituted
                else:
                    substituted = _digest("substituted-outbox-certificate")
                    by_kind["outbox_install"]["data"][
                        "certificate_sha256"
                    ] = substituted
                    by_kind["outbox_recover"]["data"][
                        "certificate_sha256"
                    ] = substituted
                self.rechain_and_approve(document)
                with self.assertRaisesRegex(
                    physical.PhysicalDeviceEvidenceError, expected_error
                ):
                    self.verify(document)

    def test_rejects_equal_transition_states(self) -> None:
        document = self.fresh()
        advance = next(
            event for event in document["events"] if event["kind"] == "advance_state"
        )
        advance["data"]["successor"] = advance["data"]["predecessor"]
        self.rechain_and_approve(document)
        with self.assertRaisesRegex(
            physical.PhysicalDeviceEvidenceError,
            "distinct predecessor and successor",
        ):
            self.verify(document)

    def test_rejects_operation_id_reuse_outside_recovery(self) -> None:
        cases = ("primary reused by advance", "two rejection attempts reused")
        for case in cases:
            with self.subTest(case=case):
                document = self.fresh()
                by_kind = {
                    event["kind"]: event for event in document["events"]
                }
                if case == "primary reused by advance":
                    by_kind["advance_state"]["data"]["operation_id"] = by_kind[
                        "prepare"
                    ]["data"]["operation_id"]
                else:
                    by_kind["stale_predecessor_rejected"]["data"][
                        "operation_id"
                    ] = by_kind["second_successor_rejected"]["data"][
                        "operation_id"
                    ]
                self.rechain_and_approve(document)
                with self.assertRaisesRegex(
                    physical.PhysicalDeviceEvidenceError,
                    "operation identifier is reused",
                ):
                    self.verify(document)

    def test_cli_emits_one_canonical_report(self) -> None:
        document = self.fresh()
        expected = self.verify(document)
        with tempfile.TemporaryDirectory() as temporary:
            evidence_path = Path(temporary) / "physical-evidence.json"
            evidence_payload = release.canonical_json_bytes(document)
            evidence_path.write_bytes(evidence_payload)
            result = subprocess.run(
                [
                    sys.executable,
                    str(Path(physical.__file__).resolve()),
                    "--evidence",
                    str(evidence_path.resolve()),
                    "--evidence-sha256",
                    hashlib.sha256(evidence_payload).hexdigest(),
                    "--observer-policy",
                    str(self.policy_path),
                    "--observer-policy-sha256",
                    self.policy_sha256,
                ],
                check=False,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=30,
            )
        self.assertEqual(result.returncode, 0, result.stderr.decode("utf-8"))
        self.assertEqual(result.stdout, release.canonical_json_bytes(expected))

    def test_rejects_tampered_hash_chain(self) -> None:
        document = self.fresh()
        next(event for event in document["events"] if event["kind"] == "thermal_fold")["data"][
            "energy_millijoules"
        ] += 1
        with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, "event_hash"):
            self.verify(document)

    def test_rejects_replayed_operation(self) -> None:
        document = self.fresh()
        folds = [event for event in document["events"] if event["kind"] == "thermal_fold"]
        folds[1]["data"] = copy.deepcopy(folds[0]["data"])
        self.rechain_and_approve(document)
        with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, "replays an operation"):
            self.verify(document)

    def test_rejects_missing_recovery_boundary(self) -> None:
        document = self.fresh()
        document["events"] = [event for event in document["events"] if event["kind"] != "recover_commit"]
        self.rechain_and_approve(document)
        with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, "boundary"):
            self.verify(document)

    def test_rejects_each_missing_or_substituted_crash_control_cycle(self) -> None:
        for kind, occurrence_count in (("restart_begin", 2), ("power_loss_begin", 3)):
            for occurrence in range(occurrence_count):
                with self.subTest(kind=kind, occurrence=occurrence):
                    document = self.fresh()
                    matching = [
                        index
                        for index, event in enumerate(document["events"])
                        if event["kind"] == kind
                    ]
                    document["events"].pop(matching[occurrence])
                    self.rechain_and_approve(document)
                    with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, "boundary"):
                        self.verify(document)

        substituted = self.fresh()
        restart_begins = [
            event for event in substituted["events"] if event["kind"] == "restart_begin"
        ]
        restart_ends = [
            event for event in substituted["events"] if event["kind"] == "restart_end"
        ]
        replayed_control = restart_begins[0]["data"]["control_id"]
        restart_begins[1]["data"]["control_id"] = replayed_control
        restart_ends[1]["data"]["control_id"] = replayed_control
        self.rechain_and_approve(substituted)
        with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, "replayed"):
            self.verify(substituted)

    def test_rejects_software_endpoint(self) -> None:
        document = self.fresh()
        document["endpoint"]["kind"] = "software"
        self.builder.approve(document)
        with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, "allowed value"):
            self.verify(document)

    def test_rejects_simulator_endpoint(self) -> None:
        document = self.fresh()
        document["endpoint"]["platform_class"] = "simulator"
        self.builder.approve(document)
        with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, "allowed value"):
            self.verify(document)

    def test_rejects_resource_limit_violation(self) -> None:
        document = self.fresh()
        next(event for event in document["events"] if event["kind"] == "thermal_fold")["data"][
            "rss_bytes"
        ] = physical.MAX_RSS_BYTES + 1
        self.rechain_and_approve(document)
        with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, "RSS exceeds"):
            self.verify(document)

    def test_rejects_invalid_observer_signature(self) -> None:
        document = self.fresh()
        signature = bytearray.fromhex(document["approvals"][0]["signature"])
        signature[0] ^= 1
        document["approvals"][0]["signature"] = bytes(signature).hex()
        with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, "invalid detached"):
            self.verify(document)

    def test_rejects_unknown_and_missing_fields(self) -> None:
        unknown = self.fresh()
        unknown["passed"] = True
        with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, "unknown fields"):
            self.verify(unknown)
        missing = self.fresh()
        del missing["endpoint"]["attestation_digest"]
        with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, "missing fields"):
            self.verify(missing)

    def test_rejects_oversized_or_noncanonical_input(self) -> None:
        with self.assertRaisesRegex(physical.PhysicalDeviceEvidenceError, "between 1"):
            physical.verify_bytes(b"x" * (physical.MAX_EVIDENCE_BYTES + 1), self.policy)
        pretty = (str(self.fresh())).encode("utf-8")
        with self.assertRaises(physical.PhysicalDeviceEvidenceError):
            physical.verify_bytes(pretty, self.policy)


if __name__ == "__main__":
    unittest.main()
