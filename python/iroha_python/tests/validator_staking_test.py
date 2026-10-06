"""Exact Rust fixture parity and hostile staking inputs; native admission is mandatory."""
from dataclasses import FrozenInstanceError, replace
from pathlib import Path

import pytest
from norito.codec import NoritoDecoder, NoritoEncoder
from norito.header import COMPACT_LEN
from norito.errors import DecodeError

from iroha_python import KotodamaQuantity
from iroha_python.address import (AccountAddress, MultisigMember,
    asset_definition_id_from_bytes, asset_definition_id_to_bytes)
from iroha_python.crypto import NetworkId
from iroha_python.validator_staking import (
    StakingScopeV1, StakingAssetScopeV1, StakingPeerIdV1,
    StakingMonetaryPlanV1, StakingMonetaryRegistrationV1, StakingMonetaryBondV1,
    StakingMonetaryUnbondV1, StakingMonetarySlashV1, StakingRewardClaimPlanV1,
    StakingValidatorGenerationV1, StakingEpochAuthorizationV1,
)


def _rows():
    lines = (Path(__file__).resolve().parents[3] / "fixtures/validator_staking/norito_v1.tsv").read_text().splitlines()
    return {name: bytes.fromhex(encoded) for name, encoded in (line.split("\t") for line in lines if line and not line.startswith("#"))}


_ROWS = _rows()
_TYPES = {
    "validator_generation": StakingValidatorGenerationV1,
    "epoch_authorization": StakingEpochAuthorizationV1,
    "monetary_plan": StakingMonetaryPlanV1,
    "monetary_bond_plan": StakingMonetaryPlanV1,
    "monetary_unbond_plan": StakingMonetaryPlanV1,
    "monetary_slash_plan": StakingMonetaryPlanV1,
    "reward_claim_plan": StakingRewardClaimPlanV1,
    "fee_reward_claim_plan": StakingRewardClaimPlanV1,
}


def _plan(name="monetary_plan"):
    return StakingMonetaryPlanV1.from_norito(_ROWS[name])


def _claim():
    return StakingRewardClaimPlanV1.from_norito(_ROWS["fee_reward_claim_plan"])


def _record(parts):
    out = NoritoEncoder(COMPACT_LEN)
    for part in parts:
        out.write_length(len(part), compact=True); out.write_bytes(part)
    return out.finish()


def _fields(payload):
    decoder = NoritoDecoder(payload, COMPACT_LEN); parts = []
    while decoder.remaining(): parts.append(decoder.read_bytes(decoder.read_length(compact=True)))
    return parts


@pytest.mark.parametrize("name,model", _TYPES.items())
def test_staking_shared_rust_rows_are_exact(name, model):
    value = model.from_norito(_ROWS[name])
    assert value.to_norito() == _ROWS[name]
    with pytest.raises((ValueError, TypeError, DecodeError)): model.from_norito(_ROWS[name][:-1])
    with pytest.raises((ValueError, TypeError, DecodeError)): model.from_norito(_ROWS[name] + b"\0")


def test_staking_preconditions_bind_real_xor_and_exact_tenure():
    types = (StakingMonetaryRegistrationV1, StakingMonetaryBondV1, StakingMonetaryUnbondV1, StakingMonetarySlashV1)
    rows = ("monetary_plan", "monetary_bond_plan", "monetary_unbond_plan", "monetary_slash_plan")
    authority = StakingValidatorGenerationV1.from_norito(_ROWS["validator_generation"])
    for name, expected in zip(rows, types):
        plan = _plan(name)
        assert type(plan.precondition) is expected
        assert plan.precondition.activation_height == 201
        assert plan.source_asset.definition == "6TEAJqbb8oEPmLncoNiMRbLEK6tw"
        assert plan.source_asset.scope.dataspace is None
        assert plan.destination_asset.definition == plan.source_asset.definition
        assert plan.network_scope.network_id.to_bytes() == authority.network_id.to_bytes()
        assert str(plan.amount) == "1000"
    assert _plan("monetary_bond_plan").precondition.peer_id == authority.validators[0]
    assert _plan("monetary_unbond_plan").precondition.request_hash == bytes([0x75]) * 32
    assert _plan("monetary_unbond_plan").source_asset == _plan().destination_asset
    assert str(_plan("monetary_slash_plan").precondition.slashable_exposure) == "1500"
    with pytest.raises(TypeError): replace(_plan(), precondition=b"opaque")
    for bad in (bytes(31), bytes(33), bytes(32)):
        with pytest.raises(ValueError): StakingMonetaryUnbondV1(201, bad)
    with pytest.raises((TypeError, ValueError, DecodeError)): StakingPeerIdV1("00")
    for bad in (-1, 1 << 64, True, 1.0):
        with pytest.raises(ValueError): replace(_plan(), valid_until_height=bad)


