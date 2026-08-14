#[test]
fn governance_stage_decisions_are_equal_and_mutually_exclusive() {
    let first = governance_stage_account(b"stage-decision-first");
    let second = governance_stage_account(b"stage-decision-second");
    let mut record = GovernanceStageApproval {
        epoch: 1,
        approvers: BTreeSet::new(),
        rejections: BTreeSet::new(),
        abstentions: BTreeSet::new(),
        required: 2,
        quorum_bps: 6_667,
    };
    assert!(record.record_decision(
        first.clone(),
        iroha_data_model::isi::governance::ParliamentDecision::Approve,
    ));
    assert!(record.record_decision(
        first.clone(),
        iroha_data_model::isi::governance::ParliamentDecision::Reject,
    ));
    assert!(!record.approvers.contains(&first));
    assert!(record.rejections.contains(&first));
    assert_eq!(record.rejections.len(), 1);
    assert!(record.record_decision(
        second,
        iroha_data_model::isi::governance::ParliamentDecision::Reject,
    ));
    assert!(u32::try_from(record.rejections.len()).unwrap_or(u32::MAX) >= record.required);
}
#[test]
fn governance_stage_rejection_quorum_requires_positive_threshold() {
    let mut approvals = GovernanceStageApprovals::default();
    approvals.stages.insert(
        ParliamentBody::RulesCommittee,
        GovernanceStageApproval {
            epoch: 1,
            approvers: BTreeSet::new(),
            rejections: BTreeSet::new(),
            abstentions: BTreeSet::new(),
            required: 0,
            quorum_bps: 0,
        },
    );
    assert!(approvals.quorum_met(ParliamentBody::RulesCommittee, 1));
    assert!(!approvals.rejection_quorum_met(ParliamentBody::RulesCommittee, 1));
    let rejecter = governance_stage_account(b"stage-rejection-quorum");
    let stage = approvals
        .stages
        .get_mut(&ParliamentBody::RulesCommittee)
        .expect("stage present");
    stage.required = 1;
    stage.rejections.insert(rejecter);
    assert!(approvals.rejection_quorum_met(ParliamentBody::RulesCommittee, 1));
}
fn fee_sponsor_activation_lease(
    program_id: FeeSponsorProgramId,
    revision: u64,
    expires_at_height: u64,
) -> VerifiedFeeSponsorVaultAllocation {
    let asset_definition_id = AssetDefinitionId::parse_address_literal(
        &iroha_config::parameters::defaults::nexus::fees::fee_asset_id(),
    )
    .expect("default fee asset is canonical");
    let lease_id =
        Hash::new(format!("fee-sponsor-activation-{revision}-{expires_at_height}").as_bytes());
    VerifiedFeeSponsorVaultAllocation::new(
        program_id,
        revision,
        asset_definition_id,
        Quantity::from(10_u32),
        DataSpaceId::UNIVERSAL,
        1,
        Hash::new(b"fee-sponsor-activation-source-state"),
        expires_at_height,
        lease_id,
        Hash::new(b"fee-sponsor-activation-proof"),
        *Hash::new(b"fee-sponsor-activation-statement").as_ref(),
        Hash::new(b"fee-sponsor-activation-proof-digest"),
        1,
        *Hash::new(b"fee-sponsor-activation-manifest").as_ref(),
        AxtFastpqBinding {
            parameter: "fastpq-lane-balanced".to_owned(),
            source_dsid: DataSpaceId::UNIVERSAL.as_u64(),
            source_dataspace: "universal".to_owned(),
            source_receipt_id: "fee-sponsor-activation".to_owned(),
            source_tx_commitment: "aa".repeat(32),
            claim_type: "fee_sponsor_vault_allocation".to_owned(),
            claim_digest: "bb".repeat(32),
            witness_commitment: "cc".repeat(32),
            policy_commitment: "dd".repeat(32),
            verified_effect_type: "fee_sponsor_vault_allocation".to_owned(),
            corridor: "fee-sponsor".to_owned(),
            verifier_id: "fastpq".to_owned(),
            verifier_version: "v1".to_owned(),
            target_dsids: vec![DataSpaceId::UNIVERSAL.as_u64()],
            effect_binding: None,
        },
    )
}
fn insert_fee_sponsor_activation_lease(
    world: &mut WorldTransaction<'_, '_>,
    record: VerifiedFeeSponsorVaultAllocation,
) {
    let key: StatePath = VerifiedFeeSponsorVaultAllocation::state_key_for(
        &record.program_id,
        &record.asset_definition_id,
        &record.lease_id,
    )
    .parse()
    .expect("verified allocation state key");
    let json = Json::try_new(record).expect("verified allocation JSON");
    world.smart_contract_state.insert(
        key,
        norito::to_bytes(&json).expect("verified allocation state"),
    );
}
#[test]
fn fee_sponsor_safe_activation_height_fails_closed_for_non_draining_lease() {
    let sponsor = governance_stage_account(b"fee-sponsor-never-drains");
    let program_id =
        FeeSponsorProgramId::new(sponsor, "never_drains".parse().expect("program name"));
    let record = fee_sponsor_activation_lease(program_id.clone(), 1, u64::MAX);
    let mut world = World::default();
    let key: StatePath = VerifiedFeeSponsorVaultAllocation::state_key_for(
        &record.program_id,
        &record.asset_definition_id,
        &record.lease_id,
    )
    .parse()
    .expect("verified allocation state key");
    world.smart_contract_state_mut_for_testing().insert(
        key,
        norito::to_bytes(&Json::try_new(record).expect("verified allocation JSON"))
            .expect("verified allocation state"),
    );
    let world = world.block();
    let error = fee_sponsor_revision_safe_activation_height(&world, &program_id, 2, 2, 2)
        .expect_err("u64::MAX lease must not admit a successor revision");
    assert!(error.contains("never drains"));
}
#[test]
fn fee_sponsor_safe_activation_height_preserves_later_request() {
    let sponsor = governance_stage_account(b"fee-sponsor-later-activation");
    let program_id = FeeSponsorProgramId::new(sponsor, "later".parse().expect("program name"));
    let record = fee_sponsor_activation_lease(program_id.clone(), 1, 5);
    let mut world = World::default();
    let key: StatePath = VerifiedFeeSponsorVaultAllocation::state_key_for(
        &record.program_id,
        &record.asset_definition_id,
        &record.lease_id,
    )
    .parse()
    .expect("verified allocation state key");
    world.smart_contract_state_mut_for_testing().insert(
        key,
        norito::to_bytes(&Json::try_new(record).expect("verified allocation JSON"))
            .expect("verified allocation state"),
    );
    let world = world.block();
    assert_eq!(
        fee_sponsor_revision_safe_activation_height(&world, &program_id, 2, 2, 9)
            .expect("finite lease drains"),
        9
    );
    assert_eq!(
        fee_sponsor_revision_safe_activation_height(&world, &program_id, 2, 2, 2)
            .expect("finite lease drains"),
        6
    );
}
#[test]
fn fee_sponsor_revision_activation_materializes_at_scheduled_block_height() {
    use iroha_data_model::nexus::{
        FeeSponsorEligibility, FeeSponsorProgramActivation, FeeSponsorProgramRevisionKey,
    };
    let sponsor = governance_stage_account(b"fee-sponsor-activation");
    let program_id = FeeSponsorProgramId::new(sponsor, "scheduled".parse().expect("program name"));
    let revision = FeeSponsorProgramRevision {
        program_id: program_id.clone(),
        revision: 1,
        eligibility: FeeSponsorEligibility::EnrolledOnly,
        rules: Vec::new(),
        asset_budgets: Vec::new(),
    };
    let mut program = FeeSponsorProgram::new(program_id.clone(), program_id.sponsor.clone());
    program.staged_revision = Some(1);
    program.scheduled_activation = Some(FeeSponsorProgramActivation {
        revision: 1,
        activate_at_height: 2,
    });
    let state = State::new(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let first_header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut first_block = state.block(first_header);
    {
        let mut transaction = first_block.transaction();
        transaction.world.fee_sponsor_program_revisions.insert(
            FeeSponsorProgramRevisionKey::new(program_id.clone(), 1),
            revision,
        );
        transaction
            .world
            .fee_sponsor_programs
            .insert(program_id.clone(), program);
        transaction.apply();
    }
    first_block.commit().expect("commit scheduled program");
    let second_header = BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, None, 0, 0);
    let second_block = state.block(second_header);
    let activated = second_block
        .world
        .fee_sponsor_programs
        .get(&program_id)
        .expect("scheduled program exists");
    assert_eq!(activated.active_revision, Some(1));
    assert_eq!(activated.staged_revision, None);
    assert_eq!(activated.scheduled_activation, None);
    assert_eq!(activated.lifecycle, FeeSponsorProgramLifecycle::Active);
}
#[test]
fn fee_sponsor_revision_activation_waits_for_old_lease_to_drain() {
    use iroha_data_model::nexus::{
        FeeSponsorEligibility, FeeSponsorProgramActivation, FeeSponsorProgramRevisionKey,
    };
    let sponsor = governance_stage_account(b"fee-sponsor-drain-activation");
    let program_id = FeeSponsorProgramId::new(sponsor, "drain".parse().expect("program name"));
    let revision = |revision| FeeSponsorProgramRevision {
        program_id: program_id.clone(),
        revision,
        eligibility: FeeSponsorEligibility::EnrolledOnly,
        rules: Vec::new(),
        asset_budgets: Vec::new(),
    };
    let mut program = FeeSponsorProgram::new(program_id.clone(), program_id.sponsor.clone());
    program.lifecycle = FeeSponsorProgramLifecycle::Active;
    program.active_revision = Some(1);
    program.staged_revision = Some(2);
    program.scheduled_activation = Some(FeeSponsorProgramActivation {
        revision: 2,
        activate_at_height: 2,
    });
    let state = State::new(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let first_header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut first_block = state.block(first_header);
    {
        let mut transaction = first_block.transaction();
        for revision in [revision(1), revision(2)] {
            transaction.world.fee_sponsor_program_revisions.insert(
                FeeSponsorProgramRevisionKey::new(program_id.clone(), revision.revision),
                revision,
            );
        }
        transaction
            .world
            .fee_sponsor_programs
            .insert(program_id.clone(), program);
        insert_fee_sponsor_activation_lease(
            &mut transaction.world,
            fee_sponsor_activation_lease(program_id.clone(), 1, 3),
        );
        transaction.apply();
    }
    first_block.commit().expect("commit scheduled program");
    let second_header = BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, None, 0, 0);
    let second_block = state.block(second_header);
    let deferred = second_block
        .world
        .fee_sponsor_programs
        .get(&program_id)
        .expect("scheduled program exists");
    assert_eq!(deferred.active_revision, Some(1));
    assert_eq!(
        deferred.scheduled_activation,
        Some(FeeSponsorProgramActivation {
            revision: 2,
            activate_at_height: 4,
        })
    );
    second_block.commit().expect("commit deferred activation");
    let third_header = BlockHeader::new(NonZeroU64::new(3).unwrap(), None, None, None, 0, 0);
    state
        .block(third_header)
        .commit()
        .expect("commit final lease height");
    let fourth_header = BlockHeader::new(NonZeroU64::new(4).unwrap(), None, None, None, 0, 0);
    let fourth_block = state.block(fourth_header);
    let activated = fourth_block
        .world
        .fee_sponsor_programs
        .get(&program_id)
        .expect("scheduled program exists");
    assert_eq!(activated.active_revision, Some(2));
    assert_eq!(activated.staged_revision, None);
    assert_eq!(activated.scheduled_activation, None);
}
fn governance_stage_account(seed: &[u8]) -> iroha_data_model::account::AccountId {
    iroha_data_model::account::AccountId::new(
        iroha_crypto::KeyPair::try_from_seed(seed.to_vec(), iroha_crypto::Algorithm::Ed25519)
            .expect("derive governance stage fixture key")
            .public_key()
            .clone(),
    )
}
