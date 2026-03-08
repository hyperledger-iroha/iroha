#!/usr/bin/env python3
"""Bootstrap helper for Nexus lane manifests and catalog snippets (NX-7)."""

import argparse
import json
import math
import re
import subprocess
import sys
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Sequence


@dataclass
class RuntimeUpgradeHook:
    """Optional runtime-upgrade hook metadata embedded in the manifest."""

    metadata_key: str
    require_metadata: bool
    allow: bool
    allowed_ids: List[str] = field(default_factory=list)


@dataclass
class LaneBootstrapConfig:
    """Normalized configuration derived from CLI arguments."""

    lane_alias: str
    slug: str
    lane_id: int
    dataspace_alias: str
    dataspace_id: int
    governance_module: str
    settlement_handle: str
    description: Optional[str]
    dataspace_description: Optional[str]
    visibility: str
    lane_type: str
    storage_profile: str
    validators: List[str]
    quorum: int
    protected_namespaces: List[str]
    metadata: Dict[str, str]
    route_instructions: List[str]
    route_accounts: List[str]
    runtime_upgrade: Optional[RuntimeUpgradeHook]
    output_dir: Path
    manifest_path: Path
    catalog_path: Path
    summary_path: Path
    space_directory_out: Path
    force: bool
    dry_run: bool
    encode_space_directory: bool
    cargo_bin: str


@dataclass
class LaneBootstrapPlan:
    """Concrete artefacts ready to be written to disk."""

    slug: str
    kura_segment: str
    merge_segment: str
    manifest_path: Path
    catalog_path: Path
    summary_path: Path
    manifest: dict
    catalog_snippet: str
    summary: dict
    dry_run: bool
    force: bool
    space_directory_path: Path
    encode_space_directory: bool
    cargo_bin: str


def main(argv: Optional[Sequence[str]] = None) -> None:
    """Parse CLI arguments and write the generated artefacts."""
    plan = build_plan(parse_args(argv))
    if plan.dry_run:
        print(json.dumps(plan.summary, indent=2, sort_keys=True))
        print("\n# Catalog snippet\n")
        print(plan.catalog_snippet)
        return
    write_plan(plan)
    print(f"[nexus] manifest written to {plan.manifest_path}")
    print(f"[nexus] catalog snippet written to {plan.catalog_path}")
    print(f"[nexus] summary written to {plan.summary_path}")


