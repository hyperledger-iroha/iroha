#!/usr/bin/env python3
"""Compare FASTPQ row-usage snapshots and fail if transfer rows regress.

The CLI output of `iroha_cli audit witness --decode` (FASTPQ batches are
enabled by default; pass `--no-fastpq-batches` to suppress them) contains a
`fastpq_batches` array whose entries include a `row_usage` object. This helper
loads two such JSON blobs (typically a baseline and a transfer-gadget
candidate) and ensures that transfer ratios and total row counts do not exceed
the configured tolerances.
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path
from typing import Dict, Iterable, List, Tuple


RowUsage = Dict[str, int]
BatchMap = Dict[str, RowUsage]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Compare FASTPQ row_usage outputs produced by `iroha_cli audit witness --decode` "
            "(the batches are enabled by default; use --no-fastpq-batches to disable them)."
        )
    )
    parser.add_argument(
        "--baseline",
        required=True,
        type=Path,
        help="Path to the baseline/expected row_usage JSON.",
    )
    parser.add_argument(
        "--candidate",
        required=True,
        type=Path,
        help="Path to the transfer-gadget row_usage JSON.",
    )
    parser.add_argument(
        "--max-transfer-ratio-increase",
        type=float,
        default=0.0,
        help=(
            "Maximum allowed increase in transfer_ratio (absolute difference). "
            "Default: 0 (no regression allowed)."
        ),
    )
    parser.add_argument(
        "--max-total-rows-increase",
        type=int,
        default=0,
        help=(
            "Maximum allowed increase in total_rows compared to the baseline. "
            "Default: 0 (candidate must not exceed baseline row counts)."
        ),
    )
    parser.add_argument(
        "--quiet",
        action="store_true",
        help="Suppress the per-batch summary; only emit failures.",
    )
    parser.add_argument(
        "--summary-out",
        type=Path,
        help=(
            "Optional path to write a JSON summary (per-batch ratios/row deltas and aggregate)."
        ),
    )
    return parser.parse_args()


def load_json(path: Path):
    try:
        with path.open("r", encoding="utf-8") as handle:
            return json.load(handle)
    except FileNotFoundError as err:
        raise SystemExit(f"[error] file not found: {path}") from err
    except json.JSONDecodeError as err:
        raise SystemExit(f"[error] invalid JSON in {path}: {err}") from err


def gather_batches(blob) -> List[dict]:
    """Return every dict that looks like a transition batch."""

    stack: List[object] = [blob]
    batches: List[dict] = []
    while stack:
        current = stack.pop()
        if isinstance(current, dict):
            if "row_usage" in current and (
                "parameter" in current or "metadata" in current
            ):
                batches.append(current)
                continue
            stack.extend(current.values())
        elif isinstance(current, list):
            stack.extend(current)
    if not batches:
        raise SystemExit(
            "[error] no FASTPQ batches with row_usage were found in the provided blob"
        )
    return batches


def key_for_batch(batch: dict, index: int, seen: Dict[str, int]) -> str:
    key = batch.get("entry_hash") or f"batch:{index}"
    if key in seen:
        seen[key] += 1
        key = f"{key}#{seen[key]}"
    else:
        seen[key] = 0
    return key


def build_row_usage_map(blob) -> BatchMap:
    batches = gather_batches(blob)
    seen: Dict[str, int] = {}
    mapping: BatchMap = {}
    for idx, batch in enumerate(batches):
        key = key_for_batch(batch, idx, seen)
        usage = batch.get("row_usage")
        if not isinstance(usage, dict):
            raise SystemExit(f"[error] batch {key} missing row_usage object")
        mapping[key] = usage
    return mapping


def ratio(usage: RowUsage) -> float:
    total = max(int(usage.get("total_rows", 0)), 0)
    transfers = max(int(usage.get("transfer_rows", 0)), 0)
    if total == 0:
        return 0.0
    return transfers / total


def summary_line(name: str, baseline: RowUsage, candidate: RowUsage) -> str:
    base_ratio = ratio(baseline)
    cand_ratio = ratio(candidate)
    base_total = int(baseline.get("total_rows", 0))
    cand_total = int(candidate.get("total_rows", 0))
    delta_ratio = cand_ratio - base_ratio
    delta_rows = cand_total - base_total
    return (
        f"{name}: transfer_ratio {cand_ratio:.4f} "
        f"(baseline {base_ratio:.4f}, Δ {delta_ratio:+.4f}); "
        f"total_rows {cand_total} (baseline {base_total}, Δ {delta_rows:+d})"
    )


def aggregate(usages: Iterable[RowUsage]) -> RowUsage:
    agg: RowUsage = {
        "total_rows": 0,
        "transfer_rows": 0,
        "mint_rows": 0,
        "burn_rows": 0,
        "role_grant_rows": 0,
        "role_revoke_rows": 0,
        "meta_set_rows": 0,
        "permission_rows": 0,
    }
    for usage in usages:
        for key in agg:
            agg[key] += int(usage.get(key, 0))
    return agg


def ensure_same_entries(
    baseline: BatchMap, candidate: BatchMap
) -> Tuple[set, set]:
    base_keys = set(baseline)
    cand_keys = set(candidate)
    missing = base_keys - cand_keys
    extra = cand_keys - base_keys
    return missing, extra


def main() -> None:
    args = parse_args()
    baseline = build_row_usage_map(load_json(args.baseline))
    candidate = build_row_usage_map(load_json(args.candidate))

    missing, extra = ensure_same_entries(baseline, candidate)
    if missing:
        raise SystemExit(
            "[error] candidate row_usage is missing entries: "
            + ", ".join(sorted(missing))
        )
    if extra:
        raise SystemExit(
            "[error] candidate produced unexpected entries: "
            + ", ".join(sorted(extra))
        )

    failures: List[str] = []
    summary_rows: List[Dict[str, object]] = []
    for key in sorted(baseline):
        base_usage = baseline[key]
        cand_usage = candidate[key]
        base_ratio = ratio(base_usage)
        cand_ratio = ratio(cand_usage)
        total_delta = int(cand_usage.get("total_rows", 0)) - int(
            base_usage.get("total_rows", 0)
        )
        ratio_delta = cand_ratio - base_ratio

        summary_rows.append(
            {
                "batch": key,
                "baseline": {
                    "transfer_ratio": base_ratio,
                    "total_rows": int(base_usage.get("total_rows", 0)),
                    "transfer_rows": int(base_usage.get("transfer_rows", 0)),
                },
                "candidate": {
                    "transfer_ratio": cand_ratio,
                    "total_rows": int(cand_usage.get("total_rows", 0)),
                    "transfer_rows": int(cand_usage.get("transfer_rows", 0)),
                },
                "delta": {
                    "transfer_ratio": ratio_delta,
                    "total_rows": total_delta,
                },
            }
        )

        if ratio_delta > args.max_transfer_ratio_increase + 1e-9:
            failures.append(
                f"{key}: transfer_ratio regression {ratio_delta:+.4f} exceeds "
                f"limit {args.max_transfer_ratio_increase:+.4f}"
            )
        if total_delta > args.max_total_rows_increase:
            failures.append(
                f"{key}: total_rows regression {total_delta:+d} exceeds "
                f"limit {args.max_total_rows_increase:+d}"
            )

        if not args.quiet:
            print(summary_line(key, base_usage, cand_usage))

    base_agg = aggregate(baseline.values())
    cand_agg = aggregate(candidate.values())
    agg_ratio_delta = ratio(cand_agg) - ratio(base_agg)
    agg_total_delta = int(cand_agg["total_rows"]) - int(base_agg["total_rows"])
    if agg_ratio_delta > args.max_transfer_ratio_increase + 1e-9:
        failures.append(
            f"aggregate: transfer_ratio regression {agg_ratio_delta:+.4f} exceeds "
            f"limit {args.max_transfer_ratio_increase:+.4f}"
        )
    if agg_total_delta > args.max_total_rows_increase:
        failures.append(
            f"aggregate: total_rows regression {agg_total_delta:+d} exceeds "
            f"limit {args.max_total_rows_increase:+d}"
        )
    if not args.quiet:
        print(summary_line("aggregate", base_agg, cand_agg))

    if args.summary_out:
        args.summary_out.parent.mkdir(parents=True, exist_ok=True)
        summary_payload = {
            "baseline": str(args.baseline),
            "candidate": str(args.candidate),
            "max_transfer_ratio_increase": args.max_transfer_ratio_increase,
            "max_total_rows_increase": args.max_total_rows_increase,
            "batches": summary_rows,
            "aggregate": {
                "baseline": {
                    "transfer_ratio": ratio(base_agg),
                    "total_rows": int(base_agg.get("total_rows", 0)),
                    "transfer_rows": int(base_agg.get("transfer_rows", 0)),
                },
                "candidate": {
                    "transfer_ratio": ratio(cand_agg),
                    "total_rows": int(cand_agg.get("total_rows", 0)),
                    "transfer_rows": int(cand_agg.get("transfer_rows", 0)),
                },
                "delta": {
                    "transfer_ratio": agg_ratio_delta,
                    "total_rows": agg_total_delta,
                },
            },
        }
        args.summary_out.write_text(json.dumps(summary_payload, indent=2) + "\n", encoding="utf-8")

    if failures:
        print("", file=sys.stderr)
        for failure in failures:
            print(f"[error] {failure}", file=sys.stderr)
        raise SystemExit(1)


if __name__ == "__main__":
    main()
