#!/usr/bin/env python3
"""Rank Norito streaming relays and derive incentive weights.

This helper consumes the persisted provider scoreboard emitted by
`sorafs_cli fetch --orchestrator-config …` (or any component wiring the
`Scoreboard::persist_to_path` helper) and optionally augments the
ranking with live telemetry exported by the streaming orchestrator. The
output highlights the relays that satisfy the reliability threshold,
their blended trust scores, and the proportional reward shares required
to keep incentives aligned with observed usage.

The script is intentionally dependency-free so it can run inside CI or
air‑gapped operator environments.
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Tuple


DEFAULT_MIN_SCORE = 0.35


@dataclass
class TelemetrySample:
    provider_id: str
    delivered_bytes: float
    uptime_ratio: float
    failure_ratio: float
    tickets_valid: float
    tickets_invalid: float

    @property
    def reliability(self) -> float:
        uptime = max(0.0, min(1.0, self.uptime_ratio))
        failure = max(0.0, min(1.0, self.failure_ratio))
        # High uptime and low failure are rewarded equally.
        return uptime * (1.0 - failure)

    @property
    def ticket_penalty(self) -> float:
        issued = self.tickets_valid + self.tickets_invalid
        if issued <= 0.0:
            return 1.0
        invalid_ratio = max(0.0, min(1.0, self.tickets_invalid / issued))
        # Penalise quadraticly so recurring invalid tickets stand out.
        return 1.0 - math.pow(invalid_ratio, 2.0)


@dataclass
class RankedRelay:
    provider_id: str
    base_weight: float
    telemetry: Optional[TelemetrySample]
    blended_score: float
    reward_share: float


def load_scoreboard(path: Path) -> Dict[str, float]:
    try:
        payload = json.loads(path.read_text())
    except json.JSONDecodeError as err:
        raise SystemExit(f"Failed to parse scoreboard JSON: {err}") from err

    entries = payload.get("entries")
    if not isinstance(entries, list):
        raise SystemExit("scoreboard JSON missing `entries` array")

    result: Dict[str, float] = {}
    for entry in entries:
        if not isinstance(entry, dict):
            continue
        provider = entry.get("provider_id")
        weight = entry.get("normalised_weight")
        if not isinstance(provider, str):
            continue
        if not isinstance(weight, (int, float)):
            continue
        result[provider] = float(weight)
    return result


def load_metrics(path: Optional[Path]) -> Dict[str, TelemetrySample]:
    if path is None:
        return {}
    samples: Dict[str, TelemetrySample] = {}
    with path.open(newline="", encoding="utf-8") as handle:
        reader = csv.DictReader(handle)
        required = {
            "provider_id",
            "delivered_bytes",
            "uptime_ratio",
            "failure_ratio",
            "tickets_valid",
            "tickets_invalid",
        }
        if reader.fieldnames is None or not required.issubset(reader.fieldnames):
            raise SystemExit(
                "metrics CSV must contain columns: " + ", ".join(sorted(required))
            )
        for row in reader:
            provider = row.get("provider_id", "").strip()
            if not provider:
                continue
            try:
                sample = TelemetrySample(
                    provider_id=provider,
                    delivered_bytes=float(row["delivered_bytes"] or 0.0),
                    uptime_ratio=float(row["uptime_ratio"] or 0.0),
                    failure_ratio=float(row["failure_ratio"] or 0.0),
                    tickets_valid=float(row["tickets_valid"] or 0.0),
                    tickets_invalid=float(row["tickets_invalid"] or 0.0),
                )
            except (TypeError, ValueError) as err:
                raise SystemExit(
                    f"invalid numeric telemetry entry for provider '{provider}': {err}"
                ) from err
            samples[provider] = sample
    return samples


def blend_scores(
    weights: Dict[str, float],
    telemetry: Dict[str, TelemetrySample],
    min_score: float,
    top: Optional[int],
) -> List[RankedRelay]:
    if not weights:
        return []

    usage_total = sum(sample.delivered_bytes for sample in telemetry.values()) or 1.0

    blended: List[Tuple[str, float, Optional[TelemetrySample], float]] = []
    for provider, weight in weights.items():
        telem = telemetry.get(provider)
        usage_factor = (
            (telem.delivered_bytes / usage_total) if telem is not None else 0.0
        )
        reliability = telem.reliability if telem is not None else 0.75
        ticket_penalty = telem.ticket_penalty if telem is not None else 1.0

        # The reputational multiplier is conservative: 40% from base weight,
        # 40% from reliability, and 20% from observed usage share.
        blended_score = (
            0.4 * normalized(weight, weights.values())
            + 0.4 * reliability
            + 0.2 * usage_factor
        ) * ticket_penalty

        blended.append((provider, weight, telem, blended_score))

    # Filter unreliable relays and compute reward shares.
    candidates = [item for item in blended if item[3] >= min_score]
    if not candidates:
        candidates = blended  # fallback: keep top relays even if below threshold

    candidates.sort(key=lambda item: item[3], reverse=True)
    if top is not None:
        candidates = candidates[:top]

    total_score = sum(item[3] for item in candidates) or 1.0
    ranked: List[RankedRelay] = []
    for provider, weight, telem, score in candidates:
        ranked.append(
            RankedRelay(
                provider_id=provider,
                base_weight=weight,
                telemetry=telem,
                blended_score=score,
                reward_share=score / total_score,
            )
        )
    return ranked


def normalized(value: float, universe: Iterable[float]) -> float:
    span = max(universe) - min(universe)
    if span <= 0.0:
        return 1.0 if value > 0.0 else 0.0
    minimum = min(universe)
    return (value - minimum) / span


def emit_report(ranked: List[RankedRelay], json_out: Optional[Path]) -> None:
    rows = []
    for item in ranked:
        telemetry = item.telemetry
        rows.append(
            {
                "provider_id": item.provider_id,
                "base_weight": item.base_weight,
                "blended_score": round(item.blended_score, 6),
                "reward_share": round(item.reward_share, 6),
                "delivered_bytes": telemetry.delivered_bytes if telemetry else None,
                "uptime_ratio": telemetry.uptime_ratio if telemetry else None,
                "failure_ratio": telemetry.failure_ratio if telemetry else None,
                "tickets_valid": telemetry.tickets_valid if telemetry else None,
                "tickets_invalid": telemetry.tickets_invalid if telemetry else None,
            }
        )

    if json_out is not None:
        json_out.write_text(json.dumps({"ranked_relays": rows}, indent=2) + "\n")

    print("Ranked relays (ordered by blended score):")
    col_headers = (
        "provider_id",
        "score",
        "reward_share",
        "delivered_bytes",
        "uptime",
        "failure",
        "invalid_tickets",
    )
    print(" | ".join(h.center(16) for h in col_headers))
    print("-" * (len(col_headers) * 18))
    for row in rows:
        print(
            f"{row['provider_id']:16} | "
            f"{row['blended_score']:>16.6f} | "
            f"{row['reward_share']:>16.6f} | "
            f"{(row['delivered_bytes'] or 0):>16.0f} | "
            f"{(row['uptime_ratio'] or 0):>16.4f} | "
            f"{(row['failure_ratio'] or 0):>16.4f} | "
            f"{(row['tickets_invalid'] or 0):>16.2f}"
        )


def parse_args(arguments: Optional[List[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--scoreboard",
        required=True,
        type=Path,
        help="Path to the persisted scoreboard JSON produced by the orchestrator",
    )
    parser.add_argument(
        "--metrics",
        type=Path,
        default=None,
        help="Optional CSV file with telemetry columns (see script header)",
    )
    parser.add_argument(
        "--min-score",
        type=float,
        default=DEFAULT_MIN_SCORE,
        help="Minimum blended score required to keep a relay in the final ranking",
    )
    parser.add_argument(
        "--top",
        type=int,
        default=None,
        help="Limit the output to the best N relays after filtering",
    )
    parser.add_argument(
        "--json-out",
        type=Path,
        default=None,
        help="Optional path to emit the JSON report (defaults to stdout only)",
    )
    return parser.parse_args(arguments)


def main(args: Optional[List[str]] = None) -> int:
    opts = parse_args(args)
    scoreboard = load_scoreboard(opts.scoreboard)
    telemetry = load_metrics(opts.metrics)
    ranked = blend_scores(scoreboard, telemetry, opts.min_score, opts.top)
    if not ranked:
        print("No relays satisfied the selection criteria", file=sys.stderr)
        return 1
    emit_report(ranked, opts.json_out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
