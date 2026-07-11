"""Regression tests for the first-release SoraFS PoR client surface."""

import ast
from pathlib import Path


CLIENT_SOURCE = Path(__file__).resolve().parents[1] / "src" / "iroha_python" / "client.py"


def torii_client_methods() -> set[str]:
    """Return methods declared directly on the public Torii client."""

    module = ast.parse(CLIENT_SOURCE.read_text(encoding="utf-8"))
    client = next(
        node
        for node in module.body
        if isinstance(node, ast.ClassDef) and node.name == "ToriiClient"
    )
    return {
        node.name
        for node in client.body
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
    }


def test_retired_por_mutation_methods_are_absent() -> None:
    """Clients must not expose challenge injection or manual observations."""

    methods = torii_client_methods()
    for method_name in (
        "record_sorafs_por_challenge",
        "submit_sorafs_por_observation",
    ):
        assert method_name not in methods


def test_live_por_methods_remain_available() -> None:
    """Authenticated proof/verdict and read-only methods stay public."""

    methods = torii_client_methods()
    for method_name in (
        "record_sorafs_por_proof",
        "record_sorafs_por_verdict",
        "get_sorafs_por_status",
        "export_sorafs_por_status",
        "get_sorafs_por_weekly_report",
        "get_sorafs_por_ingestion_status",
    ):
        assert method_name in methods