def test_staking_fee_claim_is_required_and_has_independent_custody_sequence():
    plan = _claim(); fee = plan.fee_claim
    assert fee.beneficiary_revision == 4 and fee.expected_claim_sequence == 5
    assert str(fee.amount) == "7"
    assert fee.source_asset == _plan().destination_asset and fee.destination_asset == _plan().source_asset
    args = dict(plan.__dict__); args.pop("fee_claim")
    with pytest.raises(TypeError): StakingRewardClaimPlanV1(**args)
    no_fee = StakingRewardClaimPlanV1.from_norito(_ROWS["reward_claim_plan"])
    assert no_fee.fee_claim is None
    assert _ROWS["reward_claim_plan"][-2:] == b"\1\0"
    with pytest.raises((TypeError, ValueError, DecodeError)): StakingRewardClaimPlanV1.from_norito(_ROWS["reward_claim_plan"][:-2])
    for amount in (KotodamaQuantity("0"),):
        with pytest.raises(ValueError): replace(fee, amount=amount)
    with pytest.raises(ValueError): replace(fee, lifecycle_seal=bytes(32))
    with pytest.raises(ValueError): replace(fee, source_asset=replace(fee.source_asset, scope=StakingAssetScopeV1(1)))
    with pytest.raises(ValueError): replace(plan, fee_claim=replace(fee, destination_asset=fee.source_asset))
    with pytest.raises(ValueError): replace(plan.sources[0], expected_accrued=KotodamaQuantity("0"))
    dust = replace(plan, sources=(replace(plan.sources[0], payout=KotodamaQuantity("0")),))
    assert StakingRewardClaimPlanV1.from_norito(dust.to_norito()).sources[0].payout.mantissa == 0


def test_staking_reward_order_bounds_and_immutable_identity_are_preserved():
    plan = _claim(); source = plan.sources[0]
    # Rust's Ed25519 custody key starts 0x5b and recipient key starts 0xe2.
    # This order uses key bytes, not account text or variable frame lengths.
    second = replace(source, source_asset=source.destination_asset, expected_accrued=None)
    ordered = replace(plan, sources=(source, second))
    assert StakingRewardClaimPlanV1.from_norito(ordered.to_norito()).sources == ordered.sources
    with pytest.raises(ValueError): replace(plan, sources=(second, source))
    with pytest.raises(ValueError): replace(plan, sources=(source, source))
    with pytest.raises(ValueError): replace(plan, records=plan.records * 65)
    with pytest.raises(ValueError): replace(plan, sources=plan.sources * 65)
    with pytest.raises(ValueError): replace(plan, records=plan.records * 2)
    body = _fields(plan.to_norito()); body[3] = (65).to_bytes(8, "little")
    with pytest.raises(ValueError, match="bound"): StakingRewardClaimPlanV1.from_norito(_record(body))
    raw = bytearray(plan.fee_claim.lifecycle_seal)
    owned = replace(plan.fee_claim, lifecycle_seal=raw); raw[0] ^= 1
    assert owned.lifecycle_seal == plan.fee_claim.lifecycle_seal
    with pytest.raises(FrozenInstanceError): owned.expected_claim_sequence = 0


