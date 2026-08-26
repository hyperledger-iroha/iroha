"""Strict transport and binding tests for the Python Parliament API V1."""

from __future__ import annotations

import base64
import copy
import hashlib
from typing import Any, List, Optional

import pytest
import requests

from client_test_support import CANONICAL_OWNER
from iroha_torii_client import (
    PARLIAMENT_ATTEMPT_CREATE_WIRE_ID_V1,
    PARLIAMENT_TIMED_OVN_CASTING_PROOF_REQUEST_SCHEMA_V1,
    PARLIAMENT_TIMED_OVN_CASTING_PROOF_RESPONSE_SCHEMA_V1,
    PARLIAMENT_TIMED_OVN_BALLOT_CHUNK_MAX_RECORDS_V1,
    PARLIAMENT_TRANSITION_SUBMIT_WIRE_ID_V1,
    GovernanceCapabilitiesV1,
    ParliamentAttemptDraftResponseV1,
    ParliamentAttemptReadResponseV1,
    ParliamentTimedOvnCastingContextResponseV1,
    ParliamentTlePartialReleaseShareV1,
    ParliamentTleReleaseContextResponseV1,
    ParliamentTransitionDraftResponseV1,
    ToriiCanonicalRequestAuth,
    ToriiClient,
)
from iroha_torii_client.norito_frame import schema_hash_for_type_name
from sumeragi_exact_json_test_support import RecordingSession, StubResponse

NETWORK_ID = "hash:A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5#95D7"
OTHER_NETWORK_ID = "hash:45A5D35A09D284480FBA74A402D7F303B82DA0C153FC1E1083AEFC822ED07C2D#7C0F"
PROPOSAL_ID = "11" * 32
ATTEMPT_ID = "22" * 32
BODY_ID = "33" * 32
BALLOT_ID = "44" * 32
KEY_SESSION_ID = "55" * 32
PARAMETER_HASH = bytes([0x66]) * 32


def _auth(
    messages: Optional[List[bytes]] = None,
    *,
    network_id: str = NETWORK_ID,
) -> ToriiCanonicalRequestAuth:
    def sign(message: bytes) -> bytes:
        if messages is not None:
            messages.append(message)
        return bytes([0x77]) * 64

    return ToriiCanonicalRequestAuth(
        network_id=network_id,
        account_id=CANONICAL_OWNER,
        signer=sign,
        timestamp_ms=4_102_444_801_000,
        nonce="python-parliament-api-test",
    )


def _crc64_xz(payload: bytes) -> int:
    polynomial = 0xC96C_5795_D787_0F42
    mask = (1 << 64) - 1
    crc = mask
    for byte in payload:
        crc ^= byte
        for _ in range(8):
            crc = (crc >> 1) ^ (polynomial if crc & 1 else 0)
    return (crc ^ mask) & mask


def _frame(
    payload: bytes,
    schema: str = "test.opaque.v1",
    *,
    padding: int = 0,
) -> bytes:
    return b"".join(
        (
            b"NRT0",
            b"\x00\x00",
            schema_hash_for_type_name(schema),
            b"\x00",
            len(payload).to_bytes(8, "little"),
            _crc64_xz(payload).to_bytes(8, "little"),
            b"\x00",
            bytes(padding),
            payload,
        )
    )


def _json_response(payload: Any, **headers: str) -> StubResponse:
    return StubResponse(
        200,
        payload,
        headers={"Content-Type": "application/json", **headers},
    )


def _proposal() -> dict[str, Any]:
    return {
        "kind": "DeployContract",
        "payload": {
            "contract_address": "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw",
            "code_hash": "11" * 32,
            "abi_hash": "22" * 32,
            "abi_version": 1,
            "manifest_provenance": None,
        },
    }


def _runtime_upgrade_proposal(start_height: int, end_height: int) -> dict[str, Any]:
    return {
        "kind": "RuntimeUpgrade",
        "payload": {
            "manifest": {
                "name": "runtime-v1",
                "description": "exact public JSON",
                "abi_version": 1,
                "abi_hash": [0x22] * 32,
                "added_syscalls": [],
                "added_pointer_types": [],
                "start_height": start_height,
                "end_height": end_height,
                "sbom_digests": [],
                "slsa_attestation": "",
                "provenance": [],
            }
        },
    }


