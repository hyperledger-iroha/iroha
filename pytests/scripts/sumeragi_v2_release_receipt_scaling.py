"""Source-pinned synthetic scaling bundles for release-receipt contract tests.

The actual validator must recompute this synthetic fixture before it is used by
a receipt test. It is not live scaling evidence or a release qualification.
"""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys

from scripts.tests.multilane_scaling_fixture import ScalingRunFixture, fixed_workload
from pytests.scripts.sumeragi_v2_release_receipt_test_support import (
    SCALING_CONFIGURATION_DATA,
    SCALING_IROHAD_SHA256,
    SCALING_IROHA_CLI_SHA256,
    SCALING_TRIAL_HARNESS_DATA,
    sha256,
)

ROOT_DIR = Path(__file__).resolve().parents[2]


def make_scaling_evidence(
    tmp_path: Path, *, head: str, sealed_manifest: str
) -> dict[str, Path | str]:
    root = tmp_path / "scaling"
    inputs = root / "inputs"
    tooling_dir = root / "tooling"
    inputs.mkdir(parents=True)
    tooling_dir.mkdir()

    def write_json(path: Path, value: object) -> None:
        path.write_text(
            json.dumps(value, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )

    def ref(path: Path) -> dict[str, str]:
        return {
            "path": path.relative_to(root).as_posix(),
            "sha256": sha256(path),
        }

    config = inputs / "nexus_config.toml"
    config.write_bytes(SCALING_CONFIGURATION_DATA)
    identity = {
        "schema": "iroha.sumeragi_v2.multilane_scaling.identity.v1",
        "hardware": {
            "machine_id": "receipt-contract-host",
            "cpu_model": "Receipt Contract CPU",
            "physical_core_count": 8,
            "logical_core_count": 16,
            "memory_bytes": 32_000_000_000,
            "storage_model": "Receipt Contract NVMe",
        },
        "software": {
            "os": "ContractOS",
            "kernel": "contract-kernel",
            "architecture": "x86_64",
            "python_version": "3.9.contract",
            "rustc_version": "rustc contract",
            "source_revision": head,
            "workspace_source_sha256": sealed_manifest,
            "nexus_config_sha256": sha256(config),
            "irohad_sha256": SCALING_IROHAD_SHA256,
            "iroha_cli_sha256": SCALING_IROHA_CLI_SHA256,
        },
    }
    identity_path = inputs / "identity.json"
    write_json(identity_path, identity)
    harness = inputs / "trial_harness.sh"
    harness.write_bytes(SCALING_TRIAL_HARNESS_DATA)

    validator_source = (
        ROOT_DIR / "scripts" / "nexus" / "validate_multilane_scaling_evidence.py"
    )
    validator = tooling_dir / validator_source.name
    shutil.copy2(validator_source, validator)
    required_tooling = (
        ("localnet", "scripts/deploy_localnet.sh"),
        ("load_generator", "scripts/tx_load.py"),
        ("nexus_load_bundle", "scripts/nexus_lane_load_test.py"),
    )
    tooling = []
    for role, source_path in required_tooling:
        source = ROOT_DIR / source_path
        artifact = tooling_dir / source.name
        shutil.copy2(source, artifact)
        tooling.append(
            {
                "role": role,
                "source_path": source_path,
                "artifact": ref(artifact),
            }
        )

    workload = fixed_workload()
    run_fixture = ScalingRunFixture(root, identity, workload)
    budgets = {
        "queue_depth_max": 100,
        "index_entries_max": 200,
        "memory_bytes_max": 10_000,
        "disk_bytes_max": 20_000,
    }
    namespace = "receipt-contract-g-scale"
    runs = []
    sequence = 0
    for pair_index in range(1, 6):
        seed = hashlib.sha256(
            f"{namespace}:{pair_index}".encode("utf-8")
        ).hexdigest()
        for variant, lane_count, committed, latency in (
            ("one_lane", 1, 100, 10.0),
            ("four_lane", 4, 160, 12.0),
        ):
            sequence += 1
            run_dir = root / "runs" / f"pair_{pair_index:02d}" / variant
            run_dir.mkdir(parents=True)
            raw = run_dir / "raw_samples.json"
            write_json(
                raw,
                run_fixture.raw_run(
                    pair_index=pair_index,
                    variant=variant,
                    active_lanes=lane_count,
                    seed=seed,
                    committed=committed,
                    latency=latency,
                ),
            )
            command_log = run_dir / "trial.log"
            command_log.write_text("receipt scaling trial passed\n", encoding="utf-8")
            runs.append(
                {
                    "sequence": sequence,
                    "pair_index": pair_index,
                    "variant": variant,
                    "active_execution_lanes": lane_count,
                    "seed": seed,
                    "status": "passed",
                    "skipped": False,
                    "exit_code": 0,
                    "raw_samples": ref(raw),
                    "command_log": ref(command_log),
                }
            )

    manifest = root / "scaling_evidence.json"
    write_json(
        manifest,
        {
            "schema": "iroha.sumeragi_v2.multilane_scaling.evidence.v1",
            "generated_at_utc": "2026-07-23T12:00:00Z",
            "pair_count": 5,
            "seed_namespace": namespace,
            "seed_derivation": (
                "sha256(seed_namespace + ':' + decimal_pair_index)"
            ),
            "identity": ref(identity_path),
            "configuration": ref(config),
            "workload": workload,
            "budgets": budgets,
            "observation_scope": {
                "queue": "maximum per-peer queue depth",
                "index": "designated peer lane index entries",
                "memory": "aggregate peer RSS",
                "disk": "aggregate lane storage bytes",
            },
            "thresholds": {
                "min_four_lane_throughput_ratio": 1.5,
                "max_four_lane_p95_latency_ratio": 1.25,
            },
            "trial_harness": ref(harness),
            "validator": ref(validator),
            "tooling": tooling,
            "runs": runs,
        },
    )
    report = root / "validation_report.json"
    validation = subprocess.run(
        [
            sys.executable,
            str(validator_source),
            str(manifest),
            "--report",
            str(report),
            "--quiet",
        ],
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if validation.returncode != 0:
        details = report.read_text(encoding="utf-8") if report.is_file() else "validation report was not produced"
        raise AssertionError(
            f"synthetic scaling fixture validation failed ({validation.returncode}):\n"
            f"{details}\nstdout: {validation.stdout}\nstderr: {validation.stderr}"
        )
    return {
        "scaling_root": root,
        "scaling_manifest": manifest,
        "scaling_report": report,
        "scaling_identity": identity_path,
        "scaling_validator": validator,
        "scaling_trial_log": root / runs[0]["command_log"]["path"],
        "expected_scaling_trial_harness_sha256": sha256(harness),
        "expected_scaling_configuration_sha256": sha256(config),
        "expected_scaling_irohad_sha256": SCALING_IROHAD_SHA256,
        "expected_scaling_iroha_cli_sha256": SCALING_IROHA_CLI_SHA256,
    }
