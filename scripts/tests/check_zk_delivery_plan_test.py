"""Exercise structural failures and output parity of the ZK plan checker."""

from __future__ import annotations

from contextlib import redirect_stdout
import importlib.util
from io import StringIO
import json
from pathlib import Path
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "check_zk_delivery_plan.py"
SPEC = importlib.util.spec_from_file_location("check_zk_delivery_plan", SCRIPT)
assert SPEC and SPEC.loader
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)


def ledger_fixture() -> dict:
    """Embed the complete inventories without depending on archived files."""
    reference = {
        "source_id": "r6", "path": "dist/zk_checker_intentionally_absent_source.md",
        "line_start": 1, "line_end": 1,
    }
    rationale = "Mapped to the canonical implementation and its acceptance tests."
    return {
        "schema_version": 1,
        "sources": [{"id": "r6", "path": reference["path"], "sha256": "0" * 64, "available_optional": True}],
        "requirements": [{
            "id": f"REQ-{index:03d}", "source": dict(reference),
            "original_text": f"Original requirement {index}", "summary": f"Requirement {index}",
            "disposition": "retained", "task_ids": ["B.1"], "rationale": rationale,
        } for index in range(1, 120)],
        "findings": [{
            "id": f"R6-{index}", "source": dict(reference),
            "original_text": f"Original finding {index}", "title": f"Finding {index}",
            "severity": "minor", "disposition": "addressed_in_plan",
            "task_ids": ["B.1"], "rationale": rationale,
        } for index in range(23)],
        "prior_findings": [{
            "id": f"F{index}", "source": dict(reference),
            "original_record": {"triage": {"id": f"F{index}"}, "original_report_text": f"Original review {index}"},
            "disposition": "carried_forward", "task_ids": ["B.1"], "rationale": rationale,
        } for index in range(133)],
        "original_tasks": [{
            "id": f"O.{index}", "original_record": {"id": f"O.{index}", "title": f"Original task {index}"},
            "task_ids": ["B.1"], "rationale": rationale,
        } for index in range(52)],
    }


def fixture() -> dict:
    """Return a small graph with independent runtime and review deliveries."""
    tasks = []
    for identifier, dependencies in (("B.1", []), ("R.1", []), ("H.3", []), ("P.5", ["B.1"]), ("C.1", ["B.1"]), ("V.4", []), ("V.6", [])):
        tasks.append({
            "id": identifier,
            "title": f"Task {identifier}",
            "owner": "Core",
            "requires": dependencies,
            "outputs": ["Concrete artifact"],
            "acceptance": ["Observable acceptance"],
            "status": "planned",
        })
    ledger = ledger_fixture()
    return {
        "schema_version": 1,
        "source_commit": "abc123",
        "source_plan_available": True,
        "revision": 8,
        "status": "ready_for_development",
        "start_immediately": ["B.1", "R.1", "H.3", "V.4", "V.6"],
        "tasks": tasks,
        "required_task_ids": ["B.1", "C.1"],
        "reconciliation_baseline": {
            "requirement_ids": [record["id"] for record in ledger["requirements"]],
            "original_task_ids": [record["id"] for record in ledger["original_tasks"]],
            "ledger_inventory_sha256": CHECKER.reconciliation_inventory_sha256(ledger),
        },
        "developer_surface_contract": {
            "id": "operation-v1", "native_isi": True, "kotodama": True,
            "one_call_sdk": True, "prepared_form": True, "four_validator_restart": True,
            "supported_sdks": list(CHECKER.SUPPORTED_SDKS),
            "atomic_semantics": "One atomic transaction for each atomic operation.",
            "progress_semantics": "Explicit durable progress for multi-round workflows.",
        },
        "deliveries": [
            {"id": "D.1", "title": "Delivery", "requires": ["R.1", "P.5", "C.1"], "kind": "functional", "surface_contract": "operation-v1"},
            {"id": "D.EVIDENCE", "title": "Evidence", "requires": ["H.3", "V.4", "V.6"], "kind": "evidence"},
        ],
        "constraints": {
            "required_ancestors": [["P.5", "B.1"]],
            "forbidden_ancestors": [{"task": "C.1", "prefix": "R."}],
            "non_gating_tasks": ["H.3", "V.4", "V.6"],
            "runtime_admission_task": "P.5",
        },
    }


