#!/usr/bin/env python3
"""Promote aggregated sm_perf captures into baseline JSON files.

This helper takes the aggregated output produced by
`scripts/sm_perf_aggregate.py` (or the capture helper matrix) and writes
baseline files in the format expected by `scripts/sm_perf.sh` /
`sm_perf_check`. It applies the standard tolerance map used for the SM
benchmarks so lab operators do not have to hand-edit the JSON.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Dict, Mapping, MutableMapping, Sequence

# Standard tolerance map used for SM perf baselines.
DEFAULT_TOLERANCES: Dict[str, float] = {
    "sm2_vs_ed25519_sign/sm2_sign": 0.15,
    "sm2_vs_ed25519_sign/ed25519_sign": 0.12,
    "sm2_vs_ed25519_verify/verify/sm2": 0.15,
    "sm2_vs_ed25519_verify/verify/ed25519": 0.12,
    "sm3_vs_sha256_hash/sm3_hash": 0.12,
    "sm3_vs_sha256_hash/sha256_hash": 0.08,
    "sm4_vs_chacha20poly1305_encrypt/sm4_gcm_encrypt": 0.12,
    "sm4_vs_chacha20poly1305_encrypt/chacha20poly1305_encrypt": 0.10,
    "sm4_vs_chacha20poly1305_decrypt/sm4_gcm_decrypt": 0.12,
    "sm4_vs_chacha20poly1305_decrypt/chacha20poly1305_decrypt": 0.10,
}

# Optional compare tolerances used in existing baselines.
DEFAULT_COMPARE_TOLERANCES: Dict[str, float] = {
    # SM3 auto/neon paths still run slightly slower than the scalar baseline;
    # allow a modest buffer so captures do not flap on legitimate variance.
    "sm3_vs_sha256_hash/sm3_hash": 0.65,
}


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    payload = load_aggregated(args.aggregated)

    modes = payload.get("modes")
    if not isinstance(modes, Mapping):
        sys.stderr.write("aggregated JSON missing 'modes' object\n")
        return 1

    requested = set(args.modes) if args.modes else None
    wrote_any = False
    for mode_name, mode_payload in modes.items():
        if requested and mode_name not in requested:
            continue
        target_arch, target_os = extract_platform(mode_payload, args.target_os)
        cpu_label = args.cpu_label or extract_cpu_label(mode_payload)
        benchmarks = extract_benchmarks(mode_payload)

        out_path = args.out_dir / baseline_filename(
            target_arch, target_os, mode_name
        )
        if out_path.exists() and not args.overwrite:
            sys.stderr.write(
                f"refusing to overwrite existing file: {out_path} (use --overwrite)\n"
            )
            return 1

        baseline = build_baseline(
            benchmarks,
            target_arch=target_arch,
            target_os=target_os,
            cpu_label=cpu_label,
        )
        write_json(out_path, baseline)
        print(f"Wrote baseline for mode '{mode_name}' -> {out_path}")
        wrote_any = True

    if requested and not wrote_any:
        sys.stderr.write(
            "no modes matched the provided filters: "
            + ", ".join(sorted(requested))
            + "\n"
        )
        return 1

    return 0


def parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Promote aggregated sm_perf captures into baseline JSON files."
    )
    parser.add_argument(
        "aggregated",
        type=Path,
        help="Path to aggregated.json produced by sm_perf_aggregate.py",
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=Path("crates/iroha_crypto/benches"),
        help="Directory to write baseline files (default: crates/iroha_crypto/benches)",
    )
    parser.add_argument(
        "--mode",
        dest="modes",
        action="append",
        help="Only emit baselines for the specified mode (repeatable).",
    )
    parser.add_argument(
        "--target-os",
        help="Override the target_os token from the aggregated file (e.g., unknown_linux_gnu).",
    )
    parser.add_argument(
        "--cpu-label",
        help="Override the CPU label to store in metadata (defaults to first source entry).",
    )
    parser.add_argument(
        "--overwrite",
        action="store_true",
        help="Allow overwriting existing baseline files.",
    )
    return parser.parse_args(argv)


def load_aggregated(path: Path) -> Mapping[str, object]:
    try:
        with path.open("r", encoding="utf-8") as handle:
            return json.load(handle)
    except FileNotFoundError as exc:
        raise SystemExit(f"aggregated file not found: {path}") from exc
    except json.JSONDecodeError as exc:
        raise SystemExit(f"failed to parse aggregated JSON: {exc}") from exc


def extract_platform(mode_payload: Mapping[str, object], override_os: str | None):
    target_arch = mode_payload.get("target_arch")
    target_os = override_os or mode_payload.get("target_os")
    if not isinstance(target_arch, str) or not isinstance(target_os, str):
        raise SystemExit("mode payload missing target_arch or target_os")
    return target_arch, target_os


def extract_cpu_label(mode_payload: Mapping[str, object]) -> str | None:
    sources = mode_payload.get("sources")
    if isinstance(sources, Sequence) and sources:
        entry = sources[0]
        if isinstance(entry, Mapping):
            cpu_label = entry.get("cpu_label")
            if cpu_label is not None:
                return str(cpu_label)
    return None


def extract_benchmarks(mode_payload: Mapping[str, object]) -> Dict[str, float]:
    benchmarks = mode_payload.get("benchmarks")
    if not isinstance(benchmarks, Mapping):
        raise SystemExit("mode payload missing benchmarks")
    output: Dict[str, float] = {}
    for key, value in benchmarks.items():
        if not isinstance(key, str):
            raise SystemExit(f"invalid benchmark key: {key!r}")
        if not isinstance(value, (int, float)):
            raise SystemExit(
                f"benchmark '{key}' must be numeric, found {type(value).__name__}"
            )
        output[key] = float(value)
    return output


def baseline_filename(target_arch: str, target_os: str, mode: str) -> str:
    sanitized_mode = mode.replace("-", "_")
    return f"sm_perf_baseline_{target_arch}_{target_os}_{sanitized_mode}.json"


def build_baseline(
    benchmarks: Mapping[str, float],
    *,
    target_arch: str,
    target_os: str,
    cpu_label: str | None,
) -> Mapping[str, object]:
    metadata: MutableMapping[str, object] = {
        "target_arch": target_arch,
        "target_os": target_os,
    }
    if cpu_label:
        metadata["cpu"] = cpu_label

    return {
        "benchmarks": benchmarks,
        "tolerances": DEFAULT_TOLERANCES,
        "compare_tolerances": DEFAULT_COMPARE_TOLERANCES,
        "metadata": metadata,
    }


def write_json(path: Path, payload: Mapping[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as handle:
        json.dump(payload, handle, indent=2)
        handle.write("\n")


if __name__ == "__main__":
    raise SystemExit(main())
