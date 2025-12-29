"""Unit tests for scripts/check_swift_dashboard_data.py."""

from __future__ import annotations

import importlib.util
from pathlib import Path
from unittest import TestCase

MODULE_PATH = Path(__file__).resolve().parents[1] / "check_swift_dashboard_data.py"
SPEC = importlib.util.spec_from_file_location("check_swift_dashboard_data", MODULE_PATH)
CHECKER = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(CHECKER)  # type: ignore[attr-defined]


def make_parity_payload() -> dict:
    return {
        "generated_at": "2026-03-15T00:00:00Z",
        "fixtures": {
            "outstanding_diffs": 0,
            "oldest_diff_hours": 6,
            "details": [],
        },
        "pipeline": {"last_run": "2026-03-15T00:05:00Z", "status": "ok", "failed_tests": []},
        "regen_sla": {
            "last_success": "2026-03-14T23:00:00Z",
            "hours_since_success": 2,
            "breach": False,
        },
        "alerts": [],
    }


def make_ci_payload() -> dict:
    return {
        "generated_at": "2026-03-15T00:00:00Z",
        "buildkite": {
            "lanes": [
                {
                    "name": "ci/xcode-swift-parity",
                    "success_rate_14_runs": 0.97,
                    "last_failure": None,
                    "flake_count": 1,
                    "mttr_hours": 1.5,
                    "device_tag": "iphone-15",
                }
            ],
            "queue_depth": 0,
            "average_runtime_minutes": 15.0,
        },
        "devices": {
            "emulators": {"passes": 5, "failures": 1},
            "strongbox_capable": {"passes": 2, "failures": 0},
        },
        "alert_state": {"consecutive_failures": 0, "open_incidents": []},
    }


