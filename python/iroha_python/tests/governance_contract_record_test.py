"""Tests for the exact governed-contract lifecycle response projection."""

from __future__ import annotations

import copy

import pytest

from iroha_python.client import GovernanceContractRecord

CONTRACT_ADDRESS = "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
OWNER = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"


def active_response() -> dict[str, object]:
    """Return one exact active governed-contract response."""

    return {
        "found": True,
        "contract_address": CONTRACT_ADDRESS,
        "contract_subject_account": OWNER,
        "dataspace": "universal",
        "active": True,
        "lifecycle": {
            "version": 1,
            "origin": "direct",
            "origin_account": OWNER,
            "origin_proposal_content_id_hex": None,
            "origin_governance_attempt_id_hex": None,
            "owner": OWNER,
            "pending_owner": "parliament",
            "parliament_delegated": True,
            "active_code_hash_hex": "22" * 32,
            "revision": 7,
            "emergency_hold": None,
        },
        "emergency_hold_active": False,
        "code_hash_hex": "22" * 32,
        "abi_hash_hex": "33" * 32,
        "public_entrypoints": ["transfer", "view_balance"],
    }


def test_governance_contract_record_parses_active_and_absent_shapes() -> None:
    active = GovernanceContractRecord.from_payload(active_response())
    assert active.active is True
    assert active.lifecycle is not None
    assert active.lifecycle.version == 1
    assert active.lifecycle.revision == 7
    assert active.public_entrypoints == ("transfer", "view_balance")

    absent = GovernanceContractRecord.from_payload(
        {
            "found": False,
            "contract_address": CONTRACT_ADDRESS,
            "dataspace": "universal",
        }
    )
    assert absent.found is False
    assert absent.active is None
    assert absent.lifecycle is None


def test_governance_contract_record_rejects_cross_field_and_shape_drift() -> None:
    invalid = []
    mismatched = active_response()
    mismatched["lifecycle"]["active_code_hash_hex"] = "44" * 32  # type: ignore[index]
    invalid.append(mismatched)
    unsorted = active_response()
    unsorted["public_entrypoints"] = ["view_balance", "transfer"]
    invalid.append(unsorted)
    alias_owner = active_response()
    alias_owner["lifecycle"]["owner"] = "alice@universal"  # type: ignore[index]
    invalid.append(alias_owner)
    direct_with_parliament_ids = active_response()
    direct_with_parliament_ids["lifecycle"]["origin_proposal_content_id_hex"] = (  # type: ignore[index]
        "55" * 32
    )
    invalid.append(direct_with_parliament_ids)
    unsupported_version = active_response()
    unsupported_version["lifecycle"]["version"] = 2  # type: ignore[index]
    invalid.append(unsupported_version)
    absent_with_extra = {
        "found": False,
        "contract_address": CONTRACT_ADDRESS,
        "dataspace": "universal",
        "active": None,
    }
    invalid.append(absent_with_extra)

    for payload in invalid:
        with pytest.raises((TypeError, ValueError)):
            GovernanceContractRecord.from_payload(copy.deepcopy(payload))
