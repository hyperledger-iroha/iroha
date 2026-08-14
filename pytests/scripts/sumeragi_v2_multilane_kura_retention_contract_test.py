"""Modular-runner negative controls for Kura advert retention."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest


_SUITE_PATH = Path(__file__).with_name("sumeragi_v2_multilane_models_test.py")
_SPEC = importlib.util.spec_from_file_location(
    "sumeragi_v2_multilane_models_kura_helpers", _SUITE_PATH
)
assert _SPEC is not None
assert _SPEC.loader is not None
_SUITE = importlib.util.module_from_spec(_SPEC)
sys.modules[_SPEC.name] = _SUITE
_SPEC.loader.exec_module(_SUITE)


def test_kura_replica_retention_contract_rejects_unbound_runner_refresh_owner(
    tmp_path: Path,
) -> None:
    module = _SUITE.load_checker()
    contract = _SUITE.copy_kura_retention_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_runner.rs"
    _SUITE.replace_once_after(
        path,
        "fn run_inner(",
        "KuraReplicaAdvertRefreshOwner::from_kura(kura.as_ref(), Instant::now())",
        "KuraReplicaAdvertRefreshOwner::from_unbound_kura(kura.as_ref(), Instant::now())",
    )
    errors = _SUITE.validate_kura_retention_fixture(tmp_path, module, contract)
    assert any(
        "run_inner" in error
        and "KuraReplicaAdvertRefreshOwner::from_kura" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("relative", "anchor", "symbol"),
    (
        (
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
            "fn run_lifecycle_active_height(",
            "run_lifecycle_active_height",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
            "fn run_pending_active_height(",
            "run_pending_active_height",
        ),
    ),
)
def test_kura_replica_retention_contract_rejects_missing_lifecycle_refresh_turn(
    tmp_path: Path,
    relative: str,
    anchor: str,
    symbol: str,
) -> None:
    module = _SUITE.load_checker()
    contract = _SUITE.copy_kura_retention_fixture(tmp_path, module)
    _SUITE.replace_once_after(
        tmp_path / relative,
        anchor,
        ".service_kura_replica_advert_refresh_turn(Instant::now())",
        ".skip_kura_replica_advert_refresh_turn(Instant::now())",
    )
    errors = _SUITE.validate_kura_retention_fixture(tmp_path, module, contract)
    assert any(
        symbol in error
        and ".service_kura_replica_advert_refresh_turn(Instant::now())" in error
        for error in errors
    ), errors
