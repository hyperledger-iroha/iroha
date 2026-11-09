// Component coverage of the selected authority host through the production due-effect owner.

use crate::smartcontracts::isi::sorafs_authority as authority_host;

fn activate_sorafs_authority_fixture(tx: &mut StateTransaction<'_, '_>) {
    crate::governance::parliament::seed_npos_sortition_cadence_for_tests(tx, 100);
    authority_host::validate_activation_inventory(&tx.world).unwrap();
    let change = crate::runtime::draft_addition(
        &tx.world,
        *tx.network_id(),
        "iroha.sorafs.authority_governance.v1",
        tx.block_height(),
        [0x74; 32],
    )
    .unwrap();
    let applied = change
        .apply(
            tx.world.runtime_selection(),
            &crate::runtime::change_set::PublishedDefinitions(tx.world.runtime_definitions()),
        )
        .unwrap();
    for (id, descriptor) in applied.published {
        tx.world.runtime_definitions.insert(id, descriptor);
    }
    *tx.world.runtime_selection.get_mut() = applied.selection;
}

fn authority_proposal_fixture(
    tx: &StateTransaction<'_, '_>,
    authority: iroha_data_model::isi::sorafs_authority::SorafsOperatingAuthorityV1,
) -> iroha_data_model::isi::sorafs_authority::SorafsAuthorityProposalV1 {
    use iroha_data_model::isi::sorafs_authority::{
        SorafsAuthorityActionV1, SorafsAuthorityProposalV1,
    };
    SorafsAuthorityProposalV1 {
        grid: iroha_data_model::isi::sorafs_authority::SorafsAuthorityTermGridV1 {
            epoch_length_blocks: 100,
        },
        term: 1,
        network: *tx.network_id(),
        root: iroha_data_model::runtime::lifecycle::RootScopeV1::Universal,
        account: ALICE_ID.clone(),
        authority,
        action: SorafsAuthorityActionV1::Grant,
        expected_revision: 0,
        completion_policy: None,
    }
}

fn seed_due_authority_certificate(
    tx: &mut StateTransaction<'_, '_>,
    proposal: iroha_data_model::isi::sorafs_authority::SorafsAuthorityProposalV1,
) -> DueParliamentCertificateFixture {
    authority_host::validate_proposal(&proposal, tx).unwrap();
    let window = proposal.grid.window(proposal.term).unwrap();
    let created_height = if proposal.action
        == iroha_data_model::isi::sorafs_authority::SorafsAuthorityActionV1::Revoke
    {
        window.start_height
    } else {
        window.admission_height
    };
    seed_due_hosted_parliament_certificate(
        tx,
        ParliamentProposalHost::Sorafs(authority_host::SorafsAuthorityProposalRecordV1 {
            deadline_closure: None,
            proposer: ALICE_ID.clone(),
            proposal,
            created_height,
            status: crate::state::GovernanceProposalStatus::Proposed,
        }),
    )
}

#[test]
fn sorafs_authority_admission_rejects_standalone_aliases_and_executor_pruning() {
    use iroha_data_model::isi::sorafs_authority::{
        ProposeSorafsAuthorityGovernanceV1, SorafsOperatingAuthorityV1,
    };
    let state = component_state_with_crypto_policy(blank_test_state());
    let header =
        new_dummy_block_at_height(NonZeroU64::new(PARLIAMENT_DUE_CERTIFICATE_HEIGHT).unwrap());
    let mut block = state.block(header.as_ref().header());
    let mut tx = block.transaction();
    bootstrap_alice_account(&mut tx);
    activate_sorafs_authority_fixture(&mut tx);
    tx.world.citizens.insert(
        ALICE_ID.clone(),
        crate::state::CitizenshipRecord {
            owner: ALICE_ID.clone(),
            amount: tx.gov.citizenship_bond_amount.clone(),
            bonded_height: 1,
        },
    );
    let proposal = authority_proposal_fixture(&tx, SorafsOperatingAuthorityV1::SetReservePolicy);
    let selector = hex::encode(proposal.fingerprint());
    tx.world.governance_locks.insert(
        selector.clone(),
        crate::state::GovernanceLocksForReferendum::default(),
    );
    let submit = ProposeSorafsAuthorityGovernanceV1 {
        proposal: proposal.clone(),
    };
    assert!(submit.clone().execute(&ALICE_ID, &mut tx).is_err());
    assert!(
        tx.world
            .sorafs_authority_proposals
            .get(&proposal.fingerprint())
            .is_none()
    );
    tx.world.governance_locks.remove(selector.clone());
    submit.execute(&ALICE_ID, &mut tx).unwrap();
    for selector in [selector.clone(), selector.to_ascii_uppercase()] {
        assert!(typed_proposal_for_standalone_referendum(&selector, &tx).is_err());
    }
    let before = tx.world.executor_data_model.get().clone();
    assert!(
        tx.world
            .apply_executor_data_model(iroha_data_model::executor::ExecutorDataModel::default())
            .is_err()
    );
    assert_eq!(tx.world.executor_data_model.get(), &before);
}

#[test]
fn sorafs_authority_attempt_keeps_admitted_content_finishable_after_target_removal() {
    use iroha_data_model::isi::sorafs_authority::{
        CreateSorafsAuthorityParliamentAttemptV1, SorafsOperatingAuthorityV1,
    };
    let mut state = component_state_with_crypto_policy(blank_test_state());
    // Account removal must exercise the authority guard with its original
    // committed monetary policies present, just as ordinary retirement does.
    crate::smartcontracts::isi::domain::monetary_test_support::install(&mut state.world);
    let header =
        new_dummy_block_at_height(NonZeroU64::new(PARLIAMENT_DUE_CERTIFICATE_HEIGHT).unwrap());
    let mut block = state.block(header.as_ref().header());
    let mut tx = block.transaction();
    bootstrap_alice_account(&mut tx);
    activate_sorafs_authority_fixture(&mut tx);
    crate::governance::parliament::seed_npos_sortition_cadence_for_tests(&mut tx, 100);
    tx.world
        .add_account_permission(&ALICE_ID, CanManageParliament.into());
    let target = parliament_test_account(2);
    Register::account(Account::new(target.clone()))
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
    let mut proposal =
        authority_proposal_fixture(&tx, SorafsOperatingAuthorityV1::SetReservePolicy);
    proposal.account = target.clone();
    authority_host::validate_proposal(&proposal, &tx).unwrap();
    tx.world.sorafs_authority_proposals.insert(
        proposal.fingerprint(),
        authority_host::SorafsAuthorityProposalRecordV1 {
            deadline_closure: None,
            proposer: ALICE_ID.clone(),
            proposal: proposal.clone(),
            created_height: 1,
            status: crate::state::GovernanceProposalStatus::Proposed,
        },
    );
    Unregister::account(target)
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
    assert!(authority_host::validate_proposal(&proposal, &tx).is_err());
    let create = CreateSorafsAuthorityParliamentAttemptV1 {
        proposal: proposal.clone(),
        attempt_sequence: 0,
    };
    let id = create.governance_attempt_id();
    create.execute(&ALICE_ID, &mut tx).unwrap();
    let attempt = tx.world.parliament_attempts.get(&id).unwrap();
    assert_eq!(
        attempt.expected_head(),
        authority_host::admitted_expected_head(&proposal)
    );
    assert_ne!(
        attempt.expected_head(),
        authority_host::expected_head(&proposal, &tx.world)
    );
    ParliamentProposalHost::resolve(&tx.world, proposal.fingerprint())
        .unwrap()
        .validate_bindings(attempt)
        .unwrap();
}

