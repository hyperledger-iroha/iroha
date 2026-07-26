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
        self.root = root
        self.manifest_path = root / "scaling_evidence.json"
        self.workload = {
            "offered_load_tps": 20.0,
            "warmup_seconds": 5.0,
            "measurement_seconds": 20.0,
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

    def flush_manifest(self) -> None:
        self.write_json(self.manifest_path, self.manifest)

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
        accepted: int = 400,
    ) -> dict[str, Any]:
        intervals = 20

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
                    "status_file": lifecycle.name,
                    "metrics_file": metrics.name,
                },
            },
        )
        return {
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
                "accepted_count": accepted,
                "committed_count": committed,
                "queue_depth_max": 12,
                "index_entries_max": 24,
                "memory_bytes_max": 1_019,
                "disk_bytes_max": 2_019,
            },
            "samples": samples,
            "artifacts": {
                "nexus_load_test_manifest": self.ref(nexus_manifest),
                "lifecycle_snapshot": self.ref(lifecycle),
                "metrics_snapshot": self.ref(metrics),
                "load_generator_log": self.ref(load_log),
            },
        }

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


class MultilaneScalingEvidenceValidatorTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.bundle = EvidenceBundle(Path(self.temporary.name))

    def assert_invalid(self, fragment: str) -> None:
        with self.assertRaisesRegex(VALIDATOR.EvidenceError, fragment):
            VALIDATOR.validate_evidence(self.bundle.manifest_path)

    def test_valid_bundle_recomputes_both_release_thresholds(self) -> None:
        metrics = VALIDATOR.validate_evidence(self.bundle.manifest_path)
        self.assertEqual(metrics["pair_count"], 5)
        self.assertEqual(metrics["run_count"], 10)
        self.assertAlmostEqual(metrics["four_to_one_median_throughput_ratio"], 1.6)
        self.assertAlmostEqual(metrics["four_to_one_p95_latency_ratio"], 1.2)
        self.assertEqual(metrics["one_lane_resource_maxima"]["queue_depth_max"], 12)
        self.assertEqual(metrics["four_lane_resource_maxima"]["disk_bytes_max"], 2_019)
        self.assertEqual(len(metrics["pairs"]), 5)

    def test_release_binding_accepts_exact_source_workspace_and_validator(self) -> None:
        validator_path = (
            self.bundle.root / self.bundle.manifest["validator"]["path"]
        )
        metrics = VALIDATOR.validate_evidence(
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
                    VALIDATOR.validate_evidence(
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
            "expected_repository_root": retained_root,
        }
        metrics = VALIDATOR.validate_evidence(
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
                    VALIDATOR.validate_evidence(
                        self.bundle.manifest_path,
                        **drifted,
                    )

        first_tool = self.bundle.manifest["tooling"][0]
        retained_tool = retained_root.joinpath(
            *Path(first_tool["source_path"]).parts
        )
        retained_tool.write_bytes(b"retained tool drift\n")
        with self.assertRaisesRegex(
            VALIDATOR.EvidenceError,
            "does not match retained repository tool",
        ):
            VALIDATOR.validate_evidence(
                self.bundle.manifest_path,
                **expectations,
            )

    def test_cli_writes_machine_readable_pass_report(self) -> None:
        report = self.bundle.root / "validation_report.json"
        result = subprocess.run(
            [
                sys.executable,
                str(VALIDATOR_PATH),
                str(self.bundle.manifest_path),
                "--report",
                str(report),
                "--quiet",
            ],
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        payload = json.loads(report.read_text(encoding="utf-8"))
        self.assertEqual(payload["schema"], VALIDATOR.REPORT_SCHEMA)
        self.assertEqual(payload["result"], "pass")
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

    def test_rejects_unmatched_actual_offered_count(self) -> None:
        replacement = self.bundle.raw_run(
            pair_index=2,
            variant="four_lane",
            active_lanes=4,
            seed=self.bundle.entry(2, "four_lane")["seed"],
            committed=160,
            latency=12.0,
            offered=398,
            accepted=398,
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
                    accepted=398,
                ),
            )
        self.assert_invalid("offered count drifted across trials")

    def test_rejects_nonfinite_json_values(self) -> None:
        self.bundle.mutate_raw(
            1,
            "one_lane",
            lambda raw: raw["samples"][0]["commit_latencies_ms"].__setitem__(0, float("nan")),
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
                bundle = EvidenceBundle(Path(temporary.name))
                raw = bundle.load_raw(1, "one_lane")
                raw["samples"][0][sample_field] = bundle.budgets[budget_field] + 1
                raw["summary"][budget_field] = bundle.budgets[budget_field] + 1
                bundle.replace_raw(1, "one_lane", raw)
                with self.assertRaisesRegex(VALIDATOR.EvidenceError, "exceeds .* budget"):
                    VALIDATOR.validate_evidence(bundle.manifest_path)

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
                bundle = EvidenceBundle(Path(temporary.name))
                bundle.manifest["runs"][4][field] = value
                bundle.flush_manifest()
                with self.assertRaisesRegex(VALIDATOR.EvidenceError, fragment):
                    VALIDATOR.validate_evidence(bundle.manifest_path)

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
                bundle = EvidenceBundle(Path(temporary.name))
                bundle.mutate_raw(
                    1,
                    "one_lane",
                    lambda raw, field=field, value=value: raw["summary"].update(
                        {field: value}
                    ),
                )
                with self.assertRaisesRegex(VALIDATOR.EvidenceError, fragment):
                    VALIDATOR.validate_evidence(bundle.manifest_path)

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
                bundle = EvidenceBundle(Path(temporary.name))
                bundle.manifest[section][field] = value
                bundle.flush_manifest()
                with self.assertRaisesRegex(VALIDATOR.EvidenceError, fragment):
                    VALIDATOR.validate_evidence(bundle.manifest_path)

    def test_rejects_tampered_or_out_of_bundle_raw_artifacts(self) -> None:
        raw_path = self.bundle.raw_path(1, "one_lane")
        raw_path.write_text("{}\n", encoding="utf-8")
        self.assert_invalid("sha256 mismatch")

        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        bundle = EvidenceBundle(Path(temporary.name))
        bundle.manifest["runs"][0]["raw_samples"]["path"] = "../outside.json"
        bundle.flush_manifest()
        with self.assertRaisesRegex(VALIDATOR.EvidenceError, "normalized relative"):
            VALIDATOR.validate_evidence(bundle.manifest_path)

    def test_rejects_unexpected_file_and_directory_inventory(self) -> None:
        unexpected = self.bundle.root / "unexpected.txt"
        unexpected.write_text("not referenced\n", encoding="utf-8")
        self.assert_invalid("file inventory.*unexpected")

        unexpected.unlink()
        (self.bundle.root / "empty").mkdir()
        self.assert_invalid("directory inventory.*unexpected")

    def test_rejects_bundle_symlinks(self) -> None:
        link = self.bundle.root / "linked-artifact"
        try:
            link.symlink_to(self.bundle.manifest_path)
        except (NotImplementedError, OSError) as error:
            self.skipTest(f"symlinks unavailable: {error}")
        self.assert_invalid("contains a symlink")

    def test_rejects_bundle_hardlink_aliases(self) -> None:
        alias = self.bundle.root / "manifest-alias"
        try:
            os.link(self.bundle.manifest_path, alias)
        except OSError as error:
            self.skipTest(f"hard links unavailable: {error}")
        self.assert_invalid("hard-link alias")

    def test_rejects_bundle_nonregular_entries(self) -> None:
        if not hasattr(os, "mkfifo"):
            self.skipTest("FIFOs unavailable")
        fifo = self.bundle.root / "unexpected-fifo"
        os.mkfifo(fifo)
        self.assert_invalid("nonregular entry")

    def test_rejects_unsafe_bundle_path_components(self) -> None:
        unsafe = self.bundle.root / "unsafe\nname"
        unsafe.write_text("unsafe\n", encoding="utf-8")
        self.assert_invalid("unsafe path component")

    def test_rejects_oversize_files_before_hashing(self) -> None:
        with mock.patch.object(VALIDATOR, "MAX_BUNDLE_FILE_BYTES", 1):
            self.assert_invalid("file exceeds 1 bytes")

    def test_rejects_excessive_file_count(self) -> None:
        with mock.patch.object(VALIDATOR, "MAX_BUNDLE_FILE_COUNT", 1):
            self.assert_invalid("file-count limit 1")

    def test_rejects_excessive_aggregate_size(self) -> None:
        with mock.patch.object(VALIDATOR, "MAX_BUNDLE_TOTAL_BYTES", 1):
            self.assert_invalid("aggregate size limit 1 bytes")

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

    def test_cli_failure_report_is_machine_readable(self) -> None:
        self.bundle.manifest["runs"][0]["skipped"] = True
        self.bundle.flush_manifest()
        report = self.bundle.root / "validation_report.json"
        result = subprocess.run(
            [
                sys.executable,
                str(VALIDATOR_PATH),
                str(self.bundle.manifest_path),
                "--report",
                str(report),
                "--quiet",
            ],
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(result.returncode, 1)
        payload = json.loads(report.read_text(encoding="utf-8"))
        self.assertEqual(payload["result"], "fail")
        self.assertIsNone(payload["metrics"])
        self.assertIn("skipped must be false", payload["errors"][0])


if __name__ == "__main__":
    unittest.main()
