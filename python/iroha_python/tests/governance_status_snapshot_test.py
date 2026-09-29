"""Tests for the closed first-release governance counters in `/v1/status`."""

import pytest

import iroha_python
from iroha_python.client import ToriiStatusPayload


def test_governance_status_uses_closed_proposal_lifecycle_counters() -> None:
    status = ToriiStatusPayload.from_payload(
        {
            "governance": {
                "proposals": {
                    "proposed": 1,
                    "rejected": 2,
                    "enacted": 3,
                    "superseded": 4,
                    "execution_failed": 5,
                },
                "protected_namespace": {
                    "total_checks": 0,
                    "allowed": 0,
                    "rejected": 0,
                },
                "manifest_admission": {
                    "total_checks": 0,
                    "allowed": 0,
                    "missing_manifest": 0,
                    "non_validator_authority": 0,
                    "quorum_rejected": 0,
                    "protected_namespace_rejected": 0,
                    "runtime_hook_rejected": 0,
                },
                "manifest_quorum": {
                    "total_checks": 0,
                    "satisfied": 0,
                    "rejected": 0,
                },
                "recent_manifest_activations": [],
            }
        }
    )

    assert status.governance is not None
    assert status.governance.proposals.proposed == 1
    assert status.governance.proposals.rejected == 2
    assert status.governance.proposals.enacted == 3
    assert status.governance.proposals.superseded == 4
    assert status.governance.proposals.execution_failed == 5
    assert not hasattr(status.governance.proposals, "approved")


@pytest.mark.parametrize(
    "field_name", ["lane_commitments", "dataspace_commitments", "pipeline_execution"]
)
@pytest.mark.parametrize("value", [None, [], [{"block_height": 10}]])
def test_status_rejects_retired_commitment_projections(field_name: str, value: object) -> None:
    with pytest.raises(ValueError, match=f"retired field `{field_name}`"):
        ToriiStatusPayload.from_payload({field_name: value})


def test_status_has_no_retired_commitment_projection_exports() -> None:
    status = ToriiStatusPayload.from_payload({"lane_governance": []})
    for field_name in ("lane_commitments", "dataspace_commitments"):
        assert not hasattr(status, field_name)
    for type_name in ("ToriiLaneCommitmentSnapshot", "ToriiDataspaceCommitmentSnapshot"):
        assert not hasattr(iroha_python, type_name)