// These explicit component heights commit World overlays, not fabricated finalized blocks.
fn with_authority_component_height<R>(
    state: &State,
    height: u64,
    run: impl FnOnce(&mut StateTransaction<'_, '_>) -> R,
) -> R {
    let header = new_dummy_block_at_height(NonZeroU64::new(height).unwrap());
    let mut block = state.block(header.as_ref().header());
    let mut tx = block.transaction();
    let result = run(&mut tx);
    tx.apply();
    block.commit_world_overlay_for_testing().unwrap();
    result
}

fn enact_authority_decision(
    tx: &mut StateTransaction<'_, '_>,
    proposal: iroha_data_model::isi::sorafs_authority::SorafsAuthorityProposalV1,
) -> DueParliamentCertificateFixture {
    let fixture = seed_due_authority_certificate(tx, proposal);
    assert_eq!(
        execute_due_parliament_certificate_v1(fixture.governance_attempt_id, tx).unwrap(),
        DueParliamentCertificateExecutionV1::Applied
    );
    assert_eq!(
        tx.world
            .sorafs_authority_proposals
            .get(&fixture.proposal_id)
            .unwrap()
            .status,
        crate::state::GovernanceProposalStatus::Enacted
    );
    assert_eq!(
        tx.world
            .parliament_attempts
            .get(&fixture.governance_attempt_id)
            .unwrap()
            .attempt()
            .status,
        GovernanceAttemptStatusV1::Enacted
    );
    assert!(
        execute_due_parliament_certificate_v1(fixture.governance_attempt_id, tx).is_err(),
        "an enacted authorization cannot be consumed twice"
    );
    fixture
}

fn approve_authority_pair(
    tx: &mut StateTransaction<'_, '_>,
    plan: &iroha_data_model::isi::sorafs_authority::SorafsAuthorityProposalV1,
    terminal_first: bool,
) {
    use iroha_data_model::isi::sorafs_authority::SorafsAuthorityActionV1;
    let mut terminal = plan.clone();
    terminal.action = SorafsAuthorityActionV1::AuthorizeTerminalRevoke;
    let pair = if terminal_first {
        [terminal, plan.clone()]
    } else {
        [plan.clone(), terminal]
    };
    for proposal in pair {
        enact_authority_decision(tx, proposal);
    }
    let lease = tx
        .world
        .sorafs_authority_leases
        .get(&plan.plan_id())
        .unwrap();
    assert!(lease.grant_approval.is_some() && lease.terminal_approval.is_some());
    assert!(
        lease.activation.is_none(),
        "approvals do not directly install authority"
    );
}

fn assert_authority_projection_valid(tx: &StateTransaction<'_, '_>) {
    authority_host::validate_projection(
        &tx.world,
        tx.network_id(),
        tx.world.sccp_committee_generations.allocation_budget(),
    )
    .unwrap();
}

