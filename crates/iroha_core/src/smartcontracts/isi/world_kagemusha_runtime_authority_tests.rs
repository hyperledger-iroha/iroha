// Genuine loaded-runtime publication and exact-head reload regression; included in world tests.

#[test]
#[ignore = "requires two complete threshold-authenticated native production artifact bundles"]
fn parliament_runtime_preload_does_not_choose_governed_publication() {
    use crate::smartcontracts::isi::kagemusha::runtime_publication_tests::{
        OriginalBundle, check_transaction,
    };
    use iroha_data_model::{
        governance::types::KagemushaVerifierReleaseInstallProposalV1,
        kagemusha::KagemushaGovernedVerifierRegistryV1,
    };
    let root = std::path::PathBuf::from(
        std::env::var_os("IROHA_KAGEMUSHA_RUNTIME_AUTHORITY_FIXTURES")
            .expect("set exact genuine bundle directory containing current/ and standby/"),
    );
    let current = OriginalBundle::read(&root.join("current"));
    let standby = OriginalBundle::read(&root.join("standby"));
    assert_eq!(current.policy, standby.policy);
    assert_eq!(current.manifest.network_id, standby.manifest.network_id);
    assert_ne!(current.release_id(), standby.release_id());
    let mut predecessor = KagemushaGovernedVerifierRegistryV1::default();
    predecessor
        .initialize_authority_policy(current.policy.clone())
        .unwrap();
    predecessor
        .install_authenticated_release(&current.manifest, &current.receipt, &current.attestation)
        .unwrap();
    predecessor
        .activate_standby(None, current.release_id())
        .unwrap();
    let payload = KagemushaVerifierReleaseInstallProposalV1 {
        proposal_operator: ALICE_ID.clone(),
        network_id: current.manifest.network_id,
        expected_predecessor: predecessor.clone(),
        manifest: standby.manifest.clone(),
        receipt: standby.receipt.clone(),
        attestation: standby.attestation.clone(),
    };
    let expected = payload.successor().unwrap();
    let expected_bytes = norito::encode_canonical(&expected).unwrap();
    // The actual install API forbids preloading extra, not-yet-governed releases. The legal
    // branches are unloaded or exact predecessor-loaded; the latter becomes stale at publish.
    for predecessor_loaded in [false, true] {
        let world = World::default();
        {
            let mut seed = world.block();
            *seed.kagemusha_verifier_registry.get_mut() = predecessor.clone();
            seed.commit();
        }
        let state = State::new_with_chain_and_network_id_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            "generic-testnet".parse().unwrap(),
            current.manifest.network_id,
        );
        if predecessor_loaded {
            state
                .install_kagemusha_v1_runtime_verifier(
                    state.kagemusha_v1_runtime_reload_head(),
                    current.load(),
                )
                .unwrap();
        }
        for height in 1..PARLIAMENT_DUE_CERTIFICATE_HEIGHT {
            state
                .block(iroha_data_model::block::BlockHeader::new(
                    NonZeroU64::new(height).unwrap(),
                    None,
                    None,
                    0,
                    0,
                ))
                .commit_empty_block_for_testing()
                .unwrap();
        }
        let stale_head = state.kagemusha_v1_runtime_reload_head();
        let block =
            new_dummy_block_at_height(NonZeroU64::new(PARLIAMENT_DUE_CERTIFICATE_HEIGHT).unwrap());
        let mut state_block = state.block(block.as_ref().header());
        let fixture = {
            let mut seed = state_block.transaction();
            let fixture = seed_due_parliament_certificate(
                &mut seed,
                ProposalKind::KagemushaVerifierReleaseInstall(payload.clone()),
            );
            seed.apply();
            fixture
        };
        {
            let mut tx = state_block.transaction();
            assert_eq!(
                execute_due_parliament_certificate_v1(fixture.governance_attempt_id, &mut tx)
                    .unwrap(),
                DueParliamentCertificateExecutionV1::Applied
            );
            assert_eq!(tx.world.kagemusha_verifier_registry.get(), &expected);
            assert_exact_due_parliament_effect_enacted(&tx, &fixture);
            tx.apply();
        }
        state_block
            .commit_empty_block_for_testing()
            .expect("certified publication independent of local preload");
        assert_eq!(
            norito::encode_canonical(state.world.kagemusha_verifier_registry.view().get()).unwrap(),
            expected_bytes
        );
        let (authority_observation, authority_current, authority_predecessor) = {
            let pair = state
                .world
                .kagemusha_verifier_registry
                .try_committed_borrow()
                .unwrap();
            let current_bytes = norito::encode_canonical(pair.current()).unwrap();
            let predecessor_bytes = pair
                .undo()
                .as_ref()
                .map(|value| norito::encode_canonical(value).unwrap());
            assert_eq!(current_bytes, expected_bytes);
            assert_eq!(
                predecessor_bytes.as_ref().unwrap(),
                &norito::encode_canonical(&predecessor).unwrap()
            );
            (
                pair.release_observation().unwrap(),
                current_bytes,
                predecessor_bytes,
            )
        };
        // RejectAll and the genuine predecessor-loaded (now stale) cache retain
        // exactly the same canonical current and predecessor governed authority.
        let next_header = iroha_data_model::block::BlockHeader::new(
            NonZeroU64::new(PARLIAMENT_DUE_CERTIFICATE_HEIGHT + 1).unwrap(),
            None,
            None,
            0,
            0,
        );
        {
            let mut next = state.block(next_header);
            let mut tx = next.transaction();
            check_transaction(&mut tx, current.release_id(), true, false);
        }
        let installed = state.kagemusha_v1_runtime_verifier();
        assert!(
            state
                .install_kagemusha_v1_runtime_verifier(stale_head, current.load())
                .is_err(),
            "pre-publication reload head is stale"
        );
        assert!(std::sync::Arc::ptr_eq(
            &installed,
            &state.kagemusha_v1_runtime_verifier()
        ));
        assert!(
            state
                .install_kagemusha_v1_runtime_verifier(
                    state.kagemusha_v1_runtime_reload_head(),
                    current.load()
                )
                .is_err(),
            "incomplete current set cannot acquire successor authority"
        );
        assert!(std::sync::Arc::ptr_eq(
            &installed,
            &state.kagemusha_v1_runtime_verifier()
        ));
        let mut complete = current.load();
        standby.install_into(&mut complete);
        state
            .install_kagemusha_v1_runtime_verifier(
                state.kagemusha_v1_runtime_reload_head(),
                complete,
            )
            .unwrap();
        {
            let mut next = state.block(next_header);
            let mut tx = next.transaction();
            check_transaction(&mut tx, current.release_id(), true, true);
        }
        {
            let mut next = state.block(next_header);
            let mut tx = next.transaction();
            check_transaction(&mut tx, standby.release_id(), false, true);
        }
        assert!(authority_observation.try_matches_current().unwrap());
        {
            let pair = state
                .world
                .kagemusha_verifier_registry
                .try_committed_borrow()
                .unwrap();
            assert_eq!(
                norito::encode_canonical(pair.current()).unwrap(),
                authority_current
            );
            assert_eq!(
                pair.undo()
                    .as_ref()
                    .map(|value| norito::encode_canonical(value).unwrap()),
                authority_predecessor
            );
        }
        state
            .block(next_header)
            .commit_empty_block_for_testing()
            .unwrap();
        assert_eq!(
            norito::encode_canonical(state.world.kagemusha_verifier_registry.view().get()).unwrap(),
            expected_bytes
        );
    }
}
