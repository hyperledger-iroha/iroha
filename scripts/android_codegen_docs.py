#!/usr/bin/env python3
"""
Generate Markdown documentation from the Android Norito codegen manifests.

This script consumes the JSON artifacts emitted by norito_codegen_exporter and
produces human-readable references under docs/source/sdk/android/generated/.
Alongside the instruction/builder summaries it now emits a manifest catalog
that cross-references discriminants with their schema hashes and builder
metadata so Docs/DevRel can link a single index from the roadmap deliverables.
When invoked with `--locale` it additionally ensures the requested translation
stubs (for example `.ja.md` or `.he.md`) exist next to the English output so
localization stays in lockstep with the generated docs.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Iterable, List

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent

if str(SCRIPT_DIR) not in sys.path:
    sys.path.append(str(SCRIPT_DIR))

try:
    import sync_docs_i18n
except ModuleNotFoundError:  # pragma: no cover - tooling script should exist
    sync_docs_i18n = None

I18N_MANIFEST_PATH = REPO_ROOT / "docs" / "i18n" / "manifest.json"


def load_json(path: Path) -> dict:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def _blockquote(text: str) -> list[str]:
    stripped = text.strip()
    if not stripped:
        return []
    return [f"> {line}" if line else ">" for line in stripped.splitlines()]


def _render_struct_fields(fields: list[dict], is_tuple: bool) -> list[str]:
    lines = ["| Field | Type |", "|-------|------|"]
    for field in fields:
        name = field["name"]
        if is_tuple and name.startswith("_"):
            name = name[1:]
        lines.append(f"| `{name}` | `{field['type']}` |")
    return lines


def _render_enum_variants(variants: list[dict]) -> list[str]:
    lines = ["| Tag | Discriminant | Payload |", "|-----|--------------|---------|"]
    for variant in variants:
        payload = variant.get("payload")
        payload_cell = f"`{payload}`" if payload else "—"
        lines.append(
            f"| `{variant['tag']}` | {variant['discriminant']} | {payload_cell} |"
        )
    return lines


def _render_layout(layout: dict) -> Iterable[str]:
    kind = layout.get("kind")
    yield f"**Layout:** `{kind}`"
    yield ""
    if kind in {"struct", "tuple"}:
        fields = layout.get("fields", [])
        if fields:
            yield from _render_struct_fields(fields, kind == "tuple")
            yield ""
            return
    if kind == "enum":
        variants = layout.get("variants", [])
        if variants:
            yield from _render_enum_variants(variants)
            yield ""
            return
    # Fallback to JSON for unhandled shapes.
    yield "```json"
    yield json.dumps(layout, indent=2)
    yield "```"
    yield ""


def render_instructions(instructions: list[dict], out_path: Path) -> None:
    lines: list[str] = []
    lines.append("<!-- Auto-generated via scripts/android_codegen_docs.py -->")
    lines.append("# Android Instruction Reference")
    lines.append("")
    lines.append(
        "This file is generated from `instruction_manifest.json`. "
        "Do not edit manually."
    )
    lines.append("")

    for entry in sorted(instructions, key=lambda item: item["discriminant"]):
        disc = entry["discriminant"]
        type_name = entry["type_name"]
        lines.append(f"## `{disc}`")
        lines.append("")
        documentation = entry.get("documentation", "").strip()
        if documentation:
            lines.extend(_blockquote(documentation))
            lines.append("")
        lines.append(f"- Rust type: `{type_name}`")
        lines.append(f"- Schema hash: `{entry['schema_hash']}`")
        lines.append("")
        lines.extend(_render_layout(entry["layout"]))
        lines.extend(_render_manifest_tables(_gather_manifest_types(entry["layout"])))
        notes = SMART_CONTRACT_NOTES.get(type_name)
        if notes:
            lines.append("**Smart-contract notes:**")
            lines.append("")
            for note in notes:
                lines.append(f"- {note}")
            lines.append("")

    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text("\n".join(lines).rstrip() + "\n", encoding="utf-8")


def render_builders(builders: list[dict], out_path: Path) -> None:
    rows = []
    header = (
        "| Discriminant | Builder | Package | Stability | Features | Notes |\n"
        "|--------------|---------|---------|-----------|----------|-------|"
    )
    rows.append("<!-- Auto-generated via scripts/android_codegen_docs.py -->")
    rows.append("# Android Builder Reference")
    rows.append("")
    rows.append(
        "Information in this file is derived from `builder_index.json`. "
        "Do not edit manually."
    )
    rows.append("")
    rows.append(header)

    for entry in sorted(builders, key=lambda item: item["discriminant"]):
        features = ", ".join(entry.get("feature_gates", [])) or "—"
        notes = entry.get("notes", "—")
        rows.append(
            f"| `{entry['discriminant']}` "
            f"| `{entry['builder_name']}` "
            f"| `{entry['package']}` "
            f"| `{entry['stability']}` "
            f"| {features} "
            f"| {notes} |"
        )

    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text("\n".join(rows).rstrip() + "\n", encoding="utf-8")


def _builder_map(builders: list[dict]) -> dict[str, list[dict]]:
    mapping: dict[str, list[dict]] = {}
    for entry in builders:
        disc = entry.get("discriminant")
        if not disc:
            continue
        bucket = mapping.setdefault(disc, [])
        bucket.append(entry)
    for bucket in mapping.values():
        bucket.sort(key=lambda item: item.get("builder_name", ""))
    return mapping


def _format_builder_cell(entries: list[dict]) -> str:
    if not entries:
        return "—"
    formatted: list[str] = []
    for entry in entries:
        name = entry.get("builder_name", "—")
        package = entry.get("package", "—")
        stability = entry.get("stability", "—")
        features = ", ".join(entry.get("feature_gates", [])) or "—"
        notes = entry.get("notes")
        note_suffix = "" if not notes or notes == "—" else f"; notes: {notes}"
        formatted.append(
            f"`{name}` (`{package}`; {stability}; features: {features}{note_suffix})"
        )
    return "<br />".join(formatted)


def render_manifest_catalog(
    instructions: list[dict], builders: list[dict], out_path: Path
) -> None:
    lines: list[str] = []
    lines.append("<!-- Auto-generated via scripts/android_codegen_docs.py -->")
    lines.append("# Android Norito Manifest Catalog")
    lines.append("")
    lines.append(
        "This catalog cross-references `instruction_manifest.json` and "
        "`builder_index.json`. Do not edit manually."
    )
    lines.append("")
    lines.append(
        "| Discriminant | Rust type | Schema hash | Builders |\n"
        "|--------------|-----------|-------------|----------|"
    )

    by_discriminant = _builder_map(builders)

    for entry in sorted(instructions, key=lambda item: item.get("discriminant", "")):
        discriminant = entry.get("discriminant", "—")
        type_name = entry.get("type_name", "—")
        schema_hash = entry.get("schema_hash", "—")
        builder_cell = _format_builder_cell(by_discriminant.get(discriminant, []))
        lines.append(
            f"| `{discriminant}` | `{type_name}` | `{schema_hash}` | {builder_cell} |"
        )

    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text("\n".join(lines).rstrip() + "\n", encoding="utf-8")


MANIFEST_TYPE_TABLES: dict[str, dict] = {
    "PinPolicy": {
        "kind": "struct",
        "description": "Storage replication policy negotiated with the pin registry.",
        "fields": [
            {
                "name": "min_replicas",
                "type": "u16",
                "description": "Minimum replica count required by governance.",
            },
            {
                "name": "storage_class",
                "type": "StorageClass",
                "description": "Requested storage tier (`Hot`, `Warm`, `Cold`).",
            },
            {
                "name": "retention_epoch",
                "type": "u64",
                "description": "Inclusive epoch through which the manifest must remain pinned.",
            },
        ],
        "dependencies": ["StorageClass"],
    },
    "ContractManifest": {
        "kind": "struct",
        "description": "Optional metadata attached to smart-contract deployments; hash fields must match the canonical host-computed values before admission.",
        "fields": [
            {
                "name": "code_hash",
                "type": "Option<Hash>",
                "description": "Blake2b-32 digest of the `.to` program body (bytes after the IVM header).",
            },
            {
                "name": "abi_hash",
                "type": "Option<Hash>",
                "description": "Hash of the syscall/pointer ABI surface for the supplied `abi_version` (see `docs/source/ivm_header.md`).",
            },
            {
                "name": "compiler_fingerprint",
                "type": "Option<String>",
                "description": "Compiler + toolchain note recorded for provenance.",
            },
            {
                "name": "features_bitmap",
                "type": "Option<u64>",
                "description": "Bitmask of build features (SIMD, CUDA, etc.).",
            },
            {
                "name": "access_set_hints",
                "type": "Option<AccessSetHints>",
                "description": "Advisory read/write key hints for the scheduler.",
            },
            {
                "name": "entrypoints",
                "type": "Option<Vec<EntrypointDescriptor>>",
                "description": "Optional entrypoint descriptors advertised by the compiler.",
            },
        ],
        "dependencies": ["AccessSetHints", "EntrypointDescriptor"],
    },
    "AccessSetHints": {
        "kind": "struct",
        "description": "Declarative read/write key hints stored inside smart-contract manifests.",
        "fields": [
            {
                "name": "read_keys",
                "type": "Vec<String>",
                "description": "Canonical keys (e.g., `account:6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9`) the contract expects to read.",
            },
            {
                "name": "write_keys",
                "type": "Vec<String>",
                "description": "Keys that the contract expects to write during execution.",
            },
        ],
    },
    "EntrypointDescriptor": {
        "kind": "struct",
        "description": "Metadata emitted per Kotodama entrypoint.",
        "fields": [
            {
                "name": "name",
                "type": "String",
                "description": "Symbol name declared in Kotodama source.",
            },
            {
                "name": "kind",
                "type": "EntryPointKind",
                "description": "Role of the entrypoint (`Public`, `Hajimari`, or `Kaizen`).",
            },
            {
                "name": "permission",
                "type": "Option<String>",
                "description": "Optional dispatcher permission required before invocation.",
            },
            {
                "name": "read_keys",
                "type": "Vec<String>",
                "description": "Advisory read set scoped to the entrypoint.",
            },
            {
                "name": "write_keys",
                "type": "Vec<String>",
                "description": "Advisory write set scoped to the entrypoint.",
            },
            {
                "name": "access_hints_complete",
                "type": "Option<bool>",
                "description": "Whether access-set hints are complete or explicitly provided.",
            },
            {
                "name": "access_hints_skipped",
                "type": "Vec<String>",
                "description": "Reasons access hints were skipped for this entrypoint.",
            },
            {
                "name": "triggers",
                "type": "Vec<TriggerDescriptor>",
                "description": "Trigger declarations that call this entrypoint.",
            },
        ],
        "dependencies": ["TriggerDescriptor"],
    },
    "TriggerCallback": {
        "kind": "struct",
        "description": "Entrypoint callback target referenced by a trigger declaration.",
        "fields": [
            {
                "name": "namespace",
                "type": "Option<String>",
                "description": "Optional contract namespace for cross-contract callbacks.",
            },
            {
                "name": "entrypoint",
                "type": "String",
                "description": "Entrypoint name to invoke.",
            },
        ],
    },
    "TriggerDescriptor": {
        "kind": "struct",
        "description": "Declarative trigger metadata attached to an entrypoint.",
        "fields": [
            {
                "name": "id",
                "type": "TriggerId",
                "description": "Trigger identifier.",
            },
            {
                "name": "repeats",
                "type": "Repeats",
                "description": "Repeat policy for the trigger action.",
            },
            {
                "name": "filter",
                "type": "EventFilterBox",
                "description": "Event filter that drives execution.",
            },
            {
                "name": "authority",
                "type": "Option<AccountId>",
                "description": "Optional explicit authority override.",
            },
            {
                "name": "metadata",
                "type": "Metadata",
                "description": "Trigger metadata payload (JSON map).",
            },
            {
                "name": "callback",
                "type": "TriggerCallback",
                "description": "Callback target for this trigger.",
            },
        ],
        "dependencies": ["TriggerCallback"],
    },
    "ChunkerProfileHandle": {
        "kind": "struct",
        "description": "Registry descriptor describing the chunking profile used to build the CAR.",
        "fields": [
            {
                "name": "profile_id",
                "type": "u32",
                "description": "Numeric profile identifier (`ProfileId`).",
            },
            {
                "name": "namespace",
                "type": "String",
                "description": "Registry namespace (typically `sorafs`).",
            },
            {
                "name": "name",
                "type": "String",
                "description": "Human-readable profile name (for example `sf1`).",
            },
            {
                "name": "semver",
                "type": "String",
                "description": "Semantic version string of the parameter set.",
            },
            {
                "name": "multihash_code",
                "type": "u64",
                "description": "Multihash code used when deriving chunk digests.",
            },
        ],
    },
    "ManifestAliasBinding": {
        "kind": "struct",
        "description": "Alias binding payload approved alongside a manifest.",
        "fields": [
            {"name": "name", "type": "String", "description": "Alias label (e.g., `docs`)."},
            {
                "name": "namespace",
                "type": "String",
                "description": "Alias namespace (e.g., `sora`).",
            },
            {
                "name": "proof",
                "type": "Vec<u8>",
                "description": "Norito-encoded alias proof bytes (base64 in JSON).",
            },
        ],
    },
    "StorageClass": {
        "kind": "enum",
        "description": "Storage tier classification for `SoraFS` replicas.",
        "variants": [
            {"name": "Hot", "description": "Low-latency replicas for developer workflows."},
            {"name": "Warm", "description": "Cost-optimised replicas with relaxed latency."},
            {"name": "Cold", "description": "Archival replicas retained for compliance."},
        ],
    },
}

SMART_CONTRACT_FIXTURE = (
    "docs/source/sdk/android/generated/fixtures/"
    "smart_contract_code_executor_hashes.json"
)

SMART_CONTRACT_NOTES: dict[str, list[str]] = {
    "iroha_data_model::isi::smart_contract_code::RegisterSmartContractCode": [
        "Nodes recompute `manifest.code_hash` from the `.to` artifact and reject mismatches; `manifest.abi_hash` must equal the canonical ABI digest for the declared version.",
        f"Sample hash pair derived from `defaults/executor.to` lives in `{SMART_CONTRACT_FIXTURE}` for deterministic builder tests.",
    ],
    "iroha_data_model::isi::smart_contract_code::RegisterSmartContractBytes": [
        "`code_hash` must equal the Blake2b-32 digest of the program body (bytes after the IVM header); duplicate uploads re-use the stored bytes.",
        f"Use the hashes in `{SMART_CONTRACT_FIXTURE}` to verify `.to` parsing logic in automation.",
    ],
    "iroha_data_model::isi::smart_contract_code::ActivateContractInstance": [
        "Requires manifests and bytecode to exist for the supplied `code_hash`; activation binds `(namespace, contract_id)` to that digest.",
        "Protected namespaces continue to enforce governance approval, so Android SDKs should surface deterministic errors when admission fails.",
    ],
    "iroha_data_model::isi::smart_contract_code::RemoveSmartContractBytes": [
        "Removal succeeds only when no manifest or active instance references the target `code_hash`; provide an audit reason when automating removals.",
    ],
}


def _canonical_type_name(type_name: str) -> str:
    """Extract the base type from generic wrappers like Option<Vec<_>>."""

    name = type_name.strip()
    changed = True

    def strip_wrapper(value: str, prefix: str, suffix: str = ">") -> tuple[str, bool]:
        if value.startswith(prefix) and value.endswith(suffix):
            inner = value[len(prefix) : -len(suffix)]
            return inner.strip(), True
        return value, False

    while changed:
        changed = False
        for wrapper in ("Option<", "Vec<", "Box<"):
            name, stripped = strip_wrapper(name, wrapper)
            if stripped:
                changed = True
        if name.startswith("Array<") and name.endswith(">"):
            inner = name[len("Array<") : -1]
            comma = inner.find(",")
            if comma != -1:
                name = inner[:comma].strip()
                changed = True
    return name


def _gather_manifest_types(layout: dict) -> list[str]:
    if layout.get("kind") != "struct":
        return []

    discovered: list[str] = []
    seen: set[str] = set()

    def add_type(type_name: str) -> None:
        if type_name in seen:
            return
        meta = MANIFEST_TYPE_TABLES.get(type_name)
        if not meta:
            return
        seen.add(type_name)
        discovered.append(type_name)
        for dep in meta.get("dependencies", []):
            add_type(dep)

    for field in layout.get("fields", []):
        base = _canonical_type_name(field["type"])
        add_type(base)

    return discovered


def _render_manifest_tables(type_names: list[str]) -> list[str]:
    if not type_names:
        return []

    lines: list[str] = []
    lines.append("### Manifest field details")
    lines.append("")

    for type_name in type_names:
        meta = MANIFEST_TYPE_TABLES[type_name]
        title_suffix = "variants" if meta["kind"] == "enum" else "fields"
        lines.append(f"#### {type_name} {title_suffix}")
        lines.append("")
        description = meta.get("description")
        if description:
            lines.append(description)
            lines.append("")
        if meta["kind"] == "enum":
            lines.append("| Variant | Description |")
            lines.append("|---------|-------------|")
            for variant in meta.get("variants", []):
                lines.append(f"| `{variant['name']}` | {variant['description']} |")
            lines.append("")
            continue
        lines.append("| Field | Type | Description |")
        lines.append("|-------|------|-------------|")
        for field in meta.get("fields", []):
            lines.append(
                f"| `{field['name']}` | `{field['type']}` | {field['description']} |"
            )
        lines.append("")

    return lines


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Render Markdown docs from Android codegen manifests"
    )
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--builders", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument(
        "--locale",
        dest="locales",
        action="append",
        default=[],
        help=(
            "Locale code to generate translation stubs for. "
            "May be specified multiple times (e.g., --locale ja --locale he)."
        ),
    )
    args = parser.parse_args()

    manifest = load_json(args.manifest)
    builder_index = load_json(args.builders)

    output_dir = args.out.resolve()
    instructions_path = output_dir / "instructions.md"
    builders_path = output_dir / "builders.md"
    catalog_path = output_dir / "manifest_catalog.md"

    render_instructions(manifest.get("instructions", []), instructions_path)
    render_builders(builder_index.get("builders", []), builders_path)
    render_manifest_catalog(
        manifest.get("instructions", []),
        builder_index.get("builders", []),
        catalog_path,
    )
    ensure_locale_stubs(
        [instructions_path, builders_path, catalog_path], args.locales or []
    )


def ensure_locale_stubs(out_paths: List[Path], locales: List[str]) -> None:
    """Ensure requested locale stubs exist next to generated docs."""

    normalized = sorted({code.strip() for code in locales if code})
    if not normalized:
        return

    if sync_docs_i18n is None:
        raise RuntimeError(
            "sync_docs_i18n.py is unavailable; cannot generate locale stubs"
        )

    manifest = sync_docs_i18n.Manifest.load(I18N_MANIFEST_PATH)
    lang_map = {lang.code: lang for lang in manifest.targets}
    unknown = [code for code in normalized if code not in lang_map]
    if unknown:
        raise ValueError(
            f"Unknown locale code(s) requested: {', '.join(sorted(unknown))}"
        )

    for out_path in out_paths:
        rel = out_path.relative_to(REPO_ROOT)
        for code in normalized:
            lang = lang_map[code]
            translation_rel = sync_docs_i18n.compute_translation_path(rel, code)
            translation_abs = REPO_ROOT / translation_rel
            action = sync_docs_i18n.ensure_stub(
                out_path, translation_abs, lang, dry_run=False
            )
            if action == "create":
                print(f"[i18n] created {translation_rel.as_posix()}")
            elif action == "update":
                print(f"[i18n] updated {translation_rel.as_posix()}")


if __name__ == "__main__":
    main()
