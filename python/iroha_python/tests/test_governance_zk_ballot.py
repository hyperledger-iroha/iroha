from __future__ import annotations

import json

import pytest

from iroha_python.address import AccountAddress
from iroha_python.client import ToriiClient

from .helpers import RecordingSession, StubResponse


def _canonical_owner_literal(domain: str = "wonderland") -> str:
    address = AccountAddress.from_account(domain=domain, public_key=bytes([0x11] * 32))
    ih58 = address.to_ih58(0x02F1)
    return f"{ih58}@{domain}"


def _noncanonical_owner_literal(domain: str = "wonderland") -> str:
    address = AccountAddress.from_account(domain=domain, public_key=bytes([0x22] * 32))
    return f"{address.canonical_hex()}@{domain}"


def test_governance_submit_zk_ballot_rejects_deprecated_public_inputs() -> None:
    session = RecordingSession(StubResponse(payload={"ok": True}))
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(ValueError, match="durationBlocks"):
        client.governance_submit_zk_ballot(
            {
                "authority": "alice@wonderland",
                "chain_id": "chain",
                "election_id": "election-1",
                "proof_b64": "AAAA",
                "public": {
                    "owner": _canonical_owner_literal(),
                    "amount": "100",
                    "durationBlocks": 5,
                },
            }
        )


def test_governance_submit_zk_ballot_normalizes_public_inputs() -> None:
    session = RecordingSession(StubResponse(payload={"ok": True}))
    client = ToriiClient("http://node.test", session=session)

    client.governance_submit_zk_ballot(
        {
            "authority": "alice@wonderland",
            "chain_id": "chain",
            "election_id": "election-1",
            "proof_b64": "AAAA",
            "public": {
                "owner": _canonical_owner_literal(),
                "amount": "100",
                "duration_blocks": 5,
                "root_hint": f"0x{'Cc' * 32}",
                "nullifier": bytes.fromhex("DD" * 32),
            },
        }
    )

    payload = json.loads(session.calls[0]["data"].decode("utf-8"))
    public = payload["public"]
    assert public["root_hint"] == "cc" * 32
    assert public["nullifier"] == "dd" * 32


def test_governance_submit_zk_ballot_rejects_incomplete_lock_hints() -> None:
    session = RecordingSession(StubResponse(payload={"ok": True}))
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(ValueError, match="owner, amount, duration_blocks"):
        client.governance_submit_zk_ballot(
            {
                "authority": "alice@wonderland",
                "chain_id": "chain",
                "election_id": "election-1",
                "proof_b64": "AAAA",
                "public": {"owner": _canonical_owner_literal()},
            }
        )


def test_governance_submit_zk_ballot_v1_rejects_incomplete_lock_hints() -> None:
    session = RecordingSession(StubResponse(payload={"ok": True}))
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(ValueError, match="owner, amount, duration_blocks"):
        client.governance_submit_zk_ballot_v1(
            {
                "authority": "alice@wonderland",
                "chain_id": "chain",
                "election_id": "election-1",
                "backend": "halo2/ipa",
                "envelope_b64": "AAAA",
                "owner": _canonical_owner_literal(),
            }
        )


def test_governance_submit_zk_ballot_v1_rejects_noncanonical_owner() -> None:
    session = RecordingSession(StubResponse(payload={"ok": True}))
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(ValueError, match="canonical account id form"):
        client.governance_submit_zk_ballot_v1(
            {
                "authority": "alice@wonderland",
                "chain_id": "chain",
                "election_id": "election-1",
                "backend": "halo2/ipa",
                "envelope_b64": "AAAA",
                "owner": _noncanonical_owner_literal(),
                "amount": "100",
                "duration_blocks": 5,
            }
        )