def _capabilities() -> dict[str, Any]:
    routes = [
        "/v1/gov/capabilities",
        "/v1/gov/parliament/attempts/draft",
        "/v1/gov/parliament/attempts/{governance_attempt_id}",
        "/v1/gov/parliament/ballots/{ballot_attempt_id}/casting-context",
        "/v1/gov/parliament/ballots/{ballot_attempt_id}/casting-proof",
        "/v1/gov/parliament/ballots/{ballot_attempt_id}/release-context",
        "/v1/gov/parliament/ballots/{ballot_attempt_id}/partial-release",
        "/v1/gov/parliament/transitions/draft",
    ]
    return {
        "schema": "iroha.governance.capabilities.v1",
        "version": 1,
        "network_id": NETWORK_ID,
        "current_height": "20",
        "network_prefix": "753",
        "abi_version": "1",
        "data_model_version": "4",
        "approval_mode": "PARLIAMENT_ATTEMPT_TIMED_OVN_V1",
        "private_ballot_protocol": "TIMED_OVN_TLE_THRESHOLD_BLS_V1",
        "mandatory_private_ballots": True,
        "proposal_backed_referendum_ballots_supported": False,
        "standalone_plain_ballots_supported": False,
        "standalone_zk_ballots_supported": True,
        "citizenship_asset_id": "citizenship#governance",
        "citizenship_bond_amount": "100",
        "citizenship_escrow_account": CANONICAL_OWNER,
        "voting_asset_id": "xor#sora",
        "min_bond_amount": "1",
        "bond_escrow_account": CANONICAL_OWNER,
        "min_enactment_delay": "10",
        "invitation_phase_blocks": "10",
        "registration_phase_blocks": "10",
        "survivor_freeze_phase_blocks": "10",
        "commitment_phase_blocks": "10",
        "release_delay_blocks": "10",
        "opening_phase_blocks": "10",
        "max_ballot_retries": "3",
        "max_corpus_entries": "1000",
        "target_body_sizes": {
            "rules_committee": "4",
            "agenda_council": "4",
            "interest_panel": "4",
            "review_panel": "4",
            "coordination_council": "4",
            "mpc_committee": "4",
            "fma_committee": "4",
            "oversight_committee": "4",
            "policy_jury": "4",
            "confirmation_jury": "4",
        },
        "supported_proposal_kinds": [
            "DEPLOY_CONTRACT",
            "MUSUBI_REGISTRY_GOVERNANCE",
            "RUNTIME_UPGRADE",
            "SCCP_ROUTE_GOVERNANCE",
            "SORAFS_PROVIDER_GOVERNANCE",
            "VALIDATION_FEE_PAYOUT_LIFECYCLE",
            "VALIDATION_FEE_POLICY",
        ],
        "supported_routes": routes,
    }


def _key_session() -> dict[str, Any]:
    bytes32 = [0x91] * 32
    bytes96 = [0x92] * 96
    return {
        "version": 1,
        "key_session_id": KEY_SESSION_ID,
        "network_id": list(bytes.fromhex(NETWORK_ID[5:69])),
        "roster_hash": bytes32,
        "committee_size": 4,
        "threshold": 2,
        "generator_h": bytes96,
        "generator_v": [0x93] * 96,
        "qualified_dealers": [1, 2],
        "qualified_dealer_commitments": [
            {
                "dealer_index": index,
                "coefficient_commitments": [[0x94 + index] * 96, [0x96 + index] * 96],
                "constant_pok_commitment": [0x98 + index] * 96,
                "constant_pok_response": [index] * 32,
            }
            for index in (1, 2)
        ],
        "dkg_event_hash": [0xA1] * 32,
        "group_public_key": [0xA2] * 96,
        "public_shares": [
            {
                "index": index,
                "participant_hash": [0xA2 + index] * 32,
                "public_key_share": [0xA6 + index] * 96,
            }
            for index in range(1, 5)
        ],
        "transcript_hash": [0xB1] * 32,
    }


def _release_identity() -> dict[str, Any]:
    return {
        "tle_key_session_id": KEY_SESSION_ID,
        "governance_attempt_id": ATTEMPT_ID,
        "body_instance_id": BODY_ID,
        "ballot_attempt_id": BALLOT_ID,
        "survivor_corpus_root": [0xC1] * 32,
        "no_recovery_root": [0xC2] * 32,
        "target_finalized_height": 30,
        "parameter_hash": list(PARAMETER_HASH),
    }


def _identity_payload() -> bytes:
    return b"".join(
        (
            b"iroha.parliament.tle.identity-payload.v1\0",
            (1).to_bytes(2, "big"),
            bytes.fromhex(ATTEMPT_ID),
            bytes.fromhex(BODY_ID),
            bytes.fromhex(BALLOT_ID),
            bytes([0xC1]) * 32,
            bytes([0xC2]) * 32,
            (30).to_bytes(8, "big"),
            PARAMETER_HASH,
        )
    )


def _identity_digest(payload: bytes) -> bytes:
    session = _key_session()
    message = b"".join(
        (
            b"iroha.threshold-bls.message.v1\0",
            b"iroha.threshold-bls.session.v1\0",
            (1).to_bytes(2, "big"),
            b"\x02",
            bytes(session["network_id"]),
            bytes.fromhex(KEY_SESSION_ID),
            bytes(session["roster_hash"]),
            (4).to_bytes(2, "big"),
            (2).to_bytes(2, "big"),
            len(payload).to_bytes(4, "big"),
            payload,
        )
    )
    return hashlib.sha256(message).digest()


def _release_context() -> dict[str, Any]:
    payload = _identity_payload()
    return {
        "version": 1,
        "current_height": 30,
        "ballot_attempt_id": BALLOT_ID,
        "governance_attempt_id": ATTEMPT_ID,
        "body_instance_id": BODY_ID,
        "status": {"status": "Opening"},
        "release_height": 30,
        "opening_deadline_height": 40,
        "tle_key_session": _key_session(),
        "release_identity": _release_identity(),
        "identity_digest": list(_identity_digest(payload)),
        "identity_payload_hex": payload.hex(),
    }


def _casting_context() -> dict[str, Any]:
    session = _key_session()
    return {
        "version": 1,
        "current_height": 20,
        "phase": "Registered",
        "session": {
            "network_id": session["network_id"],
            "proposal_content_id": PROPOSAL_ID,
            "governance_attempt_id": ATTEMPT_ID,
            "body_instance_id": BODY_ID,
            "ballot_attempt_id": BALLOT_ID,
            "parameter_hash": list(PARAMETER_HASH),
            "tle_key_session_id": KEY_SESSION_ID,
            "tle_key_transcript_hash": session["transcript_hash"],
            "tle_master_public_key": session["group_public_key"],
        },
        "registration_opened_at_finalized_height": 10,
        "target_finalized_height": 30,
        "tle_key_session": session,
        "registration_records_hex": [],
        "survivor_participant_hashes": None,
        "release_identity": None,
        "archive_norito_base64": base64.b64encode(_frame(b"casting archive")).decode("ascii"),
    }