def parse_args(argv: Optional[Sequence[str]]) -> LaneBootstrapConfig:
    """Convert CLI arguments into a LaneBootstrapConfig."""
    parser = argparse.ArgumentParser(
        description="Generate lane manifest + config snippets for Nexus lane provisioning.",
    )

    parser.add_argument("--lane-alias", required=True, help="Human-friendly lane alias (e.g. payments)")
    parser.add_argument(
        "--lane-id",
        type=int,
        required=True,
        help="Zero-based lane identifier that must match the lane catalog index",
    )
    parser.add_argument(
        "--dataspace-alias",
        required=True,
        help="Dataspace alias bound to this lane",
    )
    parser.add_argument(
        "--dataspace-id",
        type=int,
        help="Explicit dataspace identifier override (defaults to lane-id)",
    )
    parser.add_argument(
        "--dataspace-hash",
        help="32-byte hex hash that derives the dataspace id (used when dataspace-id is omitted)",
    )
    parser.add_argument(
        "--dataspace-description",
        help="Optional dataspace description for docs and dashboards",
    )
    parser.add_argument("--description", help="Lane description for docs/dashboards")
    parser.add_argument(
        "--governance-module",
        required=True,
        help="Registered governance module name (e.g. parliament)",
    )
    parser.add_argument(
        "--settlement-handle",
        required=True,
        help="Settlement/fee policy identifier (e.g. xor_global)",
    )
    parser.add_argument(
        "--validator",
        action="append",
        default=[],
        help="Validator account id (repeatable)",
    )
    parser.add_argument(
        "--validators-file",
        type=Path,
        help="Path to newline-delimited validator list",
    )
    parser.add_argument(
        "--quorum",
        type=int,
        help="Explicit manifest quorum (defaults to simple majority)",
    )
    parser.add_argument(
        "--protected-namespace",
        action="append",
        default=[],
        help="Namespace guarded by this manifest (repeatable)",
    )
    parser.add_argument(
        "--visibility",
        default="public",
        help="Lane visibility descriptor (public/private/hybrid)",
    )
    parser.add_argument("--lane-type", default="default_public", help="Lane profile/type identifier")
    parser.add_argument(
        "--storage-profile",
        default="full_replica",
        help="Storage profile (`full_replica`, `commitment_only`, etc.)",
    )
    parser.add_argument(
        "--metadata",
        action="append",
        default=[],
        metavar="KEY=VALUE",
        help="Metadata key-value pair (repeatable)",
    )
    parser.add_argument(
        "--route-instruction",
        action="append",
        default=[],
        help="Instruction matcher routed to this lane (repeatable)",
    )
    parser.add_argument(
        "--route-account",
        action="append",
        default=[],
        help="Account matcher routed to this lane (repeatable)",
    )
    parser.add_argument(
        "--allow-runtime-upgrades",
        action="store_true",
        help="Enable runtime-upgrade hook in the manifest",
    )
    parser.add_argument(
        "--runtime-upgrade-metadata-key",
        help="Metadata key required when runtime-upgrade hook is enabled",
    )
    parser.add_argument(
        "--runtime-upgrade-allowed-id",
        action="append",
        default=[],
        help="Allowed runtime-upgrade identifier (repeatable)",
    )
    parser.add_argument(
        "--require-runtime-metadata",
        action="store_true",
        help="Mark runtime-upgrade hook metadata as mandatory",
    )
    parser.add_argument("--telemetry-contact", help="Contact email/handle stored in metadata.contact")
    parser.add_argument("--telemetry-channel", help="Pager/Slack handle stored in metadata.channel")
    parser.add_argument("--telemetry-runbook", help="Runbook URL stored in metadata.runbook")
    parser.add_argument(
        "--encode-space-directory",
        action="store_true",
        help="Encode the manifest into a Norito `.to` file via `cargo xtask space-directory encode`.",
    )
    parser.add_argument(
        "--space-directory-out",
        help="Path for the encoded Space Directory manifest (defaults to <slug>.manifest.to).",
    )
    parser.add_argument(
        "--cargo-bin",
        default="cargo",
        help="Cargo binary used when invoking `cargo xtask space-directory encode`.",
    )

    parser.add_argument("--output-dir", type=Path, default=Path.cwd(), help="Directory for generated artefacts")
    parser.add_argument("--manifest-out", help="Relative/absolute path for manifest JSON output")
    parser.add_argument("--catalog-out", help="Relative/absolute path for catalog snippet output")
    parser.add_argument("--summary-out", help="Relative/absolute path for JSON summary output")
    parser.add_argument("--force", action="store_true", help="Overwrite existing files")
    parser.add_argument("--dry-run", action="store_true", help="Print results instead of writing files")

    args = parser.parse_args(argv)

    validators = collect_validators(args.validator, args.validators_file)
    if not validators:
        parser.error("at least one --validator or --validators-file entry is required")

    quorum = args.quorum or default_quorum(len(validators))
    if quorum < 1 or quorum > len(validators):
        parser.error("--quorum must be between 1 and the number of validators")

    protected = normalize_protected_namespaces(args.protected_namespace, args.lane_alias)
    metadata = parse_metadata(args.metadata)
    if args.telemetry_contact:
        metadata.setdefault("contact", args.telemetry_contact)
    if args.telemetry_channel:
        metadata.setdefault("channel", args.telemetry_channel)
    if args.telemetry_runbook:
        metadata.setdefault("runbook", args.telemetry_runbook)

    runtime_hook = None
    if args.allow_runtime_upgrades:
        if not args.runtime_upgrade_metadata_key:
            parser.error("--runtime-upgrade-metadata-key is required when enabling runtime upgrades")
        runtime_hook = RuntimeUpgradeHook(
            metadata_key=args.runtime_upgrade_metadata_key,
            require_metadata=args.require_runtime_metadata,
            allow=True,
            allowed_ids=sorted(set(args.runtime_upgrade_allowed_id)),
        )

    dataspace_id = compute_dataspace_id(args)

    output_dir = Path(args.output_dir).expanduser().resolve()
    slug = slugify_alias(args.lane_alias, args.lane_id)
    manifest_path = resolve_out_path(output_dir, args.manifest_out, f"{slug}.manifest.json")
    catalog_path = resolve_out_path(output_dir, args.catalog_out, f"{slug}.catalog.toml")
    summary_path = resolve_out_path(output_dir, args.summary_out, f"{slug}.summary.json")
    space_dir_path = resolve_out_path(output_dir, args.space_directory_out, f"{slug}.manifest.to")

    return LaneBootstrapConfig(
        lane_alias=args.lane_alias,
        slug=slug,
        lane_id=args.lane_id,
        dataspace_alias=args.dataspace_alias,
        dataspace_id=dataspace_id,
        governance_module=args.governance_module,
        settlement_handle=args.settlement_handle,
        description=args.description,
        dataspace_description=args.dataspace_description,
        visibility=args.visibility,
        lane_type=args.lane_type,
        storage_profile=args.storage_profile,
        validators=validators,
        quorum=quorum,
        protected_namespaces=protected,
        metadata=metadata,
        route_instructions=args.route_instruction or [],
        route_accounts=args.route_account or [],
        runtime_upgrade=runtime_hook,
        output_dir=output_dir,
        manifest_path=manifest_path,
        catalog_path=catalog_path,
        summary_path=summary_path,
        space_directory_out=space_dir_path,
        force=args.force,
        dry_run=args.dry_run,
        encode_space_directory=args.encode_space_directory,
        cargo_bin=args.cargo_bin,
    )


