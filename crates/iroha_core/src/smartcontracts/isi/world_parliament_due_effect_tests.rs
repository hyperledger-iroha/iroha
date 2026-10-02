// Exact-due automatic-enactment coverage for the remaining typed Parliament effects.

fn assert_exact_due_parliament_effect_enacted(
    state_transaction: &StateTransaction<'_, '_>,
    fixture: &DueParliamentCertificateFixture,
) {
    assert_eq!(
        state_transaction.block_height(),
        PARLIAMENT_DUE_CERTIFICATE_HEIGHT,
        "the effect must execute at the certificate's exact due height",
    );
    let attempt = state_transaction
        .world
        .parliament_attempts
        .get(&fixture.governance_attempt_id)
        .expect("terminal exact-due Parliament attempt");
    assert_eq!(attempt.attempt().status, GovernanceAttemptStatusV1::Enacted);
    assert_eq!(
        attempt.terminal_height(),
        Some(PARLIAMENT_DUE_CERTIFICATE_HEIGHT)
    );
    assert_eq!(
        attempt
            .certificate()
            .map(|certificate| certificate.enact_at_height),
        Some(PARLIAMENT_DUE_CERTIFICATE_HEIGHT)
    );
    let proposal = state_transaction
        .world
        .governance_proposals
        .get(&fixture.proposal_id)
        .expect("enacted exact-due governance proposal");
    assert_eq!(
        proposal.status,
        crate::state::GovernanceProposalStatus::Enacted
    );
    assert!(
        state_transaction
            .world
            .internal_event_buf
            .iter()
            .any(|event| matches!(
                event.as_ref(),
                iroha_data_model::events::data::DataEvent::Governance(
                    GovernanceEvent::ProposalEnacted(enacted)
                ) if enacted.id == fixture.proposal_id
            )),
        "automatic success must emit the typed ProposalEnacted event",
    );
    assert_automatic_parliament_execution_event(
        state_transaction,
        fixture,
        gov::ParliamentAutomaticExecutionOutcomeV1::Enacted,
    );
}

#[test]
fn parliament_kagemusha_initial_policy_installs_only_at_exact_due_height() {
    use iroha_data_model::{
        governance::types::KagemushaVerifierPolicyInstallProposalV1,
        kagemusha::{
            KAGEMUSHA_WIRE_VERSION_V1, KagemushaGovernedVerifierRegistryV1,
            KagemushaReleaseAuthorityPolicyV1,
        },
    };

    let state = blank_test_state();
    for height in 1..PARLIAMENT_DUE_CERTIFICATE_HEIGHT {
        let header = iroha_data_model::block::BlockHeader::new(
            NonZeroU64::new(height).expect("predecessor height is nonzero"),
            None,
            None,
            0,
            0,
        );
        state
            .block(header)
            .commit_empty_block_for_testing()
            .expect("contiguous State predecessor for exact-due certificate");
    }
    let block = new_dummy_block_at_height(
        NonZeroU64::new(PARLIAMENT_DUE_CERTIFICATE_HEIGHT).expect("due height is nonzero"),
    );
    let mut state_block = state.block(block.as_ref().header());
    let policy = KagemushaReleaseAuthorityPolicyV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        authority_set_id: [0xD1; 32],
        threshold: 1,
        authorized_signers: vec![
            iroha_crypto::KeyPair::try_random()
                .expect("fixture signer")
                .public_key()
                .clone(),
        ],
    };
    let fixture = {
        let mut seed = state_block.transaction();
        let kind = ProposalKind::KagemushaVerifierPolicyInstall(
            KagemushaVerifierPolicyInstallProposalV1 {
                proposal_operator: ALICE_ID.clone(),
                network_id: seed.network_id,
                expected_predecessor: KagemushaGovernedVerifierRegistryV1::default(),
                authority_policy: policy.clone(),
            },
        );
        let fixture = seed_due_parliament_certificate(&mut seed, kind);
        seed.apply();
        fixture
    };
    let certificate_digest =
        crate::governance::parliament::kagemusha_registry_certificate_digest_v1(
            &fixture.certificate,
        )
        .expect("canonical certificate digest");
    let domain = b"iroha:state:kagemusha-registry-transition-token:certificate:v1";
    let mut expected_digest = blake3::Hasher::new();
    expected_digest.update(
        &u64::try_from(domain.len())
            .expect("fixed certificate domain length")
            .to_le_bytes(),
    );
    expected_digest.update(domain);
    expected_digest.update(
        &norito::encode_canonical(&fixture.certificate).expect("canonical certificate frame"),
    );
    assert_eq!(certificate_digest, *expected_digest.finalize().as_bytes());
    let mut changed_certificate = fixture.certificate.clone();
    changed_certificate.effect_preimage_hash[0] ^= 1;
    assert_ne!(
        certificate_digest,
        crate::governance::parliament::kagemusha_registry_certificate_digest_v1(
            &changed_certificate,
        )
        .expect("changed canonical certificate digest"),
    );

    let authorization = {
        let inspection = state_block.transaction();
        let proposal = inspection
            .world
            .governance_proposals
            .get(&fixture.proposal_id)
            .expect("retained exact policy proposal");
        let attempt = inspection
            .world
            .parliament_attempts
            .get(&fixture.governance_attempt_id)
            .expect("certified exact policy attempt");
        let head = parliament_expected_head_v1(&proposal.kind, &inspection)
            .expect("canonical empty registry head");
        let mut wrong_head = head;
        if let GovernanceExpectedHeadV1::Present(ref mut present) = wrong_head {
            present.head_root[0] ^= 1;
        }
        assert!(
            crate::governance::parliament::KagemushaRegistryTransitionAuthorizationV1::issue(
                attempt,
                proposal,
                &fixture.certificate,
                inspection.network_id,
                inspection.block_height(),
                wrong_head,
                inspection.world.kagemusha_verifier_registry.get(),
            )
            .is_err(),
            "changed compare-and-set head cannot issue the State token"
        );
        crate::governance::parliament::KagemushaRegistryTransitionAuthorizationV1::issue(
            attempt,
            proposal,
            &fixture.certificate,
            inspection.network_id,
            inspection.block_height(),
            head,
            inspection.world.kagemusha_verifier_registry.get(),
        )
        .expect("exact certified policy State token")
    };

    let mut execution = state_block.transaction();
    assert_eq!(
        execute_due_parliament_certificate_v1(fixture.governance_attempt_id, &mut execution)
            .expect("execute certified policy install"),
        DueParliamentCertificateExecutionV1::Applied
    );
    assert_eq!(
        execution
            .world
            .kagemusha_verifier_registry
            .get()
            .authority_policy,
        Some(policy)
    );
    assert_exact_due_parliament_effect_enacted(&execution, &fixture);
    let committed_proposal = execution
        .world
        .governance_proposals
        .get(&fixture.proposal_id)
        .expect("enacted policy proposal");
    let committed_attempt = execution
        .world
        .parliament_attempts
        .get(&fixture.governance_attempt_id)
        .expect("enacted policy attempt");
    let predecessor = KagemushaGovernedVerifierRegistryV1::default();
    let successor = execution.world.kagemusha_verifier_registry.get();
    let verify =
        |network_id: iroha_data_model::NetworkId,
         height: u64,
         predecessor: &KagemushaGovernedVerifierRegistryV1,
         successor: &KagemushaGovernedVerifierRegistryV1,
         proposal: Option<&crate::state::GovernanceProposalRecord>,
         attempt: Option<&crate::governance::parliament::ParliamentAttemptStateV1>| {
            authorization.validate_for_state_commit(
                network_id,
                height,
                predecessor,
                successor,
                proposal,
                attempt,
            )
        };
    verify(
        execution.network_id,
        execution.block_height(),
        &predecessor,
        successor,
        Some(committed_proposal),
        Some(committed_attempt),
    )
    .expect("exact certified State transition");
    assert!(
        verify(
            execution.network_id,
            execution.block_height(),
            &predecessor,
            successor,
            None,
            Some(committed_attempt),
        )
        .is_err()
    );
    assert!(
        verify(
            execution.network_id,
            execution.block_height(),
            &predecessor,
            successor,
            Some(committed_proposal),
            None,
        )
        .is_err()
    );
    assert!(
        verify(
            execution.network_id,
            execution.block_height() + 1,
            &predecessor,
            successor,
            Some(committed_proposal),
            Some(committed_attempt),
        )
        .is_err()
    );
    let wrong_network = iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
        iroha_data_model::block::BlockHeader,
    >::from_untyped_unchecked(
        iroha_crypto::Hash::prehashed([0xEE; 32]),
    ));
    assert_ne!(wrong_network, execution.network_id);
    assert!(
        verify(
            wrong_network,
            execution.block_height(),
            &predecessor,
            successor,
            Some(committed_proposal),
            Some(committed_attempt),
        )
        .is_err()
    );
    let mut wrong_successor = successor.clone();
    wrong_successor.authority_policy = None;
    assert!(
        verify(
            execution.network_id,
            execution.block_height(),
            &predecessor,
            &wrong_successor,
            Some(committed_proposal),
            Some(committed_attempt),
        )
        .is_err()
    );
    let mut wrong_predecessor = predecessor.clone();
    wrong_predecessor.version = 0;
    assert!(
        verify(
            execution.network_id,
            execution.block_height(),
            &wrong_predecessor,
            successor,
            Some(committed_proposal),
            Some(committed_attempt),
        )
        .is_err()
    );
    execution.apply();
    let mut replay = state_block.transaction();
    assert_ne!(
        replay.world.kagemusha_verifier_registry.get(),
        &KagemushaGovernedVerifierRegistryV1::default()
    );
    assert_eq!(
        replay
            .world
            .parliament_attempts
            .get(&fixture.governance_attempt_id)
            .expect("retained attempt")
            .attempt()
            .status,
        GovernanceAttemptStatusV1::Enacted
    );
    assert!(
        execute_due_parliament_certificate_v1(fixture.governance_attempt_id, &mut replay).is_err(),
        "one certified policy attempt cannot enact twice"
    );
    drop(replay);
    state_block
        .commit_empty_block_for_testing()
        .expect("certified exact-due policy transition publishes to State");
    assert!(
        state
            .world
            .kagemusha_verifier_registry
            .view()
            .get()
            .authority_policy
            .is_some()
    );
}

