"""Dataspace planning helpers for SDK users.

The helpers in this module are intentionally pure Python: they generate the
manifest, config snippet, and rollout summary needed to add a Nexus dataspace,
but they do not mutate a node or shell out to cargo.
"""

from __future__ import annotations

import json
import math
import re
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, Iterable, List, Mapping, Optional

__all__ = [
    "RuntimeUpgradeHook",
    "DataspaceSpec",
    "DataspacePlan",
    "DataspaceStatus",
    "default_quorum",
    "normalize_dataspace_hash",
    "compute_dataspace_id",
    "compute_dataspace_manifest_hash",
    "normalize_protected_namespaces",
    "slugify_alias",
    "plan_dataspace",
    "write_dataspace_plan",
]


@dataclass(frozen=True)
class RuntimeUpgradeHook:
    """Optional runtime-upgrade hook metadata embedded in a dataspace manifest."""

    metadata_key: str
    require_metadata: bool = False
    allow: bool = True
    allowed_ids: List[str] = field(default_factory=list)


@dataclass(frozen=True)
class DataspaceSpec:
    """Developer-facing inputs for a Nexus dataspace/lane rollout."""

    dataspace_alias: str
    lane_alias: str
    lane_id: int
    governance_module: str
    settlement_handle: str
    validators: List[str]
    dataspace_id: Optional[int] = None
    dataspace_hash: Optional[str] = None
    dataspace_description: Optional[str] = None
    lane_description: Optional[str] = None
    visibility: str = "public"
    lane_type: str = "default_public"
    storage_profile: str = "full_replica"
    quorum: Optional[int] = None
    protected_namespaces: List[str] = field(default_factory=list)
    metadata: Dict[str, str] = field(default_factory=dict)
    route_instructions: List[str] = field(default_factory=list)
    route_accounts: List[str] = field(default_factory=list)
    runtime_upgrade: Optional[RuntimeUpgradeHook] = None
    manifest_name: Optional[str] = None
    catalog_name: Optional[str] = None
    summary_name: Optional[str] = None
    space_directory_name: Optional[str] = None


@dataclass(frozen=True)
class DataspacePlan:
    """Concrete dataspace artifacts ready to write or publish."""

    slug: str
    kura_segment: str
    merge_segment: str
    manifest: Mapping[str, Any]
    catalog_snippet: str
    summary: Mapping[str, Any]
    manifest_name: str
    catalog_name: str
    summary_name: str
    space_directory_name: str


@dataclass(frozen=True)
class DataspaceStatus:
    """Readiness view derived from Torii status lane governance entries."""

    alias: str
    dataspace_id: int
    lane_id: int
    found: bool
    ready: bool
    manifest_required: bool
    manifest_ready: bool
    sealed: bool
    lane: Mapping[str, Any] = field(default_factory=dict)


def _require_non_empty(value: str, context: str) -> str:
    text = str(value or "").strip()
    if not text:
        raise ValueError(f"{context} must be non-empty")
    return text


def _toml_string(value: Any) -> str:
    return json.dumps(str(value), ensure_ascii=False)


def _artifact_name(value: str, context: str) -> str:
    text = _require_non_empty(value, context)
    if "\\" in text:
        raise ValueError(f"{context} must be a file name, not a path")
    path = Path(text)
    if path.is_absolute() or len(path.parts) != 1 or path.name != text:
        raise ValueError(f"{context} must be a file name, not a path")
    return text


def _dedupe(values: Iterable[str]) -> List[str]:
    deduped: List[str] = []
    for value in values:
        text = str(value or "").strip()
        if text and text not in deduped:
            deduped.append(text)
    return deduped


def slugify_alias(alias: str, lane_id: int) -> str:
    """Convert a human alias into a filesystem-friendly slug."""

    normalized = re.sub(r"[^a-z0-9]+", "_", str(alias).lower()).strip("_")
    return normalized or f"lane{int(lane_id):03d}"


def default_quorum(count: int) -> int:
    """Return a simple-majority quorum for ``count`` validators."""

    parsed = int(count)
    if parsed < 1:
        raise ValueError("validator count must be positive")
    return math.floor(parsed / 2) + 1