def _attempt_read() -> dict[str, Any]:
    return {
        "version": 1,
        "current_height": 20,
        "attempt": {
            "id": ATTEMPT_ID,
            "proposal_content_id": PROPOSAL_ID,
            "sequence": 0,
            "risk_tier": {"tier": "Routine"},
            "stage": {"stage": "Qualification"},
            "status": {"status": "Active"},
        },
        "policy_version": 1,
        "required_bodies": [
            {"body": "rules-committee", "decision_mode": {"mode": "PublicFinding"}}
        ],
        "body_states": [
            {
                "body": "rules-committee",
                "body_instance_id": None,
                "status": None,
                "public_finding_opened_at_height": None,
                "public_finding_phase_blocks": None,
                "public_finding_deadline_height": None,
                "no_result_kind": None,
                "no_result_height": None,
                "timed_ovn_progress": None,
            }
        ],
        "certificate": None,
        "terminal_height": None,
        "execution_failure_root": None,
        "superseding_head": None,
        "state_payload_hex": _frame(b"attempt state").hex(),
    }


def _certificate_attempt_read(*, hidden_ballot: bool = False) -> dict[str, Any]:
    payload = _attempt_read()
    body = "policy-jury" if hidden_ballot else "rules-committee"
    decision_mode = "HiddenBindingBallot" if hidden_ballot else "PublicFinding"
    election_attempt_id = "77" * 32
    request_id = "88" * 32
    beacon_session_id = "99" * 32
    payload["required_bodies"] = [
        {"body": body, "decision_mode": {"mode": decision_mode}}
    ]
    payload["body_states"] = [
        {
            "body": body,
            "body_instance_id": BODY_ID,
            "status": {"status": "Approved"},
            "public_finding_opened_at_height": None,
            "public_finding_phase_blocks": None,
            "public_finding_deadline_height": None,
            "no_result_kind": None,
            "no_result_height": None,
            "timed_ovn_progress": {
                "ballot_attempt_id": BALLOT_ID,
                "status": {"status": "Finalized"},
                "frozen_survivor_count": 1,
                "accepted_ballot_prefix_count": 1,
            }
            if hidden_ballot
            else None,
        }
    ]
    payload["certificate"] = {
        "proposal_content_id": PROPOSAL_ID,
        "governance_attempt_id": ATTEMPT_ID,
        "governance_attempt_sequence": 0,
        "risk_tier": {"tier": "Routine"},
        "body_bindings": [
            {
                "body_instance_id": BODY_ID,
                "election_attempt_id": election_attempt_id,
                "election_attempt_sequence": 0,
                "sortition_request_id": request_id,
                "sortition_request": {
                    "id": request_id,
                    "governance_attempt_id": ATTEMPT_ID,
                    "body_election_attempt_id": election_attempt_id,
                    "body": body,
                    "candidate_root": [0x31] * 32,
                    "candidate_count": 1,
                    "target_seats": 1,
                    "request_height": 10,
                    "pulse_height": 20,
                    "beacon_session_id": beacon_session_id,
                },
                "body": body,
                "original_seats": 1,
                "beacon_session_id": beacon_session_id,
                "beacon_pulse_id": "aa" * 32,
                "roster_root": [0x32] * 32,
                "assignment_root": [0x33] * 32,
                "result_root": [0x34] * 32,
                "result_height": 25,
                "public_finding": None
                if hidden_ballot
                else {
                    "endorsement_root": [0x35] * 32,
                    "endorsing_assignments": ["bb" * 32],
                    "endorsements": 1,
                    "quorum": 1,
                },
                "ballot": {} if hidden_ballot else None,
            }
        ],
        "policy_version": 1,
        "effect_preimage_hash": [0x36] * 32,
        "expected_head": {"state": "Absent", "head": {"subject_id": [0x37] * 32}},
        "certified_at_height": 30,
        "enact_at_height": 40,
    }
    return payload


def _ballot_certificate_binding(*, commitment_closed_at_height: int = 12) -> dict[str, Any]:
    return {
        "ballot_attempt_id": BALLOT_ID,
        "ballot_attempt_sequence": 0,
        "tle_session_id": "cc" * 32,
        "tle_key_session_id": "dd" * 32,
        "registration_root": [0x41] * 32,
        "dropout_root": [0x42] * 32,
        "survivor_root": [0x43] * 32,
        "corpus_root": [0x44] * 32,
        "no_recovery_root": [0x45] * 32,
        "timed_commitment_root": [0x46] * 32,
        "release_beacon_session_id": "ee" * 32,
        "registered_at_height": 1,
        "registration_close_height": 5,
        "survivor_freeze_height": 10,
        "commitment_close_height": 20,
        "registration_closed_at_height": 5,
        "survivors_frozen_at_height": 10,
        "commitment_closed_at_height": commitment_closed_at_height,
        "max_ballot_retries": 16,
        "max_corpus_entries": 1,
        "release_height": 22,
        "opening_deadline_height": 29,
        "release_pulse_id": "ff" * 32,
        "opening_height": 23,
        "opening_root": [0x47] * 32,
        "tally": {
            "original_seats": 1,
            "accepted_ballots": 1,
            "aye": 1,
            "nay": 0,
            "abstain": 0,
        },
        "outcome": {"outcome": "Approved"},
    }


def _partial_release() -> dict[str, Any]:
    return {
        "key_session_id": KEY_SESSION_ID,
        "identity_digest": list(_identity_digest(_identity_payload())),
        "participant_index": 2,
        "sigma": [0xD1] * 48,
        "proof_x": [0xD2] * 96,
        "proof_y": [0xD3] * 48,
        "z_s": [0xD4] * 32,
        "z_r": [0xD5] * 32,
        "z_u": [0xD6] * 32,
    }


