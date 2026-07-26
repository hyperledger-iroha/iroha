#!/usr/bin/env python3
"""Convert acceleration-state JSON into a Prometheus textfile.

The input shape matches `cargo xtask acceleration-state --format json`:
{
  "config": { ... },
  "metal": { "supported": true, ... },
  "cuda": { ... }
}

Example:
    python3 scripts/acceleration/export_prometheus.py \
        --input artifacts/acceleration_state.json \
        --output artifacts/acceleration_state.prom \
        --instance macos-m4
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys
from typing import Any, Dict, Iterable, Tuple

BackendLabels = Dict[str, str]


def _load_state(path: Path) -> Dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def _bool_to_int(value: Any) -> int:
    return 1 if bool(value) else 0


def _escape(value: str) -> str:
    return value.replace("\\", "\\\\").replace('"', '\\"')


def _format_metric(name: str, value: Any, labels: BackendLabels | None = None) -> str:
    label_suffix = ""
    if labels:
        encoded = ",".join(f'{key}="{_escape(str(val))}"' for key, val in sorted(labels.items()))
        label_suffix = f"{{{encoded}}}"
    return f"{name}{label_suffix} {value}"


def _emit_optional_config(
    lines: list[str],
    name: str,
    maybe_value: Any,
    labels: BackendLabels,
) -> None:
    if maybe_value is None:
        return
    lines.append(_format_metric(name, maybe_value, labels))


def render_prometheus(state: Dict[str, Any], instance: str | None = None) -> str:
    """Render the acceleration state into Prometheus text format."""
    labels: BackendLabels = {}
    if instance:
        labels["instance"] = instance

    lines: list[str] = [
        "# HELP acceleration_backend_supported Whether the backend is compiled and supported on this host",
        "# TYPE acceleration_backend_supported gauge",
        "# HELP acceleration_backend_configured Whether the backend is enabled by configuration",
        "# TYPE acceleration_backend_configured gauge",
        "# HELP acceleration_backend_available Whether the backend successfully initialised at runtime",
        "# TYPE acceleration_backend_available gauge",
        "# HELP acceleration_backend_parity_ok Whether backend self-tests passed",
        "# TYPE acceleration_backend_parity_ok gauge",
        "# HELP acceleration_config_toggle Acceleration configuration toggles (1=true, 0=false)",
        "# TYPE acceleration_config_toggle gauge",
        "# HELP acceleration_config_threshold Acceleration thresholds set via configuration",
        "# TYPE acceleration_config_threshold gauge",
        "# HELP acceleration_backend_last_error Info metric carrying last error per backend when present",
        "# TYPE acceleration_backend_last_error gauge",
    ]

    config = state.get("config", {})
    for backend_name in ("metal", "cuda"):
        cfg_value = config.get(f"enable_{backend_name}")
        if cfg_value is not None:
            lines.append(
                _format_metric(
                    "acceleration_config_toggle",
                    _bool_to_int(cfg_value),
                    {**labels, "backend": backend_name},
                )
            )

    _emit_optional_config(
        lines,
        "acceleration_config_threshold",
        config.get("max_gpus"),
        {**labels, "kind": "max_gpus"},
    )
    for field, kind in (
        ("merkle_min_leaves_gpu", "merkle_gpu"),
        ("merkle_min_leaves_metal", "merkle_metal"),
        ("merkle_min_leaves_cuda", "merkle_cuda"),
        ("prefer_cpu_sha2_max_leaves_aarch64", "cpu_sha2_aarch64"),
        ("prefer_cpu_sha2_max_leaves_x86", "cpu_sha2_x86"),
    ):
        _emit_optional_config(
            lines,
            "acceleration_config_threshold",
            config.get(field),
            {**labels, "kind": kind},
        )

    for backend_name in ("metal", "cuda"):
        backend_state = state.get(backend_name, {})
        backend_labels = {**labels, "backend": backend_name}
        lines.append(
            _format_metric(
                "acceleration_backend_supported",
                _bool_to_int(backend_state.get("supported", False)),
                backend_labels,
            )
        )
        lines.append(
            _format_metric(
                "acceleration_backend_configured",
                _bool_to_int(backend_state.get("configured", False)),
                backend_labels,
            )
        )
        lines.append(
            _format_metric(
                "acceleration_backend_available",
                _bool_to_int(backend_state.get("available", False)),
                backend_labels,
            )
        )
        lines.append(
            _format_metric(
                "acceleration_backend_parity_ok",
                _bool_to_int(backend_state.get("parity_ok", False)),
                backend_labels,
            )
        )
        last_error = backend_state.get("last_error")
        if last_error:
            lines.append(
                _format_metric(
                    "acceleration_backend_last_error",
                    1,
                    {**backend_labels, "message": str(last_error)},
                )
            )

    return "\n".join(lines) + "\n"


def parse_args(argv: Iterable[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--input",
        required=True,
        type=Path,
        help="Path to acceleration-state JSON (from `cargo xtask acceleration-state --format json`).",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="Path to write Prometheus textfile; omit for stdout.",
    )
    parser.add_argument(
        "--instance",
        type=str,
        help="Optional instance label to attach to every metric line (e.g., macos-m4 or ci-runner-01).",
    )
    return parser.parse_args(argv)


def main(argv: Iterable[str] | None = None) -> int:
    args = parse_args(argv if argv is not None else sys.argv[1:])
    state = _load_state(args.input)
    rendered = render_prometheus(state, instance=args.instance)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered, encoding="utf-8")
    else:
        sys.stdout.write(rendered)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
