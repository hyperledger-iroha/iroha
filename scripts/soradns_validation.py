#!/usr/bin/env python3
"""
Validation helper for the SoraDNS resolver plan (SNNet-15B).

The utility provides two subcommands:
- ``edns-matrix`` runs a small EDNS capability sweep against a resolver.
- ``ds-validate`` checks sample zone descriptors for DS/DNSKEY coverage.
"""

from __future__ import annotations

import argparse
import json
import shutil
import string
import subprocess
import sys
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Callable, Iterable


DS_DIGEST_HEX_LENGTHS = {
    # digest_type => hex length
    1: 40,  # SHA-1
    2: 64,  # SHA-256
    4: 96,  # SHA-384
}


CommandRunner = Callable[[list[str]], subprocess.CompletedProcess[str]]


@dataclass
class ProbeResult:
    bufsize: int
    status: str
    truncated: bool
    udp_size: int | None
    query_time_ms: int | None
    resolver: str
    qname: str
    record_type: str
    tool: str
    return_code: int
    stderr: str


def choose_dns_tool(preferred: str | None) -> str:
    """Select the DNS client to run."""
    candidates = [preferred] if preferred else []
    candidates.extend(["kdig", "dig"])
    for tool in candidates:
        if tool and shutil.which(tool):
            return tool
    raise RuntimeError("No DNS client found (tried kdig, dig)")


def _parse_status(line: str) -> str | None:
    if "status:" not in line.lower():
        return None
    try:
        return line.split("status:")[1].split(",")[0].strip()
    except IndexError:
        return None


def _parse_flags(line: str) -> bool:
    if "flags:" not in line.lower():
        return False
    flags = line.split("flags:", 1)[1].split(";")[0].lower()
    return " tc" in f" {flags} "


def _parse_udp_size(line: str) -> int | None:
    if "udp:" not in line.lower():
        return None
    chunk = line.split("udp:", 1)[1].strip()
    digits = "".join(ch for ch in chunk if ch.isdigit())
    return int(digits) if digits else None


def _parse_query_time(line: str) -> int | None:
    if "query time" not in line.lower():
        return None
    parts = line.lower().split("query time:")
    if len(parts) < 2:
        return None
    fragment = parts[1].strip()
    digits = "".join(ch for ch in fragment if ch.isdigit())
    return int(digits) if digits else None


def parse_dns_output(output: str) -> dict[str, int | str | bool | None]:
    """Extract status flags from dig/kdig output."""
    status = "UNKNOWN"
    truncated = False
    udp_size: int | None = None
    query_time_ms: int | None = None

    for line in output.splitlines():
        status_value = _parse_status(line)
        if status_value:
            status = status_value
        truncated = truncated or _parse_flags(line)
        udp = _parse_udp_size(line)
        udp_size = udp_size if udp_size is not None else udp
        qtime = _parse_query_time(line)
        query_time_ms = query_time_ms if query_time_ms is not None else qtime

    return {
        "status": status,
        "truncated": truncated,
        "udp_size": udp_size,
        "query_time_ms": query_time_ms,
    }


def run_command(cmd: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        cmd,
        capture_output=True,
        text=True,
        check=False,
    )


def probe_edns(
    *,
    resolver: str,
    qname: str,
    record_type: str,
    bufsize: int,
    tool: str,
    command_runner: CommandRunner = run_command,
) -> ProbeResult:
    """Run a single EDNS probe."""
    base = [tool, f"@{resolver}", qname, record_type, "+dnssec", f"+bufsize={bufsize}"]
    if tool.endswith("dig"):
        base.append("+noednsnegotiation")
    if tool.endswith("kdig"):
        base.extend(["+nocookie", "+notcp"])
    result = command_runner(base)
    parsed = parse_dns_output(result.stdout)
    return ProbeResult(
        bufsize=bufsize,
        status=str(parsed.get("status", "UNKNOWN")),
        truncated=bool(parsed.get("truncated")),
        udp_size=parsed.get("udp_size"),  # type: ignore[arg-type]
        query_time_ms=parsed.get("query_time_ms"),  # type: ignore[arg-type]
        resolver=resolver,
        qname=qname,
        record_type=record_type,
        tool=tool,
        return_code=result.returncode,
        stderr=result.stderr.strip(),
    )


def collect_edns_matrix(
    *,
    resolver: str,
    qname: str,
    record_type: str,
    bufsizes: Iterable[int],
    tool: str,
    command_runner: CommandRunner = run_command,
) -> list[dict[str, object]]:
    """Run probes for multiple buffer sizes."""
    results: list[dict[str, object]] = []
    for bufsize in bufsizes:
        probe = probe_edns(
            resolver=resolver,
            qname=qname,
            record_type=record_type,
            bufsize=bufsize,
            tool=tool,
            command_runner=command_runner,
        )
        results.append(asdict(probe))
    return results


