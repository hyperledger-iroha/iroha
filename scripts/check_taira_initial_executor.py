#!/usr/bin/env python3
"""Check the reviewed Taira native instruction inventory before Cargo.

Requires Python 3.10+ and repository sources; no environment inputs or writes.
This early source check supplements the authoritative Rust census and execution tests.
It accepts the concrete registry grammar used by these two source files and fails
closed on family entries it cannot parse, duplicate types, or missing dispositions.
Removed citizen-bond operations must stay absent from both production registries.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import re

CLOSED = {
    "soracloud": {"RecordSoracloudMailboxMessage", "ApplySoracloudOrderedMailboxResult"},
    "sorafs": {"RegisterProviderOwner", "UnregisterProviderOwner"},
}
RETIRED_SORAFS_INSTRUCTIONS = {
    "RegisterSorafsCitizenBond", "RotateSorafsCitizenBondAuthorization",
    "RequestSorafsCitizenBondExit",
}


def without_comments(source: str) -> str:
    """Remove line/nested block comments while preserving quoted wire IDs."""
    result = list(source)
    index = 0
    while index < len(source):
        if source[index] == '"':
            index += 1
            while index < len(source):
                if source[index] == "\\":
                    index += 2
                elif source[index] == '"':
                    index += 1
                    break
                else:
                    index += 1
            continue
        if source.startswith("//", index):
            end = source.find("\n", index)
            end = len(source) if end < 0 else end
        elif source.startswith("/*", index):
            end, depth = index + 2, 1
            while depth and end < len(source):
                if source.startswith("/*", end):
                    depth += 1
                    end += 2
                elif source.startswith("*/", end):
                    depth -= 1
                    end += 2
                else:
                    end += 1
            if depth:
                raise ValueError("unterminated block comment")
        else:
            index += 1
            continue
        for cursor in range(index, end):
            if result[cursor] != "\n":
                result[cursor] = " "
        index = end
    return "".join(result)


def census(registry_source: str, wire_source: str) -> dict:
    registry = without_comments(registry_source)
    wire = without_comments(wire_source)
    blocks = re.findall(r"(?m)^define_instruction_handlers!\s*\{(.*?)^\}", registry, re.S)
    if len(blocks) != 1:
        raise ValueError("expected exactly one canonical dispatch registry invocation")
    registry = blocks[0]
    result = {}
    for family, closed in CLOSED.items():
        prefix = rf"iroha_data_model::isi::{family}::"
        rows = re.findall(
            rf"(?m)^\s*(\w+)::<\s*{prefix}(\w+)\s*,?\s*>"
            r"\s*(?:=>\s*(\w+))?\s*,", registry,
        )
        mentions = re.findall(prefix, registry)
        if len(rows) != len(mentions):
            raise ValueError(f"{family}: unsupported dispatch registry grammar")
        types = [row[1] for row in rows]
        wire_types = re.findall(rf"built_in_wire_id!\(\s*{family}::(\w+)\s*=>", wire)
        if len(types) != len(set(types)) or len(wire_types) != len(set(wire_types)):
            raise ValueError(f"{family}: duplicate native instruction type")
        if family == "sorafs" and RETIRED_SORAFS_INSTRUCTIONS.intersection(types + wire_types):
            raise ValueError("sorafs: retired citizen-bond operations must not be registered")
        if set(types) != set(wire_types):
            raise ValueError(f"{family}: wire/dispatch mismatch: {sorted(set(types) ^ set(wire_types))}")
        if not rows or any(row[2] not in {"CoreAuthorized", "Closed"} for row in rows):
            raise ValueError(f"{family}: missing or unknown reviewed Initial disposition")
        actual_closed = {row[1] for row in rows if row[2] == "Closed"}
        if actual_closed != closed:
            raise ValueError(f"{family}: explicit closed operation set changed")
        for handler, name, _ in rows:
            if handler != "dispatch_instruction":
                raise ValueError(f"{family}::{name}: unexpected typed handler")
        result[family] = {"wire": len(wire_types), "admitted": len(rows) - len(closed), "closed": len(closed)}
    return result


def mutation_tests(registry: str, wire: str) -> int:
    marker = "dispatch_instruction::<iroha_data_model::isi::soracloud::DeploySoracloudService> => CoreAuthorized,"
    if registry.count(marker) != 1:
        raise ValueError("mutation fixture anchor is not unique")
    mutations = [
        (registry.replace(marker, marker.replace(" => CoreAuthorized", "")), wire),
        (registry.replace(marker, ""), wire),
        (registry.replace(marker, "// " + marker), wire),
        (registry, wire + '\nbuilt_in_wire_id!(soracloud::UnreviewedNewInstruction => "new"),\n'),
        (registry.replace(marker, marker + "\n" + marker), wire),
        (registry.replace(marker, marker.replace("CoreAuthorized", "Closed")), wire),
        (registry.replace(marker, marker.replace("dispatch_instruction", "unavailable_instruction")), wire),
    ]
    for name in sorted(RETIRED_SORAFS_INSTRUCTIONS):
        row = f"dispatch_instruction::<iroha_data_model::isi::sorafs::{name}> => CoreAuthorized,"
        wire_entry = f'\nbuilt_in_wire_id!(sorafs::{name} => "iroha.instruction.v1::sorafs::{name}"),\n'
        mutations.append((registry.replace(marker, marker + "\n" + row), wire + wire_entry))
    for index, (changed_registry, changed_wire) in enumerate(mutations):
        try:
            census(changed_registry, changed_wire)
        except ValueError:
            continue
        raise AssertionError(f"mutation {index} escaped the source gate")
    return len(mutations)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    registry = (args.repo / "crates/iroha_core/src/smartcontracts/isi/mod.rs").read_text()
    wire = (args.repo / "crates/iroha_data_model/src/isi/registry/wire_ids.rs").read_text()
    report = {"families": census(registry, wire)}
    if args.self_test:
        report["rejected_mutations"] = mutation_tests(registry, wire)
    print(json.dumps(report, sort_keys=True))
