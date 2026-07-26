#!/usr/bin/env python3
"""Run the five-pair Sumeragi V2 horizontal multilane scaling release gate.

Prerequisites:

* a pinned identity JSON document matching the validator's identity schema;
* a pinned Nexus configuration;
* an executable trial harness that composes the existing localnet, ``tx_load``,
  and Nexus lane-load tooling and writes the requested raw-sample JSON file.

The harness receives all run parameters through ``IROHA_GSCALE_*`` environment
variables.  This orchestrator never builds Iroha and never synthesizes samples.
It runs exactly ten trials in one-lane/four-lane pair order, archives the
inputs, command logs, raw outputs, and tool hashes, then invokes the strict
evidence validator.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Sequence


SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parents[1]
VALIDATOR = SCRIPT_DIR / "validate_multilane_scaling_evidence.py"

EVIDENCE_SCHEMA = "iroha.sumeragi_v2.multilane_scaling.evidence.v1"
RUN_SCHEMA = "iroha.sumeragi_v2.multilane_scaling.run.v1"
EXPECTED_PAIR_COUNT = 5
MIN_INTERVAL_SAMPLES = 20
MIN_LATENCY_SAMPLES = 100
MIN_THROUGHPUT_RATIO = 1.5
MAX_P95_LATENCY_RATIO = 1.25
MAX_OFFERED_LOAD_DEVIATION_FRACTION = 0.01
SEED_DERIVATION = "sha256(seed_namespace + ':' + decimal_pair_index)"

REQUIRED_TOOLING = (
    ("localnet", Path("scripts/deploy_localnet.sh")),
    ("load_generator", Path("scripts/tx_load.py")),
    ("nexus_load_bundle", Path("scripts/nexus_lane_load_test.py")),
)


class RunnerError(RuntimeError):
    """The scaling-gate runner cannot produce valid evidence."""


def sha256_file(path: Path) -> str:
    """Return the SHA-256 digest of *path* without loading it all into memory."""

    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def derive_seed(namespace: str, pair_index: int) -> str:
    """Return the deterministic seed shared by both variants of one pair."""

    return hashlib.sha256(f"{namespace}:{pair_index}".encode("utf-8")).hexdigest()


def file_ref(path: Path, root: Path) -> dict[str, str]:
    """Return an in-bundle relative path plus its SHA-256 digest."""

    return {
        "path": path.relative_to(root).as_posix(),
        "sha256": sha256_file(path),
    }


def require_input_file(path: Path, label: str, *, executable: bool = False) -> Path:
    """Resolve one regular, non-symlink input file."""

    if path.is_symlink() or not path.is_file():
        raise RunnerError(f"{label} must be a regular non-symlink file: {path}")
    resolved = path.resolve()
    if executable and not os.access(resolved, os.X_OK):
        raise RunnerError(f"{label} must be executable: {path}")
    return resolved


def write_json(path: Path, payload: Any) -> None:
    """Atomically write deterministic pretty JSON."""

    temporary = path.with_name(f".{path.name}.tmp-{os.getpid()}")
    temporary.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    os.replace(temporary, path)


def copy_artifact(source: Path, destination: Path) -> None:
    """Copy a pinned regular file into the evidence bundle."""

    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination, follow_symlinks=False)


def positive_int(raw: str) -> int:
    try:
        value = int(raw)
    except ValueError as error:
        raise argparse.ArgumentTypeError("expected an integer") from error
    if value <= 0:
        raise argparse.ArgumentTypeError("expected an integer greater than zero")
    return value


def nonnegative_float(raw: str) -> float:
    try:
        value = float(raw)
    except ValueError as error:
        raise argparse.ArgumentTypeError("expected a number") from error
    if not math.isfinite(value) or value < 0:
        raise argparse.ArgumentTypeError("expected a finite number >= 0")
    return value


def positive_float(raw: str) -> float:
    value = nonnegative_float(raw)
    if value <= 0:
        raise argparse.ArgumentTypeError("expected a finite number > 0")
    return value


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--artifact-dir",
        required=True,
        type=Path,
        help="New directory that will contain the complete evidence bundle.",
    )
    parser.add_argument(
        "--identity-file",
        required=True,
        type=Path,
        help="Pinned hardware/software identity JSON.",
    )
    parser.add_argument(
        "--config-file",
        required=True,
        type=Path,
        help="Exact Nexus/localnet configuration used by every trial.",
    )
    parser.add_argument(
        "--trial-command",
        required=True,
        type=Path,
        help="Executable no-argument harness invoked once per trial.",
    )
    parser.add_argument("--seed-namespace", required=True)
    parser.add_argument("--offered-load-tps", required=True, type=positive_float)
    parser.add_argument("--warmup-seconds", required=True, type=nonnegative_float)
    parser.add_argument("--measurement-seconds", required=True, type=positive_float)
    parser.add_argument(
        "--min-interval-samples",
        type=positive_int,
        default=MIN_INTERVAL_SAMPLES,
        help=f"Minimum intervals per run; cannot be below {MIN_INTERVAL_SAMPLES}.",
    )
    parser.add_argument(
        "--min-latency-samples",
        type=positive_int,
        default=MIN_LATENCY_SAMPLES,
        help=f"Minimum per-transaction latencies per run; cannot be below {MIN_LATENCY_SAMPLES}.",
    )
    parser.add_argument("--max-queue-depth", required=True, type=positive_int)
    parser.add_argument("--max-index-entries", required=True, type=positive_int)
    parser.add_argument("--max-memory-bytes", required=True, type=positive_int)
    parser.add_argument("--max-disk-bytes", required=True, type=positive_int)
    parser.add_argument("--queue-observation-scope", required=True)
    parser.add_argument("--index-observation-scope", required=True)
    parser.add_argument("--memory-observation-scope", required=True)
    parser.add_argument("--disk-observation-scope", required=True)
    parser.add_argument(
        "--trial-timeout-seconds",
        type=positive_float,
        default=3600.0,
        help="Hard timeout for each trial command (default: 3600).",
    )
    parser.add_argument(
        "--python",
        default=sys.executable,
        help="Python interpreter used for the evidence validator.",
    )
    args = parser.parse_args(argv)
    if args.min_interval_samples < MIN_INTERVAL_SAMPLES:
        parser.error(f"--min-interval-samples cannot be below {MIN_INTERVAL_SAMPLES}")
    if args.min_latency_samples < MIN_LATENCY_SAMPLES:
        parser.error(f"--min-latency-samples cannot be below {MIN_LATENCY_SAMPLES}")
    if (
        not args.seed_namespace
        or args.seed_namespace != args.seed_namespace.strip()
        or len(args.seed_namespace) > 128
        or not args.seed_namespace[0].isalnum()
        or any(
            not (character.isascii() and (character.isalnum() or character in "._-"))
            for character in args.seed_namespace
        )
    ):
        parser.error(
            "--seed-namespace must be 1-128 ASCII letters, digits, '.', '_', or '-', "
            "starting with a letter or digit"
        )
    for field in (
        "queue_observation_scope",
        "index_observation_scope",
        "memory_observation_scope",
        "disk_observation_scope",
    ):
        value = getattr(args, field)
        if (
            not value
            or value != value.strip()
            or "\n" in value
            or "\r" in value
            or "\x00" in value
        ):
            parser.error(f"--{field.replace('_', '-')} must be a trimmed single-line declaration")
    return args


def _base_manifest(
    args: argparse.Namespace,
    *,
    identity_ref: dict[str, str],
    config_ref: dict[str, str],
    harness_ref: dict[str, str],
    validator_ref: dict[str, str],
    tooling: list[dict[str, Any]],
) -> dict[str, Any]:
    runs = []
    sequence = 0
    for pair_index in range(1, EXPECTED_PAIR_COUNT + 1):
        seed = derive_seed(args.seed_namespace, pair_index)
        for variant, lanes in (("one_lane", 1), ("four_lane", 4)):
            sequence += 1
            runs.append(
                {
                    "sequence": sequence,
                    "pair_index": pair_index,
                    "variant": variant,
                    "active_execution_lanes": lanes,
                    "seed": seed,
                    "status": "pending",
                    "skipped": False,
                    "exit_code": None,
                    "raw_samples": None,
                    "command_log": None,
                }
            )
    return {
        "schema": EVIDENCE_SCHEMA,
        "generated_at_utc": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "pair_count": EXPECTED_PAIR_COUNT,
        "seed_namespace": args.seed_namespace,
        "seed_derivation": SEED_DERIVATION,
        "identity": identity_ref,
        "configuration": config_ref,
        "workload": {
            "offered_load_tps": args.offered_load_tps,
            "warmup_seconds": args.warmup_seconds,
            "measurement_seconds": args.measurement_seconds,
            "min_interval_samples": args.min_interval_samples,
            "min_latency_samples": args.min_latency_samples,
            "max_offered_load_deviation_fraction": MAX_OFFERED_LOAD_DEVIATION_FRACTION,
        },
        "budgets": {
            "queue_depth_max": args.max_queue_depth,
            "index_entries_max": args.max_index_entries,
            "memory_bytes_max": args.max_memory_bytes,
            "disk_bytes_max": args.max_disk_bytes,
        },
        "observation_scope": {
            "queue": args.queue_observation_scope,
            "index": args.index_observation_scope,
            "memory": args.memory_observation_scope,
            "disk": args.disk_observation_scope,
        },
        "thresholds": {
            "min_four_lane_throughput_ratio": MIN_THROUGHPUT_RATIO,
            "max_four_lane_p95_latency_ratio": MAX_P95_LATENCY_RATIO,
        },
        "trial_harness": harness_ref,
        "validator": validator_ref,
        "tooling": tooling,
        "runs": runs,
    }


def _trial_environment(
    *,
    manifest: dict[str, Any],
    run: dict[str, Any],
    artifact_dir: Path,
    run_dir: Path,
    raw_path: Path,
    identity_path: Path,
    config_path: Path,
) -> dict[str, str]:
    env = os.environ.copy()
    values = {
        "IROHA_GSCALE_RUN_SCHEMA": RUN_SCHEMA,
        "IROHA_GSCALE_REPO_ROOT": str(REPO_ROOT),
        "IROHA_GSCALE_ARTIFACT_DIR": str(artifact_dir),
        "IROHA_GSCALE_RUN_DIR": str(run_dir),
        "IROHA_GSCALE_RAW_SAMPLES_OUT": str(raw_path),
        "IROHA_GSCALE_IDENTITY_FILE": str(identity_path),
        "IROHA_GSCALE_CONFIG_FILE": str(config_path),
        "IROHA_GSCALE_DEPLOY_LOCALNET": str(REPO_ROOT / "scripts" / "deploy_localnet.sh"),
        "IROHA_GSCALE_TX_LOAD": str(REPO_ROOT / "scripts" / "tx_load.py"),
        "IROHA_GSCALE_NEXUS_LANE_LOAD_TEST": str(
            REPO_ROOT / "scripts" / "nexus_lane_load_test.py"
        ),
        "IROHA_GSCALE_SEQUENCE": run["sequence"],
        "IROHA_GSCALE_PAIR_INDEX": run["pair_index"],
        "IROHA_GSCALE_VARIANT": run["variant"],
        "IROHA_GSCALE_ACTIVE_EXECUTION_LANES": run["active_execution_lanes"],
        "IROHA_GSCALE_SEED": run["seed"],
        "IROHA_GSCALE_OFFERED_LOAD_TPS": manifest["workload"]["offered_load_tps"],
        "IROHA_GSCALE_WARMUP_SECONDS": manifest["workload"]["warmup_seconds"],
        "IROHA_GSCALE_MEASUREMENT_SECONDS": manifest["workload"]["measurement_seconds"],
        "IROHA_GSCALE_MIN_INTERVAL_SAMPLES": manifest["workload"]["min_interval_samples"],
        "IROHA_GSCALE_MIN_LATENCY_SAMPLES": manifest["workload"]["min_latency_samples"],
        "IROHA_GSCALE_MAX_QUEUE_DEPTH": manifest["budgets"]["queue_depth_max"],
        "IROHA_GSCALE_MAX_INDEX_ENTRIES": manifest["budgets"]["index_entries_max"],
        "IROHA_GSCALE_MAX_MEMORY_BYTES": manifest["budgets"]["memory_bytes_max"],
        "IROHA_GSCALE_MAX_DISK_BYTES": manifest["budgets"]["disk_bytes_max"],
    }
    env.update({name: str(value) for name, value in values.items()})
    return env


def _run_validator(
    args: argparse.Namespace,
    validator_path: Path,
    manifest_path: Path,
    report_path: Path,
) -> int:
    result = subprocess.run(
        [
            args.python,
            str(validator_path),
            str(manifest_path),
            "--report",
            str(report_path),
        ],
        cwd=REPO_ROOT,
        check=False,
    )
    return result.returncode


def run(args: argparse.Namespace) -> int:
    artifact_dir = args.artifact_dir.absolute()
    if artifact_dir.exists():
        raise RunnerError(
            f"--artifact-dir already exists; refusing to overwrite release evidence: {artifact_dir}"
        )

    identity_source = require_input_file(args.identity_file, "--identity-file")
    config_source = require_input_file(args.config_file, "--config-file")
    trial_source = require_input_file(
        args.trial_command,
        "--trial-command",
        executable=True,
    )
    tooling_sources: list[tuple[str, Path, Path]] = []
    for role, relative in REQUIRED_TOOLING:
        source = require_input_file(REPO_ROOT / relative, f"required {role} tool")
        tooling_sources.append((role, relative, source))
    validator_source = require_input_file(VALIDATOR, "scaling evidence validator")

    # Parse the declaration before creating output.  Full field validation is
    # intentionally delegated to the release validator.
    try:
        identity_payload = json.loads(identity_source.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise RunnerError(f"--identity-file is not UTF-8 JSON: {error}") from error
    if not isinstance(identity_payload, dict):
        raise RunnerError("--identity-file must contain a JSON object")

    artifact_dir.mkdir(parents=True)
    inputs_dir = artifact_dir / "inputs"
    tools_dir = artifact_dir / "tooling"
    inputs_dir.mkdir()
    tools_dir.mkdir()

    identity_path = inputs_dir / "identity.json"
    config_suffix = config_source.suffix if config_source.suffix else ".bin"
    config_path = inputs_dir / f"nexus_config{config_suffix}"
    harness_suffix = trial_source.suffix if trial_source.suffix else ".bin"
    harness_path = inputs_dir / f"trial_harness{harness_suffix}"
    validator_path = tools_dir / "validate_multilane_scaling_evidence.py"
    copy_artifact(identity_source, identity_path)
    copy_artifact(config_source, config_path)
    copy_artifact(trial_source, harness_path)
    copy_artifact(validator_source, validator_path)

    tooling: list[dict[str, Any]] = []
    pinned_sources: dict[Path, str] = {
        identity_source: sha256_file(identity_source),
        config_source: sha256_file(config_source),
        trial_source: sha256_file(trial_source),
    }
    for role, relative, source in tooling_sources:
        destination = tools_dir / relative.name
        copy_artifact(source, destination)
        pinned_sources[source] = sha256_file(source)
        tooling.append(
            {
                "role": role,
                "source_path": relative.as_posix(),
                "artifact": file_ref(destination, artifact_dir),
            }
        )

    manifest = _base_manifest(
        args,
        identity_ref=file_ref(identity_path, artifact_dir),
        config_ref=file_ref(config_path, artifact_dir),
        harness_ref=file_ref(harness_path, artifact_dir),
        validator_ref=file_ref(validator_path, artifact_dir),
        tooling=tooling,
    )
    manifest_path = artifact_dir / "scaling_evidence.json"
    report_path = artifact_dir / "validation_report.json"
    write_json(manifest_path, manifest)

    for run_entry in manifest["runs"]:
        pair_index = run_entry["pair_index"]
        variant = run_entry["variant"]
        run_dir = artifact_dir / "runs" / f"pair_{pair_index:02d}" / variant
        run_dir.mkdir(parents=True)
        raw_path = run_dir / "raw_samples.json"
        log_path = run_dir / "trial.log"

        drifted_input: Path | None = None
        for source, expected_digest in pinned_sources.items():
            if not source.is_file() or source.is_symlink() or sha256_file(source) != expected_digest:
                drifted_input = source
                break
        if drifted_input is not None:
            log_path.write_text(
                f"pinned input drifted before trial: {drifted_input}\n",
                encoding="utf-8",
            )
            run_entry.update(
                {
                    "status": "failed",
                    "exit_code": 4,
                    "command_log": file_ref(log_path, artifact_dir),
                }
            )
            write_json(manifest_path, manifest)
            break

        print(
            f"[g-scale] pair {pair_index}/{EXPECTED_PAIR_COUNT} "
            f"variant={variant} seed={run_entry['seed']}"
        )
        env = _trial_environment(
            manifest=manifest,
            run=run_entry,
            artifact_dir=artifact_dir,
            run_dir=run_dir,
            raw_path=raw_path,
            identity_path=identity_path,
            config_path=config_path,
        )
        exit_code: int
        with log_path.open("wb") as log:
            try:
                result = subprocess.run(
                    [str(trial_source)],
                    cwd=REPO_ROOT,
                    env=env,
                    stdout=log,
                    stderr=subprocess.STDOUT,
                    timeout=args.trial_timeout_seconds,
                    check=False,
                )
                exit_code = result.returncode
            except subprocess.TimeoutExpired:
                log.write(
                    (
                        f"\ntrial timed out after {args.trial_timeout_seconds} seconds\n"
                    ).encode("utf-8")
                )
                exit_code = 124

        for source, expected_digest in pinned_sources.items():
            if not source.is_file() or source.is_symlink() or sha256_file(source) != expected_digest:
                with log_path.open("ab") as log:
                    log.write(f"\npinned input drifted during trial: {source}\n".encode("utf-8"))
                exit_code = 4
                break

        raw_ref: dict[str, str] | None = None
        if raw_path.is_file() and not raw_path.is_symlink():
            raw_ref = file_ref(raw_path, artifact_dir)
        elif exit_code == 0:
            with log_path.open("ab") as log:
                log.write(b"\ntrial returned zero without a regular raw_samples.json file\n")
            exit_code = 3

        run_entry.update(
            {
                "status": "passed" if exit_code == 0 else "failed",
                "exit_code": exit_code,
                "raw_samples": raw_ref,
                "command_log": file_ref(log_path, artifact_dir),
            }
        )
        write_json(manifest_path, manifest)
        if exit_code != 0:
            break

    validation_status = _run_validator(args, validator_path, manifest_path, report_path)
    if validation_status != 0:
        print(
            f"[g-scale] evidence validation failed; report: {report_path}",
            file=sys.stderr,
        )
        return 1
    print(f"[g-scale] evidence bundle passed validation: {artifact_dir}")
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        return run(args)
    except (RunnerError, OSError) as error:
        print(f"[g-scale] runner error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
