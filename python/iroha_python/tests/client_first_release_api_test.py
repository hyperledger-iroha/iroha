"""First-release Python SDK surface and typed-response contracts."""

from __future__ import annotations

from pathlib import Path

import pytest

from iroha_python import (
    ConfidentialGasSchedule,
    ConfigurationSnapshot,
    NetworkTimeSnapshot,
    NetworkTimeStatus,
    ToriiClient,
)

from .helpers import RecordingSession, StubResponse

_CONFIGURATION_PAYLOAD = {
    "public_key": "ed0120" + "11" * 32,
    "logger": {"level": "INFO", "filter": None},
    "network": {
        "block_gossip_size": 4,
        "block_gossip_period_ms": 1_000,
        "transaction_gossip_size": 16,
        "transaction_gossip_period_ms": 250,
    },
    "confidential_gas": {
        "proof_base": 1,
        "per_public_input": 2,
        "per_proof_byte": 3,
        "per_nullifier": 4,
        "per_commitment": 5,
    },
}


def test_configuration_methods_return_validated_models(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    response = StubResponse(payload=_CONFIGURATION_PAYLOAD)
    client = ToriiClient(
        "https://torii.example",
        session=RecordingSession(response),
        max_retries=0,
    )
    monkeypatch.setattr(client, "_operator_get", lambda *_args, **_kwargs: response)

    snapshot = client.get_configuration()
    gas = client.get_confidential_gas_schedule()

    assert isinstance(snapshot, ConfigurationSnapshot)
    assert snapshot.network.block_gossip_size == 4
    assert isinstance(gas, ConfidentialGasSchedule)
    assert gas.per_commitment == 5


def test_network_time_methods_return_validated_models(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    now_response = StubResponse(
        payload={"now": 1_000, "offset_ms": -3, "confidence_ms": 7}
    )
    client = ToriiClient(
        "https://torii.example",
        session=RecordingSession(now_response),
        max_retries=0,
    )

    now = client.get_time_now()
    assert isinstance(now, NetworkTimeSnapshot)
    assert now.now_ms == 1_000

    status_response = StubResponse(
        payload={"peers": 0, "samples": [], "rtt": {"buckets": [], "sum_ms": 0, "count": 0}}
    )
    monkeypatch.setattr(client, "_operator_get", lambda *_args, **_kwargs: status_response)
    status = client.get_time_status()
    assert isinstance(status, NetworkTimeStatus)
    assert status.peers == 0


@pytest.mark.parametrize(
    "retired_name",
    [
        "get_configuration_typed",
        "get_confidential_gas_schedule_typed",
        "get_time_now_typed",
        "get_time_status_typed",
    ],
)
def test_first_release_surface_has_no_duplicate_typed_names(retired_name: str) -> None:
    assert not hasattr(ToriiClient, retired_name)


def test_distribution_declares_inline_types() -> None:
    package_root = Path(__file__).resolve().parents[1] / "src" / "iroha_python"
    assert (package_root / "py.typed").is_file()