#[test]
fn parliament_sorafs_authority_grant_revoke_regrant_preserves_receipts_and_tombstones() {
    use iroha_data_model::{
        isi::sorafs_authority::{SorafsAuthorityActionV1, SorafsOperatingAuthorityV1},
        sorafs::{
            capacity::ProviderId,
            pin_registry::{
                ProviderIngestCompletionAuthorityV1, ProviderIngestCompletionSignerPolicyV1,
            },
        },
    };
    let state = component_state_with_crypto_policy(blank_test_state());
    let plans = with_authority_component_height(&state, PARLIAMENT_DUE_CERTIFICATE_HEIGHT, |tx| {
        bootstrap_alice_account(tx);
        activate_sorafs_authority_fixture(tx);
        let provider_id = ProviderId::new([0x86; 32]);
        let policy = ProviderIngestCompletionAuthorityV1::new(
            ALICE_ID.clone(),
            ALICE_ID.clone(),
            ProviderIngestCompletionSignerPolicyV1 {
                policy_id: [0x87; 32],
                revision: 1,
                predecessor_digest: None,
                policy_digest: [0x88; 32],
            },
        );
        tx.world
            .provider_owners
            .insert(provider_id, ALICE_ID.clone());
        tx.world
            .provider_ingest_completion_authorities
            .insert(provider_id, policy.clone());
        let mut plans = Vec::new();
        for (index, authority) in [
            SorafsOperatingAuthorityV1::CompleteReplicationOrder { provider_id },
            SorafsOperatingAuthorityV1::SetReservePolicy,
            SorafsOperatingAuthorityV1::UpsertProviderCredit,
            SorafsOperatingAuthorityV1::GatewayComplianceOperator,
        ]
        .into_iter()
        .enumerate()
        {
            let mut plan = authority_proposal_fixture(tx, authority);
            if matches!(
                authority,
                SorafsOperatingAuthorityV1::CompleteReplicationOrder { .. }
            ) {
                plan.completion_policy = Some(policy.clone());
            }
            approve_authority_pair(tx, &plan, index % 2 == 0);
            assert!(
                tx.world
                    .sorafs_authority_heads
                    .get(&plan.governed_subject_id_v1())
                    .is_none()
            );
            plans.push(plan);
        }
        assert!(
            !crate::runtime::obligations::LiveObligations::of(&tx.world).is_empty(),
            "dormant irrevocable mandates retain their completion owner"
        );
        assert_authority_projection_valid(tx);
        plans
    });
    let first_window = plans[0].grid.window(1).unwrap();
    with_authority_component_height(&state, first_window.start_height, |tx| {
        authority_host::advance_terms(tx).unwrap();
        for plan in &plans {
            let lease = tx
                .world
                .sorafs_authority_leases
                .get(&plan.plan_id())
                .unwrap();
            let head = tx
                .world
                .sorafs_authority_heads
                .get(&plan.governed_subject_id_v1())
                .unwrap();
            assert!(head.granted);
            assert_eq!(head.revision, 1);
            assert_eq!(head.plan_id, plan.plan_id());
            assert_eq!(head.enacted_at_height, first_window.start_height);
            assert_eq!(
                head.certificate_id,
                lease.grant_approval.as_ref().unwrap().certificate_id
            );
            assert!(lease.activation.is_some());
        }
        assert_authority_projection_valid(tx);
    });
    with_authority_component_height(&state, first_window.start_height + 100, |tx| {
        for plan in &plans {
            let mut revoke = plan.clone();
            revoke.action = SorafsAuthorityActionV1::Revoke;
            let fixture = enact_authority_decision(tx, revoke);
            let head = tx
                .world
                .sorafs_authority_heads
                .get(&plan.governed_subject_id_v1())
                .unwrap();
            assert!(!head.granted);
            assert_eq!(head.revision, 2);
            assert_eq!(
                head.certificate_id,
                GovernanceCertificateId::derive_v1(&fixture.certificate)
            );
            let mut same_term = plan.clone();
            same_term.expected_revision = 2;
            assert!(
                authority_host::validate_proposal(&same_term, tx).is_err(),
                "same-term regrant cannot create a fresh exposure or budget"
            );
        }
        assert_authority_projection_valid(tx);
    });
    let next_plans =
        with_authority_component_height(&state, first_window.start_height + 101, |tx| {
            plans
                .iter()
                .map(|plan| {
                    let mut next = plan.clone();
                    next.term = 2;
                    next.expected_revision = 2;
                    approve_authority_pair(tx, &next, false);
                    next
                })
                .collect::<Vec<_>>()
        });
    with_authority_component_height(&state, first_window.end_height, |tx| {
        authority_host::advance_terms(tx).unwrap();
        // Old expiries cannot revoke a later incarnation after an early tombstone.
        authority_host::advance_terms(tx).unwrap();
        for plan in &next_plans {
            let head = tx
                .world
                .sorafs_authority_heads
                .get(&plan.governed_subject_id_v1())
                .unwrap();
            assert!(head.granted);
            assert_eq!(head.revision, 3);
            assert_eq!(head.plan_id, plan.plan_id());
        }
        assert_authority_projection_valid(tx);
    });
    with_authority_component_height(
        &state,
        next_plans[0].grid.window(2).unwrap().end_height,
        |tx| {
            authority_host::advance_terms(tx).unwrap();
            for plan in &next_plans {
                let lease = tx
                    .world
                    .sorafs_authority_leases
                    .get(&plan.plan_id())
                    .unwrap();
                let head = tx
                    .world
                    .sorafs_authority_heads
                    .get(&plan.governed_subject_id_v1())
                    .unwrap();
                assert!(!head.granted);
                assert_eq!(head.revision, 4);
                assert_eq!(
                    head.certificate_id,
                    lease.terminal_approval.as_ref().unwrap().certificate_id
                );
                assert_ne!(
                    head.enacted_at_height,
                    lease.terminal_approval.as_ref().unwrap().approved_at_height
                );
            }
            authority_host::advance_terms(tx).unwrap();
            assert!(crate::runtime::obligations::LiveObligations::of(&tx.world).is_empty());
            assert_authority_projection_valid(tx);
            let retained_id = plans[0].fingerprint();
            let mut retired = crate::runtime::founded_selection().unwrap();
            retired.installed_storage = tx.world.runtime_selection().installed_storage.clone();
            retired
                .check_closure(&crate::runtime::change_set::PublishedDefinitions(
                    tx.world.runtime_definitions(),
                ))
                .unwrap();
            *tx.world.runtime_selection.get_mut() = retired;
            assert_authority_projection_valid(tx);
            assert!(ParliamentProposalHost::resolve(&tx.world, retained_id).is_err());
            assert!(ParliamentProposalHost::resolve_retained(&tx.world, retained_id).is_ok());
            assert!(
                typed_proposal_for_standalone_referendum(&hex::encode(retained_id), tx)
                    .unwrap()
                    .is_none()
            );
            assert!(
                authority_host::validate_activation_inventory(&tx.world).is_err(),
                "retirement cannot erase retained lease history"
            );
        },
    );
}

#[test]
fn parliament_sorafs_authority_missing_mismatched_or_stale_policy_pair_cannot_start() {
    use iroha_data_model::{
        isi::sorafs_authority::{SorafsAuthorityActionV1, SorafsOperatingAuthorityV1},
        sorafs::{
            capacity::ProviderId,
            pin_registry::{
                ProviderIngestCompletionAuthorityV1, ProviderIngestCompletionSignerPolicyV1,
            },
        },
    };
    for case in 0..4 {
        let state = component_state_with_crypto_policy(blank_test_state());
        let plan =
            with_authority_component_height(&state, PARLIAMENT_DUE_CERTIFICATE_HEIGHT, |tx| {
                bootstrap_alice_account(tx);
                activate_sorafs_authority_fixture(tx);
                let provider_id = ProviderId::new([0x86; 32]);
                let policy = ProviderIngestCompletionAuthorityV1::new(
                    ALICE_ID.clone(),
                    ALICE_ID.clone(),
                    ProviderIngestCompletionSignerPolicyV1 {
                        policy_id: [0x87; 32],
                        revision: 1,
                        predecessor_digest: None,
                        policy_digest: [0x88; 32],
                    },
                );
                tx.world
                    .provider_owners
                    .insert(provider_id, ALICE_ID.clone());
                tx.world
                    .provider_ingest_completion_authorities
                    .insert(provider_id, policy.clone());
                let mut plan = authority_proposal_fixture(
                    tx,
                    SorafsOperatingAuthorityV1::CompleteReplicationOrder { provider_id },
                );
                plan.completion_policy = Some(policy.clone());
                if case != 1 {
                    enact_authority_decision(tx, plan.clone());
                }
                if case != 0 {
                    let mut terminal = plan.clone();
                    terminal.action = SorafsAuthorityActionV1::AuthorizeTerminalRevoke;
                    if case == 2 {
                        let mut corrected = policy.clone();
                        corrected.signer_policy.policy_digest = [0x99; 32];
                        tx.world
                            .provider_ingest_completion_authorities
                            .insert(provider_id, corrected.clone());
                        terminal.completion_policy = Some(corrected);
                        assert_eq!(terminal.family_id(), plan.family_id());
                        assert_ne!(terminal.plan_id(), plan.plan_id());
                    }
                    enact_authority_decision(tx, terminal);
                }
                if case == 3 {
                    let mut changed = policy;
                    changed.signer_policy.policy_digest = [0x99; 32];
                    tx.world
                        .provider_ingest_completion_authorities
                        .insert(provider_id, changed);
                }
                plan
            });
        with_authority_component_height(&state, plan.grid.window(1).unwrap().start_height, |tx| {
            authority_host::advance_terms(tx).unwrap();
            assert!(
                tx.world
                    .sorafs_authority_heads
                    .get(&plan.governed_subject_id_v1())
                    .is_none()
            );
            for (_, lease) in tx.world.sorafs_authority_leases.iter() {
                assert!(lease.start_processed);
                assert!(lease.activation.is_none());
            }
            authority_host::advance_terms(tx).unwrap();
            assert_authority_projection_valid(tx);
        });
    }
}

