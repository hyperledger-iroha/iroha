#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""
Generate Prometheus-style metrics for SNS bulk registrar releases.

The metrics aggregate a manifest (RegisterNameRequestV1 list) and the structured
submission log emitted by `scripts/sns_bulk_onboard.py --submission-log`.
"""

from __future__ import annotations

import argparse
import json
import os
from collections import defaultdict
from pathlib import Path
from typing import Dict, List, Tuple


def _parse_amount(value: object) -> int:
    """Convert decimal or hex strings to integers; fall back to zero."""
    if value is None:
        return 0
    if isinstance(value, (int, float)):
        return int(value)
    text = str(value).strip()
    if not text:
        return 0
    if text.startswith(("0x", "0X")):
        return int(text, 16)
    return int(text, 10)


def render_metrics(manifest_path: Path, submission_log: Path, release: str) -> str:
    manifest = {}
    if manifest_path.exists():
        manifest = json.loads(manifest_path.read_text())
    requests: List[Dict[str, object]] = manifest.get("requests", []) or []

    suffix_counts: Dict[str, int] = defaultdict(int)
    gross_totals: Dict[str, int] = defaultdict(int)
    net_totals: Dict[str, int] = defaultdict(int)

    for req in requests:
        selector = req.get("selector") or {}
        suffix_id = str(selector.get("suffix_id", "unknown"))
        suffix_counts[suffix_id] += 1

        payment = req.get("payment") or {}
        asset = payment.get("asset_id")
        if asset:
            gross_totals[asset] += _parse_amount(payment.get("gross_amount"))
            net_totals[asset] += _parse_amount(payment.get("net_amount"))

    total_requests = len(requests)

    submission_counts: Dict[Tuple[str, str], int] = defaultdict(int)
    if submission_log.exists():
        for line in submission_log.read_text().splitlines():
            line = line.strip()
            if not line:
                continue
            try:
                entry = json.loads(line)
            except json.JSONDecodeError:
                continue
            mode = str(entry.get("mode", "unknown"))
            success = "true" if entry.get("success") else "false"
            submission_counts[(mode, success)] += 1

    lines: List[str] = [
        "# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.",
        "# TYPE sns_bulk_release_requests_total gauge",
        f'sns_bulk_release_requests_total{{release="{release}",suffix_id="all"}} {total_requests}',
    ]

    for suffix_id in sorted(suffix_counts):
        value = suffix_counts[suffix_id]
        lines.append(
            f'sns_bulk_release_requests_total{{release="{release}",suffix_id="{suffix_id}"}} {value}'
        )

    lines.extend(
        [
            "# HELP sns_bulk_release_payment_gross_units Sum of gross payment units per asset.",
            "# TYPE sns_bulk_release_payment_gross_units gauge",
        ]
    )
    for asset_id in sorted(gross_totals):
        gross_value = gross_totals[asset_id]
        lines.append(
            f'sns_bulk_release_payment_gross_units{{release="{release}",asset_id="{asset_id}"}} {gross_value}'
        )

    lines.extend(
        [
            "# HELP sns_bulk_release_payment_net_units Sum of net payment units per asset.",
            "# TYPE sns_bulk_release_payment_net_units gauge",
        ]
    )
    for asset_id in sorted(net_totals):
        net_value = net_totals[asset_id]
        lines.append(
            f'sns_bulk_release_payment_net_units{{release="{release}",asset_id="{asset_id}"}} {net_value}'
        )

    lines.extend(
        [
            "# HELP sns_bulk_release_submission_events_total Submission events grouped by mode and success.",
            "# TYPE sns_bulk_release_submission_events_total counter",
        ]
    )
    for (mode, success), count in sorted(submission_counts.items()):
        lines.append(
            f'sns_bulk_release_submission_events_total{{release="{release}",mode="{mode}",success="{success}"}} {count}'
        )

    lines.append("")
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True, help="Manifest JSON path")
    parser.add_argument(
        "--submission-log", type=Path, required=True, help="Submission log (NDJSON)"
    )
    parser.add_argument("--release", required=True, help="Release identifier label")
    parser.add_argument("--output", type=Path, required=True, help="Output metrics file")
    args = parser.parse_args()

    metrics = render_metrics(args.manifest, args.submission_log, args.release)
    args.output.write_text(metrics)


if __name__ == "__main__":
    main()
