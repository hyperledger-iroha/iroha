"""Unit tests for scripts/swift_smoke_anomalies.py."""

from __future__ import annotations

import importlib.util
import sys
import json
import tempfile
from pathlib import Path
from typing import Any
from unittest import TestCase

MODULE_PATH = Path(__file__).resolve().parents[1] / "swift_smoke_anomalies.py"
SPEC = importlib.util.spec_from_file_location("swift_smoke_anomalies", MODULE_PATH)
ANOMALIES = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules["swift_smoke_anomalies"] = ANOMALIES
SPEC.loader.exec_module(ANOMALIES)  # type: ignore[attr-defined]


class SwiftSmokeAnomaliesTests(TestCase):
    def _write_result(self, root: Path, lanes: list[dict[str, Any]]) -> Path:
        payload = {
            "generated_at": "2026-03-15T00:00:00Z",
            "buildkite": {"lanes": lanes, "queue_depth": 0, "average_runtime_minutes": 1.0},
            "devices": {"emulators": {"passes": 0, "failures": 0}, "strongbox_capable": {"passes": 0, "failures": 0}},
            "alert_state": {"consecutive_failures": 1, "open_incidents": ["xcframework_smoke_fallback"]},
        }
        path = root / "result.json"
        path.write_text(json.dumps(payload), encoding="utf-8")
        return path

    def test_summarize_anomalies_filters_passing_lanes(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            result_path = self._write_result(
                root,
                [
                    {
                        "name": "ci/xcframework-smoke:iphone-sim",
                        "status": "pass",
                        "success_rate_14_runs": 1.0,
                        "last_failure": None,
                        "flake_count": 0,
                        "mttr_hours": 0.0,
                    },
                    {
                        "name": "ci/xcframework-smoke:strongbox",
                        "status": "fail",
                        "success_rate_14_runs": 0.0,
                        "last_failure": "2026-03-14T00:00:00Z",
                        "flake_count": 1,
                        "mttr_hours": 1.0,
                        "notes": "device offline",
                    },
                ],
            )
            logs_root = root / "logs"
            (logs_root / "strongbox").mkdir(parents=True)
            (logs_root / "strongbox" / "build.log").write_text("fail", encoding="utf-8")

            payload = ANOMALIES.summarize_anomalies(result_path, logs_root)
            self.assertEqual(payload["open_incidents"], ["xcframework_smoke_fallback"])
            anomalies = payload["lanes"]
            self.assertEqual(len(anomalies), 1)
            entry = anomalies[0]
            self.assertEqual(entry["lane"], "ci/xcframework-smoke:strongbox")
            self.assertEqual(entry["status"], "fail")
            self.assertIn("build.log", entry["logs"][0])
            self.assertEqual(entry["message"], "device offline")

    def test_main_writes_summary(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            result_path = self._write_result(
                root,
                [
                    {
                        "name": "ci/xcframework-smoke:macos-fallback",
                        "success_rate_14_runs": 0.0,
                        "last_failure": None,
                        "flake_count": 0,
                        "mttr_hours": 0.0,
                        "notes": "fallback lane",
                    }
                ],
            )
            out_path = root / "out/summary.json"
            rc = ANOMALIES.main(["--result", str(result_path), "--logs-root", str(root), "--out", str(out_path)])
            self.assertEqual(rc, 0)
            payload = json.loads(out_path.read_text(encoding="utf-8"))
            self.assertEqual(payload["lanes"][0]["lane"], "ci/xcframework-smoke:macos-fallback")
            self.assertEqual(payload["lanes"][0]["status"], "fail")
