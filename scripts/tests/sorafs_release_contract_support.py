"""Assertions against the executable SoraFS release contract, independent of journals."""

from __future__ import annotations

import importlib
import sys
from functools import lru_cache
from pathlib import Path
from types import ModuleType


SCRIPTS_DIR = Path(__file__).resolve().parents[1]


@lru_cache(maxsize=None)
def release_module(name: str) -> ModuleType:
    """Load one repository-owned release module using its canonical import identity."""

    source = SCRIPTS_DIR / f"{name}.py"
    assert source.is_file(), f"missing release owner: {source}"
    if str(SCRIPTS_DIR) not in sys.path:
        sys.path.insert(0, str(SCRIPTS_DIR))
    module = importlib.import_module(name)
    assert Path(module.__file__).resolve() == source.resolve()
    return module


def required_release_kinds(gate: str) -> tuple[str, ...]:
    """Return the required evidence kinds enforced by the aggregate promotion gate."""

    contract = release_module("sorafs_production_readiness_contract")
    assert gate in contract.DEFAULT_REQUIRED_GATES
    return contract.GATE_BY_NAME[gate].required_kinds


def assert_canary_matches_release_gate(gate: str) -> None:
    """Bind a canary's supported schemas to the actual aggregate release contract."""

    builder = release_module(f"build_sorafs_{gate}_canary")
    contract = release_module("sorafs_production_readiness_contract")
    schemas = contract.GATE_REQUIRED_KIND_SCHEMAS[gate]
    assert {name: kind.schema for name, kind in builder.KIND_BY_NAME.items()} == schemas
    assert set(required_release_kinds(gate)) <= schemas.keys()
    assert builder.CANARY_KINDS
    assert len(builder.CANARY_KINDS) == len(set(builder.CANARY_KINDS))
    assert set(builder.CANARY_KINDS) <= schemas.keys()
