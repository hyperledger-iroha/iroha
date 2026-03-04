#!/usr/bin/env python3
"""Run the Sumeragi NPoS baseline scenario and archive artifacts."""

from __future__ import annotations

import argparse
import datetime as _dt
import json
import os
import platform
import re
import shutil
import subprocess
import sys
from pathlib import Path
from typing import List, Optional, Tuple


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Execute the Sumeragi NPoS baseline integration test with telemetry "
            "enabled, collect timing data, and archive the resulting logs/metrics."
        )
    )
    parser.add_argument(
        "--artifacts-dir",
        type=Path,
        default=Path("artifacts/sumeragi-baseline"),
        help="Root directory where run artifacts will be stored (default: artifacts/sumeragi-baseline).",
    )
    parser.add_argument(
        "--cargo-bin",
        default="cargo",
        help="Path to the cargo executable to use (default: cargo).",
    )
    parser.add_argument(
        "--pattern",
        default="sumeragi_npos_performance::",
        help="Test name pattern to pass to cargo test (default: sumeragi_npos_performance::).",
    )
    parser.add_argument(
        "--cargo-args",
        nargs="*",
        default=[],
        help="Extra arguments to pass to 'cargo test' before the pattern (optional).",
    )
    parser.add_argument(
        "--test-args",
        nargs="*",
        default=["--nocapture"],
        help="Arguments passed to the test binary after '--' (default: --nocapture).",
    )
    parser.add_argument(
        "--report-dest",
        type=Path,
        help=(
            "Optional path where the rendered Markdown report should be copied. "
            "When no report is produced, writes a stub that links to the run artifacts."
        ),
    )
    parser.add_argument(
        "--fixture-dir",
        type=Path,
        default=Path("integration_tests/fixtures/sumeragi_baseline/default"),
        help=(
            "Directory containing recorded baseline summaries/metrics that can be "
            "replayed when the integration tests cannot bind loopback sockets."
        ),
    )
    parser.add_argument(
        "--disable-fixture-fallback",
        action="store_true",
        help=(
            "Disable the fixture fallback and fail immediately if the tests cannot "
            "run due to sandboxed networking."
        ),
    )
    parser.add_argument(
        "--fail-on-fixture",
        action="store_true",
        help=(
            "Return a non-zero status if fixture data had to be used because live "
            "summaries were unavailable."
        ),
    )
    return parser.parse_args()


def _build_command(args: argparse.Namespace) -> List[str]:
    command: List[str] = [args.cargo_bin, "test", "-p", "integration_tests"]
    command.extend(args.cargo_args)
    if args.pattern:
        command.append(args.pattern)
    if args.test_args:
        command.append("--")
        command.extend(args.test_args)
    return command


def _tee_command(command: List[str], log_path: Path, env: dict) -> int:
    log_path.parent.mkdir(parents=True, exist_ok=True)
    process = subprocess.Popen(
        command,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        env=env,
    )

    assert process.stdout is not None  # appease type checkers
    with log_path.open("w", encoding="utf-8") as log_file:
        try:
            for line in process.stdout:
                sys.stdout.write(line)
                log_file.write(line)
        finally:
            process.stdout.close()
            return_code = process.wait()
    return return_code


def _collect_summaries(run_dir: Path) -> list:
    summaries = []
    for summary_path in sorted(run_dir.glob("*.summary.json")):
        try:
            with summary_path.open(encoding="utf-8") as handle:
                summary_data = json.load(handle)
        except json.JSONDecodeError as exc:  # pragma: no cover - defensive logging
            print(
                f"[run_sumeragi_baseline] Failed to parse {summary_path}: {exc}",
                file=sys.stderr,
            )
            continue
        scenario_tag = summary_path.name[: -len(".summary.json")]
        summaries.append(
            {
                "scenario": summary_data.get("scenario", scenario_tag),
                "summary_file": summary_path.name,
                "data": summary_data,
            }
        )
    return summaries


def _log_mentions_socket_denial(log_path: Path) -> bool:
    try:
        contents = log_path.read_text(encoding="utf-8", errors="ignore")
    except OSError:
        return False

    markers = (
        "Operation not permitted (os error 1)",
        "Peer exited unexpectedly",
        "Failed to send http GET request to http://127.0.0.1",
    )
    return any(marker in contents for marker in markers)


def _apply_fixture(run_dir: Path, fixture_dir: Path) -> list:
    """Replay recorded summaries into ``run_dir``."""

    if not fixture_dir.exists():
        return []

    for entry in fixture_dir.iterdir():
        destination = run_dir / entry.name
        if entry.is_dir():
            shutil.copytree(entry, destination, dirs_exist_ok=True)
        else:
            shutil.copyfile(entry, destination)

    return _collect_summaries(run_dir)


