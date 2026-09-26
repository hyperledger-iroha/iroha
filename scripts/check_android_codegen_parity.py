#!/usr/bin/env python3
"""Verify Android codegen manifests match the recorded metadata."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from copy import deepcopy
from pathlib import Path
from typing import Any, Dict, List, Optional


def _canonical_sha256(payload: Dict[str, Any]) -> str:
    normalized = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(normalized).hexdigest()


def _sorted_entries(entries: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
    return sorted(entries, key=lambda entry: entry.get("discriminant", ""))


def _load_json(path: Path) -> Dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise ValueError(f"invalid JSON in {path}: {exc}") from exc


def _logical_summary_path(
    path: Path,
    actual_root: Optional[Path],
    logical_root: Path,
) -> str:
    """Keep only the path within a replay root in comparable summaries."""

    if actual_root is None:
        return str(path)
    if not actual_root.is_dir():
        raise ValueError(f"summary root is not a directory: {actual_root}")
    try:
        relative = path.resolve(strict=True).relative_to(actual_root.resolve(strict=True))
    except (OSError, RuntimeError, ValueError) as exc:
        raise ValueError(f"{path} is outside summary root {actual_root}") from exc
    return (logical_root / relative).as_posix()


def _load_metadata(path: Path) -> Dict[str, Any]:
    metadata = _load_json(path)
    for key in ("instruction_manifest", "builder_index"):
        if key not in metadata:
            raise ValueError(f"metadata missing `{key}` block: {path}")
    return metadata


def build_codegen_metadata(
    manifest_payload: Dict[str, Any],
    builder_payload: Dict[str, Any],
) -> Dict[str, Any]:
    """Build deterministic metadata for the two Android codegen descriptors.

    The exporter records a wall-clock ``generated_at`` value. It is deliberately
    blanked before hashing and omitted from the checked-in metadata so two
    equivalent exporter runs produce byte-identical generated documentation.
    """

    manifest_entries = manifest_payload.get("instructions")
    builder_entries = builder_payload.get("builders")
    if not isinstance(manifest_entries, list):
        raise ValueError("instruction manifest missing `instructions` array")
    if not isinstance(builder_entries, list):
        raise ValueError("builder index missing `builders` array")

    manifest_canonical = deepcopy(manifest_payload)
    builder_canonical = deepcopy(builder_payload)
    manifest_canonical["instructions"] = _sorted_entries(manifest_entries)
    builder_canonical["builders"] = _sorted_entries(builder_entries)
    manifest_canonical["generated_at"] = ""
    builder_canonical["generated_at"] = ""

    return {
        "instruction_manifest": {
            "sha256": _canonical_sha256(manifest_canonical),
            "entry_count": len(manifest_entries),
        },
        "builder_index": {
            "sha256": _canonical_sha256(builder_canonical),
            "entry_count": len(builder_entries),
        },
    }


def _compare(actual: int | str, expected: Optional[int | str], label: str, errors: List[str]) -> None:
    if expected is None:
        return
    if actual != expected:
        errors.append(f"{label} mismatch: expected {expected}, got {actual}")


def _split_js_top_level(source: str, separator: str) -> List[str]:
    """Split a small declarative JS expression without splitting nested syntax."""

    parts: List[str] = []
    stack: List[str] = []
    quote: Optional[str] = None
    escaped = False
    start = 0
    pairs = {")": "(", "]": "[", "}": "{"}
    for index, char in enumerate(source):
        if quote is not None:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == quote:
                quote = None
            continue
        if char in ('"', "'", "`"):
            quote = char
        elif char in "([{":
            stack.append(char)
        elif char in ")]}":
            if not stack or stack.pop() != pairs[char]:
                raise ValueError("malformed JavaScript instruction binding expression")
        elif char == separator and not stack:
            parts.append(source[start:index].strip())
            start = index + 1
    if quote is not None or stack:
        raise ValueError("unterminated JavaScript instruction binding expression")
    parts.append(source[start:].strip())
    return parts


def _js_string_expression(source: str, name: str, resolving: Optional[set[str]] = None) -> str:
    """Resolve only literals, templates, identifiers, and string concatenation."""

    resolving = set() if resolving is None else resolving
    expression = name.strip()
    if expression.startswith("(") and expression.endswith(")"):
        try:
            enclosed = _split_js_top_level(expression[1:-1], ",")
            if len(enclosed) == 1:
                return _js_string_expression(source, enclosed[0], resolving)
        except ValueError:
            pass
    pieces = _split_js_top_level(expression, "+")
    if len(pieces) > 1:
        return "".join(_js_string_expression(source, piece, resolving) for piece in pieces)
    if expression.startswith('"') and expression.endswith('"'):
        value = json.loads(expression)
        if isinstance(value, str):
            return value
    if expression.startswith("`") and expression.endswith("`"):
        template = expression[1:-1]
        result = re.sub(
            r"\$\{([A-Z][A-Z0-9_]*)\}",
            lambda match: _js_string_expression(source, match.group(1), resolving),
            template,
        )
        if "${" not in result and "`" not in result:
            return result
    if re.fullmatch(r"[A-Z][A-Z0-9_]*", expression):
        if expression in resolving:
            raise ValueError(f"cyclic JavaScript instruction binding constant {expression}")
        declaration = re.findall(
            rf"(?m)^const {re.escape(expression)}\s*=\s*([^;]+);",
            source,
        )
        if len(declaration) != 1:
            raise ValueError(f"missing or repeated JS instruction constant {expression}")
        return _js_string_expression(source, declaration[0], resolving | {expression})
    raise ValueError(f"unsupported JS instruction binding expression for {name}: {expression}")


def _instruction_names_from_module(path: Path, family: str, namespace: str) -> List[str]:
    """Read the two imported declarative instruction-name inventories."""

    source = path.read_text(encoding="utf-8")
    names_symbol = f"{family}_INSTRUCTION_NAMES_V1"
    wire_symbol = f"{family}_INSTRUCTION_WIRE_IDS_V1"
    match = re.search(
        rf"export const {names_symbol} = Object\.freeze\(\s*\[([^\]]+)\]\s*\);",
        source,
    )
    if match is None:
        raise ValueError(f"{path} missing {names_symbol} inventory")
    names = json.loads("[" + re.sub(r",\s*$", "", match.group(1)) + "]")
    if not names or any(not isinstance(name, str) or not name for name in names):
        raise ValueError(f"{path} has invalid {names_symbol} inventory")
    wire_derivation = re.search(
        rf"export const {wire_symbol} = Object\.freeze\(\s*"
        rf"{names_symbol}\.map\(\s*\(?name\)?\s*=>\s*"
        rf"`iroha\.instruction\.v1::{namespace}::\$\{{name\}}`\s*\),?\s*\);",
        source,
    )
    if wire_derivation is None:
        raise ValueError(f"{path} does not derive {wire_symbol} from names")
    if len(names) != len(set(names)):
        raise ValueError(f"{path} repeats an instruction name")
    return names


def _extract_js_instruction_type_map(path: Path) -> Dict[str, str]:
    """Resolve every declared JS outer wire ID to its exact Rust inner type."""

    source = path.read_text(encoding="utf-8")
    if re.search(r"\bconst INNER_SCHEMA_HASH_BY_WIRE_ID\b", source):
        raise ValueError(f"{path} retains a separate instruction schema-hash map")
    marker = "const INNER_TYPE_NAME_BY_WIRE_ID = Object.freeze({"
    start = source.find(marker)
    end = source.find("\n});", start)
    if start < 0 or end < 0 or source.find(marker, start + 1) >= 0:
        raise ValueError(f"{path} missing or repeated INNER_TYPE_NAME_BY_WIRE_ID")
    body = source[start + len(marker) : end]
    projection = re.search(
        r"const INSTRUCTION_WIRE_SCHEMA_BINDINGS\s*=\s*"
        r"/\* @__PURE__ \*/\s*\(\(\)\s*=>\s*Object\.freeze\(\s*"
        r"Object\.entries\(INNER_TYPE_NAME_BY_WIRE_ID\)\.map\(\s*"
        r"\(\[outerWireId,\s*innerTypeName\]\)\s*=>\s*"
        r"Object\.freeze\(\{\s*outerWireId,\s*innerTypeName\s*\}\),?\s*"
        r"\),?\s*\)\)\(\);",
        source[end:],
    )
    if projection is None or "_instructionWireSchemaBindings: () => INSTRUCTION_WIRE_SCHEMA_BINDINGS" not in source:
        raise ValueError(f"{path} does not expose the source-derived instruction bindings")

    expected_record_id = "iroha.instruction.v1::bridge::RecordSccpMessage"
    record_id = _js_string_expression(source, "RECORD_SCCP_MESSAGE_WIRE_ID")
    if record_id != expected_record_id:
        raise ValueError(f"{path} changes RECORD_SCCP_MESSAGE_WIRE_ID")

    families = {
        "NFT_MARKET": (
            path.with_name("noritoNftMarketCodecs.js"), "nft_market"
        ),
        "GAME": (path.with_name("noritoGameRegistry.js"), "game"),
    }
    mapping: Dict[str, str] = {}
    rows = _split_js_top_level(body, ",")
    for index, row in enumerate(rows):
        if not row:
            if index != len(rows) - 1:
                raise ValueError(f"{path} has an empty JS instruction binding")
            continue
        if row.startswith("...Object.fromEntries("):
            for family, (module, namespace) in families.items():
                expected = (
                    f"...Object.fromEntries({family}_INSTRUCTION_NAMES_V1.map((name, index)"
                    f" => [{family}_INSTRUCTION_WIRE_IDS_V1[index], "
                    f"`${{TEXT_IROHA_DATA_MODEL_ISI}}{namespace}::${{name}}`]))"
                )
                if re.sub(r"\s+", "", row) == re.sub(r"\s+", "", expected):
                    for name in _instruction_names_from_module(module, family, namespace):
                        wire_id = f"iroha.instruction.v1::{namespace}::{name}"
                        if wire_id in mapping:
                            raise ValueError(f"{path} repeats JS instruction wire ID `{wire_id}`")
                        mapping[wire_id] = f"iroha_data_model::isi::{namespace}::{name}"
                    break
            else:
                raise ValueError(f"{path} has an unsupported JS instruction spread")
            continue
        parts = _split_js_top_level(row, ":")
        if len(parts) != 2:
            raise ValueError(f"{path} has a malformed JS instruction binding: {row}")
        key, value = parts
        if key.startswith("[") and key.endswith("]"):
            wire_id = _js_string_expression(source, key[1:-1])
        else:
            wire_id = _js_string_expression(source, key)
        type_name = _js_string_expression(source, value)
        if not wire_id or not type_name.startswith("iroha_data_model::isi::"):
            raise ValueError(f"{path} has a noncanonical JS instruction binding")
        if wire_id.startswith("iroha.instruction.v1::") and wire_id != (
            "iroha.instruction.v1::" + type_name.removeprefix("iroha_data_model::isi::")
        ):
            raise ValueError(f"{path} changes canonical outer/inner identity for `{wire_id}`")
        if wire_id.startswith("iroha_data_model::") and wire_id != type_name:
            raise ValueError(f"{path} maps Rust wire ID `{wire_id}` to a different type")
        if wire_id in mapping:
            raise ValueError(f"{path} repeats JS instruction wire ID `{wire_id}`")
        mapping[wire_id] = type_name
    if mapping.get(expected_record_id) != "iroha_data_model::isi::bridge::RecordSccpMessage":
        raise ValueError(f"{path} must bind RecordSccpMessage to its Rust inner type")
    return mapping


def _check_js_instruction_type_maps(
    manifest_path: Path,
    js_source_path: Path,
    errors: List[str],
    source_summary_path: Optional[str] = None,
) -> Dict[str, Any]:
    manifest_payload = _load_json(manifest_path)
    entries = manifest_payload.get("instructions")
    if not isinstance(entries, list):
        raise ValueError(f"{manifest_path} missing `instructions` array")
    manifest_entries: Dict[str, Dict[str, Any]] = {}
    for entry in entries:
        if not isinstance(entry, dict):
            raise ValueError(f"{manifest_path} contains a malformed instruction entry")
        wire_id = entry.get("discriminant")
        if not isinstance(wire_id, str) or not wire_id:
            raise ValueError(f"{manifest_path} contains an instruction without a wire ID")
        if wire_id in manifest_entries:
            raise ValueError(f"{manifest_path} repeats instruction wire ID `{wire_id}`")
        manifest_entries[wire_id] = entry

    source_map = _extract_js_instruction_type_map(js_source_path)
    matched = 0
    for wire_id, type_name in source_map.items():
        manifest_entry = manifest_entries.get(wire_id)
        if manifest_entry is None:
            # Android exports the Rust type name as a discriminant for some
            # instructions whose JavaScript outer ID has the V1 prefix.
            manifest_entry = manifest_entries.get(type_name)
        if manifest_entry is None:
            continue
        matched += 1
        if manifest_entry.get("type_name") != type_name:
            errors.append(
                "JavaScript instruction type-name mismatch for "
                f"{wire_id}: expected {manifest_entry.get('type_name')}, got {type_name}"
            )
        expected_hash = hashlib.sha256(
            b"norito:v1:type-name\0" + type_name.encode("utf-8")
        ).digest()[:16].hex()
        if manifest_entry.get("schema_hash") != expected_hash:
            errors.append(
                "JavaScript instruction schema-hash mismatch for "
                f"{wire_id}: expected {expected_hash}, got {manifest_entry.get('schema_hash')}"
            )

    return {
        "source_path": source_summary_path or str(js_source_path),
        "entry_count": len(source_map),
        "manifest_matched_entry_count": matched,
        "wire_binding_sha256": _canonical_sha256(source_map),
        "derived_from_type_names": True,
    }


def _build_summary(
    manifest_path: Path,
    builder_path: Path,
    metadata: Dict[str, Any],
    errors: List[str],
    manifest_summary_path: Optional[str] = None,
    builder_summary_path: Optional[str] = None,
) -> Dict[str, Any]:
    manifest_payload = _load_json(manifest_path)
    builder_payload = _load_json(builder_path)
    actual_metadata = build_codegen_metadata(manifest_payload, builder_payload)
    manifest_actual = actual_metadata["instruction_manifest"]
    builder_actual = actual_metadata["builder_index"]
    manifest_sha = manifest_actual["sha256"]
    builder_sha = builder_actual["sha256"]
    manifest_entry_count = manifest_actual["entry_count"]
    builder_entry_count = builder_actual["entry_count"]

    manifest_meta = metadata["instruction_manifest"]
    builder_meta = metadata["builder_index"]

    _compare(manifest_sha, manifest_meta.get("sha256"), "instruction_manifest sha256", errors)
    _compare(manifest_entry_count, manifest_meta.get("entry_count"), "instruction_manifest entry_count", errors)
    _compare(builder_sha, builder_meta.get("sha256"), "builder_index sha256", errors)
    _compare(builder_entry_count, builder_meta.get("entry_count"), "builder_index entry_count", errors)

    status = "ok" if not errors else "error"
    return {
        "status": status,
        "instruction_manifest": {
            "path": manifest_summary_path or str(manifest_path),
            "sha256": manifest_sha,
            "entry_count": manifest_entry_count,
            "expected_sha256": manifest_meta.get("sha256"),
            "expected_entry_count": manifest_meta.get("entry_count"),
        },
        "builder_index": {
            "path": builder_summary_path or str(builder_path),
            "sha256": builder_sha,
            "entry_count": builder_entry_count,
            "expected_sha256": builder_meta.get("sha256"),
            "expected_entry_count": builder_meta.get("entry_count"),
        },
        "errors": errors,
    }


def _write_summary(path: Path, summary: Dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(summary, indent=2), encoding="utf-8")


def parse_args(argv: Optional[List[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Check Android codegen manifests against recorded metadata"
    )
    parser.add_argument(
        "--manifest",
        type=Path,
        default=Path("target-codex/android_codegen/instruction_manifest.json"),
        help="Path to instruction_manifest.json (default: %(default)s)",
    )
    parser.add_argument(
        "--builder-index",
        type=Path,
        default=Path("target-codex/android_codegen/builder_index.json"),
        help="Path to builder_index.json (default: %(default)s)",
    )
    parser.add_argument(
        "--metadata",
        type=Path,
        default=Path("specs/sdk/android/generated/codegen_manifest_metadata.json"),
        help="Recorded metadata JSON (default: %(default)s)",
    )
    parser.add_argument(
        "--json-out",
        type=Path,
        help="Write a parity summary JSON file.",
    )
    parser.add_argument(
        "--codegen-root",
        type=Path,
        help="Actual replay codegen root; summarize paths relative to it.",
    )
    parser.add_argument(
        "--source-root",
        type=Path,
        help="Actual replay source root; summarize JS path relative to it.",
    )
    parser.add_argument(
        "--js-source",
        type=Path,
        default=Path("javascript/iroha_js/src/norito.js"),
        help="JavaScript Norito source to check (default: %(default)s)",
    )
    parser.add_argument(
        "--quiet",
        action="store_true",
        help="Suppress success output.",
    )
    return parser.parse_args(argv)


def main(argv: Optional[List[str]] = None) -> int:
    args = parse_args(argv)

    missing_paths = [
        path
        for path in (
            args.manifest,
            args.builder_index,
            args.metadata,
            args.js_source,
        )
        if not path.exists()
    ]
    if missing_paths:
        for missing in missing_paths:
            print(f"[android-codegen] missing required file: {missing}", file=sys.stderr)
        return 2

    errors: List[str] = []
    try:
        metadata = _load_metadata(args.metadata)
        manifest_summary_path = _logical_summary_path(
            args.manifest,
            args.codegen_root,
            Path("target-codex/android_codegen"),
        )
        builder_summary_path = _logical_summary_path(
            args.builder_index,
            args.codegen_root,
            Path("target-codex/android_codegen"),
        )
        source_summary_path = _logical_summary_path(
            args.js_source,
            args.source_root,
            Path("."),
        )
        summary = _build_summary(
            args.manifest,
            args.builder_index,
            metadata,
            errors,
            manifest_summary_path,
            builder_summary_path,
        )
        summary["javascript_instruction_schema_map"] = (
            _check_js_instruction_type_maps(
                args.manifest,
                args.js_source,
                errors,
                source_summary_path,
            )
        )
        summary["status"] = "ok" if not errors else "error"
        summary["errors"] = errors
    except ValueError as exc:
        print(f"[android-codegen] {exc}", file=sys.stderr)
        return 2

    if args.json_out is not None:
        _write_summary(args.json_out, summary)

    if summary["status"] == "ok":
        if not args.quiet:
            print(
                "[android-codegen] manifests match recorded metadata "
                f"(sha={summary['instruction_manifest']['sha256'][:12]}..., "
                f"entries={summary['instruction_manifest']['entry_count']})"
            )
        return 0

    print("[android-codegen] parity check failed:", file=sys.stderr)
    for error in summary["errors"]:
        print(f"  - {error}", file=sys.stderr)
    print("Re-run `make android-codegen-docs` and update the metadata/docs before retrying.", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
