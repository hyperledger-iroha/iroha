"""First-release SoraFS PoR client-surface regressions."""

import ast
from pathlib import Path

import iroha_python
import pytest
from iroha_python import ToriiClient
from iroha_python.client import _build_sorafs_por_export_params


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
        assert not hasattr(ToriiClient, method_name)
    assert not hasattr(iroha_python, "SorafsPorObservationResponse")


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
        assert hasattr(ToriiClient, method_name)


def test_por_export_epoch_bounds_are_an_exact_pair() -> None:
    """A client must reject a half-specified range before sending it."""

    with pytest.raises(ValueError, match="must be supplied together"):
        _build_sorafs_por_export_params(41, None, None, None, None)
    with pytest.raises(ValueError, match="must be supplied together"):
        _build_sorafs_por_export_params(None, 43, None, None, None)

    params = _build_sorafs_por_export_params(41, 43, 9, 16_384, "AA")
    assert params == {
        "start_epoch": 41,
        "end_epoch": 43,
        "limit": 9,
        "max_bytes": 16_384,
        "cursor": "AA",
    }
