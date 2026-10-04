#!/usr/bin/env python3
"""Check structural consistency of the ZK delivery graph and Markdown plan.

Requires Python 3.10+ and its standard library; no environment variables or
installed packages are needed. Defaults read specs/zk_delivery_graph.json and
specs/zk_delivery_plan.md relative to this script's repository. This read-only
check never changes files and does not establish code or cryptographic readiness.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
SCOPE = "Structural consistency only; not code or cryptographic qualification."
IDENTIFIER = re.compile(r"[A-Z][A-Za-z0-9_-]*\.[A-Za-z0-9_-]+\Z")


def check_plan(graph: Any, plan: str) -> dict[str, Any]:
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
    if graph.get("source_plan_available") is not False:
        errors.append("source_plan_available must be false for this reconstructed plan")

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
    report["task_count"] = len(task_ids)
    report["delivery_count"] = len(delivery_ids)
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
            for rule in forbidden
        ):
            errors.append("constraints.forbidden_ancestors must contain task/prefix objects")
        string_list(constraints.get("non_gating_tasks"), "constraints.non_gating_tasks")
        if not isinstance(constraints.get("runtime_admission_task"), str):
            errors.append("constraints.runtime_admission_task must be a task ID")
    # Schema failures must not turn into traceback exceptions during graph walks.
    if errors:
        return report

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
        matches = sorted(item for item in ancestors[rule["task"]] if item.startswith(rule["prefix"]))
        if matches:
            errors.append(f"forbidden ancestry for {rule['task']}: {', '.join(matches)}")
    admission = constraints["runtime_admission_task"]
    for identifier in constraints["non_gating_tasks"]:
        if identifier == admission or identifier in ancestors[admission]:
            errors.append(f"runtime admission {admission} depends on non-gating task {identifier}")

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
    parser.add_argument("--format", choices=("text", "json"), default="text")
    args = parser.parse_args(argv)
    try:
        graph = json.loads(args.graph.read_text(encoding="utf-8"))
        plan = args.plan.read_text(encoding="utf-8")
        report = check_plan(graph, plan)
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
