"""Synthetic scheduled runs shared by scaling-validator and receipt tests.

These in-temporary-directory fixtures exercise evidence validation. Their
deterministic transaction identities and observations are synthetic test data,
never measurements or qualification evidence from an executing network.
"""
from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path
from typing import Any


def digest_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def digest_file(path: Path) -> str:
    return digest_bytes(path.read_bytes())


def fixed_workload() -> dict[str, float | int]:
    """Return an independent copy of the fixture's fixed open-loop workload."""
    return {
        "offered_load_tps": 20.0,
        "warmup_seconds": 5.0,
        "measurement_seconds": 20.0,
        "drain_seconds": 2.0,
        "max_submission_lag_ms": 10.0,
        "min_interval_samples": 20,
        "min_latency_samples": 100,
        "max_offered_load_deviation_fraction": 0.01,
    }


class ScalingRunFixture:
    """Write complete synthetic cohorts, observations and bounded drain bins."""

    def __init__(self, root: Path, identity: dict[str, Any], workload: dict[str, Any]) -> None:
        self.root = root
        self.identity = identity
        self.workload = workload

    def write_json(self, path: Path, payload: Any, *, allow_nan: bool = False) -> None:
        path.write_text(
            json.dumps(payload, indent=2, sort_keys=True, allow_nan=allow_nan) + "\n",
            encoding="utf-8",
        )

    def ref(self, path: Path) -> dict[str, str]:
        return {
            "path": path.relative_to(self.root).as_posix(),
            "sha256": digest_file(path),
        }

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
        accepted: int | None = None,
    ) -> dict[str, Any]:
        intervals = 20
        if accepted is None:
            accepted = committed

        def distribute(total: int) -> list[int]:
            quotient, remainder = divmod(total, intervals)
            return [quotient + (1 if index < remainder else 0) for index in range(intervals)]

        offered_parts = distribute(offered)
        accepted_parts = distribute(accepted)
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
                    "accepted_count": accepted_parts[index],
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
                is_accepted = cohort == "warmup" or index % 20 < accepted_parts[index // 20]
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
                        "status": "Accepted" if is_accepted else "Rejected",
                        "rejection": None if is_accepted else "synthetic explicit admission rejection",
                    },
                    "applied": {
                        "offset_ns": offer + round(latency * 1_000_000),
                        "hash": tx_hash,
                        "scope": "global",
                        "resolved_from": "state",
                        "status": "Applied",
                        "block_height": 1 + index // 20,
                    } if is_accepted else None,
                })
        self.write_json(trace_path, {
            "schema": "iroha.sumeragi_v2.multilane_scaling.trace.v1",
            "pair_index": pair_index,
            "variant": variant,
            "seed": seed,
            "clock": "monotonic_nanoseconds_relative_to_measurement_start",
            "logical_id_derivation": "sha256(seed + ':' + cohort + ':' + decimal_sequence)",
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
        return {
            "schema": "iroha.sumeragi_v2.multilane_scaling.run.v1",
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
                "accepted_count": accepted,
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

