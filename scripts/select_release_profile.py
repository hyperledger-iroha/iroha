#!/usr/bin/env python3
"""
Utility for mapping deployment identifiers to release packaging profiles.

The script consumes `release/network_profiles.toml`, which captures the
current policy for routing networks to either the Iroha 2 (single-lane)
or Iroha 3 (Nexus) build artifacts.

Usage examples:

  $ scripts/select_release_profile.py --network sora-nexus
  iroha3

  $ scripts/select_release_profile.py --chain-id sora:nexus:global
  iroha3

  $ scripts/select_release_profile.py --emit-manifest out.json \
      --artifact iroha2=artifacts/iroha2-linux.tar.zst \
      --artifact iroha3=artifacts/iroha3-linux.tar.zst
"""
from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path

import ast


REPO_ROOT = Path(__file__).resolve().parents[1]
MANIFEST_PATH = REPO_ROOT / "release" / "network_profiles.toml"


def load_manifest() -> dict:
    data: dict[str, object] = {"profiles": []}
    current_section: dict[str, object] | None = None

    with MANIFEST_PATH.open("r", encoding="utf-8") as fh:
        for raw_line in fh:
            line = raw_line.strip()
            if not line or line.startswith("#"):
                continue
            if line == "[[profiles]]":
                current_section = {}
                data.setdefault("profiles", []).append(current_section)
                continue
            if "=" not in line:
                continue

            key, value = line.split("=", 1)
            key = key.strip()
            value_str = value.strip()

            parsed_value = _parse_value(value_str)
            if current_section is not None:
                current_section[key] = parsed_value
            else:
                data[key] = parsed_value

    if "default_profile" not in data:
        raise SystemExit("network_profiles.toml missing `default_profile` entry")

    return data


def _parse_value(value: str) -> object:
    # Handle arrays and quoted strings via ast.literal_eval for convenience.
    try:
        if value.startswith("["):
            literal = ast.literal_eval(value)
            return [str(item) for item in literal]
        if value.startswith(("'", '"')):
            return str(ast.literal_eval(value))
    except (SyntaxError, ValueError) as exc:  # pragma: no cover - defensive fallback
        raise SystemExit(f"Failed to parse value from manifest: {value}") from exc

    return value


def normalize(value: str) -> str:
    return value.strip().lower().replace(" ", "-")


def resolve_profile(manifest: dict, *, network: str | None, chain_id: str | None) -> str:
    default_profile: str = manifest["default_profile"]
    profiles = manifest.get("profiles", [])

    target_network = normalize(network) if network else None
    target_chain = normalize(chain_id) if chain_id else None

    for entry in profiles:
        profile = entry["profile"]
        names = {normalize(item) for item in entry.get("networks", [])}
        names.update(normalize(alias) for alias in entry.get("aliases", []))

        chains = {normalize(item) for item in entry.get("chain_ids", [])}

        if target_network and target_network in names:
            return profile
        if target_chain and target_chain in chains:
            return profile

    return default_profile


def emit_manifest(manifest: dict, artifacts: dict[str, str], output: Path) -> None:
    rendered = {
        "default_profile": manifest["default_profile"],
        "profiles": [],
    }

    for entry in manifest.get("profiles", []):
        profile_id = entry["profile"]
        rendered_entry = {
            "profile": profile_id,
            "networks": entry.get("networks", []),
            "aliases": entry.get("aliases", []),
            "chain_ids": entry.get("chain_ids", []),
            "description": entry.get("description", ""),
        }
        if profile_id in artifacts:
            rendered_entry["artifact"] = os.path.normpath(artifacts[profile_id])
        rendered["profiles"].append(rendered_entry)

    rendered["artifacts"] = {
        profile: os.path.normpath(path) for profile, path in artifacts.items()
    }

    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open("w", encoding="utf-8") as fh:
        json.dump(rendered, fh, indent=2)
        fh.write("\n")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--network",
        help="Human-readable network alias (e.g., sora-nexus, testnet)",
    )
    parser.add_argument(
        "--chain-id",
        help="Chain identifier, if available (e.g., sora:nexus:global)",
    )
    parser.add_argument(
        "--emit-manifest",
        help="Optional path to emit a JSON manifest describing the profile mapping.",
    )
    parser.add_argument(
        "--artifact",
        action="append",
        default=[],
        metavar="PROFILE=PATH",
        help="Associate a built artifact with a profile (repeatable).",
    )
    parser.add_argument(
        "--list",
        action="store_true",
        help="Print the manifest entries and exit.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    manifest = load_manifest()

    if args.list:
        json.dump(manifest, sys.stdout, indent=2)
        sys.stdout.write("\n")
        return 0

    artifacts: dict[str, str] = {}
    for binding in args.artifact:
        if "=" not in binding:
            raise SystemExit(f"Invalid --artifact binding (expected profile=path): {binding}")
        profile, path = binding.split("=", 1)
        artifacts[profile.strip()] = path.strip()

    if args.emit_manifest:
        output_path = Path(args.emit_manifest)
        emit_manifest(manifest, artifacts, output_path)

    if not args.network and not args.chain_id:
        # If only emitting manifest, nothing else to print.
        return 0

    profile = resolve_profile(
        manifest,
        network=args.network,
        chain_id=args.chain_id,
    )
    print(profile)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