#[test]
fn parliament_kagemusha_release_installs_standby_only_at_exact_due_height() {
    use iroha_data_model::{
        governance::types::KagemushaVerifierReleaseInstallProposalV1,
        isi::governance::ProposeKagemushaVerifierReleaseInstallV1,
        kagemusha::KAGEMUSHA_RELEASE_STANDBY_V1,
    };

    let instruction: ProposeKagemushaVerifierReleaseInstallV1 =
        norito::decode_canonical(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/governance/kagemusha_verifier_release_install_v1.bin"
        )))
        .expect("canonical authenticated release fixture");
    let KagemushaVerifierReleaseInstallProposalV1 {
        expected_predecessor,
        ..
    } = &instruction.proposal;
    let predecessor = expected_predecessor.clone();
    let world = World::default();
    {
        let mut initial = world.block();
        *initial.kagemusha_verifier_registry.get_mut() = predecessor.clone();
        initial.commit();
    }
    let state = State::new_with_chain_and_network_id_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
        "generic-testnet".parse().expect("fixture chain"),
        instruction.proposal.network_id,
    );
    for height in 1..PARLIAMENT_DUE_CERTIFICATE_HEIGHT {
        let header = iroha_data_model::block::BlockHeader::new(
            NonZeroU64::new(height).expect("predecessor height is nonzero"),
            None,
            None,
            0,
            0,
        );
        state
            .block(header)
            .commit_empty_block_for_testing()
            .expect("contiguous State predecessor for exact-due certificate");
    }
    let block = new_dummy_block_at_height(
        NonZeroU64::new(PARLIAMENT_DUE_CERTIFICATE_HEIGHT).expect("due height is nonzero"),
    );
    let mut state_block = state.block(block.as_ref().header());
    let fixture = {
        let mut seed = state_block.transaction();
        let mut proposal = instruction.proposal;
        proposal.proposal_operator = ALICE_ID.clone();
        assert_eq!(proposal.expected_predecessor, predecessor);
        proposal
            .validate()
            .expect("exact authenticated release proposal");
        let fixture = seed_due_parliament_certificate(
            &mut seed,
            ProposalKind::KagemushaVerifierReleaseInstall(proposal),
        );
        seed.apply();
        fixture
    };

    let authorization = {
        let inspection = state_block.transaction();
        let proposal = inspection
            .world
            .governance_proposals
            .get(&fixture.proposal_id)
            .expect("retained release proposal");
        let attempt = inspection
            .world
            .parliament_attempts
            .get(&fixture.governance_attempt_id)
            .expect("certified release attempt");
        let head = parliament_expected_head_v1(&proposal.kind, &inspection)
            .expect("canonical predecessor registry head");
        crate::governance::parliament::KagemushaRegistryTransitionAuthorizationV1::issue(
            attempt,
            proposal,
            &fixture.certificate,
            inspection.network_id,
            inspection.block_height(),
            head,
            inspection.world.kagemusha_verifier_registry.get(),
        )
        .expect("exact certified release State token")
    };

    let mut execution = state_block.transaction();
    assert_eq!(
        execute_due_parliament_certificate_v1(fixture.governance_attempt_id, &mut execution)
            .expect("execute certified release install"),
        DueParliamentCertificateExecutionV1::Applied
    );
    let successor = execution.world.kagemusha_verifier_registry.get();
    assert_eq!(successor.active_release_id, None);
    assert_eq!(successor.releases.len(), 1);
    assert_eq!(successor.releases[0].status, KAGEMUSHA_RELEASE_STANDBY_V1);
    assert_exact_due_parliament_effect_enacted(&execution, &fixture);
    let committed_proposal = execution
        .world
        .governance_proposals
        .get(&fixture.proposal_id)
        .expect("enacted release proposal");
    let committed_attempt = execution
        .world
        .parliament_attempts
        .get(&fixture.governance_attempt_id)
        .expect("enacted release attempt");
    authorization
        .validate_for_state_commit(
            execution.network_id,
            execution.block_height(),
            &predecessor,
            successor,
            Some(committed_proposal),
            Some(committed_attempt),
        )
        .expect("exact release successor accepted by State");
    let mut forged = successor.clone();
    forged.releases[0].receipt_digest[0] ^= 1;
    assert!(
        authorization
            .validate_for_state_commit(
                execution.network_id,
                execution.block_height(),
                &predecessor,
                &forged,
                Some(committed_proposal),
                Some(committed_attempt),
            )
            .is_err(),
        "State token cannot authorize a different release"
    );
    execution.apply();
    state_block
        .commit_empty_block_for_testing()
        .expect("certified inactive release publishes to State");
    assert_eq!(
        state
            .world
            .kagemusha_verifier_registry
            .view()
            .get()
            .releases
            .len(),
        1
    );
    state
        .validate_kagemusha_v1_runtime_for_startup()
        .expect("inactive release keeps runtime rejection closed");
}