def test_all_canonical_routes_are_authenticated_bounded_and_bound() -> None:
    session = RecordingSession()
    request_frame = _frame(
        b"request",
        PARLIAMENT_TIMED_OVN_CASTING_PROOF_REQUEST_SCHEMA_V1,
    )
    response_frame = _frame(
        b"response",
        PARLIAMENT_TIMED_OVN_CASTING_PROOF_RESPONSE_SCHEMA_V1,
    )
    transition_digest = bytes([0xE1]) * 32
    for response in (
        _json_response(_capabilities()),
        _json_response(
            {
                "version": 1,
                "proposal_content_id": PROPOSAL_ID,
                "governance_attempt_id": ATTEMPT_ID,
                "tx_instructions": [
                    {
                        "wire_id": PARLIAMENT_ATTEMPT_CREATE_WIRE_ID_V1,
                        "payload_hex": _frame(b"attempt instruction").hex(),
                    }
                ],
            }
        ),
        _json_response(_attempt_read()),
        _json_response(_casting_context()),
        StubResponse(
            200,
            raw=response_frame,
            headers={"Content-Type": "application/x-norito"},
        ),
        _json_response(_release_context()),
        _json_response(_partial_release()),
        _json_response(
            {
                "version": 1,
                "governance_attempt_id": ATTEMPT_ID,
                "transition_kind": {"kind": "CompleteQualification"},
                "transition_digest": list(transition_digest),
                "tx_instructions": [
                    {
                        "wire_id": PARLIAMENT_TRANSITION_SUBMIT_WIRE_ID_V1,
                        "payload_hex": _frame(b"transition instruction").hex(),
                    }
                ],
            }
        ),
    ):
        session.queue(response)
    client = ToriiClient("https://node.test", session=session)
    auth = _auth([])

    assert isinstance(client.get_governance_capabilities_v1(canonical_auth=auth), GovernanceCapabilitiesV1)
    assert isinstance(
        client.draft_parliament_attempt_v1(
            _proposal(),
            0,
            expected_proposal_content_id=PROPOSAL_ID,
            expected_governance_attempt_id=ATTEMPT_ID,
            canonical_auth=auth,
        ),
        ParliamentAttemptDraftResponseV1,
    )
    assert isinstance(
        client.get_parliament_attempt_v1(ATTEMPT_ID, canonical_auth=auth),
        ParliamentAttemptReadResponseV1,
    )
    assert isinstance(
        client.get_parliament_timed_ovn_casting_context_v1(BALLOT_ID, canonical_auth=auth),
        ParliamentTimedOvnCastingContextResponseV1,
    )
    assert (
        client.get_parliament_timed_ovn_casting_proof_page_v1(
            BALLOT_ID, request_frame, canonical_auth=auth
        )
        == response_frame
    )
    context = client.get_parliament_tle_release_context_v1(BALLOT_ID, canonical_auth=auth)
    assert isinstance(context, ParliamentTleReleaseContextResponseV1)
    assert isinstance(
        client.request_parliament_tle_partial_release_v1(
            BALLOT_ID, context, canonical_auth=auth
        ),
        ParliamentTlePartialReleaseShareV1,
    )
    assert isinstance(
        client.draft_parliament_transition_v1(
            ATTEMPT_ID,
            {"transition": "CompleteQualification"},
            expected_transition_digest=transition_digest,
            canonical_auth=auth,
        ),
        ParliamentTransitionDraftResponseV1,
    )

    assert [call["method"] for call in session.calls] == [
        "GET",
        "POST",
        "GET",
        "GET",
        "POST",
        "GET",
        "POST",
        "POST",
    ]
    assert all(call["allow_redirects"] is False for call in session.calls)
    assert all(call["stream"] is True for call in session.calls)
    partial_call = session.calls[6]
    assert partial_call["data"] is None
    assert "Content-Type" not in partial_call["headers"]
    casting_call = session.calls[4]
    assert casting_call["data"] == request_frame
    assert casting_call["headers"]["Content-Type"] == "application/x-norito"
    assert casting_call["headers"]["Accept"] == "application/x-norito"


@pytest.mark.parametrize("identifier", ["0" * 64, "AA" * 32, "11" * 31, "not-an-id"])
def test_noncanonical_or_zero_ids_fail_before_dispatch(identifier: str) -> None:
    session = RecordingSession()
    client = ToriiClient("https://node.test", session=session)
    with pytest.raises(TypeError, match="64 lowercase non-zero"):
        client.get_parliament_attempt_v1(identifier, canonical_auth=_auth())
    assert session.calls == []


def test_strict_json_rejects_duplicates_media_type_and_size() -> None:
    duplicate = RecordingSession()
    duplicate.queue(
        StubResponse(
            200,
            raw=b'{"schema":"first","schema":"second"}',
            headers={"Content-Type": "application/json"},
        )
    )
    with pytest.raises(ValueError, match="duplicate field `schema`"):
        ToriiClient("https://node.test", session=duplicate).get_governance_capabilities_v1(
            canonical_auth=_auth()
        )

    media = RecordingSession()
    media.queue(StubResponse(200, raw=b"{}", headers={"Content-Type": "text/plain"}))
    with pytest.raises(TypeError, match="application/json content type"):
        ToriiClient("https://node.test", session=media).get_governance_capabilities_v1(
            canonical_auth=_auth()
        )

    oversized = RecordingSession()
    oversized.queue(
        StubResponse(
            200,
            raw=b"{}",
            headers={
                "Content-Type": "application/json",
                "Content-Length": str(64 * 1024 + 1),
            },
        )
    )
    with pytest.raises(ValueError, match="65536-byte size bound"):
        ToriiClient("https://node.test", session=oversized).get_governance_capabilities_v1(
            canonical_auth=_auth()
        )


