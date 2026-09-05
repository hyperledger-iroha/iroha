"""Regressions for the closed KAGEMUSHA V1 physical-device verifier."""

from __future__ import annotations

import copy
import hashlib
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
                "artifact_set_digest": _digest("artifacts"),
                "started_at_ms": 1_700_000_000_000,
                "ended_at_ms": 1_700_000_200_000,
            },
            "events": self.events,
            "approvals": [],
        }
        self.approve(document)
        return document

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

    def test_accepts_signed_physical_transcript_and_derives_report(self) -> None:
        report = self.verify(self.fresh())
        self.assertEqual(report["schema"], physical.REPORT_SCHEMA)
        self.assertEqual(report["physical_checks"], list(physical.PHYSICAL_CHECKS))
        self.assertIs(report["passed"], True)

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
