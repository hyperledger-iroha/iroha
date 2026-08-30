"""Tests for the fail-closed AtomicPrivateSettlementV1 release runner."""

from __future__ import annotations

import copy
import importlib.util
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "private_settlement_release_runner.py"
SPEC = importlib.util.spec_from_file_location(
    "private_settlement_release_runner", SCRIPT
)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

COMMIT = "a" * 40
HARDWARE = "b" * 64
HARDWARE_PROFILE = "9" * 64
CONFIGURATION = "c" * 64
EXECUTABLE = "d" * 64
INVOCATION_NONCE = "8" * 64


def plan() -> dict[str, Any]:
    """Return the response-validation subset of a frozen plan."""

    return {
        "commit": COMMIT,
        "hardware": {
            "sha256": HARDWARE,
            "profile_sha256": HARDWARE_PROFILE,
        },
    }


def process_inventory(participants: int) -> list[dict[str, Any]]:
    """Build the exact real-process topology acknowledgement."""

    rows: list[tuple[str, int | None, int | None]] = [("coordinator", None, None)]
    rows.extend(
        ("global_validator", None, validator)
        for validator in range(MODULE.GLOBAL_VALIDATORS)
    )
    rows.extend(
        ("dataspace_validator", dataspace, validator)
        for dataspace in range(participants)
        for validator in range(MODULE.VALIDATORS_PER_DATASPACE)
    )
    return [
        {
            "role": role,
            "dataspace_ordinal": dataspace,
            "validator_ordinal": validator,
            "pid": index + 100,
            "executable_sha256": EXECUTABLE,
            "revision": COMMIT,
            "health_observed": True,
        }
        for index, (role, dataspace, validator) in enumerate(rows)
    ]


def fault_job(participants: int = 3) -> dict[str, Any]:
    return {
        "request_id": "e" * 64,
        "invocation_nonce": INVOCATION_NONCE,
        "kind": "fault",
        "participants": participants,
        "seed": 7,
        "run": 2,
        "configuration_sha256": CONFIGURATION,
    }


def fault_payload(participants: int = 3) -> dict[str, Any]:
    """Return every required controller and persistence acknowledgement."""

    return {
        "committee_validator_restarts": list(range(participants)),
        "maximum_simultaneously_unavailable_per_committee": 1,
        "quorum_progress_with_one_unavailable": True,
        "coordinator_restarted": True,
        "global_node_restarted": True,
        "prepare_qc_normalization": {
            "first_signer_subset": [0, 1, 2],
            "second_signer_subset": [0, 1, 3],
            "certified_body_sha256": "1" * 64,
            "first_qc_sha256": "2" * 64,
            "second_qc_sha256": "3" * 64,
            "first_normalized_barrier_sha256": "4" * 64,
            "second_normalized_barrier_sha256": "4" * 64,
            "equivalent_subsets_accepted": True,
            "changed_body_rejected": True,
            "authority_index_binding_verified": True,
            "signed_body_binding_verified": True,
        },
        "loss_trials": [
            {
                "phase": phase,
                "loss_percent": percentage,
                "control_acknowledged": True,
                "healed": True,
                "converged": True,
                "partial_visibility_observed": False,
            }
            for phase in MODULE.fault_report.REQUIRED_LOSS_PHASES
            for percentage in MODULE.fault_report.REQUIRED_LOSS_PERCENTAGES
        ],
        "phase_cut_partitions": [
            {
                "cut": cut,
                "control_acknowledged": True,
                "delayed_delivery": True,
                "healed": True,
                "converged": True,
                "partial_visibility_observed": False,
            }
            for cut in MODULE.fault_report.REQUIRED_PHASE_CUTS
        ],
        "crash_recoveries": [
            {
                "boundary": boundary,
                "process_restarted": True,
                "durable_state_reconciled": True,
                "converged": True,
                "partial_visibility_observed": False,
            }
            for boundary in MODULE.fault_report.REQUIRED_CRASH_BOUNDARIES
        ],
        "atomicity": {
            "continuous_checks": 100,
            "partial_visible_observations": 0,
            "partial_spendable_observations": 0,
            "aborted_private_state_changes": 0,
            "successful_leg_applications": participants,
            "each_leg_applied_exactly_once": True,
            "invalid_leg_state_byte_identical": True,
            "replay_rejected": True,
        },
        "all_nodes_converged": True,
    }


