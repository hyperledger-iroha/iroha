"""Native identifier and RAM-LFE instruction binding tests."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from iroha_python import Instruction, TransactionConfig, TransactionDraft, authority_fee_payment


_ROOT = Path(__file__).resolve().parents[3]
_VECTOR = json.loads(
    (_ROOT / "fixtures/soracloud/identifier_receipt_vectors_v1.json").read_text(
        encoding="utf-8"
    )
)
_OTHER_ACCOUNT = "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53"
_HASH_LITERAL = (
    "hash:0E5751C026E543B2E8AB2EB06099DAA1D1E5DF47778F7787FAAB45CDF12FE3A9"
    "#6A22"
)


def _program_policy(*, active: bool = False) -> dict[str, object]:
    policy = _VECTOR["policy"]
    return {
        "program_id": {
            "name": _VECTOR["receipt"]["payload"]["execution"]["program_id"]
        },
        "owner": policy["owner"],
        "backend": policy["backend"],
        "verification_mode": {"mode": "signed"},
        "commitment": {
            "backend": policy["backend"],
            "policy_hash": _HASH_LITERAL,
            "public_parameters": [],
        },
        "resolver_public_key": policy["resolver_public_key"],
        "output_opening_public_key": policy["resolver_public_key"],
        "active": active,
        "note": "FI-operated directory resolver",
    }


def _native_receipt() -> dict[str, object]:
    receipt = _VECTOR["receipt"]
    execution = receipt["payload"]["execution"]
    opening = receipt["payload"]["opening"]
    return {
        "payload": {
            "policy_id": {"kind": "phone", "business_rule": "retail"},
            "execution": {
                "program_id": {"name": execution["program_id"]},
                "program_digest": _HASH_LITERAL,
                "backend": execution["backend"],
                "verification_mode": {"mode": execution["verification_mode"]},
                "input_ciphertext_hash": _HASH_LITERAL,
                "output_ciphertext_hash": _HASH_LITERAL,
                "parameter_digest": _HASH_LITERAL,
                "evaluation_key_digest": _HASH_LITERAL,
                "output_hash": _HASH_LITERAL,
                "associated_data_hash": _HASH_LITERAL,
                "executed_at_ms": execution["executed_at_ms"],
                "expires_at_ms": execution["expires_at_ms"],
            },
            "opening": {
                "payload": {
                    "program_id": {"name": opening["payload"]["program_id"]},
                    "input_ciphertext_hash": _HASH_LITERAL,
                    "output_ciphertext_hash": _HASH_LITERAL,
                    "parameter_digest": _HASH_LITERAL,
                    "evaluation_key_digest": _HASH_LITERAL,
                    "opened_output_hash": _HASH_LITERAL,
                    "opened_at_ms": opening["payload"]["opened_at_ms"],
                    "expires_at_ms": opening["payload"]["expires_at_ms"],
                },
                "signature": opening["signature"],
            },
            "opaque_id": [_HASH_LITERAL],
            "receipt_hash": _HASH_LITERAL,
            "uaid": [_HASH_LITERAL],
            "account_id": receipt["payload"]["account_id"],
        },
        "attestation": {
            "kind": receipt["attestation"]["kind"],
            "value": receipt["attestation"]["signature"],
        },
    }


def test_native_identifier_and_program_policy_instructions_roundtrip() -> None:
    account = _VECTOR["receipt"]["payload"]["account_id"]
    receipt = _native_receipt()
    instructions = [
        Instruction.register_ram_lfe_program_policy(
            json.dumps(
                _program_policy(),
                ensure_ascii=False,
                separators=(",", ":"),
                sort_keys=True,
            )
        ),
        Instruction.activate_ram_lfe_program_policy("identifier_lookup_retail"),
        Instruction.deactivate_ram_lfe_program_policy("identifier_lookup_retail"),
        Instruction.register_identifier_policy(
            "phone#retail",
            account,
            "phone_e164",
            "identifier_lookup_retail",
            note="retail phone aliases",
        ),
        Instruction.activate_identifier_policy("phone#retail"),
        Instruction.claim_identifier(
            account,
            json.dumps(receipt, ensure_ascii=False, separators=(",", ":")),
        ),
        Instruction.revoke_identifier(
            "phone#retail", _VECTOR["receipt"]["payload"]["opaque_id"]
        ),
    ]
    encoded = [instruction.to_json() for instruction in instructions]
    assert [Instruction.from_json(item).to_json() for item in encoded] == encoded
    assert [instruction.wire_id() for instruction in instructions] == [
        "identity::RegisterRamLfeProgramPolicy",
        "identity::ActivateRamLfeProgramPolicy",
        "identity::DeactivateRamLfeProgramPolicy",
        "identity::RegisterIdentifierPolicy",
        "identity::ActivateIdentifierPolicy",
        "identity::ClaimIdentifier",
        "identity::RevokeIdentifier",
    ]
    assert all(
        value
        and value == value.lower()
        and len(value) % 2 == 0
        and set(value) <= set("0123456789abcdef")
        for value in (instruction.encoded_hex() for instruction in instructions)
    )

    draft = TransactionDraft(
        TransactionConfig(
            chain_id="identifier-sdk-test",
            authority=account,
            fee_payment=authority_fee_payment(charge_limits=[]),
        )
    )
    draft.register_ram_lfe_program_policy(_program_policy())
    draft.activate_ram_lfe_program_policy("identifier_lookup_retail")
    draft.register_identifier_policy(
        "phone#retail",
        account,
        "phone_e164",
        "identifier_lookup_retail",
    )
    draft.activate_identifier_policy("phone#retail")
    draft.claim_identifier(account, receipt)
    draft.revoke_identifier(
        "phone#retail", _VECTOR["receipt"]["payload"]["opaque_id"]
    )
    assert len(tuple(draft.instructions)) == 6


def test_native_identifier_helpers_reject_malformed_authority_and_rebinding() -> None:
    account = _VECTOR["receipt"]["payload"]["account_id"]
    receipt = _native_receipt()

    with pytest.raises(ValueError, match="must be inactive"):
        Instruction.register_ram_lfe_program_policy(
            json.dumps(_program_policy(active=True), ensure_ascii=False)
        )
    malformed = dict(_program_policy())
    malformed["unknown"] = True
    with pytest.raises(ValueError, match="unknown"):
        Instruction.register_ram_lfe_program_policy(
            json.dumps(malformed, ensure_ascii=False)
        )
    with pytest.raises(ValueError, match="surrounding whitespace"):
        Instruction.activate_identifier_policy(" phone#retail")
    with pytest.raises(ValueError, match="exactly match"):
        Instruction.claim_identifier(
            account,
            json.dumps(
                {
                    **receipt,
                    "payload": {
                        **receipt["payload"],
                        "account_id": _OTHER_ACCOUNT,
                    },
                },
                ensure_ascii=False,
            ),
        )
    receipt_with_extension = {**receipt, "middleware_authority": account}
    with pytest.raises(ValueError, match="unknown"):
        Instruction.claim_identifier(
            account,
            json.dumps(receipt_with_extension, ensure_ascii=False),
        )
