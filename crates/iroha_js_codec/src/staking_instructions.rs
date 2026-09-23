//! Strict native JSON projections for public-lane staking and treasury rewards.
//!
//! The model owns quantities, hashes, signatures, and canonical account strings.
//! Core separately verifies consent, custody, election readiness, and authority.

use iroha_data_model::isi::{InstructionBox, staking::*};
use norito::json::{self, Value};

use crate::{CodecError, CodecErrorKind, CodecResult};

fn safe_json_numbers(value: &Value) -> CodecResult<()> {
    match value {
        Value::Number(number) => {
            if number
                .as_u64()
                .is_some_and(|value| value > crate::json_u64::MAX_SAFE_INTEGER)
                || number
                    .as_i64()
                    .is_some_and(|value| value < -(crate::json_u64::MAX_SAFE_INTEGER as i64))
            {
                return Err(CodecError::new(
                    CodecErrorKind::InvalidArgument,
                    "staking JSON integer exceeds the JavaScript safe-integer range",
                ));
            }
        }
        Value::Array(values) => {
            for value in values {
                safe_json_numbers(value)?;
            }
        }
        Value::Object(fields) => {
            for value in fields.values() {
                safe_json_numbers(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

macro_rules! staking_codec {
    ($($ty:ident),+ $(,)?) => {
        pub(super) fn from_json(value: &Value) -> Option<CodecResult<InstructionBox>> {
            let Value::Object(fields) = value else { return None; };
            $(if let Some(payload) = fields.get(stringify!($ty)) {
                return Some((|| {
                    crate::exact_json_object_fields(value, &[stringify!($ty)], "staking instruction envelope")?;
                    safe_json_numbers(payload)?;
                    let instruction: $ty = crate::strict_typed_instruction(payload, stringify!($ty))?;
                    Ok(instruction.into())
                })());
            })+
            None
        }
        pub(super) fn to_json(instruction: &InstructionBox) -> Option<CodecResult<Value>> {
            $(if let Some(typed) = instruction.as_any().downcast_ref::<$ty>() {
                return Some((|| {
                    let payload = json::to_value(typed).map_err(crate::codec_error)?;
                    safe_json_numbers(&payload)?;
                    let decoded: $ty = crate::strict_typed_instruction(&payload, stringify!($ty))?;
                    if norito::encode_canonical(typed).map_err(crate::codec_error)?
                        != norito::encode_canonical(&decoded).map_err(crate::codec_error)? {
                        return Err(CodecError::failure("staking JSON changes canonical Norito bytes"));
                    }
                    Ok(crate::instruction_envelope(stringify!($ty), payload))
                })());
            })+
            None
        }
        pub(super) fn is_staking_instruction(instruction: &InstructionBox) -> bool {
            false $(|| instruction.as_any().is::<$ty>())+
        }
    };
}
staking_codec!(
    RegisterPublicLaneCandidate,
    RegisterPublicLaneValidator,
    RebindPublicLaneValidatorPeer,
    ActivatePublicLaneValidator,
    ExitPublicLaneValidator,
    BondPublicLaneStake,
    SchedulePublicLaneUnbond,
    FinalizePublicLaneUnbond,
    SlashPublicLaneValidator,
    ClaimPublicLaneRewards,
    RecordPublicLaneRewards,
);

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, SignatureOf};
    use iroha_data_model::{
        NetworkId, account::AccountId, parameter::system::SumeragiNposParameters,
    };
    use iroha_model_base::{metadata::Metadata, peer::PeerId, topology::LaneId};
    use iroha_primitives::numeric::Quantity;

    fn candidate() -> RegisterPublicLaneCandidate {
        let account_key = KeyPair::try_from_seed(vec![0x71; 32], Algorithm::Ed25519).unwrap();
        let peer_key = KeyPair::try_from_seed(vec![0x72; 32], Algorithm::BlsNormal).unwrap();
        let owner = AccountId::new(account_key.public_key().clone());
        let xor = SumeragiNposParameters::default().xor_asset_definition_id;
        let registration = RegisterPublicLaneValidator::new(
            LaneId::new(42),
            owner.clone(),
            PeerId::new(peer_key.public_key().clone()),
            owner.clone(),
            Quantity::from(1000_u64),
            Metadata::default(),
            iroha_data_model::nexus::PublicLaneMonetaryPlanV1 {
                network_scope: iroha_data_model::nexus::PublicLaneMonetaryScopeV1::Network(
                    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
                        b"codec-candidate-network",
                    ))),
                ),
                valid_until_height: 101,
                source_asset: iroha_data_model::asset::AssetId::new(xor.clone(), owner.clone()),
                destination_asset: iroha_data_model::asset::AssetId::new(xor, owner),
                amount: 1000_u64.into(),
                precondition:
                    iroha_data_model::nexus::PublicLaneMonetaryPreconditionV1::Registration(
                        iroha_data_model::nexus::PublicLaneRegistrationPreconditionV1 {
                            activation_height: 101,
                        },
                    ),
            },
        );
        let payload = PublicLaneCandidateAuthorization::new(
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
                b"codec-candidate-network",
            ))),
            registration.clone(),
            101,
        );
        RegisterPublicLaneCandidate {
            registration,
            activation_height: 101,
            proof_of_possession: iroha_crypto::bls_normal_pop_prove(peer_key.private_key())
                .unwrap(),
            peer_signature: SignatureOf::try_new(peer_key.private_key(), &payload).unwrap(),
        }
    }

    #[test]
    fn staking_candidate_json_and_frame_preserve_exact_native_consent() {
        let original: InstructionBox = candidate().into();
        let json = crate::instruction_to_json_value(&original).unwrap();
        let restored = crate::value_to_instruction(json.clone()).unwrap();
        assert_eq!(
            norito::encode_canonical(&original).unwrap(),
            norito::encode_canonical(&restored).unwrap()
        );
        assert!(is_staking_instruction(&restored));
        let text = norito::json::to_json(&json).unwrap();
        let frame = crate::encode_instruction_frame(&text, 753).unwrap();
        let decoded = crate::decode_instruction_frame(&frame, 753).unwrap();
        assert_eq!(norito::json::from_str::<Value>(&decoded).unwrap(), json);
    }

    #[test]
    fn staking_json_rejects_unknown_fields_and_unsafe_integer_values() {
        let original: InstructionBox = candidate().into();
        let mut value = crate::instruction_to_json_value(&original).unwrap();
        let Value::Object(fields) = &mut value else {
            unreachable!()
        };
        let Value::Object(payload) = fields.get_mut("RegisterPublicLaneCandidate").unwrap() else {
            unreachable!()
        };
        payload.insert("unexpected".into(), Value::Bool(true));
        assert!(crate::value_to_instruction(value).is_err());
        assert!(safe_json_numbers(&Value::from(crate::json_u64::MAX_SAFE_INTEGER + 1)).is_err());
        assert!(
            safe_json_numbers(&Value::from(
                -((crate::json_u64::MAX_SAFE_INTEGER + 1) as i64)
            ))
            .is_err()
        );
    }
    #[test]
    fn mandatory_staking_plans_roundtrip_and_reject_retired_json_shapes() {
        use iroha_data_model::nexus::*;
        let candidate = candidate();
        let registration = candidate.registration;
        let owner = registration.validator.clone();
        let mut plan = registration.monetary_plan.clone();
        plan.precondition = PublicLaneMonetaryPreconditionV1::Bond(
            iroha_data_model::nexus::PublicLaneBondPreconditionV1 {
                activation_height: 101,
                peer_id: registration.peer_id.clone(),
            },
        );
        let bond: InstructionBox = BondPublicLaneStake {
            lane_id: registration.lane_id,
            validator: owner.clone(),
            staker: owner.clone(),
            amount: plan.amount.clone(),
            metadata: Metadata::default(),
            monetary_plan: plan.clone(),
        }
        .into();
        let mut unbond_plan = plan.clone();
        unbond_plan.precondition = PublicLaneMonetaryPreconditionV1::Unbond(
            iroha_data_model::nexus::PublicLaneUnbondPreconditionV1 {
                activation_height: 101,
                request_hash: Hash::new(b"retained unbond"),
            },
        );
        let unbond: InstructionBox = FinalizePublicLaneUnbond {
            lane_id: registration.lane_id,
            validator: owner.clone(),
            staker: owner.clone(),
            request_id: Hash::new(b"unbond"),
            monetary_plan: unbond_plan,
        }
        .into();
        let mut slash_plan = plan.clone();
        slash_plan.precondition = PublicLaneMonetaryPreconditionV1::Slash(
            iroha_data_model::nexus::PublicLaneSlashPreconditionV1 {
                activation_height: 101,
                slashable_exposure: plan.amount.clone(),
            },
        );
        let slash: InstructionBox = SlashPublicLaneValidator {
            lane_id: registration.lane_id,
            validator: owner.clone(),
            offence_height: 101,
            slash_id: Hash::new(b"slash"),
            amount: plan.amount.clone(),
            reason_code: "double_sign".to_owned(),
            metadata: Metadata::default(),
            monetary_plan: slash_plan,
        }
        .into();
        let claim: InstructionBox = ClaimPublicLaneRewards {
            lane_id: registration.lane_id,
            account: owner,
            claim_plan: PublicLaneRewardClaimPlanV1 {
                network_scope: plan.network_scope.clone(),
                valid_until_height: 101,
                expected_state: Some(PublicLaneRewardClaimStateV1 {
                    through_epoch: Some(0),
                }),
                records: vec![],
                sources: vec![PublicLaneRewardClaimSourceV1 {
                    source_asset: plan.source_asset.clone(),
                    destination_asset: plan.destination_asset.clone(),
                    expected_accrued: Some(plan.amount.clone()),
                    payout: plan.amount.clone(),
                }],
            },
        }
        .into();
        for original in [registration.into(), bond, unbond, slash, claim] {
            let value = crate::instruction_to_json_value(&original).unwrap();
            let restored = crate::value_to_instruction(value.clone()).unwrap();
            assert_eq!(
                norito::encode_canonical(&original).unwrap(),
                norito::encode_canonical(&restored).unwrap()
            );
            let mut retired = value;
            let Value::Object(envelope) = &mut retired else {
                unreachable!()
            };
            let payload = envelope.values_mut().next().unwrap();
            let Value::Object(fields) = payload else {
                unreachable!()
            };
            if fields.remove("monetary_plan").is_none() {
                assert!(fields.remove("claim_plan").is_some());
                fields.insert("upto_epoch".to_owned(), Value::Null);
            }
            assert!(
                crate::value_to_instruction(retired).is_err(),
                "retired unbound monetary payload must fail"
            );
        }
    }
}