#[test]
fn parliament_sorafs_authority_competing_certificate_is_superseded_after_revision_change() {
    use iroha_data_model::isi::sorafs_authority::SorafsOperatingAuthorityV1;
    let state = component_state_with_crypto_policy(blank_test_state());
    let header =
        new_dummy_block_at_height(NonZeroU64::new(PARLIAMENT_DUE_CERTIFICATE_HEIGHT).unwrap());
    let mut block = state.block(header.as_ref().header());
    let mut tx = block.transaction();
    bootstrap_alice_account(&mut tx);
    activate_sorafs_authority_fixture(&mut tx);
    let proposal = authority_proposal_fixture(&tx, SorafsOperatingAuthorityV1::SetReservePolicy);
    let fixture = seed_due_authority_certificate(&mut tx, proposal.clone());
    // A different committed subject head invalidates the captured precondition even
    // when the projection has returned to its original ungranted state (ABA).
    tx.world.sorafs_authority_heads.insert(
        proposal.governed_subject_id_v1(),
        authority_host::SorafsAuthorityHeadV1 {
            plan_id: proposal.plan_id(),
            term: proposal.term,
            expires_at_height: proposal.grid.window(proposal.term).unwrap().end_height,
            authority: proposal.authority,
            account: proposal.account.clone(),
            revision: 2,
            granted: false,
            proposal_content_id: fixture.certificate.proposal_content_id,
            governance_attempt_id: fixture.governance_attempt_id,
            certificate_id: GovernanceCertificateId::derive_v1(&fixture.certificate),
            enacted_at_height: PARLIAMENT_DUE_CERTIFICATE_HEIGHT - 1,
        },
    );
    assert_eq!(
        execute_due_parliament_certificate_v1(fixture.governance_attempt_id, &mut tx).unwrap(),
        DueParliamentCertificateExecutionV1::Applied
    );
    assert_eq!(
        tx.world
            .sorafs_authority_proposals
            .get(&fixture.proposal_id)
            .unwrap()
            .status,
        crate::state::GovernanceProposalStatus::Superseded
    );
    assert_eq!(
        tx.world
            .sorafs_authority_heads
            .get(&proposal.governed_subject_id_v1())
            .unwrap()
            .revision,
        2
    );
    assert!(!tx.world.account_contains_inherent_permission(
        &ALICE_ID,
        &iroha_executor_data_model::permission::sorafs::CanSetSorafsReservePolicy.into()
    ));
}

#[test]
fn parliament_sorafs_authority_due_effect_overlay_rolls_back_all_projection_and_receipt_writes() {
    use iroha_data_model::isi::sorafs_authority::SorafsOperatingAuthorityV1;
    let state = component_state_with_crypto_policy(blank_test_state());
    let header =
        new_dummy_block_at_height(NonZeroU64::new(PARLIAMENT_DUE_CERTIFICATE_HEIGHT).unwrap());
    let mut block = state.block(header.as_ref().header());
    let fixture = {
        let mut tx = block.transaction();
        bootstrap_alice_account(&mut tx);
        activate_sorafs_authority_fixture(&mut tx);
        let proposal =
            authority_proposal_fixture(&tx, SorafsOperatingAuthorityV1::GatewayComplianceOperator);
        let fixture = seed_due_authority_certificate(&mut tx, proposal);
        tx.apply();
        fixture
    };
    let original = crate::snapshot::canonical_staged_state_snapshot_bytes(&block);
    let events = block.world.pending_external_events().to_vec();
    {
        let mut tx = block.transaction();
        assert_eq!(
            execute_due_parliament_certificate_v1(fixture.governance_attempt_id, &mut tx).unwrap(),
            DueParliamentCertificateExecutionV1::Applied
        );
        assert_eq!(tx.world.sorafs_authority_heads.iter().count(), 0);
        assert_eq!(tx.world.sorafs_authority_leases.iter().count(), 1);
        // The block owner may abandon this isolated candidate after a later local refusal.
    }
    assert_eq!(
        crate::snapshot::canonical_staged_state_snapshot_bytes(&block),
        original
    );
    assert_eq!(block.world.pending_external_events(), events.as_slice());
    let mut retry = block.transaction();
    assert_eq!(
        execute_due_parliament_certificate_v1(fixture.governance_attempt_id, &mut retry).unwrap(),
        DueParliamentCertificateExecutionV1::Applied
    );
    authority_host::validate_projection(
        &retry.world,
        retry.network_id(),
        retry.world.sccp_committee_generations.allocation_budget(),
    )
    .unwrap();
}

#[test]
fn parliament_sorafs_authority_start_overlay_rolls_back_projection_and_execution_receipt() {
    use iroha_data_model::isi::sorafs_authority::SorafsOperatingAuthorityV1;
    let state = component_state_with_crypto_policy(blank_test_state());
    let plan = with_authority_component_height(&state, PARLIAMENT_DUE_CERTIFICATE_HEIGHT, |tx| {
        bootstrap_alice_account(tx);
        activate_sorafs_authority_fixture(tx);
        let plan =
            authority_proposal_fixture(tx, SorafsOperatingAuthorityV1::GatewayComplianceOperator);
        approve_authority_pair(tx, &plan, false);
        plan
    });
    let header = new_dummy_block_at_height(
        NonZeroU64::new(plan.grid.window(plan.term).unwrap().start_height).unwrap(),
    );
    let mut block = state.block(header.as_ref().header());
    let original = crate::snapshot::canonical_staged_state_snapshot_bytes(&block);
    let events = block.world.pending_external_events().to_vec();
    {
        let mut tx = block.transaction();
        authority_host::advance_terms(&mut tx).unwrap();
        assert!(
            tx.world
                .sorafs_authority_heads
                .get(&plan.governed_subject_id_v1())
                .unwrap()
                .granted
        );
        assert!(
            tx.world
                .sorafs_authority_leases
                .get(&plan.plan_id())
                .unwrap()
                .activation
                .is_some()
        );
        // Abandon the entire isolated projection candidate after a later local refusal.
    }
    assert_eq!(
        crate::snapshot::canonical_staged_state_snapshot_bytes(&block),
        original
    );
    assert_eq!(block.world.pending_external_events(), events.as_slice());
    let mut retry = block.transaction();
    authority_host::advance_terms(&mut retry).unwrap();
    assert_authority_projection_valid(&retry);
}