def build_plan(config: LaneBootstrapConfig) -> LaneBootstrapPlan:
    """Derive the manifest, catalog snippet, and summary artefacts from config."""
    slug = config.slug
    kura_segment = f"lane_{config.lane_id:03d}_{slug}"
    merge_segment = f"lane_{config.lane_id:03d}_merge"

    manifest = build_manifest(config)
    catalog_snippet = build_catalog_snippet(config, slug)
    summary = build_summary(
        config,
        slug,
        kura_segment,
        merge_segment,
        config.space_directory_out,
        config.encode_space_directory,
        config.cargo_bin,
    )

    return LaneBootstrapPlan(
        slug=slug,
        kura_segment=kura_segment,
        merge_segment=merge_segment,
        manifest_path=config.manifest_path,
        catalog_path=config.catalog_path,
        summary_path=config.summary_path,
        manifest=manifest,
        catalog_snippet=catalog_snippet,
        summary=summary,
        dry_run=config.dry_run,
        force=config.force,
        space_directory_path=config.space_directory_out,
        encode_space_directory=config.encode_space_directory,
        cargo_bin=config.cargo_bin,
    )


def build_manifest(config: LaneBootstrapConfig) -> dict:
    """Construct the manifest dictionary."""
    manifest = {
        "lane": config.lane_alias,
        "governance": config.governance_module,
        "version": 1,
        "validators": config.validators,
        "quorum": config.quorum,
        "protected_namespaces": config.protected_namespaces,
    }
    hooks = {}
    if config.runtime_upgrade:
        hooks["runtime_upgrade"] = {
            "allow": config.runtime_upgrade.allow,
            "require_metadata": config.runtime_upgrade.require_metadata,
            "metadata_key": config.runtime_upgrade.metadata_key,
            "allowed_ids": config.runtime_upgrade.allowed_ids,
        }
    if hooks:
        manifest["hooks"] = hooks
    return manifest