#[test]
fn parliament_kagemusha_activation_publishes_only_the_exact_due_successor() {
    use iroha_data_model::{
        isi::governance::ProposeKagemushaVerifierReleaseActivateV1,
        kagemusha::KAGEMUSHA_RELEASE_ACTIVE_V1,
    };

    let instruction: ProposeKagemushaVerifierReleaseActivateV1 =
        norito::decode_canonical(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/governance/kagemusha_verifier_release_activate_v1.bin"
        )))
        .expect("canonical verifier activation fixture");
    let predecessor = instruction.proposal.expected_predecessor.clone();
    let world = World::default();
    {
        let mut initial = world.block();
        *initial.kagemusha_verifier_registry.get_mut() = predecessor.clone();
        initial.commit();
    }
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    for height in 1..PARLIAMENT_DUE_CERTIFICATE_HEIGHT {
        let header = iroha_data_model::block::BlockHeader::new(
            NonZeroU64::new(height).expect("predecessor height is nonzero"),
            None,
            None,
            0,
            0,
        );
        state
            .block(header)
            .commit_empty_block_for_testing()
            .expect("contiguous State predecessor for exact-due certificate");
    }
    let block = new_dummy_block_at_height(
        NonZeroU64::new(PARLIAMENT_DUE_CERTIFICATE_HEIGHT).expect("due height is nonzero"),
    );
    let mut state_block = state.block(block.as_ref().header());
    let fixture = {
        let mut seed = state_block.transaction();
        let mut proposal = instruction.proposal;
        proposal.proposal_operator = ALICE_ID.clone();
        proposal.network_id = seed.network_id;
        proposal
            .validate()
            .expect("exact standby activation proposal");
        let fixture = seed_due_parliament_certificate(
            &mut seed,
            ProposalKind::KagemushaVerifierReleaseActivate(proposal),
        );
        seed.apply();
        fixture
    };
    let authorization = {
        let inspection = state_block.transaction();
        let proposal = inspection
            .world
            .governance_proposals
            .get(&fixture.proposal_id)
            .expect("retained activation proposal");
        let attempt = inspection
            .world
            .parliament_attempts
            .get(&fixture.governance_attempt_id)
            .expect("certified activation attempt");
        let head = parliament_expected_head_v1(&proposal.kind, &inspection)
            .expect("canonical predecessor registry head");
        crate::governance::parliament::KagemushaRegistryTransitionAuthorizationV1::issue(
            attempt,
            proposal,
            &fixture.certificate,
            inspection.network_id,
            inspection.block_height(),
            head,
            inspection.world.kagemusha_verifier_registry.get(),
        )
        .expect("exact certified activation State token")
    };
    let mut execution = state_block.transaction();
    assert_eq!(
        execute_due_parliament_certificate_v1(fixture.governance_attempt_id, &mut execution)
            .expect("execute certified activation"),
        DueParliamentCertificateExecutionV1::Applied
    );
    let successor = execution.world.kagemusha_verifier_registry.get();
    assert_eq!(successor.releases.len(), 1);
    assert_eq!(successor.releases[0].status, KAGEMUSHA_RELEASE_ACTIVE_V1);
    assert_eq!(
        successor.active_release_id,
        Some(successor.releases[0].release_id)
    );
    let successor_active = successor.active_release_id;
    assert_exact_due_parliament_effect_enacted(&execution, &fixture);
    let committed_proposal = execution
        .world
        .governance_proposals
        .get(&fixture.proposal_id)
        .expect("enacted activation proposal");
    let committed_attempt = execution
        .world
        .parliament_attempts
        .get(&fixture.governance_attempt_id)
        .expect("enacted activation attempt");
    authorization
        .validate_for_state_commit(
            execution.network_id,
            execution.block_height(),
            &predecessor,
            successor,
            Some(committed_proposal),
            Some(committed_attempt),
        )
        .expect("exact active successor accepted by State");
    let mut forged = successor.clone();
    forged.releases[0].receipt_digest[0] ^= 1;
    assert!(
        authorization
            .validate_for_state_commit(
                execution.network_id,
                execution.block_height(),
                &predecessor,
                &forged,
                Some(committed_proposal),
                Some(committed_attempt),
            )
            .is_err()
    );
    execution.apply();
    state_block
        .commit_empty_block_for_testing()
        .expect("certified activation publishes with the fail-closed local runtime");
    assert_eq!(
        state
            .world
            .kagemusha_verifier_registry
            .view()
            .get()
            .active_release_id,
        successor_active,
    );
    state
        .validate_kagemusha_v1_runtime_for_startup()
        .expect("an active finalized release can await exact local artifact reload");
    let mut next = state.block(iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(PARLIAMENT_DUE_CERTIFICATE_HEIGHT + 1).unwrap(),
        None,
        None,
        0,
        0,
    ));
    let mut tx = next.transaction();
    crate::smartcontracts::isi::kagemusha::runtime_publication_tests::check_transaction(
        &mut tx,
        successor_active.unwrap(),
        true,
        false,
    );
}

