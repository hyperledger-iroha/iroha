"""Offline compatibility tests; no peer access or signing."""
import importlib.util
from pathlib import Path
import pytest

spec = importlib.util.spec_from_file_location("check_lifecycle_roll", Path(__file__).parents[1] / "check_lifecycle_roll.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def report():
    return dict(network_id="network", consensus_config_fingerprint="fingerprint", v2_activation_height=10, committed_height=10, active=True)


def test_equal_seats_and_active_restart():
    assert module.check_reports([report()] * 4, 10) == ("network", "fingerprint")
    active = report() | {"committed_height": 12, "active": True}
    assert module.check_reports([active] * 4, 10) == ("network", "fingerprint")


@pytest.mark.parametrize("change", [{"v2_activation_height": None}, {"v2_activation_height": 11}, {"consensus_config_fingerprint": "other"}, {"network_id": "other"}, {"active": False}, {"committed_height": 9}, {"unknown": 1}])
def test_mismatch_refused(change):
    with pytest.raises(ValueError):
        module.check_reports([report(), report() | change], 10)


def test_future_local_configuration_is_not_committed_activation():
    future = report() | {"committed_height": 3, "active": False}
    with pytest.raises(ValueError, match="has not committed"):
        module.check_reports([future] * 4, 10)
