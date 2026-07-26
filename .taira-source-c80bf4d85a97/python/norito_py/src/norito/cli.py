# Copyright 2024 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Command-line helpers for inspecting Norito payloads and streaming registry state."""

from __future__ import annotations

import argparse
import json
from dataclasses import asdict
from pathlib import Path
from typing import Optional

from . import header, relay_registry, telemetry_analysis


def _inspect_payload(path: Path) -> None:
    data = path.read_bytes()
    norito_header, _ = header.NoritoHeader.decode(data)
    print("Norito payload header:")
    print(f"  version: {norito_header.major}.{norito_header.minor}")
    print(f"  schema hash: {norito_header.schema_hash.hex()}")
    print(f"  compression: {norito_header.compression}")
    print(f"  payload length: {norito_header.payload_length}")
    print(f"  checksum: 0x{norito_header.checksum:016x}")
    print(f"  flags: 0x{norito_header.flags:02x}")


def _registry_register(args: argparse.Namespace) -> None:
    state_path = args.state.resolve()
    registry = relay_registry.StreamingRelayRegistry.load(state_path)
    record = registry.register_relay(
        args.relay_id,
        args.operator,
        advertised_mbps=args.advertised_mbps,
        bonded_amount=args.bond,
    )
    registry.dump(state_path)
    print(
        json.dumps(
            {
                "relay_id": record.relay_id,
                "operator": record.operator,
                "bond": record.bond,
                "reputation": record.reputation,
                "registered_at": record.registered_at,
            },
            indent=2,
        )
    )


def _registry_fund(args: argparse.Namespace) -> None:
    state_path = args.state.resolve()
    registry = relay_registry.StreamingRelayRegistry.load(state_path)
    session = registry.fund_session(
        args.session_id,
        args.relay_id,
        asset_id=args.asset_id,
        budget=args.budget,
        expected_chunks=args.expected_chunks,
    )
    registry.dump(state_path)
    print(
        json.dumps(
            {
                "session_id": args.session_id,
                "relay_id": session.relay_id,
                "asset_id": session.asset_id,
                "budget": session.budget,
                "per_chunk_reward": session.per_chunk_reward(),
            },
            indent=2,
        )
    )


def _registry_report(args: argparse.Namespace) -> None:
    state_path = args.state.resolve()
    registry = relay_registry.StreamingRelayRegistry.load(state_path)
    record = registry.record_performance(
        args.session_id,
        delivered_chunks=args.delivered,
        missed_chunks=args.missed,
        acknowledged_chunks=args.acknowledged,
        corrupted_chunks=args.corrupted,
        stale_proofs=args.stale_proofs,
        viewer_sessions=args.viewer_sessions,
        viewer_rebuffer_events=args.viewer_rebuffer_events,
        avg_latency_ms=args.avg_latency_ms,
        baseline_latency_ms=args.baseline_latency_ms,
        governance_flag=args.governance_flag,
    )
    registry.dump(state_path)
    print(
        json.dumps(
            {
                "relay_id": record.relay_id,
                "reputation": record.reputation,
                "strikes": record.strikes,
                "bond": record.bond,
                "locked_until": record.locked_until,
                "pending_rewards": record.pending_rewards,
            },
            indent=2,
        )
    )


def _registry_claim(args: argparse.Namespace) -> None:
    state_path = args.state.resolve()
    registry = relay_registry.StreamingRelayRegistry.load(state_path)
    amount = registry.claim_rewards(args.relay_id, args.asset_id)
    registry.dump(state_path)
    print(
        json.dumps(
            {
                "relay_id": args.relay_id,
                "asset_id": args.asset_id,
                "claimed_amount": amount,
            },
            indent=2,
        )
    )


def _registry_show(args: argparse.Namespace) -> None:
    state_path = args.state.resolve()
    registry = relay_registry.StreamingRelayRegistry.load(state_path)
    payload = {
        "records": {
            relay_id: {
                "operator": record.operator,
                "advertised_mbps": record.advertised_mbps,
                "bond": record.bond,
                "reputation": record.reputation,
                "strikes": record.strikes,
                "locked_until": record.locked_until,
                "pending_rewards": record.pending_rewards,
                "audit_frozen": record.audit_frozen,
            }
            for relay_id, record in registry.records.items()
        },
        "sessions": {sid: asdict(session) for sid, session in registry.sessions.items()},
    }
    print(json.dumps(payload, indent=2))


def _registry_rank(args: argparse.Namespace) -> None:
    state_path = args.state.resolve()
    registry = relay_registry.StreamingRelayRegistry.load(state_path)
    rankings = registry.rank_relays(
        min_reputation=args.min_reputation,
        include_ineligible=args.include_ineligible,
    )
    if args.limit is not None:
        rankings = rankings[: args.limit]
    print(json.dumps({"rankings": rankings}, indent=2))


