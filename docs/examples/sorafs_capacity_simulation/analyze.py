#!/usr/bin/env python3
"""Aggregate SoraFS capacity simulation artefacts produced by run_cli.sh."""

from __future__ import annotations

import argparse
import json
import math
import re
from pathlib import Path
from typing import Dict, List, Tuple


def load_json(path: Path) -> Dict:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def quota_analysis(artefact_dir: Path) -> Tuple[Dict, List[str]]:
    summary_dir = artefact_dir / "quota_negotiation"
    providers = ("alpha", "beta", "gamma")
    provider_summaries = {}
    for name in providers:
        path = summary_dir / f"provider_{name}_declaration_summary.json"
        provider_summaries[name] = load_json(path)

    replication_summary = load_json(summary_dir / "replication_order_summary.json")
    assignments = replication_summary.get("assignments", [])
    allocation_map = {}
    for entry in assignments:
        provider_id = entry.get("provider_id_hex")
        allocation_map[provider_id] = entry.get("slice_gib", 0)

    allocations = []
    warnings: List[str] = []
    total_assigned = 0
    total_declared = 0

    for alias, summary in provider_summaries.items():
        provider_id = summary["provider_id_hex"]
        committed = summary["committed_capacity_gib"]
        assigned = allocation_map.get(provider_id, 0)
        remaining = max(committed - assigned, 0)
        total_assigned += assigned
        total_declared += committed
        share = assigned / committed if committed else 0.0
        allocations.append(
            {
                "provider": alias,
                "provider_id_hex": provider_id,
                "committed_gib": committed,
                "assigned_gib": assigned,
                "remaining_gib": remaining,
                "capacity_share": round(share, 4),
            }
        )
        if assigned > committed:
            warnings.append(
                f"{alias} assigned {assigned} GiB which exceeds committed capacity {committed} GiB"
            )

    target_total = sum(entry.get("slice_gib", 0) for entry in assignments)
    if not math.isclose(float(total_assigned), float(target_total)):
        warnings.append(
            f"assigned total {total_assigned} GiB does not match replication plan {target_total} GiB"
        )

    result = {
        "total_declared_gib": total_declared,
        "total_assigned_gib": total_assigned,
        "target_assigned_gib": target_total,
        "allocations": allocations,
        "warnings": warnings,
    }
    return result, warnings


def failover_analysis(artefact_dir: Path) -> Dict:
    summary_dir = artefact_dir / "failover"
    alpha_primary = load_json(summary_dir / "telemetry_alpha_primary_summary.json")
    alpha_outage = load_json(summary_dir / "telemetry_alpha_outage_summary.json")
    beta_failover = load_json(summary_dir / "telemetry_beta_failover_summary.json")

    uptime_drop = max(
        0, alpha_primary["uptime_percent_milli"] - alpha_outage["uptime_percent_milli"]
    )
    por_drop = max(
        0,
        alpha_primary["por_success_percent_milli"]
        - alpha_outage["por_success_percent_milli"],
    )
    failed_delta = max(
        0, alpha_outage["failed_replications"] - alpha_primary["failed_replications"]
    )

    failover_triggered = uptime_drop > 20000 or failed_delta >= 3
    replacement_provider = (
        "beta" if beta_failover["successful_replications"] >= alpha_primary["successful_replications"] else "alpha"
    )

    return {
        "alpha_uptime_drop_milli": uptime_drop,
        "alpha_por_drop_milli": por_drop,
        "additional_failures": failed_delta,
        "failover_triggered": failover_triggered,
        "replacement_provider": replacement_provider,
        "beta_successful_replications": beta_failover["successful_replications"],
    }


def slashing_analysis(artefact_dir: Path) -> Dict:
    summary_dir = artefact_dir / "slashing"
    dispute_summary = load_json(summary_dir / "capacity_dispute_summary.json")
    remedy = dispute_summary.get("requested_remedy", "")
    percentage = None
    percentage_match = re.search(r"Slash\s+(\d+)%", remedy)
    if percentage_match:
        percentage = int(percentage_match.group(1))
    return {
        "provider_id_hex": dispute_summary["provider_id_hex"],
        "complainant_id_hex": dispute_summary["complainant_id_hex"],
        "dispute_kind": dispute_summary["kind"],
        "requested_remedy": remedy,
        "requested_slash_percent": percentage,
        "evidence_digest_hex": dispute_summary["dispute_b64"] if "dispute_b64" in dispute_summary else None,
    }


def build_report(artefact_dir: Path) -> Dict:
    quota, warnings = quota_analysis(artefact_dir)
    failover = failover_analysis(artefact_dir)
    slashing = slashing_analysis(artefact_dir)
    return {
        "quota_negotiation": quota,
        "failover": failover,
        "slashing": slashing,
        "warnings": warnings,
    }


def write_report(report: Dict, output_path: Path) -> None:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with output_path.open("w", encoding="utf-8") as handle:
        json.dump(report, handle, indent=2, sort_keys=True)
        handle.write("\n")