#[test]
fn parliament_sorafs_authority_deadline_closes_pending_and_unattempted_without_rewriting_reducer() {
    use iroha_data_model::isi::sorafs_authority::SorafsOperatingAuthorityV1;
    let mut state = component_state_with_crypto_policy(blank_test_state());
    let (plan, unattempted, attempt_id, original, members) = with_authority_component_height(
        &state,
        PARLIAMENT_DUE_CERTIFICATE_HEIGHT,
        |tx| {
            bootstrap_alice_account(tx);
            activate_sorafs_authority_fixture(tx);
            let provider_id = iroha_data_model::sorafs::capacity::ProviderId::new([0x86; 32]);
            let policy = iroha_data_model::sorafs::pin_registry::ProviderIngestCompletionAuthorityV1::new(ALICE_ID.clone(), ALICE_ID.clone(), iroha_data_model::sorafs::pin_registry::ProviderIngestCompletionSignerPolicyV1 { policy_id: [0x87; 32], revision: 1, predecessor_digest: None, policy_digest: [0x88; 32] });
            tx.world
                .provider_owners
                .insert(provider_id, ALICE_ID.clone());
            tx.world
                .provider_ingest_completion_authorities
                .insert(provider_id, policy.clone());
            let mut plan = authority_proposal_fixture(
                tx,
                SorafsOperatingAuthorityV1::CompleteReplicationOrder { provider_id },
            );
            plan.completion_policy = Some(policy);
            let unattempted =
                authority_proposal_fixture(tx, SorafsOperatingAuthorityV1::UpsertProviderCredit);
            for proposal in [&plan, &unattempted] {
                tx.world.sorafs_authority_proposals.insert(
                    proposal.fingerprint(),
                    authority_host::SorafsAuthorityProposalRecordV1 {
                        deadline_closure: None,
                        proposer: ALICE_ID.clone(),
                        proposal: proposal.clone(),
                        created_height: 1,
                        status: crate::state::GovernanceProposalStatus::Proposed,
                    },
                );
            }
            let content = plan.proposal_content_id();
            let attempt_id = GovernanceAttemptId::derive_v1(content, 0);
            let (risk_tier, requirements) =
                crate::governance::parliament::sorafs_authority_attempt_policy_v1();
            let mut attempt = crate::governance::parliament::ParliamentAttemptStateV1::try_new(
                GovernanceAttemptV1 {
                    id: attempt_id,
                    proposal_content_id: content,
                    sequence: 0,
                    risk_tier,
                    stage: GovernanceStageV1::Qualification,
                    status: GovernanceAttemptStatusV1::Active,
                },
                PARLIAMENT_GOVERNANCE_POLICY_VERSION_V1,
                10,
                21,
                plan.effect_preimage_hash_v1(),
                authority_host::admitted_expected_head(&plan),
                requirements.clone(),
            )
            .unwrap();
            attempt.complete_qualification(attempt_id).unwrap();
            let candidates = parliament_test_candidates();
            for requirement in requirements {
                let request = SortitionRequestV1::try_new_canonical(
                    attempt_id,
                    BodyElectionAttemptId::derive_v1(attempt_id, requirement.body, 0),
                    requirement.body,
                    parliament_candidate_root_v1(attempt_id, requirement.body, &candidates),
                    u32::try_from(candidates.len()).unwrap(),
                    3,
                    10,
                    20,
                    BeaconSessionId::new(parliament_test_root(0xB0)),
                    None,
                )
                .unwrap();
                attempt
                    .register_sortition_request(attempt_id, 0, request, candidates.clone())
                    .unwrap();
            }
            let members = attempt.parliament_member_reference_sets_v1().1;
            assert!(!members.is_empty());
            let original = norito::core::to_bytes(&attempt).unwrap();
            tx.world.put_parliament_attempt(attempt).unwrap();
            // Seventeen valid policy corrections reserve the complete family role budget.
            // They keep the same subject/term, and none is allowed a new draw afterwards.
            for tag in 1_u8..17 {
                let mut corrected = plan.clone();
                corrected
                    .completion_policy
                    .as_mut()
                    .unwrap()
                    .signer_policy
                    .policy_digest = [tag; 32];
                tx.world
                    .provider_ingest_completion_authorities
                    .insert(provider_id, corrected.completion_policy.clone().unwrap());
                authority_host::validate_proposal(&corrected, tx).unwrap();
                tx.world.sorafs_authority_proposals.insert(
                    corrected.fingerprint(),
                    authority_host::SorafsAuthorityProposalRecordV1 {
                        deadline_closure: None,
                        proposer: ALICE_ID.clone(),
                        proposal: corrected.clone(),
                        created_height: 1,
                        status: crate::state::GovernanceProposalStatus::Proposed,
                    },
                );
                let (risk_tier, bodies) =
                    crate::governance::parliament::sorafs_authority_attempt_policy_v1();
                let other = crate::governance::parliament::ParliamentAttemptStateV1::try_new(
                    GovernanceAttemptV1 {
                        id: GovernanceAttemptId::derive_v1(corrected.proposal_content_id(), 0),
                        proposal_content_id: corrected.proposal_content_id(),
                        sequence: 0,
                        risk_tier,
                        stage: GovernanceStageV1::Qualification,
                        status: GovernanceAttemptStatusV1::Active,
                    },
                    PARLIAMENT_GOVERNANCE_POLICY_VERSION_V1,
                    10,
                    21,
                    corrected.effect_preimage_hash_v1(),
                    authority_host::admitted_expected_head(&corrected),
                    bodies,
                )
                .unwrap();
                tx.world.put_parliament_attempt(other).unwrap();
            }
            assert_eq!(
                authority_host::family_randomness_units(&tx.world, &plan, None).unwrap(),
                17
            );

            assert!(
                tx.world
                    .parliament_required_beacon_pulse_slots
                    .iter()
                    .next()
                    .is_some()
            );
            (plan, unattempted, attempt_id, original, members)
        },
    );
    let cutoff = plan.grid.window(plan.term).unwrap().cutoff_height;
    with_authority_component_height(&state, cutoff, |tx| {
        authority_host::close_expired_proposals(tx).unwrap();
        authority_host::close_expired_proposals(tx).unwrap();
        for proposal in [&plan, &unattempted] {
            let record = tx
                .world
                .sorafs_authority_proposals
                .get(&proposal.fingerprint())
                .unwrap();
            assert_eq!(record.deadline_closure.unwrap().closed_at_height, cutoff);
            assert_eq!(
                record.status,
                crate::state::GovernanceProposalStatus::Proposed,
                "closure is not a forged shared-reducer rejection"
            );
            assert!(ParliamentProposalHost::resolve(&tx.world, proposal.fingerprint()).is_err());
            assert!(
                ParliamentProposalHost::resolve_retained(&tx.world, proposal.fingerprint()).is_ok()
            );
        }
        assert_eq!(
            norito::core::to_bytes(tx.world.parliament_attempts.get(&attempt_id).unwrap()).unwrap(),
            original
        );
        assert!(
            tx.world
                .parliament_required_beacon_pulse_slots
                .iter()
                .next()
                .is_none()
        );
        for member in &members {
            let row = tx
                .world
                .parliament_member_reference_counts
                .get(member)
                .unwrap();
            assert!(row.has_references());
            assert!(!row.retains_citizenship_bond());
        }
        let immutable = tx
            .world
            .parliament_attempts
            .get(&attempt_id)
            .unwrap()
            .clone();
        assert!(tx.world.put_parliament_attempt(immutable).is_err());
        assert!(crate::runtime::obligations::LiveObligations::of(&tx.world).is_empty());
        assert!(tx.world.sorafs_authority_heads.iter().next().is_none());
        let mut retired = crate::runtime::founded_selection().unwrap();
        retired.installed_storage = tx.world.runtime_selection().installed_storage.clone();
        *tx.world.runtime_selection.get_mut() = retired;
        assert_authority_projection_valid(tx);
    });
    // Rebuilding derived current+undo indexes must preserve the closure mask.
    // This is an explicit component observation; no consensus finality is asserted.
    state
        .world
        .rebuild_governance_read_indexes_for_testing()
        .unwrap();
    {
        let world = state.world_view();
        assert!(
            world
                .parliament_required_beacon_pulse_slots
                .iter()
                .next()
                .is_none()
        );
        for member in &members {
            let row = world
                .parliament_member_reference_counts
                .get(member)
                .unwrap();
            assert!(row.has_references());
            assert!(!row.retains_citizenship_bond());
        }
    }
    let view = state.view();
    let (_, _, plan) = crate::governance::parliament::plan_parliament_attempt_v1(
        &view,
        &state.network_id,
        &state.gov,
        attempt_id,
    )
    .unwrap();
    assert!(
        plan.due.is_empty()
            && plan.exact.is_empty()
            && plan.relay_ballots.is_empty()
            && plan.finalize_ballots.is_empty()
    );
}

