"""Focused tests for the fail-closed private-settlement process harness."""

from __future__ import annotations

import copy
import hashlib
import importlib.util
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "private_settlement_real_process_harness.py"
SPEC = importlib.util.spec_from_file_location(
    "private_settlement_real_process_harness", SCRIPT
)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

COMMIT = "a" * 40
HARDWARE = "b" * 64
HARDWARE_PROFILE = "c" * 64
NONCE = "d" * 64


def request(
    participants: int = 3,
    *,
    profile: str = "private",
    warmup: bool = False,
    run: int = 0,
) -> dict[str, Any]:
    """Build one exact canonical benchmark request."""

    seeds = list(range(10))
    configuration = MODULE.runner.build_configuration(
        participants,
        seeds=seeds,
        warmups=MODULE.runner.MIN_WARMUPS,
        measured=MODULE.runner.MIN_MEASURED,
    )
    configuration_bytes = (
        json.dumps(configuration, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    ).encode()
    configuration_sha = hashlib.sha256(configuration_bytes).hexdigest()
    seed = seeds[run % len(seeds)]
    job = {
        "kind": "benchmark",
        "profile": profile,
        "participants": participants,
        "seed": seed,
        "run": run,
        "warmup": warmup,
        "configuration_sha256": configuration_sha,
    }
    return {
        "version": MODULE.runner.VERSION,
        "protocol": MODULE.runner.PROTOCOL,
        "request_id": MODULE.runner.object_digest(job),
        "invocation_nonce": NONCE,
        "kind": "benchmark",
        "commit": COMMIT,
        "hardware_sha256": HARDWARE,
        "hardware_profile_sha256": HARDWARE_PROFILE,
        "configuration_sha256": configuration_sha,
        "participants": participants,
        "validators_per_dataspace": 4,
        "global_validators": 4,
        "quorum": "3-of-4",
        "mandatory_signed_rs16_da_rbc": True,
        "minimum_signed_rs16_da_observations": (participants + 1) * 4,
        "authenticated_message_control": True,
        "seed": seed,
        "run": run,
        "configuration": configuration,
        "payload": {
            "profile": profile,
            "warmup": warmup,
            "stages": list(MODULE.benchmark_stages(profile)),
            "resources": list(MODULE.runner.benchmark_report.RESOURCE_FIELDS),
        },
    }


def fault_request(participants: int = 3, *, run: int = 0) -> dict[str, Any]:
    """Build one exact canonical fault-campaign request."""

    result = request(participants, run=run)
    result["kind"] = "fault"
    result["payload"] = {
        "loss_phases": list(MODULE.runner.fault_report.REQUIRED_LOSS_PHASES),
        "loss_percentages": list(
            MODULE.runner.fault_report.REQUIRED_LOSS_PERCENTAGES
        ),
        "phase_cuts": list(MODULE.runner.fault_report.REQUIRED_PHASE_CUTS),
        "crash_boundaries": list(
            MODULE.runner.fault_report.REQUIRED_CRASH_BOUNDARIES
        ),
        "committee_validator_restarts": list(range(participants)),
        "restart_coordinator": True,
        "restart_global_node": True,
        "maximum_simultaneously_unavailable_per_committee": 1,
        "continuous_atomicity_checks": True,
        "prepare_qc_normalization": {
            "first_signer_subset": [0, 1, 2],
            "second_signer_subset": [0, 1, 3],
            "accept_equivalent_subsets_only_for_identical_body": True,
            "bind_authority_indices": True,
            "bind_every_signed_body": True,
            "reject_changed_certified_body": True,
        },
    }
    result["request_id"] = MODULE.runner.object_digest(
        {
            "kind": "fault",
            "participants": participants,
            "seed": result["seed"],
            "run": run,
            "configuration_sha256": result["configuration_sha256"],
        }
    )
    return result


def leakage_request(variant: str = "left") -> dict[str, Any]:
    """Build one exact commit-bound N=3 differential request."""

    result = request(3)
    manifest = MODULE.runner.build_canary_manifest(COMMIT)
    canaries = MODULE.runner.canaries_for_variant(manifest, variant)
    commitments = {
        entry["name"]: MODULE.runner.object_digest(entry) for entry in canaries
    }
    result["kind"] = "leakage"
    result["payload"] = {
        "variant": variant,
        "canaries": canaries,
        "canary_commitments": commitments,
        "only_secret_fields_change": True,
        "capture_surfaces": [
            {
                "surface": surface,
                "relative_name": MODULE.runner.SURFACE_FILES[surface],
            }
            for surface in sorted(MODULE.runner.SURFACE_FILES)
        ],
        "traffic_count_channels": list(
            MODULE.runner.leakage_audit.REQUIRED_COUNT_CHANNELS
        ),
    }
    result["request_id"] = MODULE.runner.object_digest(
        {
            "kind": "leakage",
            "participants": 3,
            "seed": result["seed"],
            "run": 0,
            "variant": variant,
            "canary_names": [entry["name"] for entry in canaries],
            "canary_commitments": commitments,
            "configuration_sha256": result["configuration_sha256"],
        }
    )
    return result


def inventory(participants: int) -> list[dict[str, Any]]:
    identities: list[tuple[str, int | None, int | None]] = [
        ("coordinator", None, None)
    ]
    identities.extend(("global_validator", None, ordinal) for ordinal in range(4))
    identities.extend(
        ("dataspace_validator", dataspace, ordinal)
        for dataspace in range(participants)
        for ordinal in range(4)
    )
    return [
        {
            "role": role,
            "dataspace_ordinal": dataspace,
            "validator_ordinal": validator,
            "pid": 10_000 + index,
            "executable_sha256": ("e" if role == "coordinator" else "f") * 64,
            "revision": COMMIT,
            "health_observed": True,
        }
        for index, (role, dataspace, validator) in enumerate(identities)
    ]


def rust_result(bound_request: dict[str, Any], request_sha: str) -> dict[str, Any]:
    participants = bound_request["participants"]
    profile = bound_request["payload"]["profile"]
    return {
        "version": 1,
        "protocol": MODULE.runner.PROTOCOL,
        "request_id": bound_request["request_id"],
        "invocation_nonce": bound_request["invocation_nonce"],
        "request_sha256": request_sha,
        "commit": bound_request["commit"],
        "participants": participants,
        "mandatory_signed_rs16_da_rbc": True,
        "signed_rs16_da_observations": (participants + 1) * 4,
        "authenticated_message_control": True,
        "process_inventory": inventory(participants),
        "payload": {
            "stages_ms": {
                stage: 1.0
                for stage in MODULE.benchmark_stages(profile)
            },
            "throughput_bundles_per_second": 1.0,
            "cpu_seconds": 1.0,
            "peak_rss_bytes": 1,
            "network_bytes": 1,
            "proof_bytes": 1 if profile == "private" else 0,
            "receipt_bytes": 1,
            "storage_growth_bytes": 0,
            "finalized_receipt_observed": True,
            "successful_leg_applications": participants,
            "each_leg_applied_exactly_once": True,
            "partial_visible_observations": 0,
            "partial_spendable_observations": 0,
        },
    }


class PrivateSettlementRealProcessHarnessTests(unittest.TestCase):
    """Exercise strict request/result bindings without launching Cargo."""

    def test_exact_argument_contract_rejects_reordering_and_extras(self) -> None:
        parsed = MODULE.parse_exact_arguments(
            [
                "--aps-request",
                "request.json",
                "--aps-response",
                "response.json",
                "--aps-evidence-dir",
                "evidence",
            ]
        )
        self.assertEqual(parsed, (Path("request.json"), Path("response.json"), Path("evidence")))
        with self.assertRaisesRegex(MODULE.HarnessError, "expected exactly"):
            MODULE.parse_exact_arguments(
                [
                    "--aps-response",
                    "response.json",
                    "--aps-request",
                    "request.json",
                    "--aps-evidence-dir",
                    "evidence",
                ]
            )
        with self.assertRaisesRegex(MODULE.HarnessError, "expected exactly"):
            MODULE.parse_exact_arguments(
                [
                    "--aps-request",
                    "request.json",
                    "--aps-response",
                    "response.json",
                    "--aps-evidence-dir",
                    "evidence",
                    "extra",
                ]
            )

    def test_capture_stop_rejects_a_child_that_exited_early(self) -> None:
        class ExitedCapture:
            returncode = 0

            @staticmethod
            def poll() -> int:
                return 0

        stderr = io.BytesIO()
        with self.assertRaisesRegex(MODULE.HarnessError, "exited before"):
            MODULE._stop_tcpdump(ExitedCapture(), stderr, Path("unused.stderr"))
        self.assertTrue(stderr.closed)

    def test_tcpdump_statistics_require_captured_packets_and_zero_drops(self) -> None:
        statistics = MODULE._parse_tcpdump_statistics(
            b"tcpdump: listening on lo0\n"
            b"12 packets captured\n"
            b"24 packets received by filter\n"
            b"0 packets dropped by kernel\n"
            b"0 packets dropped by interface\n"
        )
        self.assertEqual(statistics["captured_packets"], 12)
        self.assertEqual(statistics["received_by_filter_packets"], 24)
        self.assertEqual(statistics["drop_counters"], {"interface": 0, "kernel": 0})
        for malformed, message in (
            (
                b"0 packets captured\n0 packets received by filter\n"
                b"0 packets dropped by kernel\n",
                "zero captured",
            ),
            (
                b"1 packet captured\n1 packet received by filter\n"
                b"1 packet dropped by kernel\n",
                "dropped packets",
            ),
            (
                b"1 packet captured\n1 packet received by filter\n",
                "drop statistic",
            ),
            (
                b"1 packet captured\n1 packet received by filter\n"
                b"0 packets dropped by kernel\ntcpdump: late warning\n",
                "continued after",
            ),
        ):
            with self.subTest(message=message):
                with self.assertRaisesRegex(MODULE.HarnessError, message):
                    MODULE._parse_tcpdump_statistics(malformed)

    def test_capture_stop_parses_the_final_child_statistics(self) -> None:
        class RunningCapture:
            returncode: int | None = None

            def poll(self) -> None:
                return None

            def send_signal(self, sent: int) -> None:
                self.sent = sent

            def wait(self, timeout: float) -> int:
                self.returncode = 0
                return 0

        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "tcpdump.stderr"
            stream = path.open("xb")
            stream.write(
                b"4 packets captured\n8 packets received by filter\n"
                b"0 packets dropped by kernel\n"
            )
            capture = RunningCapture()
            statistics = MODULE._stop_tcpdump(capture, stream, path)
            self.assertEqual(statistics["captured_packets"], 4)
            self.assertEqual(capture.sent, MODULE.signal.SIGINT)
            self.assertTrue(stream.closed)

    def test_complete_raw_capture_is_scanned_before_filtering(self) -> None:
        canaries = [{"name": "memo", "kind": "text", "value": "raw-secret"}]
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "capture.pcap"
            path.write_bytes(b"pcap-header-and-public-packets")
            binding = MODULE._scan_raw_capture(path, canaries)
            self.assertEqual(binding["bytes"], path.stat().st_size)
            path.write_bytes(b"unrelated-loopback-payload-raw-secret-hidden")
            with self.assertRaisesRegex(MODULE.HarnessError, "contains planted canaries"):
                MODULE._scan_raw_capture(path, canaries)

    def test_exact_zero_observations_reject_boolean_aliases(self) -> None:
        self.assertTrue(MODULE._exact_integer(0, 0))
        self.assertFalse(MODULE._exact_integer(False, 0))
        self.assertFalse(MODULE._exact_integer(0.0, 0))

    def test_all_release_participant_shapes_validate_deterministically(self) -> None:
        for profile in MODULE.runner.PROFILES:
            for participants in MODULE.runner.PARTICIPANTS:
                fixture = request(participants, profile=profile)
                self.assertEqual(MODULE.validate_request(copy.deepcopy(fixture)), fixture)
        for participants in MODULE.runner.PARTICIPANTS:
            fixture = fault_request(participants)
            self.assertEqual(MODULE.validate_request(copy.deepcopy(fixture)), fixture)
        for variant in ("left", "right"):
            fixture = leakage_request(variant)
            self.assertEqual(MODULE.validate_request(copy.deepcopy(fixture)), fixture)

    def test_unsupported_kinds_fail_before_execution(self) -> None:
        unknown = request()
        unknown["kind"] = "unknown"
        with self.assertRaisesRegex(MODULE.HarnessError, "benchmark, fault, and leakage"):
            MODULE.validate_request(unknown)

    def test_leakage_request_rejects_canary_or_topology_substitution(self) -> None:
        malformed = leakage_request()
        malformed["payload"]["canaries"][0]["value"] = "substituted"
        with self.assertRaisesRegex(MODULE.HarnessError, "commit-bound canary"):
            MODULE.validate_request(malformed)
        wrong_topology = leakage_request()
        wrong_topology["run"] = 1
        with self.assertRaisesRegex(MODULE.HarnessError, "primary differential"):
            MODULE.validate_request(wrong_topology)

    def test_each_profile_requires_its_exact_stage_inventory(self) -> None:
        private = request()
        private["payload"]["stages"] = ["global_finality", "end_to_end"]
        with self.assertRaisesRegex(MODULE.HarnessError, "canonical profile"):
            MODULE.validate_request(private)
        transparent = request(profile="transparent_control")
        transparent["payload"]["stages"] = list(
            MODULE.runner.benchmark_report.REQUIRED_PRIVATE_STAGES
        )
        with self.assertRaisesRegex(MODULE.HarnessError, "canonical profile"):
            MODULE.validate_request(transparent)

    def test_configuration_job_and_hardware_bindings_reject_substitution(self) -> None:
        malformed = request()
        malformed["hardware_profile_sha256"] = "0" * 64
        with self.assertRaisesRegex(MODULE.HarnessError, "must be non-zero"):
            MODULE.validate_request(malformed)
        configuration = request()
        configuration["configuration"]["participants"] = 4
        with self.assertRaisesRegex(MODULE.HarnessError, "not the canonical"):
            MODULE.validate_request(configuration)
        rayon_width = request()
        rayon_width["configuration"]["execution"]["rayon_worker_threads"] = 1
        with self.assertRaisesRegex(MODULE.HarnessError, "not the canonical"):
            MODULE.validate_request(rayon_width)
        validator_width = request()
        validator_width["configuration"]["execution"]["validator_worker_threads"] = 1
        with self.assertRaisesRegex(MODULE.HarnessError, "not the canonical"):
            MODULE.validate_request(validator_width)
        cargo_jobs = request()
        cargo_jobs["configuration"]["execution"]["cargo_build_jobs"] = 2
        with self.assertRaisesRegex(MODULE.HarnessError, "not the canonical"):
            MODULE.validate_request(cargo_jobs)
        job = request()
        job["request_id"] = "1" * 64
        with self.assertRaisesRegex(MODULE.HarnessError, "does not bind"):
            MODULE.validate_request(job)

    def test_rust_environment_overrides_ambient_width_and_strips_control_inputs(
        self,
    ) -> None:
        bound = request()
        with mock.patch.dict(
            MODULE.os.environ,
            {
                "RAYON_NUM_THREADS": "1",
                "CARGO_BUILD_JOBS": "99",
                "CARGO_INCREMENTAL": "1",
                "CARGO_PROFILE_RELEASE_CODEGEN_UNITS": "99",
                "RUSTFLAGS": "-C target-cpu=native",
                "RUSTC_WRAPPER": "/tmp/substitute-rustc",
                "IROHA_TEST_STALE": "must-not-survive",
                "APS_REAL_PROCESS_STALE": "must-not-survive",
                "APS_UNRELATED": "preserved",
            },
            clear=False,
        ):
            environment = MODULE.rust_harness_environment(bound)
        self.assertEqual(
            environment["RAYON_NUM_THREADS"],
            str(MODULE.runner.RAYON_WORKER_THREADS),
        )
        self.assertEqual(
            environment["CARGO_BUILD_JOBS"],
            str(MODULE.runner.CARGO_BUILD_JOBS),
        )
        self.assertEqual(environment["CARGO_INCREMENTAL"], "0")
        self.assertEqual(
            environment["CARGO_PROFILE_RELEASE_CODEGEN_UNITS"],
            str(MODULE.runner.CARGO_RELEASE_CODEGEN_UNITS),
        )
        self.assertNotIn("RUSTFLAGS", environment)
        self.assertNotIn("RUSTC_WRAPPER", environment)
        self.assertNotIn("IROHA_TEST_STALE", environment)
        self.assertNotIn("APS_REAL_PROCESS_STALE", environment)
        self.assertEqual(environment["APS_UNRELATED"], "preserved")

    def test_rust_result_cannot_be_reused_across_request_or_nonce(self) -> None:
        for profile in MODULE.runner.PROFILES:
            bound = request(profile=profile)
            raw = (json.dumps(bound, sort_keys=True) + "\n").encode()
            request_sha = hashlib.sha256(raw).hexdigest()
            result = rust_result(bound, request_sha)
            MODULE.validate_rust_result(result, request=bound, request_sha=request_sha)
            stale = copy.deepcopy(result)
            stale["request_sha256"] = "1" * 64
            with self.assertRaisesRegex(MODULE.HarnessError, "exact invocation"):
                MODULE.validate_rust_result(stale, request=bound, request_sha=request_sha)
            stale_nonce = copy.deepcopy(result)
            stale_nonce["invocation_nonce"] = "2" * 64
            with self.assertRaisesRegex(MODULE.HarnessError, "exact invocation"):
                MODULE.validate_rust_result(
                    stale_nonce, request=bound, request_sha=request_sha
                )

    def test_transparent_control_accepts_zero_proof_bytes_but_private_does_not(self) -> None:
        transparent = request(profile="transparent_control")
        transparent_result = rust_result(transparent, "3" * 64)
        MODULE.validate_rust_result(
            transparent_result, request=transparent, request_sha="3" * 64
        )
        private = request()
        private_result = rust_result(private, "4" * 64)
        private_result["payload"]["proof_bytes"] = 0
        with self.assertRaisesRegex(MODULE.HarnessError, "proof_bytes"):
            MODULE.validate_rust_result(
                private_result, request=private, request_sha="4" * 64
            )

    def test_rust_result_requires_validator_scaled_da_and_complete_inventory(self) -> None:
        bound = request(8)
        request_sha = "3" * 64
        too_few = rust_result(bound, request_sha)
        too_few["signed_rs16_da_observations"] = 1
        with self.assertRaisesRegex(MODULE.HarnessError, "per-validator"):
            MODULE.validate_rust_result(too_few, request=bound, request_sha=request_sha)
        missing = rust_result(bound, request_sha)
        missing["process_inventory"].pop()
        with self.assertRaisesRegex(MODULE.HarnessError, "topology mismatch"):
            MODULE.validate_rust_result(missing, request=bound, request_sha=request_sha)

    def test_atomic_publication_never_replaces_existing_response(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            response_path = Path(temporary) / "response.json"
            MODULE.publish_response(response_path, {"passed": True})
            original = response_path.read_bytes()
            with self.assertRaisesRegex(MODULE.HarnessError, "appeared"):
                MODULE.publish_response(response_path, {"passed": False})
            self.assertEqual(response_path.read_bytes(), original)

    def test_canonical_paths_pin_a_symlinked_ancestor_before_use(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            real = root / "real"
            real.mkdir()
            (real / "request.json").write_text("{}", encoding="utf-8")
            (real / "evidence").mkdir()
            alias = root / "alias"
            alias.symlink_to(real, target_is_directory=True)
            request_path, response_path, evidence_path = MODULE.canonicalize_paths(
                alias / "request.json",
                alias / "response.json",
                alias / "evidence",
            )
            canonical_real = real.resolve()
            self.assertEqual(request_path, canonical_real / "request.json")
            self.assertEqual(response_path, canonical_real / "response.json")
            self.assertEqual(evidence_path, canonical_real / "evidence")

    def test_canonical_paths_reject_a_symlinked_final_component(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            request = root / "request.json"
            request.write_text("{}", encoding="utf-8")
            request_alias = root / "request-alias.json"
            request_alias.symlink_to(request)
            evidence = root / "evidence"
            evidence.mkdir()
            with self.assertRaisesRegex(MODULE.HarnessError, "symbolic links"):
                MODULE.canonicalize_paths(
                    request_alias,
                    root / "response.json",
                    evidence,
                )

    def test_static_rust_path_uses_real_processes_controller_and_signed_da(self) -> None:
        python_harness = SCRIPT.read_text(encoding="utf-8")
        localnet = (
            ROOT
            / "integration_tests/tests/nexus/atomic_private_settlement_localnet.rs"
        ).read_text(encoding="utf-8")
        harness = (
            ROOT
            / "integration_tests/tests/nexus/atomic_private_settlement_real_process_harness.rs"
        ).read_text(encoding="utf-8")
        private_benchmark = harness[
            harness.index("fn run_real_process_private_benchmark") : harness.index(
                "fn write_real_process_result"
            )
        ]
        self.assertIn("atomic_private_settlement_real_process_benchmark_harness", harness)
        self.assertIn("atomic_private_settlement_real_process_leakage_harness", harness)
        self.assertIn("run_real_process_leakage_campaign", harness)
        self.assertIn("ensure_leakage_sources_redacted", harness)
        self.assertIn("let mut output_rng = rand_core_06::OsRng;", harness)
        self.assertIn("let mut capsule_rng = rand::rngs::OsRng;", harness)
        self.assertIn("prepare_leg_with_private_data_and_rngs", harness)
        self.assertIn('TCPDUMP = Path("/usr/sbin/tcpdump")', python_harness)
        self.assertIn("process.send_signal(signal.SIGINT)", python_harness)
        self.assertIn("capture_split.derive_split_packet_counts", python_harness)
        self.assertIn("def rust_harness_environment(", python_harness)
        self.assertIn('"RAYON_NUM_THREADS": str(', python_harness)
        self.assertIn('"CARGO_BUILD_JOBS": str(', python_harness)
        self.assertIn('"CARGO_INCREMENTAL": "1" if', python_harness)
        self.assertIn('name.startswith("CARGO_PROFILE_")', python_harness)
        self.assertIn("REAL_PROCESS_VALIDATOR_WORKER_THREADS", localnet)
        self.assertIn('["concurrency", "rayon_global_threads"]', localnet)
        self.assertIn("REAL_PROCESS_RAYON_WORKER_THREADS", harness)
        self.assertIn("exact {len(runner.SURFACE_FILES)}-file inventory", python_harness)
        self.assertIn("with_consensus_message_control", harness)
        self.assertIn("wait_until_ready", harness)
        self.assertIn("process_id()", harness)
        self.assertIn("get_bridge_finality_anchor", harness)
        self.assertIn("SumeragiV2GenesisContextParameters::recommended().da_layout", harness)
        self.assertIn("private_settlement_committee_proof_v1", harness)
        self.assertIn("impl Drop for ProcessResourceSampler", harness)
        self.assertIn("impl Drop for TransparentControlAtomicityObserver", harness)
        self.assertIn("classify_transparent_control_values", harness)
        self.assertIn("atomicity_clients.len() == shape.peer_count()", harness)
        self.assertIn("atomicity_observer.begin()", harness)
        self.assertIn("atomicity_observer.finish(3)", harness)
        self.assertIn('"benchmark-before"', private_benchmark)
        self.assertIn('"benchmark-after"', private_benchmark)
        self.assertIn(
            "FaultContinuousObserverV1::start", private_benchmark
        )
        self.assertIn(
            "atomicity_observer.finish(&atomicity_after)", private_benchmark
        )
        self.assertNotIn("TODO:", private_benchmark)
        self.assertNotIn("&deltas,\n        &deltas,", private_benchmark)
        self.assertIn("run_real_process_transparent_control_benchmark", harness)
        self.assertIn("DvpIsi::new", harness)
        self.assertNotIn("Transfer::asset", harness)
        self.assertIn("request.participants - 1", harness)
        self.assertIn("CanExecuteSettlement", localnet)
        self.assertIn("wait_for_identical_native_amx_receipt", harness)
        self.assertIn("proof_bytes: 0", harness)
        self.assertIn(
            'include!("atomic_private_settlement_real_process_harness.rs")', localnet
        )


if __name__ == "__main__":
    unittest.main()