def normalize_protected_namespaces(namespaces: Iterable[str], alias: str) -> List[str]:
    """Ensure at least one protected namespace entry is present."""

    cleaned = _dedupe(namespaces)
    if not cleaned:
        cleaned = [_require_non_empty(alias, "alias")]
    return sorted(cleaned)


def normalize_dataspace_hash(raw_hash: str) -> str:
    """Normalize a dataspace manifest hash to lowercase 32-byte hex."""

    digest = str(raw_hash or "").strip().lower()
    if digest.startswith("0x"):
        digest = digest[2:]
    if len(digest) != 64:
        raise ValueError("dataspace_hash must contain 32 bytes (64 hex chars)")
    bytes.fromhex(digest)
    return digest


def compute_dataspace_id(
    *,
    dataspace_id: Optional[int] = None,
    dataspace_hash: Optional[str] = None,
    lane_id: int,
) -> int:
    """Derive a dataspace id from an explicit id, manifest hash, or lane id."""

    if dataspace_id is not None:
        parsed = int(dataspace_id)
        if parsed < 0:
            raise ValueError("dataspace_id must be non-negative")
        return parsed
    if dataspace_hash:
        data = bytes.fromhex(normalize_dataspace_hash(dataspace_hash))
        return int.from_bytes(data[:8], "little")
    parsed_lane_id = int(lane_id)
    if parsed_lane_id < 0:
        raise ValueError("lane_id must be non-negative")
    return parsed_lane_id


def compute_dataspace_manifest_hash(
    *,
    dataspace_hash: Optional[str] = None,
    dataspace_id: int,
) -> str:
    """Return the canonical 32-byte manifest hash for catalog snippets."""

    if dataspace_hash:
        return normalize_dataspace_hash(dataspace_hash)
    parsed_id = int(dataspace_id)
    if parsed_id < 0:
        raise ValueError("dataspace_id must be non-negative")
    return parsed_id.to_bytes(8, "little").hex() + ("00" * 24)


def _validate_spec(spec: DataspaceSpec) -> None:
    _require_non_empty(spec.dataspace_alias, "dataspace_alias")
    _require_non_empty(spec.lane_alias, "lane_alias")
    _require_non_empty(spec.governance_module, "governance_module")
    _require_non_empty(spec.settlement_handle, "settlement_handle")
    if int(spec.lane_id) < 0:
        raise ValueError("lane_id must be non-negative")
    validators = _dedupe(spec.validators)
    if not validators:
        raise ValueError("validators must contain at least one account id")
    for validator in validators:
        if any(ch.isspace() for ch in validator):
            raise ValueError(f"validator `{validator}` must not contain whitespace")
        if "@" in validator:
            raise ValueError(
                f"validator `{validator}` must be an encoded account identifier"
            )
    quorum = spec.quorum if spec.quorum is not None else default_quorum(len(validators))
    if quorum < 1 or quorum > len(validators):
        raise ValueError("quorum must be between 1 and the number of validators")


def _manifest(spec: DataspaceSpec, validators: List[str], quorum: int) -> Dict[str, Any]:
    manifest: Dict[str, Any] = {
        "lane": spec.lane_alias,
        "governance": spec.governance_module,
        "version": 1,
        "validators": validators,
        "quorum": quorum,
        "protected_namespaces": normalize_protected_namespaces(
            spec.protected_namespaces,
            spec.lane_alias,
        ),
    }
    if spec.runtime_upgrade is not None:
        manifest["hooks"] = {
            "runtime_upgrade": {
                "allow": bool(spec.runtime_upgrade.allow),
                "require_metadata": bool(spec.runtime_upgrade.require_metadata),
                "metadata_key": spec.runtime_upgrade.metadata_key,
                "allowed_ids": sorted(_dedupe(spec.runtime_upgrade.allowed_ids)),
            }
        }
    return manifest