def test_casting_proof_requires_exact_schema_frames() -> None:
    session = RecordingSession()
    client = ToriiClient("https://node.test", session=session)
    with pytest.raises(ValueError, match="schema hash"):
        client.get_parliament_timed_ovn_casting_proof_page_v1(
            BALLOT_ID, _frame(b"wrong", "wrong.schema"), canonical_auth=_auth()
        )
    assert session.calls == []

    bad_response = RecordingSession()
    bad_response.queue(
        StubResponse(
            200,
            raw=_frame(b"wrong response", "wrong.response.schema"),
            headers={"Content-Type": "application/x-norito"},
        )
    )
    with pytest.raises(ValueError, match="schema hash"):
        ToriiClient(
            "https://node.test", session=bad_response
        ).get_parliament_timed_ovn_casting_proof_page_v1(
            BALLOT_ID,
            _frame(b"request", PARLIAMENT_TIMED_OVN_CASTING_PROOF_REQUEST_SCHEMA_V1),
            canonical_auth=_auth(),
        )
    assert len(bad_response.calls) == 1

    padded = RecordingSession()
    with pytest.raises(ValueError, match="uses 1 bytes of alignment padding"):
        ToriiClient(
            "https://node.test", session=padded
        ).get_parliament_timed_ovn_casting_proof_page_v1(
            BALLOT_ID,
            _frame(
                b"request",
                PARLIAMENT_TIMED_OVN_CASTING_PROOF_REQUEST_SCHEMA_V1,
                padding=1,
            ),
            canonical_auth=_auth(),
        )
    assert padded.calls == []


def test_release_digest_and_partial_context_rebinding_fail_closed() -> None:
    release = _release_context()
    release["identity_digest"][0] ^= 1
    release_session = RecordingSession()
    release_session.queue(_json_response(release))
    with pytest.raises(ValueError, match="identity_digest differs"):
        ToriiClient(
            "https://node.test", session=release_session
        ).get_parliament_tle_release_context_v1(BALLOT_ID, canonical_auth=_auth())

    valid_session = RecordingSession()
    valid_session.queue(_json_response(_release_context()))
    client = ToriiClient("https://node.test", session=valid_session)
    context = client.get_parliament_tle_release_context_v1(BALLOT_ID, canonical_auth=_auth())
    partial_session = RecordingSession()
    wrong = _partial_release()
    wrong["key_session_id"] = "99" * 32
    partial_session.queue(_json_response(wrong))
    with pytest.raises(ValueError, match="key_session_id differs"):
        ToriiClient(
            "https://node.test", session=partial_session
        ).request_parliament_tle_partial_release_v1(
            BALLOT_ID, context, canonical_auth=_auth()
        )


def test_transition_rejects_private_material_and_automatic_outcomes() -> None:
    client = ToriiClient("https://node.test", session=RecordingSession())
    with pytest.raises(TypeError, match="private_key is forbidden"):
        client.draft_parliament_transition_v1(
            ATTEMPT_ID,
            {
                "transition": "EndorsePublicFinding",
                "payload": {
                    "body_instance_id": BODY_ID,
                    "result_root": [1] * 32,
                    "private_key": "never",
                },
            },
            expected_transition_digest=bytes([1]) * 32,
            canonical_auth=_auth(),
        )
    with pytest.raises(TypeError, match="unknown, removed, or consensus-owned"):
        client.draft_parliament_transition_v1(
            ATTEMPT_ID,
            {"transition": "Enacted"},
            expected_transition_digest=bytes([1]) * 32,
            canonical_auth=_auth(),
        )


def test_request_bindings_and_exact_proposal_integers_fail_before_dispatch() -> None:
    session = RecordingSession()
    client = ToriiClient("https://node.test", session=session)
    with pytest.raises(TypeError, match="64 lowercase non-zero"):
        client.draft_parliament_attempt_v1(
            _proposal(),
            0,
            expected_proposal_content_id="0" * 64,
            expected_governance_attempt_id=ATTEMPT_ID,
            canonical_auth=_auth(),
        )
    with pytest.raises(TypeError, match="9007199254740991"):
        client.draft_parliament_attempt_v1(
            _runtime_upgrade_proposal(1 << 53, (1 << 53) + 1),
            0,
            expected_proposal_content_id=PROPOSAL_ID,
            expected_governance_attempt_id=ATTEMPT_ID,
            canonical_auth=_auth(),
        )
    with pytest.raises(TypeError, match="32 non-zero immutable bytes"):
        client.draft_parliament_transition_v1(
            ATTEMPT_ID,
            {"transition": "CompleteQualification"},
            expected_transition_digest=bytes(32),
            canonical_auth=_auth(),
        )
    assert session.calls == []


def test_transition_nested_shapes_fail_before_dispatch() -> None:
    session = RecordingSession()
    client = ToriiClient("https://node.test", session=session)
    with pytest.raises(TypeError, match="canonical Parliament body"):
        client.draft_parliament_transition_v1(
            ATTEMPT_ID,
            {
                "transition": "RecordInvitationResponse",
                "payload": {
                    "election_attempt_id": BODY_ID,
                    "body": "shadow-council",
                    "decision": {"decision": "Accept"},
                },
            },
            expected_transition_digest=bytes([1]) * 32,
            canonical_auth=_auth(),
        )
    with pytest.raises(TypeError, match="missing required field `beacon_session_id`"):
        client.draft_parliament_transition_v1(
            ATTEMPT_ID,
            {
                "transition": "RegisterSortitionRequest",
                "payload": {
                    "sequence": 0,
                    "request": {},
                    "candidate_snapshot": [CANONICAL_OWNER],
                },
            },
            expected_transition_digest=bytes([1]) * 32,
            canonical_auth=_auth(),
        )
    assert session.calls == []


