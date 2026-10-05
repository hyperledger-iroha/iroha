#!/usr/bin/env python3
"""Regenerate the closed Python Kagemusha release projection from Torii OpenAPI.

This derives SDK JSON shapes only. Native governance admission owns evidence,
release identities and authorization. No SDK module is imported by this producer.
"""
from __future__ import annotations

import argparse
import ast
import hashlib
import json
from pathlib import Path
import pprint

ROOT = Path(__file__).resolve().parents[1]
OPENAPI_PATHS = (
    Path("artifacts/openapi/torii.json"),
    Path("artifacts/openapi/versions/current/torii.json"),
    Path("crates/iroha_torii/assets/openapi/torii.json"),
)
CONSUMER = Path("python/iroha_torii_client/governance_kagemusha_release_schema_v1.py")
SCHEMA_ROOTS = (
    "GovernanceKagemushaGovernedVerifierRegistryV1",
    "GovernanceKagemushaReleaseManifestV1",
    "GovernanceKagemushaInternalValidationReceiptV1",
    "GovernanceKagemushaReleaseAttestationV1",
)


def _unique_keys(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("OpenAPI contains a duplicate object key")
        result[key] = value
    return result


def schema_projection(specification):
    schemas = specification["components"]["schemas"]
    closure = set(SCHEMA_ROOTS)

    def visit(node):
        if isinstance(node, dict):
            reference = node.get("$ref")
            if reference is not None:
                prefix = "#/components/schemas/"
                if not isinstance(reference, str) or not reference.startswith(prefix):
                    raise ValueError("release schema has a nonlocal reference")
                name = reference[len(prefix):]
                if name not in schemas:
                    raise ValueError("release schema has a missing reference")
                if name not in closure:
                    closure.add(name)
                    visit(schemas[name])
            for child in node.values():
                visit(child)
        elif isinstance(node, list):
            for child in node:
                visit(child)

    def project(node):
        if isinstance(node, dict):
            return {key: project(child) for key, child in node.items()
                    if key not in {"description", "title", "example"}}
        if isinstance(node, list):
            return [project(child) for child in node]
        return node

    for name in SCHEMA_ROOTS:
        visit(schemas[name])
    return {name: project(schemas[name]) for name in sorted(closure)}


def _projection_assignment(tree):
    matches = [node for node in tree.body if isinstance(node, ast.AnnAssign)
               and isinstance(node.target, ast.Name) and node.target.id == "SCHEMAS_V1"]
    if len(matches) != 1:
        raise ValueError("expected exactly one generated SCHEMAS_V1 assignment")
    return matches[0]


def update_source(source, expected):
    """Replace changed literal nodes while preserving validator code and layout."""
    tree = ast.parse(source)
    assignment = _projection_assignment(tree)
    lines = source.splitlines(keepends=True)
    offsets = [0]
    for line in lines:
        offsets.append(offsets[-1] + len(line))
    updates = []

    def replace(node, value):
        if (isinstance(node, ast.List) and node.elts and node.lineno != node.end_lineno
                and isinstance(value, list) and all(isinstance(item, str) for item in value)):
            entry_indent = node.elts[0].col_offset
            closing_line = lines[node.end_lineno - 1]
            closing_indent = len(closing_line) - len(closing_line.lstrip())
            literal = ("[\n" + "".join(" " * entry_indent + json.dumps(item) + ",\n"
                                      for item in value) + " " * closing_indent + "]")
        else:
            literal = pprint.pformat(value, width=max(40, 100 - node.col_offset),
                                     indent=4, sort_dicts=True)
            literal = literal.replace("\n", "\n" + " " * node.col_offset)
        updates.append((offsets[node.lineno - 1] + node.col_offset,
                        offsets[node.end_lineno - 1] + node.end_col_offset,
                        literal.encode("utf-8")))

    def visit(node, value):
        current = ast.literal_eval(node)
        if current == value:
            return
        if isinstance(node, ast.Dict) and isinstance(value, dict):
            keys = [ast.literal_eval(key) for key in node.keys]
            if len(keys) == len(set(keys)) and set(keys) == set(value):
                for key, child in zip(keys, node.values):
                    visit(child, value[key])
                return
        replace(node, value)

    visit(assignment.value, expected)
    updated = source
    for start, end, replacement in sorted(updates, reverse=True):
        updated = updated[:start] + replacement + updated[end:]
    updated_tree = ast.parse(updated)
    generated = _projection_assignment(updated_tree)
    if ast.literal_eval(generated.value) != expected:
        raise ValueError("generated release projection is not the exact OpenAPI closure")
    original_other = [node for node in tree.body if node is not assignment]
    updated_other = [node for node in updated_tree.body if node is not generated]
    if ([ast.dump(node, include_attributes=False) for node in original_other]
            != [ast.dump(node, include_attributes=False) for node in updated_other]):
        raise ValueError("projection producer changed non-generated validator code")
    return updated


def main():
    parser = argparse.ArgumentParser(allow_abbrev=False, description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--check", action="store_true", help="Refuse projection drift without writing.")
    parser.add_argument("--output", type=Path, help="Write a preview to this separate destination.")
    arguments = parser.parse_args()
    if arguments.check and arguments.output is not None:
        parser.error("--check and --output are mutually exclusive")
    root = arguments.root.resolve(strict=True)
    mirrors = [(root / name).read_bytes() for name in OPENAPI_PATHS]
    if any(len(raw) > 64 * 1024 * 1024 for raw in mirrors) or any(raw != mirrors[0] for raw in mirrors[1:]):
        raise ValueError("the three canonical Torii OpenAPI mirrors must match")
    specification = json.loads(mirrors[0], object_pairs_hook=_unique_keys)
    expected = schema_projection(specification)
    path = root / CONSUMER
    if path.is_symlink():
        raise ValueError("generated projection must be a physical source file")
    source = path.read_bytes()
    updated = update_source(source, expected)
    if arguments.check:
        if updated != source:
            raise SystemExit("Kagemusha Python release projection differs; run scripts/update_governance_kagemusha_release_schema_v1.py")
    elif arguments.output is not None:
        target = arguments.output.absolute()
        if target.exists() or target.is_symlink():
            raise ValueError("projection preview destination must be absent")
        target.parent.mkdir(parents=True, exist_ok=True)
        with target.open("xb") as destination:
            destination.write(updated)
    elif updated != source:
        path.write_bytes(updated)
    print(json.dumps({"schema_count": len(expected),
                      "openapi_sha256": hashlib.sha256(mirrors[0]).hexdigest(),
                      "projection_sha256": hashlib.sha256(updated).hexdigest(),
                      "mode": "checked" if arguments.check else "preview" if arguments.output else "regenerated"},
                     sort_keys=True))


if __name__ == "__main__":
    main()