def write_metrics(report: Dict, output_path: Path) -> None:
    quota = report["quota_negotiation"]
    failover = report["failover"]
    slashing = report["slashing"]

    lines: List[str] = []
    lines.append("# HELP sorafs_simulation_quota_assigned_gib Assigned GiB per provider in the quota negotiation scenario.")
    lines.append("# TYPE sorafs_simulation_quota_assigned_gib gauge")
    for entry in quota["allocations"]:
        provider = entry["provider"]
        lines.append(
            f'sorafs_simulation_quota_assigned_gib{{provider="{provider}",scenario="quota_negotiation"}} {entry["assigned_gib"]}'
        )
        lines.append(
            f'sorafs_simulation_quota_committed_gib{{provider="{provider}",scenario="quota_negotiation"}} {entry["committed_gib"]}'
        )
        lines.append(
            f'sorafs_simulation_quota_remaining_gib{{provider="{provider}",scenario="quota_negotiation"}} {entry["remaining_gib"]}'
        )
        lines.append(
            f'sorafs_simulation_quota_share{{provider="{provider}",scenario="quota_negotiation"}} {entry["capacity_share"]}'
        )

    lines.append("# HELP sorafs_simulation_quota_total_gib Totals computed for the quota negotiation scenario.")
    lines.append("# TYPE sorafs_simulation_quota_total_gib gauge")
    lines.append(
        f'sorafs_simulation_quota_total_gib{{scope="declared",scenario="quota_negotiation"}} {quota["total_declared_gib"]}'
    )
    lines.append(
        f'sorafs_simulation_quota_total_gib{{scope="assigned",scenario="quota_negotiation"}} {quota["total_assigned_gib"]}'
    )

    lines.append("# HELP sorafs_simulation_failover_uptime_drop_milli Uptime drop observed during failover.")
    lines.append("# TYPE sorafs_simulation_failover_uptime_drop_milli gauge")
    lines.append(
        f'sorafs_simulation_failover_uptime_drop_milli{{provider="alpha",scenario="failover"}} {failover["alpha_uptime_drop_milli"]}'
    )

    lines.append("# HELP sorafs_simulation_failover_additional_failures Additional failed replications when the outage occurred.")
    lines.append("# TYPE sorafs_simulation_failover_additional_failures gauge")
    lines.append(
        f'sorafs_simulation_failover_additional_failures{{provider="alpha",scenario="failover"}} {failover["additional_failures"]}'
    )

    trigger_value = 1 if failover["failover_triggered"] else 0
    lines.append("# HELP sorafs_simulation_failover_triggered Indicator for failover activation in the scenario.")
    lines.append("# TYPE sorafs_simulation_failover_triggered gauge")
    lines.append(
        f'sorafs_simulation_failover_triggered{{scenario="failover"}} {trigger_value}'
    )

    replacement = failover["replacement_provider"]
    lines.append("# HELP sorafs_simulation_failover_replacement Replacement provider selected during the failover scenario.")
    lines.append("# TYPE sorafs_simulation_failover_replacement gauge")
    lines.append(
        f'sorafs_simulation_failover_replacement{{provider="{replacement}",scenario="failover"}} 1'
    )

    lines.append("# HELP sorafs_simulation_slash_requested Requested slashing percentage derived from the dispute.")
    lines.append("# TYPE sorafs_simulation_slash_requested gauge")
    percent = slashing.get("requested_slash_percent")
    if percent is not None:
        ratio = percent / 100.0
    else:
        ratio = 0.0
    provider_hex = slashing["provider_id_hex"]
    lines.append(
        f'sorafs_simulation_slash_requested{{provider_id="{provider_hex}",scenario="slashing"}} {ratio}'
    )

    output_path.parent.mkdir(parents=True, exist_ok=True)
    with output_path.open("w", encoding="utf-8") as handle:
        handle.write("\n".join(lines))
        handle.write("\n")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--artifacts",
        default=str(Path(__file__).resolve().parent / "artifacts"),
        help="Directory containing CLI artefacts (default: %(default)s)",
    )
    parser.add_argument(
        "--report",
        default="capacity_simulation_report.json",
        help="Report filename relative to the artifact directory (default: %(default)s)",
    )
    parser.add_argument(
        "--metrics",
        default="capacity_simulation.prom",
        help="Prometheus textfile metrics relative to the artifact directory (default: %(default)s)",
    )
    args = parser.parse_args()

    artefact_dir = Path(args.artifacts).resolve()
    if not artefact_dir.exists():
        raise SystemExit(f"artifact directory {artefact_dir} does not exist; run run_cli.sh first")

    report = build_report(artefact_dir)
    write_report(report, artefact_dir / args.report)
    write_metrics(report, artefact_dir / args.metrics)

    print(f"Wrote report to {artefact_dir / args.report}")
    print(f"Wrote Prometheus metrics to {artefact_dir / args.metrics}")


if __name__ == "__main__":
    main()