def build_catalog_snippet(config: LaneBootstrapConfig, slug: str) -> str:
    """Create a TOML snippet covering lane + dataspace + routing entries."""
    timestamp = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    lines: List[str] = [
        f"# Generated by nexus_lane_bootstrap on {timestamp}",
        f"# Lane alias '{config.lane_alias}' (id {config.lane_id})",
        "",
        "[[nexus.lane_catalog]]",
        f"index = {config.lane_id}",
        f'alias = "{config.lane_alias}"',
        f'dataspace = "{config.dataspace_alias}"',
    ]
    if config.description:
        lines.append(f'description = "{config.description}"')
    lines.extend(
        [
            f'visibility = "{config.visibility}"',
            f'lane_type = "{config.lane_type}"',
            f'governance = "{config.governance_module}"',
            f'settlement = "{config.settlement_handle}"',
            f'storage = "{config.storage_profile}"',
        ]
    )
    if config.metadata:
        lines.append("[nexus.lane_catalog.metadata]")
        for key in sorted(config.metadata):
            value = config.metadata[key]
            lines.append(f'{key} = "{value}"')
    lines.append("")
    lines.extend(
        [
            "[[nexus.dataspace_catalog]]",
            f'alias = "{config.dataspace_alias}"',
            f"id = {config.dataspace_id}",
        ]
    )
    if config.dataspace_description:
        lines.append(f'description = "{config.dataspace_description}"')
    lines.append("")

    for instruction in config.route_instructions:
        lines.extend(
            [
                "[[nexus.routing_policy.rules]]",
                f"lane = {config.lane_id}",
                f'dataspace = "{config.dataspace_alias}"',
                "[nexus.routing_policy.rules.matcher]",
                f'instruction = "{instruction}"',
                f'description = "Route instruction {instruction} to lane {config.lane_alias}"',
                "",
            ]
        )
    for account in config.route_accounts:
        lines.extend(
            [
                "[[nexus.routing_policy.rules]]",
                f"lane = {config.lane_id}",
                f'dataspace = "{config.dataspace_alias}"',
                "[nexus.routing_policy.rules.matcher]",
                f'account = "{account}"',
                f'description = "Route account {account} to lane {config.lane_alias}"',
                "",
            ]
        )
    return "\n".join(lines).rstrip() + "\n"


def build_summary(
    config: LaneBootstrapConfig,
    slug: str,
    kura_segment: str,
    merge_segment: str,
    space_directory_path: Path,
    encode_space_directory: bool,
    cargo_bin: str,
) -> dict:
    """Assemble the JSON summary for auditors and automation."""
    encode_command = [
        cargo_bin,
        "xtask",
        "space-directory",
        "encode",
        "--json",
        str(config.manifest_path),
        "--out",
        str(space_directory_path),
    ]
    encode_step = (
        f"Encoded the manifest into {space_directory_path} via `{format_shell_command(encode_command)}`."
        if encode_space_directory
        else f"Encode the manifest via `{format_shell_command(encode_command)}` before publishing to the Space Directory."
    )
    return {
        "lane_id": config.lane_id,
        "lane_alias": config.lane_alias,
        "dataspace_alias": config.dataspace_alias,
        "dataspace_id": config.dataspace_id,
        "slug": slug,
        "kura_segment": kura_segment,
        "merge_segment": merge_segment,
        "manifest_path": str(config.manifest_path),
        "catalog_snippet_path": str(config.catalog_path),
        "summary_path": str(config.summary_path),
        "space_directory_manifest_to": str(space_directory_path),
        "space_directory_encode": {
            "command": encode_command,
            "executed": encode_space_directory,
        },
        "governance_module": config.governance_module,
        "settlement_handle": config.settlement_handle,
        "visibility": config.visibility,
        "lane_type": config.lane_type,
        "storage_profile": config.storage_profile,
        "validators": config.validators,
        "quorum": config.quorum,
        "protected_namespaces": config.protected_namespaces,
        "metadata": config.metadata,
        "routing": {
            "instructions": config.route_instructions,
            "accounts": config.route_accounts,
        },
        "runtime_upgrade": None
        if not config.runtime_upgrade
        else {
            "metadata_key": config.runtime_upgrade.metadata_key,
            "require_metadata": config.runtime_upgrade.require_metadata,
            "allowed_ids": config.runtime_upgrade.allowed_ids,
        },
        "next_steps": [
            "Copy the manifest into the configured `nexus.registry.manifest_directory` (and cache directory, if set).",
            "Append the catalog snippet to your Nexus config and bump `[nexus] lane_count` accordingly.",
            encode_step,
            "Restart irohad with `--trace-config` to validate the new geometry and capture evidence for the rollout ticket.",
            "Follow docs/source/nexus_elastic_lane.md to complete validator bootstrap and telemetry wiring.",
        ],
    }


def write_plan(plan: LaneBootstrapPlan) -> None:
    """Persist artefacts to disk."""
    for path in (plan.manifest_path, plan.catalog_path, plan.summary_path):
        path.parent.mkdir(parents=True, exist_ok=True)
        if path.exists() and not plan.force:
            raise RuntimeError(f"{path} already exists (rerun with --force to overwrite)")

    plan.manifest_path.write_text(json.dumps(plan.manifest, indent=2) + "\n", encoding="utf-8")
    plan.catalog_path.write_text(plan.catalog_snippet, encoding="utf-8")
    plan.summary_path.write_text(json.dumps(plan.summary, indent=2) + "\n", encoding="utf-8")
    if plan.encode_space_directory and not plan.dry_run:
        encode_space_directory_manifest(plan)


