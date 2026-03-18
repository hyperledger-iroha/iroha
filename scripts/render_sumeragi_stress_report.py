#!/usr/bin/env python3
"""Render a Markdown summary for Sumeragi stress test runs.

The helper consumes the JSON manifest produced by `scripts/run_sumeragi_stress.py`
and emits a small table capturing pass/fail status together with pointers to the
stdout/stderr logs for each scenario. Optionally the summary can be written to a
destination file.

Example:

    python3 scripts/render_sumeragi_stress_report.py \
        --summary artifacts/sumeragi-stress-local-single/summary.json \
        --output artifacts/sumeragi-stress-local-single/README.md
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys
from typing import Iterable


def parse_args(argv: Iterable[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--summary",
        type=Path,
        required=True,
        help="Path to the summary.json produced by run_sumeragi_stress.py",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="Optional path to write the Markdown output (defaults to stdout)",
    )
    return parser.parse_args(argv)


def load_summary(path: Path) -> list[dict[str, object]]:
    if not path.exists():
        raise FileNotFoundError(f"summary file not found: {path}")
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise ValueError(f"invalid JSON in {path}: {exc}") from exc

    if not isinstance(data, list):
        raise ValueError("summary JSON must be a list of scenario objects")
    return data


def render_markdown(summary: list[dict[str, object]], summary_path: Path) -> str:
    if not summary:
        return "# Sumeragi Stress Report\n\n_No scenarios recorded in summary._\n"

    lines: list[str] = [
        "# Sumeragi Stress Report",
        "",
        f"Source summary: `{summary_path}`",
        "",
        "| Scenario | Result | Stdout log | Stderr log |",
        "|----------|--------|------------|------------|",
    ]
    for entry in summary:
        test = str(entry.get("test", "unknown"))
        rc = entry.get("returncode")
        result = "pass" if rc == 0 else f"fail ({rc})"
        stdout = entry.get("stdout")
        stderr = entry.get("stderr")
        stdout_cell = f"[log]({stdout})" if stdout else "-"
        stderr_cell = f"[log]({stderr})" if stderr else "-"
        lines.append(f"| `{test}` | {result} | {stdout_cell} | {stderr_cell} |")
    lines.append("")
    return "\n".join(lines)


def main(argv: Iterable[str]) -> int:
    args = parse_args(argv)
    summary = load_summary(args.summary)
    markdown = render_markdown(summary, args.summary)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(markdown, encoding="utf-8")
    else:
        print(markdown, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