#[test]
fn parliament_sorafs_authority_exhausted_early_revoke_cannot_block_voted_expiry() {
    use iroha_data_model::isi::sorafs_authority::{
        CreateSorafsAuthorityParliamentAttemptV1, SorafsAuthorityActionV1,
        SorafsOperatingAuthorityV1,
    };
    use iroha_executor_data_model::permission::sorafs::CanSetSorafsReservePolicy;
    let state = component_state_with_crypto_policy(blank_test_state());
    let plan = with_authority_component_height(&state, PARLIAMENT_DUE_CERTIFICATE_HEIGHT, |tx| {
        bootstrap_alice_account(tx);
        activate_sorafs_authority_fixture(tx);
        tx.world
            .add_account_permission(&ALICE_ID, CanManageParliament.into());
        let plan = authority_proposal_fixture(tx, SorafsOperatingAuthorityV1::SetReservePolicy);
        approve_authority_pair(tx, &plan, false);
        let original = tx
            .world
            .sorafs_authority_leases
            .get(&plan.plan_id())
            .unwrap()
            .clone();
        let mut orphan = original.clone();
        orphan.grant_approval = None;
        orphan.terminal_approval = None;
        tx.world
            .sorafs_authority_leases
            .insert(plan.plan_id(), orphan);
        assert!(
            authority_host::validate_projection(
                &tx.world,
                tx.network_id(),
                tx.world.sccp_committee_generations.allocation_budget()
            )
            .is_err(),
            "a lease cannot exist without an enacted approval"
        );
        tx.world
            .sorafs_authority_leases
            .insert(plan.plan_id(), original);
        plan
    });
    let window = plan.grid.window(plan.term).unwrap();
    with_authority_component_height(&state, window.start_height, |tx| {
        authority_host::advance_terms(tx).unwrap();
        assert!(
            tx.world
                .sorafs_authority_heads
                .get(&plan.governed_subject_id_v1())
                .unwrap()
                .granted
        );
    });
    let mut revoke = plan.clone();
    revoke.action = SorafsAuthorityActionV1::Revoke;
    let exhausted =
        authority_host::test_support::exhausted_attempt(&revoke, window.start_height + 1);
    let exhausted_bytes = norito::to_bytes(&exhausted).unwrap();
    let attempt_id = exhausted.attempt().id;
    with_authority_component_height(&state, window.end_height - 1, |tx| {
        authority_host::validate_proposal(&revoke, tx).unwrap();
        exhausted
            .validate_restored_height_v1(tx.block_height())
            .unwrap();
        tx.world.sorafs_authority_proposals.insert(
            revoke.fingerprint(),
            authority_host::SorafsAuthorityProposalRecordV1 {
                proposal: revoke.clone(),
                proposer: ALICE_ID.clone(),
                created_height: window.start_height,
                status: crate::state::GovernanceProposalStatus::Rejected,
                deadline_closure: None,
            },
        );
        tx.world.put_parliament_attempt(exhausted).unwrap();
        let error = CreateSorafsAuthorityParliamentAttemptV1 {
            proposal: revoke.clone(),
            attempt_sequence: 1,
        }
        .execute(&ALICE_ID, tx)
        .unwrap_err();
        assert!(
            matches!(
                &error,
                iroha_data_model::isi::error::InstructionExecutionError::InvalidParameter(
                    iroha_data_model::isi::error::InvalidParameterError::SmartContract(reason)
                ) if reason.contains("redraw budget exhausted")
            ),
            "{error:?}"
        );
        assert_eq!(
            authority_host::family_randomness_units(&tx.world, &revoke, None).unwrap(),
            17
        );
        assert!(
            tx.world
                .sorafs_authority_heads
                .get(&plan.governed_subject_id_v1())
                .unwrap()
                .granted
        );
        assert_authority_projection_valid(tx);
    });
    with_authority_component_height(&state, window.end_height, |tx| {
        authority_host::advance_terms(tx).unwrap();
        let lease = tx
            .world
            .sorafs_authority_leases
            .get(&plan.plan_id())
            .unwrap();
        let completion = lease.completion.as_ref().unwrap();
        assert_eq!(completion.at_height, window.end_height);
        assert_eq!(Some(completion.approval), lease.terminal_approval);
        let head = tx
            .world
            .sorafs_authority_heads
            .get(&plan.governed_subject_id_v1())
            .unwrap();
        assert!(!head.granted);
        assert_eq!(head.revision, 2);
        assert!(
            !tx.world
                .account_contains_inherent_permission(&ALICE_ID, &CanSetSorafsReservePolicy.into())
        );
        assert_eq!(
            norito::to_bytes(tx.world.parliament_attempts.get(&attempt_id).unwrap()).unwrap(),
            exhausted_bytes
        );
        assert_eq!(
            authority_host::family_randomness_units(&tx.world, &revoke, None).unwrap(),
            17
        );
        assert!(crate::runtime::obligations::LiveObligations::of(&tx.world).is_empty());
        assert_authority_projection_valid(tx);
    });
}

