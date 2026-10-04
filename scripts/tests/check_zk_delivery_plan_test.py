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


def fixture() -> dict:
    """Return a small graph with independent runtime and review deliveries."""
    tasks = []
    for identifier, dependencies in (("B.1", []), ("R.1", []), ("H.3", []), ("P.5", ["B.1"]), ("C.1", ["B.1"])):
        tasks.append({
            "id": identifier,
            "title": f"Task {identifier}",
            "owner": "Core",
            "requires": dependencies,
            "outputs": ["Concrete artifact"],
            "acceptance": ["Observable acceptance"],
            "status": "planned",
        })
    return {
        "schema_version": 1,
        "source_commit": "abc123",
        "source_plan_available": False,
        "start_immediately": ["B.1", "R.1", "H.3"],
        "tasks": tasks,
        "deliveries": [{"id": "D.1", "title": "Delivery", "requires": ["R.1", "H.3", "P.5", "C.1"]}],
        "constraints": {
            "required_ancestors": [["P.5", "B.1"]],
            "forbidden_ancestors": [{"task": "C.1", "prefix": "R."}],
            "non_gating_tasks": ["H.3"],
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

    def assert_failure(self, graph: dict, message: str, plan: str | None = None) -> None:
        report = CHECKER.check_plan(graph, markdown(graph) if plan is None else plan)
        self.assertFalse(report["ok"])
        self.assertTrue(any(message in error for error in report["errors"]), report)

    def test_valid_graph_and_plan(self) -> None:
        graph = fixture()
        self.assertTrue(CHECKER.check_plan(graph, markdown(graph))["ok"])

    def test_malformed_graphs_fail_without_traceback(self) -> None:
        for graph in (None, [], {}, {"tasks": [None]}, {"schema_version": True}):
            with self.subTest(graph=graph):
                self.assertFalse(CHECKER.check_plan(graph, "")["ok"])
        for field, value in (("requires", None), ("owner", ""), ("outputs", []), ("acceptance", [None]), ("status", "done")):
            graph = fixture()
            graph["tasks"][0][field] = value
            self.assert_failure(graph, field, plan="")
        graph = fixture()
        graph["constraints"]["required_ancestors"] = [None]
        self.assert_failure(graph, "required_ancestors")

    def test_unknown_and_duplicate_dependency_edges(self) -> None:
        graph = fixture()
        graph["tasks"][-1]["requires"] = ["MISSING.1"]
        self.assert_failure(graph, "requires unknown ID")
        graph["tasks"][-1]["requires"] = ["B.1", "B.1"]
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
        self.assertTrue(CHECKER.check_plan(graph, markdown(graph))["ok"])
        graph["tasks"][3]["requires"] = ["R.1"]
        self.assert_failure(graph, "required ancestry missing")

    def test_ram_lfe_dependency_is_rejected_transitively(self) -> None:
        graph = fixture()
        graph["tasks"][0]["requires"] = ["R.1"]
        graph["start_immediately"].remove("B.1")
        self.assert_failure(graph, "forbidden ancestry for C.1: R.1")

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
                self.assertFalse(CHECKER.check_plan(graph, changed)["ok"])

    def test_text_and_json_modes_have_identical_exit_and_diagnostics(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            graph_path = Path(directory) / "graph.json"
            plan_path = Path(directory) / "plan.md"
            for invalid in (False, True):
                graph = fixture()
                plan_path.write_text(markdown(graph), encoding="utf-8")
                if invalid:
                    graph["tasks"][-1]["requires"] = ["R.1"]
                graph_path.write_text(json.dumps(graph), encoding="utf-8")
                outputs = {}
                statuses = {}
                for output_format in ("text", "json"):
                    output = StringIO()
                    with redirect_stdout(output):
                        statuses[output_format] = CHECKER.main([
                            "--graph", str(graph_path), "--plan", str(plan_path), "--format", output_format,
                        ])
                    outputs[output_format] = output.getvalue()
                self.assertEqual(statuses["text"], statuses["json"])
                self.assertEqual(statuses["text"], int(invalid))
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


if __name__ == "__main__":
    unittest.main()