def load_zones(path: Path) -> list[dict[str, object]]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    zones = payload.get("zones", []) if isinstance(payload, dict) else payload
    if not isinstance(zones, list):
        raise ValueError(f"{path} does not contain a zone list")
    return zones  # type: ignore[return-value]


def _is_hex_string(value: str) -> bool:
    return all(ch in string.hexdigits for ch in value)


def validate_ds_records(zones: list[dict[str, object]]) -> list[dict[str, object]]:
    reports: list[dict[str, object]] = []
    for zone in zones:
        zone_name = str(zone.get("zone", "<unknown>"))
        ds_records = zone.get("ds", [])
        dnskeys = zone.get("dnskey", [])
        ds_tags = {int(entry.get("key_tag", -1)) for entry in ds_records if isinstance(entry, dict)}
        dnskey_tags = {int(entry.get("key_tag", -1)) for entry in dnskeys if isinstance(entry, dict)}
        missing_dnskeys = [
            entry
            for entry in ds_records
            if isinstance(entry, dict) and int(entry.get("key_tag", -1)) not in dnskey_tags
        ]
        missing_ds = [
            entry
            for entry in dnskeys
            if isinstance(entry, dict)
            and int(entry.get("flags", 0)) & 0x0001
            and int(entry.get("key_tag", -1)) not in ds_tags
        ]

        invalid_digests: list[dict[str, object]] = []
        for entry in ds_records:
            if not isinstance(entry, dict):
                continue
            digest = str(entry.get("digest", ""))
            digest_type = int(entry.get("digest_type", 0))
            expected_length = DS_DIGEST_HEX_LENGTHS.get(digest_type)
            if expected_length and len(digest) != expected_length:
                invalid_digests.append(
                    {
                        "key_tag": entry.get("key_tag"),
                        "digest_type": digest_type,
                        "reason": f"expected {expected_length} hex chars, found {len(digest)}",
                    }
                )
            elif digest and not _is_hex_string(digest):
                invalid_digests.append(
                    {
                        "key_tag": entry.get("key_tag"),
                        "digest_type": digest_type,
                        "reason": "digest contains non-hex characters",
                    }
                )

        state = "ok" if not (missing_dnskeys or missing_ds or invalid_digests) else "warn"
        reports.append(
            {
                "zone": zone_name,
                "state": state,
                "missing_dnskeys": missing_dnskeys,
                "missing_ds": missing_ds,
                "invalid_digests": invalid_digests,
                "notes": zone.get("notes", []),
            }
        )
    return reports


def emit_json(data: object, destination: Path | None) -> None:
    if destination:
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(json.dumps(data, indent=2), encoding="utf-8")
    else:
        json.dump(data, sys.stdout, indent=2)
        sys.stdout.write("\n")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="SoraDNS validation helper.")
    sub = parser.add_subparsers(dest="command", required=True)

    edns = sub.add_parser("edns-matrix", help="Run EDNS buffer-size probes.")
    edns.add_argument("--resolver", default="127.0.0.1", help="Resolver address (default: 127.0.0.1)")
    edns.add_argument("--qname", default="docs-preview.sora.link.", help="Query name for probes.")
    edns.add_argument("--type", dest="record_type", default="A", help="Record type to query.")
    edns.add_argument(
        "--bufsize",
        type=int,
        nargs="+",
        default=[512, 1232, 4096],
        help="List of EDNS buffer sizes to test.",
    )
    edns.add_argument("--tool", help="DNS client to run (defaults to kdig then dig).")
    edns.add_argument("--out", type=Path, help="Output path for JSON results.")

    ds = sub.add_parser("ds-validate", help="Validate DS/DNSKEY coverage for sample zones.")
    ds.add_argument(
        "--zones",
        type=Path,
        default=Path("fixtures/soradns/managed_zones.sample.json"),
        help="Path to the zones fixture JSON.",
    )
    ds.add_argument("--out", type=Path, help="Output path for JSON results.")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)

    try:
        if args.command == "edns-matrix":
            tool = choose_dns_tool(args.tool)
            results = collect_edns_matrix(
                resolver=args.resolver,
                qname=args.qname,
                record_type=args.record_type,
                bufsizes=args.bufsize,
                tool=tool,
            )
        elif args.command == "ds-validate":
            zones = load_zones(args.zones)
            results = validate_ds_records(zones)
        else:
            parser.error("unknown command")
            return 1
    except Exception as exc:  # pragma: no cover - CLI guard
        sys.stderr.write(f"error: {exc}\n")
        return 1

    emit_json(results, args.out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
