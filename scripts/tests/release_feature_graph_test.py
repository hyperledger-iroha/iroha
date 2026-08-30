"""Regression coverage for shipping Cargo feature isolation."""

from __future__ import annotations

import importlib.util
from pathlib import Path


REPO = Path(__file__).resolve().parents[2]
SCRIPT = REPO / "scripts" / "check_release_feature_graph.py"


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


def test_shipping_proof_consumers_keep_complete_parallel_engine() -> None:
    checker = load_checker()
    for package, required_features in checker.REQUIRED_FEATURES.items():
        graph = checker.feature_graph(REPO, package)
        assert all(feature in graph for feature in required_features)
