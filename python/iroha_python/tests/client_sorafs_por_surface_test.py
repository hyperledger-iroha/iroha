"""First-release SoraFS PoR client-surface regressions."""

import iroha_python
from iroha_python import ToriiClient


def test_unsupported_por_mutation_helpers_are_absent() -> None:
    """The SDK must not expose methods for unregistered Torii routes."""

    assert not hasattr(ToriiClient, "record_sorafs_por_challenge")
    assert not hasattr(ToriiClient, "submit_sorafs_por_observation")
    assert not hasattr(iroha_python, "SorafsPorObservationResponse")


def test_authenticated_por_evidence_helpers_remain_available() -> None:
    """Provider proof and auditor verdict submission remain supported."""

    assert hasattr(ToriiClient, "record_sorafs_por_proof")
    assert hasattr(ToriiClient, "record_sorafs_por_verdict")