#[test]
fn parliament_kagemusha_retirement_publishes_only_exact_certified_standby_removal() {
    use iroha_data_model::{
        isi::governance::ProposeKagemushaVerifierReleaseRetireV1,
        kagemusha::{
            KAGEMUSHA_RELEASE_ACTIVE_V1, KAGEMUSHA_RELEASE_STANDBY_V1,
            KAGEMUSHA_RELEASE_VERIFICATION_ONLY_V1,
        },
    };

    // The model's genuine paired producer emits this populated, threshold-signed fixture.
    // A missing producer output is a failure, never a synthesized publication substitute.
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/governance/kagemusha_verifier_release_retire_v1.bin"
    ))
    .expect("genuine populated retirement fixture must be generated before Core publication tests");
    let instruction: ProposeKagemushaVerifierReleaseRetireV1 =
        norito::decode_canonical(&bytes).expect("canonical verifier retirement fixture");
    assert_eq!(norito::encode_canonical(&instruction).unwrap(), bytes);
    instruction.proposal.validate().unwrap();
    let original_operator = instruction.proposal.proposal_operator.clone();
    let original_network = instruction.proposal.network_id;
    let original_kind = ProposalKind::KagemushaVerifierReleaseRetire(instruction.proposal.clone());
    let predecessor = instruction.proposal.expected_predecessor.clone();
    let target = instruction.proposal.standby_release_id;
    assert_eq!(predecessor.releases.len(), 3);
    for status in [
        KAGEMUSHA_RELEASE_ACTIVE_V1,
        KAGEMUSHA_RELEASE_STANDBY_V1,
        KAGEMUSHA_RELEASE_VERIFICATION_ONLY_V1,
    ] {
        assert_eq!(
            predecessor
                .releases
                .iter()
                .filter(|row| row.status == status)
                .count(),
            1
        );
    }
    assert!(
        predecessor
            .releases
            .iter()
            .any(|row| { row.release_id == target && row.status == KAGEMUSHA_RELEASE_STANDBY_V1 })
    );
    let retained_rows = predecessor
        .releases
        .iter()
        .filter(|row| row.release_id != target)
        .cloned()
        .collect::<Vec<_>>();
    let mut expected_successor = predecessor.clone();
    expected_successor.releases = retained_rows;
    expected_successor.validate().unwrap();
    assert_eq!(
        instruction.proposal.successor().unwrap(),
        expected_successor
    );
    assert_eq!(expected_successor.releases.len(), 2);
    let retained_active = predecessor
        .active_release_id
        .expect("authentic current active release");
    let world = World::default();
    {
        let mut initial = world.block();
        *initial.kagemusha_verifier_registry.get_mut() = predecessor.clone();
        initial.commit();
    }
    let state = State::new_with_chain_and_network_id_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
        "generic-testnet".parse().expect("fixture chain"),
        original_network,
    );
    for height in 1..PARLIAMENT_DUE_CERTIFICATE_HEIGHT {
        let header = iroha_data_model::block::BlockHeader::new(
            NonZeroU64::new(height).expect("predecessor height is nonzero"),
            None,
            None,
            0,
            0,
        );
        state
            .block(header)
            .commit_empty_block_for_testing()
            .expect("contiguous State predecessor for exact-due certificate");
    }
    let block = new_dummy_block_at_height(
        NonZeroU64::new(PARLIAMENT_DUE_CERTIFICATE_HEIGHT).expect("due height is nonzero"),
    );
    let mut state_block = state.block(block.as_ref().header());
    let fixture = {
        let mut seed = state_block.transaction();
        assert_eq!(seed.network_id, original_network);
        let fixture = seed_due_parliament_certificate(&mut seed, original_kind.clone());
        assert_eq!(fixture.proposal_id, original_kind.fingerprint());
        seed.apply();
        fixture
    };
    let authorization = {
        let inspection = state_block.transaction();
        let proposal = inspection
            .world
            .governance_proposals
            .get(&fixture.proposal_id)
            .expect("retained retirement proposal");
        let attempt = inspection
            .world
            .parliament_attempts
            .get(&fixture.governance_attempt_id)
            .expect("certified retirement attempt");
        assert_eq!(proposal.kind, original_kind);
        assert_eq!(proposal.proposer, original_operator);
        assert_eq!(inspection.network_id, original_network);
        let head = parliament_expected_head_v1(&proposal.kind, &inspection)
            .expect("canonical predecessor registry head");
        let mut wrong_proposer = proposal.clone();
        wrong_proposer.proposer = if original_operator == *ALICE_ID {
            BOB_ID.clone()
        } else {
            ALICE_ID.clone()
        };
        assert_ne!(wrong_proposer.proposer, original_operator);
        assert!(
            crate::governance::parliament::KagemushaRegistryTransitionAuthorizationV1::issue(
                attempt,
                &wrong_proposer,
                &fixture.certificate,
                original_network,
                inspection.block_height(),
                head,
                inspection.world.kagemusha_verifier_registry.get(),
            )
            .is_err()
        );
        let wrong_network = iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
            iroha_data_model::block::BlockHeader,
        >::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0xEE; 32]),
        ));
        assert_ne!(wrong_network, original_network);
        assert!(
            crate::governance::parliament::KagemushaRegistryTransitionAuthorizationV1::issue(
                attempt,
                proposal,
                &fixture.certificate,
                wrong_network,
                inspection.block_height(),
                head,
                inspection.world.kagemusha_verifier_registry.get(),
            )
            .is_err()
        );
        for due_height in [inspection.block_height() - 1, inspection.block_height() + 1] {
            assert!(
                crate::governance::parliament::KagemushaRegistryTransitionAuthorizationV1::issue(
                    attempt,
                    proposal,
                    &fixture.certificate,
                    inspection.network_id,
                    due_height,
                    head,
                    inspection.world.kagemusha_verifier_registry.get(),
                )
                .is_err()
            );
        }
        let mut forged_certificate = fixture.certificate.clone();
        forged_certificate.proposal_content_id[0] ^= 1;
        assert!(
            crate::governance::parliament::KagemushaRegistryTransitionAuthorizationV1::issue(
                attempt,
                proposal,
                &forged_certificate,
                inspection.network_id,
                inspection.block_height(),
                head,
                inspection.world.kagemusha_verifier_registry.get(),
            )
            .is_err()
        );
        crate::governance::parliament::KagemushaRegistryTransitionAuthorizationV1::issue(
            attempt,
            proposal,
            &fixture.certificate,
            inspection.network_id,
            inspection.block_height(),
            head,
            inspection.world.kagemusha_verifier_registry.get(),
        )
        .expect("exact certified retirement State token")
    };
    {
        let mut stale = state_block.transaction();
        stale
            .world
            .kagemusha_verifier_registry
            .get_mut()
            .retire_standby(target)
            .unwrap();
        assert_eq!(
            execute_due_parliament_certificate_v1(fixture.governance_attempt_id, &mut stale)
                .unwrap(),
            DueParliamentCertificateExecutionV1::Applied
        );
        assert_eq!(
            stale
                .world
                .governance_proposals
                .get(&fixture.proposal_id)
                .unwrap()
                .status,
            crate::state::GovernanceProposalStatus::Superseded
        );
        assert_eq!(
            stale
                .world
                .parliament_attempts
                .get(&fixture.governance_attempt_id)
                .unwrap()
                .attempt()
                .status,
            GovernanceAttemptStatusV1::Superseded
        );
        // Discard the unauthoritative synthetic mutation and supersession together.
    }
    {
        let inspect = state_block.transaction();
        assert_eq!(
            inspect.world.kagemusha_verifier_registry.get(),
            &predecessor
        );
        assert_eq!(
            inspect
                .world
                .parliament_attempts
                .get(&fixture.governance_attempt_id)
                .unwrap()
                .attempt()
                .status,
            GovernanceAttemptStatusV1::Certified
        );
    }
    {
        let mut dropped = state_block.transaction();
        assert_eq!(
            execute_due_parliament_certificate_v1(fixture.governance_attempt_id, &mut dropped)
                .unwrap(),
            DueParliamentCertificateExecutionV1::Applied
        );
        assert_eq!(
            dropped.world.kagemusha_verifier_registry.get(),
            &expected_successor
        );
        assert_exact_due_parliament_effect_enacted(&dropped, &fixture);
        // No apply: release authority, certified attempt and transition token roll back.
    }
    {
        let inspect = state_block.transaction();
        assert_eq!(
            inspect.world.kagemusha_verifier_registry.get(),
            &predecessor
        );
        assert_eq!(
            inspect
                .world
                .parliament_attempts
                .get(&fixture.governance_attempt_id)
                .unwrap()
                .attempt()
                .status,
            GovernanceAttemptStatusV1::Certified
        );
    }
    let mut execution = state_block.transaction();
    assert_eq!(
        execute_due_parliament_certificate_v1(fixture.governance_attempt_id, &mut execution)
            .expect("execute certified retirement"),
        DueParliamentCertificateExecutionV1::Applied
    );
    let successor = execution.world.kagemusha_verifier_registry.get();
    assert_eq!(successor, &expected_successor);
    assert_eq!(successor.active_release_id, Some(retained_active));
    assert_eq!(successor.authority_policy, predecessor.authority_policy);
    assert_exact_due_parliament_effect_enacted(&execution, &fixture);
    let committed_proposal = execution
        .world
        .governance_proposals
        .get(&fixture.proposal_id)
        .expect("enacted retirement proposal");
    assert_eq!(committed_proposal.kind, original_kind);
    assert_eq!(committed_proposal.proposer, original_operator);
    assert_eq!(execution.network_id, original_network);
    let committed_attempt = execution
        .world
        .parliament_attempts
        .get(&fixture.governance_attempt_id)
        .expect("enacted retirement attempt");
    authorization
        .validate_for_state_commit(
            execution.network_id,
            execution.block_height(),
            &predecessor,
            successor,
            Some(committed_proposal),
            Some(committed_attempt),
        )
        .expect("exact retired successor accepted by State");
    let forged = predecessor.clone();
    assert!(
        authorization
            .validate_for_state_commit(
                execution.network_id,
                execution.block_height(),
                &predecessor,
                &forged,
                Some(committed_proposal),
                Some(committed_attempt),
            )
            .is_err()
    );
    for (height, original, proposed, record, attempt) in [
        (
            execution.block_height() + 1,
            &predecessor,
            successor,
            Some(committed_proposal),
            Some(committed_attempt),
        ),
        (
            execution.block_height(),
            successor,
            successor,
            Some(committed_proposal),
            Some(committed_attempt),
        ),
        (
            execution.block_height(),
            &predecessor,
            successor,
            None,
            Some(committed_attempt),
        ),
        (
            execution.block_height(),
            &predecessor,
            successor,
            Some(committed_proposal),
            None,
        ),
    ] {
        assert!(
            authorization
                .validate_for_state_commit(
                    execution.network_id,
                    height,
                    original,
                    proposed,
                    record,
                    attempt
                )
                .is_err()
        );
    }
    execution.apply();
    state_block
        .commit_empty_block_for_testing()
        .expect("exact certified standby retirement publishes");
    {
        let proposals = state.world.governance_proposals.view();
        let published = proposals
            .get(&fixture.proposal_id)
            .expect("same original retirement proposal survives State publication");
        assert_eq!(published.kind, original_kind);
        assert_eq!(published.proposer, original_operator);
    }
    let current = state.world.kagemusha_verifier_registry.view();
    assert_eq!(current.get(), &expected_successor);
    assert_eq!(current.get().active_release_id, Some(retained_active));
    assert_eq!(current.get().authority_policy, predecessor.authority_policy);
    drop(current);
    state
        .validate_kagemusha_v1_runtime_for_startup()
        .expect("retirement preserves finalized active authority with a reject-all local verifier");
    let mut next = state.block(iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(PARLIAMENT_DUE_CERTIFICATE_HEIGHT + 1).unwrap(),
        None,
        None,
        0,
        0,
    ));
    let mut tx = next.transaction();
    crate::smartcontracts::isi::kagemusha::runtime_publication_tests::check_transaction(
        &mut tx,
        retained_active,
        true,
        false,
    );
}