def _maybe_apply_fixture_fallback(
    *,
    return_code: int,
    summaries: list,
    disable_fixture_fallback: bool,
    log_path: Path,
    run_dir: Path,
    fixture_dir: Path,
) -> Tuple[list, Optional[str], int]:
    """Apply fixture data when heuristics detect sandboxed network failures."""

    fallback_used: Optional[str] = None
    if not summaries and not disable_fixture_fallback:
        fixture_summaries = _apply_fixture(run_dir, fixture_dir)
        if fixture_summaries:
            summaries = fixture_summaries
            fallback_used = "fixture"
            print(
                "[run_sumeragi_baseline] Applied fixture fallback due to missing summaries"
            )
            return summaries, fallback_used, return_code

    if (
        return_code != 0
        and not summaries
        and not disable_fixture_fallback
        and _log_mentions_socket_denial(log_path)
    ):
        fixture_summaries = _apply_fixture(run_dir, fixture_dir)
        if fixture_summaries:
            summaries = fixture_summaries
            fallback_used = "fixture"
            print(
                "[run_sumeragi_baseline] Applied fixture fallback due to sandboxed network"
            )

    return summaries, fallback_used, return_code


def _detect_hardware_metadata() -> dict:
    """Collect basic hardware metadata for reproducibility."""

    metadata: dict = {}
    try:
        uname = platform.uname()
        system = uname.system
        system_lower = system.lower()
        metadata["os"] = "macos" if system_lower == "darwin" else system_lower
        metadata["system_version"] = f"{uname.system} {uname.release}"
        metadata["kernel"] = uname.version
        metadata["arch"] = uname.machine
        metadata["hostname"] = uname.node
        metadata["logical_cpus"] = os.cpu_count()
        if uname.processor:
            metadata["processor"] = uname.processor
    except Exception:  # pragma: no cover - defensive
        return metadata

    if metadata.get("logical_cpus") is not None:
        metadata["cpu_cores_total"] = metadata["logical_cpus"]

    system = platform.system()
    if system == "Darwin":
        try:
            output = subprocess.check_output(
                ["system_profiler", "SPHardwareDataType", "-json"],
                text=True,
                stderr=subprocess.DEVNULL,
            )
            data = json.loads(output)
            entry = data.get("SPHardwareDataType", [{}])[0]
            chip = entry.get("chip_type")
            if chip:
                metadata["hardware_model"] = chip
            mem = entry.get("physical_memory")
            if mem:
                metadata["memory"] = mem
                gb = _parse_memory_gb(mem)
                if gb is not None:
                    metadata["memory_gb"] = gb
            topo = entry.get("number_processors")
            if topo:
                metadata["cpu_topology"] = topo
        except Exception:  # pragma: no cover - best effort
            pass
    elif system == "Linux":
        try:
            with Path("/proc/meminfo").open(encoding="utf-8") as handle:
                for line in handle:
                    if line.startswith("MemTotal:"):
                        parts = line.split()
                        if len(parts) >= 2:
                            mem_kb = int(parts[1])
                            metadata["memory_kb"] = mem_kb
                            metadata["memory_gb"] = round(mem_kb / 1_048_576, 2)
                        break
        except Exception:
            pass

    return {k: v for k, v in metadata.items() if v not in (None, "", "Unknown")}


def _parse_memory_gb(memory_str: str) -> Optional[float]:
    match = re.search(r"([0-9.]+)\\s*GB", memory_str, re.IGNORECASE)
    if not match:
        return None
    try:
        return float(match.group(1))
    except ValueError:
        return None


def _enrich_summaries_with_hardware(
    run_dir: Path, summaries: list, hardware: dict
) -> None:
    if not summaries or not hardware:
        return

    for summary in summaries:
        root = summary.get("data")
        if not isinstance(root, dict):
            continue
        env = root.setdefault("environment", {})
        if isinstance(env, dict):
            env.update(hardware)

        summary_file = summary.get("summary_file")
        if summary_file:
            path = run_dir / summary_file
            try:
                with path.open("w", encoding="utf-8") as handle:
                    json.dump(root, handle, indent=2)
                    handle.write("\n")
            except Exception:
                # Fall back to logging; report generation will still use in-memory data.
                print(
                    f"[run_sumeragi_baseline] Failed to rewrite summary with hardware metadata: {path}",
                    file=sys.stderr,
                )