def slugify_alias(alias: str, lane_id: int) -> str:
    """Convert a human alias into a filesystem-friendly slug."""
    normalized = re.sub(r"[^a-z0-9]+", "_", alias.lower()).strip("_")
    if not normalized:
        normalized = f"lane{lane_id:03d}"
    return normalized


def collect_validators(inline: List[str], file_path: Optional[Path]) -> List[str]:
    """Load validators from CLI flags and optional file."""
    validators: List[str] = []
    for literal in inline:
        append_if_new(validators, literal.strip())
    if file_path:
        data = file_path.read_text(encoding="utf-8").splitlines()
        for line in data:
            stripped = line.strip()
            if stripped and not stripped.startswith("#"):
                append_if_new(validators, stripped)
    for validator in validators:
        if not validator:
            raise ValueError("validator account identifier must be non-empty")
        if any(ch.isspace() for ch in validator):
            raise ValueError(
                f"invalid validator `{validator}` (whitespace is not allowed in encoded account identifiers)"
            )
        if "@" in validator:
            raise ValueError(
                f"invalid validator `{validator}` (expected encoded account identifier; @domain literals are rejected)"
            )
    return validators


def append_if_new(container: List[str], value: str) -> None:
    """Append value if not already present."""
    if value and value not in container:
        container.append(value)


def default_quorum(count: int) -> int:
    """Return a simple majority quorum."""
    return math.floor(count / 2) + 1


def normalize_protected_namespaces(namespaces: List[str], alias: str) -> List[str]:
    """Ensure at least one protected namespace entry is present."""
    cleaned = [ns for ns in (ns.strip() for ns in namespaces) if ns]
    if not cleaned:
        cleaned = [alias]
    return sorted(dict.fromkeys(cleaned))


def parse_metadata(pairs: List[str]) -> Dict[str, str]:
    """Parse key=value metadata pairs."""
    metadata: dict[str, str] = {}
    for pair in pairs:
        if "=" not in pair:
            raise ValueError(f"metadata entry `{pair}` must be in KEY=VALUE format")
        key, value = pair.split("=", 1)
        key = key.strip()
        value = value.strip()
        if not key:
            raise ValueError("metadata keys must not be blank")
        metadata[key] = value
    return metadata


def compute_dataspace_id(args: argparse.Namespace) -> int:
    """Derive dataspace id from explicit id, hash, or lane id fallback."""
    if args.dataspace_id is not None:
        return args.dataspace_id
    if args.dataspace_hash:
        digest = args.dataspace_hash.strip().lower()
        if digest.startswith("0x"):
            digest = digest[2:]
        if len(digest) != 64:
            raise ValueError("--dataspace-hash must contain 32 bytes (64 hex chars)")
        data = bytes.fromhex(digest)
        return int.from_bytes(data[:8], "little")
    return args.lane_id


def resolve_out_path(base: Path, override: Optional[str], default_name: str) -> Path:
    """Resolve an output path relative to the base directory."""
    if override:
        path = Path(override).expanduser()
        if not path.is_absolute():
            path = base / path
        return path.resolve()
    return (base / default_name).resolve()


def encode_space_directory_manifest(plan: LaneBootstrapPlan) -> None:
    """Invoke `cargo xtask space-directory encode` to produce the Norito manifest."""
    plan.space_directory_path.parent.mkdir(parents=True, exist_ok=True)
    cmd = [
        plan.cargo_bin,
        "xtask",
        "space-directory",
        "encode",
        "--json",
        str(plan.manifest_path),
        "--out",
        str(plan.space_directory_path),
    ]
    subprocess.run(cmd, check=True)


def format_shell_command(command: List[str]) -> str:
    """Return a shell-friendly representation of the provided command list."""
    return " ".join(shlex_quote(arg) for arg in command)


def shlex_quote(value: str) -> str:
    """Minimal POSIX shell escaping for command previews."""
    if not value:
        return "''"
    if re.fullmatch(r"[A-Za-z0-9_./:=+-]+", value):
        return value
    return "'" + value.replace("'", "'\"'\"'") + "'"


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # pylint: disable=broad-except
        print(f"error: {exc}", file=sys.stderr)
        sys.exit(1)
