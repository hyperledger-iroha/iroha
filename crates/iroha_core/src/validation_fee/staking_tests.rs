// Signed staking principal is charged by the existing policy without becoming a fee payment.
fn staking_fee_instructions(_policy: &ValidationFeePolicyV1) -> Vec<InstructionBox> {
    use iroha_data_model::{isi::staking::*, nexus::*};
    use iroha_model_base::{peer::PeerId, topology::LaneId};
    let lane_id = LaneId::SINGLE;
    let peer_key = KeyPair::try_from_seed(vec![41; 32], Algorithm::BlsNormal).unwrap();
    let peer_id = PeerId::new(peer_key.public_key().clone());
    let plan = |source, destination, precondition| PublicLaneMonetaryPlanV1 {
        network_scope: PublicLaneMonetaryScopeV1::Network(validation_fee_test_network_id()),
        valid_until_height: 10,
        source_asset: AssetId::new(xor_asset(), account(source)),
        destination_asset: AssetId::new(xor_asset(), account(destination)),
        amount: Quantity::one(),
        precondition,
    };
    let registration = RegisterPublicLaneValidator::new(
        lane_id,
        account(1),
        peer_id.clone(),
        account(1),
        Quantity::one(),
        Metadata::default(),
        plan(
            1,
            3,
            PublicLaneMonetaryPreconditionV1::Registration(
                iroha_data_model::nexus::PublicLaneRegistrationPreconditionV1 {
                    activation_height: 10,
                },
            ),
        ),
    );
    let authorization = PublicLaneCandidateAuthorization::new(
        validation_fee_test_network_id(),
        registration.clone(),
        10,
    );
    vec![
        registration.clone().into(),
        RegisterPublicLaneCandidate {
            registration,
            activation_height: 10,
            proof_of_possession: iroha_crypto::bls_normal_pop_prove(peer_key.private_key())
                .unwrap(),
            peer_signature: iroha_crypto::SignatureOf::try_new(
                peer_key.private_key(),
                &authorization,
            )
            .unwrap(),
        }
        .into(),
        BondPublicLaneStake {
            lane_id,
            validator: account(1),
            staker: account(1),
            amount: Quantity::one(),
            metadata: Metadata::default(),
            monetary_plan: plan(
                1,
                3,
                PublicLaneMonetaryPreconditionV1::Bond(
                    iroha_data_model::nexus::PublicLaneBondPreconditionV1 {
                        activation_height: 10,
                        peer_id,
                    },
                ),
            ),
        }
        .into(),
        FinalizePublicLaneUnbond {
            lane_id,
            validator: account(1),
            staker: account(1),
            request_id: Hash::new(b"unbond"),
            monetary_plan: plan(
                3,
                1,
                PublicLaneMonetaryPreconditionV1::Unbond(
                    iroha_data_model::nexus::PublicLaneUnbondPreconditionV1 {
                        activation_height: 10,
                        request_hash: Hash::new(b"exact retained unbond"),
                    },
                ),
            ),
        }
        .into(),
        SlashPublicLaneValidator {
            lane_id,
            validator: account(1),
            offence_height: 10,
            slash_id: Hash::new(b"slash"),
            amount: Quantity::one(),
            reason_code: "double_sign".to_owned(),
            metadata: Metadata::default(),
            monetary_plan: plan(
                3,
                2,
                PublicLaneMonetaryPreconditionV1::Slash(
                    iroha_data_model::nexus::PublicLaneSlashPreconditionV1 {
                        activation_height: 10,
                        slashable_exposure: Quantity::one(),
                    },
                ),
            ),
        }
        .into(),
        ClaimPublicLaneRewards {
            lane_id,
            account: account(1),
            claim_plan: PublicLaneRewardClaimPlanV1 {
                network_scope: PublicLaneMonetaryScopeV1::Network(validation_fee_test_network_id()),
                valid_until_height: 10,
                expected_state: None,
                records: vec![PublicLaneRewardRecordRefV1 {
                    epoch: 0,
                    record_hash: Hash::new(b"record"),
                }],
                sources: vec![PublicLaneRewardClaimSourceV1 {
                    source_asset: AssetId::new(xor_asset(), account(3)),
                    destination_asset: AssetId::new(xor_asset(), account(1)),
                    expected_accrued: None,
                    payout: Quantity::one(),
                }],
            },
        }
        .into(),
    ]
}

