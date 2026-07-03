#!/usr/bin/env python3
"""
Generate the canonical SoraFS multi-source orchestrator fixture.

The resulting JSON files live under
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/` and are shared by every SDK
parity harness. Run this script whenever the chunker input or provider profile
changes so the fixture stays aligned with the deterministic Rust vectors.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path
from typing import Any, Dict, List

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from sorafs_checker_preflight import fsync_checker_output_parent

FIXTURE_NAME = "multi_peer_parity_v1"
FIXTURE_NOW_UNIX_SECS = 1_725_000_000
PROFILE_HANDLE = "sorafs.sf1@1.0.0"
PROVIDER_COUNT = 4


def repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


def read_open_flags() -> int:
    """Return descriptor flags for reading fixture inputs fail-closed."""

    flags = os.O_RDONLY
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    if nofollow:
        flags |= nofollow
    return flags


def write_open_flags() -> int:
    """Return descriptor flags for writing generated fixture files fail-closed."""

    flags = os.O_WRONLY | os.O_CREAT | os.O_TRUNC
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    if nofollow:
        flags |= nofollow
    return flags


def write_all(fd: int, chunk: bytes) -> None:
    """Write every byte to a fixture descriptor, including after short writes."""

    view = memoryview(chunk)
    while view:
        written = os.write(fd, view)
        if written <= 0:
            raise OSError("failed to write orchestrator fixture output")
        view = view[written:]


def validate_fixture_path(path: Path, label: str) -> None:
    """Reject symlinked fixture paths and parent chains before I/O."""

    if not isinstance(path, Path):
        raise ValueError(f"{label} must be a path")
    try:
        if path.is_symlink():
            raise ValueError(f"{label} `{path}` must not be a symlink")
        for parent in (path.parent, *path.parent.parents):
            if parent.is_symlink():
                raise ValueError(f"{label} parent `{parent}` must not be a symlink")
            if parent.exists() and not parent.is_dir():
                raise ValueError(
                    f"{label} parent `{parent}` must be a directory when it exists"
                )
    except ValueError:
        raise
    except OSError as error:
        raise ValueError(f"failed to inspect {label} `{path}`: {error}") from error


def ensure_fixture_directory(path: Path, label: str) -> None:
    """Create a generated fixture directory without following symlink parents."""

    validate_fixture_path(path, label)
    path.mkdir(parents=True, exist_ok=True)
    if not path.is_dir():
        raise ValueError(f"{label} `{path}` must be a directory")


def fixture_file_size(path: Path, label: str) -> int:
    """Return a regular fixture file size through a no-follow descriptor."""

    validate_fixture_path(path, label)
    if not path.is_file():
        raise ValueError(f"{label} `{path}` must exist and be a file")
    fd = -1
    try:
        fd = os.open(path, read_open_flags())
        return os.fstat(fd).st_size
    finally:
        if fd >= 0:
            os.close(fd)


def load_chunker_fixture(root: Path) -> Dict[str, Any]:
    plan_path = root / "fixtures" / "sorafs_chunker" / "sf1_profile_v1.json"
    validate_fixture_path(plan_path, "chunker fixture")
    if not plan_path.is_file():
        raise ValueError(f"chunker fixture `{plan_path}` must exist and be a file")
    fd = -1
    try:
        fd = os.open(plan_path, read_open_flags())
        handle = os.fdopen(fd, "r", encoding="utf-8")
        fd = -1
        with handle:
            return json.load(handle)
    finally:
        if fd >= 0:
            os.close(fd)


def build_plan_specs(plan_fixture: Dict[str, Any]) -> List[Dict[str, Any]]:
    specs: List[Dict[str, Any]] = []
    chunk_lengths = plan_fixture["chunk_lengths"]
    chunk_offsets = plan_fixture["chunk_offsets"]
    chunk_digests = plan_fixture["chunk_digests_blake3"]
    for idx, length in enumerate(chunk_lengths):
        specs.append(
            {
                "chunk_index": idx,
                "offset": chunk_offsets[idx],
                "length": length,
                "digest_blake3": chunk_digests[idx],
            }
        )
    return specs


def build_providers(max_chunk_length: int) -> List[Dict[str, Any]]:
    providers: List[Dict[str, Any]] = []
    max_bytes_per_sec = max_chunk_length * 8
    burst_bytes = max_chunk_length * 4
    for idx in range(PROVIDER_COUNT):
        provider_id = f"fixture-provider-{idx}"
        providers.append(
            {
                "provider_id": provider_id,
                "profile_aliases": [f"fixture-peer-{idx}"],
                "availability": "hot",
                "allow_unknown_capabilities": False,
                "capability_names": ["chunk-range", "sorafs-fetch", "sorafs-fixture"],
                "range_capability": {
                    "max_chunk_span": max_chunk_length,
                    "min_granularity": 1,
                    "supports_sparse_offsets": True,
                    "requires_alignment": False,
                    "supports_merkle_proof": True,
                },
                "stream_budget": {
                    "max_in_flight": 1,
                    "max_bytes_per_sec": max_bytes_per_sec,
                    "burst_bytes": burst_bytes,
                },
                "refresh_deadline": FIXTURE_NOW_UNIX_SECS + 600,
                "expires_at": FIXTURE_NOW_UNIX_SECS + 86_400,
                "ttl_secs": 86_400,
                "notes": "Deterministic fixture provider",
                "transport_hints": [
                    {
                        "protocol": "fixture",
                        "protocol_id": 0,
                        "priority": 0,
                    }
                ],
            }
        )
    return providers


def build_telemetry(providers: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
    entries: List[Dict[str, Any]] = []
    for idx, provider in enumerate(providers):
        entries.append(
            {
                "provider_id": provider["provider_id"],
                "qos_score": 95.0 - float(idx),
                "latency_p95_ms": 120.0 + float(idx) * 15.0,
                "failure_rate_ewma": 0.02 * float(idx),
                "token_health": 0.9,
                "staking_weight": 1.0,
                "reputation_score_bps": 10_000,
                "penalty": False,
                "last_updated_unix": FIXTURE_NOW_UNIX_SECS - 120,
            }
        )
    return entries


def build_options() -> Dict[str, Any]:
    return {
        "max_parallel": 3,
        "max_peers": 3,
        "retry_budget": 5,
        "provider_failure_threshold": 2,
        "per_chunk_retry_limit": 5,
        "global_parallel_limit": 3,
        "verify_digests": True,
        "verify_lengths": True,
        "use_scoreboard": True,
        "return_scoreboard": True,
        "telemetry_region": "fixture",
        "rollout_phase": "ramp",
        "policy_override": {
            "transport_policy": "soranet-strict",
            "anonymity_policy": "anon-strict-pq",
        },
        "scoreboard": {
            "now_unix_secs": FIXTURE_NOW_UNIX_SECS,
            "telemetry_grace_period_secs": 3_600,
            "latency_cap_ms": 2_500,
        },
    }


def write_json(path: Path, payload: Any) -> None:
    validate_fixture_path(path, "orchestrator fixture output")
    rendered = (json.dumps(payload, indent=2, allow_nan=False) + "\n").encode("utf-8")
    fd = -1
    try:
        fd = os.open(path, write_open_flags(), 0o666)
        write_all(fd, rendered)
        os.fsync(fd)
    finally:
        if fd >= 0:
            os.close(fd)
    parent_sync_errors = fsync_checker_output_parent(
        path, label="orchestrator fixture output"
    )
    if parent_sync_errors:
        raise ValueError(parent_sync_errors[0])


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Generate the SoraFS orchestrator parity fixture."
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="Target directory (defaults to fixtures/sorafs_orchestrator/multi_peer_parity_v1)",
    )
    args = parser.parse_args()

    root = repo_root()
    chunker_fixture = load_chunker_fixture(root)
    plan_specs = build_plan_specs(chunker_fixture)
    max_chunk_length = max(int(length) for length in chunker_fixture["chunk_lengths"])
    providers = build_providers(max_chunk_length)
    telemetry = build_telemetry(providers)
    options = build_options()

    output_dir = (
        args.output
        if args.output is not None
        else root / "fixtures" / "sorafs_orchestrator" / FIXTURE_NAME
    )
    ensure_fixture_directory(output_dir, "orchestrator fixture output directory")

    write_json(output_dir / "plan.json", plan_specs)
    write_json(output_dir / "providers.json", providers)
    write_json(output_dir / "telemetry.json", telemetry)
    write_json(output_dir / "options.json", options)

    payload_path = Path("fuzz") / "sorafs_chunker" / "sf1_profile_v1_input.bin"
    payload_bytes = fixture_file_size(root / payload_path, "chunker payload")

    metadata = {
        "version": 1,
        "profile_handle": PROFILE_HANDLE,
        "fixture": FIXTURE_NAME,
        "plan_file": "plan.json",
        "providers_file": "providers.json",
        "telemetry_file": "telemetry.json",
        "options_file": "options.json",
        "payload_path": str(payload_path).replace("\\", "/"),
        "payload_bytes": payload_bytes,
        "chunk_count": len(plan_specs),
        "provider_count": len(providers),
        "max_chunk_length": max_chunk_length,
        "now_unix_secs": FIXTURE_NOW_UNIX_SECS,
        "description": "Deterministic multi-peer fetch scenario shared by SDK parity tests.",
    }
    write_json(output_dir / "metadata.json", metadata)
    print(f"Wrote orchestrator fixture to {output_dir}")


if __name__ == "__main__":
    main()