class CheckSwiftDashboardDataTests(TestCase):
    def test_detect_schema_for_parity_and_ci(self) -> None:
        self.assertEqual(CHECKER.detect_schema(make_parity_payload(), "parity.json"), "parity")
        self.assertEqual(CHECKER.detect_schema(make_ci_payload(), "ci.json"), "ci")
        with self.assertRaises(ValueError):
            CHECKER.detect_schema({}, "unknown.json")

    def test_validate_parity_accepts_expected_payload(self) -> None:
        payload = make_parity_payload()
        payload["telemetry"] = {
            "salt_epoch": "2026Q1",
            "salt_rotation_age_hours": 12.5,
            "schema_version": "1",
            "overrides_open": 0,
            "device_profile_alignment": "aligned",
            "notes": ["ok"],
            "connect_metrics_present": True,
        }
        CHECKER.validate_parity(payload, "parity.json")

    def test_validate_parity_rejects_invalid_telemetry_payloads(self) -> None:
        payload = make_parity_payload()
        payload["telemetry"] = {"unknown": "field"}
        with self.assertRaises(ValueError):
            CHECKER.validate_parity(payload, "parity.json")

    def test_validate_parity_rejects_invalid_telemetry_types(self) -> None:
        payload = make_parity_payload()
        payload["telemetry"] = {"salt_epoch": 123}
        with self.assertRaises(ValueError):
            CHECKER.validate_parity(payload, "parity.json")

        payload = make_parity_payload()
        payload["telemetry"] = {"salt_rotation_age_hours": "old"}
        with self.assertRaises(ValueError):
            CHECKER.validate_parity(payload, "parity.json")

        payload = make_parity_payload()
        payload["telemetry"] = {"salt_rotation_age_hours": -1}
        with self.assertRaises(ValueError):
            CHECKER.validate_parity(payload, "parity.json")

        payload = make_parity_payload()
        payload["telemetry"] = {"overrides_open": True}
        with self.assertRaises(ValueError):
            CHECKER.validate_parity(payload, "parity.json")

        payload = make_parity_payload()
        payload["telemetry"] = {"overrides_open": -5}
        with self.assertRaises(ValueError):
            CHECKER.validate_parity(payload, "parity.json")

        payload = make_parity_payload()
        payload["telemetry"] = {"device_profile_alignment": 42}
        with self.assertRaises(ValueError):
            CHECKER.validate_parity(payload, "parity.json")

        payload = make_parity_payload()
        payload["telemetry"] = {"schema_version": 100}
        with self.assertRaises(ValueError):
            CHECKER.validate_parity(payload, "parity.json")

        payload = make_parity_payload()
        payload["telemetry"] = {"notes": ["ok", 42]}
        with self.assertRaises(ValueError):
            CHECKER.validate_parity(payload, "parity.json")

        payload = make_parity_payload()
        payload["telemetry"] = {"connect_metrics_present": "yes"}
        with self.assertRaises(ValueError):
            CHECKER.validate_parity(payload, "parity.json")

    def test_validate_parity_rejects_invalid_acceleration_payloads(self) -> None:
        payload = make_parity_payload()
        payload["acceleration"] = {"metal": {"enabled": "not-bool"}}
        with self.assertRaises(ValueError):
            CHECKER.validate_parity(payload, "parity.json")

        payload["acceleration"] = {"gpu": {"enabled": True}}
        with self.assertRaises(ValueError):
            CHECKER.validate_parity(payload, "parity.json")

    def test_validate_parity_validates_fixture_details(self) -> None:
        payload = make_parity_payload()
        payload["fixtures"]["details"] = [
            {"instruction": "RegisterContract", "age_hours": 4.5, "owner": "sdk-oncall"}
        ]
        CHECKER.validate_parity(payload, "parity.json")

        payload["fixtures"]["details"] = [
            {"instruction": "", "age_hours": 1, "owner": "sdk-oncall"}
        ]
        with self.assertRaises(ValueError):
            CHECKER.validate_parity(payload, "parity.json")

        payload["fixtures"]["details"] = [
            {"instruction": "Register", "age_hours": -1, "owner": "sdk-oncall"}
        ]
        with self.assertRaises(ValueError):
            CHECKER.validate_parity(payload, "parity.json")

        payload["fixtures"]["details"] = [
            {"instruction": "Register", "age_hours": 2, "owner": ""}
        ]
        with self.assertRaises(ValueError):
            CHECKER.validate_parity(payload, "parity.json")

    def test_parse_backend_requirements(self) -> None:
        requirements = CHECKER.parse_backend_requirements(
            ["metal=pass", "neon = pass"], "--parity-accel-require"
        )
        self.assertEqual(requirements, {"metal": "pass", "neon": "pass"})
        with self.assertRaises(ValueError):
            CHECKER.parse_backend_requirements(["missing"], "--parity-accel-require")
        with self.assertRaises(ValueError):
            CHECKER.parse_backend_requirements(["=value"], "--parity-accel-require")
        with self.assertRaises(ValueError):
            CHECKER.parse_backend_requirements(["metal="], "--parity-accel-require")

    def test_enforce_parity_acceleration(self) -> None:
        payload = make_parity_payload()
        payload["acceleration"] = {
            "metal": {"enabled": True, "parity": "pass"},
            "neon": {"enabled": True, "parity": "pass"},
        }
        CHECKER.enforce_parity_acceleration(
            payload,
            "parity.json",
            {"metal": "pass"},
            ["metal", "neon"],
        )

        with self.assertRaises(ValueError):
            CHECKER.enforce_parity_acceleration(
                payload,
                "parity.json",
                {"metal": "fail"},
                [],
            )

        payload["acceleration"]["neon"]["enabled"] = False
        with self.assertRaises(ValueError):
            CHECKER.enforce_parity_acceleration(
                payload,
                "parity.json",
                {},
                ["neon"],
            )

    def test_validate_ci_rejects_non_object_lane(self) -> None:
        payload = make_ci_payload()
        payload["buildkite"]["lanes"] = ["not-a-dict"]
        with self.assertRaises(ValueError):
            CHECKER.validate_ci(payload, "ci.json")

    def test_validate_ci_rejects_invalid_device_groups(self) -> None:
        payload = make_ci_payload()
        payload["devices"]["emulators"] = 42
        with self.assertRaises(ValueError):
            CHECKER.validate_ci(payload, "ci.json")

        payload = make_ci_payload()
        payload["devices"]["strongbox_capable"]["passes"] = -1
        with self.assertRaises(ValueError):
            CHECKER.validate_ci(payload, "ci.json")

    def test_validate_ci_rejects_invalid_acceleration_bench(self) -> None:
        payload = make_ci_payload()
        payload["acceleration_bench"] = {"metal_vs_cpu_merkle_ms": {"cpu": -1, "metal": 1}}
        with self.assertRaises(ValueError):
            CHECKER.validate_ci(payload, "ci.json")

        payload["acceleration_bench"] = {"neon_crc64_throughput_mb_s": -5}
        with self.assertRaises(ValueError):
            CHECKER.validate_ci(payload, "ci.json")

    def test_enforce_ci_acceleration_benchmarks(self) -> None:
        payload = make_ci_payload()
        payload["acceleration_bench"] = {
            "metal_vs_cpu_merkle_ms": {"cpu": 8.0, "metal": 2.0},
            "neon_crc64_throughput_mb_s": 900.0,
        }
        CHECKER.enforce_ci_acceleration_benchmarks(
            payload,
            "ci.json",
            metal_min_speedup=2.0,
            neon_min_throughput=800.0,
        )
        with self.assertRaises(ValueError):
            CHECKER.enforce_ci_acceleration_benchmarks(
                payload,
                "ci.json",
                metal_min_speedup=5.0,
                neon_min_throughput=None,
            )
        with self.assertRaises(ValueError):
            CHECKER.enforce_ci_acceleration_benchmarks(
                payload,
                "ci.json",
                metal_min_speedup=None,
                neon_min_throughput=1000.0,
            )

    def test_enforce_parity_sla_applies_all_thresholds(self) -> None:
        payload = make_parity_payload()
        payload["fixtures"]["outstanding_diffs"] = 2
        with self.assertRaises(ValueError):
            CHECKER.enforce_parity_sla(payload, "parity.json", max_diffs=1, max_oldest_hours=None,
                                       max_regen_hours=None, require_no_breach=False)

        payload = make_parity_payload()
        payload["fixtures"]["oldest_diff_hours"] = 72
        with self.assertRaises(ValueError):
            CHECKER.enforce_parity_sla(payload, "parity.json", max_diffs=None, max_oldest_hours=24,
                                       max_regen_hours=None, require_no_breach=False)

        payload = make_parity_payload()
        payload["regen_sla"]["hours_since_success"] = 30
        with self.assertRaises(ValueError):
            CHECKER.enforce_parity_sla(payload, "parity.json", max_diffs=None, max_oldest_hours=None,
                                       max_regen_hours=24, require_no_breach=False)

        payload = make_parity_payload()
        payload["regen_sla"]["breach"] = True
        with self.assertRaises(ValueError):
            CHECKER.enforce_parity_sla(payload, "parity.json", max_diffs=None, max_oldest_hours=None,
                                       max_regen_hours=None, require_no_breach=True)

        payload = make_parity_payload()
        # within limits
        CHECKER.enforce_parity_sla(payload, "parity.json", max_diffs=2, max_oldest_hours=8,
                                   max_regen_hours=4, require_no_breach=True)

    def test_parse_lane_thresholds_handles_valid_and_invalid_inputs(self) -> None:
        thresholds = CHECKER.parse_lane_thresholds(
            ["ci/xcode-swift-parity>=0.95", "ci/xcode-swift-smoke=0.90"]
        )
        self.assertEqual(
            thresholds,
            [("ci/xcode-swift-parity", 0.95), ("ci/xcode-swift-smoke", 0.90)],
        )
        with self.assertRaises(ValueError):
            CHECKER.parse_lane_thresholds(["missing-operator"])

    def test_enforce_ci_sla_enforces_lane_success_rate(self) -> None:
        payload = make_ci_payload()
        lane_thresholds = [("ci/xcode-swift-parity", 0.99)]
        with self.assertRaises(ValueError):
            CHECKER.enforce_ci_sla(payload, "ci.json", lane_thresholds, None)

    def test_enforce_ci_sla_enforces_consecutive_failures(self) -> None:
        payload = make_ci_payload()
        payload["alert_state"]["consecutive_failures"] = 5
        with self.assertRaises(ValueError):
            CHECKER.enforce_ci_sla(payload, "ci.json", [], 3)

        payload["alert_state"]["consecutive_failures"] = 1
        CHECKER.enforce_ci_sla(payload, "ci.json", [], 3)