#[test]
fn signed_monetary_staking_charges_each_actual_transfer_and_rejects_opaque_effects() {
    let policy = policy(&account(3));
    assert_ne!(xor_asset(), policy.ds_asset_id);
    for instruction in staking_fee_instructions(&policy) {
        assert_eq!(
            native_instruction_ds_effect_disposition(&instruction, &policy.ds_asset_id),
            NativeInstructionDsEffectDisposition::AuthenticatedStakingEffects
        );
        assert!(matches!(
            enforce_policy(
                &tx(1, vec![instruction.clone()], Metadata::default()),
                &policy
            ),
            Err(ValidationFeeAdmissionError::MissingFee { .. })
        ));
        let fee = transfer(
            &account(1),
            &policy.ds_asset_id,
            minor_units(TEST_VALIDATION_FEE_MINOR_UNITS),
            &account(3),
        );
        assert_eq!(
            enforce_policy(
                &tx(
                    1,
                    vec![instruction.clone(), fee.clone()],
                    metadata_for_fee_instruction(&policy, 1)
                ),
                &policy
            ),
            Ok(())
        );
        assert_eq!(
            enforce_deferred_policy(
                &account(1),
                &with_multisig_fee_marker(&policy, vec![instruction.clone(), fee], 1, None),
                &policy
            ),
            Ok(())
        );
        assert!(
            reject_opaque_fee_asset_effects(
                &account(1),
                &[instruction.clone()],
                &policy.ds_asset_id,
                None
            )
            .is_err()
        );
        assert!(
            reject_opaque_fee_asset_effects(
                &account(3),
                &[instruction],
                &policy.ds_asset_id,
                Some(&account(3))
            )
            .is_err(),
            "treasury exemption cannot authorize native staking"
        );
    }
}

#[test]
fn native_staking_principal_cannot_satisfy_explicit_fee_coordinate() {
    let policy = policy(&account(3));
    for instruction in staking_fee_instructions(&policy) {
        assert!(matches!(
            enforce_policy(
                &tx(
                    1,
                    vec![instruction],
                    metadata_for_fee_batch_entry(&policy, 0, 0)
                ),
                &policy
            ),
            Err(ValidationFeeAdmissionError::NativePrincipalCannotPayFee { .. })
        ));
    }
}

#[test]
fn signed_reward_reservations_and_dust_have_no_transfer_fee_but_reject_opaque_execution() {
    use iroha_data_model::{isi::staking::*, nexus::*};
    let policy = policy(&account(3));
    let record: InstructionBox = RecordPublicLaneRewards {
        lane_id: iroha_model_base::topology::LaneId::SINGLE,
        epoch: 0,
        reward_asset: AssetId::new(xor_asset(), account(1)),
        total_reward: Quantity::one(),
        shares: vec![PublicLaneRewardShare {
            account: account(2),
            role: PublicLaneRewardRole::Validator,
            amount: Quantity::one(),
        }],
        metadata: Metadata::default(),
    }
    .into();
    let mut dust = staking_fee_instructions(&policy)
        .pop()
        .unwrap()
        .as_any()
        .downcast_ref::<ClaimPublicLaneRewards>()
        .unwrap()
        .clone();
    dust.claim_plan.sources[0].payout = Quantity::zero();
    for instruction in [record, dust.into()] {
        assert_eq!(
            enforce_policy(
                &tx(1, vec![instruction.clone()], Metadata::default()),
                &policy
            ),
            Ok(())
        );
        assert_eq!(
            enforce_deferred_policy(&account(1), &[instruction.clone()], &policy),
            Ok(())
        );
        assert!(
            reject_opaque_fee_asset_effects(&account(1), &[instruction], &policy.ds_asset_id, None)
                .is_err()
        );
    }
}