def _catalog_snippet(
    spec: DataspaceSpec,
    *,
    slug: str,
    dataspace_id: int,
    manifest_hash: str,
) -> str:
    timestamp = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    lines: List[str] = [
        f"# Generated by iroha_python.plan_dataspace on {timestamp}",
        f"# Lane alias '{spec.lane_alias}' (id {int(spec.lane_id)})",
        "",
        "[[nexus.lane_catalog]]",
        f"index = {int(spec.lane_id)}",
        f"alias = {_toml_string(spec.lane_alias)}",
        f"dataspace = {_toml_string(spec.dataspace_alias)}",
    ]
    if spec.lane_description:
        lines.append(f"description = {_toml_string(spec.lane_description)}")
    lines.extend(
        [
            f"visibility = {_toml_string(spec.visibility)}",
            f"lane_type = {_toml_string(spec.lane_type)}",
            f"governance = {_toml_string(spec.governance_module)}",
            f"settlement = {_toml_string(spec.settlement_handle)}",
            f"storage = {_toml_string(spec.storage_profile)}",
        ]
    )
    if spec.metadata:
        lines.append("[nexus.lane_catalog.metadata]")
        for key in sorted(spec.metadata):
            lines.append(f"{_toml_string(key)} = {_toml_string(spec.metadata[key])}")
    lines.extend(
        [
            "",
            "[[nexus.dataspace_catalog]]",
            f"alias = {_toml_string(spec.dataspace_alias)}",
            f"id = {dataspace_id}",
            f"manifest_hash = {_toml_string(manifest_hash)}",
        ]
    )
    if spec.dataspace_description:
        lines.append(f"description = {_toml_string(spec.dataspace_description)}")
    lines.append("")
    for instruction in spec.route_instructions:
        lines.extend(
            [
                "[[nexus.routing_policy.rules]]",
                f"lane = {int(spec.lane_id)}",
                f"dataspace = {_toml_string(spec.dataspace_alias)}",
                "[nexus.routing_policy.rules.matcher]",
                f"instruction = {_toml_string(instruction)}",
                "description = "
                f"{_toml_string(f'Route instruction {instruction} to lane {spec.lane_alias}')}",
                "",
            ]
        )
    for account in spec.route_accounts:
        lines.extend(
            [
                "[[nexus.routing_policy.rules]]",
                f"lane = {int(spec.lane_id)}",
                f"dataspace = {_toml_string(spec.dataspace_alias)}",
                "[nexus.routing_policy.rules.matcher]",
                f"account = {_toml_string(account)}",
                "description = "
                f"{_toml_string(f'Route account {account} to lane {spec.lane_alias}')}",
                "",
            ]
        )
    return "\n".join(lines).rstrip() + "\n"


def _summary(
    spec: DataspaceSpec,
    *,
    slug: str,
    dataspace_id: int,
    manifest_hash: str,
    manifest_name: str,
    catalog_name: str,
    summary_name: str,
    space_directory_name: str,
    validators: List[str],
    quorum: int,
) -> Dict[str, Any]:
    encode_command = [
        "cargo",
        "xtask",
        "space-directory",
        "encode",
        "--json",
        manifest_name,
        "--out",
        space_directory_name,
    ]
    return {
        "lane_id": int(spec.lane_id),
        "lane_alias": spec.lane_alias,
        "dataspace_alias": spec.dataspace_alias,
        "dataspace_id": dataspace_id,
        "dataspace_manifest_hash": manifest_hash,
        "slug": slug,
        "kura_segment": f"lane_{int(spec.lane_id):03d}_{slug}",
        "merge_segment": f"lane_{int(spec.lane_id):03d}_merge",
        "manifest_path": manifest_name,
        "catalog_snippet_path": catalog_name,
        "summary_path": summary_name,
        "space_directory_manifest_to": space_directory_name,
        "space_directory_encode": {
            "command": encode_command,
            "executed": False,
        },
        "governance_module": spec.governance_module,
        "settlement_handle": spec.settlement_handle,
        "visibility": spec.visibility,
        "lane_type": spec.lane_type,
        "storage_profile": spec.storage_profile,
        "validators": validators,
        "quorum": quorum,
        "protected_namespaces": normalize_protected_namespaces(
            spec.protected_namespaces,
            spec.lane_alias,
        ),
        "metadata": dict(spec.metadata),
        "routing": {
            "instructions": list(spec.route_instructions),
            "accounts": list(spec.route_accounts),
        },
        "runtime_upgrade": None
        if spec.runtime_upgrade is None
        else {
            "metadata_key": spec.runtime_upgrade.metadata_key,
            "require_metadata": bool(spec.runtime_upgrade.require_metadata),
            "allowed_ids": sorted(_dedupe(spec.runtime_upgrade.allowed_ids)),
        },
        "next_steps": [
            "Copy the manifest into the configured `nexus.registry.manifest_directory`.",
            "Append the catalog snippet to your Nexus config and bump `[nexus] lane_count`.",
            "Encode the manifest with the listed cargo command before publishing it.",
            "Restart irohad with `--trace-config` and verify dataspace readiness.",
        ],
    }