#[test]
fn parliament_runtime_upgrade_enacts_at_the_exact_due_height() {
    let state = blank_test_state();
    let block = new_dummy_block_at_height(
        NonZeroU64::new(PARLIAMENT_DUE_CERTIFICATE_HEIGHT).expect("due height is nonzero"),
    );
    let mut state_block = state.block(block.as_ref().header());
    let manifest = iroha_data_model::runtime::RuntimeUpgradeManifest {
        name: "exact-due-runtime-upgrade".to_owned(),
        description: "activate the fixed first-release ABI at the certified height".to_owned(),
        abi_version: 1,
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        added_syscalls: Vec::new(),
        added_pointer_types: Vec::new(),
        start_height: PARLIAMENT_DUE_CERTIFICATE_HEIGHT,
        end_height: PARLIAMENT_DUE_CERTIFICATE_HEIGHT + 1,
        sbom_digests: Vec::new(),
        slsa_attestation: Vec::new(),
        provenance: Vec::new(),
    };
    let upgrade_id = manifest.id();
    let fixture = {
        let mut seed = state_block.transaction();
        let fixture = seed_due_parliament_certificate(
            &mut seed,
            ProposalKind::RuntimeUpgrade(RuntimeUpgradeProposal {
                proposal_operator: ALICE_ID.clone(),
                manifest: manifest.clone(),
            }),
        );
        seed.apply();
        fixture
    };

    let mut execution = state_block.transaction();
    execution.world.internal_event_buf.clear();
    assert_eq!(
        execute_due_parliament_certificate_v1(fixture.governance_attempt_id, &mut execution)
            .expect("execute exact-due runtime-upgrade certificate"),
        DueParliamentCertificateExecutionV1::Applied
    );
    let record = execution
        .world
        .runtime_upgrades
        .get(&upgrade_id)
        .expect("runtime-upgrade effect record");
    assert_eq!(record.manifest, manifest);
    assert_eq!(
        record.status,
        iroha_data_model::runtime::RuntimeUpgradeStatus::ActivatedAt(
            PARLIAMENT_DUE_CERTIFICATE_HEIGHT
        )
    );
    assert_eq!(&record.proposer, &*ALICE_ID);
    assert_eq!(record.created_height, PARLIAMENT_DUE_CERTIFICATE_HEIGHT);
    assert_exact_due_parliament_effect_enacted(&execution, &fixture);
}