#[test]
fn claim_fees_count_positive_sources_and_malformed_plans_fail_closed() {
    use iroha_data_model::isi::staking::{BondPublicLaneStake, ClaimPublicLaneRewards};
    let policy = policy(&account(3));
    let mut instructions = staking_fee_instructions(&policy);
    let mut claim = instructions
        .pop()
        .unwrap()
        .as_any()
        .downcast_ref::<ClaimPublicLaneRewards>()
        .unwrap()
        .clone();
    let mut second = claim.claim_plan.sources[0].clone();
    second.source_asset = AssetId::new(xor_asset(), account(4));
    claim.claim_plan.sources.push(second);
    claim
        .claim_plan
        .sources
        .sort_by(|a, b| a.source_asset.cmp(&b.source_asset));
    let fee = transfer(
        &account(1),
        &policy.ds_asset_id,
        minor_units(2 * TEST_VALIDATION_FEE_MINOR_UNITS),
        &account(3),
    );
    assert_eq!(
        enforce_policy(
            &tx(
                1,
                vec![claim.clone().into(), fee],
                metadata_for_fee_instruction(&policy, 1)
            ),
            &policy
        ),
        Ok(())
    );
    claim.claim_plan.sources.reverse();
    assert!(matches!(
        enforce_policy(&tx(1, vec![claim.into()], Metadata::default()), &policy),
        Err(ValidationFeeAdmissionError::UnsupportedNativeFeeAssetMovement { .. })
    ));
    let mut bond = instructions[2]
        .as_any()
        .downcast_ref::<BondPublicLaneStake>()
        .unwrap()
        .clone();
    bond.monetary_plan.amount = Quantity::from(2_u64);
    assert!(matches!(
        enforce_policy(&tx(1, vec![bond.into()], Metadata::default()), &policy),
        Err(ValidationFeeAdmissionError::UnsupportedNativeFeeAssetMovement { .. })
    ));
}

#[test]
fn signed_staking_plans_remain_bound_in_multisig_and_proved_overlays() {
    let policy = policy(&account(3));
    for instruction in staking_fee_instructions(&policy) {
        let fee = transfer(
            &account(1),
            &policy.ds_asset_id,
            minor_units(TEST_VALIDATION_FEE_MINOR_UNITS),
            &account(3),
        );
        let nested =
            with_multisig_fee_marker(&policy, vec![instruction.clone(), fee.clone()], 1, None);
        let proposal = MultisigPropose::new(account(1), nested.clone(), None);
        assert_eq!(
            enforce_policy_with_credit(
                &tx(2, vec![proposal.into()], metadata_for(&policy)),
                &policy
            ),
            Ok(0)
        );
        assert_eq!(
            enforce_deferred_policy_with_credit(&account(1), &nested, &policy),
            Ok(TEST_VALIDATION_FEE_MINOR_UNITS)
        );
        assert_eq!(
            enforce_policy(
                &ivm_proved_tx(
                    1,
                    vec![instruction, fee],
                    metadata_for_fee_instruction(&policy, 1)
                ),
                &policy
            ),
            Ok(())
        );
    }
}

#[test]
fn opaque_nested_proposals_cannot_reserve_native_staking_rewards() {
    use iroha_data_model::{isi::staking::RecordPublicLaneRewards, nexus::*};
    let policy = policy(&account(3));
    let record = RecordPublicLaneRewards {
        lane_id: iroha_model_base::topology::LaneId::SINGLE,
        epoch: 0,
        reward_asset: AssetId::new(xor_asset(), account(1)),
        total_reward: Quantity::one(),
        shares: vec![PublicLaneRewardShare {
            account: account(2),
            role: PublicLaneRewardRole::Validator,
            amount: Quantity::one(),
        }],
        metadata: Metadata::default(),
    };
    let nested: InstructionBox = MultisigPropose::new(account(1), vec![record.into()], None).into();
    assert!(matches!(
        reject_opaque_fee_asset_effects(
            &account(1),
            &[nested],
            &policy.ds_asset_id,
            Some(&account(1))
        ),
        Err(ValidationFeeAdmissionError::OpaqueDeferredStakingOperation { .. })
    ));
}

