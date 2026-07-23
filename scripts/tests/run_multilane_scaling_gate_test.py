"""Python and shell contract tests for the G-SCALE evidence runner."""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
RUNNER = REPO_ROOT / "scripts" / "nexus" / "run_multilane_scaling_gate.py"
SHELL_RUNNER = REPO_ROOT / "scripts" / "nexus" / "run_multilane_scaling_gate.sh"
SHELL_CONTRACT = REPO_ROOT / "scripts" / "tests" / "multilane_scaling_gate_contract.sh"
RUNBOOK = REPO_ROOT / "docs" / "source" / "sumeragi_v2_multilane_scaling_gate.md"

FAKE_TRIAL = r'''#!/usr/bin/env python3
import json
import os
import pathlib
import sys

sequence = int(os.environ["IROHA_GSCALE_SEQUENCE"])
pair_index = int(os.environ["IROHA_GSCALE_PAIR_INDEX"])
variant = os.environ["IROHA_GSCALE_VARIANT"]
active_lanes = int(os.environ["IROHA_GSCALE_ACTIVE_EXECUTION_LANES"])
order_path = os.environ.get("GSCALE_TEST_ORDER_FILE")
if order_path:
    with pathlib.Path(order_path).open("a", encoding="utf-8") as destination:
        destination.write(f"{sequence}:{pair_index}:{variant}:{active_lanes}\n")
if sequence == int(os.environ.get("GSCALE_TEST_FAIL_SEQUENCE", "0")):
    print("injected contract-test failure", file=sys.stderr)
    raise SystemExit(7)
if sequence == int(os.environ.get("GSCALE_TEST_OMIT_SEQUENCE", "0")):
    print("injected missing raw-sample output")
    raise SystemExit(0)

identity_before = json.loads(
    pathlib.Path(os.environ["IROHA_GSCALE_IDENTITY_FILE"]).read_text(encoding="utf-8")
)
identity_after = json.loads(json.dumps(identity_before))
if sequence == int(os.environ.get("GSCALE_TEST_DRIFT_SEQUENCE", "0")):
    identity_after["hardware"]["cpu_model"] = "drifted synthetic CPU"

workload = {
    "offered_load_tps": float(os.environ["IROHA_GSCALE_OFFERED_LOAD_TPS"]),
    "warmup_seconds": float(os.environ["IROHA_GSCALE_WARMUP_SECONDS"]),
    "measurement_seconds": float(os.environ["IROHA_GSCALE_MEASUREMENT_SECONDS"]),
    "min_interval_samples": int(os.environ["IROHA_GSCALE_MIN_INTERVAL_SAMPLES"]),
    "min_latency_samples": int(os.environ["IROHA_GSCALE_MIN_LATENCY_SAMPLES"]),
    "max_offered_load_deviation_fraction": 0.01,
}
intervals = 20
offered = 400
accepted = 400
committed = 100 if variant == "one_lane" else 160
latency = 10.0 if variant == "one_lane" else 12.0

def distribute(total):
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
            "queue_depth": 12,
            "index_entries": 24,
            "memory_bytes": 1019,
            "disk_bytes": 2019,
        }
    )
lane_ids = ["lane-a"] if active_lanes == 1 else ["lane-a", "lane-b", "lane-c", "lane-d"]
run_dir = pathlib.Path(os.environ["IROHA_GSCALE_RUN_DIR"])
support_dir = run_dir / "support"
support_dir.mkdir()
lifecycle = support_dir / "lifecycle.json"
metrics = support_dir / "metrics.prom"
load_log = support_dir / "tx_load.log"
nexus_manifest = support_dir / "load_test_manifest.json"
lifecycle.write_text(
    json.dumps({"active_execution_lanes": lane_ids}, sort_keys=True) + "\n",
    encoding="utf-8",
)
metrics.write_text(f"nexus_lane_configured_total {active_lanes}\n", encoding="utf-8")
load_log.write_text("synthetic tx_load contract output\n", encoding="utf-8")
nexus_manifest.write_text(
    json.dumps(
        {
            "version": 1,
            "lanes": lane_ids,
            "workload_seed": os.environ["IROHA_GSCALE_SEED"],
            "inputs": {
                "status_file": lifecycle.name,
                "metrics_file": metrics.name,
            },
        },
        indent=2,
        sort_keys=True,
    )
    + "\n",
    encoding="utf-8",
)

def artifact_ref(path):
    import hashlib
    return {
        "path": path.relative_to(pathlib.Path(os.environ["IROHA_GSCALE_ARTIFACT_DIR"])).as_posix(),
        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
    }

payload = {
    "schema": os.environ["IROHA_GSCALE_RUN_SCHEMA"],
    "pair_index": pair_index,
    "variant": variant,
    "active_execution_lanes": active_lanes,
    "execution_lane_ids": lane_ids,
    "seed": os.environ["IROHA_GSCALE_SEED"],
    "identity_before": identity_before,
    "identity_after": identity_after,
    "workload": workload,
    "status": {"outcome": "passed", "skipped": False, "failure": None},
    "summary": {
        "offered_count": offered,
        "accepted_count": accepted,
        "committed_count": committed,
        "queue_depth_max": 12,
        "index_entries_max": 24,
        "memory_bytes_max": 1019,
        "disk_bytes_max": 2019,
    },
    "samples": samples,
    "artifacts": {
        "nexus_load_test_manifest": artifact_ref(nexus_manifest),
        "lifecycle_snapshot": artifact_ref(lifecycle),
        "metrics_snapshot": artifact_ref(metrics),
        "load_generator_log": artifact_ref(load_log),
    },
}
output = pathlib.Path(os.environ["IROHA_GSCALE_RAW_SAMPLES_OUT"])
output.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
print(f"wrote {variant} pair {pair_index}")
'''


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


