#!/usr/bin/env python3
"""
Generate synthetic Android telemetry load for staging drills.

The script emits deterministic, privacy-safe payloads that mimic the Android SDK
exporter and POSTs them to the configured endpoint. It is designed to seed the
`android-telemetry-stg` cluster ahead of chaos rehearsals so queue replay,
redaction alerts, and override workflows have active traffic to inspect.

Example usage::

    # Five minutes of traffic against android-telemetry-stg (default path)
    scripts/telemetry/generate_android_load.py --cluster android-telemetry-stg

    # Twenty-minute run with custom rate, headers, and dry-run logging
    scripts/telemetry/generate_android_load.py \
        --cluster android-telemetry-stg \
        --duration 20m \
        --rps 8 \
        --header "Authorization=Bearer token" \
        --dry-run
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import random
import sys
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, Iterable, List, Optional

DEFAULT_LOG_PATH = Path("artifacts/android/telemetry/load-generator.log")
DEFAULT_DURATION = "5m"
DEFAULT_RPS = 4.0
DEFAULT_PATH = "/api/android/telemetry/mock_ingest"
DEFAULT_DEVICE_PROFILES = ("emulator", "consumer", "enterprise")
DEFAULT_EVENTS = (
    "queue_replay",
    "redaction_probe",
    "override_sample",
    "attestation_check",
)


@dataclass
class LoadResult:
    attempts: int = 0
    successes: int = 0
    failures: int = 0

    def record_success(self) -> None:
        self.attempts += 1
        self.successes += 1

    def record_failure(self) -> None:
        self.attempts += 1
        self.failures += 1


def parse_duration(value: str) -> float:
    """Parse duration strings like 30s, 5m, 2h into seconds."""
    if not value:
        raise argparse.ArgumentTypeError("duration must be non-empty")
    suffix = value[-1].lower()
    number_str = value[:-1] if suffix in {"s", "m", "h"} else value
    unit = suffix if suffix in {"s", "m", "h"} else "s"
    try:
        amount = float(number_str)
    except ValueError as exc:
        raise argparse.ArgumentTypeError(
            f"invalid duration '{value}'"
        ) from exc
    multiplier = {"s": 1.0, "m": 60.0, "h": 3600.0}[unit]
    return amount * multiplier


def parse_headers(raw_headers: Iterable[str]) -> Dict[str, str]:
    headers: Dict[str, str] = {"Content-Type": "application/json"}
    for entry in raw_headers:
        if "=" not in entry:
            raise argparse.ArgumentTypeError(
                f"header '{entry}' must use KEY=VALUE format"
            )
        key, value = entry.split("=", 1)
        headers[key.strip()] = value.strip()
    return headers


def resolve_endpoint(cluster: Optional[str], endpoint: Optional[str], path: str) -> str:
    if endpoint:
        return endpoint
    if not cluster:
        raise SystemExit(
            "[error] --cluster or --endpoint must be supplied to target a service."
        )
    base = cluster if "://" in cluster else f"https://{cluster}"
    if base.endswith("/") and path.startswith("/"):
        return base[:-1] + path
    return base + path


def build_payload(
    *,
    sequence: int,
    profile: str,
    event: str,
    authority_prefix: str,
    version: str,
    salt_epoch: str,
    rng: random.Random,
) -> Dict[str, object]:
    timestamp = datetime.now(timezone.utc).isoformat()
    authority_seed = f"{authority_prefix}:{profile}:{sequence}"
    authority_hash = hashlib.blake2b(
        authority_seed.encode("utf-8"), digest_size=16
    ).hexdigest()
    queue_depth = rng.randint(0, 25)
    pending = rng.randint(0, 6)
    latency_ms = rng.randint(80, 900)
    payload = {
        "timestamp": timestamp,
        "sequence": sequence,
        "event": event,
        "device_profile": profile,
        "client_version": version,
        "authority_hash": authority_hash,
        "salt_epoch": salt_epoch,
        "queue_depth": queue_depth,
        "pending_queue": pending,
        "latency_ms": latency_ms,
        "metadata": {
            "battery_level": rng.randint(35, 100),
            "network_type": rng.choice(["wifi", "cellular", "offline"]),
            "roaming": rng.choice([True, False]),
            "sdk_build": version,
        },
    }
    return payload


def post_payload(
    url: str, payload: Dict[str, object], headers: Dict[str, str], timeout: float
) -> int:
    if url.startswith("file://"):
        path = Path(url.removeprefix("file://"))
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("a", encoding="utf-8") as handle:
            json.dump(payload, handle, separators=(",", ":"))
            handle.write("\n")
        return 200
    data = json.dumps(payload).encode("utf-8")
    request = urllib.request.Request(url, data=data, headers=headers, method="POST")
    with urllib.request.urlopen(request, timeout=timeout) as response:  # nosec B310
        response.read()  # drain the body for keep-alive friendliness
        # Python <3.11 uses getcode, >=3.11 exposes status attribute.
        return getattr(response, "status", response.getcode())


def log_line(handle, message: str) -> None:
    timestamp = datetime.now(timezone.utc).isoformat()
    handle.write(f"{timestamp} {message}\n")
    handle.flush()


def run_load(args: argparse.Namespace) -> int:
    duration = parse_duration(args.duration)
    headers = parse_headers(args.header or [])
    endpoint = resolve_endpoint(args.cluster, args.endpoint, args.path)
    log_path = Path(args.log_file or DEFAULT_LOG_PATH)
    log_path.parent.mkdir(parents=True, exist_ok=True)

    rng = random.Random()
    rng.seed(args.seed or int(time.time()))

    profiles = args.device_profile or list(DEFAULT_DEVICE_PROFILES)
    events = args.event or list(DEFAULT_EVENTS)
    if not profiles:
        raise SystemExit("[error] provide at least one --device-profile value")
    if not events:
        raise SystemExit("[error] provide at least one --event value")

    credit = 0.0
    rate = max(float(args.rps), 0.0)
    tick = 0
    start = time.time()
    result = LoadResult()
    with log_path.open("a", encoding="utf-8") as log_handle:
        log_line(
            log_handle,
            f"starting load: endpoint={endpoint} duration={duration:.1f}s "
            f"rps={args.rps} dry_run={args.dry_run}",
        )
        print(
            f"[load-generator] hitting {endpoint} for {duration:.1f}s "
            f"at ~{args.rps:.2f} rps (log: {log_path})"
        )
        sequence = 0
        while time.time() - start < duration:
            tick += 1
            credit += rate
            ops = int(credit)
            credit -= ops
            tick_success = 0
            tick_fail = 0
            for _ in range(ops):
                profile = rng.choice(profiles)
                event = rng.choice(events)
                payload = build_payload(
                    sequence=sequence,
                    profile=profile,
                    event=event,
                    authority_prefix=args.authority_prefix,
                    version=args.client_version,
                    salt_epoch=args.salt_epoch,
                    rng=rng,
                )
                sequence += 1
                if args.dry_run:
                    tick_success += 1
                    result.record_success()
                    log_line(
                        log_handle,
                        f"DRYRUN seq={sequence} profile={profile} event={event} "
                        f"queue_depth={payload['queue_depth']}",
                    )
                    continue
                try:
                    status = post_payload(
                        endpoint, payload, headers=headers, timeout=args.timeout
                    )
                except urllib.error.URLError as exc:
                    tick_fail += 1
                    result.record_failure()
                    log_line(
                        log_handle,
                        f"ERROR seq={sequence} profile={profile} event={event} "
                        f"reason={exc}",
                    )
                    continue
                if 200 <= status < 300:
                    tick_success += 1
                    result.record_success()
                else:
                    tick_fail += 1
                    result.record_failure()
                    log_line(
                        log_handle,
                        f"WARN seq={sequence} profile={profile} event={event} "
                        f"status={status}",
                    )
            elapsed = time.time() - start
            print(
                f"[tick {tick:03d}] elapsed={elapsed:6.1f}s "
                f"success={tick_success} failure={tick_fail}"
            )
            time.sleep(1.0)
        log_line(
            log_handle,
            f"completed load: attempts={result.attempts} "
            f"success={result.successes} failure={result.failures}",
        )
    print(
        "[load-generator] done "
        f"(attempts={result.attempts} success={result.successes} "
        f"failure={result.failures})"
    )
    if result.attempts == 0 or result.successes == 0:
        return 2
    if result.failures:
        return 1
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Generate synthetic Android telemetry load for staging drills."
    )
    parser.add_argument(
        "--cluster",
        help="Cluster hostname (e.g. android-telemetry-stg). "
        "Combined with --path unless --endpoint is provided.",
    )
    parser.add_argument(
        "--endpoint",
        help="Full URL to POST the telemetry payloads to. "
        "Overrides --cluster/--path when provided. Use file:///path/to/output.ndjson "
        "to append payloads locally when network access is unavailable.",
    )
    parser.add_argument(
        "--path",
        default=DEFAULT_PATH,
        help=f"HTTP path appended to the cluster host (default: {DEFAULT_PATH}).",
    )
    parser.add_argument(
        "--duration",
        default=DEFAULT_DURATION,
        help="How long to run (e.g. 45s, 10m, 1h). Default: 5m.",
    )
    parser.add_argument(
        "--rps",
        type=float,
        default=DEFAULT_RPS,
        help="Approximate requests per second (default: 4).",
    )
    parser.add_argument(
        "--device-profile",
        action="append",
        help="Device profile bucket to cycle through. "
        "Can be specified multiple times (default: emulator, consumer, enterprise).",
    )
    parser.add_argument(
        "--event",
        action="append",
        help="Event names to cycle through (default set covers queue, override, "
        "redaction, and attestation probes).",
    )
    parser.add_argument(
        "--authority-prefix",
        default="android-lab",
        help="Prefix used when hashing authority identifiers (default: android-lab).",
    )
    parser.add_argument(
        "--salt-epoch",
        default="lab-epoch",
        help="Salt epoch label recorded in the payload (default: lab-epoch).",
    )
    parser.add_argument(
        "--client-version",
        default="AND7-lab",
        help="Client version string written into the payload metadata.",
    )
    parser.add_argument(
        "--header",
        action="append",
        help="Additional HTTP headers in KEY=VALUE form (repeatable). "
        "Content-Type defaults to application/json.",
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=5.0,
        help="HTTP timeout in seconds (default: 5).",
    )
    parser.add_argument(
        "--log-file",
        default=str(DEFAULT_LOG_PATH),
        help=f"Path to append load-generator logs (default: {DEFAULT_LOG_PATH}).",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Skip HTTP calls and log the synthetic payloads instead.",
    )
    parser.add_argument(
        "--seed",
        type=int,
        help="Seed for the pseudo-random generator (defaults to current epoch).",
    )
    return parser


def main(argv: Optional[List[str]] = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return run_load(args)
    except KeyboardInterrupt:
        print("[load-generator] interrupted, exiting...")
        return 130


if __name__ == "__main__":
    sys.exit(main())