#[test]
fn opaque_staking_cannot_move_or_reserve_real_xor_when_the_fee_asset_differs() {
    use iroha_data_model::{isi::staking::*, nexus::*};

    let policy = policy(&account(3));
    let non_fee_asset = xor_asset();
    assert_ne!(non_fee_asset, policy.ds_asset_id);

    let instructions = staking_fee_instructions(&policy);
    let mut bond = instructions[2]
        .as_any()
        .downcast_ref::<BondPublicLaneStake>()
        .unwrap()
        .clone();
    bond.monetary_plan.source_asset = AssetId::new(non_fee_asset.clone(), account(1));
    bond.monetary_plan.destination_asset = AssetId::new(non_fee_asset.clone(), account(3));

    let mut claim = instructions[5]
        .as_any()
        .downcast_ref::<ClaimPublicLaneRewards>()
        .unwrap()
        .clone();
    claim.claim_plan.sources[0].source_asset = AssetId::new(non_fee_asset.clone(), account(3));
    claim.claim_plan.sources[0].destination_asset = AssetId::new(non_fee_asset.clone(), account(1));

    let record = RecordPublicLaneRewards {
        lane_id: iroha_model_base::topology::LaneId::SINGLE,
        epoch: 0,
        reward_asset: AssetId::new(non_fee_asset, account(3)),
        total_reward: Quantity::one(),
        shares: vec![PublicLaneRewardShare {
            account: account(1),
            role: PublicLaneRewardRole::Validator,
            amount: Quantity::one(),
        }],
        metadata: Metadata::default(),
    };

    for instruction in [
        InstructionBox::from(bond),
        InstructionBox::from(claim),
        InstructionBox::from(record),
    ] {
        // A direct signed plan is visible to admission. The same balance or
        // reserve effect generated from opaque code must never gain authority.
        let result = enforce_policy(
            &tx(1, vec![instruction.clone()], Metadata::default()),
            &policy,
        );
        if instruction
            .as_any()
            .downcast_ref::<RecordPublicLaneRewards>()
            .is_some()
        {
            assert_eq!(result, Ok(()), "reward reservation is not a transfer");
        } else {
            assert!(matches!(
                result,
                Err(ValidationFeeAdmissionError::MissingFee { .. })
            ));
        }
        let trigger_id: iroha_data_model::trigger::TriggerId =
            "opaque_staking_non_fee".parse().unwrap();
        let deferred: InstructionBox = RegisterBox::Trigger(Register::trigger(Trigger::new(
            trigger_id.clone(),
            Action::new(
                vec![instruction.clone()],
                Repeats::Indefinitely,
                account(1),
                ExecuteTriggerEventFilter::new().for_trigger(trigger_id),
            )
            .unwrap(),
        )))
        .into();
        for opaque in [
            instruction.clone(),
            MultisigPropose::new(account(1), vec![instruction], None).into(),
            deferred,
        ] {
            assert!(matches!(
                reject_opaque_fee_asset_effects(&account(1), &[opaque], &policy.ds_asset_id, None),
                Err(ValidationFeeAdmissionError::OpaqueDeferredStakingOperation { .. })
            ));
        }
    }
}

