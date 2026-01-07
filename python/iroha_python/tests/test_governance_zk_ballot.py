from __future__ import annotations

import json

import pytest

from iroha_python.client import ToriiClient

from .helpers import RecordingSession, StubResponse


def test_governance_submit_zk_ballot_normalizes_public_aliases() -> None:
    session = RecordingSession(StubResponse(payload={"ok": True}))
    client = ToriiClient("http://node.test", session=session)

    client.governance_submit_zk_ballot(
        {
            "authority": "alice@wonderland",
            "chain_id": "chain",
            "election_id": "election-1",
            "proof_b64": "AAAA",
            "public": {
                "owner": "alice@wonderland",
                "amount": "100",
                "durationBlocks": 5,
                "rootHintHex": f"0x{'Aa' * 32}",
                "nullifierHex": f"blake2b32:{'BB' * 32}",
            },
        }
    )

    payload = json.loads(session.calls[0]["data"].decode("utf-8"))
    public = payload["public"]
    assert public["owner"] == "alice@wonderland"
    assert public["amount"] == "100"
    assert public["duration_blocks"] == 5
    assert "durationBlocks" not in public
    assert public["root_hint"] == "aa" * 32
    assert "rootHintHex" not in public
    assert public["nullifier_hex"] == "bb" * 32
    assert "nullifierHex" not in public


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
                "public": {"owner": "alice@wonderland"},
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
                "owner": "alice@wonderland",
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
                "ballot": {"owner": "alice@wonderland"},
            }
        )


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
            "rootHintHex": f"0x{'Aa' * 32}",
            "nullifierHex": f"blake2b32:{'BB' * 32}",
        }
    )

    payload = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert payload["root_hint_hex"] == "aa" * 32
    assert payload["nullifier_hex"] == "bb" * 32
    assert "rootHintHex" not in payload
    assert "nullifierHex" not in payload


def test_governance_submit_zk_ballot_v1_rejects_invalid_hex_hints() -> None:
    session = RecordingSession(StubResponse(payload={"ok": True}))
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(ValueError, match="root_hint_hex"):
        client.governance_submit_zk_ballot_v1(
            {
                "authority": "alice@wonderland",
                "chain_id": "chain",
                "election_id": "election-1",
                "backend": "halo2/ipa",
                "envelope_b64": "AAAA",
                "root_hint_hex": "not-hex",
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
                    "owner": "alice@wonderland",
                    "amount": "100",
                    "duration_blocks": 5,
                    "root_hint": "not-hex",
                },
            }
        )
