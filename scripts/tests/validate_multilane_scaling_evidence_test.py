"""Contract tests for the Sumeragi V2 horizontal multilane evidence validator."""

from __future__ import annotations

import copy
import hashlib
import importlib.util
import json
import os
import stat
import subprocess
import sys
import tempfile
import unittest
from unittest import mock
from pathlib import Path
from typing import Any, Callable


REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts/nexus"))
sys.path.insert(0, str(REPO_ROOT / "scripts/tests"))
import scaling_main_component_fixture as COMPONENT
VALIDATOR_PATH = REPO_ROOT / "scripts" / "nexus" / "validate_multilane_scaling_evidence.py"
SPEC = importlib.util.spec_from_file_location("multilane_scaling_validator", VALIDATOR_PATH)
assert SPEC is not None and SPEC.loader is not None
VALIDATOR = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = VALIDATOR
SPEC.loader.exec_module(VALIDATOR)


def digest_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def digest_file(path: Path) -> str:
    return digest_bytes(path.read_bytes())


class EvidenceBundle:
    """Build a synthetic, in-temporary-directory contract fixture."""

    def __init__(self, root: Path) -> None:
        root = root.resolve(strict=True)
        self.root = root
        self.manifest_path = root / "scaling_evidence.json"
        self.workload = {
            "offered_load_tps": 20.0,
            "warmup_seconds": 5.0,
            "measurement_seconds": 20.0,
            "drain_seconds": 2.0,
            "max_submission_lag_ms": 10.0,
            "min_interval_samples": 20,
            "min_latency_samples": 100,
            "max_offered_load_deviation_fraction": 0.01,
        }
        self.budgets = {
            "queue_depth_max": 100,
            "index_entries_max": 200,
            "memory_bytes_max": 10_000,
            "disk_bytes_max": 20_000,
        }
        config = root / "inputs" / "nexus_config.toml"
        config.parent.mkdir(parents=True)
        config.write_text("[nexus]\nenabled = true\n", encoding="utf-8")
        self.identity = {
            "schema": VALIDATOR.IDENTITY_SCHEMA,
            "hardware": {
                "machine_id": "synthetic-contract-host",
                "cpu_model": "Synthetic Contract CPU",
                "physical_core_count": 8,
                "logical_core_count": 16,
                "memory_bytes": 32_000_000_000,
                "storage_model": "Synthetic NVMe",
            },
            "software": {
                "os": "SyntheticOS 1",
                "kernel": "synthetic-kernel-1",
                "architecture": "x86_64",
                "python_version": "3.11.contract",
                "rustc_version": "rustc contract",
                "source_revision": "1" * 40,
                "workspace_source_sha256": "2" * 64,
                "nexus_config_sha256": digest_file(config),
                "irohad_sha256": "3" * 64,
                "iroha_cli_sha256": "4" * 64,
            },
        }
        identity_path = root / "inputs" / "identity.json"
        self.write_json(identity_path, self.identity)

        harness = root / "inputs" / "trial_harness.sh"
        harness.write_text("#!/usr/bin/env bash\nexit 0\n", encoding="utf-8")
        validator = root / "tooling" / "validate_multilane_scaling_evidence.py"
        validator.parent.mkdir(parents=True)
        validator.write_text("# archived validator\n", encoding="utf-8")

        tooling = []
        for role, source_path in VALIDATOR.REQUIRED_TOOLING:
            artifact = root / "tooling" / Path(source_path).name
            artifact.write_text(f"# archived {role}\n", encoding="utf-8")
            tooling.append(
                {
                    "role": role,
                    "source_path": source_path,
                    "artifact": self.ref(artifact),
                }
            )

        runs: list[dict[str, Any]] = []
        sequence = 0
        namespace = "contract-g-scale"
        for pair_index in range(1, 6):
            seed = VALIDATOR.derive_seed(namespace, pair_index)
            for variant, active_lanes, committed, latency in (
                ("one_lane", 1, 100, 10.0),
                ("four_lane", 4, 160, 12.0),
            ):
                sequence += 1
                run_dir = root / "runs" / f"pair_{pair_index:02d}" / variant
                run_dir.mkdir(parents=True)
                raw_path = run_dir / "raw_samples.json"
                log_path = run_dir / "trial.log"
                log_path.write_text("synthetic contract fixture\n", encoding="utf-8")
                self.write_json(
                    raw_path,
                    self.raw_run(
                        pair_index=pair_index,
                        variant=variant,
                        active_lanes=active_lanes,
                        seed=seed,
                        committed=committed,
                        latency=latency,
                    ),
                )
                runs.append(
                    {
                        "sequence": sequence,
                        "pair_index": pair_index,
                        "variant": variant,
                        "active_execution_lanes": active_lanes,
                        "seed": seed,
                        "status": "passed",
                        "skipped": False,
                        "exit_code": 0,
                        "raw_samples": self.ref(raw_path),
                        "command_log": self.ref(log_path),
                    }
                )

        self.manifest = {
            "schema": VALIDATOR.EVIDENCE_SCHEMA,
            "generated_at_utc": "2026-07-23T12:00:00Z",
            "pair_count": 5,
            "seed_namespace": namespace,
            "seed_derivation": VALIDATOR.SEED_DERIVATION,
            "identity": self.ref(identity_path),
            "configuration": self.ref(config),
            "workload": self.workload,
            "budgets": self.budgets,
            "observation_scope": {
                "queue": "maximum transaction queue depth reported by any peer",
                "index": "lane index entries on the designated storage peer",
                "memory": "aggregate RSS of all localnet peer processes",
                "disk": "aggregate bytes under all localnet lane storage roots",
            },
            "thresholds": {
                "min_four_lane_throughput_ratio": 1.5,
                "max_four_lane_p95_latency_ratio": 1.25,
            },
            "trial_harness": self.ref(harness),
            "validator": self.ref(validator),
            "tooling": tooling,
            "runs": runs,
        }
        self.flush_manifest()
        COMPONENT.initialize(self, VALIDATOR)

    def write_json(self, path: Path, payload: Any, *, allow_nan: bool = False) -> None:
        path.write_text(
            json.dumps(payload, indent=2, sort_keys=True, allow_nan=allow_nan) + "\n",
            encoding="utf-8",
        )
        path.chmod(0o600)

    def ref(self, path: Path) -> dict[str, str]:
        digest = digest_file(path)
        COMPONENT.pin(self, path, digest)
        return {
            "path": path.relative_to(self.root).as_posix(),
            "sha256": digest,
        }

    def flush_manifest(self) -> None:
        self.write_json(self.manifest_path, self.manifest)
        COMPONENT.pin(self, self.manifest_path)

    def entry(self, pair_index: int, variant: str) -> dict[str, Any]:
        return next(
            entry
            for entry in self.manifest["runs"]
            if entry["pair_index"] == pair_index and entry["variant"] == variant
        )

    def raw_path(self, pair_index: int, variant: str) -> Path:
        entry = self.entry(pair_index, variant)
        return self.root / entry["raw_samples"]["path"]

    def load_raw(self, pair_index: int, variant: str) -> dict[str, Any]:
        return json.loads(self.raw_path(pair_index, variant).read_text(encoding="utf-8"))

    def replace_raw(
        self,
        pair_index: int,
        variant: str,
        payload: dict[str, Any],
        *,
        allow_nan: bool = False,
        refresh_digest: bool = True,
    ) -> None:
        path = self.raw_path(pair_index, variant)
        self.write_json(path, payload, allow_nan=allow_nan)
        if refresh_digest:
            self.entry(pair_index, variant)["raw_samples"] = self.ref(path)
        self.flush_manifest()

    def mutate_raw(
        self,
        pair_index: int,
        variant: str,
        mutation: Callable[[dict[str, Any]], None],
        *,
        allow_nan: bool = False,
        refresh_digest: bool = True,
    ) -> None:
        payload = self.load_raw(pair_index, variant)
        mutation(payload)
        self.replace_raw(
            pair_index,
            variant,
            payload,
            allow_nan=allow_nan,
            refresh_digest=refresh_digest,
        )

    def raw_run(
        self,
        *,
        pair_index: int,
        variant: str,
        active_lanes: int,
        seed: str,
        committed: int,
        latency: float,
        offered: int = 400,
    ) -> dict[str, Any]:
        intervals = 20
        def distribute(total: int) -> list[int]:
            quotient, remainder = divmod(total, intervals)
            return [quotient + (1 if index < remainder else 0) for index in range(intervals)]

        offered_parts = distribute(offered)
        committed_parts = distribute(committed)
        samples = []
        for index in range(intervals):
            interval_committed = committed_parts[index]
            samples.append(
                {
                    "sequence": index + 1,
                    "start_offset_seconds": float(index),
                    "end_offset_seconds": float(index + 1),
                    "offered_count": offered_parts[index],
                    "accepted_count": offered_parts[index],
                    "committed_count": interval_committed,
                    "commit_latencies_ms": [latency] * interval_committed,
                    "queue_depth": 10 + (index % 3),
                    "index_entries": 20 + (index % 5),
                    "memory_bytes": 1_000 + index,
                    "disk_bytes": 2_000 + index,
                }
            )
        lane_ids = ["lane-a"] if active_lanes == 1 else ["lane-a", "lane-b", "lane-c", "lane-d"]
        support_dir = self.root / "runs" / f"pair_{pair_index:02d}" / variant / "support"
        support_dir.mkdir(parents=True, exist_ok=True)
        lifecycle = support_dir / "lifecycle.json"
        metrics = support_dir / "metrics.prom"
        load_log = support_dir / "tx_load.log"
        nexus_manifest = support_dir / "load_test_manifest.json"
        trace_path = support_dir / "transaction_trace.json"
        self.write_json(lifecycle, {"active_execution_lanes": lane_ids})
        metrics.write_text(
            f"nexus_lane_configured_total {active_lanes}\n",
            encoding="utf-8",
        )
        load_log.write_text("synthetic tx_load contract output\n", encoding="utf-8")
        self.write_json(
            nexus_manifest,
            {
                "version": 1,
                "lanes": lane_ids,
                "workload_seed": seed,
                "inputs": {
                    "lifecycle_file": lifecycle.name,
                    "metrics_file": metrics.name,
                    "telemetry_file": None,
                    "alias_migrations": [],
                },
            },
        )
        transactions = []
        for cohort, count, start in (("warmup", 100, -7_000_000_000), ("measurement", offered, 0)):
            for index in range(count):
                sequence = index + 1
                scheduled = start + index * 50_000_000
                offer = scheduled + 100_000
                tx_hash = digest_bytes(f"{pair_index}:{variant}:{cohort}:{sequence}".encode())[:-1] + "1"
                transactions.append({
                    "cohort": cohort,
                    "sequence": sequence,
                    "logical_id": digest_bytes(f"{seed}:{cohort}:{sequence}".encode()),
                    "hash": tx_hash,
                    "scheduled_offset_ns": scheduled,
                    "offer_offset_ns": offer,
                    "submission_lag_ns": 100_000,
                    "acknowledgment": {
                        "offset_ns": offer + 1_000_000,
                        "hash": tx_hash,
                        "status": "Accepted",
                        "rejection": None,
                    },
                    "applied": {
                        "offset_ns": offer + round(latency * 1_000_000),
                        "hash": tx_hash,
                        "scope": "global",
                        "resolved_from": "state",
                        "status": "Applied",
                        "block_height": 1 + index // 20,
                    },
                })
        self.write_json(trace_path, {
            "schema": VALIDATOR.TRACE_SCHEMA,
            "pair_index": pair_index,
            "variant": variant,
            "seed": seed,
            "clock": "monotonic_nanoseconds_relative_to_measurement_start",
            "logical_id_derivation": VALIDATOR.LOGICAL_ID_DERIVATION,
            "transaction_hash_source": "iroha_data_model::transaction::SignedTransaction::hash",
            "transactions": transactions,
        })
        drain_samples = []
        for index in range(2):
            sample = copy.deepcopy(samples[-1])
            sample.update({
                "sequence": index + 1,
                "start_offset_seconds": 20.0 + index,
                "end_offset_seconds": 21.0 + index,
                "offered_count": 0,
                "accepted_count": 0,
                "committed_count": 0,
                "commit_latencies_ms": [],
            })
            drain_samples.append(sample)
        raw = {
            "schema": VALIDATOR.RUN_SCHEMA,
            "pair_index": pair_index,
            "variant": variant,
            "active_execution_lanes": active_lanes,
            "execution_lane_ids": lane_ids,
            "seed": seed,
            "identity_before": copy.deepcopy(self.identity),
            "identity_after": copy.deepcopy(self.identity),
            "workload": copy.deepcopy(self.workload),
            "status": {"outcome": "passed", "skipped": False, "failure": None},
            "summary": {
                "offered_count": offered,
                "accepted_count": offered,
                "committed_count": committed,
                "queue_depth_max": 12,
                "index_entries_max": 24,
                "memory_bytes_max": 1_019,
                "disk_bytes_max": 2_019,
            },
            "samples": samples,
            "warmup": {"offered_count": 100, "accepted_count": 100, "committed_count": 100},
            "drain": {
                "summary": {
                    "offered_count": 0, "accepted_count": 0, "committed_count": 0,
                    "queue_depth_max": samples[-1]["queue_depth"],
                    "index_entries_max": samples[-1]["index_entries"],
                    "memory_bytes_max": samples[-1]["memory_bytes"],
                    "disk_bytes_max": samples[-1]["disk_bytes"],
                },
                "samples": drain_samples,
            },
            "artifacts": {
                "nexus_load_test_manifest": self.ref(nexus_manifest),
                "lifecycle_snapshot": self.ref(lifecycle),
                "metrics_snapshot": self.ref(metrics),
                "load_generator_log": self.ref(load_log),
                "transaction_trace": self.ref(trace_path),
            },
        }

        trace = json.loads(trace_path.read_text())
        return COMPONENT.complete_outcomes(self, raw, trace, committed, latency)

    def replace_variant_runs(self, variant: str, *, committed: int, latency: float) -> None:
        for pair_index in range(1, 6):
            entry = self.entry(pair_index, variant)
            self.replace_raw(
                pair_index,
                variant,
                self.raw_run(
                    pair_index=pair_index,
                    variant=variant,
                    active_lanes=entry["active_execution_lanes"],
                    seed=entry["seed"],
                    committed=committed,
                    latency=latency,
                ),
            )

            COMPONENT.refresh_journal(self, pair_index, variant)

    def load_trace(self, pair_index: int, variant: str) -> dict[str, Any]:
        raw = self.load_raw(pair_index, variant)
        return json.loads((self.root / raw["artifacts"]["transaction_trace"]["path"]).read_text())

    def replace_trace(
        self, pair_index: int, variant: str, trace: dict[str, Any], *, recount: bool = False,
    ) -> None:
        """Refresh independent artifact hashes; optionally rebuild fixture event bins."""

        raw = self.load_raw(pair_index, variant)
        path = self.root / raw["artifacts"]["transaction_trace"]["path"]
        self.write_json(path, trace)
        raw["artifacts"]["transaction_trace"] = self.ref(path)
        if recount:
            rows = [row for row in trace["transactions"] if row["cohort"] == "measurement"]
            for phase in (raw, raw["drain"]):
                for sample in phase["samples"]:
                    start = round(sample["start_offset_seconds"] * 1_000_000_000)
                    end = round(sample["end_offset_seconds"] * 1_000_000_000)

                    def inside(offset: int) -> bool:
                        return start <= offset < end or (
                            phase is raw["drain"] and sample is phase["samples"][-1] and offset == end
                        )

                    sample["offered_count"] = sum(inside(row["offer_offset_ns"]) for row in rows)
                    sample["accepted_count"] = sum(
                        row["acknowledgment"]["status"] == "Accepted" and inside(row["acknowledgment"]["offset_ns"])
                        for row in rows
                    )
                    applied = sorted(
                        (row["applied"]["offset_ns"], row["sequence"],
                         (row["applied"]["offset_ns"] - row["offer_offset_ns"]) / 1_000_000)
                        for row in rows if row["applied"] is not None and inside(row["applied"]["offset_ns"])
                    )
                    sample["committed_count"] = len(applied)
                    sample["commit_latencies_ms"] = [event[2] for event in applied]
                for name in ("offered_count", "accepted_count", "committed_count"):
                    phase["summary"][name] = sum(sample[name] for sample in phase["samples"])
        self.replace_raw(pair_index, variant, raw)
        if recount:
            COMPONENT.refresh_journal(self, pair_index, variant)


class MultilaneScalingEvidenceValidatorTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.published_owner = tempfile.TemporaryDirectory()
        cls.addClassCleanup(cls.published_owner.cleanup)
        cls.published = EvidenceBundle(Path(cls.published_owner.name))

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.bundle = COMPONENT.clone_fixture(self.published, Path(self.temporary.name))

    def assert_invalid(self, fragment: str) -> None:
        with self.assertRaisesRegex(ValueError, fragment):
            COMPONENT.validate_component(self.bundle.manifest_path)

    def test_rehashed_full_bundle_rejects_journal_submission_lag_disagreement(self) -> None:
        raw = self.bundle.load_raw(1, "one_lane")
        path = self.bundle.root / raw["artifacts"]["collector_journal"]["path"]
        original = path.read_bytes()
        first, remaining = original.split(b"\n", 1)
        plan = json.loads(first)
        self.assertEqual(plan["submission_lag_bound_ns"], 10_000_000)
        trace = self.bundle.load_trace(1, "one_lane")
        self.assertGreater(min(row["submission_lag_ns"] for row in trace["transactions"]), 1)
        for declared in (1, 20_000_000, True):
            with self.subTest(declared=declared):
                # Refresh all independently supplied digests so only semantic
                # agreement with the workload/trace can reject this capture.
                changed = dict(plan, submission_lag_bound_ns=declared)
                path.write_bytes(COMPONENT.encode(changed) + b"\n" + remaining)
                raw["artifacts"]["collector_journal"] = self.bundle.ref(path)
                self.bundle.replace_raw(1, "one_lane", raw)
                self.assert_invalid("collector journal disagrees with the exact trace or workload")

    def test_valid_bundle_recomputes_both_release_thresholds(self) -> None:
        metrics = COMPONENT.validate_component(self.bundle.manifest_path)
        self.assertEqual(metrics["pair_count"], 5)
        self.assertEqual(metrics["run_count"], 10)
        self.assertAlmostEqual(metrics["four_to_one_median_throughput_ratio"], 1.6)
        self.assertAlmostEqual(metrics["four_to_one_p95_latency_ratio"], 1.2)
        self.assertEqual(metrics["one_lane_resource_maxima"]["queue_depth_max"], 12)
        self.assertEqual(metrics["four_lane_resource_maxima"]["disk_bytes_max"], 2_019)
        self.assertEqual(len(metrics["pairs"]), 5)

    def test_complete_trace_preserves_cross_interval_events_and_late_acknowledgments(self) -> None:
        trace = self.bundle.load_trace(1, "one_lane")
        measurement = [row for row in trace["transactions"] if row["cohort"] == "measurement"]
        # State observation can precede the admission response, even across bins.
        measurement[20]["acknowledgment"]["offset_ns"] = 2_001_000_000
        last = measurement[-1]
        last["acknowledgment"].update(status="Accepted", rejection=None, offset_ns=20_200_000_000)
        last["applied"] = {
            "offset_ns": 20_100_000_000, "hash": last["hash"], "scope": "global",
            "resolved_from": "state", "status": "Applied", "block_height": 21,
        }
        self.bundle.replace_trace(1, "one_lane", trace, recount=True)
        raw = self.bundle.load_raw(1, "one_lane")
        self.assertGreater(raw["samples"][1]["committed_count"], raw["samples"][1]["accepted_count"])
        self.assertEqual(raw["drain"]["summary"]["accepted_count"], 1)
        self.assertEqual(raw["summary"]["accepted_count"], 399)
        metrics = COMPONENT.validate_component(self.bundle.manifest_path)
        pair = metrics["pairs"][0]
        self.assertEqual(pair["one_lane_cohort_accepted_count"], 400)
        self.assertEqual(pair["one_lane_latency_samples"], 400)
        self.assertEqual(pair["one_lane_drain_committed_count"], 300)
        self.assertEqual(pair["one_lane_committed_throughput_tps"], 5.0)

    def test_complete_cohort_p95_includes_tail_that_would_fail_only_after_drain(self) -> None:
        for pair_index in range(1, 6):
            trace = self.bundle.load_trace(pair_index, "four_lane")
            tail = [row for row in trace["transactions"] if row["cohort"] == "measurement"][:21]
            for row in tail:
                row["acknowledgment"].update(status="Accepted", rejection=None, offset_ns=20_500_000_000)
                row["applied"] = {
                    "offset_ns": 21_500_000_000, "hash": row["hash"], "scope": "global",
                    "resolved_from": "state", "status": "Applied", "block_height": 21,
                }
            self.bundle.replace_trace(pair_index, "four_lane", trace, recount=True)
        self.assertEqual(self.bundle.load_raw(1, "four_lane")["summary"]["committed_count"], 160)
        self.assert_invalid("four-lane pooled p95 commit latency gate failed")

    def test_final_drain_deadline_is_inclusive_without_changing_measurement_boundary(self) -> None:
        trace = self.bundle.load_trace(1, "one_lane")
        row = trace["transactions"][-1]
        row["acknowledgment"].update(status="Accepted", rejection=None, offset_ns=22_000_000_000)
        row["applied"] = {
            "offset_ns": 22_000_000_000, "hash": row["hash"], "scope": "global",
            "resolved_from": "state", "status": "Applied", "block_height": 22,
        }
        self.bundle.replace_trace(1, "one_lane", trace, recount=True)
        metrics = COMPONENT.validate_component(self.bundle.manifest_path)
        self.assertEqual(metrics["pairs"][0]["one_lane_drain_accepted_count"], 1)
        self.assertEqual(metrics["pairs"][0]["one_lane_committed_count"], 100)

    def test_rehashed_trace_rejects_missing_duplicate_reordered_and_unknown_observations(self) -> None:
        baseline = self.bundle.load_trace(1, "one_lane")
        cases = (
            ("missing request", lambda trace: trace["transactions"].pop(), "missing or extra scheduled requests"),
            ("extra request", lambda trace: trace["transactions"].append(copy.deepcopy(trace["transactions"][-1])), "missing or extra scheduled requests"),
            ("reordered", lambda trace: trace["transactions"].reverse(), "reordered scheduled"),
            ("duplicate hash", lambda trace: trace["transactions"][101].__setitem__("hash", trace["transactions"][100]["hash"]), "duplicates a transaction identity"),
            ("sequence", lambda trace: trace["transactions"][100].__setitem__("sequence", 2), "reordered scheduled"),
            ("boolean sequence", lambda trace: trace["transactions"][100].__setitem__("sequence", True), "reordered scheduled"),
            ("logical provenance", lambda trace: trace["transactions"][100].__setitem__("logical_id", "0" * 64), "pair-seeded request"),
            ("seed provenance", lambda trace: trace.__setitem__("seed", "0" * 64), "declared provenance"),
            ("hash provenance", lambda trace: trace.__setitem__("transaction_hash_source", "aggregate_counter"), "declared provenance"),
            ("clock provenance", lambda trace: trace.__setitem__("clock", "wall_clock"), "declared provenance"),
            ("unknown ack", lambda trace: trace["transactions"][100]["acknowledgment"].__setitem__("status", "Unknown"), "unknown status"),
            ("ack hash", lambda trace: trace["transactions"][100]["acknowledgment"].__setitem__("hash", "1" * 64), "mismatched transaction identity"),
            ("missing accepted outcome", lambda trace: trace["transactions"][100].__setitem__("applied", None), "applied must be an object"),
            ("cached Applied", lambda trace: trace["transactions"][100]["applied"].__setitem__("resolved_from", "cache"), "authoritative global StateApplied"),
            ("lane Applied", lambda trace: trace["transactions"][100]["applied"].__setitem__("scope", "lane"), "authoritative global StateApplied"),
            ("state rejection", lambda trace: trace["transactions"][100]["applied"].__setitem__("status", "Rejected"), "authoritative global StateApplied"),
            ("state hash", lambda trace: trace["transactions"][100]["applied"].__setitem__("hash", "1" * 64), "authoritative global StateApplied"),
            ("zero height", lambda trace: trace["transactions"][100]["applied"].__setitem__("block_height", 0), "block_height must be an integer >= 1"),
            ("unbounded height", lambda trace: trace["transactions"][100]["applied"].__setitem__("block_height", 1 << 64), "authoritative u64 height bound"),
            ("late ack", lambda trace: trace["transactions"][100]["acknowledgment"].__setitem__("offset_ns", 22_000_000_001), "after the drain deadline"),
            ("late Applied", lambda trace: trace["transactions"][100]["applied"].__setitem__("offset_ns", 22_000_000_001), "after the drain deadline"),
            ("ack before offer", lambda trace: trace["transactions"][100]["acknowledgment"].__setitem__("offset_ns", 0), "before offer"),
            ("Applied before offer", lambda trace: trace["transactions"][100]["applied"].__setitem__("offset_ns", 0), "before offer"),
            ("accepted rejection", lambda trace: trace["transactions"][100]["acknowledgment"].__setitem__("rejection", "rejected"), "null rejection"),
            ("missing rejection", lambda trace: (trace["transactions"][-1]["acknowledgment"].update(status="Rejected", rejection=None), trace["transactions"][-1].__setitem__("applied", None)), "single-line string"),
            ("rejected yet Applied", lambda trace: (trace["transactions"][-1]["acknowledgment"].update(status="Rejected", rejection="explicit synthetic rejection"), trace["transactions"][-1].__setitem__("applied", copy.deepcopy(trace["transactions"][100]["applied"]))), "cannot also claim StateApplied"),
        )
        for name, mutation, expected in cases:
            with self.subTest(name=name):
                trace = copy.deepcopy(baseline)
                mutation(trace)
                self.bundle.replace_trace(1, "one_lane", trace)
                self.assert_invalid(expected)

    def test_rehashed_trace_rejects_missed_schedule_rescheduling_and_catchup(self) -> None:
        baseline = self.bundle.load_trace(1, "one_lane")
        cases = (
            ("schedule", {"scheduled_offset_ns": 1}, "reschedules the fixed open-loop offer"),
            ("lag mismatch", {"submission_lag_ns": 0}, "submission-lag bound"),
            ("before slot", {"offer_offset_ns": -1, "submission_lag_ns": -1}, "submission-lag bound"),
            ("late slot", {"offer_offset_ns": 10_000_001, "submission_lag_ns": 10_000_001}, "submission-lag bound"),
            ("boolean clock", {"offer_offset_ns": True}, "integer nanosecond offset"),
            ("unbounded clock", {"offer_offset_ns": 1 << 63}, "integer nanosecond offset"),
        )
        for name, updates, expected in cases:
            with self.subTest(name=name):
                trace = copy.deepcopy(baseline)
                trace["transactions"][100].update(updates)
                self.bundle.replace_trace(1, "one_lane", trace)
                self.assert_invalid(expected)
        trace = copy.deepcopy(baseline)
        trace["transactions"][101].update(offer_offset_ns=100_000, submission_lag_ns=-49_900_000)
        self.bundle.replace_trace(1, "one_lane", trace)
        self.assert_invalid("submission-lag bound")

    def test_transaction_hash_shape_matches_the_existing_sdk_owner(self) -> None:
        baseline = self.bundle.load_trace(1, "one_lane")
        for invalid in ("1" * 63, "1" * 65, "A" * 63 + "1", "1" * 63 + "0", "g" * 63 + "1"):
            with self.subTest(hash=invalid):
                trace = copy.deepcopy(baseline)
                trace["transactions"][100]["hash"] = invalid
                self.bundle.replace_trace(1, "one_lane", trace)
                self.assert_invalid("canonical signed transaction hash")

    def test_warmup_must_be_separate_and_fully_drained(self) -> None:
        baseline = self.bundle.load_trace(1, "one_lane")
        for event in ("acknowledgment", "applied"):
            with self.subTest(event=event):
                trace = copy.deepcopy(baseline)
                trace["transactions"][0][event]["offset_ns"] = 0
                self.bundle.replace_trace(1, "one_lane", trace)
                self.assert_invalid("leaves warmup undrained")
        self.bundle.replace_trace(1, "one_lane", baseline)
        self.bundle.mutate_raw(1, "one_lane", lambda raw: raw["warmup"].__setitem__("committed_count", 99))
        self.assert_invalid("separate fully drained cohort")

    def test_trace_reconciles_counts_and_unclipped_interval_latencies(self) -> None:
        baseline = self.bundle.load_trace(1, "one_lane")
        for event, value, expected in (
            ("acknowledgment", 2_001_000_000, "accepted_count disagrees"),
            ("applied", 2_001_000_000, "committed_count disagrees"),
            ("applied", 1_011_100_000, "complete transaction trace latencies"),
        ):
            with self.subTest(event=event, value=value):
                trace = copy.deepcopy(baseline)
                trace["transactions"][120][event]["offset_ns"] = value
                self.bundle.replace_trace(1, "one_lane", trace)
                self.assert_invalid(expected)

    def test_drain_observations_are_mandatory_bounded_and_budgeted(self) -> None:
        baseline = self.bundle.load_raw(1, "one_lane")
        cases = (
            ("missing drain", lambda raw: raw.pop("drain"), "fields differ from schema"),
            ("no samples", lambda raw: raw["drain"].__setitem__("samples", []), "complete drain observations"),
            ("missing last interval", lambda raw: raw["drain"]["samples"].pop(), "exactly cover the bounded drain"),
            ("gap", lambda raw: raw["drain"]["samples"][1].__setitem__("start_offset_seconds", 21.1), "leaves a drain gap"),
            ("weak cadence", lambda raw: raw["drain"]["samples"][0].__setitem__("end_offset_seconds", 22), "weakens observation cadence"),
            ("reordered", lambda raw: raw["drain"]["samples"].reverse(), "sequence is unordered"),
            ("counter", lambda raw: raw["drain"]["summary"].__setitem__("accepted_count", 1), "inconsistent with raw drain samples"),
        )
        for name, mutation, expected in cases:
            with self.subTest(name=name):
                raw = copy.deepcopy(baseline)
                mutation(raw)
                self.bundle.replace_raw(1, "one_lane", raw)
                self.assert_invalid(expected)
        for name in self.bundle.budgets:
            with self.subTest(resource=name):
                raw = copy.deepcopy(baseline)
                raw["drain"]["samples"][0][name.removesuffix("_max")] = self.bundle.budgets[name] + 1
                self.bundle.replace_raw(1, "one_lane", raw)
                self.assert_invalid("budget during drain")

    def test_open_loop_bounds_fail_closed_before_trace_processing(self) -> None:
        baseline = copy.deepcopy(self.bundle.workload)
        cases = (
            ("drain_seconds", 0, "greater than zero"),
            ("drain_seconds", 301, "at most 300"),
            ("max_submission_lag_ms", 12.500001, "one quarter of an arrival period"),
            ("measurement_seconds", 0.0000000001, "exact bounded integer nanoseconds"),
            ("offered_load_tps", 1e12, "one quarter of an arrival period"),
            ("measurement_seconds", 1_000_000, "transaction trace row bound"),
            ("offered_load_tps", 1 << 4096, "JSON numeric token limit"),
        )
        for field, value, expected in cases:
            with self.subTest(field=field, value=value):
                self.bundle.manifest["workload"] = dict(baseline, **{field: value})
                self.bundle.flush_manifest()
                self.assert_invalid(expected)
        # The bounded decoder now rejects this token before the numeric owner.
        # Retain the original finite-number guard directly as a second negative.
        with self.assertRaisesRegex(VALIDATOR.EvidenceError, "finite number"):
            VALIDATOR._require_number(1 << 4096, "workload.offered_load_tps")
        workload = dict(baseline, offered_load_tps=3, max_submission_lag_ms=0)
        period, warmup, measurement, drain, lag = VALIDATOR._schedule(workload)
        self.assertEqual(period.numerator, 1_000_000_000)
        self.assertEqual(period.denominator, 3)
        self.assertEqual((warmup, measurement, drain, lag), (5_000_000_000, 20_000_000_000, 2_000_000_000, 0))

    def test_zero_warmup_has_no_phantom_requests_and_late_acknowledgments_remain_visible(self) -> None:
        self.bundle.workload["warmup_seconds"] = 0.0
        COMPONENT.refresh_warmup_scope(self.bundle)
        for entry in self.bundle.manifest["runs"]:
            pair, variant = entry["pair_index"], entry["variant"]
            raw = self.bundle.load_raw(pair, variant)
            raw["workload"]["warmup_seconds"] = 0.0
            raw["warmup"] = {"offered_count": 0, "accepted_count": 0, "committed_count": 0}
            self.bundle.replace_raw(pair, variant, raw)
            trace = self.bundle.load_trace(pair, variant)
            trace["transactions"] = [row for row in trace["transactions"] if row["cohort"] == "measurement"]
            trace["transactions"][-1]["acknowledgment"]["offset_ns"] = 21_000_000_000
            self.bundle.replace_trace(pair, variant, trace, recount=True)
        result = COMPONENT.validate_component(self.bundle.manifest_path)
        self.assertEqual(result["pairs"][0]["one_lane_drain_accepted_count"], 1)
        self.assertEqual(result["pairs"][0]["one_lane_cohort_accepted_count"], 400)
        # Retain late-rejection accounting as a negative: the production
        # collector's completed cohort requires every request to be Applied.
        trace = self.bundle.load_trace(1, "one_lane")
        trace["transactions"][-1]["acknowledgment"].update(
            status="Rejected", rejection="explicit synthetic admission rejection"
        )
        trace["transactions"][-1]["applied"] = None
        self.bundle.replace_trace(1, "one_lane", trace)
        raw = self.bundle.load_raw(1, "one_lane")
        COMPONENT.recount(raw, trace)
        self.bundle.replace_raw(1, "one_lane", raw)
        self.assertEqual(raw["drain"]["summary"]["accepted_count"], 0)
        self.assertEqual(raw["summary"]["accepted_count"], 399)
        self.assert_invalid("collector journal disagrees with the exact trace or workload")

    def test_drain_resource_maximum_is_included_in_release_report(self) -> None:
        raw = self.bundle.load_raw(1, "one_lane")
        for sample in raw["drain"]["samples"]:
            sample["queue_depth"] = 90
        raw["drain"]["summary"]["queue_depth_max"] = 90
        self.bundle.replace_raw(1, "one_lane", raw)
        COMPONENT.republish_drain_queue(self.bundle, 1, "one_lane", 90)
        result = COMPONENT.validate_component(self.bundle.manifest_path)
        self.assertEqual(result["one_lane_resource_maxima"]["queue_depth_max"], 90)

    def test_trace_artifact_and_strict_first_release_fields_cannot_be_omitted_or_extended(self) -> None:
        baseline_raw = self.bundle.load_raw(1, "one_lane")
        raw = copy.deepcopy(baseline_raw)
        del raw["artifacts"]["transaction_trace"]
        self.bundle.replace_raw(1, "one_lane", raw)
        self.assert_invalid("artifacts fields differ from schema")
        self.bundle.replace_raw(1, "one_lane", baseline_raw)
        baseline = self.bundle.load_trace(1, "one_lane")
        for field in ("acknowledgment", "applied", "scheduled_offset_ns", "submission_lag_ns"):
            with self.subTest(missing=field):
                trace = copy.deepcopy(baseline)
                del trace["transactions"][100][field]
                self.bundle.replace_trace(1, "one_lane", trace)
                self.assert_invalid("fields differ from schema")
        trace = copy.deepcopy(baseline)
        trace["transactions"][100]["aggregate_estimated_latency"] = 10
        self.bundle.replace_trace(1, "one_lane", trace)
        self.assert_invalid("fields differ from schema")

    def test_trace_identity_cannot_be_reused_between_warmup_measurement_or_runs(self) -> None:
        baseline = self.bundle.load_trace(1, "one_lane")
        duplicate = baseline["transactions"][0]["hash"]
        for pair, variant, index in ((1, "one_lane", 100), (1, "four_lane", 0)):
            with self.subTest(pair=pair, variant=variant):
                trace = self.bundle.load_trace(pair, variant)
                trace["transactions"][index]["hash"] = duplicate
                self.bundle.replace_trace(pair, variant, trace)
                self.assert_invalid("duplicates a transaction identity")
                if variant == "one_lane":
                    self.bundle.replace_trace(1, "one_lane", baseline)

    def test_release_binding_accepts_exact_source_workspace_and_validator(self) -> None:
        validator_path = (
            self.bundle.root / self.bundle.manifest["validator"]["path"]
        )
        metrics = COMPONENT.validate_component(
            self.bundle.manifest_path,
            expected_source_revision=self.bundle.identity["software"]["source_revision"],
            expected_workspace_source_sha256=(
                self.bundle.identity["software"]["workspace_source_sha256"]
            ),
            expected_validator_sha256=digest_file(validator_path),
        )
        self.assertEqual(metrics["pair_count"], 5)

    def test_release_binding_rejects_source_workspace_or_validator_drift(self) -> None:
        expectations = (
            (
                {"expected_source_revision": "f" * 40},
                "source_revision does not match",
            ),
            (
                {"expected_workspace_source_sha256": "f" * 64},
                "workspace_source_sha256 does not match",
            ),
            (
                {"expected_validator_sha256": "f" * 64},
                "validator does not match",
            ),
        )
        for arguments, message in expectations:
            with self.subTest(arguments=arguments):
                with self.assertRaisesRegex(VALIDATOR.EvidenceError, message):
                    COMPONENT.validate_component(
                        self.bundle.manifest_path,
                        **arguments,
                    )

    def test_release_trust_anchors_bind_all_executable_measurement_inputs(self) -> None:
        retained_temporary = tempfile.TemporaryDirectory()
        self.addCleanup(retained_temporary.cleanup)
        retained_root = Path(retained_temporary.name).resolve(strict=True)
        for entry in self.bundle.manifest["tooling"]:
            archived = self.bundle.root / entry["artifact"]["path"]
            retained = retained_root.joinpath(
                *Path(entry["source_path"]).parts
            )
            retained.parent.mkdir(parents=True, exist_ok=True)
            retained.write_bytes(archived.read_bytes())
        harness = self.bundle.root / self.bundle.manifest["trial_harness"]["path"]
        configuration = (
            self.bundle.root / self.bundle.manifest["configuration"]["path"]
        )
        expectations = {
            "expected_trial_harness_sha256": digest_file(harness),
            "expected_configuration_sha256": digest_file(configuration),
            "expected_irohad_sha256": self.bundle.identity["software"][
                "irohad_sha256"
            ],
            "expected_iroha_cli_sha256": self.bundle.identity["software"][
                "iroha_cli_sha256"
            ],
        }
        metrics = COMPONENT.validate_component(
            self.bundle.manifest_path,
            **expectations,
        )
        self.assertEqual(metrics["run_count"], 10)

        for name in (
            "expected_trial_harness_sha256",
            "expected_configuration_sha256",
            "expected_irohad_sha256",
            "expected_iroha_cli_sha256",
        ):
            drifted = dict(expectations)
            drifted[name] = "f" * 64
            with self.subTest(name=name):
                with self.assertRaises(VALIDATOR.EvidenceError):
                    COMPONENT.validate_component(
                        self.bundle.manifest_path,
                        **drifted,
                    )

        with self.assertRaisesRegex(VALIDATOR.EvidenceError, "repository path trust is retired"):
            COMPONENT.validate_component(
                self.bundle.manifest_path, expected_repository_root=retained_root
            )
        first_tool = self.bundle.manifest["tooling"][0]
        retained_tool = retained_root.joinpath(
            *Path(first_tool["source_path"]).parts
        )
        # Independently pinned archived bytes replace retired repository-path trust.
        retained_tool.write_bytes(b"retained tool drift\n")
        archived_tool = self.bundle.root / first_tool["artifact"]["path"]
        archived_bytes = archived_tool.read_bytes()
        archived_tool.write_bytes(b"!" + archived_bytes[1:])
        with self.assertRaisesRegex(
            COMPONENT.bundle.BundleError,
            "control_digest_mismatch",
        ):
            COMPONENT.validate_component(
                self.bundle.manifest_path,
                **expectations,
            )

    def test_component_report_is_machine_readable_and_public_cli_cannot_qualify(self) -> None:
        metrics = COMPONENT.validate_component(self.bundle.manifest_path)
        report = self.bundle.root / "validation_report.json"
        result = subprocess.run([sys.executable, str(VALIDATOR_PATH), str(self.bundle.manifest_path),
            "--report", str(report), "--quiet"], text=True, capture_output=True, check=False)
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertFalse(report.exists())
        # Report-writer component coverage is explicit; it supplies no canonical proof.
        VALIDATOR._write_report(report, {"schema": VALIDATOR.REPORT_SCHEMA, "result": "component_pass",
            "errors": [], "metrics": metrics, "manifest_sha256": digest_file(self.bundle.manifest_path)})
        payload = json.loads(report.read_text(encoding="utf-8"))
        self.assertEqual(payload["schema"], VALIDATOR.REPORT_SCHEMA)
        self.assertEqual(payload["result"], "component_pass")
        self.assertEqual(payload["errors"], [])
        self.assertRegex(payload["manifest_sha256"], r"^[0-9a-f]{64}$")

    def test_report_publication_handles_partial_writes_and_is_deterministic(
        self,
    ) -> None:
        directory = self.bundle.root / "report-partial-write"
        directory.mkdir()
        report_path = directory / "report.json"
        report = {"z": [3, 2, 1], "a": {"result": "pass"}}
        expected = (
            json.dumps(report, indent=2, sort_keys=True) + "\n"
        ).encode("utf-8")
        real_write = os.write

        def partial_write(descriptor: int, data: bytes) -> int:
            return real_write(descriptor, data[:7])

        with mock.patch.object(VALIDATOR.os, "write", side_effect=partial_write):
            VALIDATOR._write_report(report_path, report)

        self.assertEqual(report_path.read_bytes(), expected)
        metadata = report_path.lstat()
        self.assertTrue(stat.S_ISREG(metadata.st_mode))
        self.assertEqual(stat.S_IMODE(metadata.st_mode), 0o600)
        self.assertEqual(metadata.st_nlink, 1)
        self.assertEqual(list(directory.glob(".gscale-report-*")), [])

    def test_report_publication_rejects_preexisting_stage_symlink(self) -> None:
        directory = self.bundle.root / "report-stage-symlink"
        directory.mkdir()
        report_path = directory / "report.json"
        victim = directory / "victim"
        victim.write_bytes(b"do not overwrite\n")
        token = "a" * 32
        stage = directory / f".gscale-report-{token}"
        try:
            stage.symlink_to(victim)
        except (NotImplementedError, OSError) as error:
            self.skipTest(f"symlinks unavailable: {error}")

        with mock.patch.object(VALIDATOR.secrets, "token_hex", return_value=token):
            with self.assertRaises(FileExistsError):
                VALIDATOR._write_report(report_path, {"result": "pass"})

        self.assertEqual(victim.read_bytes(), b"do not overwrite\n")
        self.assertTrue(stage.is_symlink())
        self.assertFalse(report_path.exists())

    def test_report_publication_never_replaces_destination_or_racer(self) -> None:
        existing_directory = self.bundle.root / "report-existing"
        existing_directory.mkdir()
        existing = existing_directory / "report.json"
        existing.write_bytes(b"existing\n")
        with self.assertRaises(FileExistsError):
            VALIDATOR._write_report(existing, {"result": "pass"})
        self.assertEqual(existing.read_bytes(), b"existing\n")

        symlink_directory = self.bundle.root / "report-destination-symlink"
        symlink_directory.mkdir()
        victim = symlink_directory / "victim"
        victim.write_bytes(b"victim\n")
        destination_symlink = symlink_directory / "report.json"
        try:
            destination_symlink.symlink_to(victim)
        except (NotImplementedError, OSError) as error:
            self.skipTest(f"symlinks unavailable: {error}")
        with self.assertRaises(FileExistsError):
            VALIDATOR._write_report(destination_symlink, {"result": "pass"})
        self.assertEqual(victim.read_bytes(), b"victim\n")
        self.assertTrue(destination_symlink.is_symlink())

        race_directory = self.bundle.root / "report-race"
        race_directory.mkdir()
        raced_destination = race_directory / "report.json"
        token = "b" * 32
        stage = race_directory / f".gscale-report-{token}"
        real_link = os.link

        def race_destination(*args: Any, **kwargs: Any) -> None:
            raced_destination.write_bytes(b"racer\n")
            real_link(*args, **kwargs)

        with (
            mock.patch.object(VALIDATOR.secrets, "token_hex", return_value=token),
            mock.patch.object(VALIDATOR.os, "link", side_effect=race_destination),
        ):
            with self.assertRaises(FileExistsError):
                VALIDATOR._write_report(raced_destination, {"result": "pass"})
        self.assertEqual(raced_destination.read_bytes(), b"racer\n")
        self.assertFalse(stage.exists())

    def test_report_publication_cleans_stage_after_write_or_file_fsync_failure(
        self,
    ) -> None:
        short_directory = self.bundle.root / "report-zero-write"
        short_directory.mkdir()
        short_report = short_directory / "report.json"
        with mock.patch.object(VALIDATOR.os, "write", return_value=0):
            with self.assertRaisesRegex(OSError, "short write"):
                VALIDATOR._write_report(short_report, {"result": "pass"})
        self.assertEqual(list(short_directory.iterdir()), [])

        fsync_directory = self.bundle.root / "report-file-fsync"
        fsync_directory.mkdir()
        fsync_report = fsync_directory / "report.json"
        with mock.patch.object(
            VALIDATOR.os,
            "fsync",
            side_effect=OSError("injected file fsync failure"),
        ):
            with self.assertRaisesRegex(OSError, "injected file fsync failure"):
                VALIDATOR._write_report(fsync_report, {"result": "pass"})
        self.assertEqual(list(fsync_directory.iterdir()), [])

    def test_report_publication_cleans_owned_paths_after_publish_failure(
        self,
    ) -> None:
        link_directory = self.bundle.root / "report-link-failure"
        link_directory.mkdir()
        link_report = link_directory / "report.json"
        with mock.patch.object(
            VALIDATOR.os,
            "link",
            side_effect=OSError("injected link failure"),
        ):
            with self.assertRaisesRegex(OSError, "injected link failure"):
                VALIDATOR._write_report(link_report, {"result": "pass"})
        self.assertEqual(list(link_directory.iterdir()), [])

        directory_fsync = self.bundle.root / "report-directory-fsync"
        directory_fsync.mkdir()
        fsync_report = directory_fsync / "report.json"
        real_fsync = os.fsync
        fsync_calls = 0

        def fail_first_directory_fsync(descriptor: int) -> None:
            nonlocal fsync_calls
            fsync_calls += 1
            if fsync_calls == 2:
                raise OSError("injected directory fsync failure")
            real_fsync(descriptor)

        with mock.patch.object(
            VALIDATOR.os,
            "fsync",
            side_effect=fail_first_directory_fsync,
        ):
            with self.assertRaisesRegex(OSError, "injected directory fsync failure"):
                VALIDATOR._write_report(fsync_report, {"result": "pass"})
        self.assertEqual(list(directory_fsync.iterdir()), [])

    def test_requires_exactly_five_complete_pairs(self) -> None:
        self.bundle.manifest["runs"].pop()
        self.bundle.flush_manifest()
        self.assert_invalid("exactly ten entries")

    def test_rejects_duplicate_or_unordered_pair_entries(self) -> None:
        self.bundle.manifest["runs"][1] = copy.deepcopy(self.bundle.manifest["runs"][0])
        self.bundle.manifest["runs"][1]["sequence"] = 2
        self.bundle.flush_manifest()
        self.assert_invalid("missing, duplicate, or unordered pairs")

    def test_rejects_pair_order_swap(self) -> None:
        self.bundle.manifest["runs"][0], self.bundle.manifest["runs"][1] = (
            self.bundle.manifest["runs"][1],
            self.bundle.manifest["runs"][0],
        )
        self.bundle.flush_manifest()
        self.assert_invalid("missing, duplicate, or unordered pairs")

    def test_rejects_nondeterministic_or_unpaired_seed(self) -> None:
        self.bundle.manifest["runs"][1]["seed"] = "f" * 64
        self.bundle.flush_manifest()
        self.assert_invalid("deterministic pair derivation")

    def test_rejects_identity_drift(self) -> None:
        self.bundle.mutate_raw(
            3,
            "four_lane",
            lambda raw: raw["identity_after"]["hardware"].update(
                {"cpu_model": "drifted CPU"}
            ),
        )
        self.assert_invalid("identity_after drifted")

    def test_rejects_nexus_lane_load_manifest_drift(self) -> None:
        raw = self.bundle.load_raw(3, "four_lane")
        artifact_path = (
            self.bundle.root / raw["artifacts"]["nexus_load_test_manifest"]["path"]
        )
        nexus_manifest = json.loads(artifact_path.read_text(encoding="utf-8"))
        nexus_manifest["lanes"] = ["lane-a"]
        self.bundle.write_json(artifact_path, nexus_manifest)
        raw["artifacts"]["nexus_load_test_manifest"] = self.bundle.ref(artifact_path)
        self.bundle.replace_raw(3, "four_lane", raw)
        self.assert_invalid("manifest lanes do not match active execution lanes")

    def test_rejects_retired_nexus_status_file_input(self) -> None:
        raw = self.bundle.load_raw(3, "four_lane")
        artifact_path = (
            self.bundle.root / raw["artifacts"]["nexus_load_test_manifest"]["path"]
        )
        nexus_manifest = json.loads(artifact_path.read_text(encoding="utf-8"))
        inputs = nexus_manifest["inputs"]
        inputs["status_file"] = inputs.pop("lifecycle_file")
        self.bundle.write_json(artifact_path, nexus_manifest)
        raw["artifacts"]["nexus_load_test_manifest"] = self.bundle.ref(artifact_path)
        self.bundle.replace_raw(3, "four_lane", raw)
        self.assert_invalid("manifest inputs fields differ from schema")

    def test_rejects_unmatched_actual_offered_count(self) -> None:
        replacement = self.bundle.raw_run(
            pair_index=2,
            variant="four_lane",
            active_lanes=4,
            seed=self.bundle.entry(2, "four_lane")["seed"],
            committed=160,
            latency=12.0,
            offered=398,
        )
        self.bundle.replace_raw(2, "four_lane", replacement)
        self.assert_invalid("offered load is not matched")

    def test_rejects_offered_count_drift_between_pairs(self) -> None:
        for variant, active_lanes, committed, latency in (
            ("one_lane", 1, 100, 10.0),
            ("four_lane", 4, 160, 12.0),
        ):
            self.bundle.replace_raw(
                2,
                variant,
                self.bundle.raw_run(
                    pair_index=2,
                    variant=variant,
                    active_lanes=active_lanes,
                    seed=self.bundle.entry(2, variant)["seed"],
                    committed=committed,
                    latency=latency,
                    offered=398,
                ),
            )
        self.assert_invalid("offered count drifted across trials")

    def test_rejects_nonfinite_json_values(self) -> None:
        self.bundle.mutate_raw(
            1,
            "one_lane",
            lambda raw: next(sample["commit_latencies_ms"] for sample in raw["samples"] if sample["commit_latencies_ms"]).__setitem__(0, float("nan")),
            allow_nan=True,
        )
        self.assert_invalid("nonfinite JSON numeric literal")

    def test_rejects_each_resource_budget_violation(self) -> None:
        cases = (
            ("queue_depth", "queue_depth_max"),
            ("index_entries", "index_entries_max"),
            ("memory_bytes", "memory_bytes_max"),
            ("disk_bytes", "disk_bytes_max"),
        )
        for sample_field, budget_field in cases:
            with self.subTest(sample_field=sample_field):
                temporary = tempfile.TemporaryDirectory()
                self.addCleanup(temporary.cleanup)
                bundle = COMPONENT.clone_fixture(self.published, Path(temporary.name))
                raw = bundle.load_raw(1, "one_lane")
                raw["samples"][0][sample_field] = bundle.budgets[budget_field] + 1
                raw["summary"][budget_field] = bundle.budgets[budget_field] + 1
                bundle.replace_raw(1, "one_lane", raw)
                with self.assertRaisesRegex(VALIDATOR.EvidenceError, "exceeds .* budget"):
                    COMPONENT.validate_component(bundle.manifest_path)

    def test_rejects_skipped_and_failed_runs(self) -> None:
        cases = (
            ("skipped", True, "skipped must be false"),
            ("status", "failed", "status must be 'passed'"),
            ("exit_code", 7, "exit_code must be integer zero"),
        )
        for field, value, fragment in cases:
            with self.subTest(field=field):
                temporary = tempfile.TemporaryDirectory()
                self.addCleanup(temporary.cleanup)
                bundle = COMPONENT.clone_fixture(self.published, Path(temporary.name))
                bundle.manifest["runs"][4][field] = value
                bundle.flush_manifest()
                with self.assertRaisesRegex(VALIDATOR.EvidenceError, fragment):
                    COMPONENT.validate_component(bundle.manifest_path)

    def test_rejects_weak_interval_sample_count(self) -> None:
        self.bundle.mutate_raw(
            1,
            "one_lane",
            lambda raw: raw["samples"].pop(),
        )
        self.assert_invalid("weak interval sample count")

    def test_rejects_weak_latency_sample_count(self) -> None:
        entry = self.bundle.entry(1, "one_lane")
        self.bundle.replace_raw(
            1,
            "one_lane",
            self.bundle.raw_run(
                pair_index=1,
                variant="one_lane",
                active_lanes=1,
                seed=entry["seed"],
                committed=99,
                latency=10.0,
            ),
        )
        self.assert_invalid("weak latency sample count")

    def test_rejects_unordered_or_gapped_raw_samples(self) -> None:
        self.bundle.mutate_raw(
            1,
            "one_lane",
            lambda raw: raw["samples"][2].update({"start_offset_seconds": 2.5}),
        )
        self.assert_invalid("unordered or leaves a measurement interval gap")

    def test_rejects_inconsistent_counters_and_maxima(self) -> None:
        cases = (
            ("offered_count", 401, "inconsistent with raw samples"),
            ("queue_depth_max", 11, "inconsistent with raw samples"),
        )
        for field, value, fragment in cases:
            with self.subTest(field=field):
                temporary = tempfile.TemporaryDirectory()
                self.addCleanup(temporary.cleanup)
                bundle = COMPONENT.clone_fixture(self.published, Path(temporary.name))
                bundle.mutate_raw(
                    1,
                    "one_lane",
                    lambda raw, field=field, value=value: raw["summary"].update(
                        {field: value}
                    ),
                )
                with self.assertRaisesRegex(VALIDATOR.EvidenceError, fragment):
                    COMPONENT.validate_component(bundle.manifest_path)

    def test_rejects_wrong_or_duplicate_active_execution_lanes(self) -> None:
        self.bundle.mutate_raw(
            1,
            "four_lane",
            lambda raw: raw.update(
                {"execution_lane_ids": ["lane-a", "lane-b", "lane-c", "lane-c"]}
            ),
        )
        self.assert_invalid("duplicate lane")

    def test_rejects_active_lane_identity_drift_across_pairs(self) -> None:
        raw = self.bundle.load_raw(5, "four_lane")
        drifted_lanes = ["lane-a", "lane-b", "lane-c", "lane-e"]
        raw["execution_lane_ids"] = drifted_lanes
        artifact_path = (
            self.bundle.root / raw["artifacts"]["nexus_load_test_manifest"]["path"]
        )
        nexus_manifest = json.loads(artifact_path.read_text(encoding="utf-8"))
        nexus_manifest["lanes"] = drifted_lanes
        self.bundle.write_json(artifact_path, nexus_manifest)
        raw["artifacts"]["nexus_load_test_manifest"] = self.bundle.ref(artifact_path)
        self.bundle.replace_raw(5, "four_lane", raw)
        self.assert_invalid("identity drifted across trials")

    def test_enforces_median_committed_throughput_ratio(self) -> None:
        self.bundle.replace_variant_runs("four_lane", committed=140, latency=12.0)
        self.assert_invalid("median committed throughput gate failed")

    def test_enforces_pooled_p95_commit_latency_ratio(self) -> None:
        self.bundle.replace_variant_runs("four_lane", committed=160, latency=13.0)
        rows = self.bundle.load_trace(1, "four_lane")["transactions"]
        latencies = [(row["applied"]["offset_ns"] - row["offer_offset_ns"]) / 1_000_000
                     for row in rows if row["cohort"] == "measurement"]
        self.assertEqual(VALIDATOR._nearest_rank_p95(latencies), 20_800.0)
        self.assert_invalid("pooled p95 commit latency gate failed")

    def test_thresholds_and_sample_floors_cannot_be_weakened(self) -> None:
        cases = (
            ("thresholds", "min_four_lane_throughput_ratio", 1.49, "cannot weaken"),
            ("thresholds", "max_four_lane_p95_latency_ratio", 1.26, "cannot weaken"),
            ("workload", "min_interval_samples", 19, "integer >= 20"),
            ("workload", "min_latency_samples", 99, "integer >= 100"),
        )
        for section, field, value, fragment in cases:
            with self.subTest(field=field):
                temporary = tempfile.TemporaryDirectory()
                self.addCleanup(temporary.cleanup)
                bundle = COMPONENT.clone_fixture(self.published, Path(temporary.name))
                bundle.manifest[section][field] = value
                bundle.flush_manifest()
                with self.assertRaisesRegex(VALIDATOR.EvidenceError, fragment):
                    COMPONENT.validate_component(bundle.manifest_path)

    def test_rejects_tampered_or_out_of_bundle_raw_artifacts(self) -> None:
        raw_path = self.bundle.raw_path(1, "one_lane")
        raw_path.write_text("{}\n", encoding="utf-8")
        self.assert_invalid("control_digest_mismatch")

        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        bundle = COMPONENT.clone_fixture(self.published, Path(temporary.name))
        bundle.manifest["runs"][0]["raw_samples"]["path"] = "../outside.json"
        bundle.flush_manifest()
        with self.assertRaisesRegex(VALIDATOR.EvidenceError, "normalized relative"):
            COMPONENT.validate_component(bundle.manifest_path)

    def test_rejects_unexpected_file_and_directory_inventory(self) -> None:
        unexpected = self.bundle.root / "unexpected.txt"
        unexpected.write_text("not referenced\n", encoding="utf-8")
        self.assert_invalid("directory_member_count_exceeded")

        unexpected.unlink()
        (self.bundle.root / "empty").mkdir()
        self.assert_invalid("directory_member_count_exceeded")

    def test_rejects_bundle_symlinks(self) -> None:
        link = self.bundle.root / "linked-artifact"
        try:
            link.symlink_to(self.bundle.manifest_path)
        except (NotImplementedError, OSError) as error:
            self.skipTest(f"symlinks unavailable: {error}")
        self.assert_invalid("directory_member_count_exceeded")
        # Preserve the new-entry control, then reach the actual no-follow file
        # type guard without an earlier exact-directory-membership rejection.
        link.unlink()
        raw = self.bundle.raw_path(1, "one_lane")
        raw.unlink()
        raw.symlink_to(self.bundle.manifest_path)
        self.assert_invalid("regular_single_link_file_required")

    def test_rejects_bundle_hardlink_aliases(self) -> None:
        alias = self.bundle.root / "manifest-alias"
        try:
            os.link(self.bundle.manifest_path, alias)
        except OSError as error:
            self.skipTest(f"hard links unavailable: {error}")
        self.assert_invalid("directory_member_count_exceeded")
        alias.unlink()
        raw = self.bundle.raw_path(1, "one_lane")
        raw.unlink()
        os.link(self.bundle.manifest_path, raw)
        self.assert_invalid("regular_single_link_file_required")

    def test_rejects_bundle_nonregular_entries(self) -> None:
        if not hasattr(os, "mkfifo"):
            self.skipTest("FIFOs unavailable")
        fifo = self.bundle.root / "unexpected-fifo"
        os.mkfifo(fifo)
        self.assert_invalid("directory_member_count_exceeded")
        fifo.unlink()
        raw = self.bundle.raw_path(1, "one_lane")
        raw.unlink()
        os.mkfifo(raw, 0o600)
        self.assert_invalid("regular_single_link_file_required")

    def test_rejects_unsafe_bundle_path_components(self) -> None:
        unsafe = self.bundle.root / "unsafe\nname"
        unsafe.write_text("unsafe\n", encoding="utf-8")
        self.assert_invalid("entry_name_invalid")

    def test_rejects_oversize_files_before_hashing(self) -> None:
        # Admission now owns the physical cap. Prove rejection precedes any
        # raw read/hash, rather than changing a retired scanner constant.
        with mock.patch.object(COMPONENT.bundle, "MAX_FILE_BYTES", 1), mock.patch.object(
            COMPONENT.bundle.os, "pread", side_effect=AssertionError("oversize file was read")
        ):
            self.assert_invalid("file_allocation_exceeded")

    def test_rejects_excessive_file_count(self) -> None:
        with mock.patch.object(COMPONENT.bundle, "MAX_CONTROL_FILES", 1):
            self.assert_invalid("controls_invalid")

    def test_rejects_excessive_aggregate_size(self) -> None:
        with mock.patch.object(COMPONENT.bundle, "MAX_TOTAL_BYTES", 1):
            self.assert_invalid("total_allocation_exceeded")

    def test_rejects_duplicate_json_object_keys(self) -> None:
        path = self.bundle.raw_path(1, "one_lane")
        text = path.read_text(encoding="utf-8")
        text = text.replace(
            '"schema": "iroha.sumeragi_v2.multilane_scaling.run.v1",',
            '"schema": "iroha.sumeragi_v2.multilane_scaling.run.v1",\n'
            '  "schema": "iroha.sumeragi_v2.multilane_scaling.run.v1",',
            1,
        )
        path.write_text(text, encoding="utf-8")
        self.bundle.entry(1, "one_lane")["raw_samples"] = self.bundle.ref(path)
        self.bundle.flush_manifest()
        self.assert_invalid("duplicate JSON object key")

    def test_component_failure_report_is_machine_readable_and_cli_stays_closed(self) -> None:
        self.bundle.manifest["runs"][0]["skipped"] = True
        self.bundle.flush_manifest()
        with self.assertRaisesRegex(VALIDATOR.EvidenceError, "skipped must be false") as failed:
            COMPONENT.validate_component(self.bundle.manifest_path)
        report = self.bundle.root / "validation_report.json"
        result = subprocess.run([sys.executable, str(VALIDATOR_PATH), str(self.bundle.manifest_path),
            "--report", str(report), "--quiet"], text=True, capture_output=True, check=False)
        self.assertEqual(result.returncode, 2)
        self.assertFalse(report.exists())
        VALIDATOR._write_report(report, {"schema": VALIDATOR.REPORT_SCHEMA,
            "result": "component_fail", "metrics": None, "errors": [str(failed.exception)]})
        payload = json.loads(report.read_text(encoding="utf-8"))
        self.assertEqual(payload["result"], "component_fail")
        self.assertIsNone(payload["metrics"])
        self.assertIn("skipped must be false", payload["errors"][0])


if __name__ == "__main__":
    unittest.main()