fn committee_fee_instruction() -> InstructionBox {
    use iroha_data_model::{isi::kagemusha_v1::*, nexus::*, parameter::Parameter};
    let peer = KeyPair::try_from_seed(vec![42; 32], Algorithm::BlsNormal).unwrap();
    let keys = KagemushaMintFinalityValidatorKeysV1 {
        validator: iroha_model_base::peer::PeerId::new(peer.public_key().clone()),
        eq_proof_public_key: [1; 32],
        ep_proof_public_key: [2; 32],
    };
    // Classification inspects signed instruction structure only. The native
    // command owner independently authenticates the curves, proofs and state.
    let possession = KagemushaMintFinalityPairedPossessionProofV1 {
        eq_proof_signature: KagemushaPastaSchnorrSignatureV1 {
            nonce_commitment: [3; 32],
            response: [4; 32],
        },
        ep_proof_signature: KagemushaPastaSchnorrSignatureV1 {
            nonce_commitment: [5; 32],
            response: [6; 32],
        },
    };
    let consent = ValidatorCandidateKeyAuthorizationV1::new(
        validation_fee_test_network_id(),
        1,
        keys.clone(),
        possession,
    );
    let command = ValidatorCommitteeOperationV1::PublishCandidate(ValidatorCandidateKeysV1 {
        network_id: validation_fee_test_network_id(),
        generation: 1,
        keys,
        possession,
        peer_signature: iroha_crypto::SignatureOf::new(peer.private_key(), &consent),
    });
    let parameter = command.into_custom_parameter();
    assert!(
        ValidatorCommitteeOperationV1::from_custom_parameter(&parameter)
            .unwrap()
            .is_some()
    );
    SetParameter::new(Parameter::Custom(parameter)).into()
}

#[test]
fn signed_committee_preparation_is_balance_neutral_in_native_multisig_and_proved_overlays() {
    let policy = policy(&account(3));
    let instruction = committee_fee_instruction();
    assert_eq!(
        native_instruction_ds_effect_disposition(&instruction, &policy.ds_asset_id),
        NativeInstructionDsEffectDisposition::AuditedNoDsEffect,
    );
    assert_eq!(
        enforce_policy(
            &tx(1, vec![instruction.clone()], Metadata::default()),
            &policy
        ),
        Ok(())
    );
    assert_eq!(
        enforce_policy(
            &ivm_proved_tx(1, vec![instruction.clone()], Metadata::default()),
            &policy
        ),
        Ok(())
    );
    assert_eq!(
        enforce_policy(
            &tx(
                2,
                vec![MultisigPropose::new(account(1), vec![instruction.clone()], None).into()],
                Metadata::default()
            ),
            &policy
        ),
        Ok(())
    );
    assert_eq!(
        enforce_deferred_policy(&account(1), &[instruction], &policy),
        Ok(())
    );
}

#[test]
fn opaque_committee_preparation_rejects_direct_nested_and_resolved_approval_commands() {
    use iroha_data_model::{
        nexus::ValidatorCommitteeOperationV1,
        parameter::{CustomParameter, Parameter},
    };
    use iroha_executor_data_model::isi::multisig::MultisigApprove;
    let instruction = committee_fee_instruction();
    let malformed: InstructionBox = SetParameter::new(Parameter::Custom(CustomParameter::new(
        ValidatorCommitteeOperationV1::parameter_id(),
        iroha_primitives::json::Json::new(0_u64),
    )))
    .into();
    let trigger_id: iroha_data_model::trigger::TriggerId =
        "opaque_committee_command".parse().unwrap();
    let trigger: InstructionBox = RegisterBox::Trigger(Register::trigger(Trigger::new(
        trigger_id.clone(),
        Action::new(
            vec![instruction.clone()],
            Repeats::Indefinitely,
            account(1),
            ExecuteTriggerEventFilter::new().for_trigger(trigger_id),
        )
        .unwrap(),
    )))
    .into();
    for command in [
        instruction.clone(),
        malformed,
        trigger,
        MultisigPropose::new(account(1), vec![instruction.clone()], None).into(),
    ] {
        assert!(matches!(
            committee_effects::reject_opaque_committee_operations_with(
                &[command],
                &mut Default::default(),
                0,
                &mut |_| None,
            ),
            Err(ValidationFeeAdmissionError::OpaqueDeferredCommitteeOperation { .. })
        ));
    }
    let nested = vec![instruction];
    let approval = MultisigApprove::new(account(1), iroha_crypto::HashOf::new(&nested));
    let command: InstructionBox = approval.clone().into();
    assert!(matches!(
        committee_effects::reject_opaque_committee_operations_with(
            &[command.clone()],
            &mut Default::default(),
            0,
            &mut |candidate| (candidate == &approval).then(|| (account(1), nested.clone())),
        ),
        Err(ValidationFeeAdmissionError::OpaqueDeferredCommitteeOperation { .. })
    ));
    assert!(matches!(
        committee_effects::reject_opaque_committee_operations_with(
            &[command],
            &mut Default::default(),
            0,
            &mut |_| None,
        ),
        Err(ValidationFeeAdmissionError::UnresolvedOpaqueDeferredMultisigApproval { .. })
    ));
}

