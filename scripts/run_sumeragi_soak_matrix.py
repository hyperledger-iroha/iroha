#!/usr/bin/env python3
"""Run the Sumeragi NPoS stress scenarios across a multi-peer soak matrix.

The helper iterates over a set of (peers, collectors_k, redundant_send_r)
combinations, executes ``scripts/run_sumeragi_stress.py`` for each, renders a
per-scenario Markdown summary, and records an aggregate matrix report.  The
generated directory can optionally be packed into a ZIP archive that operators
attach to their sign-off notes.

Example:

    python3 scripts/run_sumeragi_soak_matrix.py \
        --artifacts-root artifacts/sumeragi-soak-$(date +%Y%m%d-%H%M) \
        --pack artifacts/sumeragi-soak-$(date +%Y%m%d-%H%M)/signoff.zip

Use ``--scenario`` to override the default matrix.  Each scenario must be
specified as a comma-separated ``key=value`` list containing at least
``name`` and ``peers`` (for example:
``--scenario name=peers10_k3_r3,peers=10,collectors_k=3,redundant_send_r=3``).
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Iterable, List, Optional


DEFAULT_MATRIX: List[dict[str, int | str]] = [
    {"name": "peers4_k2_r2", "peers": 4, "collectors_k": 2, "redundant_send_r": 2},
    {"name": "peers6_k3_r2", "peers": 6, "collectors_k": 3, "redundant_send_r": 2},
    {"name": "peers8_k3_r3", "peers": 8, "collectors_k": 3, "redundant_send_r": 3},
]

SCRIPTS_DIR = Path(__file__).resolve().parent
RUN_STRESS = SCRIPTS_DIR / "run_sumeragi_stress.py"
RENDER_STRESS = SCRIPTS_DIR / "render_sumeragi_stress_report.py"

ENV_PEERS = "SUMERAGI_NPOS_STRESS_PEERS"
ENV_COLLECTORS_K = "SUMERAGI_NPOS_STRESS_COLLECTORS_K"
ENV_REDUNDANT = "SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R"


class ScenarioParseError(ValueError):
    """Raised when a custom scenario specification is invalid."""


def parse_scenario(spec: str) -> dict[str, int | str]:
    entries: dict[str, str] = {}
    for chunk in spec.split(","):
        chunk = chunk.strip()
        if not chunk:
            continue
        if "=" not in chunk:
            raise ScenarioParseError(
                f"invalid scenario fragment '{chunk}' (expected key=value)"
            )
        key, value = chunk.split("=", 1)
        entries[key.strip()] = value.strip()

    if "name" not in entries:
        raise ScenarioParseError("scenario requires a 'name' field")
    if "peers" not in entries:
        raise ScenarioParseError("scenario requires a 'peers' field")

    try:
        peers = int(entries["peers"])
    except ValueError as exc:  # pragma: no cover - defensive
        raise ScenarioParseError(f"invalid peers value: {entries['peers']}") from exc
    if peers < 4:
        raise ScenarioParseError("peer count must be at least 4")

    collectors_k = int(entries.get("collectors_k", 2))
    if collectors_k < 1:
        raise ScenarioParseError("collectors_k must be >= 1")
    redundant_send_r = int(entries.get("redundant_send_r", 2))
    if redundant_send_r < 0:
        raise ScenarioParseError("redundant_send_r must be >= 0")

    return {
        "name": entries["name"],
        "peers": peers,
        "collectors_k": collectors_k,
        "redundant_send_r": redundant_send_r,
    }


def parse_args(argv: Iterable[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--artifacts-root",
        type=Path,
        default=Path("artifacts/sumeragi-soak-matrix"),
        help="Directory where per-scenario artefacts will be written.",
    )
    parser.add_argument(
        "--scenario",
        action="append",
        help=(
            "Custom scenario specification "
            "(e.g. 'name=peers10,peers=10,collectors_k=3,redundant_send_r=3'). "
            "When provided, the default matrix is replaced."
        ),
    )
    parser.add_argument(
        "--tests",
        nargs="*",
        help=(
            "Optional subset of stress tests to run. Passed through to "
            "run_sumeragi_stress.py via '--tests'."
        ),
    )
    parser.add_argument(
        "--pack",
        type=Path,
        help="Optional path for a ZIP archive containing the resulting matrix directory.",
    )
    return parser.parse_args(list(argv))


def build_matrix(args: argparse.Namespace) -> List[dict[str, int | str]]:
    if not args.scenario:
        return list(DEFAULT_MATRIX)
    matrix: List[dict[str, int | str]] = []
    for spec in args.scenario:
        matrix.append(parse_scenario(spec))
    return matrix


def run_stress_for_scenario(
    scenario: dict[str, int | str],
    artifacts_root: Path,
    selected_tests: Optional[list[str]],
) -> dict[str, object]:
    scenario_dir = artifacts_root / str(scenario["name"])
    summary_path = scenario_dir / "summary.json"
    scenario_dir.mkdir(parents=True, exist_ok=True)

    env = os.environ.copy()
    env[ENV_PEERS] = str(scenario["peers"])
    env[ENV_COLLECTORS_K] = str(scenario["collectors_k"])
    env[ENV_REDUNDANT] = str(scenario["redundant_send_r"])

    cmd = [
        sys.executable,
        str(RUN_STRESS),
        "--artifacts",
        scenario_dir.as_posix(),
        "--summary",
        summary_path.as_posix(),
    ]
    if selected_tests:
        cmd.append("--tests")
        cmd.extend(selected_tests)

    result = subprocess.run(cmd, env=env, check=False)
    render_cmd = [
        sys.executable,
        str(RENDER_STRESS),
        "--summary",
        summary_path.as_posix(),
        "--output",
        (scenario_dir / "README.md").as_posix(),
    ]
    if summary_path.exists():
        subprocess.run(render_cmd, check=False)

    status = "pass" if result.returncode == 0 else f"fail ({result.returncode})"
    return {
        "scenario": scenario["name"],
        "peers": scenario["peers"],
        "collectors_k": scenario["collectors_k"],
        "redundant_send_r": scenario["redundant_send_r"],
        "summary": summary_path.as_posix(),
        "report": (scenario_dir / "README.md").as_posix(),
        "status": status,
        "returncode": result.returncode,
    }


def write_matrix_reports(
    artifacts_root: Path, entries: List[dict[str, object]]
) -> None:
    matrix_report = artifacts_root / "matrix_report.md"
    lines = [
        "# Sumeragi NPoS Soak Matrix",
        "",
        f"Artifacts directory: `{artifacts_root}`",
        "",
        "| Scenario | Peers | Collectors K | Redundant r | Result | Summary | Report |",
        "|----------|-------|--------------|-------------|--------|---------|--------|",
    ]
    for entry in entries:
        summary_link = (
            f"[summary]({entry['summary']})"
            if Path(str(entry["summary"])).exists()
            else "-"
        )
        report_link = (
            f"[report]({entry['report']})"
            if Path(str(entry["report"])).exists()
            else "-"
        )
        lines.append(
            "| `{scenario}` | {peers} | {collectors_k} | {redundant} | {status} | {summary} | {report} |".format(
                scenario=entry["scenario"],
                peers=entry["peers"],
                collectors_k=entry["collectors_k"],
                redundant=entry["redundant_send_r"],
                status=entry["status"],
                summary=summary_link,
                report=report_link,
            )
        )
    lines.append("")
    matrix_report.write_text("\n".join(lines), encoding="utf-8")

    (artifacts_root / "matrix_report.json").write_text(
        json.dumps(entries, indent=2), encoding="utf-8"
    )


def maybe_pack_directory(artifacts_root: Path, pack_path: Path) -> Path:
    pack_path = pack_path.with_suffix(".zip")
    base_name = pack_path.with_suffix("")
    base_name.parent.mkdir(parents=True, exist_ok=True)
    archive = shutil.make_archive(base_name.as_posix(), "zip", root_dir=artifacts_root)
    archive_path = Path(archive)
    if archive_path != pack_path:
        archive_path.replace(pack_path)
    return pack_path


def cleanup_partial_artifact_roots(artifacts_root: Path) -> None:
    """Remove leftover directories created by truncated artefact paths."""
    parent = artifacts_root.parent
    if not parent.exists():
        return

    stem = artifacts_root.name
    if "-" not in stem:
        return

    parts = stem.split("-")
    prefixes: list[str] = []
    accumulated: list[str] = []
    for part in parts[:-1]:
        accumulated.append(part)
        prefixes.append("-".join(accumulated) + "-")

    for prefix in prefixes:
        candidate = parent / prefix
        if not candidate.exists():
            continue
        if candidate.is_symlink() or candidate.is_file():
            print(
                f"[run_sumeragi_soak_matrix] Removing stray artefact path "
                f"{candidate.as_posix()}",
                file=sys.stderr,
            )
            candidate.unlink()
            continue
        if candidate.is_dir():
            print(
                f"[run_sumeragi_soak_matrix] Removing stray artefact directory "
                f"{candidate.as_posix()}",
                file=sys.stderr,
            )
            shutil.rmtree(candidate)


def main(argv: Iterable[str]) -> int:
    args = parse_args(argv)
    matrix = build_matrix(args)
    artifacts_root = args.artifacts_root
    cleanup_partial_artifact_roots(artifacts_root)
    if artifacts_root.exists():
        # Remove stale artefacts to keep each run self-contained.
        if artifacts_root.is_symlink() or artifacts_root.is_file():
            artifacts_root.unlink()
        elif artifacts_root.is_dir() and any(artifacts_root.iterdir()):
            print(
                f"[run_sumeragi_soak_matrix] Removing existing artefacts directory "
                f"{artifacts_root.as_posix()}",
                file=sys.stderr,
            )
            shutil.rmtree(artifacts_root)
    artifacts_root.mkdir(parents=True, exist_ok=True)

    results: List[dict[str, object]] = []
    return_codes: List[int] = []
    for scenario in matrix:
        print(
            f"[run_sumeragi_soak_matrix] Running scenario {scenario['name']} "
            f"(peers={scenario['peers']}, collectors_k={scenario['collectors_k']}, "
            f"redundant_send_r={scenario['redundant_send_r']})"
        )
        entry = run_stress_for_scenario(
            scenario,
            artifacts_root,
            args.tests,
        )
        results.append(entry)
        return_codes.append(int(entry["returncode"]))

    write_matrix_reports(artifacts_root, results)
    print(
        f"[run_sumeragi_soak_matrix] Wrote matrix report to "
        f"{(artifacts_root / 'matrix_report.md').as_posix()}"
    )

    if args.pack:
        archive_path = maybe_pack_directory(artifacts_root, args.pack)
        print(f"[run_sumeragi_soak_matrix] Packed sign-off archive {archive_path}")

    failures = [code for code in return_codes if code != 0]
    if failures:
        print(
            f"[run_sumeragi_soak_matrix] {len(failures)} scenario(s) failed "
            "- see reports above.",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entry
    raise SystemExit(main(sys.argv[1:]))