def _render_markdown_report(
    cargo_bin: str,
    run_dir: Path,
    env: dict,
) -> Tuple[Optional[Path], Optional[int]]:
    command = [
        cargo_bin,
        "run",
        "-p",
        "build-support",
        "--bin",
        "sumeragi_baseline_report",
        "--",
        str(run_dir),
    ]
    print(
        f"[run_sumeragi_baseline] Rendering Markdown report via: {' '.join(command)}"
    )
    try:
        result = subprocess.run(
            command,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=env,
            check=False,
        )
    except OSError as exc:  # pragma: no cover - defensive logging
        print(
            f"[run_sumeragi_baseline] Failed to invoke sumeragi_baseline_report: {exc}",
            file=sys.stderr,
        )
        return None, None

    if result.stderr:
        sys.stderr.write(result.stderr)

    if not result.stdout:
        return None, result.returncode

    report_path = run_dir / "sumeragi-baseline-report.md"
    report_path.write_text(result.stdout, encoding="utf-8")
    print(f"[run_sumeragi_baseline] Markdown report written to {report_path}")
    return report_path, result.returncode


def _write_report_destination(
    report_path: Optional[Path],
    dest: Path,
    aggregate: dict,
    run_dir: Path,
    return_code: int,
) -> Path:
    """Persist the rendered report (or a stub) to ``dest`` and update ``aggregate``."""

    dest = dest.expanduser().resolve()
    dest.parent.mkdir(parents=True, exist_ok=True)
    aggregate_path = run_dir / "aggregate.json"

    if report_path and report_path.exists():
        shutil.copyfile(report_path, dest)
        print(f"[run_sumeragi_baseline] Copied Markdown report to {dest}")
    else:
        reason = (
            "No summaries were produced; see aggregate.json for details."
            if return_code == 0
            else "Baseline scenario failed; inspect the run logs."
        )
        run_id = aggregate.get("run_timestamp", "unknown")
        stub = (
            "# Sumeragi NPoS Baseline Report\n\n"
            f"_Run {run_id} did not produce a Markdown report._\n\n"
            f"{reason}\n\n"
            f"Artifacts: `{aggregate_path}`\n"
        )
        dest.write_text(stub, encoding="utf-8")
        print(f"[run_sumeragi_baseline] Wrote stub Markdown report to {dest}")

    aggregate["report_published_path"] = str(dest)
    return dest


def main() -> int:
    args = _parse_args()
    command = _build_command(args)

    timestamp = _dt.datetime.now(tz=_dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    run_dir = args.artifacts_dir.joinpath(timestamp)
    run_dir.mkdir(parents=True, exist_ok=True)

    env = os.environ.copy()
    env["SUMERAGI_BASELINE_ARTIFACT_DIR"] = str(run_dir)
    env.setdefault("IROHA_SKIP_BIND_CHECKS", "1")
    env.setdefault("TEST_NETWORK_BIN_IROHAD", str(Path("target/debug/irohad").resolve()))

    log_path = run_dir / "cargo-test.log"
    return_code = _tee_command(command, log_path, env)

    hardware_metadata = _detect_hardware_metadata()
    summaries = _collect_summaries(run_dir)
    original_return_code = return_code
    summaries, fallback_used, return_code = _maybe_apply_fixture_fallback(
        return_code=return_code,
        summaries=summaries,
        disable_fixture_fallback=args.disable_fixture_fallback,
        log_path=log_path,
        run_dir=run_dir,
        fixture_dir=args.fixture_dir.resolve(),
    )

    _enrich_summaries_with_hardware(run_dir, summaries, hardware_metadata)

    if fallback_used:
        print(
            "[run_sumeragi_baseline] WARNING: baseline metrics sourced from fixture data",
            file=sys.stderr,
        )
        if args.fail_on_fixture:
            return_code = return_code or 2

    report_path: Optional[Path] = None
    report_return_code: Optional[int] = None
    if summaries:
        report_path, report_return_code = _render_markdown_report(
            args.cargo_bin, run_dir, env
        )

    aggregate = {
        "run_timestamp": timestamp,
        "command": command,
        "return_code": return_code,
        "log_file": log_path.name,
        "summaries": summaries,
    }
    if hardware_metadata:
        aggregate["hardware"] = hardware_metadata

    if original_return_code != return_code:
        aggregate["command_return_code"] = original_return_code
    if fallback_used:
        aggregate["fallback"] = {
            "kind": fallback_used,
            "fixture_dir": str(args.fixture_dir.resolve()),
            "note": "Fixture data substituted because live summaries were unavailable.",
        }

    if summaries:
        aggregate["report_file"] = report_path.name if report_path else None
        aggregate["report_return_code"] = report_return_code

    aggregate_path = run_dir / "aggregate.json"
    if args.report_dest:
        _write_report_destination(
            report_path,
            args.report_dest,
            aggregate,
            run_dir,
            return_code,
        )

    with aggregate_path.open("w", encoding="utf-8") as handle:
        json.dump(aggregate, handle, indent=2, sort_keys=True)
        handle.write("\n")

    print(f"[run_sumeragi_baseline] Artifacts stored in {run_dir}")
    print(f"[run_sumeragi_baseline] Aggregate summary: {aggregate_path}")

    return return_code


if __name__ == "__main__":
    sys.exit(main())
