"""Tests for the closed first-release governance counters in `/v1/status`."""

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
