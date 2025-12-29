#!/usr/bin/env python3
"""Generate differential-privacy calibration artefacts for SoraNet telemetry.

This lightweight harness produces deterministic synthetic workloads covering
entry, middle, and exit relays. It derives Laplace noise scales from the
configured epsilon budget and emits:

* `artifacts/soranet_privacy_dp/summary.json` – scenario-level epsilon/δ,
  sensitivity, and suppression coverage.
* `artifacts/soranet_privacy_dp/suppression_matrix.csv` – contributor count vs.
  suppression flag for governance review.

The script is intentionally dependency-free so it can run in CI and offline
operator environments.
"""

from __future__ import annotations

import csv
import json
import math
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, List

# Directories relative to repo root.
ARTIFACT_ROOT = Path("artifacts/soranet_privacy_dp")
ARTIFACT_ROOT.mkdir(parents=True, exist_ok=True)

@dataclass(frozen=True)
class Scenario:
    name: str
    role: str
    contributors: int
    sensitivity: float
    epsilon: float
    delta: float

    def laplace_scale(self) -> float:
        return self.sensitivity / self.epsilon

    def noise_stddev(self) -> float:
        return math.sqrt(2) * self.laplace_scale()

SCENARIOS: List[Scenario] = [
    Scenario(
        name="entry-baseline",
        role="entry",
        contributors=18,
        sensitivity=1.0,
        epsilon=1.2,
        delta=5e-6,
    ),
    Scenario(
        name="middle-congested",
        role="middle",
        contributors=12,
        sensitivity=1.0,
        epsilon=0.95,
        delta=1e-5,
    ),
    Scenario(
        name="exit-low-sample",
        role="exit",
        contributors=9,
        sensitivity=1.0,
        epsilon=0.85,
        delta=1e-5,
    ),
]

SUPPRESSION_THRESHOLD = 12


def compute_summary(scenarios: Iterable[Scenario]) -> dict:
    summary = {
        "schema": "soranet_privacy_dp_summary_v1",
        "suppression_threshold": SUPPRESSION_THRESHOLD,
        "scenarios": [],
    }
    for scenario in scenarios:
        suppressed = scenario.contributors < SUPPRESSION_THRESHOLD
        summary["scenarios"].append(
            {
                "name": scenario.name,
                "role": scenario.role,
                "contributors": scenario.contributors,
                "epsilon": scenario.epsilon,
                "delta": scenario.delta,
                "sensitivity": scenario.sensitivity,
                "laplace_scale": round(scenario.laplace_scale(), 4),
                "noise_stddev": round(scenario.noise_stddev(), 4),
                "suppressed": suppressed,
            }
        )
    return summary


def write_summary(summary: dict) -> None:
    out_path = ARTIFACT_ROOT / "summary.json"
    with out_path.open("w", encoding="utf-8") as fh:
        json.dump(summary, fh, indent=2)
        fh.write("\n")


def write_suppression_matrix(scenarios: Iterable[Scenario]) -> None:
    out_path = ARTIFACT_ROOT / "suppression_matrix.csv"
    with out_path.open("w", newline="", encoding="utf-8") as fh:
        writer = csv.writer(fh)
        writer.writerow(["scenario", "role", "contributors", "suppressed"])
        for scenario in scenarios:
            writer.writerow(
                [
                    scenario.name,
                    scenario.role,
                    scenario.contributors,
                    "yes" if scenario.contributors < SUPPRESSION_THRESHOLD else "no",
                ]
            )


def main() -> None:
    summary = compute_summary(SCENARIOS)
    write_summary(summary)
    write_suppression_matrix(SCENARIOS)
    print(f"Wrote calibration artefacts to {ARTIFACT_ROOT}")


if __name__ == "__main__":
    main()