def markdown(graph: dict) -> str:
    """Render exact task and delivery rows for the test fixture."""
    rows = ["| ID | Title | Owner | Requires |", "| --- | --- | --- | --- |"]
    for task in graph["tasks"]:
        requires = ", ".join(task["requires"]) or "—"
        rows.append(f"| {task['id']} | {task['title']} | {task['owner']} | {requires} |")
    rows.extend(["", "| ID | Title | Requires |", "| --- | --- | --- |"])
    for delivery in graph["deliveries"]:
        requires = ", ".join(delivery["requires"]) or "—"
        rows.append(f"| {delivery['id']} | {delivery['title']} | {requires} |")
    for task in graph["tasks"]:
        rows.append(
            f"\n### {task['id']} {task['title']}\n\n"
            f"Deliverable: {' '.join(task['outputs'])}\n\n"
            f"Acceptance: {' '.join(task['acceptance'])}"
        )
    return "\n".join(rows)


class CheckZkDeliveryPlanTests(unittest.TestCase):
    """The same semantic validator backs both command-line output modes."""

    def assert_failure(self, graph: dict, message: str, plan: str | None = None, ledger: dict | None = None) -> None:
        report = CHECKER.check_plan(graph, markdown(graph) if plan is None else plan, ledger_fixture() if ledger is None else ledger)
        self.assertFalse(report["ok"])
        self.assertTrue(any(message in error for error in report["errors"]), report)

    def test_valid_graph_and_plan(self) -> None:
        graph = fixture()
        self.assertTrue(CHECKER.check_plan(graph, markdown(graph), ledger_fixture())["ok"])

    def test_malformed_graphs_fail_without_traceback(self) -> None:
        for graph in (None, [], {}, {"tasks": [None]}, {"schema_version": True}):
            with self.subTest(graph=graph):
                self.assertFalse(CHECKER.check_plan(graph, "", ledger_fixture())["ok"])
        for field, value in (("requires", None), ("owner", ""), ("outputs", []), ("acceptance", [None]), ("status", "done")):
            graph = fixture()
            graph["tasks"][0][field] = value
            self.assert_failure(graph, field, plan="")
        graph = fixture()
        graph["constraints"]["required_ancestors"] = [None]
        self.assert_failure(graph, "required_ancestors")

    def test_unknown_and_duplicate_dependency_edges(self) -> None:
        graph = fixture()
        graph["tasks"][4]["requires"] = ["MISSING.1"]
        self.assert_failure(graph, "requires unknown ID")
        graph["tasks"][4]["requires"] = ["B.1", "B.1"]
        self.assert_failure(graph, "duplicate edges")

    def test_duplicate_ids_across_task_and_delivery(self) -> None:
        graph = fixture()
        graph["deliveries"][0]["id"] = "B.1"
        self.assert_failure(graph, "duplicate task/delivery ID")

    def test_dependency_cycle(self) -> None:
        graph = fixture()
        graph["tasks"][0]["requires"] = ["P.5"]
        self.assert_failure(graph, "dependency cycle")

    def test_false_immediate_roots(self) -> None:
        graph = fixture()
        graph["start_immediately"].append("P.5")
        self.assert_failure(graph, "must equal task roots")
        graph["start_immediately"] = ["B.1"]
        self.assert_failure(graph, "must equal task roots")

    def test_orphan_task(self) -> None:
        graph = fixture()
        graph["deliveries"][0]["requires"].remove("C.1")
        self.assert_failure(graph, "orphan task C.1")

    def test_required_ancestry_is_transitive(self) -> None:
        graph = fixture()
        graph["tasks"][3]["requires"] = ["C.1"]
        self.assertTrue(CHECKER.check_plan(graph, markdown(graph), ledger_fixture())["ok"])
        graph["tasks"][3]["requires"] = ["R.1"]
        self.assert_failure(graph, "required ancestry missing")

    def test_ram_lfe_dependency_is_rejected_transitively(self) -> None:
        graph = fixture()
        graph["tasks"][0]["requires"] = ["R.1"]
        graph["start_immediately"].remove("B.1")
        self.assert_failure(graph, "forbidden ancestry for C.1: R.1")

    def test_exact_forbidden_ancestor_distinguishes_r1_from_r11(self) -> None:
        graph = fixture()
        task = dict(graph["tasks"][1], id="R.11", title="Task R.11")
        graph["tasks"].append(task)
        graph["start_immediately"].append("R.11")
        graph["tasks"][4]["requires"].append("R.11")
        graph["constraints"]["forbidden_ancestors"] = [{"task": "C.1", "prefix": "R.1", "exact": True}]
        self.assertTrue(CHECKER.check_plan(graph, markdown(graph), ledger_fixture())["ok"])
        graph["tasks"][4]["requires"].append("R.1")
        self.assert_failure(graph, "forbidden ancestry for C.1: R.1")
        graph["constraints"]["forbidden_ancestors"][0]["exact"] = "true"
        self.assert_failure(graph, "forbidden_ancestors must contain task/prefix objects")

    def test_non_gating_review_cannot_gate_runtime(self) -> None:
        graph = fixture()
        graph["tasks"][0]["requires"] = ["H.3"]
        graph["start_immediately"].remove("B.1")
        self.assert_failure(graph, "runtime admission P.5 depends on non-gating task H.3")

    def test_plan_drift_in_any_column_or_row(self) -> None:
        graph = fixture()
        plan = markdown(graph)
        for changed in (
            plan.replace("Task B.1", "Changed title"),
            plan.replace("| Core |", "| Wrong owner |", 1),
            plan.replace("| P.5 | Task P.5 | Core | B.1 |", "| P.5 | Task P.5 | Core | R.1 |"),
            plan.replace("| B.1 | Task B.1 | Core | — |", ""),
            plan + "\n| Z.9 | Unknown | Core | — |",
            plan + "\n| B.1 | Task B.1 | Core | — |",
            plan.replace("| D.1 | Delivery |", "| D.1 | Renamed delivery |"),
            plan.replace("Acceptance: Observable acceptance", "Acceptance: Weaker acceptance", 1),
            plan.replace("Acceptance: Observable acceptance", "Acceptance: Observable acceptance changed", 1),
            plan.replace("Deliverable: Concrete artifact", "Deliverable: Different artifact", 1),
        ):
            with self.subTest(changed=changed):
                self.assertFalse(CHECKER.check_plan(graph, changed, ledger_fixture())["ok"])

    def test_text_and_json_modes_have_identical_exit_and_diagnostics(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            graph_path = Path(directory) / "graph.json"
            plan_path = Path(directory) / "plan.md"
            ledger_path = Path(directory) / "ledger.json"
            for invalid in (None, "graph", "ledger"):
                graph = fixture()
                ledger = ledger_fixture()
                plan_path.write_text(markdown(graph), encoding="utf-8")
                if invalid == "graph":
                    graph["tasks"][4]["requires"] = ["R.1"]
                elif invalid == "ledger":
                    ledger["prior_findings"].pop()
                ledger_path.write_text(json.dumps(ledger), encoding="utf-8")
                graph_path.write_text(json.dumps(graph), encoding="utf-8")
                outputs = {}
                statuses = {}
                for output_format in ("text", "json"):
                    output = StringIO()
                    with redirect_stdout(output):
                        statuses[output_format] = CHECKER.main([
                            "--graph", str(graph_path), "--plan", str(plan_path), "--format", output_format,
                            "--reconciliation", str(ledger_path),
                        ])
                    outputs[output_format] = output.getvalue()
                self.assertEqual(statuses["text"], statuses["json"])
                self.assertEqual(statuses["text"], int(invalid is not None))
                report = json.loads(outputs["json"])
                self.assertIn(CHECKER.SCOPE, outputs["text"])
                for error in report["errors"]:
                    self.assertIn(error, outputs["text"])

    def test_cli_input_errors_are_reports(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "bad.json"
            path.write_text("{bad", encoding="utf-8")
            output = StringIO()
            with redirect_stdout(output):
                status = CHECKER.main(["--graph", str(path), "--format", "json"])
            self.assertEqual(status, 1)
            self.assertIn("cannot read inputs", json.loads(output.getvalue())["errors"][0])

    def test_missing_original_sources_do_not_block_validation(self) -> None:
        graph, ledger = fixture(), ledger_fixture()
        self.assertFalse((CHECKER.ROOT / ledger["sources"][0]["path"]).exists())
        self.assertTrue(CHECKER.check_plan(graph, markdown(graph), ledger)["ok"])

    def test_malformed_reconciliation_fails_without_traceback(self) -> None:
        graph = fixture()
        for ledger in (None, [], {}, {"sources": [None]}):
            self.assertFalse(CHECKER.check_plan(graph, markdown(graph), ledger)["ok"])
        for section, field, value in (
            ("requirements", "disposition", []),
            ("requirements", "source", []),
            ("findings", "task_ids", [{}]),
            ("prior_findings", "original_record", "missing original"),
            ("sources", "sha256", "not a hash"),
        ):
            ledger = ledger_fixture()
            ledger[section][0][field] = value
            self.assertFalse(CHECKER.check_plan(graph, markdown(graph), ledger)["ok"])

    def test_all_original_inventories_are_complete(self) -> None:
        for section in ("requirements", "findings", "prior_findings", "original_tasks"):
            with self.subTest(section=section):
                graph, ledger = fixture(), ledger_fixture()
                ledger[section].pop()
                self.assert_failure(graph, f"reconciliation.{section} inventory differs", ledger=ledger)

    def test_requirement_inventory_cannot_be_self_certified_by_dropping_baseline(self) -> None:
        graph, ledger = fixture(), ledger_fixture()
        ledger["requirements"].pop()
        graph["reconciliation_baseline"]["requirement_ids"].pop()
        graph["reconciliation_baseline"]["ledger_inventory_sha256"] = CHECKER.reconciliation_inventory_sha256(ledger)
        self.assert_failure(graph, "must preserve REQ-001 through REQ-119", ledger=ledger)

    def test_source_fingerprint_detects_modified_originals(self) -> None:
        graph, ledger = fixture(), ledger_fixture()
        ledger["requirements"][0]["original_text"] = "Changed original source requirement"
        self.assert_failure(graph, "source inventory fingerprint differs", ledger=ledger)
        ledger = ledger_fixture()
        ledger["prior_findings"][0]["original_record"]["original_report_text"] = "Changed original finding"
        self.assert_failure(graph, "source inventory fingerprint differs", ledger=ledger)

    def test_reconciliation_mapping_and_evidence_metadata(self) -> None:
        for section in ("requirements", "findings", "prior_findings", "original_tasks"):
            graph, ledger = fixture(), ledger_fixture()
            ledger[section][0]["task_ids"] = ["MISSING.1"]
            self.assert_failure(graph, "must map to existing tasks", ledger=ledger)
        for field, value, message in (
            ("disposition", "fixed_in_code", "disposition"),
            ("rationale", "TODO", "rationale"),
            ("source", {"source_id": "missing", "path": "wrong", "line_start": 2, "line_end": 1}, "source"),
        ):
            graph, ledger = fixture(), ledger_fixture()
            ledger["requirements"][0][field] = value
            self.assert_failure(graph, message, ledger=ledger)

    def test_functional_deliveries_cannot_depend_on_evidence(self) -> None:
        graph = fixture()
        graph["deliveries"][0]["requires"].append("V.4")
        self.assert_failure(graph, "functional delivery D.1 depends on evidence gates")
        graph["deliveries"][0]["requires"][-1] = "D.EVIDENCE"
        self.assert_failure(graph, "functional delivery D.1 depends on evidence gates")

    def test_functional_surface_cannot_be_weakened(self) -> None:
        graph = fixture()
        graph["deliveries"][0]["surface_contract"] = "sdk-only"
        self.assert_failure(graph, "must use the operation-v1")
        for field in ("native_isi", "kotodama", "one_call_sdk", "prepared_form", "four_validator_restart"):
            graph = fixture()
            graph["developer_surface_contract"][field] = False
            self.assert_failure(graph, f"developer_surface_contract.{field} must be true")
        graph = fixture()
        graph["developer_surface_contract"]["supported_sdks"].remove("C#")
        self.assert_failure(graph, "must preserve all six")

    def test_required_source_scope_task_cannot_be_omitted(self) -> None:
        graph = fixture()
        graph["tasks"] = [task for task in graph["tasks"] if task["id"] != "C.1"]
        graph["deliveries"][0]["requires"].remove("C.1")
        graph["constraints"]["forbidden_ancestors"] = []
        self.assert_failure(graph, "required task missing from graph: C.1")


if __name__ == "__main__":
    unittest.main()