def test_staking_generation_epoch_bindings_do_not_couple_signing_lifetime():
    authority = StakingValidatorGenerationV1.from_norito(_ROWS["validator_generation"])
    epoch = StakingEpochAuthorizationV1.from_norito(_ROWS["epoch_authorization"])
    assert authority.generation == epoch.authority_generation == 0
    assert authority.network_id.to_bytes() == epoch.network_id.to_bytes()
    later = replace(epoch, epoch=42, first_height=4201, last_height=4300, decision="retain")
    assert StakingEpochAuthorizationV1.from_norito(later.to_norito()).authority_generation == 0
    for count in (3, 5, 32):
        with pytest.raises(ValueError): replace(authority, validators=(authority.validators * 8)[:count])
    with pytest.raises(TypeError): replace(authority, version=1)
    with pytest.raises(ValueError, match="ordered"):
        replace(authority, validators=tuple(reversed(authority.validators)))
    with pytest.raises(ValueError, match="ordered"):
        replace(authority, validators=(authority.validators[0],) * 4)
    ed_key = AccountAddress.from_i105(_plan().source_asset.account).controller
    from iroha_python.crypto import public_key_multihash
    non_bls = StakingPeerIdV1(public_key_multihash("ed25519", ed_key.public_key))
    with pytest.raises(ValueError, match="BLS-normal"):
        replace(authority, validators=(non_bls,) + authority.validators[1:])
    # The retired generation frame prepended a version and carried paired keys.
    with pytest.raises((ValueError, TypeError, DecodeError)):
        StakingValidatorGenerationV1.from_norito(_record([b"\1\0", *_fields(authority.to_norito())]))
    with pytest.raises(ValueError): replace(epoch, first_height=0)
    with pytest.raises(ValueError): replace(epoch, decision="future")
    with pytest.raises(ValueError): NetworkId.from_bytes(bytes(32))


def test_staking_exact_fields_and_shared_account_owner_reject_alternate_layouts():
    plan = _plan(); bytes_ = plan.to_norito(); assert bytes_[0] == 37
    with pytest.raises(ValueError, match="canonical"): StakingMonetaryPlanV1.from_norito(b"\xa5\0" + bytes_[1:])
    parts = _fields(bytes_); parts[5] = (4).to_bytes(4, "little")
    with pytest.raises(ValueError, match="unknown monetary"): StakingMonetaryPlanV1.from_norito(_record(parts))
    # No defaulting from an omitted asset scope.
    parts = _fields(bytes_); asset = _fields(parts[2]); parts[2] = _record(asset[:-1])
    with pytest.raises((ValueError, DecodeError)): StakingMonetaryPlanV1.from_norito(_record(parts))
    # Source geometry is checked before allocating/iterating a forged public-key count.
    parts = _fields(bytes_); asset = _fields(parts[2]); asset[0] = bytes(4) + _record([(1 << 40).to_bytes(8, "little")]); parts[2] = _record(asset)
    with pytest.raises(ValueError, match="bound|geometry"): StakingMonetaryPlanV1.from_norito(_record(parts))
    key = AccountAddress.from_i105(plan.source_asset.account).controller
    multi = AccountAddress.from_multisig_policy(threshold=1, members=[MultisigMember(key.curve, key.public_key, 1)]).to_i105()
    exact = replace(plan, source_asset=replace(plan.source_asset, account=multi))
    assert StakingMonetaryPlanV1.from_norito(exact.to_norito()).source_asset.account == multi
    with pytest.raises(ValueError): replace(plan.source_asset, account=plan.source_asset.account + "@example")


def test_asset_definition_uuid_adapters_share_existing_canonical_address_validation():
    address = "6TEAJqbb8oEPmLncoNiMRbLEK6tw"
    raw = asset_definition_id_to_bytes(address)
    assert len(raw) == 16 and asset_definition_id_from_bytes(raw) == address
    for bad in (raw[:15], raw + b"\0", bytes(16)):
        with pytest.raises(ValueError): asset_definition_id_from_bytes(bad)
    with pytest.raises(ValueError): asset_definition_id_to_bytes(address + "1")