def _telemetry_analyze(args: argparse.Namespace) -> None:
    baseline_config: Optional[telemetry_analysis.BaselineConfig] = None
    if args.baseline is not None:
        baseline_payload = json.loads(
            args.baseline.resolve().read_text(encoding="utf8")
        )
        baseline_config = telemetry_analysis.BaselineConfig.from_dict(
            baseline_payload,
            default_tolerance=args.tolerance,
            default_vmaf_floor=args.vmaf_floor,
        )

    analyzer = telemetry_analysis.TelemetryAnalyzer(
        baseline=baseline_config,
        default_tolerance=args.tolerance,
        default_vmaf_floor=args.vmaf_floor,
        min_samples=args.min_samples,
    )

    for line in args.path.resolve().read_text(encoding="utf8").splitlines():
        if not line.strip():
            continue
        payload = json.loads(line)
        analyzer.ingest_dict(payload)

    print(json.dumps(analyzer.build_report(), indent=2))


def main() -> None:
    parser = argparse.ArgumentParser(description="Norito developer CLI")
    subparsers = parser.add_subparsers(dest="command", required=True)

    inspect_cmd = subparsers.add_parser("inspect", help="Inspect a Norito payload header")
    inspect_cmd.add_argument("path", type=Path, help="Path to the payload file")
    inspect_cmd.set_defaults(func=lambda ns: _inspect_payload(ns.path))

    registry_cmd = subparsers.add_parser(
        "relay-registry", help="Manage streaming relay registry state"
    )
    registry_cmd.add_argument(
        "--state",
        type=Path,
        default=Path("relay_registry.json"),
        help="Path to the registry state JSON (default: relay_registry.json)",
    )
    registry_sub = registry_cmd.add_subparsers(dest="registry_command", required=True)

    register = registry_sub.add_parser("register", help="Register a new relay operator")
    register.add_argument("relay_id")
    register.add_argument("operator")
    register.add_argument("--advertised-mbps", type=float, required=True)
    register.add_argument("--bond", type=float, help="Override bonded amount (optional)")
    register.set_defaults(func=_registry_register)

    fund = registry_sub.add_parser("fund-session", help="Fund a relay session budget")
    fund.add_argument("session_id")
    fund.add_argument("relay_id")
    fund.add_argument("--asset-id", required=True)
    fund.add_argument("--budget", type=float, required=True)
    fund.add_argument("--expected-chunks", type=int, required=True)
    fund.set_defaults(func=_registry_fund)

    report = registry_sub.add_parser("report", help="Report relay performance window")
    report.add_argument("session_id")
    report.add_argument("--delivered", type=int, required=True)
    report.add_argument("--missed", type=int, required=True)
    report.add_argument("--acknowledged", type=int)
    report.add_argument("--corrupted", type=int, default=0)
    report.add_argument("--stale-proofs", action="store_true")
    report.add_argument("--viewer-sessions", type=int, default=0)
    report.add_argument("--viewer-rebuffer-events", type=int, default=0)
    report.add_argument("--avg-latency-ms", type=float, default=0.0)
    report.add_argument("--baseline-latency-ms", type=float, default=0.0)
    report.add_argument("--governance-flag", action="store_true")
    report.set_defaults(func=_registry_report)

    claim = registry_sub.add_parser("claim", help="Claim relay rewards")
    claim.add_argument("relay_id")
    claim.add_argument("--asset-id", required=True)
    claim.set_defaults(func=_registry_claim)

    show = registry_sub.add_parser("show", help="Print registry state")
    show.set_defaults(func=_registry_show)

    rank = registry_sub.add_parser(
        "rank", help="Rank relays for automated selection heuristics"
    )
    rank.add_argument("--min-reputation", type=float, default=0.3)
    rank.add_argument("--limit", type=int, help="Return at most N relays")
    rank.add_argument(
        "--include-ineligible",
        action="store_true",
        help="Keep locked or audit-frozen relays in the output",
    )
    rank.set_defaults(func=_registry_rank)

    telemetry_cmd = subparsers.add_parser(
        "telemetry", help="Analyze Norito streaming telemetry logs"
    )
    telemetry_sub = telemetry_cmd.add_subparsers(dest="telemetry_command", required=True)

    telemetry_analyze = telemetry_sub.add_parser(
        "analyze",
        help="Summarize bitrate/VMAF samples and suggest quantiser table adjustments",
    )
    telemetry_analyze.add_argument(
        "path",
        type=Path,
        help="Path to a JSONL telemetry file (one telemetry sample per line)",
    )
    telemetry_analyze.add_argument(
        "--baseline",
        type=Path,
        help="Optional JSON baseline with per-stream or per-quantiser targets",
    )
    telemetry_analyze.add_argument(
        "--tolerance",
        type=float,
        default=0.05,
        help="Relative bitrate tolerance before flagging a regression (default: 0.05)",
    )
    telemetry_analyze.add_argument(
        "--vmaf-floor",
        type=float,
        default=92.0,
        help="Minimum acceptable VMAF score (default: 92.0)",
    )
    telemetry_analyze.add_argument(
        "--min-samples",
        type=int,
        default=12,
        help="Minimum samples before emitting tuning guidance (default: 12)",
    )
    telemetry_analyze.set_defaults(func=_telemetry_analyze)

    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":  # pragma: no cover - CLI entrypoint
    main()