#[test]
fn parliament_sorafs_provider_owner_enacts_at_the_exact_due_height() {
    use iroha_data_model::isi::sorafs::{
        EstablishSorafsProviderOwnerV1, SorafsProviderGovernanceActionV1,
    };
    use iroha_executor_data_model::permission::sorafs::CanOperateSorafsRepair;

    let state = blank_test_state();
    let block = new_dummy_block_at_height(
        NonZeroU64::new(PARLIAMENT_DUE_CERTIFICATE_HEIGHT).expect("due height is nonzero"),
    );
    let mut state_block = state.block(block.as_ref().header());
    let provider_id = iroha_data_model::sorafs::capacity::ProviderId::new([0xA8; 32]);
    let fixture = {
        let mut seed = state_block.transaction();
        bootstrap_alice_account(&mut seed);
        let fixture = seed_due_parliament_certificate(
            &mut seed,
            ProposalKind::SorafsProviderGovernance(SorafsProviderGovernanceProposal {
                action: Box::new(SorafsProviderGovernanceActionV1::Establish(
                    EstablishSorafsProviderOwnerV1 {
                        provider_id,
                        owner: ALICE_ID.clone(),
                    },
                )),
            }),
        );
        seed.apply();
        fixture
    };

    let mut execution = state_block.transaction();
    execution.world.internal_event_buf.clear();
    assert_eq!(
        execute_due_parliament_certificate_v1(fixture.governance_attempt_id, &mut execution)
            .expect("execute exact-due SoraFS provider certificate"),
        DueParliamentCertificateExecutionV1::Applied
    );
    assert_eq!(
        execution.world.provider_owners.get(&provider_id),
        Some(&*ALICE_ID)
    );
    let repair_permission = Permission::from(CanOperateSorafsRepair { provider_id });
    assert!(
        execution
            .world
            .account_permissions
            .get(&ALICE_ID)
            .is_some_and(|permissions| permissions.contains(&repair_permission)),
        "provider enactment must install its typed repair-worker authorization",
    );
    assert_exact_due_parliament_effect_enacted(&execution, &fixture);
}

#[test]
fn parliament_musubi_action_authorization_enacts_at_the_exact_due_height() {
    use iroha_data_model::musubi::{
        MusubiGovernanceDecisionV1, MusubiPackageIdV1, MusubiPackageScopeV1,
        MusubiParliamentActionV1, MusubiRecoverPackageOwnersV1,
    };

    let state = blank_test_state();
    let block = new_dummy_block_at_height(
        NonZeroU64::new(PARLIAMENT_DUE_CERTIFICATE_HEIGHT).expect("due height is nonzero"),
    );
    let mut state_block = state.block(block.as_ref().header());
    let package = MusubiPackageIdV1::new(
        DataSpaceId::new(7),
        MusubiPackageScopeV1::DataspaceRoot,
        "exact-due-recovery"
            .parse()
            .expect("canonical Musubi package name"),
    );
    let action = MusubiParliamentActionV1::RecoverPackageOwners(MusubiRecoverPackageOwnersV1 {
        package: package.clone(),
        owners: vec![ALICE_ID.clone()],
        expected_revision: 1,
    });
    action.validate().expect("valid exact Musubi action");
    let fixture = {
        let mut seed = state_block.transaction();
        let fixture = seed_due_parliament_certificate(
            &mut seed,
            ProposalKind::MusubiRegistryGovernance(action.clone()),
        );
        seed.apply();
        fixture
    };

    let mut execution = state_block.transaction();
    execution.world.internal_event_buf.clear();
    assert_eq!(
        execute_due_parliament_certificate_v1(fixture.governance_attempt_id, &mut execution)
            .expect("execute exact-due Musubi authorization certificate"),
        DueParliamentCertificateExecutionV1::Applied
    );
    let proposal = execution
        .world
        .governance_proposals
        .get(&fixture.proposal_id)
        .expect("enacted Musubi authorization proposal");
    assert!(matches!(
        &proposal.kind,
        ProposalKind::MusubiRegistryGovernance(enacted) if enacted == &action
    ));
    assert!(
        execution.world.musubi_packages.get(&package).is_none(),
        "enactment authorizes the delayed Musubi mutation without bypassing its target ISI",
    );
    assert!(
        execution
            .world
            .musubi_governance_decisions
            .get(&fixture.proposal_id)
            .is_none(),
        "the enacted authorization must remain unconsumed until the delayed target ISI",
    );
    let decision = MusubiGovernanceDecisionV1 {
        decision_id: fixture.proposal_id,
        action_digest: action.action_digest(),
        enacted_at_height: PARLIAMENT_DUE_CERTIFICATE_HEIGHT,
        execute_after_height: PARLIAMENT_DUE_CERTIFICATE_HEIGHT
            .checked_add(execution.gov.min_enactment_delay.max(1))
            .expect("Musubi execution boundary does not overflow"),
    };
    decision
        .validate()
        .expect("typed delayed Musubi authorization is valid");
    assert_exact_due_parliament_effect_enacted(&execution, &fixture);
}