def test_governance_submit_zk_ballot_proof_v1_rejects_incomplete_lock_hints() -> None:
    session = RecordingSession(StubResponse(payload={"ok": True}))
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(ValueError, match="owner, amount, duration_blocks"):
        client.governance_submit_zk_ballot_proof_v1(
            {
                "authority": "alice@wonderland",
                "chain_id": "chain",
                "election_id": "election-1",
                "ballot": {"owner": _canonical_owner_literal()},
            }
        )


def test_governance_submit_zk_ballot_proof_v1_rejects_noncanonical_owner() -> None:
    session = RecordingSession(StubResponse(payload={"ok": True}))
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(ValueError, match="canonical account id form"):
        client.governance_submit_zk_ballot_proof_v1(
            {
                "authority": "alice@wonderland",
                "chain_id": "chain",
                "election_id": "election-1",
                "ballot": {
                    "owner": _noncanonical_owner_literal(),
                    "amount": "100",
                    "duration_blocks": 5,
                },
            }
        )


def test_governance_submit_zk_ballot_proof_v1_normalizes_hex_hints() -> None:
    session = RecordingSession(StubResponse(payload={"ok": True}))
    client = ToriiClient("http://node.test", session=session)

    client.governance_submit_zk_ballot_proof_v1(
        {
            "authority": "alice@wonderland",
            "chain_id": "chain",
            "election_id": "election-1",
            "ballot": {
                "backend": "halo2/ipa",
                "envelope_bytes": "AAE=",
                "root_hint": f"blake2b32:{'Aa' * 32}",
                "nullifier": bytes.fromhex("BB" * 32),
            },
        }
    )

    payload = json.loads(session.calls[0]["data"].decode("utf-8"))
    ballot = payload["ballot"]
    assert ballot["root_hint"] == "aa" * 32
    assert ballot["nullifier"] == "bb" * 32


def test_governance_submit_zk_ballot_v1_normalizes_hex_hints() -> None:
    session = RecordingSession(StubResponse(payload={"ok": True}))
    client = ToriiClient("http://node.test", session=session)

    client.governance_submit_zk_ballot_v1(
        {
            "authority": "alice@wonderland",
            "chain_id": "chain",
            "election_id": "election-1",
            "backend": "halo2/ipa",
            "envelope_b64": "AAAA",
            "root_hint": f"0x{'Aa' * 32}",
            "nullifier": f"blake2b32:{'BB' * 32}",
        }
    )

    payload = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert payload["root_hint"] == "aa" * 32
    assert payload["nullifier"] == "bb" * 32


def test_governance_submit_zk_ballot_v1_rejects_invalid_hex_hints() -> None:
    session = RecordingSession(StubResponse(payload={"ok": True}))
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(ValueError, match="root_hint"):
        client.governance_submit_zk_ballot_v1(
            {
                "authority": "alice@wonderland",
                "chain_id": "chain",
                "election_id": "election-1",
                "backend": "halo2/ipa",
                "envelope_b64": "AAAA",
                "root_hint": "not-hex",
            }
        )


def test_governance_submit_zk_ballot_rejects_invalid_hex_hints() -> None:
    session = RecordingSession(StubResponse(payload={"ok": True}))
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(ValueError, match="root_hint"):
        client.governance_submit_zk_ballot(
            {
                "authority": "alice@wonderland",
                "chain_id": "chain",
                "election_id": "election-1",
                "proof_b64": "AAAA",
                "public": {
                    "owner": _canonical_owner_literal(),
                    "amount": "100",
                    "duration_blocks": 5,
                    "root_hint": "not-hex",
                },
            }
        )


def test_governance_submit_zk_ballot_rejects_noncanonical_owner() -> None:
    session = RecordingSession(StubResponse(payload={"ok": True}))
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(ValueError, match="canonical account id form"):
        client.governance_submit_zk_ballot(
            {
                "authority": "alice@wonderland",
                "chain_id": "chain",
                "election_id": "election-1",
                "proof_b64": "AAAA",
                "public": {
                    "owner": _noncanonical_owner_literal(),
                    "amount": "100",
                    "duration_blocks": 5,
                },
            }
        )
