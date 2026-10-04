#!/usr/bin/env python3
"""Check structural consistency of the ZK delivery graph and Markdown plan.

Requires Python 3.10+ and its standard library; no environment variables or
installed packages are needed. Defaults read specs/zk_delivery_graph.json,
specs/zk_delivery_plan.md and specs/zk_delivery_reconciliation.json relative to
this script's repository. Archived source files need not be present. This read-only
check never changes files and does not establish code or cryptographic readiness.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
SCOPE = "Structural consistency only; not code or cryptographic qualification."
IDENTIFIER = re.compile(r"[A-Z][A-Za-z0-9_-]*\.[A-Za-z0-9_-]+\Z")
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
SUPPORTED_SDKS = ["Rust", "Kotlin/Java/Android", "Swift", "JavaScript", "Python", "C#"]
NON_GATING_TASKS = {"H.3", "V.4", "V.6"}
SOURCE_REQUIREMENT_IDS = {f"REQ-{index:03d}" for index in range(1, 120)}


def reconciliation_inventory_sha256(ledger: dict[str, Any]) -> str:
    """Hash embedded source evidence, independently of the new plan's mappings."""
    fields = {
        "sources": ("id", "path", "sha256"),
        "requirements": ("id", "source", "original_text"),
        "findings": ("id", "source", "original_text"),
        "prior_findings": ("id", "source", "original_record"),
        "original_tasks": ("id", "original_record"),
    }
    snapshot = {
        section: sorted(
            ({field: record[field] for field in names} for record in ledger[section]),
            key=lambda record: record["id"],
        )
        for section, names in fields.items()
    }
    encoded = json.dumps(snapshot, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    return hashlib.sha256(encoded.encode("utf-8")).hexdigest()


def check_reconciliation(
    ledger: Any, baseline: Any, task_ids: set[str]
) -> list[str]:
    """Validate complete embedded source inventories and their current task maps."""
    errors: list[str] = []
    if not isinstance(ledger, dict):
        return ["reconciliation must be an object"]
    if type(ledger.get("schema_version")) is not int or ledger["schema_version"] != 1:
        errors.append("reconciliation.schema_version must be 1")
    if not isinstance(baseline, dict):
        return errors + ["reconciliation_baseline must be an object"]
    for key in ("requirement_ids", "original_task_ids"):
        value = baseline.get(key)
        if (
            not isinstance(value, list) or not value
            or any(not isinstance(item, str) or not item.strip() for item in value)
            or len(set(value)) != len(value)
        ):
            errors.append(f"reconciliation_baseline.{key} must be unique nonempty IDs")
    fingerprint = baseline.get("ledger_inventory_sha256")
    if not isinstance(fingerprint, str) or not SHA256.fullmatch(fingerprint):
        errors.append("reconciliation_baseline.ledger_inventory_sha256 must be a SHA256 digest")
    if errors:
        return errors
    if set(baseline["requirement_ids"]) != SOURCE_REQUIREMENT_IDS:
        errors.append("reconciliation_baseline.requirement_ids must preserve REQ-001 through REQ-119")
    if len(baseline["original_task_ids"]) != 52:
        errors.append("reconciliation_baseline.original_task_ids must preserve all 52 original tasks")

    sections: dict[str, dict[str, dict[str, Any]]] = {}
    for name in ("sources", "requirements", "findings", "prior_findings", "original_tasks"):
        records = ledger.get(name)
        sections[name] = {}
        if not isinstance(records, list) or not records:
            errors.append(f"reconciliation.{name} must be a nonempty list")
            continue
        for record in records:
            if not isinstance(record, dict):
                errors.append(f"reconciliation.{name} records must be objects")
                continue
            identifier = record.get("id")
            if not isinstance(identifier, str) or not identifier.strip():
                errors.append(f"reconciliation.{name} IDs must be nonempty strings")
                continue
            if identifier in sections[name]:
                errors.append(f"duplicate reconciliation.{name} ID {identifier}")
                continue
            sections[name][identifier] = record
    expected = {
        "requirements": SOURCE_REQUIREMENT_IDS,
        "original_tasks": set(baseline["original_task_ids"]),
        "findings": {f"R6-{index}" for index in range(23)},
        "prior_findings": {f"F{index}" for index in range(133)},
    }
    for name, identifiers in expected.items():
        actual = set(sections[name])
        if actual != identifiers:
            missing = ", ".join(sorted(identifiers - actual)) or "none"
            unexpected = ", ".join(sorted(actual - identifiers)) or "none"
            errors.append(f"reconciliation.{name} inventory differs: missing {missing}; unexpected {unexpected}")

    for identifier, source in sections["sources"].items():
        if not isinstance(source.get("path"), str) or not source["path"].strip():
            errors.append(f"source {identifier}.path must be nonempty")
        digest = source.get("sha256")
        if not isinstance(digest, str) or not SHA256.fullmatch(digest):
            errors.append(f"source {identifier}.sha256 must be a SHA256 digest")
        if source.get("available_optional") is not True:
            errors.append(f"source {identifier}.available_optional must be true")

    dispositions = {
        "requirements": {"retained", "corrected"},
        "findings": {"addressed_in_plan", "addressed_with_correction"},
        "prior_findings": {"carried_forward", "superseded_by_user_direction"},
    }
    for name in ("requirements", "findings", "prior_findings", "original_tasks"):
        for identifier, record in sections[name].items():
            mapping = record.get("task_ids")
            if not isinstance(mapping, list) or not mapping or any(
                not isinstance(item, str) or item not in task_ids for item in mapping
            ):
                errors.append(f"{identifier}.task_ids must map to existing tasks")
            elif len(mapping) != len(set(mapping)):
                errors.append(f"{identifier}.task_ids contains duplicates")
            rationale = record.get("rationale")
            if not isinstance(rationale, str) or len(rationale.strip()) < 20:
                errors.append(f"{identifier}.rationale must explain the mapping (at least 20 characters)")
            if name in dispositions:
                disposition = record.get("disposition")
                if not isinstance(disposition, str) or disposition not in dispositions[name]:
                    errors.append(f"{identifier}.disposition is not a supported source disposition")
            if name in ("requirements", "findings"):
                for key in (("original_text", "summary") if name == "requirements" else ("original_text", "title")):
                    if not isinstance(record.get(key), str) or not record[key].strip():
                        errors.append(f"{identifier}.{key} must be nonempty")
            if name == "findings" and record.get("severity") not in ("blocker", "major", "minor"):
                errors.append(f"{identifier}.severity must be blocker, major or minor")
            if name in ("prior_findings", "original_tasks"):
                if not isinstance(record.get("original_record"), dict) or not record["original_record"]:
                    errors.append(f"{identifier}.original_record must preserve the original object")
                elif name == "prior_findings":
                    original = record["original_record"]
                    if not isinstance(original.get("triage"), dict) or not original["triage"]:
                        errors.append(f"{identifier}.original_record.triage must preserve the original object")
                    if not isinstance(original.get("original_report_text"), str) or not original["original_report_text"].strip():
                        errors.append(f"{identifier}.original_record.original_report_text must be nonempty")
            if name == "original_tasks":
                original = record.get("original_record")
                if isinstance(original, dict) and original.get("id") != identifier:
                    errors.append(f"{identifier}.original_record.id differs from the original task ID")
                continue
            reference = record.get("source")
            if not isinstance(reference, dict):
                errors.append(f"{identifier}.source must identify original source lines")
                continue
            source_id = reference.get("source_id")
            source = sections["sources"].get(source_id) if isinstance(source_id, str) else None
            if source is None or reference.get("path") != source.get("path"):
                errors.append(f"{identifier}.source must reference a declared source and its exact path")
            start, end = reference.get("line_start"), reference.get("line_end")
            if type(start) is not int or type(end) is not int or start < 1 or end < start:
                errors.append(f"{identifier}.source must contain a positive ordered line span")
    if errors:
        return errors
    if reconciliation_inventory_sha256(ledger) != fingerprint:
        errors.append("reconciliation source inventory fingerprint differs from the graph baseline")
    return errors


def check_plan(graph: Any, plan: str, reconciliation: Any) -> dict[str, Any]:
    """Return one format-independent report for the graph and plan."""
    errors: list[str] = []
    report: dict[str, Any] = {
        "ok": False,
        "scope": SCOPE,
        "errors": errors,
        "task_count": 0,
        "delivery_count": 0,
        "roots": [],
    }
    if not isinstance(graph, dict):
        errors.append("graph must be an object")
        return report
    if type(graph.get("schema_version")) is not int or graph["schema_version"] != 1:
        errors.append("schema_version must be 1")
    if not isinstance(graph.get("source_commit"), str) or not graph["source_commit"].strip():
        errors.append("source_commit must be a nonempty string")
    if graph.get("source_plan_available") is not True:
        errors.append("source_plan_available must be true for the reconciled source plan")
    if type(graph.get("revision")) is not int or graph["revision"] != 8:
        errors.append("revision must be 8")
    if graph.get("status") != "ready_for_development":
        errors.append("graph status must be ready_for_development")

    def string_list(value: Any, label: str, *, nonempty: bool = False) -> bool:
        valid = isinstance(value, list) and all(
            isinstance(item, str) and bool(item.strip()) for item in value
        )
        if not valid or (nonempty and not value):
            errors.append(f"{label} must be {'a nonempty' if nonempty else 'a'} list of nonempty strings")
            return False
        return True

    nodes: dict[str, dict[str, Any]] = {}
    task_ids: set[str] = set()
    delivery_ids: set[str] = set()
    for kind, ids in (("tasks", task_ids), ("deliveries", delivery_ids)):
        records = graph.get(kind)
        if not isinstance(records, list) or not records:
            errors.append(f"{kind} must be a nonempty list")
            continue
        for index, record in enumerate(records):
            label = f"{kind}[{index}]"
            if not isinstance(record, dict):
                errors.append(f"{label} must be an object")
                continue
            identifier = record.get("id")
            if not isinstance(identifier, str) or not IDENTIFIER.fullmatch(identifier):
                errors.append(f"{label}.id must be a dotted identifier such as W.1")
                continue
            if identifier in nodes:
                errors.append(f"duplicate task/delivery ID: {identifier}")
                continue
            nodes[identifier] = record
            ids.add(identifier)
            for field in (("title", "owner") if kind == "tasks" else ("title",)):
                if not isinstance(record.get(field), str) or not record[field].strip():
                    errors.append(f"{identifier}.{field} must be a nonempty string")
            if string_list(record.get("requires"), f"{identifier}.requires"):
                if len(record["requires"]) != len(set(record["requires"])):
                    errors.append(f"{identifier}.requires contains duplicate edges")
            if kind == "tasks":
                for field in ("outputs", "acceptance"):
                    string_list(record.get(field), f"{identifier}.{field}", nonempty=True)
                if record.get("status") != "planned":
                    errors.append(f"{identifier}.status must be planned")
            else:
                if record.get("kind") not in ("functional", "evidence"):
                    errors.append(f"{identifier}.kind must be functional or evidence")
                if record.get("kind") == "functional" and record.get("surface_contract") != "operation-v1":
                    errors.append(f"{identifier} must use the operation-v1 developer surface contract")
    report["task_count"] = len(task_ids)
    report["delivery_count"] = len(delivery_ids)
    if string_list(graph.get("required_task_ids"), "required_task_ids", nonempty=True):
        for identifier in graph["required_task_ids"]:
            if identifier not in task_ids:
                errors.append(f"required task missing from graph: {identifier}")
        if len(graph["required_task_ids"]) != len(set(graph["required_task_ids"])):
            errors.append("required_task_ids contains duplicate IDs")
    surface = graph.get("developer_surface_contract")
    if not isinstance(surface, dict):
        errors.append("developer_surface_contract must define the common operation-v1 surface")
    else:
        if surface.get("id") != "operation-v1":
            errors.append("developer_surface_contract.id must be operation-v1")
        for field in ("native_isi", "kotodama", "one_call_sdk", "prepared_form", "four_validator_restart"):
            if surface.get(field) is not True:
                errors.append(f"developer_surface_contract.{field} must be true")
        if surface.get("supported_sdks") != SUPPORTED_SDKS:
            errors.append("developer_surface_contract.supported_sdks must preserve all six canonical SDK targets")
        for field in ("atomic_semantics", "progress_semantics"):
            if not isinstance(surface.get(field), str) or not surface[field].strip():
                errors.append(f"developer_surface_contract.{field} must be nonempty")
    if string_list(graph.get("start_immediately"), "start_immediately"):
        if len(graph["start_immediately"]) != len(set(graph["start_immediately"])):
            errors.append("start_immediately contains duplicate IDs")

    constraints = graph.get("constraints")
    if not isinstance(constraints, dict):
        errors.append("constraints must be an object")
    else:
        pairs = constraints.get("required_ancestors")
        if not isinstance(pairs, list) or any(
            not isinstance(pair, list)
            or len(pair) != 2
            or any(not isinstance(item, str) or not item for item in pair)
            for pair in pairs
        ):
            errors.append("constraints.required_ancestors must contain [dependent, ancestor] pairs")
        forbidden = constraints.get("forbidden_ancestors")
        if not isinstance(forbidden, list) or any(
            not isinstance(rule, dict)
            or not isinstance(rule.get("task"), str)
            or not rule["task"]
            or not isinstance(rule.get("prefix"), str)
            or not rule["prefix"]
            or ("exact" in rule and type(rule["exact"]) is not bool)
            for rule in forbidden
        ):
            errors.append("constraints.forbidden_ancestors must contain task/prefix objects")
        if string_list(constraints.get("non_gating_tasks"), "constraints.non_gating_tasks"):
            if not NON_GATING_TASKS.issubset(set(constraints["non_gating_tasks"])):
                errors.append("non_gating_tasks must include H.3, V.4 and V.6")
        if not isinstance(constraints.get("runtime_admission_task"), str):
            errors.append("constraints.runtime_admission_task must be a task ID")
    # Schema failures must not turn into traceback exceptions during graph walks.
    if errors:
        return report

    errors.extend(check_reconciliation(reconciliation, graph.get("reconciliation_baseline"), task_ids))

    for identifier, record in nodes.items():
        for dependency in record["requires"]:
            if dependency not in nodes:
                errors.append(f"{identifier} requires unknown ID {dependency}")
    for identifier in graph["start_immediately"]:
        if identifier not in task_ids:
            errors.append(f"start_immediately contains unknown task {identifier}")
    for dependent, ancestor in constraints["required_ancestors"]:
        for identifier in (dependent, ancestor):
            if identifier not in nodes:
                errors.append(f"required_ancestors contains unknown ID {identifier}")
    for rule in constraints["forbidden_ancestors"]:
        if rule["task"] not in task_ids:
            errors.append(f"forbidden_ancestors contains unknown task {rule['task']}")
    for identifier in constraints["non_gating_tasks"] + [constraints["runtime_admission_task"]]:
        if identifier not in task_ids:
            errors.append(f"admission constraints contain unknown task {identifier}")
    if errors:
        return report

    # Iterative traversal also handles malformed, very deep graphs predictably.
    ancestors: dict[str, set[str]] = {}
    for identifier in nodes:
        reachable: set[str] = set()
        pending = list(nodes[identifier]["requires"])
        while pending:
            dependency = pending.pop()
            if dependency in reachable:
                continue
            reachable.add(dependency)
            pending.extend(nodes[dependency]["requires"])
        ancestors[identifier] = reachable
        if identifier in reachable:
            errors.append(f"dependency cycle includes {identifier}")
    roots = sorted(identifier for identifier in task_ids if not nodes[identifier]["requires"])
    report["roots"] = roots
    if set(roots) != set(graph["start_immediately"]):
        errors.append(f"start_immediately must equal task roots: {', '.join(roots) or '(none)'}")
    delivered = set().union(*(ancestors[identifier] for identifier in delivery_ids))
    for identifier in sorted(task_ids - delivered):
        errors.append(f"orphan task {identifier} reaches no delivery")
    for dependent, ancestor in constraints["required_ancestors"]:
        if ancestor not in ancestors[dependent]:
            errors.append(f"required ancestry missing: {dependent} must depend on {ancestor}")
    for rule in constraints["forbidden_ancestors"]:
        matches = sorted(
            item for item in ancestors[rule["task"]]
            if (item == rule["prefix"] if rule.get("exact", False) else item.startswith(rule["prefix"]))
        )
        if matches:
            errors.append(f"forbidden ancestry for {rule['task']}: {', '.join(matches)}")
    admission = constraints["runtime_admission_task"]
    for identifier in constraints["non_gating_tasks"]:
        if identifier == admission or identifier in ancestors[admission]:
            errors.append(f"runtime admission {admission} depends on non-gating task {identifier}")
    evidence_deliveries = {
        identifier for identifier in delivery_ids if nodes[identifier]["kind"] == "evidence"
    }
    for identifier in delivery_ids:
        if nodes[identifier]["kind"] != "functional":
            continue
        gates = ancestors[identifier] & (set(constraints["non_gating_tasks"]) | evidence_deliveries)
        if gates:
            errors.append(f"functional delivery {identifier} depends on evidence gates: {', '.join(sorted(gates))}")

    rows: dict[str, list[str]] = {}
    for line_number, line in enumerate(plan.splitlines(), 1):
        line = line.strip()
        if not line.startswith("|") or not line.endswith("|"):
            continue
        cells = [cell.strip() for cell in line[1:-1].split("|")]
        if not cells or not IDENTIFIER.fullmatch(cells[0]):
            continue
        identifier = cells[0]
        if identifier in rows:
            errors.append(f"duplicate plan table row for {identifier} at line {line_number}")
        rows[identifier] = cells
        if identifier not in nodes:
            errors.append(f"plan table contains unknown ID {identifier} at line {line_number}")
    for identifier, record in nodes.items():
        expected = [identifier, record["title"]]
        if identifier in task_ids:
            expected.append(record["owner"])
        expected.append(", ".join(record["requires"]) if record["requires"] else "—")
        if identifier not in rows:
            errors.append(f"plan table is missing {identifier}")
        elif rows[identifier] != expected:
            errors.append(f"plan table differs from graph for {identifier}")
        if identifier in task_ids:
            contract = (
                f"### {identifier} {record['title']}\n\n"
                f"Deliverable: {' '.join(record['outputs'])}\n\n"
                f"Acceptance: {' '.join(record['acceptance'])}"
            )
            if not re.search(r"^" + re.escape(contract) + r"(?=\n\n|\n?\Z)", plan, re.MULTILINE):
                errors.append(f"plan task contract differs from graph for {identifier}")
    report["ok"] = not errors
    return report


def main(argv: list[str] | None = None) -> int:
    """Read inputs, validate once, and render the same report in either format."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--graph", type=Path, default=ROOT / "specs/zk_delivery_graph.json")
    parser.add_argument("--plan", type=Path, default=ROOT / "specs/zk_delivery_plan.md")
    parser.add_argument("--reconciliation", type=Path, default=ROOT / "specs/zk_delivery_reconciliation.json")
    parser.add_argument("--format", choices=("text", "json"), default="text")
    args = parser.parse_args(argv)
    try:
        graph = json.loads(args.graph.read_text(encoding="utf-8"))
        plan = args.plan.read_text(encoding="utf-8")
        reconciliation = json.loads(args.reconciliation.read_text(encoding="utf-8"))
        report = check_plan(graph, plan, reconciliation)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        report = {"ok": False, "scope": SCOPE, "errors": [f"cannot read inputs: {error}"]}
    if args.format == "json":
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        print("PASS" if report["ok"] else "FAIL")
        print(SCOPE)
        for error in report["errors"]:
            print(f"- {error}")
        if report["ok"]:
            print(f"{report['task_count']} tasks, {report['delivery_count']} deliveries; graph and plan agree.")
    return 0 if report["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