def test_freeze_timed_ovn_corpus_enforces_the_32_record_call_boundary() -> None:
    digest = bytes([0xE2]) * 32
    canonical_record = [0xFF] * 2_858
    accepted = RecordingSession()
    accepted.queue(
        _json_response(
            {
                "version": 1,
                "governance_attempt_id": ATTEMPT_ID,
                "transition_kind": {"kind": "FreezeTimedOvnCorpus"},
                "transition_digest": list(digest),
                "tx_instructions": [
                    {
                        "wire_id": PARLIAMENT_TRANSITION_SUBMIT_WIRE_ID_V1,
                        "payload_hex": _frame(b"bounded freeze instruction").hex(),
                    }
                ],
            }
        )
    )
    result = ToriiClient(
        "https://node.test", session=accepted
    ).draft_parliament_transition_v1(
        ATTEMPT_ID,
        {
            "transition": "FreezeTimedOvnCorpus",
            "payload": {
                "ballot_attempt_id": BALLOT_ID,
                "ballot_records": [
                    canonical_record
                    for _ in range(PARLIAMENT_TIMED_OVN_BALLOT_CHUNK_MAX_RECORDS_V1)
                ],
            },
        },
        expected_transition_digest=digest,
        canonical_auth=_auth(),
    )
    assert result.transition_kind == "FreezeTimedOvnCorpus"
    assert len(accepted.calls) == 1

    rejected = RecordingSession()
    with pytest.raises(ValueError, match="one through 32 records"):
        ToriiClient(
            "https://node.test", session=rejected
        ).draft_parliament_transition_v1(
            ATTEMPT_ID,
            {
                "transition": "FreezeTimedOvnCorpus",
                "payload": {
                    "ballot_attempt_id": BALLOT_ID,
                    "ballot_records": [
                        canonical_record
                        for _ in range(
                            PARLIAMENT_TIMED_OVN_BALLOT_CHUNK_MAX_RECORDS_V1 + 1
                        )
                    ],
                },
            },
            expected_transition_digest=digest,
            canonical_auth=_auth(),
        )
    assert rejected.calls == []


def test_instruction_payload_must_be_a_canonical_norito_frame() -> None:
    session = RecordingSession()
    session.queue(
        _json_response(
            {
                "version": 1,
                "proposal_content_id": PROPOSAL_ID,
                "governance_attempt_id": ATTEMPT_ID,
                "tx_instructions": [
                    {
                        "wire_id": PARLIAMENT_ATTEMPT_CREATE_WIRE_ID_V1,
                        "payload_hex": "00ff",
                    }
                ],
            }
        )
    )
    with pytest.raises(ValueError, match="40-byte Norito header"):
        ToriiClient("https://node.test", session=session).draft_parliament_attempt_v1(
            _proposal(),
            0,
            expected_proposal_content_id=PROPOSAL_ID,
            expected_governance_attempt_id=ATTEMPT_ID,
            canonical_auth=_auth(),
        )
    assert len(session.calls) == 1


def test_attempt_certificate_rejects_unknown_nested_sortition_fields() -> None:
    payload = _certificate_attempt_read()
    payload["certificate"]["body_bindings"][0]["sortition_request"][
        "candidate_snapshot"
    ] = [CANONICAL_OWNER]
    session = RecordingSession()
    session.queue(_json_response(payload))
    with pytest.raises(TypeError, match="contains unknown field `candidate_snapshot`"):
        ToriiClient("https://node.test", session=session).get_parliament_attempt_v1(
            ATTEMPT_ID, canonical_auth=_auth()
        )


def test_attempt_certificate_rejects_unparsed_finding_and_ballot_objects() -> None:
    public = _certificate_attempt_read()
    public["certificate"]["body_bindings"][0]["public_finding"] = {}
    public_session = RecordingSession()
    public_session.queue(_json_response(public))
    with pytest.raises(TypeError, match="missing required field `endorsement_root`"):
        ToriiClient(
            "https://node.test", session=public_session
        ).get_parliament_attempt_v1(ATTEMPT_ID, canonical_auth=_auth())

    hidden = _certificate_attempt_read(hidden_ballot=True)
    hidden_session = RecordingSession()
    hidden_session.queue(_json_response(hidden))
    with pytest.raises(TypeError, match="missing required field `ballot_attempt_id`"):
        ToriiClient(
            "https://node.test", session=hidden_session
        ).get_parliament_attempt_v1(ATTEMPT_ID, canonical_auth=_auth())


def test_attempt_certificate_accepts_early_terminal_commitment_chunk_only_inside_window() -> None:
    early = _certificate_attempt_read(hidden_ballot=True)
    early["certificate"]["body_bindings"][0]["ballot"] = _ballot_certificate_binding()
    session = RecordingSession()
    session.queue(_json_response(early))
    normalized = ToriiClient(
        "https://node.test", session=session
    ).get_parliament_attempt_v1(ATTEMPT_ID, canonical_auth=_auth())
    assert (
        normalized.raw["certificate"]["body_bindings"][0]["ballot"][
            "commitment_closed_at_height"
        ]
        == 12
    )

    for invalid_height in (10, 21):
        invalid = _certificate_attempt_read(hidden_ballot=True)
        invalid["certificate"]["body_bindings"][0]["ballot"] = (
            _ballot_certificate_binding(
                commitment_closed_at_height=invalid_height
            )
        )
        invalid_session = RecordingSession()
        invalid_session.queue(_json_response(invalid))
        with pytest.raises(ValueError, match="outside their slots"):
            ToriiClient(
                "https://node.test", session=invalid_session
            ).get_parliament_attempt_v1(ATTEMPT_ID, canonical_auth=_auth())