#[test]
fn parliament_sorafs_authority_closed_future_certificate_keeps_its_original_restore_horizon() {
    use iroha_data_model::isi::sorafs_authority::SorafsOperatingAuthorityV1;
    let state = component_state_with_crypto_policy(blank_test_state());
    let (plan, fixture, original) =
        with_authority_component_height(&state, PARLIAMENT_DUE_CERTIFICATE_HEIGHT, |tx| {
            bootstrap_alice_account(tx);
            activate_sorafs_authority_fixture(tx);
            let plan = authority_proposal_fixture(tx, SorafsOperatingAuthorityV1::SetReservePolicy);
            let enact = plan.grid.window(plan.term).unwrap().cutoff_height + 10;
            let fixture = seed_hosted_parliament_certificate_at(
                tx,
                ParliamentProposalHost::Sorafs(authority_host::SorafsAuthorityProposalRecordV1 {
                    deadline_closure: None,
                    proposer: ALICE_ID.clone(),
                    proposal: plan.clone(),
                    created_height: 1,
                    status: crate::state::GovernanceProposalStatus::Proposed,
                }),
                enact,
            );
            let original = norito::to_bytes(
                tx.world
                    .parliament_attempts
                    .get(&fixture.governance_attempt_id)
                    .unwrap(),
            )
            .unwrap();
            (plan, fixture, original)
        });
    let cutoff = plan.grid.window(plan.term).unwrap().cutoff_height;
    with_authority_component_height(&state, cutoff, |tx| {
        authority_host::close_expired_proposals(tx).unwrap();
        assert!(
            tx.world
                .parliament_certified_enactments
                .iter()
                .next()
                .is_none()
        );
    });
    with_authority_component_height(&state, fixture.certificate.enact_at_height + 1, |tx| {
        let attempt = tx
            .world
            .parliament_attempts
            .get(&fixture.governance_attempt_id)
            .unwrap();
        assert!(
            attempt
                .validate_restored_height_v1(tx.block_height())
                .is_err()
        );
        let horizon = authority_host::retained_attempt_validation_height(
            &tx.world,
            plan.proposal_content_id(),
            tx.block_height(),
        );
        assert_eq!(horizon, cutoff - 1);
        attempt.validate_restored_height_v1(horizon).unwrap();
        assert_eq!(norito::to_bytes(attempt).unwrap(), original);
        assert_eq!(
            attempt.attempt().status,
            GovernanceAttemptStatusV1::Certified
        );
        assert!(tx.world.sorafs_authority_heads.iter().next().is_none());
        assert_authority_projection_valid(tx);
    });
}

#[test]
fn parliament_sorafs_authority_scheduled_start_obeys_committed_restrictions() {
    use iroha_data_model::{
        isi::sorafs_authority::SorafsOperatingAuthorityV1,
        runtime::definition::{RestrictionEntryV1, RestrictionV1},
    };

    // The dormant mandate is admitted work, but installing its authority adds exposure.
    // Future restrictions must not affect this height, and a skipped start never retries.
    for (restrict_interface, restriction, grants) in [
        (true, RestrictionV1::AdmissionClosed(6_401), true),
        (false, RestrictionV1::AdmissionClosed(6_401), true),
        (false, RestrictionV1::CompletionOnly(6_401), false),
        (false, RestrictionV1::CompletionOnly(6_402), true),
    ] {
        let state = component_state_with_crypto_policy(blank_test_state());
        let plan =
            with_authority_component_height(&state, PARLIAMENT_DUE_CERTIFICATE_HEIGHT, |tx| {
                bootstrap_alice_account(tx);
                activate_sorafs_authority_fixture(tx);
                let plan =
                    authority_proposal_fixture(tx, SorafsOperatingAuthorityV1::SetReservePolicy);
                approve_authority_pair(tx, &plan, false);
                let binding = tx
                    .world
                    .runtime_selection()
                    .bindings
                    .iter()
                    .find(|binding| binding.wire_id == "iroha.sorafs.authority.propose.v1")
                    .unwrap();
                let definition = if restrict_interface {
                    binding.interface
                } else {
                    binding.implementation
                };
                tx.world.runtime_selection.get_mut().restrictions = vec![RestrictionEntryV1 {
                    definition,
                    restriction,
                }];
                plan
            });
        let window = plan.grid.window(plan.term).unwrap();
        assert_eq!(window.start_height, 6_401);
        with_authority_component_height(&state, window.start_height, |tx| {
            authority_host::advance_terms(tx).unwrap();
            let lease = tx
                .world
                .sorafs_authority_leases
                .get(&plan.plan_id())
                .unwrap();
            assert!(lease.start_processed);
            assert_eq!(lease.activation.is_some(), grants, "{restriction:?}");
            assert!(lease.grant_approval.is_some() && lease.terminal_approval.is_some());
            assert_eq!(
                tx.world
                    .sorafs_authority_heads
                    .get(&plan.governed_subject_id_v1())
                    .is_some_and(|head| head.granted),
                grants,
                "{restriction:?}"
            );
            assert_authority_projection_valid(tx);
        });
        if !grants {
            with_authority_component_height(&state, window.start_height + 1, |tx| {
                tx.world.runtime_selection.get_mut().restrictions.clear();
                authority_host::advance_terms(tx).unwrap();
                let lease = tx
                    .world
                    .sorafs_authority_leases
                    .get(&plan.plan_id())
                    .unwrap();
                assert!(lease.start_processed);
                assert!(
                    lease.activation.is_none(),
                    "lifting containment cannot replay a start"
                );
                assert!(
                    tx.world
                        .sorafs_authority_heads
                        .get(&plan.governed_subject_id_v1())
                        .is_none()
                );
                assert_authority_projection_valid(tx);
            });
        }
    }
}