#[test]
fn parliament_validation_fee_policy_enacts_at_the_exact_due_height() {
    use iroha_data_model::validation_fee::{
        VALIDATION_FEE_DS_SCALE, VALIDATION_FEE_POLICY_SCHEMA_VERSION,
        VALIDATION_FEE_TREASURY_PAYOUT_EXEMPTION_CLASS, ValidationFeeChargingMode,
        ValidationFeePolicyV1,
    };
    let ds_asset_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("fees", "paynet").unwrap(),
        "fee_token".parse().unwrap(),
    );
    crate::validation_fee::tests::with_validation_fee_payout_state_at_time(
        PARLIAMENT_DUE_CERTIFICATE_HEIGHT,
        1_790_859_600_000,
        |execution, deployer, code, code_hash| {
            let mut wrapper = crate::validation_fee::tests::activate_bound_payout_runtime(
                execution,
                deployer,
                code,
                code_hash,
                0,
                ds_asset_id.clone(),
                "exact_due_policy_wrapper",
            );
            let pool = crate::validation_fee::tests::activate_bound_payout_runtime(
                execution,
                deployer,
                code,
                code_hash,
                1,
                ds_asset_id.clone(),
                "exact_due_policy_pool",
            );
            wrapper.binding.pool_vault_account_id = pool.binding.treasury_account_id;
            wrapper.binding.pool_contract_address = pool.binding.contract_address;
            wrapper.binding.pool_code_hash = pool.binding.code_hash;
            let binding = wrapper.binding;
            let payout_fixture = seed_due_parliament_certificate(
                execution,
                ProposalKind::ValidationFeePayoutLifecycle(ValidationFeePayoutLifecycleProposal {
                    proposal_operator: ALICE_ID.clone(),
                    payout_binding: binding.clone(),
                }),
            );
            assert_eq!(
                execute_due_parliament_certificate_v1(
                    payout_fixture.governance_attempt_id,
                    execution,
                )
                .unwrap(),
                DueParliamentCertificateExecutionV1::Applied
            );
            let policy = ValidationFeePolicyV1 {
                retail_schedule: iroha_data_model::validation_fee::RetailFeeScheduleV1::default(),
                effective_from_ms: 1_793_451_600_000,
                notice_published_at_ms: 1_790_859_600_000,
                schema_version: VALIDATION_FEE_POLICY_SCHEMA_VERSION,
                network_id: execution.network_id,
                policy_version: 1,
                previous_policy_hash: None,
                ds_asset_id,
                ds_scale: VALIDATION_FEE_DS_SCALE,
                fee: "0.10".parse().unwrap(),
                treasury_account_id: binding.treasury_account_id.clone(),
                charging_mode: ValidationFeeChargingMode::RetailMonthlyAllowance,

                exemption_classes: vec![VALIDATION_FEE_TREASURY_PAYOUT_EXEMPTION_CLASS.into()],
                reward_custody: binding.custody(),
            };
            validate_validation_fee_policy_proposal(&policy, execution).unwrap();
            let fixture = seed_due_parliament_certificate(
                execution,
                ProposalKind::ValidationFeePolicy(ValidationFeePolicyProposal {
                    proposal_operator: ALICE_ID.clone(),
                    policy: policy.clone(),
                }),
            );
            execution.world.internal_event_buf.clear();
            assert_eq!(
                execute_due_parliament_certificate_v1(fixture.governance_attempt_id, execution,)
                    .unwrap(),
                DueParliamentCertificateExecutionV1::Applied
            );
            let registry = validation_fee_policy_registry(execution)
                .expect("read validation-fee registry")
                .expect("automatic enactment installs validation-fee registry");
            assert_eq!(registry.registered_policies.len(), 1);
            let entry = registry
                .head()
                .expect("validation-fee registry has its enacted head");
            assert_eq!(entry.policy, policy);
            assert_eq!(
                registry.payout_policies.head().unwrap().lifecycle_seal,
                binding.lifecycle_seal().unwrap()
            );
            let proposal = execution
                .world
                .governance_proposals
                .get(&fixture.proposal_id)
                .expect("retained validation-fee policy proposal");
            let authorization = validation_fee_parliament_authorization(
                fixture.proposal_id,
                proposal,
                &fixture.certificate,
                PARLIAMENT_DUE_CERTIFICATE_HEIGHT,
            )
            .expect("derive exact typed validation-fee authorization");
            assert_eq!(entry.parliament_authorization, authorization);
            assert_eq!(&authorization.proposal_operator, &*ALICE_ID);
            assert_eq!(authorization.proposal_fingerprint, fixture.proposal_id);
            assert_eq!(
                authorization.governance_certificate_id,
                GovernanceCertificateId::derive_v1(&fixture.certificate)
            );
            assert_eq!(authorization.invariant_error(), None);
            assert_exact_due_parliament_effect_enacted(execution, &fixture);
        },
    );
}

#[test]
fn parliament_validation_fee_payout_lifecycle_enacts_at_the_exact_due_height() {
    let ds_asset_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("fees", "paynet").expect("payout fixture asset domain"),
        "fee_token".parse().expect("payout fixture asset name"),
    );
    crate::validation_fee::tests::with_original_validation_fee_payout_state_at_height(
        PARLIAMENT_DUE_CERTIFICATE_HEIGHT,
        original_world_config,
        |execution, deployer, code, code_hash| {
            let mut wrapper = crate::validation_fee::tests::activate_bound_payout_runtime(
                execution,
                deployer,
                code,
                code_hash,
                0,
                ds_asset_id.clone(),
                "exact_due_validation_fee_wrapper",
            );
            let pool = crate::validation_fee::tests::activate_bound_payout_runtime(
                execution,
                deployer,
                code,
                code_hash,
                1,
                ds_asset_id,
                "exact_due_validation_fee_pool",
            );
            wrapper.binding.pool_vault_account_id = pool.binding.treasury_account_id.clone();
            wrapper.binding.pool_contract_address = pool.binding.contract_address.clone();
            wrapper.binding.pool_code_hash = pool.binding.code_hash;
            let binding = wrapper.binding;
            assert_eq!(binding.invariant_error(), None);
            validate_validation_fee_payout_lifecycle_runtime_before_effect_install(
                &binding, execution,
            )
            .expect("exact payout topology is vacant before enactment");
            let fixture = seed_due_parliament_certificate(
                execution,
                ProposalKind::ValidationFeePayoutLifecycle(ValidationFeePayoutLifecycleProposal {
                    proposal_operator: ALICE_ID.clone(),
                    payout_binding: binding.clone(),
                }),
            );
            execution.world.internal_event_buf.clear();

            assert_eq!(
                execute_due_parliament_certificate_v1(fixture.governance_attempt_id, execution,)
                    .expect("execute exact-due validation-fee payout certificate"),
                DueParliamentCertificateExecutionV1::Applied
            );
            validate_validation_fee_payout_lifecycle_runtime(&binding, execution)
                .expect("automatic enactment installs the exact protected payout topology");
            let pool_contract_address = execution
                .world
                .contract_subject_addresses
                .get(&binding.pool_vault_account_id)
                .cloned()
                .expect("bound payout pool contract address");
            for (permission, required_holder, permission_label) in
                validation_fee_runtime_permissions(&binding, &pool_contract_address)
            {
                assert!(
                    execution
                        .world
                        .account_permissions
                        .get(&required_holder)
                        .is_some_and(|permissions| permissions.contains(&permission)),
                    "automatic enactment must install {permission_label}",
                );
            }
            let proposal = execution
                .world
                .governance_proposals
                .get(&fixture.proposal_id)
                .expect("retained validation-fee payout proposal");
            assert!(matches!(
                &proposal.kind,
                ProposalKind::ValidationFeePayoutLifecycle(payload)
                    if payload.payout_binding == binding
            ));
            let authorization = validation_fee_parliament_authorization(
                fixture.proposal_id,
                proposal,
                &fixture.certificate,
                PARLIAMENT_DUE_CERTIFICATE_HEIGHT,
            )
            .expect("derive exact payout-lifecycle Parliament authorization");
            assert_eq!(&authorization.proposal_operator, &*ALICE_ID);
            assert_eq!(authorization.proposal_fingerprint, fixture.proposal_id);
            assert_eq!(
                authorization.governance_certificate_id,
                GovernanceCertificateId::derive_v1(&fixture.certificate)
            );
            assert_eq!(authorization.invariant_error(), None);
            assert_exact_due_parliament_effect_enacted(execution, &fixture);
        },
    );
}

