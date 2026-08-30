"""Regression coverage for shipping Cargo feature isolation."""

from __future__ import annotations

import importlib.util
from pathlib import Path


REPO = Path(__file__).resolve().parents[2]
SCRIPT = REPO / "scripts" / "check_release_feature_graph.py"
WORKFLOW = REPO / ".github" / "workflows" / "pr.yml"


def load_checker():
    spec = importlib.util.spec_from_file_location("release_feature_graph", SCRIPT)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_shipping_packages_exclude_test_fixtures() -> None:
    checker = load_checker()
    for package in checker.DEFAULT_PACKAGES:
        graph = checker.feature_graph(REPO, package)
        assert all(feature not in graph for feature in checker.FORBIDDEN_FEATURES)


def test_release_package_inventory_matches_built_binaries() -> None:
    checker = load_checker()
    assert checker.DEFAULT_PACKAGES == (
        "irohad",
        "iroha_cli",
        "iroha_genesis",
        "iroha_kagami",
        "ivm",
    )


def test_forbidden_feature_detection_rejects_core_test_surface() -> None:
    checker = load_checker()
    graph = 'iroha_core feature "iroha-core-tests"\n'
    assert checker.forbidden_features_in_graph(graph) == (
        'iroha_core feature "iroha-core-tests"',
    )


def test_pr_workflow_runs_release_feature_graph_guard() -> None:
    workflow = WORKFLOW.read_text(encoding="utf-8")
    assert "scripts/tests/release_feature_graph_test.py" in workflow
    assert "python3 scripts/check_release_feature_graph.py" in workflow