#[test]
fn opaque_committee_preparation_rejects_before_no_fee_policy_return() {
    let state = crate::state::State::new_with_chain_and_network_id_for_testing(
        crate::state::World::new(),
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
        "generic-testnet".parse().unwrap(),
        validation_fee_test_network_id(),
    );
    let mut block = state.block(BlockHeader::new(
        std::num::NonZeroU64::new(1).unwrap(),
        None,
        None,
        0,
        0,
    ));
    let mut transaction = block.transaction();
    let command = committee_fee_instruction();
    let groups = std::collections::BTreeMap::from([(account(1), vec![command.clone()])]);
    let error = enforce_opaque_deferred_instruction_groups(
        &groups,
        &[(account(1), command)],
        &mut transaction,
        None,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        TransactionRejectionReason::Validation(ValidationFail::NotPermitted(reason))
            if reason.contains("complete preparation command must be a signed instruction")
    ));
    assert!(
        transaction
            .world
            .validator_candidate_keys()
            .iter()
            .next()
            .is_none()
    );
}

#[test]
fn opaque_real_xor_staking_rejects_before_no_fee_policy_return() {
    use iroha_data_model::{isi::staking::*, nexus::*};

    let state = crate::state::State::new_with_chain_and_network_id_for_testing(
        crate::state::World::new(),
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
        "generic-testnet".parse().unwrap(),
        validation_fee_test_network_id(),
    );
    let mut block = state.block(BlockHeader::new(
        std::num::NonZeroU64::new(1).unwrap(),
        None,
        None,
        0,
        0,
    ));
    let mut transaction = block.transaction();
    let bond = staking_fee_instructions(&policy(&account(3)))[2].clone();
    let record: InstructionBox = RecordPublicLaneRewards {
        lane_id: iroha_model_base::topology::LaneId::SINGLE,
        epoch: 0,
        reward_asset: AssetId::new(xor_asset(), account(3)),
        total_reward: Quantity::one(),
        shares: vec![PublicLaneRewardShare {
            account: account(1),
            role: PublicLaneRewardRole::Validator,
            amount: Quantity::one(),
        }],
        metadata: Metadata::default(),
    }
    .into();
    for instruction in [bond, record] {
        let trigger_id: iroha_data_model::trigger::TriggerId =
            "opaque_staking_without_policy".parse().unwrap();
        let trigger: InstructionBox = RegisterBox::Trigger(Register::trigger(Trigger::new(
            trigger_id.clone(),
            Action::new(
                vec![instruction.clone()],
                Repeats::Indefinitely,
                account(1),
                ExecuteTriggerEventFilter::new().for_trigger(trigger_id),
            )
            .unwrap(),
        )))
        .into();
        for opaque in [
            instruction.clone(),
            MultisigPropose::new(account(1), vec![instruction], None).into(),
            trigger,
        ] {
            let groups = std::collections::BTreeMap::from([(account(1), vec![opaque.clone()])]);
            let error = enforce_opaque_deferred_instruction_groups(
                &groups,
                &[(account(1), opaque)],
                &mut transaction,
                None,
            )
            .unwrap_err();
            assert!(matches!(
                error,
                TransactionRejectionReason::Validation(ValidationFail::NotPermitted(reason))
                    if reason.contains("staking")
            ));
        }
    }
}