def plan_dataspace(spec: DataspaceSpec) -> DataspacePlan:
    """Build manifest/config artifacts for a dataspace without writing files."""

    _validate_spec(spec)
    validators = _dedupe(spec.validators)
    quorum = spec.quorum if spec.quorum is not None else default_quorum(len(validators))
    slug = slugify_alias(spec.lane_alias, spec.lane_id)
    dataspace_id = compute_dataspace_id(
        dataspace_id=spec.dataspace_id,
        dataspace_hash=spec.dataspace_hash,
        lane_id=spec.lane_id,
    )
    manifest_hash = compute_dataspace_manifest_hash(
        dataspace_hash=spec.dataspace_hash,
        dataspace_id=dataspace_id,
    )
    manifest_name = _artifact_name(
        spec.manifest_name or f"{slug}.manifest.json",
        "manifest_name",
    )
    catalog_name = _artifact_name(
        spec.catalog_name or f"{slug}.catalog.toml",
        "catalog_name",
    )
    summary_name = _artifact_name(
        spec.summary_name or f"{slug}.summary.json",
        "summary_name",
    )
    space_directory_name = _artifact_name(
        spec.space_directory_name or f"{slug}.manifest.to",
        "space_directory_name",
    )
    return DataspacePlan(
        slug=slug,
        kura_segment=f"lane_{int(spec.lane_id):03d}_{slug}",
        merge_segment=f"lane_{int(spec.lane_id):03d}_merge",
        manifest=_manifest(spec, validators, quorum),
        catalog_snippet=_catalog_snippet(
            spec,
            slug=slug,
            dataspace_id=dataspace_id,
            manifest_hash=manifest_hash,
        ),
        summary=_summary(
            spec,
            slug=slug,
            dataspace_id=dataspace_id,
            manifest_hash=manifest_hash,
            manifest_name=manifest_name,
            catalog_name=catalog_name,
            summary_name=summary_name,
            space_directory_name=space_directory_name,
            validators=validators,
            quorum=quorum,
        ),
        manifest_name=manifest_name,
        catalog_name=catalog_name,
        summary_name=summary_name,
        space_directory_name=space_directory_name,
    )


def write_dataspace_plan(
    plan: DataspacePlan,
    output_dir: str | Path,
    *,
    force: bool = False,
) -> Dict[str, Path]:
    """Write a dataspace plan and return the generated paths."""

    base = Path(output_dir).expanduser().resolve()
    names = {
        "manifest": _artifact_name(plan.manifest_name, "manifest_name"),
        "catalog": _artifact_name(plan.catalog_name, "catalog_name"),
        "summary": _artifact_name(plan.summary_name, "summary_name"),
    }
    outputs = {
        "manifest": base / names["manifest"],
        "catalog": base / names["catalog"],
        "summary": base / names["summary"],
    }
    for path in outputs.values():
        if path.exists() and not force:
            raise FileExistsError(f"{path} already exists")
    base.mkdir(parents=True, exist_ok=True)
    outputs["manifest"].write_text(
        json.dumps(plan.manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    outputs["catalog"].write_text(plan.catalog_snippet, encoding="utf-8")
    outputs["summary"].write_text(
        json.dumps(plan.summary, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return outputs
