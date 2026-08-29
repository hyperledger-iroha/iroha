"""Tests for the AtomicPrivateSettlementV1 real-process fault reporter."""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "private_settlement_fault_report.py"
SPEC = importlib.util.spec_from_file_location("private_settlement_fault_report", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def run_record(participants: int, seed: int, run: int) -> dict[str, object]:
    """Build one complete synthetic record for structural validation tests."""

    transcript_sha256 = MODULE.hashlib.sha256(
        b"aggregate authenticated fault-control transcript\n"
    ).hexdigest()
    capture_sha256 = MODULE.hashlib.sha256(
        b"aggregate atomicity observation capture\n"
    ).hexdigest()

    def evidence_record(kind: str, name: str) -> str:
        return f"n{participants}:s{seed}:r{run}:{kind}:{name}"

    return {
        "version": 1,
        "protocol": MODULE.PROTOCOL,
        "commit": "a" * 40,
        "hardware_sha256": "b" * 64,
        "configuration_sha256": f"{participants:064x}",
        "participants": participants,
        "seed": seed,
        "run": run,
        "validators_per_dataspace": 4,
        "quorum": "3-of-4",
        "mandatory_signed_rs16_da_rbc": True,
        "authenticated_message_control": True,
        "committee_validator_restarts": list(range(participants)),
        "maximum_simultaneously_unavailable_per_committee": 1,
        "quorum_progress_with_one_unavailable": True,
        "coordinator_restarted": True,
        "global_node_restarted": True,
        "loss_trials": [
            {
                "phase": phase,
                "loss_percent": percentage,
                "control_acknowledged": True,
                "healed": True,
                "converged": True,
                "partial_visibility_observed": False,
                "control_transcript_sha256": transcript_sha256,
                "control_transcript_record": evidence_record(
                    "loss", f"{phase}:{percentage}"
                ),
                "observation_capture_sha256": capture_sha256,
                "observation_capture_record": evidence_record(
                    "loss", f"{phase}:{percentage}"
                ),
            }
            for phase in MODULE.REQUIRED_LOSS_PHASES
            for percentage in MODULE.REQUIRED_LOSS_PERCENTAGES
        ],
        "phase_cut_partitions": [
            {
                "cut": cut,
                "control_acknowledged": True,
                "delayed_delivery": True,
                "healed": True,
                "converged": True,
                "partial_visibility_observed": False,
                "control_transcript_sha256": transcript_sha256,
                "control_transcript_record": evidence_record("cut", cut),
                "observation_capture_sha256": capture_sha256,
                "observation_capture_record": evidence_record("cut", cut),
            }
            for cut in MODULE.REQUIRED_PHASE_CUTS
        ],
        "crash_recoveries": [
            {
                "boundary": boundary,
                "process_restarted": True,
                "durable_state_reconciled": True,
                "converged": True,
                "partial_visibility_observed": False,
                "control_transcript_sha256": transcript_sha256,
                "control_transcript_record": evidence_record("crash", boundary),
                "observation_capture_sha256": capture_sha256,
                "observation_capture_record": evidence_record("crash", boundary),
            }
            for boundary in MODULE.REQUIRED_CRASH_BOUNDARIES
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


def complete_matrix() -> list[tuple[int, int, int, str, str, str]]:
    return [
        MODULE.parse_run(run_record(participants, seed, seed), "fixture")
        for participants in MODULE.REQUIRED_PARTICIPANTS
        for seed in range(MODULE.REQUIRED_SEEDS_PER_PARTICIPANT)
    ]


class PrivateSettlementFaultReportTests(unittest.TestCase):
    """Exercise exact matrix, controller, crash, and atomicity checks."""

    def test_complete_ten_seed_matrix_passes(self) -> None:
        report = MODULE.build_report(complete_matrix())
        self.assertTrue(report["passed"])
        self.assertEqual(set(report["matrix"]), {"2", "3", "4", "8", "16"})

    def test_missing_participant_or_seed_is_rejected(self) -> None:
        with self.assertRaises(MODULE.FaultEvidenceError):
            MODULE.build_report([run for run in complete_matrix() if run[0] != 16])
        with self.assertRaises(MODULE.FaultEvidenceError):
            MODULE.build_report(
                [run for run in complete_matrix() if not (run[0] == 3 and run[1] == 9)]
            )

    def test_unacknowledged_loss_or_missing_crash_boundary_is_rejected(self) -> None:
        unacknowledged = run_record(3, 0, 0)
        unacknowledged["loss_trials"][0]["control_acknowledged"] = False  # type: ignore[index]
        with self.assertRaises(MODULE.FaultEvidenceError):
            MODULE.parse_run(unacknowledged, "fixture")
        missing_crash = run_record(3, 0, 0)
        missing_crash["crash_recoveries"] = missing_crash["crash_recoveries"][:-1]  # type: ignore[index]
        with self.assertRaises(MODULE.FaultEvidenceError):
            MODULE.parse_run(missing_crash, "fixture")

    def test_missing_or_reused_trial_evidence_is_rejected(self) -> None:
        missing = run_record(3, 0, 0)
        del missing["loss_trials"][0]["control_transcript_sha256"]  # type: ignore[index]
        with self.assertRaises(MODULE.FaultEvidenceError):
            MODULE.parse_run(missing, "fixture")

        reused = run_record(3, 0, 0)
        reused["phase_cut_partitions"][1]["control_transcript_sha256"] = reused[  # type: ignore[index]
            "phase_cut_partitions"
        ][0]["control_transcript_sha256"]
        reused["phase_cut_partitions"][1]["observation_capture_sha256"] = reused[  # type: ignore[index]
            "phase_cut_partitions"
        ][0]["observation_capture_sha256"]
        reused["phase_cut_partitions"][1]["control_transcript_record"] = reused[  # type: ignore[index]
            "phase_cut_partitions"
        ][0]["control_transcript_record"]
        reused["phase_cut_partitions"][1]["observation_capture_record"] = reused[  # type: ignore[index]
            "phase_cut_partitions"
        ][0]["observation_capture_record"]
        with self.assertRaises(MODULE.FaultEvidenceError):
            MODULE.parse_run(reused, "fixture")

        reused_across_categories = run_record(3, 0, 0)
        for field in (
            "control_transcript_sha256",
            "control_transcript_record",
            "observation_capture_sha256",
            "observation_capture_record",
        ):
            reused_across_categories["phase_cut_partitions"][0][field] = (  # type: ignore[index]
                reused_across_categories["loss_trials"][0][field]  # type: ignore[index]
            )
        with self.assertRaises(MODULE.FaultEvidenceError):
            MODULE.parse_run(reused_across_categories, "fixture")

    def test_evidence_records_cannot_be_reused_across_runs(self) -> None:
        first = run_record(3, 0, 0)
        second = run_record(3, 1, 1)
        for collection in (
            "loss_trials",
            "phase_cut_partitions",
            "crash_recoveries",
        ):
            for first_trial, second_trial in zip(
                first[collection], second[collection], strict=True  # type: ignore[arg-type]
            ):
                for field in (
                    "control_transcript_sha256",
                    "control_transcript_record",
                    "observation_capture_sha256",
                    "observation_capture_record",
                ):
                    second_trial[field] = first_trial[field]
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "faults.jsonl"
            path.write_text(
                f"{json.dumps(first)}\n{json.dumps(second)}\n", encoding="utf-8"
            )
            with self.assertRaisesRegex(
                MODULE.FaultEvidenceError, "from another run"
            ):
                MODULE.load_runs([path])

    def test_any_partial_visibility_or_replay_gap_is_rejected(self) -> None:
        partial = run_record(3, 0, 0)
        partial["atomicity"]["partial_visible_observations"] = 1  # type: ignore[index]
        with self.assertRaises(MODULE.FaultEvidenceError):
            MODULE.parse_run(partial, "fixture")
        replay = run_record(3, 0, 0)
        replay["atomicity"]["replay_rejected"] = False  # type: ignore[index]
        with self.assertRaises(MODULE.FaultEvidenceError):
            MODULE.parse_run(replay, "fixture")

    def test_runs_from_different_source_commits_are_rejected(self) -> None:
        runs = complete_matrix()
        participants, seed, run, _, hardware, configuration = runs[-1]
        runs[-1] = (
            participants,
            seed,
            run,
            "b" * 40,
            hardware,
            configuration,
        )
        with self.assertRaisesRegex(
            MODULE.FaultEvidenceError, "one exact source commit"
        ):
            MODULE.build_report(runs)

    def test_runs_from_different_hardware_or_n_configuration_are_rejected(self) -> None:
        runs = complete_matrix()
        participants, seed, run, commit, _, configuration = runs[-1]
        runs[-1] = (participants, seed, run, commit, "c" * 64, configuration)
        with self.assertRaisesRegex(
            MODULE.FaultEvidenceError, "one pinned hardware description"
        ):
            MODULE.build_report(runs)

        runs = complete_matrix()
        participants, seed, run, commit, hardware, _ = runs[-1]
        runs[-1] = (participants, seed, run, commit, hardware, "d" * 64)
        with self.assertRaisesRegex(
            MODULE.FaultEvidenceError, "one pinned configuration"
        ):
            MODULE.build_report(runs)


if __name__ == "__main__":
    unittest.main()