def test_attempt_read_exposes_only_phase_bound_aggregate_timed_ovn_progress() -> None:
    finalized = _certificate_attempt_read(hidden_ballot=True)
    finalized["certificate"]["body_bindings"][0]["ballot"] = (
        _ballot_certificate_binding()
    )
    session = RecordingSession()
    session.queue(_json_response(finalized))
    normalized = ToriiClient(
        "https://node.test", session=session
    ).get_parliament_attempt_v1(ATTEMPT_ID, canonical_auth=_auth())
    progress = normalized.timed_ovn_progress[0]
    assert progress is not None
    assert progress.ballot_attempt_id == BALLOT_ID
    assert progress.accepted_ballot_prefix_count == 1

    active = _certificate_attempt_read(hidden_ballot=True)
    active["attempt"]["status"] = {"status": "Active"}
    active["certificate"] = None
    active["body_states"][0]["status"] = {"status": "Balloting"}
    active_progress = active["body_states"][0]["timed_ovn_progress"]
    active_progress["status"] = {"status": "TimedCommitment"}
    active_progress["accepted_ballot_prefix_count"] = 0
    active_session = RecordingSession()
    active_session.queue(_json_response(active))
    parsed = ToriiClient(
        "https://node.test", session=active_session
    ).get_parliament_attempt_v1(ATTEMPT_ID, canonical_auth=_auth())
    assert parsed.timed_ovn_progress[0].accepted_ballot_prefix_count == 0

    for mutate, message in (
        (
            lambda item: item["timed_ovn_progress"].__setitem__(
                "accepted_ballot_prefix_count", 1
            ),
            "prefix must remain incomplete",
        ),
        (
            lambda item: item["timed_ovn_progress"].__setitem__(
                "frozen_survivor_count", None
            ),
            "must appear together",
        ),
        (
            lambda item: item["timed_ovn_progress"].__setitem__("ballot_records", []),
            "unknown field `ballot_records`",
        ),
    ):
        malformed = copy.deepcopy(active)
        mutate(malformed["body_states"][0])
        malformed_session = RecordingSession()
        malformed_session.queue(_json_response(malformed))
        with pytest.raises((TypeError, ValueError), match=message):
            ToriiClient(
                "https://node.test", session=malformed_session
            ).get_parliament_attempt_v1(ATTEMPT_ID, canonical_auth=_auth())

    forged = copy.deepcopy(finalized)
    forged["body_states"][0]["timed_ovn_progress"]["ballot_attempt_id"] = "ab" * 32
    forged_session = RecordingSession()
    forged_session.queue(_json_response(forged))
    with pytest.raises(ValueError, match="differs from timed_ovn_progress"):
        ToriiClient(
            "https://node.test", session=forged_session
        ).get_parliament_attempt_v1(ATTEMPT_ID, canonical_auth=_auth())


def test_attempt_certificate_rejects_forged_repeated_and_sortition_bindings() -> None:
    mutations = (
        (
            lambda payload: payload["certificate"].__setitem__(
                "proposal_content_id", "ab" * 32
            ),
            "proposal_content_id differs",
        ),
        (
            lambda payload: payload["certificate"].__setitem__(
                "governance_attempt_sequence", 1
            ),
            "governance_attempt_sequence differs",
        ),
        (
            lambda payload: payload["certificate"].__setitem__(
                "risk_tier", {"tier": "Emergency"}
            ),
            "risk_tier differs",
        ),
        (
            lambda payload: payload["certificate"].__setitem__("policy_version", 2),
            "policy_version differs",
        ),
        (
            lambda payload: payload["certificate"].__setitem__("enact_at_height", 30),
            "must follow certified_at_height",
        ),
        (
            lambda payload: payload["certificate"]["body_bindings"][0][
                "sortition_request"
            ].__setitem__("beacon_session_id", "bc" * 32),
            "sortition_request differs from its binding indexes",
        ),
        (
            lambda payload: payload["certificate"]["body_bindings"][0][
                "sortition_request"
            ].__setitem__("request_height", 0),
            "request_height",
        ),
        (
            lambda payload: payload["certificate"]["body_bindings"][0][
                "sortition_request"
            ].__setitem__("candidate_count", 1_001),
            "candidate_count",
        ),
    )
    canonical = _certificate_attempt_read()
    for mutate, message in mutations:
        forged = copy.deepcopy(canonical)
        mutate(forged)
        session = RecordingSession()
        session.queue(_json_response(forged))
        with pytest.raises((TypeError, ValueError), match=message):
            ToriiClient(
                "https://node.test", session=session
            ).get_parliament_attempt_v1(ATTEMPT_ID, canonical_auth=_auth())