#[test]
fn parliament_sorafs_admission_council_enacts_only_the_exact_certified_effect() {
    use iroha_data_model::{
        isi::sorafs::SorafsProviderGovernanceActionV1,
        sorafs::provider_admission::{
            ProviderAdmissionCouncilPolicyV1, governance::ProviderAdmissionGovernanceActionV1,
        },
    };
    let state = blank_test_state();
    // Provider admission records require a real nonzero observation timestamp.
    let header = BlockHeader::new(
        NonZeroU64::new(PARLIAMENT_DUE_CERTIFICATE_HEIGHT).unwrap(),
        None,
        None,
        1,
        0,
    );
    let mut state_block = state.block(header);
    let mut seed = state_block.transaction();
    bootstrap_alice_account(&mut seed);
    let policy = ProviderAdmissionCouncilPolicyV1 {
        version: 1,
        network_id: *seed.network_id().as_bytes(),
        policy_id: [0xe1; 32],
        revision: 1,
        predecessor_policy_digest: None,
        trusted_signers: vec![ALICE_KEYPAIR.public_key().to_bytes().1.try_into().unwrap()],
        signature_threshold: 1,
        paused: false,
    };
    let kind = ProposalKind::SorafsProviderGovernance(SorafsProviderGovernanceProposal {
        action: Box::new(SorafsProviderGovernanceActionV1::Admission(
            ProviderAdmissionGovernanceActionV1::ConfigureCouncil(
                norito::encode_canonical(&policy).unwrap(),
            ),
        )),
    });
    let fixture = seed_due_parliament_certificate(&mut seed, kind);
    seed.apply();
    let mut execution = state_block.transaction();
    assert_eq!(
        execute_due_parliament_certificate_v1(fixture.governance_attempt_id, &mut execution)
            .unwrap(),
        DueParliamentCertificateExecutionV1::Applied
    );
    assert_eq!(
        crate::query::provider_admission::read_policy(execution.world()).unwrap(),
        Some(policy)
    );
    assert_exact_due_parliament_effect_enacted(&execution, &fixture);
}

#[test]
fn parliament_terminal_effect_failure_retains_atomic_rollback_and_exact_outcome() {
    use iroha_data_model::isi::sorafs::{
        EstablishSorafsProviderOwnerV1, SorafsProviderGovernanceActionV1,
    };
    let state = blank_test_state();
    let block =
        new_dummy_block_at_height(NonZeroU64::new(PARLIAMENT_DUE_CERTIFICATE_HEIGHT).unwrap());
    let mut state_block = state.block(block.as_ref().header());
    let provider_id = iroha_data_model::sorafs::capacity::ProviderId::new([0xE8; 32]);
    let fixture = {
        let mut seed = state_block.transaction();
        assert!(
            seed.world.account(&ALICE_ID).is_err(),
            "the certified target account is absent"
        );
        let fixture = seed_due_parliament_certificate(
            &mut seed,
            ProposalKind::SorafsProviderGovernance(SorafsProviderGovernanceProposal {
                action: Box::new(SorafsProviderGovernanceActionV1::Establish(
                    EstablishSorafsProviderOwnerV1 {
                        provider_id,
                        owner: ALICE_ID.clone(),
                    },
                )),
            }),
        );
        seed.apply();
        fixture
    };
    let failure_root = {
        let mut execution = state_block.transaction();
        let result =
            execute_due_parliament_certificate_v1(fixture.governance_attempt_id, &mut execution)
                .expect("missing target account is a completed effect failure, not local refusal");
        let DueParliamentCertificateExecutionV1::EffectFailed { failure_root } = result else {
            panic!("an absent account cannot receive the certified provider ownership: {result:?}");
        };
        assert!(execution.execution_deferral().is_none());
        assert!(execution.world.provider_owners.get(&provider_id).is_none());
        assert_eq!(
            execution
                .world
                .parliament_attempts
                .get(&fixture.governance_attempt_id)
                .unwrap()
                .attempt()
                .status,
            GovernanceAttemptStatusV1::Certified
        );
        // Drop exactly the failed original transaction before installing its terminal outcome.
        failure_root
    };
    assert!(
        state_block
            .world
            .provider_owners
            .get(&provider_id)
            .is_none()
    );
    let mut failure = state_block.transaction();
    record_due_parliament_execution_failure_v1(
        fixture.governance_attempt_id,
        failure_root,
        &mut failure,
    )
    .expect("record the exact deterministic failure after rollback");
    assert!(failure.execution_deferral().is_none());
    assert_eq!(
        failure
            .world
            .governance_proposals
            .get(&fixture.proposal_id)
            .unwrap()
            .status,
        crate::state::GovernanceProposalStatus::ExecutionFailed
    );
    assert_eq!(
        failure
            .world
            .parliament_attempts
            .get(&fixture.governance_attempt_id)
            .unwrap()
            .attempt()
            .status,
        GovernanceAttemptStatusV1::ExecutionFailed
    );
    assert_automatic_parliament_execution_event(
        &failure,
        &fixture,
        gov::ParliamentAutomaticExecutionOutcomeV1::ExecutionFailed(
            gov::ParliamentAutomaticExecutionFailedV1 {
                effect_preimage_hash: fixture.certificate.effect_preimage_hash,
                failure_root,
            },
        ),
    );
    assert!(failure.world.provider_owners.get(&provider_id).is_none());
    failure.apply();
    assert!(
        state_block
            .world
            .provider_owners
            .get(&provider_id)
            .is_none()
    );
    assert_eq!(
        state_block
            .world
            .governance_proposals
            .get(&fixture.proposal_id)
            .unwrap()
            .status,
        crate::state::GovernanceProposalStatus::ExecutionFailed
    );
}

include!("world_kagemusha_runtime_authority_tests.rs");
