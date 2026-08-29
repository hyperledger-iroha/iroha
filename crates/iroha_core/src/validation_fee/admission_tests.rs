#[test]
fn newly_dispatchable_native_instruction_fails_until_explicitly_classified() {
    let treasury = account(3);
    let policy = policy(&treasury);
    let instruction: InstructionBox = iroha_data_model::isi::InvalidInstruction::new(
        "future.native.instruction",
        [0xAB; 32],
        "classification coverage sentinel",
    )
    .into();
    let type_name = core::any::type_name::<iroha_data_model::isi::InvalidInstruction>();
    assert_eq!(
        crate::smartcontracts::isi::registered_native_instruction_type_name(&instruction),
        Some(type_name),
        "coverage sentinel must be on the real native dispatch surface"
    );
    assert_eq!(
        native_instruction_ds_effect_disposition(&instruction, &policy_fee_asset(&policy)),
        NativeInstructionDsEffectDisposition::UnclassifiedDispatchable(type_name)
    );
    assert_eq!(
        enforce_policy(&tx(1, vec![instruction], Metadata::default()), &policy),
        Err(ValidationFeeAdmissionError::UnclassifiedNativeInstruction {
            context_index: 0,
            instruction_index: 0,
            registered_type_name: Some(type_name),
        })
    );
}
#[test]
fn moderation_challenge_custody_paths_are_classified_as_state_derived_ds_effects() {
    use iroha_data_model::{
        isi::sorafs::{
            ExpireSorafsModerationChallenge, FinalizeSorafsModerationCase,
            RaiseSorafsModerationChallenge, ResolveSorafsModerationChallenge,
        },
        sorafs::moderation_ledger::{ModerationChallengeDecisionV1, ModerationChallengeKindV1},
    };

    let policy = policy(&account(3));
    let instructions: Vec<InstructionBox> = vec![
        RaiseSorafsModerationChallenge::new(
            "case-1".to_owned(),
            "round-1".to_owned(),
            "challenge-1".to_owned(),
            ModerationChallengeKindV1::Other,
            None,
            [0x41; 32],
            "evidence".to_owned(),
        )
        .into(),
        ResolveSorafsModerationChallenge::new(
            "case-1".to_owned(),
            "round-1".to_owned(),
            "challenge-1".to_owned(),
            ModerationChallengeDecisionV1::Rejected,
        )
        .into(),
        ExpireSorafsModerationChallenge::new(
            "case-1".to_owned(),
            "round-1".to_owned(),
            "challenge-1".to_owned(),
        )
        .into(),
        FinalizeSorafsModerationCase::new("case-1".to_owned(), "round-1".to_owned()).into(),
    ];
    for instruction in instructions {
        assert!(matches!(
            native_instruction_ds_effect_disposition(&instruction, &policy_fee_asset(&policy),),
            NativeInstructionDsEffectDisposition::RejectKnownDsCapable(_)
        ));
    }
}
#[test]
fn threshold_key_lifecycle_certificate_is_balance_neutral() {
    use iroha_data_model::isi::consensus_keys::{
        ApplyThresholdKeyLifecycleCertificateV1, ThresholdKeyLifecycleActionV1,
        ThresholdKeyLifecycleCertificateV1,
    };

    let treasury = account(3);
    let policy = policy(&treasury);
    let instruction: InstructionBox = ApplyThresholdKeyLifecycleCertificateV1 {
        certificate: ThresholdKeyLifecycleCertificateV1 {
            version: crate::state::THRESHOLD_KEY_LIFECYCLE_CERTIFICATE_VERSION_V1,
            action: ThresholdKeyLifecycleActionV1::RetireParliamentTleKey,
            expected_active_session_id: Some([0x71; 32]),
            effective_height: TEST_POLICY_EFFECTIVE_HEIGHT,
            network_id: validation_fee_test_network_id(),
            roster_hash: [0x72; 32],
            committee_size: 4,
            quorum: 3,
            session_id: [0x71; 32],
            transcript_hash: [0x73; 32],
            public_state: Vec::new(),
            signatures: Vec::new(),
        },
    }
    .into();

    assert_eq!(
        native_instruction_ds_effect_disposition(&instruction, &policy_fee_asset(&policy)),
        NativeInstructionDsEffectDisposition::AuditedNoDsEffect,
    );
    assert_eq!(
        enforce_policy(&tx(1, vec![instruction], Metadata::default()), &policy),
        Ok(()),
    );
}
#[test]
fn parliament_attempt_and_transition_instructions_are_balance_neutral() {
    use iroha_data_model::{
        governance::types::{ProposalKind, ValidationFeePolicyProposal},
        isi::governance::{
            CreateParliamentGovernanceAttemptV1, ParliamentLifecycleTransitionV1,
            SubmitParliamentLifecycleTransitionV1,
        },
    };

    let treasury = account(3);
    let policy = policy(&treasury);
    let create: InstructionBox = CreateParliamentGovernanceAttemptV1 {
        proposal: ProposalKind::ValidationFeePolicy(ValidationFeePolicyProposal {
            proposal_operator: account(1),
            policy: policy.clone(),
            payout_lifecycle_proposal_id: None,
        }),
        attempt_sequence: 0,
    }
    .into();
    let transition: InstructionBox = SubmitParliamentLifecycleTransitionV1 {
        governance_attempt_id: GovernanceAttemptId::new([0x42; 32]),
        transition: ParliamentLifecycleTransitionV1::CompleteQualification,
    }
    .into();
    let instructions = vec![create, transition];

    for instruction in &instructions {
        assert_eq!(
            native_instruction_ds_effect_disposition(instruction, &policy_fee_asset(&policy)),
            NativeInstructionDsEffectDisposition::AuditedNoDsEffect,
        );
    }
    assert_eq!(
        enforce_policy(&tx(1, instructions, Metadata::default()), &policy),
        Ok(()),
    );
}
#[test]
fn active_policy_exempts_only_private_parliament_control_transactions() {
    let deployer_key = key_pair(55);
    let deployer = AccountId::new(deployer_key.public_key().clone());
    let state = crate::state::State::new_with_chain_and_network_id_for_testing(
        validation_fee_payout_world(&deployer),
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
        "generic-testnet".parse().expect("chain id"),
        validation_fee_test_network_id(),
    );
    let header = BlockHeader::new(
        std::num::NonZeroU64::new(TEST_POLICY_EFFECTIVE_HEIGHT)
            .expect("test policy effective height is non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut state_tx = block.transaction();
    let policy =
        install_active_bound_validation_fee_policy(&mut state_tx, &deployer, &deployer_key);
    assert!(
        active_policy(&state_tx)
            .expect("active policy lookup succeeds")
            .is_some(),
        "fixture must exercise active-policy admission"
    );

    let private_control: InstructionBox = CreateParliamentGovernanceAttemptV1 {
        proposal: iroha_data_model::governance::types::ProposalKind::ValidationFeePolicy(
            iroha_data_model::governance::types::ValidationFeePolicyProposal {
                proposal_operator: deployer.clone(),
                policy: policy.clone(),
                payout_lifecycle_proposal_id: None,
            },
        ),
        attempt_sequence: 0,
    }
    .into();
    let control_only = tx(55, vec![private_control], Metadata::default());
    assert!(is_validation_fee_control_plane_transaction(&control_only));
    assert_eq!(
        enforce_validation_fee_admission(&control_only, &state_tx)
            .expect("private Parliament control transactions remain live"),
        None
    );

    let plaintext_ballot: InstructionBox = iroha_data_model::isi::governance::CastPlainBallot {
        referendum_id: "successor-validation-fee-policy".to_owned(),
        owner: deployer,
        amount: 150_u64.into(),
        duration_blocks: 3_600,
        direction: 0,
    }
    .into();
    let ballot_only = tx(55, vec![plaintext_ballot], Metadata::default());
    assert!(
        !is_validation_fee_control_plane_transaction(&ballot_only),
        "plaintext ballots are ordinary fee-subject transactions"
    );
    enforce_validation_fee_admission(&ballot_only, &state_tx)
        .expect_err("plaintext ballots must not bypass active-policy admission");
}
#[test]
fn parliament_authorization_certificate_hash_ignores_ambient_norito_layout() {
    let policy = policy_with_treasury_payout_lifecycle(treasury_payout_binding(
        test_contract_address(),
        b"canonical-roster-hash",
    ));
    let registry = policy_registry(std::slice::from_ref(&policy));
    let state = crate::state::State::new_with_chain_and_network_id_for_testing(
        crate::state::World::default(),
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
        "generic-testnet".parse().expect("chain id"),
        validation_fee_test_network_id(),
    );
    let header = BlockHeader::new(
        std::num::NonZeroU64::new(1).expect("non-zero test height"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut state_tx = block.transaction();
    install_policy_registry_fixture(&registry, &mut state_tx);

    let authorization = &registry.registered_policies[0].parliament_authorization;
    let canonical = norito::encode_canonical(&authorization.governance_certificate)
        .expect("encode canonical Parliament certificate fixture");
    let alternate_flags =
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
    assert_ne!(
        norito::to_bytes(&authorization.governance_certificate)
            .expect("encode alternate-layout Parliament certificate fixture"),
        canonical,
        "fixture must exercise a distinct ambient Norito layout"
    );
    assert_eq!(
        GovernanceCertificateId::derive_v1(&authorization.governance_certificate),
        authorization.governance_certificate_id,
        "certificate identity must use canonical Norito independently of ambient flags"
    );
    validate_registry_entry_governance(&registry.registered_policies[0], &state_tx.world)
        .expect("retained Parliament authorization must use the canonical certificate hash");
}
#[test]
fn restored_effective_payout_policy_requires_its_exact_runtime_binding() {
    let policy = policy_with_treasury_payout_lifecycle(treasury_payout_binding(
        test_contract_address(),
        b"missing-restored-payout-runtime",
    ));
    let registry = policy_registry(std::slice::from_ref(&policy));
    let state = crate::state::State::new_with_chain_and_network_id_for_testing(
        crate::state::World::default(),
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
        "generic-testnet".parse().expect("chain id"),
        validation_fee_test_network_id(),
    );
    let header = BlockHeader::new(
        std::num::NonZeroU64::new(1).expect("non-zero test height"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut state_tx = block.transaction();
    install_policy_registry_fixture(&registry, &mut state_tx);

    let error =
        validate_persisted_policy_registry_runtime_v1(&state_tx, policy.effective_from_height)
            .expect_err("an effective payout policy cannot restore without its contract runtime");
    assert!(
        error.contains("requires treasury")
            && error.contains("active immutable non-signable contract subject"),
        "restore rejection identifies the missing payout runtime: {error}"
    );
}
#[test]
fn restored_future_payout_policy_does_not_require_runtime_before_effective_height() {
    let policy = policy_with_treasury_payout_lifecycle(treasury_payout_binding(
        test_contract_address(),
        b"future-restored-payout-runtime",
    ));
    let restored_height = policy
        .effective_from_height
        .checked_sub(1)
        .expect("fee policy fixture is not effective at genesis");
    let registry = policy_registry(std::slice::from_ref(&policy));
    let state = crate::state::State::new_with_chain_and_network_id_for_testing(
        crate::state::World::default(),
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
        "generic-testnet".parse().expect("chain id"),
        validation_fee_test_network_id(),
    );
    let header = BlockHeader::new(
        std::num::NonZeroU64::new(1).expect("non-zero test height"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut state_tx = block.transaction();
    install_policy_registry_fixture(&registry, &mut state_tx);

    validate_persisted_policy_registry_runtime_v1(&state_tx, restored_height)
        .expect("a future payout policy must not require its runtime before its effective height");
}
#[test]
fn custom_instruction_without_effect_disposition_fails_closed() {
    let treasury = account(3);
    let policy = policy(&treasury);
    let instruction: InstructionBox =
        CustomInstruction::new(Json::new("unclassified custom effect")).into();
    assert_eq!(
        native_instruction_ds_effect_disposition(&instruction, &policy_fee_asset(&policy)),
        NativeInstructionDsEffectDisposition::Unknown
    );
    assert_eq!(
        enforce_policy(&tx(1, vec![instruction], Metadata::default()), &policy),
        Err(ValidationFeeAdmissionError::UnclassifiedNativeInstruction {
            context_index: 0,
            instruction_index: 0,
            registered_type_name: None,
        })
    );
}
#[test]
fn active_policy_allows_balance_neutral_permissionless_contract_deployment_steps() {
    use iroha_data_model::{
        isi::smart_contract_code::{
            ActivateContractInstance, CancelSmartContractCodeUpload, CommitContractDeployment,
            FinalizeSmartContractCodeUpload, RegisterSmartContractBytes, RegisterSmartContractCode,
            UploadSmartContractCodeChunk,
        },
        smart_contract::manifest::ContractManifest,
    };
    let treasury = account(3);
    let policy = policy(&treasury);
    let code_hash = Hash::new(b"permissionless-contract-artifact");
    let contract_address: iroha_data_model::smart_contract::ContractAddress =
        "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh"
            .parse()
            .expect("contract address");
    let instructions: Vec<InstructionBox> = vec![
        RegisterSmartContractBytes {
            code_hash,
            code: Vec::new(),
        }
        .into(),
        UploadSmartContractCodeChunk {
            code_hash,
            total_size: 1,
            chunk_index: 0,
            chunk_count: 1,
            chunk: vec![0],
        }
        .into(),
        FinalizeSmartContractCodeUpload {
            code_hash,
            total_size: 1,
            chunk_count: 1,
        }
        .into(),
        CancelSmartContractCodeUpload { code_hash }.into(),
        RegisterSmartContractCode {
            manifest: ContractManifest {
                seiyaku_name: None,
                code_hash: Some(code_hash),
                abi_hash: None,
                compiler_fingerprint: None,
                features_bitmap: None,
                access_set_hints: None,
                entrypoints: None,
                states: None,
                kotoba: None,
                error_codes: None,
                provenance: None,
            },
        }
        .into(),
        ActivateContractInstance {
            contract_address: contract_address.clone(),
            expected_revision: 1,
            code_hash,
        }
        .into(),
        CommitContractDeployment {
            expected_deploy_nonce: 0,
            contract_address,
            code_hash,
            contract_alias: "payments::universal".parse().expect("contract alias"),
            lease_expiry_ms: None,
            expected_previous_contract_address: None,
        }
        .into(),
    ];
    for instruction in &instructions {
        assert_eq!(
            native_instruction_ds_effect_disposition(instruction, &policy_fee_asset(&policy),),
            NativeInstructionDsEffectDisposition::AuditedNoDsEffect,
        );
    }
    assert_eq!(
        enforce_policy(&tx(1, instructions, Metadata::default()), &policy),
        Ok(()),
    );
}
#[test]
fn active_policy_rejects_contract_rebinding_and_artifact_removal_steps() {
    use iroha_data_model::isi::smart_contract_code::{
        CommitContractDeployment, DeactivateContractInstance, RemoveSmartContractBytes,
    };
    let treasury = account(3);
    let policy = policy(&treasury);
    let code_hash = Hash::new(b"immutable-contract-artifact");
    let contract_address: iroha_data_model::smart_contract::ContractAddress =
        "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh"
            .parse()
            .expect("contract address");
    let instructions: Vec<InstructionBox> = vec![
        DeactivateContractInstance {
            contract_address: contract_address.clone(),
            expected_revision: 1,
            reason: Some("attempted policy-era rebind".to_owned()),
        }
        .into(),
        RemoveSmartContractBytes {
            code_hash,
            reason: Some("attempted policy-era removal".to_owned()),
        }
        .into(),
        CommitContractDeployment {
            expected_deploy_nonce: 1,
            contract_address: contract_address.clone(),
            code_hash,
            contract_alias: "payments::universal".parse().expect("contract alias"),
            lease_expiry_ms: None,
            expected_previous_contract_address: Some(contract_address),
        }
        .into(),
    ];
    for (index, instruction) in instructions.into_iter().enumerate() {
        let instruction_wire_id = match index {
            0 => core::any::type_name::<DeactivateContractInstance>(),
            1 => core::any::type_name::<RemoveSmartContractBytes>(),
            _ => core::any::type_name::<CommitContractDeployment>(),
        };
        assert_eq!(
            enforce_policy(&tx(1, vec![instruction], Metadata::default()), &policy,),
            Err(
                ValidationFeeAdmissionError::UnsupportedNativeFeeAssetMovement {
                    context_index: 0,
                    instruction_index: 0,
                    instruction_wire_id,
                },
            ),
        );
    }
}
#[test]
fn numeric_supply_changes_are_disabled_while_policy_is_active() {
    let user = account(1);
    let treasury = account(3);
    let policy = policy(&treasury);
    let mint: InstructionBox =
        Mint::asset_quantity(1_u64, AssetId::new(policy_fee_asset(&policy), user)).into();
    let instruction_wire_id = core::any::type_name::<MintBox>();
    assert_eq!(
        enforce_policy(&tx(1, vec![mint], Metadata::default()), &policy),
        Err(
            ValidationFeeAdmissionError::UnsupportedNativeFeeAssetMovement {
                context_index: 0,
                instruction_index: 0,
                instruction_wire_id,
            }
        )
    );
}
#[test]
fn policy_ds_transfer_controls_cannot_encumber_balances() {
    let treasury = account(3);
    let policy = policy(&treasury);
    let availability: InstructionBox = iroha_data_model::isi::SetAssetTransferAvailability::new(
        treasury,
        policy_fee_asset(&policy),
        0,
        iroha_data_model::asset::AssetTransferAvailability::Disabled,
        iroha_data_model::asset::AssetTransferAvailability::Disabled,
        Some("encumber policy DS".to_owned()),
    )
    .into();
    let instruction_wire_id =
        core::any::type_name::<iroha_data_model::isi::SetAssetTransferAvailability>();
    assert_eq!(
        enforce_policy(&tx(1, vec![availability], Metadata::default()), &policy),
        Err(
            ValidationFeeAdmissionError::UnsupportedNativeFeeAssetMovement {
                context_index: 0,
                instruction_index: 0,
                instruction_wire_id,
            }
        )
    );
}
#[test]
fn audited_no_ds_effect_instruction_remains_available() {
    let treasury = account(3);
    let policy = policy(&treasury);
    let log: InstructionBox = Log::new(Level::INFO, "audit-only".to_owned()).into();
    enforce_policy(&tx(1, vec![log], Metadata::default()), &policy)
        .expect("audited no-DS-effect instruction should remain available");
}
#[test]
fn kaigi_instruction_surface_is_audited_as_no_ds_effect() {
    use iroha_data_model::{
        isi::kaigi::{
            CreateKaigi, EndKaigi, JoinKaigi, LeaveKaigi, RecordKaigiUsage, RegisterKaigiRelay,
            ReportKaigiRelayHealth, SetKaigiRelayManifest, UnregisterKaigiRelay,
        },
        kaigi::{KaigiId, KaigiRelayHealthStatus, KaigiRelayRegistration, NewKaigi},
    };

    let treasury = account(3);
    let policy = policy(&treasury);
    let host = account(4);
    let participant = account(5);
    let relay = account(6);
    let call_id = KaigiId::new(
        DomainId::try_new("kaigi", "universal").expect("domain id"),
        Name::from_str("validation-fee").expect("call name"),
    );
    let instructions: Vec<InstructionBox> = vec![
        CreateKaigi {
            call: NewKaigi::with_defaults(call_id.clone(), host),
            commitment: None,
            nullifier: None,
            roster_root: None,
            proof: None,
        }
        .into(),
        JoinKaigi {
            call_id: call_id.clone(),
            participant: participant.clone(),
            commitment: None,
            nullifier: None,
            roster_root: None,
            proof: None,
        }
        .into(),
        LeaveKaigi {
            call_id: call_id.clone(),
            participant,
            commitment: None,
            nullifier: None,
            roster_root: None,
            proof: None,
        }
        .into(),
        EndKaigi {
            call_id: call_id.clone(),
            ended_at_ms: None,
            commitment: None,
            nullifier: None,
            roster_root: None,
            proof: None,
        }
        .into(),
        RecordKaigiUsage {
            call_id: call_id.clone(),
            duration_ms: 1,
            billed_gas: 0,
            usage_commitment: None,
            proof: None,
        }
        .into(),
        SetKaigiRelayManifest {
            call_id: call_id.clone(),
            relay_manifest: None,
        }
        .into(),
        RegisterKaigiRelay {
            relay: KaigiRelayRegistration {
                relay_id: relay.clone(),
                hpke_public_key: vec![1],
                bandwidth_class: 1,
            },
        }
        .into(),
        UnregisterKaigiRelay {
            relay_id: relay.clone(),
        }
        .into(),
        ReportKaigiRelayHealth {
            call_id,
            relay_id: relay,
            status: KaigiRelayHealthStatus::Healthy,
            reported_at_ms: 0,
            notes: None,
        }
        .into(),
    ];

    for instruction in &instructions {
        assert_eq!(
            native_instruction_ds_effect_disposition(instruction, &policy_fee_asset(&policy),),
            NativeInstructionDsEffectDisposition::AuditedNoDsEffect,
            "{} must remain classified while validation fees are active",
            instruction.id()
        );
    }
    enforce_policy(&tx(1, instructions, Metadata::default()), &policy)
        .expect("Kaigi instructions must remain usable while validation fees are active");
}
#[test]
fn active_policy_admits_publicly_bound_kagemusha_fee_asset_conversions() {
    let treasury = account(3);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let top_up: InstructionBox =
        TopUpKagemushaRecursiveV4::new(kagemusha_top_up_request(&fee_asset)).into();
    let redeem: InstructionBox =
        RedeemKagemushaRecursiveV4::new(kagemusha_redeem_request(&fee_asset)).into();
    for instruction in [top_up, redeem] {
        assert_eq!(
            native_instruction_ds_effect_disposition(&instruction, &fee_asset),
            NativeInstructionDsEffectDisposition::AuditedKagemushaOfflineConversion,
        );
        let collection = collect_asset_transfers(
            &Executable::Instructions(vec![instruction.clone()].into()),
            &account(1),
            &fee_asset,
        )
        .expect("a publicly bound Kagemusha conversion must be classifiable");
        assert!(
            collection.transfers.is_empty(),
            "closed transparent/escrow conversion is not an account-to-account Transfer ISI",
        );
        enforce_policy(&tx(1, vec![instruction], Metadata::default()), &policy)
            .expect("Kagemusha conversion must remain usable for the policy fee asset");
    }
}
#[test]
fn kagemusha_conversion_admission_rejects_redirected_public_bindings() {
    let treasury = account(3);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let mut top_up = kagemusha_top_up_request(&fee_asset);
    top_up.authorization.authority = account(2);
    assert_eq!(
        enforce_policy(
            &tx(
                1,
                vec![TopUpKagemushaRecursiveV4::new(top_up).into()],
                Metadata::default(),
            ),
            &policy,
        ),
        Err(
            ValidationFeeAdmissionError::InvalidKagemushaOfflineConversion {
                context_index: 0,
                instruction_index: 0,
                instruction_wire_id: core::any::type_name::<TopUpKagemushaRecursiveV4>(),
            },
        ),
    );
    let mut redeem = kagemusha_redeem_request(&fee_asset);
    redeem.recipient = account(2);
    assert_eq!(
        enforce_policy(
            &tx(
                1,
                vec![RedeemKagemushaRecursiveV4::new(redeem).into()],
                Metadata::default(),
            ),
            &policy,
        ),
        Err(
            ValidationFeeAdmissionError::InvalidKagemushaOfflineConversion {
                context_index: 0,
                instruction_index: 0,
                instruction_wire_id: core::any::type_name::<RedeemKagemushaRecursiveV4>(),
            },
        ),
    );
}
#[test]
fn kagemusha_conversion_does_not_exempt_adjacent_fee_asset_transfers() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let top_up: InstructionBox =
        TopUpKagemushaRecursiveV4::new(kagemusha_top_up_request(&fee_asset)).into();
    let redeem: InstructionBox =
        RedeemKagemushaRecursiveV4::new(kagemusha_redeem_request(&fee_asset)).into();
    let principal = transfer(&user, &fee_asset, Quantity::from(1_u64), &recipient);
    for conversion in [top_up, redeem] {
        assert_eq!(
            enforce_policy(
                &tx(
                    1,
                    vec![conversion.clone(), principal.clone()],
                    Metadata::default(),
                ),
                &policy,
            ),
            Err(ValidationFeeAdmissionError::MissingFee {
                required_minor_units: TEST_VALIDATION_FEE_MINOR_UNITS,
            }),
            "an adjacent ordinary DS transfer must still pay the exact validation fee",
        );
        let fee = transfer(
            &user,
            &fee_asset,
            minor_units(TEST_VALIDATION_FEE_MINOR_UNITS),
            &treasury,
        );
        enforce_policy(
            &tx(
                1,
                vec![conversion, principal.clone(), fee],
                metadata_for_fee_instruction(&policy, 2),
            ),
            &policy,
        )
        .expect("the ordinary transfer remains admissible with its exact signed fee");
    }
}
#[test]
fn transfer_to_unregistered_account_is_rejected_as_hidden_fee_candidate() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let collection = collect_asset_transfers(
        &Executable::Instructions(
            vec![transfer(
                &user,
                &fee_asset,
                Quantity::from(1_u64),
                &recipient,
            )]
            .into(),
        ),
        &user,
        &fee_asset,
    )
    .expect("transparent transfer is classifiable");
    assert_eq!(
        reject_potential_implicit_account_admission_fee_with(&collection, |_| false),
        Err(
            ValidationFeeAdmissionError::PotentialImplicitAccountAdmissionFee {
                context_index: 0,
                instruction_index: 0,
                entry_index: None,
                destination_account_id: recipient.to_string(),
            }
        )
    );
    reject_potential_implicit_account_admission_fee_with(&collection, |_| true)
        .expect("an already registered recipient cannot derive account-admission fees");
}
#[test]
fn active_policy_rejects_same_label_with_a_different_exact_network() {
    let treasury = account(3);
    let policy = policy(&treasury);
    let first_display_label = ChainId::from("shared-display-label");
    let second_display_label = ChainId::from("shared-display-label");
    assert_eq!(first_display_label, second_display_label);
    validate_policy_network_id(&policy, &validation_fee_test_network_id())
        .expect("matching exact network id should validate");
    let foreign_network_id = iroha_data_model::NetworkId::from_genesis_hash(
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([9; 32])),
    );
    assert_eq!(
        validate_policy_network_id(&policy, &foreign_network_id),
        Err(ValidationFeeAdmissionError::WrongPolicyNetwork {
            expected: foreign_network_id.to_string(),
            found: policy.network_id.to_string(),
        })
    );
}
#[test]
fn active_policy_registry_requires_monotonic_chain() {
    let treasury = account(3);
    let first = policy(&treasury);
    let second = successor_policy(&first);
    let registry = policy_registry(&[first.clone(), second.clone()]);
    registry.validate().expect("valid policy chain");
    let mut skipped = registry.clone();
    skipped.registered_policies[1].policy.policy_version = 3;
    assert!(matches!(
            skipped.validate(),
            Err(iroha_data_model::validation_fee::ValidationFeePolicyRegistryError::UnexpectedPolicyVersion {
                expected: 2,
                found: 3,
            })
        ));
    let mut broken_previous = registry.clone();
    broken_previous.registered_policies[1]
        .policy
        .previous_policy_hash = Some([9; 32]);
    assert!(matches!(
            broken_previous.validate(),
            Err(iroha_data_model::validation_fee::ValidationFeePolicyRegistryError::BrokenPreviousPolicyHash {
                policy_version: 2,
            })
        ));
}
#[test]
fn enacted_initial_policy_remains_inactive_until_delayed_effective_height() {
    let future = policy_with_treasury_payout_lifecycle(treasury_payout_binding(
        test_contract_address(),
        b"future-policy-payout",
    ));
    let registry = policy_registry(std::slice::from_ref(&future));
    let state = crate::state::State::new_with_chain_and_network_id_for_testing(
        crate::state::World::default(),
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
        "generic-testnet".parse().expect("chain id"),
        validation_fee_test_network_id(),
    );
    let header = iroha_data_model::block::BlockHeader::new(
        std::num::NonZeroU64::new(9).expect("height"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut state_tx = block.transaction();
    install_policy_registry_fixture(&registry, &mut state_tx);
    assert!(
        active_policy(&state_tx)
            .expect("future initial policy is valid")
            .is_none(),
        "the mandatory 120,960-block activation delay must not halt pre-activation writes"
    );
}
#[test]
fn active_policy_lookup_rejects_the_exact_expiry_height() {
    use iroha_data_model::block::BlockHeader;
    let deployer_key = key_pair(55);
    let deployer = AccountId::new(deployer_key.public_key().clone());
    let state = crate::state::State::new_with_chain_and_network_id_for_testing(
        validation_fee_payout_world(&deployer),
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
        "generic-testnet".parse().expect("chain id"),
        validation_fee_test_network_id(),
    );
    let expiry_height = TEST_POLICY_EFFECTIVE_HEIGHT + 100;
    let header = BlockHeader::new(
        std::num::NonZeroU64::new(expiry_height).expect("expiry height is non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut state_tx = block.transaction();
    let policy =
        install_active_bound_validation_fee_policy(&mut state_tx, &deployer, &deployer_key);
    assert_eq!(policy.expires_after_height, Some(expiry_height));
    let error = active_policy(&state_tx)
        .expect_err("the exclusive expiry height must reject fee admission");
    assert!(
        matches!(
            error,
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(ref message))
                if message.contains("expired at height")
                    && message.contains(&expiry_height.to_string())
        ),
        "unexpected expired-policy rejection: {error:?}",
    );
}
#[test]
fn active_policy_window_rejects_expired_policy() {
    let treasury = account(3);
    let policy = policy(&treasury);
    assert!(!policy.is_active_at_height(policy.effective_from_height - 1));
    assert!(policy.is_active_at_height(policy.effective_from_height));
    let successor = successor_policy(&policy);
    assert!(!successor.is_active_at_height(successor.effective_from_height.saturating_sub(1)));
    assert!(policy.is_active_at_height(policy.expires_after_height.expect("expiry height") - 1));
    assert!(!policy.is_active_at_height(policy.expires_after_height.expect("expiry height")));
}
#[test]
fn active_policy_requires_exact_fee_and_transaction_bound_metadata() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let tx = tx(
        1,
        vec![
            transfer(&user, &fee_asset, Quantity::from(1_u64), &recipient),
            transfer(&user, &fee_asset, minor_units(10), &treasury),
        ],
        metadata_for_fee_instruction(&policy, 1),
    );
    enforce_policy(&tx, &policy).expect("valid fee-bearing transaction");
}
#[test]
fn hijiri_account_risk_changes_the_exact_validation_fee() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let hijiri = hijiri_parameters(Q16::ONE);
    let multiplier = hijiri
        .multiplier_for(&user, None)
        .expect("default multiplier");
    let quote_hash = hijiri
        .fee_quote_hash(&user, None)
        .expect("default quote hash");
    assert_eq!(
        required_fee_minor_units(1, &policy, Some(multiplier)).expect("bounded Hijiri fee"),
        20
    );

    let exact = tx(
        1,
        vec![
            transfer(&user, &fee_asset, Quantity::from(1_u64), &recipient),
            transfer(&user, &fee_asset, minor_units(20), &treasury),
        ],
        metadata_for_hijiri_fee_instruction(&policy, quote_hash, 1),
    );
    assert_eq!(
        enforce_policy_with_credit_and_hijiri(
            &exact,
            &policy,
            Some(&hijiri),
            &no_hijiri_account_risk,
        )
        .expect("risk-adjusted fee validates"),
        20
    );

    let stale_base_fee = tx(
        1,
        vec![
            transfer(&user, &fee_asset, Quantity::from(1_u64), &recipient),
            transfer(&user, &fee_asset, minor_units(10), &treasury),
        ],
        metadata_for_hijiri_fee_instruction(&policy, quote_hash, 1),
    );
    assert_eq!(
        enforce_policy_with_credit_and_hijiri(
            &stale_base_fee,
            &policy,
            Some(&hijiri),
            &no_hijiri_account_risk,
        ),
        Err(ValidationFeeAdmissionError::WrongFeeAmount {
            expected_minor_units: 20,
            observed_minor_units: 10,
        })
    );
}
#[test]
fn hijiri_explicit_low_risk_preserves_the_base_fee() {
    let user = account(1);
    let policy = policy(&account(3));
    let hijiri = hijiri_parameters(Q16::ONE);
    let account_risk = HijiriAccountRiskV1::try_new(user.clone(), 1, None, Q16::ZERO).unwrap();
    let multiplier = hijiri
        .multiplier_for(&user, Some(&account_risk))
        .expect("explicit multiplier");
    assert_eq!(
        required_fee_minor_units(3, &policy, Some(multiplier)).expect("bounded Hijiri fee"),
        30
    );
}
#[test]
fn hijiri_quote_binding_rejects_same_fee_after_risk_record_appears() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let hijiri = hijiri_parameters(Q16::ZERO);
    let stale_quote_hash = hijiri.fee_quote_hash(&user, None).unwrap();
    let account_risk = HijiriAccountRiskV1::try_new(user.clone(), 1, None, Q16::ZERO).unwrap();
    let expected_quote_hash = hijiri.fee_quote_hash(&user, Some(&account_risk)).unwrap();
    assert_ne!(stale_quote_hash, expected_quote_hash);
    let signed_before_record = tx(
        1,
        vec![
            transfer(&user, &fee_asset, Quantity::from(1_u64), &recipient),
            transfer(&user, &fee_asset, minor_units(10), &treasury),
        ],
        metadata_for_hijiri_fee_instruction(&policy, stale_quote_hash, 1),
    );
    let risk_for_user =
        |account_id: &AccountId| Ok((account_id == &user).then(|| account_risk.clone()));
    assert_eq!(
        enforce_policy_with_credit_and_hijiri(
            &signed_before_record,
            &policy,
            Some(&hijiri),
            &risk_for_user,
        ),
        Err(
            ValidationFeeAdmissionError::WrongHijiriFeeQuoteHashMetadata {
                expected_hash_hex: hex::encode(expected_quote_hash),
                observed_hash_hex: hex::encode(stale_quote_hash),
            }
        )
    );
}
#[test]
fn hijiri_quote_binding_is_mandatory_and_canonical_only_while_active() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let hijiri = hijiri_parameters(Q16::ZERO);
    let quote_hash = hijiri.fee_quote_hash(&user, None).unwrap();
    let instructions = || {
        vec![
            transfer(&user, &fee_asset, Quantity::from(1_u64), &recipient),
            transfer(&user, &fee_asset, minor_units(10), &treasury),
        ]
    };

    let missing = tx(1, instructions(), metadata_for_fee_instruction(&policy, 1));
    assert_eq!(
        enforce_policy_with_credit_and_hijiri(
            &missing,
            &policy,
            Some(&hijiri),
            &no_hijiri_account_risk,
        ),
        Err(ValidationFeeAdmissionError::MissingHijiriFeeQuoteHashMetadata)
    );

    let unexpected = tx(
        1,
        instructions(),
        metadata_for_hijiri_fee_instruction(&policy, quote_hash, 1),
    );
    assert_eq!(
        enforce_policy_with_credit_and_hijiri(&unexpected, &policy, None, &no_hijiri_account_risk,),
        Err(ValidationFeeAdmissionError::UnexpectedHijiriFeeQuoteHashMetadata)
    );

    let mut malformed_metadata = metadata_for_hijiri_fee_instruction(&policy, quote_hash, 1);
    malformed_metadata.insert(
        Name::from_str(VALIDATION_FEE_HIJIRI_FEE_QUOTE_HASH_METADATA_KEY).expect("metadata key"),
        Json::new(hex::encode_upper(quote_hash)),
    );
    let malformed = tx(1, instructions(), malformed_metadata);
    assert_eq!(
        enforce_policy_with_credit_and_hijiri(
            &malformed,
            &policy,
            Some(&hijiri),
            &no_hijiri_account_risk,
        ),
        Err(ValidationFeeAdmissionError::MalformedHijiriFeeQuoteHashMetadata)
    );
}
#[test]
fn hijiri_quote_hash_only_metadata_is_detected_and_rejected_as_incomplete() {
    let policy = policy(&account(3));
    let mut metadata = Metadata::default();
    metadata.insert(
        Name::from_str(VALIDATION_FEE_HIJIRI_FEE_QUOTE_HASH_METADATA_KEY).expect("metadata key"),
        Json::new("ab".repeat(32)),
    );
    let transaction = tx(1, Vec::new(), metadata);

    assert!(has_validation_fee_metadata(transaction.metadata()));
    assert!(transaction_has_validation_fee_metadata(&transaction));
    assert_eq!(
        enforce_policy(&transaction, &policy),
        Err(ValidationFeeAdmissionError::MissingPolicyVersionMetadata)
    );
}
#[test]
fn active_policy_rejects_raw_contract_and_ivm_executables_fail_closed() {
    let treasury = account(3);
    let policy = policy(&treasury);
    let contract_call = contract_call_tx(1, metadata_for(&policy));
    let raw_ivm = ivm_tx(1, metadata_for(&policy));
    assert_eq!(
        enforce_policy(&contract_call, &policy),
        Err(ValidationFeeAdmissionError::UnsupportedExecutable)
    );
    assert_eq!(
        enforce_policy(&raw_ivm, &policy),
        Err(ValidationFeeAdmissionError::UnsupportedExecutable)
    );
}
#[test]
fn native_repo_ds_movements_fail_closed_at_top_level() {
    let initiator = account(1);
    let counterparty = account(2);
    let treasury = account(3);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let xor = asset_definition("xor");
    let blocked = [
        (
            InstructionBox::from(RepoInstructionBox::from(repo_initiate(
                "repo_ds_cash",
                &initiator,
                &counterparty,
                &fee_asset,
                &xor,
            ))),
            RepoIsi::WIRE_ID,
        ),
        (
            InstructionBox::from(RepoInstructionBox::from(repo_initiate(
                "repo_ds_collateral",
                &initiator,
                &counterparty,
                &xor,
                &fee_asset,
            ))),
            RepoIsi::WIRE_ID,
        ),
        (
            InstructionBox::from(RepoInstructionBox::Reverse(repo_reverse(
                "reverse_repo_state_derived_boxed",
            ))),
            ReverseRepoIsi::WIRE_ID,
        ),
        (
            InstructionBox::from(repo_reverse("reverse_repo_state_derived_direct")),
            ReverseRepoIsi::WIRE_ID,
        ),
    ];
    for (instruction, instruction_wire_id) in blocked {
        let transaction = tx(1, vec![instruction], Metadata::default());
        assert_eq!(
            enforce_policy(&transaction, &policy),
            Err(
                ValidationFeeAdmissionError::UnsupportedNativeFeeAssetMovement {
                    context_index: 0,
                    instruction_index: 0,
                    instruction_wire_id,
                }
            )
        );
    }
    let non_ds = tx(
        1,
        vec![
            RepoInstructionBox::from(repo_initiate(
                "repo_non_ds",
                &initiator,
                &counterparty,
                &xor,
                &xor,
            ))
            .into(),
        ],
        Metadata::default(),
    );
    enforce_policy(&non_ds, &policy).expect("non-DS repo remains generic");
    let margin_call = tx(
        1,
        vec![
            RepoInstructionBox::MarginCall(RepoMarginCallIsi::new(
                "repo_margin_only".parse().expect("repo agreement id"),
            ))
            .into(),
        ],
        Metadata::default(),
    );
    enforce_policy(&margin_call, &policy).expect("repo margin call has no balance effect");
}
#[test]
fn native_settlement_ds_movements_fail_closed_through_wrappers() {
    let initiator = account(1);
    let counterparty = account(2);
    let treasury = account(3);
    let multisig = account(4);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let xor = asset_definition("xor");
    let dvp = DvpIsi::new(
        "wrapped_ds_dvp".parse().expect("settlement id"),
        settlement_leg(&xor, &initiator, &counterparty),
        settlement_leg(&fee_asset, &counterparty, &initiator),
        SettlementPlan::default(),
    );
    let proved = ivm_proved_tx(
        1,
        vec![SettlementInstructionBox::Dvp(dvp).into()],
        Metadata::default(),
    );
    assert_eq!(
        enforce_policy(&proved, &policy),
        Err(
            ValidationFeeAdmissionError::UnsupportedNativeFeeAssetMovement {
                context_index: 0,
                instruction_index: 0,
                instruction_wire_id: DvpIsi::WIRE_ID,
            }
        )
    );
    let pvp = PvpIsi::new(
        "multisig_ds_pvp".parse().expect("settlement id"),
        settlement_leg(&xor, &multisig, &counterparty),
        settlement_leg(&fee_asset, &counterparty, &multisig),
        SettlementPlan::default(),
    );
    let proposed = tx(
        1,
        vec![
            MultisigPropose::new(
                multisig,
                vec![SettlementInstructionBox::Pvp(pvp).into()],
                None,
            )
            .into(),
        ],
        Metadata::default(),
    );
    assert_eq!(
        enforce_policy(&proposed, &policy),
        Err(
            ValidationFeeAdmissionError::UnsupportedNativeFeeAssetMovement {
                context_index: 1,
                instruction_index: 0,
                instruction_wire_id: PvpIsi::WIRE_ID,
            }
        )
    );
    let non_ds_dvp = DvpIsi::new(
        "wrapped_non_ds_dvp".parse().expect("settlement id"),
        settlement_leg(&xor, &initiator, &counterparty),
        settlement_leg(&xor, &counterparty, &initiator),
        SettlementPlan::default(),
    );
    let non_ds_proved = ivm_proved_tx(
        1,
        vec![SettlementInstructionBox::Dvp(non_ds_dvp).into()],
        Metadata::default(),
    );
    enforce_policy(&non_ds_proved, &policy).expect("non-DS settlement remains generic");
}
#[test]
fn opaque_trigger_artifacts_reject_native_repo_ds_movement() {
    let initiator = account(1);
    let counterparty = account(2);
    let treasury = account(3);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let xor = asset_definition("xor");
    let trigger_id: iroha_data_model::trigger::TriggerId =
        "opaque_repo_ds_trigger".parse().expect("trigger id");
    let trigger = Trigger::new(
        trigger_id.clone(),
        Action::new(
            vec![InstructionBox::from(RepoInstructionBox::from(
                repo_initiate(
                    "trigger_repo_ds",
                    &initiator,
                    &counterparty,
                    &fee_asset,
                    &xor,
                ),
            ))],
            Repeats::Indefinitely,
            initiator.clone(),
            ExecuteTriggerEventFilter::new().for_trigger(trigger_id),
        )
        .expect("trigger action fixture satisfies validation invariants"),
    );
    let instruction_groups = std::collections::BTreeMap::from([(
        initiator,
        vec![RegisterBox::Trigger(Register::trigger(trigger)).into()],
    )]);
    assert_eq!(
        enforce_opaque_deferred_policy(&instruction_groups, &policy, None),
        Err(
            ValidationFeeAdmissionError::UnsupportedNativeFeeAssetMovement {
                context_index: 0,
                instruction_index: 0,
                instruction_wire_id: RepoIsi::WIRE_ID,
            }
        )
    );
}
#[test]
fn ivm_proved_overlay_requires_exact_validation_fee() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let exact = ivm_proved_tx(
        1,
        vec![
            transfer(&user, &fee_asset, Quantity::from(1_u64), &recipient),
            transfer(&user, &fee_asset, minor_units(10), &treasury),
        ],
        metadata_for_fee_instruction(&policy, 1),
    );
    enforce_policy(&exact, &policy).expect("exact proved-IVM overlay fee should validate");
    let missing = ivm_proved_tx(
        1,
        vec![transfer(
            &user,
            &fee_asset,
            Quantity::from(1_u64),
            &recipient,
        )],
        metadata_for(&policy),
    );
    assert_eq!(
        enforce_policy(&missing, &policy),
        Err(ValidationFeeAdmissionError::MissingFee {
            required_minor_units: 10,
        })
    );
    for observed_minor_units in [9, 11] {
        let wrong = ivm_proved_tx(
            1,
            vec![
                transfer(&user, &fee_asset, Quantity::from(1_u64), &recipient),
                transfer(
                    &user,
                    &fee_asset,
                    minor_units(observed_minor_units),
                    &treasury,
                ),
            ],
            metadata_for_fee_instruction(&policy, 1),
        );
        assert_eq!(
            enforce_policy(&wrong, &policy),
            Err(ValidationFeeAdmissionError::WrongFeeAmount {
                expected_minor_units: 10,
                observed_minor_units,
            })
        );
    }
}
#[test]
fn deferred_instruction_list_requires_exact_execution_time_fee() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let principal = || transfer(&user, &fee_asset, Quantity::from(1_u64), &recipient);
    assert_eq!(
        enforce_deferred_policy(&user, &[principal()], &policy),
        Err(ValidationFeeAdmissionError::MissingMultisigFeeMarker { context_index: 0 })
    );
    let missing_fee = with_multisig_fee_marker(&policy, vec![principal()], 1, None);
    assert_eq!(
        enforce_deferred_policy(&user, &missing_fee, &policy),
        Err(ValidationFeeAdmissionError::FeeInstructionNotFound {
            instruction_index: 1,
            entry_index: None,
        })
    );
    let exact = with_multisig_fee_marker(
        &policy,
        vec![
            principal(),
            transfer(&user, &fee_asset, minor_units(10), &treasury),
        ],
        1,
        None,
    );
    enforce_deferred_policy(&user, &exact, &policy)
        .expect("deferred principal and exact fee should validate atomically");
    for observed_minor_units in [9, 11] {
        assert_eq!(
            enforce_deferred_policy(
                &user,
                &with_multisig_fee_marker(
                    &policy,
                    vec![
                        principal(),
                        transfer(
                            &user,
                            &fee_asset,
                            minor_units(observed_minor_units),
                            &treasury,
                        ),
                    ],
                    1,
                    None,
                ),
                &policy
            ),
            Err(ValidationFeeAdmissionError::WrongFeeAmount {
                expected_minor_units: 10,
                observed_minor_units,
            })
        );
    }
}
#[test]
fn deferred_multisig_marker_rejects_stale_hijiri_quote_with_same_fee() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let hijiri = hijiri_parameters(Q16::ZERO);
    let stale_hash = hijiri.fee_quote_hash(&user, None).unwrap();
    let account_risk = HijiriAccountRiskV1::try_new(user.clone(), 1, None, Q16::ZERO).unwrap();
    let current_hash = hijiri.fee_quote_hash(&user, Some(&account_risk)).unwrap();
    let instructions = |quote_hash| {
        with_multisig_fee_marker_and_hijiri(
            &policy,
            Some(quote_hash),
            vec![
                transfer(&user, &fee_asset, Quantity::from(1_u64), &recipient),
                transfer(&user, &fee_asset, minor_units(10), &treasury),
            ],
            1,
            None,
        )
    };
    let resolve_risk =
        |account_id: &AccountId| Ok((account_id == &user).then(|| account_risk.clone()));
    assert_eq!(
        enforce_deferred_policy_with_credit_and_hijiri(
            &user,
            &instructions(stale_hash),
            &policy,
            Some(&hijiri),
            &resolve_risk,
        ),
        Err(
            ValidationFeeAdmissionError::WrongMultisigFeeMarkerHijiriFeeQuoteHash {
                expected_hash_hex: Some(hex::encode(current_hash)),
                observed_hash_hex: Some(hex::encode(stale_hash)),
            }
        )
    );
    assert_eq!(
        enforce_deferred_policy_with_credit_and_hijiri(
            &user,
            &instructions(current_hash),
            &policy,
            Some(&hijiri),
            &resolve_risk,
        )
        .expect("current Hijiri marker must validate"),
        10
    );
}
#[test]
fn deferred_multisig_marker_is_unique_policy_bound_and_batch_aware() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let principal = || transfer(&user, &fee_asset, Quantity::from(1_u64), &recipient);
    let fee = || transfer(&user, &fee_asset, minor_units(10), &treasury);
    let mut duplicate = with_multisig_fee_marker(&policy, vec![principal(), fee()], 1, None);
    duplicate.push(
        ValidationFeeMultisigMarkerV1::new(
            policy.policy_version,
            policy.policy_hash().expect("policy hash"),
            None,
            1,
            None,
        )
        .into_instruction(),
    );
    assert_eq!(
        enforce_deferred_policy(&user, &duplicate, &policy),
        Err(ValidationFeeAdmissionError::DuplicateMultisigFeeMarkers {
            context_index: 0,
            count: 2,
        })
    );
    let malformed: InstructionBox = Log::new(
        Level::TRACE,
        "iroha:validation_fee:multisig:v1:malformed".to_owned(),
    )
    .into();
    assert_eq!(
        enforce_deferred_policy(&user, &[principal(), fee(), malformed], &policy),
        Err(ValidationFeeAdmissionError::MalformedMultisigFeeMarker {
            context_index: 0,
            instruction_index: 2,
        })
    );
    let wrong_version = vec![
        principal(),
        fee(),
        ValidationFeeMultisigMarkerV1::new(
            policy.policy_version + 1,
            policy.policy_hash().expect("policy hash"),
            None,
            1,
            None,
        )
        .into_instruction(),
    ];
    assert_eq!(
        enforce_deferred_policy(&user, &wrong_version, &policy),
        Err(
            ValidationFeeAdmissionError::WrongMultisigFeeMarkerPolicyVersion {
                expected_version: policy.policy_version,
                observed_version: policy.policy_version + 1,
            }
        )
    );
    let wrong_hash = vec![
        principal(),
        fee(),
        ValidationFeeMultisigMarkerV1::new(policy.policy_version, [0x55; 32], None, 1, None)
            .into_instruction(),
    ];
    assert_eq!(
        enforce_deferred_policy(&user, &wrong_hash, &policy),
        Err(
            ValidationFeeAdmissionError::WrongMultisigFeeMarkerPolicyHash {
                expected_hash_hex: hex::encode(policy.policy_hash().expect("policy hash")),
                observed_hash_hex: hex::encode([0x55; 32]),
            }
        )
    );
    let wrong_coordinate = with_multisig_fee_marker(&policy, vec![principal(), fee()], 0, None);
    assert!(matches!(
        enforce_deferred_policy(&user, &wrong_coordinate, &policy),
        Err(ValidationFeeAdmissionError::WrongFeeBeneficiary { .. })
    ));
    let batch = TransferAssetBatch::new(vec![
        TransferAssetBatchEntry::new(user.clone(), recipient, fee_asset.clone(), 1_u64),
        TransferAssetBatchEntry::new(
            user.clone(),
            treasury.clone(),
            fee_asset.clone(),
            minor_units(10),
        ),
    ]);
    let batch_with_marker =
        with_multisig_fee_marker(&policy, vec![InstructionBox::from(batch)], 0, Some(1));
    enforce_deferred_policy(&user, &batch_with_marker, &policy)
        .expect("canonical batch-entry marker validates exact deferred fee");
    let unrelated_treasury_inflow = with_multisig_fee_marker(
        &policy,
        vec![transfer(&user, &fee_asset, minor_units(10), &treasury)],
        0,
        None,
    );
    assert_eq!(
        enforce_deferred_policy(&user, &unrelated_treasury_inflow, &policy),
        Err(ValidationFeeAdmissionError::WrongFeeAmount {
            expected_minor_units: 0,
            observed_minor_units: 10,
        })
    );
}
#[test]
fn opaque_deferred_artifacts_reject_fee_asset_but_allow_generic_assets() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let xor = asset_definition("xor");
    let mut instruction_groups = std::collections::BTreeMap::new();
    instruction_groups.insert(
        user.clone(),
        vec![transfer(&user, &xor, Quantity::from(1_u64), &recipient)],
    );
    enforce_opaque_deferred_policy(&instruction_groups, &policy, None)
        .expect("opaque non-fee-asset artifacts remain generic");
    instruction_groups.insert(
        user.clone(),
        vec![
            transfer(&user, &fee_asset, Quantity::from(1_u64), &recipient),
            transfer(&user, &fee_asset, minor_units(10), &treasury),
        ],
    );
    assert_eq!(
        enforce_opaque_deferred_policy(&instruction_groups, &policy, None),
        Err(
            ValidationFeeAdmissionError::OpaqueDeferredFeeAssetTransfer {
                execution_account_id: user.to_string(),
                instruction_index: 0,
                entry_index: None,
            }
        )
    );
    let trigger_id: iroha_data_model::trigger::TriggerId =
        "opaque_derived_ds_trigger".parse().expect("trigger id");
    let trigger = Trigger::new(
        trigger_id.clone(),
        Action::new(
            vec![
                transfer(&user, &fee_asset, Quantity::from(1_u64), &recipient),
                transfer(&user, &fee_asset, minor_units(10), &treasury),
            ],
            Repeats::Indefinitely,
            user.clone(),
            ExecuteTriggerEventFilter::new().for_trigger(trigger_id),
        )
        .expect("trigger action fixture satisfies validation invariants"),
    );
    instruction_groups.insert(
        user.clone(),
        vec![RegisterBox::Trigger(Register::trigger(trigger)).into()],
    );
    assert!(matches!(
        enforce_opaque_deferred_policy(&instruction_groups, &policy, None),
        Err(ValidationFeeAdmissionError::OpaqueDeferredFeeAssetTransfer { .. })
    ));
    let nested_trigger_id: iroha_data_model::trigger::TriggerId =
        "multisig_wrapped_opaque_ds_trigger"
            .parse()
            .expect("trigger id");
    let nested_trigger = Trigger::new(
        nested_trigger_id.clone(),
        Action::new(
            vec![
                transfer(&user, &fee_asset, Quantity::from(1_u64), &recipient),
                transfer(&user, &fee_asset, minor_units(10), &treasury),
            ],
            Repeats::Indefinitely,
            user.clone(),
            ExecuteTriggerEventFilter::new().for_trigger(nested_trigger_id),
        )
        .expect("trigger action fixture satisfies validation invariants"),
    );
    let multisig = account(4);
    instruction_groups.insert(
        user.clone(),
        vec![
            MultisigPropose::new(
                multisig.clone(),
                vec![RegisterBox::Trigger(Register::trigger(nested_trigger)).into()],
                None,
            )
            .into(),
        ],
    );
    assert!(matches!(
        enforce_opaque_deferred_policy(&instruction_groups, &policy, None),
        Err(ValidationFeeAdmissionError::OpaqueDeferredFeeAssetTransfer { .. })
    ));
    let proposal_instructions = vec![
        transfer(&multisig, &fee_asset, Quantity::from(1_u64), &recipient),
        transfer(&multisig, &fee_asset, minor_units(10), &treasury),
    ];
    let proposal_hash = HashOf::new(&proposal_instructions);
    let approve = MultisigApprove::new(multisig.clone(), proposal_hash);
    let approve_instruction: InstructionBox = approve.clone().into();
    let assert_indirect_approval_rejected = |instructions: Vec<InstructionBox>| {
        let mut visited = std::collections::BTreeSet::new();
        let mut resolver = |candidate: &MultisigApprove| {
            (candidate == &approve).then(|| (multisig.clone(), proposal_instructions.clone()))
        };
        assert!(matches!(
            reject_opaque_deferred_approval_effects_with(
                &user,
                &instructions,
                &fee_asset,
                &mut visited,
                0,
                &mut resolver,
            ),
            Err(ValidationFeeAdmissionError::OpaqueDeferredFeeAssetTransfer { .. })
        ));
    };
    assert_indirect_approval_rejected(vec![approve_instruction.clone()]);
    assert_indirect_approval_rejected(vec![
        MultisigPropose::new(account(5), vec![approve_instruction.clone()], None).into(),
    ]);
    let approval_trigger_id: iroha_data_model::trigger::TriggerId =
        "opaque_multisig_approval_trigger"
            .parse()
            .expect("trigger id");
    let approval_trigger = Trigger::new(
        approval_trigger_id.clone(),
        Action::new(
            vec![approve_instruction],
            Repeats::Indefinitely,
            user.clone(),
            ExecuteTriggerEventFilter::new().for_trigger(approval_trigger_id),
        )
        .expect("trigger action fixture satisfies validation invariants"),
    );
    assert_indirect_approval_rejected(vec![
        RegisterBox::Trigger(Register::trigger(approval_trigger)).into(),
    ]);
}
#[test]
fn opaque_treasury_payout_exception_is_direct_source_and_authority_bound() {
    let binding = treasury_payout_binding(test_contract_address(), b"bound-pool");
    let treasury = binding.treasury_account_id.clone();
    let recipient = account(2);
    let other = account(7);
    let policy = policy_with_treasury_payout_lifecycle(binding);
    let fee_asset = policy_fee_asset(&policy);
    let direct_payout = std::collections::BTreeMap::from([(
        treasury.clone(),
        vec![transfer(
            &treasury,
            &fee_asset,
            Quantity::from(1_u64),
            &recipient,
        )],
    )]);
    assert_eq!(
        enforce_opaque_deferred_policy(&direct_payout, &policy, None),
        Err(
            ValidationFeeAdmissionError::OpaqueDeferredFeeAssetTransfer {
                execution_account_id: treasury.to_string(),
                instruction_index: 0,
                entry_index: None,
            }
        ),
        "a payout exemption must not apply without a verified runtime origin",
    );
    enforce_opaque_deferred_policy(&direct_payout, &policy, Some(&treasury))
        .expect("a verified contract-subject treasury may make its enacted-lifecycle payout");
    let wrong_authority = std::collections::BTreeMap::from([(
        other.clone(),
        vec![transfer(
            &treasury,
            &fee_asset,
            Quantity::from(1_u64),
            &recipient,
        )],
    )]);
    assert!(matches!(
        enforce_opaque_deferred_policy(&wrong_authority, &policy, Some(&treasury)),
        Err(ValidationFeeAdmissionError::OpaqueDeferredFeeAssetTransfer {
            execution_account_id,
            ..
        }) if execution_account_id == other.to_string()
    ));
    let wrong_source = std::collections::BTreeMap::from([(
        treasury.clone(),
        vec![transfer(
            &other,
            &fee_asset,
            Quantity::from(1_u64),
            &recipient,
        )],
    )]);
    assert!(matches!(
        enforce_opaque_deferred_policy(&wrong_source, &policy, Some(&treasury)),
        Err(ValidationFeeAdmissionError::OpaqueDeferredFeeAssetTransfer { .. })
    ));
    let nested = MultisigPropose::new(
        treasury.clone(),
        vec![transfer(
            &treasury,
            &fee_asset,
            Quantity::from(1_u64),
            &recipient,
        )],
        None,
    );
    let nested_group =
        std::collections::BTreeMap::from([(treasury.clone(), vec![InstructionBox::from(nested)])]);
    assert!(matches!(
        enforce_opaque_deferred_policy(&nested_group, &policy, Some(&treasury)),
        Err(ValidationFeeAdmissionError::OpaqueDeferredFeeAssetTransfer { .. })
    ));
}
#[test]
fn treasury_payout_terms_scale_partial_credit_with_checked_rounding() {
    let binding = treasury_payout_binding(test_contract_address(), b"bound-pool");
    let scale = u32::from(TEST_VALIDATION_FEE_ASSET_SCALE);
    let cases = [
        ("0", None),
        ("0.01", Some(("0.01", "0.10"))),
        ("3.33", Some(("1.34", "33.30"))),
        ("10", Some(("4", "100"))),
        ("11", Some(("4", "100"))),
    ];
    for (credit, expected) in cases {
        let credit: Quantity = credit.parse().expect("canonical test DS credit");
        let terms =
            validation_fee_payout_terms(&credit, &binding, TEST_VALIDATION_FEE_ASSET_SCALE, scale)
                .expect("test quote arithmetic is valid");
        match (terms, expected) {
            (None, None) => {}
            (Some(terms), Some((min, max))) => {
                assert_eq!(terms.debit_ds, credit.min(binding.batch_ds.clone()));
                assert_eq!(terms.min_xor_out, min.parse().expect("test minimum XOR"));
                assert_eq!(terms.max_xor_out, max.parse().expect("test maximum XOR"));
            }
            (observed, expected) => {
                panic!("quote-term mismatch for credit {credit}: {observed:?} != {expected:?}")
            }
        }
    }
    assert!(
        validation_fee_payout_terms(
            &"0.01".parse().expect("minimum DS minor unit"),
            &binding,
            TEST_VALIDATION_FEE_ASSET_SCALE,
            0,
        )
        .expect("integer XOR arithmetic is valid")
        .is_none(),
        "ceil(min) above floor(max) must produce a no-quote result"
    );
}
#[test]
fn treasury_payout_split_is_dust_free_in_canonical_account_order() {
    let binding = treasury_payout_binding(test_contract_address(), b"bound-pool");
    let payouts = canonical_validation_fee_payouts(
        &binding,
        &"20.03".parse().expect("fractional XOR quote"),
        u32::from(TEST_VALIDATION_FEE_ASSET_SCALE),
    )
    .expect("split arithmetic is valid")
    .expect("quote funds all recipients");
    assert!(
        payouts.windows(2).all(|pair| pair[0].0 < pair[1].0),
        "recipient order must be canonical AccountId order"
    );
    assert_eq!(
        payouts
            .iter()
            .map(|(_, amount)| amount.clone())
            .collect::<Vec<_>>(),
        vec![
            "5.01".parse().expect("first remainder share"),
            "5.01".parse().expect("second remainder share"),
            "5.01".parse().expect("third remainder share"),
            "5".parse().expect("quotient-only share"),
        ]
    );
    let total = payouts
        .iter()
        .try_fold(Quantity::zero(), |sum, (_, amount)| sum.checked_add(amount))
        .expect("split sum remains representable");
    assert_eq!(total, "20.03".parse().expect("exact XOR total"));
}
#[test]
fn treasury_payout_effect_plan_rejects_every_unbound_substitution() {
    let binding = treasury_payout_binding(test_contract_address(), b"bound-pool");
    let treasury = binding.treasury_account_id.clone();
    let canonical = canonical_treasury_payout_plan(&binding, Quantity::from(20_u64));
    let canonical_groups =
        std::collections::BTreeMap::from([(treasury.clone(), canonical.clone())]);
    let canonical_ordered = ordered_treasury_payout_plan(&binding, &canonical);
    let terms = validation_fee_payout_terms(
        &binding.batch_ds,
        &binding,
        TEST_VALIDATION_FEE_ASSET_SCALE,
        u32::from(TEST_VALIDATION_FEE_ASSET_SCALE),
    )
    .expect("derive full-batch test payout terms")
    .expect("full-batch test payout has a nonempty range");
    assert!(
        validate_treasury_payout_effect_plan(
            &canonical_groups,
            &canonical_ordered,
            &binding,
            &terms,
        )
        .expect("the exact six-transfer plan is well formed")
    );
    let mut missing = canonical.clone();
    missing.pop();
    let missing_groups = std::collections::BTreeMap::from([(treasury.clone(), missing.clone())]);
    assert_treasury_payout_plan_mismatch(
        &binding,
        &missing_groups,
        &ordered_treasury_payout_plan(&binding, &missing),
    );
    let mut extra = canonical.clone();
    extra.push(canonical[5].clone());
    let extra_groups = std::collections::BTreeMap::from([(treasury.clone(), extra.clone())]);
    assert_treasury_payout_plan_mismatch(
        &binding,
        &extra_groups,
        &ordered_treasury_payout_plan(&binding, &extra),
    );
    let mut reordered = canonical_ordered.clone();
    reordered.swap(0, 1);
    assert_treasury_payout_plan_mismatch(&binding, &canonical_groups, &reordered);
    let mut wrong_batch = canonical.clone();
    wrong_batch[0] = transfer(
        &treasury,
        &binding.ds_asset_id,
        Quantity::from(2_u64),
        &binding.pool_vault_account_id,
    );
    let wrong_batch_groups =
        std::collections::BTreeMap::from([(treasury.clone(), wrong_batch.clone())]);
    assert_treasury_payout_plan_mismatch(
        &binding,
        &wrong_batch_groups,
        &ordered_treasury_payout_plan(&binding, &wrong_batch),
    );
    let mut wrong_ds_asset = canonical.clone();
    wrong_ds_asset[0] = transfer(
        &treasury,
        &binding.xor_asset_id,
        binding.batch_ds.clone(),
        &binding.pool_vault_account_id,
    );
    let wrong_ds_asset_groups =
        std::collections::BTreeMap::from([(treasury.clone(), wrong_ds_asset.clone())]);
    assert_treasury_payout_plan_mismatch(
        &binding,
        &wrong_ds_asset_groups,
        &ordered_treasury_payout_plan(&binding, &wrong_ds_asset),
    );
    let mut wrong_vault = canonical.clone();
    wrong_vault[1] = transfer(
        &account(7),
        &binding.xor_asset_id,
        Quantity::from(20_u64),
        &treasury,
    );
    let wrong_vault_groups =
        std::collections::BTreeMap::from([(treasury.clone(), wrong_vault.clone())]);
    assert_treasury_payout_plan_mismatch(
        &binding,
        &wrong_vault_groups,
        &ordered_treasury_payout_plan(&binding, &wrong_vault),
    );
    for outside_bound in [3_u64, 101_u64] {
        let out_of_bounds = canonical_treasury_payout_plan(&binding, Quantity::from(outside_bound));
        let out_of_bounds_groups =
            std::collections::BTreeMap::from([(treasury.clone(), out_of_bounds.clone())]);
        assert_treasury_payout_plan_mismatch(
            &binding,
            &out_of_bounds_groups,
            &ordered_treasury_payout_plan(&binding, &out_of_bounds),
        );
    }
    let mut wrong_validator = canonical.clone();
    wrong_validator[2] = transfer(
        &treasury,
        &binding.xor_asset_id,
        Quantity::from(5_u64),
        &account(7),
    );
    let wrong_validator_groups =
        std::collections::BTreeMap::from([(treasury.clone(), wrong_validator.clone())]);
    assert_treasury_payout_plan_mismatch(
        &binding,
        &wrong_validator_groups,
        &ordered_treasury_payout_plan(&binding, &wrong_validator),
    );
    let mut wrong_final_amount = canonical.clone();
    wrong_final_amount[5] = transfer(
        &treasury,
        &binding.xor_asset_id,
        Quantity::from(4_u64),
        &binding.recipients[3].account_id,
    );
    let wrong_final_groups =
        std::collections::BTreeMap::from([(treasury.clone(), wrong_final_amount.clone())]);
    assert_treasury_payout_plan_mismatch(
        &binding,
        &wrong_final_groups,
        &ordered_treasury_payout_plan(&binding, &wrong_final_amount),
    );
    let mut changed_shares = binding.clone();
    changed_shares.recipients[0].share = "0.20".parse().expect("changed share");
    changed_shares.recipients[1].share = "0.30".parse().expect("changed share");
    assert_treasury_payout_plan_mismatch(&changed_shares, &canonical_groups, &canonical_ordered);
    let other_authority = account(7);
    let wrong_authority_groups =
        std::collections::BTreeMap::from([(other_authority.clone(), canonical.clone())]);
    let wrong_authority_ordered = canonical
        .iter()
        .cloned()
        .map(|instruction| (other_authority.clone(), instruction))
        .collect::<Vec<_>>();
    assert_treasury_payout_plan_mismatch(
        &binding,
        &wrong_authority_groups,
        &wrong_authority_ordered,
    );
    let mut split_groups =
        std::collections::BTreeMap::from([(treasury.clone(), canonical[..5].to_vec())]);
    split_groups.insert(other_authority.clone(), vec![canonical[5].clone()]);
    let mut split_ordered = canonical_ordered;
    split_ordered[5].0 = other_authority;
    assert_treasury_payout_plan_mismatch(&binding, &split_groups, &split_ordered);
}
#[test]
fn opaque_deferred_unresolved_multisig_approval_fails_closed_against_state_mutation() {
    let user = account(1);
    let multisig = account(4);
    let fee_asset = fee_asset();
    let proposal_hash = HashOf::new(&Vec::<InstructionBox>::new());
    let approve = MultisigApprove::new(multisig.clone(), proposal_hash);
    let approve_instruction: InstructionBox = approve.clone().into();
    let mut visited = std::collections::BTreeSet::new();
    let mut resolver =
        |_candidate: &MultisigApprove| -> Option<(AccountId, Vec<InstructionBox>)> { None };
    assert_eq!(
        reject_opaque_deferred_approval_effects_with(
            &user,
            &[approve_instruction],
            &fee_asset,
            &mut visited,
            0,
            &mut resolver,
        ),
        Err(
            ValidationFeeAdmissionError::UnresolvedOpaqueDeferredMultisigApproval {
                account_id: multisig.to_string(),
                instructions_hash_hex: hex::encode(approve.instructions_hash.as_ref()),
            }
        )
    );
}
#[test]
fn missing_fee_is_rejected() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let tx = tx(
        1,
        vec![transfer(
            &user,
            &fee_asset,
            Quantity::from(1_u64),
            &recipient,
        )],
        metadata_for(&policy),
    );
    assert_eq!(
        enforce_policy(&tx, &policy),
        Err(ValidationFeeAdmissionError::MissingFee {
            required_minor_units: 10
        })
    );
}
#[test]
fn ivm_proved_axt_without_overlay_fee_fails_closed() {
    let treasury = account(3);
    let policy = policy(&treasury);
    let tx = ivm_proved_tx(1, Vec::new(), Metadata::default());
    assert_eq!(
        enforce_policy(&tx, &policy),
        Ok(()),
        "the signed overlay alone cannot observe an AXT-carried DS effect"
    );
    assert_eq!(
        reject_ivm_proved_completed_axt_effects(1),
        Err(ValidationFeeAdmissionError::OpaqueIvmProvedAxtEffects {
            completed_envelopes: 1,
        })
    );
}
#[test]
fn ivm_proved_axt_with_exact_overlay_fee_still_fails_closed() {
    let user = account(1);
    let recipient = account(2);
    let treasury = account(3);
    let policy = policy(&treasury);
    let fee_asset = policy_fee_asset(&policy);
    let tx = ivm_proved_tx(
        1,
        vec![
            transfer(&user, &fee_asset, Quantity::from(1_u64), &recipient),
            transfer(
                &user,
                &fee_asset,
                minor_units(TEST_VALIDATION_FEE_MINOR_UNITS),
                &treasury,
            ),
        ],
        metadata_for_fee_instruction(&policy, 1),
    );
    assert_eq!(
        enforce_policy(&tx, &policy),
        Ok(()),
        "the explicit signed overlay fee is exact"
    );
    assert_eq!(
        reject_ivm_proved_completed_axt_effects(1),
        Err(ValidationFeeAdmissionError::OpaqueIvmProvedAxtEffects {
            completed_envelopes: 1,
        }),
        "an exact overlay fee cannot cover opaque AXT DS effects"
    );
}
#[test]
fn typed_treasury_payout_policy_cannot_name_a_signable_treasury() {
    let mut policy = policy_with_treasury_payout_lifecycle(treasury_payout_binding(
        test_contract_address(),
        b"bound-pool",
    ));
    policy.treasury_account_id = account(7);
    assert_eq!(
        policy.policy_invariant_error(),
        Some("validation-fee treasury payout contract subject must equal the policy treasury")
    );
}