def response(job: dict[str, Any], payload: dict[str, Any]) -> dict[str, Any]:
    """Wrap a job-specific payload in the exact process-harness envelope."""

    return {
        "version": MODULE.VERSION,
        "protocol": MODULE.PROTOCOL,
        "request_id": job["request_id"],
        "invocation_nonce": job["invocation_nonce"],
        "kind": job["kind"],
        "commit": COMMIT,
        "hardware_sha256": HARDWARE,
        "hardware_profile_sha256": HARDWARE_PROFILE,
        "configuration_sha256": job["configuration_sha256"],
        "participants": job["participants"],
        "passed": True,
        "mandatory_signed_rs16_da_rbc": True,
        "signed_rs16_da_observations": (
            MODULE.minimum_signed_rs16_da_observations(job["participants"])
        ),
        "authenticated_message_control": True,
        "process_inventory": process_inventory(job["participants"]),
        "payload": payload,
    }


class PrivateSettlementReleaseRunnerTests(unittest.TestCase):
    """Exercise deterministic planning and fail-closed response materialization."""

    def test_job_matrix_is_complete_canonical_and_deterministic(self) -> None:
        canaries = MODULE.build_canary_manifest(COMMIT)
        configurations = {
            participants: f"{participants:064x}"
            for participants in MODULE.PARTICIPANTS
        }
        first = MODULE.build_jobs(
            configurations,
            tuple(range(10)),
            MODULE.MIN_WARMUPS,
            MODULE.MIN_MEASURED,
            canaries,
        )
        second = MODULE.build_jobs(
            configurations,
            tuple(range(10)),
            MODULE.MIN_WARMUPS,
            MODULE.MIN_MEASURED,
            canaries,
        )
        expected = (
            len(MODULE.PARTICIPANTS) * 10
            + len(MODULE.PROFILES)
            * len(MODULE.PARTICIPANTS)
            * (MODULE.MIN_WARMUPS + MODULE.MIN_MEASURED)
            + 2
        )
        self.assertEqual(first, second)
        self.assertEqual(len(first), expected)
        self.assertEqual(len({job["request_id"] for job in first}), expected)
        self.assertEqual(first[0]["kind"], "fault")
        self.assertEqual(first[-2]["variant"], "left")
        self.assertEqual(first[-1]["variant"], "right")

    def test_canary_sets_cover_both_secret_only_variants(self) -> None:
        manifest = MODULE.build_canary_manifest(COMMIT)
        names = [entry["name"] for entry in manifest["canaries"]]
        self.assertEqual(names, sorted(names))
        self.assertTrue(
            set(MODULE.release_evidence.REQUIRED_LEAKAGE_CANARY_NAMES).issubset(names)
        )
        left = MODULE.canaries_for_variant(manifest, "left")
        right = MODULE.canaries_for_variant(manifest, "right")
        self.assertEqual(len(left), 6)
        self.assertEqual(len(right), 6)
        self.assertTrue(
            all(
                MODULE.object_digest(a) != MODULE.object_digest(b)
                for a, b in zip(left, right)
            )
        )

    def test_process_inventory_must_name_every_real_validator_and_coordinator(
        self,
    ) -> None:
        MODULE.validate_process_inventory(
            process_inventory(3), participants=3, commit=COMMIT, label="fixture"
        )
        missing = process_inventory(3)[:-1]
        with self.assertRaisesRegex(MODULE.RunnerError, "process topology mismatch"):
            MODULE.validate_process_inventory(
                missing, participants=3, commit=COMMIT, label="fixture"
            )
        duplicate_pid = process_inventory(3)
        duplicate_pid[-1]["pid"] = duplicate_pid[0]["pid"]
        with self.assertRaisesRegex(MODULE.RunnerError, "reuses PID"):
            MODULE.validate_process_inventory(
                duplicate_pid, participants=3, commit=COMMIT, label="fixture"
            )
        reordered = process_inventory(3)
        reordered[1], reordered[2] = reordered[2], reordered[1]
        with self.assertRaisesRegex(MODULE.RunnerError, "reordered"):
            MODULE.validate_process_inventory(
                reordered, participants=3, commit=COMMIT, label="fixture"
            )

    def test_common_response_requires_freshness_and_validator_scaled_da(self) -> None:
        job = fault_job()
        valid = response(job, fault_payload())
        MODULE.validate_common_response(valid, plan=plan(), job=job)
        stale = copy.deepcopy(valid)
        stale["invocation_nonce"] = "7" * 64
        with self.assertRaisesRegex(MODULE.RunnerError, "frozen request"):
            MODULE.validate_common_response(stale, plan=plan(), job=job)
        insufficient_da = copy.deepcopy(valid)
        insufficient_da["signed_rs16_da_observations"] -= 1
        with self.assertRaisesRegex(MODULE.RunnerError, "cover every validator"):
            MODULE.validate_common_response(
                insufficient_da, plan=plan(), job=job
            )

    def test_fault_response_materializes_reporter_valid_bound_evidence(self) -> None:
        job = fault_job()
        result = response(job, fault_payload())
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            raw, artifacts = MODULE.materialize_fault_response(
                result,
                plan=plan(),
                job=job,
                publication_root=root,
            )
            parsed = MODULE.fault_report.parse_run(raw, "fixture")
            self.assertEqual(parsed[:3], (3, 7, 2))
            self.assertEqual(
                {artifact["kind"] for artifact in artifacts},
                {"operator_log", "sanitized_capture"},
            )
            for artifact in artifacts:
                self.assertTrue((root / artifact["path"]).is_file())

    def test_any_missing_control_acknowledgement_fails_closed(self) -> None:
        job = fault_job()
        result = response(job, fault_payload())
        result["payload"]["loss_trials"][0]["control_acknowledged"] = False
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaisesRegex(MODULE.RunnerError, "fault harness result"):
                MODULE.materialize_fault_response(
                    result,
                    plan=plan(),
                    job=job,
                    publication_root=Path(temporary),
                )
        result = response(job, fault_payload())
        result["payload"]["prepare_qc_normalization"][
            "second_normalized_barrier_sha256"
        ] = "5" * 64
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaisesRegex(MODULE.RunnerError, "quorum-equivalent"):
                MODULE.materialize_fault_response(
                    result,
                    plan=plan(),
                    job=job,
                    publication_root=Path(temporary),
                )

    def test_benchmark_requires_positive_real_measurements_and_atomic_finality(
        self,
    ) -> None:
        job = {
            "request_id": "f" * 64,
            "invocation_nonce": INVOCATION_NONCE,
            "kind": "benchmark",
            "profile": "private",
            "participants": 3,
            "seed": 1,
            "run": 0,
            "warmup": False,
            "configuration_sha256": CONFIGURATION,
        }
        payload = {
            "stages_ms": {
                stage: float(index + 1)
                for index, stage in enumerate(
                    MODULE.benchmark_report.REQUIRED_PRIVATE_STAGES
                )
            },
            **{
                field: 10.0
                for field in MODULE.benchmark_report.RESOURCE_FIELDS
            },
            "finalized_receipt_observed": True,
            "successful_leg_applications": 3,
            "each_leg_applied_exactly_once": True,
            "partial_visible_observations": 0,
            "partial_spendable_observations": 0,
        }
        raw = MODULE.materialize_benchmark_response(
            response(job, payload), plan=plan(), job=job
        )
        self.assertEqual(raw["profile"], "private")
        broken = copy.deepcopy(payload)
        broken["network_bytes"] = 0
        with self.assertRaisesRegex(
            MODULE.RunnerError, "network_bytes must be positive"
        ):
            MODULE.materialize_benchmark_response(
                response(job, broken), plan=plan(), job=job
            )

    def test_leakage_response_must_bind_every_capture_file_and_count(self) -> None:
        canaries = MODULE.build_canary_manifest(COMMIT)
        selected = MODULE.canaries_for_variant(canaries, "left")
        job = {
            "request_id": "1" * 64,
            "invocation_nonce": INVOCATION_NONCE,
            "kind": "leakage",
            "participants": 3,
            "seed": 0,
            "run": 0,
            "variant": "left",
            "canary_names": [entry["name"] for entry in selected],
            "canary_commitments": {
                entry["name"]: MODULE.object_digest(entry) for entry in selected
            },
            "configuration_sha256": CONFIGURATION,
        }
        with tempfile.TemporaryDirectory() as temporary:
            evidence = Path(temporary)
            artifact_rows = []
            for index, surface in enumerate(sorted(MODULE.SURFACE_FILES)):
                path = evidence / MODULE.SURFACE_FILES[surface]
                if path.suffix == ".json":
                    path.write_text(
                        json.dumps({"opaque": f"capture-{index:02d}"}) + "\n",
                        encoding="utf-8",
                    )
                else:
                    path.write_bytes(f"opaque-capture-{index:02d}\n".encode())
                artifact_rows.append(
                    {
                        "surface": surface,
                        "relative_name": MODULE.SURFACE_FILES[surface],
                        **MODULE.file_binding(path),
                    }
                )
            payload = {
                "variant": "left",
                "canaries_injected": job["canary_names"],
                "canary_commitments": job["canary_commitments"],
                "only_secret_fields_changed": True,
                "capture_complete": True,
                "finalized_receipt_observed": True,
                "successful_leg_applications": 3,
                "each_leg_applied_exactly_once": True,
                "partial_visible_observations": 0,
                "partial_spendable_observations": 0,
                "artifacts": artifact_rows,
                "message_counts": {
                    channel: 1
                    for channel in MODULE.leakage_audit.REQUIRED_COUNT_CHANNELS
                },
            }
            counts, surfaces = MODULE.validate_leakage_response(
                response(job, payload),
                plan=plan(),
                job=job,
                evidence_dir=evidence,
            )
            self.assertEqual(len(surfaces), len(MODULE.SURFACE_FILES))
            self.assertTrue(all(value == 1 for value in counts.values()))
            self.assertTrue(all(binding["bytes"] > 0 for _, _, binding in surfaces))
            broken = copy.deepcopy(payload)
            broken["artifacts"] = broken["artifacts"][:-1]
            with self.assertRaisesRegex(MODULE.RunnerError, "every required surface"):
                MODULE.validate_leakage_response(
                    response(job, broken),
                    plan=plan(),
                    job=job,
                    evidence_dir=evidence,
                )
            reordered = copy.deepcopy(payload)
            reordered["artifacts"][0], reordered["artifacts"][1] = (
                reordered["artifacts"][1],
                reordered["artifacts"][0],
            )
            with self.assertRaisesRegex(MODULE.RunnerError, "canonically ordered"):
                MODULE.validate_leakage_response(
                    response(job, reordered),
                    plan=plan(),
                    job=job,
                    evidence_dir=evidence,
                )
            empty_counts = copy.deepcopy(payload)
            first_channel = MODULE.leakage_audit.REQUIRED_COUNT_CHANNELS[0]
            empty_counts["message_counts"][first_channel] = 0
            with self.assertRaisesRegex(MODULE.RunnerError, "must be in 1"):
                MODULE.validate_leakage_response(
                    response(job, empty_counts),
                    plan=plan(),
                    job=job,
                    evidence_dir=evidence,
                )
            empty_capture = copy.deepcopy(payload)
            empty_row = empty_capture["artifacts"][-1]
            empty_path = evidence / empty_row["relative_name"]
            original_bytes = empty_path.read_bytes()
            empty_path.write_bytes(b"")
            empty_row.update(MODULE.file_binding(empty_path))
            with self.assertRaisesRegex(MODULE.RunnerError, "must not be empty"):
                MODULE.validate_leakage_response(
                    response(job, empty_capture),
                    plan=plan(),
                    job=job,
                    evidence_dir=evidence,
                )
            empty_path.write_bytes(original_bytes)
            surface, source, expected_binding = surfaces[0]
            source.write_bytes(source.read_bytes() + b"mutation")
            with self.assertRaisesRegex(MODULE.RunnerError, "changed before copy"):
                MODULE.copy_bound_file(
                    source,
                    evidence / f"copy-{surface}",
                    expected=expected_binding,
                )

    def test_differential_manifest_is_accepted_by_release_validator(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            artifacts: dict[Any, Any] = {}
            for variant in ("left", "right"):
                for index, surface in enumerate(sorted(MODULE.SURFACE_FILES)):
                    path = root / "leakage" / variant / MODULE.SURFACE_FILES[surface]
                    path.parent.mkdir(parents=True, exist_ok=True)
                    if path.suffix == ".json":
                        path.write_text(
                            json.dumps({"opaque": f"capture-{index:02d}"}) + "\n",
                            encoding="utf-8",
                        )
                    else:
                        content = bytearray(
                            f"opaque-capture-{index:02d}\n".encode()
                        )
                        if (
                            variant == "right"
                            and surface
                            in MODULE.REQUIRED_DIFFERENTIAL_STATE_CHANGES
                        ):
                            content[0] ^= 1
                        path.write_bytes(content)
                    binding = MODULE.file_binding(path, relative_to=root)
                    relative = MODULE.PurePosixPath(binding["path"])
                    artifacts[relative] = MODULE.release_evidence.Artifact(
                        kind=surface,
                        path=relative,
                        sha256=binding["sha256"],
                        bytes=binding["bytes"],
                    )
            manifest_path = root / "differential-pairs-v1.json"
            MODULE.write_json(
                manifest_path,
                MODULE.differential_pair_manifest(root, COMMIT),
            )
            bindings = MODULE.release_evidence._validate_differential_pair_manifest(
                manifest_path,
                commit=COMMIT,
                root=root,
                artifacts_by_path=artifacts,
            )
            self.assertEqual(len(bindings), len(MODULE.SURFACE_FILES) * 2)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            self.assertEqual(
                [pair["surface"] for pair in manifest["pairs"]],
                sorted(MODULE.SURFACE_FILES),
            )
            canary_path = root / "canary-manifest-v1.json"
            MODULE.write_json(canary_path, MODULE.build_canary_manifest(COMMIT))
            count_paths = []
            for variant in ("left", "right"):
                count_path = root / f"message-counts-{variant}.json"
                MODULE.write_json(
                    count_path,
                    {
                        "version": 1,
                        "channels": {
                            channel: 1
                            for channel in MODULE.leakage_audit.REQUIRED_COUNT_CHANNELS
                        },
                    },
                )
                count_paths.append(count_path)
            audit = MODULE.leakage_audit.run_audit(
                canary_path,
                [
                    *(root / "leakage" / "left").iterdir(),
                    *(root / "leakage" / "right").iterdir(),
                    *count_paths,
                ],
                differential_left=root / "leakage" / "left",
                differential_right=root / "leakage" / "right",
                message_counts_left=count_paths[0],
                message_counts_right=count_paths[1],
            )
            self.assertTrue(audit["passed"])
            changed_surface = "block_wire_capture"
            left_state = (
                root
                / "leakage"
                / "left"
                / MODULE.SURFACE_FILES[changed_surface]
            )
            right_state = (
                root
                / "leakage"
                / "right"
                / MODULE.SURFACE_FILES[changed_surface]
            )
            right_bytes = right_state.read_bytes()
            right_state.write_bytes(left_state.read_bytes())
            with self.assertRaisesRegex(MODULE.RunnerError, "did not change"):
                MODULE.differential_pair_manifest(root, COMMIT)
            right_state.write_bytes(right_bytes)

    def test_bound_plan_paths_reject_symlinked_parent_components(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            root = workspace / "plan"
            outside = workspace / "outside"
            root.mkdir()
            outside.mkdir()
            (outside / "config.json").write_text("{}\n", encoding="utf-8")
            (root / "linked").symlink_to(outside, target_is_directory=True)
            with self.assertRaisesRegex(MODULE.RunnerError, "symbolic link"):
                MODULE.regular_file_under(
                    root,
                    MODULE.PurePosixPath("linked/config.json"),
                    "fixture",
                )

    def test_strict_json_rejects_duplicate_keys_and_nonfinite_values(self) -> None:
        with self.assertRaisesRegex(MODULE.RunnerError, "duplicate key"):
            MODULE.strict_json_loads('{"passed":true,"passed":false}', "fixture")
        with self.assertRaisesRegex(MODULE.RunnerError, "non-JSON constant"):
            MODULE.strict_json_loads('{"latency":NaN}', "fixture")

    def test_request_revalidates_canary_contents_and_commitments(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            canary_path = root / "canary-manifest-v1.json"
            canaries = MODULE.build_canary_manifest(COMMIT)
            MODULE.write_json(canary_path, canaries)
            configuration_path = root / "configurations" / "n3.json"
            MODULE.write_json(
                configuration_path,
                MODULE.build_configuration(
                    3,
                    seeds=tuple(range(MODULE.MIN_FAULT_SEEDS)),
                    warmups=MODULE.MIN_WARMUPS,
                    measured=MODULE.MIN_MEASURED,
                ),
            )
            configuration_binding = MODULE.file_binding(configuration_path)
            manifest_path = root / "configuration-manifest-v1.json"
            MODULE.write_json(
                manifest_path,
                {
                    "configurations": [
                        {
                            "participants": 3,
                            "path": "configurations/n3.json",
                            **configuration_binding,
                        }
                    ]
                },
            )
            selected = MODULE.canaries_for_variant(canaries, "left")
            job = {
                "request_id": "1" * 64,
                "invocation_nonce": INVOCATION_NONCE,
                "kind": "leakage",
                "participants": 3,
                "seed": 0,
                "run": 0,
                "variant": "left",
                "canary_names": [entry["name"] for entry in selected],
                "canary_commitments": {
                    entry["name"]: MODULE.object_digest(entry)
                    for entry in selected
                },
                "configuration_sha256": configuration_binding["sha256"],
            }
            frozen_plan = {
                **plan(),
                "configuration_manifest": {
                    "path": manifest_path.name,
                    **MODULE.file_binding(manifest_path),
                },
                "canary_manifest": {
                    "path": canary_path.name,
                    **MODULE.file_binding(canary_path),
                },
            }
            request = MODULE.build_request(frozen_plan, root, job)
            self.assertEqual(request["payload"]["canaries"], selected)
            canaries["canaries"][0]["value"] = "changed-secret"
            MODULE.write_json(canary_path, canaries)
            frozen_plan["canary_manifest"].update(
                MODULE.file_binding(canary_path)
            )
            with self.assertRaisesRegex(MODULE.RunnerError, "frozen job binding"):
                MODULE.build_request(frozen_plan, root, job)

    def test_success_without_response_file_is_not_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            harness = Path(temporary) / "empty-harness.sh"
            harness.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            os.chmod(harness, 0o700)
            with self.assertRaisesRegex(
                MODULE.RunnerError, "without a regular response"
            ):
                MODULE.invoke_harness(
                    harness,
                    {"kind": "fault"},
                    timeout_seconds=5,
                )

    def test_publication_fragment_replays_applicable_final_validators(self) -> None:
        from scripts.tests.private_settlement_release_evidence_test import (
            PrivateSettlementReleaseEvidenceTests,
        )

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = PrivateSettlementReleaseEvidenceTests().make_bundle(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            MODULE.validate_publication_fragment(
                root,
                manifest["artifacts"],
                commit=manifest["commit"],
            )
            benchmark_artifact = next(
                artifact
                for artifact in manifest["artifacts"]
                if artifact["kind"] == "benchmark_report"
            )
            baseline = json.loads(
                (root / benchmark_artifact["path"]).read_text(encoding="utf-8")
            )
            MODULE.validate_benchmark_baseline(baseline, "fixture baseline")
            baseline["passed"] = False
            with self.assertRaises(MODULE.RunnerError):
                MODULE.validate_benchmark_baseline(baseline, "fixture baseline")

    def test_plan_seed_and_sample_minima_cannot_be_weakened(self) -> None:
        with self.assertRaises(MODULE.RunnerError):
            MODULE.verify_seed_policy(tuple(range(9)))
        with self.assertRaises(MODULE.RunnerError):
            MODULE.verify_seed_policy((0, 1, 2, 3, 4, 5, 6, 7, 8, 8))
        with self.assertRaisesRegex(MODULE.RunnerError, "unsigned 64-bit"):
            MODULE.verify_seed_policy(
                (*range(9), MODULE.MAX_SEED + 1)
            )
        with self.assertRaisesRegex(MODULE.RunnerError, "at most"):
            MODULE.verify_seed_policy(tuple(range(MODULE.MAX_FAULT_SEEDS + 1)))
        with self.assertRaisesRegex(MODULE.RunnerError, "warmups"):
            MODULE.build_configuration(
                3,
                seeds=tuple(range(10)),
                warmups=MODULE.MAX_WARMUPS + 1,
                measured=MODULE.MIN_MEASURED,
            )
        configuration = MODULE.build_configuration(
            3,
            seeds=tuple(range(10)),
            warmups=5,
            measured=30,
        )
        self.assertTrue(
            configuration["consensus"]["mandatory_signed_rs16_da_rbc"]
        )
        self.assertFalse(configuration["consensus"]["legacy_rbc_bypass_permitted"])
        self.assertTrue(
            configuration["fault_matrix"]["prepare_qc_normalization"][
                "accept_equivalent_subsets_only_for_identical_body"
            ]
        )
        self.assertEqual(
            configuration["topology"]["total_validator_processes"], 16
        )
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary)
            with self.assertRaisesRegex(MODULE.RunnerError, "outside"):
                MODULE.require_external_output(source / "evidence", source)


if __name__ == "__main__":
    unittest.main()