class RunnerFixture:
    def __init__(self, root: Path) -> None:
        self.root = root
        self.config = root / "nexus.toml"
        self.config.write_text("[nexus]\nenabled = true\n", encoding="utf-8")
        self.identity = root / "identity.json"
        identity = {
            "schema": "iroha.sumeragi_v2.multilane_scaling.identity.v1",
            "hardware": {
                "machine_id": "runner-contract-host",
                "cpu_model": "Runner Contract CPU",
                "physical_core_count": 4,
                "logical_core_count": 8,
                "memory_bytes": 16_000_000_000,
                "storage_model": "Runner Contract NVMe",
            },
            "software": {
                "os": "ContractOS",
                "kernel": "contract-kernel",
                "architecture": "arm64",
                "python_version": "3.11.contract",
                "rustc_version": "rustc contract",
                "source_revision": "a" * 40,
                "workspace_source_sha256": "b" * 64,
                "nexus_config_sha256": digest(self.config),
                "irohad_sha256": "c" * 64,
                "iroha_cli_sha256": "d" * 64,
            },
        }
        self.identity.write_text(
            json.dumps(identity, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        self.trial = root / "fake_trial.py"
        self.trial.write_text(FAKE_TRIAL, encoding="utf-8")
        self.trial.chmod(0o755)
        self.bundle = root / "bundle"

    def command(self, *extra: str) -> list[str]:
        return [
            sys.executable,
            str(RUNNER),
            "--artifact-dir",
            str(self.bundle),
            "--identity-file",
            str(self.identity),
            "--config-file",
            str(self.config),
            "--trial-command",
            str(self.trial),
            "--seed-namespace",
            "runner-contract",
            "--offered-load-tps",
            "20",
            "--warmup-seconds",
            "5",
            "--measurement-seconds",
            "20",
            "--max-queue-depth",
            "100",
            "--max-index-entries",
            "200",
            "--max-memory-bytes",
            "10000",
            "--max-disk-bytes",
            "20000",
            "--queue-observation-scope",
            "maximum per-peer transaction queue depth",
            "--index-observation-scope",
            "lane index entries on the designated peer",
            "--memory-observation-scope",
            "aggregate localnet peer RSS",
            "--disk-observation-scope",
            "aggregate localnet lane-storage bytes",
            "--trial-timeout-seconds",
            "10",
            *extra,
        ]


class MultilaneScalingGateRunnerTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.fixture = RunnerFixture(Path(self.temporary.name))

    def run_runner(
        self,
        *,
        env_updates: dict[str, str] | None = None,
        extra: tuple[str, ...] = (),
    ) -> subprocess.CompletedProcess[str]:
        env = os.environ.copy()
        if env_updates:
            env.update(env_updates)
        return subprocess.run(
            self.fixture.command(*extra),
            cwd=REPO_ROOT,
            env=env,
            text=True,
            capture_output=True,
            check=False,
        )

    def test_shell_entrypoint_parses_and_exposes_runner_help(self) -> None:
        syntax = subprocess.run(
            ["bash", "-n", str(SHELL_RUNNER)],
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(syntax.returncode, 0, syntax.stderr)
        help_result = subprocess.run(
            ["bash", str(SHELL_RUNNER), "--help"],
            cwd=REPO_ROOT,
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(help_result.returncode, 0, help_result.stderr)
        self.assertIn("--trial-command", help_result.stdout)
        self.assertIn("--max-memory-bytes", help_result.stdout)

    def test_checked_in_shell_contract(self) -> None:
        result = subprocess.run(
            ["bash", str(SHELL_CONTRACT)],
            cwd=REPO_ROOT,
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("shell surface passed", result.stdout)

    def test_runner_executes_exactly_five_pairs_in_canonical_order(self) -> None:
        order = self.fixture.root / "order.log"
        result = self.run_runner(env_updates={"GSCALE_TEST_ORDER_FILE": str(order)})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            order.read_text(encoding="utf-8").splitlines(),
            [
                f"{sequence}:{pair}:{variant}:{lanes}"
                for sequence, (pair, variant, lanes) in enumerate(
                    (
                        (pair, variant, lanes)
                        for pair in range(1, 6)
                        for variant, lanes in (("one_lane", 1), ("four_lane", 4))
                    ),
                    start=1,
                )
            ],
        )
        manifest = json.loads(
            (self.fixture.bundle / "scaling_evidence.json").read_text(encoding="utf-8")
        )
        self.assertEqual(len(manifest["runs"]), 10)
        self.assertTrue(all(entry["status"] == "passed" for entry in manifest["runs"]))
        for offset in range(0, 10, 2):
            self.assertEqual(manifest["runs"][offset]["seed"], manifest["runs"][offset + 1]["seed"])
        report = json.loads(
            (self.fixture.bundle / "validation_report.json").read_text(encoding="utf-8")
        )
        self.assertEqual(report["result"], "pass")
        self.assertAlmostEqual(
            report["metrics"]["four_to_one_median_throughput_ratio"],
            1.6,
        )
        self.assertAlmostEqual(report["metrics"]["four_to_one_p95_latency_ratio"], 1.2)

    def test_runner_archives_identity_config_harness_validator_and_existing_tools(self) -> None:
        result = self.run_runner()
        self.assertEqual(result.returncode, 0, result.stderr)
        manifest = json.loads(
            (self.fixture.bundle / "scaling_evidence.json").read_text(encoding="utf-8")
        )
        for field in ("identity", "configuration", "trial_harness", "validator"):
            artifact = self.fixture.bundle / manifest[field]["path"]
            self.assertTrue(artifact.is_file(), field)
            self.assertEqual(digest(artifact), manifest[field]["sha256"])
        self.assertEqual(
            [(entry["role"], entry["source_path"]) for entry in manifest["tooling"]],
            [
                ("localnet", "scripts/deploy_localnet.sh"),
                ("load_generator", "scripts/tx_load.py"),
                ("nexus_load_bundle", "scripts/nexus_lane_load_test.py"),
            ],
        )

    def test_runner_fail_fast_records_failed_and_pending_trials(self) -> None:
        result = self.run_runner(env_updates={"GSCALE_TEST_FAIL_SEQUENCE": "3"})
        self.assertEqual(result.returncode, 1)
        manifest = json.loads(
            (self.fixture.bundle / "scaling_evidence.json").read_text(encoding="utf-8")
        )
        self.assertEqual([entry["status"] for entry in manifest["runs"][:3]], [
            "passed",
            "passed",
            "failed",
        ])
        self.assertEqual(manifest["runs"][2]["exit_code"], 7)
        self.assertTrue(all(entry["status"] == "pending" for entry in manifest["runs"][3:]))
        report = json.loads(
            (self.fixture.bundle / "validation_report.json").read_text(encoding="utf-8")
        )
        self.assertEqual(report["result"], "fail")
        self.assertIn("status must be 'passed'", report["errors"][0])

    def test_runner_surfaces_identity_drift_from_raw_observation(self) -> None:
        result = self.run_runner(env_updates={"GSCALE_TEST_DRIFT_SEQUENCE": "4"})
        self.assertEqual(result.returncode, 1)
        report = json.loads(
            (self.fixture.bundle / "validation_report.json").read_text(encoding="utf-8")
        )
        self.assertEqual(report["result"], "fail")
        self.assertIn("identity_after drifted", report["errors"][0])

    def test_runner_rejects_zero_exit_without_raw_sample_file(self) -> None:
        result = self.run_runner(env_updates={"GSCALE_TEST_OMIT_SEQUENCE": "2"})
        self.assertEqual(result.returncode, 1)
        manifest = json.loads(
            (self.fixture.bundle / "scaling_evidence.json").read_text(encoding="utf-8")
        )
        self.assertEqual(manifest["runs"][1]["status"], "failed")
        self.assertEqual(manifest["runs"][1]["exit_code"], 3)
        log_path = self.fixture.bundle / manifest["runs"][1]["command_log"]["path"]
        self.assertIn(
            "without a regular raw_samples.json",
            log_path.read_text(encoding="utf-8"),
        )

    def test_runner_refuses_weak_sample_floor(self) -> None:
        result = self.run_runner(extra=("--min-interval-samples", "19"))
        self.assertEqual(result.returncode, 2)
        self.assertIn("cannot be below 20", result.stderr)
        self.assertFalse(self.fixture.bundle.exists())

    def test_runner_refuses_to_overwrite_existing_artifact_directory(self) -> None:
        self.fixture.bundle.mkdir()
        sentinel = self.fixture.bundle / "keep.txt"
        sentinel.write_text("keep\n", encoding="utf-8")
        result = self.run_runner()
        self.assertEqual(result.returncode, 2)
        self.assertIn("refusing to overwrite", result.stderr)
        self.assertEqual(sentinel.read_text(encoding="utf-8"), "keep\n")

    def test_runner_has_no_cargo_execution_path_or_skip_option(self) -> None:
        runner_text = RUNNER.read_text(encoding="utf-8")
        shell_text = SHELL_RUNNER.read_text(encoding="utf-8")
        self.assertNotIn("cargo test", runner_text)
        self.assertNotIn("cargo run", runner_text)
        self.assertNotIn("--skip", runner_text)
        self.assertNotIn("--continue-on-failure", runner_text)
        self.assertNotIn("cargo", shell_text.lower())

    def test_runbook_records_gate_math_raw_evidence_and_open_handoff(self) -> None:
        text = RUNBOOK.read_text(encoding="utf-8")
        for required in (
            "exactly five pairs",
            "at least `1.5`",
            "no greater than",
            "`1.25`",
            "identity_before",
            "identity_after",
            "scripts/deploy_localnet.sh",
            "scripts/tx_load.py",
            "scripts/nexus_lane_load_test.py",
            "necessary but not by",
            "itself sufficient",
            "does not update the ledger, `status.md`, or `roadmap.md`",
        ):
            self.assertIn(required, text)
        self.assertNotIn('"result": "pass",', text)


if __name__ == "__main__":
    unittest.main()