def test_attempt_certificate_rejects_required_body_and_hidden_ballot_forgery() -> None:
    wrong_mode = _attempt_read()
    wrong_mode["required_bodies"][0]["decision_mode"] = {
        "mode": "HiddenBindingBallot"
    }
    wrong_mode_session = RecordingSession()
    wrong_mode_session.queue(_json_response(wrong_mode))
    with pytest.raises(ValueError, match="differs from the body protocol"):
        ToriiClient(
            "https://node.test", session=wrong_mode_session
        ).get_parliament_attempt_v1(ATTEMPT_ID, canonical_auth=_auth())

    mutations = (
        lambda ballot: ballot.__setitem__("ballot_attempt_sequence", 17),
        lambda ballot: ballot.__setitem__("max_corpus_entries", 0),
        lambda ballot: ballot["tally"].__setitem__("abstain", 1),
        lambda ballot: ballot.__setitem__("outcome", {"outcome": "Rejected"}),
    )
    for mutate in mutations:
        forged = _certificate_attempt_read(hidden_ballot=True)
        ballot = _ballot_certificate_binding()
        mutate(ballot)
        forged["certificate"]["body_bindings"][0]["ballot"] = ballot
        session = RecordingSession()
        session.queue(_json_response(forged))
        with pytest.raises((TypeError, ValueError)):
            ToriiClient(
                "https://node.test", session=session
            ).get_parliament_attempt_v1(ATTEMPT_ID, canonical_auth=_auth())

def test_partial_release_rejects_cross_network_context_before_dispatch() -> None:
    context_session = RecordingSession()
    context_session.queue(_json_response(_release_context()))
    context = ToriiClient(
        "https://node.test", session=context_session
    ).get_parliament_tle_release_context_v1(BALLOT_ID, canonical_auth=_auth())

    partial_session = RecordingSession()
    with pytest.raises(ValueError, match="context network differs"):
        ToriiClient(
            "https://other-node.test", session=partial_session
        ).request_parliament_tle_partial_release_v1(
            BALLOT_ID,
            context,
            canonical_auth=_auth(network_id=OTHER_NETWORK_ID),
        )
    assert partial_session.calls == []


def test_ambient_credentials_and_nonidentity_responses_fail_closed() -> None:
    ambient = RecordingSession()
    ambient.auth = ("ambient", "credential")
    with pytest.raises(ValueError, match="Session.auth credential fallback"):
        ToriiClient(
            "https://node.test", session=ambient
        ).get_governance_capabilities_v1(canonical_auth=_auth())
    assert ambient.calls == []

    encoded = RecordingSession()
    encoded.queue(
        StubResponse(
            200,
            _capabilities(),
            headers={
                "Content-Type": "application/json",
                "Content-Encoding": "gzip",
            },
        )
    )
    with pytest.raises(TypeError, match="must not use content encoding"):
        ToriiClient(
            "https://node.test", session=encoded
        ).get_governance_capabilities_v1(canonical_auth=_auth())


def test_environment_netrc_cannot_add_ambient_authorization(monkeypatch: Any) -> None:
    monkeypatch.setattr(
        requests.sessions,
        "get_netrc_auth",
        lambda _url: ("ambient-user", "ambient-password"),
    )
    session = RecordingSession()
    session.trust_env = True
    session.queue(_json_response(_capabilities()))
    ToriiClient(
        "https://node.test", session=session
    ).get_governance_capabilities_v1(canonical_auth=_auth())
    assert "Authorization" not in session.calls[0]["headers"]


def test_environment_proxy_and_ca_bundle_are_not_merged(monkeypatch: Any) -> None:
    class TransportRecordingSession(RecordingSession):
        send_settings: dict[str, Any]

        def send(
            self,
            request: requests.PreparedRequest,
            **kwargs: Any,
        ) -> requests.Response:
            self.send_settings = dict(kwargs)
            return super().send(request, **kwargs)

    monkeypatch.setenv(
        "HTTPS_PROXY",
        "https://ambient-user:ambient-password@ambient-proxy.test:8443",
    )
    monkeypatch.setenv("REQUESTS_CA_BUNDLE", "/ambient/ca-bundle.pem")
    session = TransportRecordingSession()
    session.trust_env = True
    session.queue(_json_response(_capabilities()))

    ToriiClient(
        "https://node.test", session=session
    ).get_governance_capabilities_v1(canonical_auth=_auth())

    assert session.send_settings["proxies"] == {}
    assert session.send_settings["verify"] is True
    assert session.send_settings["cert"] is None
    assert "Proxy-Authorization" not in session.calls[0]["headers"]


def test_json_media_parameters_and_integer_tokens_are_strict() -> None:
    media = RecordingSession()
    media.queue(
        StubResponse(
            200,
            _capabilities(),
            headers={"Content-Type": "application/json; boundary=not-json"},
        )
    )
    with pytest.raises(TypeError, match="UTF-8 application/json"):
        ToriiClient(
            "https://node.test", session=media
        ).get_governance_capabilities_v1(canonical_auth=_auth())

    floating = RecordingSession()
    floating.queue(
        StubResponse(
            200,
            raw=b'{"version":1.0}',
            headers={"Content-Type": "application/json"},
        )
    )
    with pytest.raises(ValueError, match="must not contain floating-point numbers"):
        ToriiClient(
            "https://node.test", session=floating
        ).get_governance_capabilities_v1(canonical_auth=_auth())

    boolean_version = _capabilities()
    boolean_version["version"] = True
    boolean = RecordingSession()
    boolean.queue(_json_response(boolean_version))
    with pytest.raises(TypeError, match="version must be an integer"):
        ToriiClient(
            "https://node.test", session=boolean
        ).get_governance_capabilities_v1(canonical_auth=_auth())


def test_capabilities_reject_unsupported_data_model_version() -> None:
    payload = _capabilities()
    payload["data_model_version"] = "5"
    session = RecordingSession()
    session.queue(_json_response(payload))
    with pytest.raises(TypeError, match="unsupported data-model version"):
        ToriiClient(
            "https://node.test", session=session
        ).get_governance_capabilities_v1(canonical_auth=_auth())