#[test]
fn parliament_sorafs_authority_completion_only_preserves_scheduled_expiry() {
    use iroha_data_model::{
        isi::sorafs_authority::SorafsOperatingAuthorityV1,
        runtime::definition::{RestrictionEntryV1, RestrictionV1},
    };

    let state = component_state_with_crypto_policy(blank_test_state());
    let plan = with_authority_component_height(&state, PARLIAMENT_DUE_CERTIFICATE_HEIGHT, |tx| {
        bootstrap_alice_account(tx);
        activate_sorafs_authority_fixture(tx);
        let plan = authority_proposal_fixture(tx, SorafsOperatingAuthorityV1::SetReservePolicy);
        approve_authority_pair(tx, &plan, true);
        plan
    });
    let window = plan.grid.window(plan.term).unwrap();
    with_authority_component_height(&state, window.start_height, |tx| {
        authority_host::advance_terms(tx).unwrap();
        let implementation = tx
            .world
            .sorafs_authority_leases
            .get(&plan.plan_id())
            .unwrap()
            .activation
            .unwrap()
            .implementation;
        tx.world.runtime_selection.get_mut().restrictions = vec![RestrictionEntryV1 {
            definition: implementation,
            restriction: RestrictionV1::CompletionOnly(window.start_height + 1),
        }];
    });
    with_authority_component_height(&state, window.end_height, |tx| {
        authority_host::advance_terms(tx).unwrap();
        let lease = tx
            .world
            .sorafs_authority_leases
            .get(&plan.plan_id())
            .unwrap();
        let completion = lease.completion.unwrap();
        assert_eq!(completion.at_height, window.end_height);
        assert_eq!(Some(completion.approval), lease.terminal_approval);
        let head = tx
            .world
            .sorafs_authority_heads
            .get(&plan.governed_subject_id_v1())
            .unwrap();
        assert!(!head.granted);
        assert_eq!(head.revision, 2);
        assert_eq!(head.enacted_at_height, window.end_height);
        assert_authority_projection_valid(tx);
    });
}

#[test]
fn parliament_sorafs_authority_renewal_expires_before_starting_at_the_same_height() {
    use iroha_data_model::isi::sorafs_authority::SorafsOperatingAuthorityV1;

    let state = component_state_with_crypto_policy(blank_test_state());
    let first = with_authority_component_height(&state, PARLIAMENT_DUE_CERTIFICATE_HEIGHT, |tx| {
        bootstrap_alice_account(tx);
        activate_sorafs_authority_fixture(tx);
        let plan = authority_proposal_fixture(tx, SorafsOperatingAuthorityV1::SetReservePolicy);
        approve_authority_pair(tx, &plan, false);
        plan
    });
    let first_window = first.grid.window(first.term).unwrap();
    with_authority_component_height(&state, first_window.start_height, |tx| {
        authority_host::advance_terms(tx).unwrap();
        assert_authority_projection_valid(tx);
    });
    let next = with_authority_component_height(&state, first_window.start_height + 100, |tx| {
        let mut next = first.clone();
        next.term = 2;
        next.expected_revision = 2;
        let head = tx
            .world
            .sorafs_authority_heads
            .get(&first.governed_subject_id_v1())
            .unwrap();
        assert!(head.granted);
        assert_eq!(head.revision, 1);
        assert_eq!(
            authority_host::expected_head(&next, &tx.world),
            authority_host::admitted_expected_head(&next),
            "the irrevocable terminal mandate supplies the future predecessor"
        );
        approve_authority_pair(tx, &next, true);
        let first_lease = tx
            .world
            .sorafs_authority_leases
            .get(&first.plan_id())
            .unwrap();
        assert!(first_lease.activation.is_some() && first_lease.completion.is_none());
        assert_authority_projection_valid(tx);
        next
    });
    let next_window = next.grid.window(next.term).unwrap();
    assert_eq!(first_window.end_height, next_window.start_height);
    with_authority_component_height(&state, next_window.start_height, |tx| {
        authority_host::advance_terms(tx).unwrap();
        let first_lease = tx
            .world
            .sorafs_authority_leases
            .get(&first.plan_id())
            .unwrap();
        let completion = first_lease.completion.unwrap();
        assert_eq!(completion.revision, 2);
        assert_eq!(completion.at_height, next_window.start_height);
        assert_eq!(Some(completion.approval), first_lease.terminal_approval);
        let activation = tx
            .world
            .sorafs_authority_leases
            .get(&next.plan_id())
            .unwrap()
            .activation
            .unwrap();
        assert_eq!(activation.revision, 3);
        assert_eq!(activation.at_height, completion.at_height);
        let head = tx
            .world
            .sorafs_authority_heads
            .get(&next.governed_subject_id_v1())
            .unwrap()
            .clone();
        assert!(head.granted);
        assert_eq!(head.plan_id, next.plan_id());
        assert_eq!(head.revision, 3);
        authority_host::advance_terms(tx).unwrap();
        assert_eq!(
            tx.world
                .sorafs_authority_heads
                .get(&next.governed_subject_id_v1()),
            Some(&head),
            "the old expiry cannot repeat against the renewed incarnation"
        );
        assert_authority_projection_valid(tx);
    });
    with_authority_component_height(&state, next_window.end_height, |tx| {
        authority_host::advance_terms(tx).unwrap();
        let head = tx
            .world
            .sorafs_authority_heads
            .get(&next.governed_subject_id_v1())
            .unwrap();
        assert!(!head.granted);
        assert_eq!(head.revision, 4);
        assert_eq!(head.enacted_at_height, next_window.end_height);
        assert!(crate::runtime::obligations::LiveObligations::of(&tx.world).is_empty());
        assert_authority_projection_valid(tx);
    });
}
