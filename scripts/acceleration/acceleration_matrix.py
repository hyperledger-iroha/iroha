#!/usr/bin/env python3
"""Aggregate acceleration-state captures into a coverage matrix.

Each input is the JSON emitted by `cargo xtask acceleration-state --format json`.
The script emits either JSON (default) or a Markdown table summarising backend
support/availability so smoke harnesses can attach deterministic evidence.

Example:
    python3 scripts/acceleration/acceleration_matrix.py \
        --state macos-m4=artifacts/acceleration_state_macos_m4.json \
        --state sim-m3=artifacts/acceleration_state_sim_m3.json \
        --format markdown \
        --output artifacts/acceleration_matrix.md
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys
from typing import Any, Dict, Iterable, List, Tuple

State = Dict[str, Any]


def _load_state(path: Path) -> State:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def _backend_view(name: str, state: State) -> Dict[str, Any]:
    return {
        "supported": bool(state.get("supported", False)),
        "configured": bool(state.get("configured", False)),
        "available": bool(state.get("available", False)),
        "parity_ok": bool(state.get("parity_ok", False)),
        "last_error": state.get("last_error"),
    }


def build_matrix(entries: List[Tuple[str, State]]) -> Dict[str, Any]:
    rows = []
    for label, state in entries:
        rows.append(
            {
                "device": label,
                "config": state.get("config", {}),
                "metal": _backend_view("metal", state.get("metal", {})),
                "cuda": _backend_view("cuda", state.get("cuda", {})),
            }
        )
    return {"devices": rows}


def render_json(matrix: Dict[str, Any]) -> str:
    return json.dumps(matrix, indent=2, sort_keys=True)


def render_markdown(matrix: Dict[str, Any]) -> str:
    lines = [
        "| device | metal.supported | metal.available | cuda.supported | cuda.available | metal.last_error | cuda.last_error |",
        "| ------ | --------------- | --------------- | -------------- | -------------- | ---------------- | --------------- |",
    ]
    for row in matrix.get("devices", []):
        metal = row.get("metal", {})
        cuda = row.get("cuda", {})
        lines.append(
            "| {device} | {m_supported} | {m_available} | {c_supported} | {c_available} | {m_error} | {c_error} |".format(
                device=row.get("device", "-"),
                m_supported=_flag(metal.get("supported", False)),
                m_available=_flag(metal.get("available", False)),
                c_supported=_flag(cuda.get("supported", False)),
                c_available=_flag(cuda.get("available", False)),
                m_error=_error(metal.get("last_error")),
                c_error=_error(cuda.get("last_error")),
            )
        )
    return "\n".join(lines) + "\n"


def _flag(value: Any) -> str:
    return "yes" if bool(value) else "no"


def _error(value: Any) -> str:
    if value is None:
        return "-"
    text = str(value).strip()
    return text if text else "-"


def parse_args(argv: Iterable[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--state",
        action="append",
        metavar="LABEL=PATH",
        required=True,
        help="Acceleration-state JSON with a device label (e.g., macos-m4=path/to/state.json). "
        "Pass multiple times to build the matrix.",
    )
    parser.add_argument(
        "--format",
        choices=["json", "markdown"],
        default="json",
        help="Output format (default: json).",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="Path to write the rendered matrix; omit for stdout.",
    )
    return parser.parse_args(argv)


def _parse_state_arguments(raw_entries: Iterable[str]) -> List[Tuple[str, Path]]:
    entries: List[Tuple[str, Path]] = []
    for raw in raw_entries:
        if "=" not in raw:
            raise ValueError(f"invalid --state entry `{raw}` (expected LABEL=PATH)")
        label, path_str = raw.split("=", 1)
        label = label.strip()
        if not label:
            raise ValueError(f"missing label in state entry `{raw}`")
        path = Path(path_str)
        entries.append((label, path))
    return entries


def main(argv: Iterable[str] | None = None) -> int:
    args = parse_args(argv if argv is not None else sys.argv[1:])
    entries = _parse_state_arguments(args.state)
    loaded: List[Tuple[str, State]] = []
    for label, path in entries:
        loaded.append((label, _load_state(path)))

    matrix = build_matrix(loaded)
    if args.format == "markdown":
        rendered = render_markdown(matrix)
    else:
        rendered = render_json(matrix)

    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered, encoding="utf-8")
    else:
        sys.stdout.write(rendered)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
