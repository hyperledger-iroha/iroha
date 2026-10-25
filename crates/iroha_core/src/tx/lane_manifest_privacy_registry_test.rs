#[test]
fn install_lane_manifests_updates_privacy_registry() {
    let chain: ChainId = "lane-privacy-registry".parse().unwrap();
    let world = World::default();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_with_chain(world, kura, query_handle, chain);
    let commitment = LanePrivacyCommitment::merkle(
        LaneCommitmentId::new(9),
        MerkleCommitment::from_root_bytes([0x11; 32], 8),
    );
    let status = LaneManifestStatus {
        lane: TestLaneId::SINGLE,
        alias: "private".to_string(),
        dataspace: TestDataSpaceId::UNIVERSAL,
        visibility: LaneVisibility::Public,
        storage: LaneStorageProfile::CommitmentOnly,
        governance: None,
        manifest_path: Some(PathBuf::from("/tmp/privacy.json")),
        governance_rules: None,
        privacy_commitments: vec![commitment],
    };
    let mut statuses = BTreeMap::new();
    statuses.insert(TestLaneId::SINGLE, status);
    let registry = Arc::new(LaneManifestRegistry::from_statuses(statuses));
    state.install_lane_manifests_for_testing(&registry);
    let snapshot = state.lane_privacy_registry.read().clone();
    assert!(!snapshot.is_empty(), "privacy registry should not be empty");
    assert!(
        snapshot.lane(TestLaneId::SINGLE).is_some(),
        "privacy registry should contain lane entry"
    );
}

#[test]
fn transaction_privacy_admission_uses_manifest_when_cache_is_stale() {
    let chain: ChainId = "lane-privacy-cache-independent".parse().unwrap();
    let (world, authority, keypair) = world_with_authority("wonderland");
    let state = State::new_with_chain(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
        chain,
    );
    let commitment_id = LaneCommitmentId::new(17);
    let witness = iroha_crypto::privacy::MerkleWitness::new(
        [0x12; 32],
        MerkleProof::from_audit_path_bytes(0, vec![[0x34; 32]]),
    );
    let root = witness.implied_root(8).unwrap();
    let commitment = LanePrivacyCommitment::merkle(commitment_id, MerkleCommitment::new(root, 8));
    let mut status = LaneManifestStatus {
        lane: TestLaneId::SINGLE,
        alias: "private".to_owned(),
        dataspace: TestDataSpaceId::UNIVERSAL,
        visibility: LaneVisibility::Public,
        storage: LaneStorageProfile::FullReplica,
        governance: None,
        manifest_path: Some(PathBuf::from("/tmp/privacy-cache-independent.json")),
        governance_rules: None,
        privacy_commitments: vec![commitment],
    };
    let manifests = Arc::new(LaneManifestRegistry::from_statuses(BTreeMap::from([(
        TestLaneId::SINGLE,
        status.clone(),
    )])));
    let backend = Ident::from_str("halo2/ipa").unwrap();
    let mut attachment = ProofAttachment::new_ref(
        backend.clone(),
        ProofBox::new(backend.clone(), vec![0xAA]),
        VerifyingKeyId::new(backend, "privacy-cache-independent"),
    );
    attachment.lane_privacy = Some(LanePrivacyProof {
        commitment_id,
        witness: LanePrivacyWitness::Merkle(LanePrivacyMerkleWitness {
            leaf: *witness.leaf(),
            proof: witness.proof().clone(),
        }),
    });
    let tx = TransactionBuilder::new(
        test_network_id(),
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "privacy admission".into())])
    .with_proof_attachments(vec![attachment])
    .expect("valid proof carriers")
    .sign(keypair.private_key());

    {
        let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
        block.lane_manifests = Arc::clone(&manifests);
        block.lane_privacy_registry = LanePrivacyRegistry::empty();
        let stx = block.transaction();
        let assignment = single_lane_assignment(&stx.nexus.dataspace_catalog);
        assert!(
            super::enforce_lane_policies(&tx, &stx, &assignment).is_ok(),
            "empty cached privacy cannot reject a proof authorized by the manifest"
        );
    }

    status.privacy_commitments.clear();
    let revoked = Arc::new(LaneManifestRegistry::from_statuses(BTreeMap::from([(
        TestLaneId::SINGLE,
        status,
    )])));
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
    block.lane_manifests = revoked;
    block.lane_privacy_registry =
        LanePrivacyRegistry::try_from_manifest_registry(&manifests, &state.ivm_execution_budget())
            .expect("original fixture pool admits stale graph");
    let stx = block.transaction();
    let assignment = single_lane_assignment(&stx.nexus.dataspace_catalog);
    let error = super::enforce_lane_policies(&tx, &stx, &assignment).unwrap_err();
    assert!(matches!(
        error,
        ExecutionAttemptError::Rejected(TransactionRejectionReason::Validation(
            ValidationFail::NotPermitted(message),
        )) if message.contains("lane privacy proof rejected")
    ));
}

#[test]
fn transaction_privacy_policy_refuses_original_pool_then_retries_same_input() {
    let (world, authority, keypair) = world_with_authority("wonderland");
    let state = State::new(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let mut status = lane_manifest_status(TestLaneId::SINGLE, None, GovernanceRules::default());
    status.privacy_commitments = vec![LanePrivacyCommitment::merkle(
        LaneCommitmentId::new(7),
        MerkleCommitment::from_root_bytes([0x11; 32], 4),
    )];
    let manifests = Arc::new(LaneManifestRegistry::from_statuses(BTreeMap::from([(
        TestLaneId::SINGLE,
        status,
    )])));
    let tx = TransactionBuilder::new(
        test_network_id(),
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "privacy budget retry".into())])
    .sign(keypair.private_key());
    let wire = tx.encode_wire_v1().unwrap();
    let generation = state.state_view_generation();
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
    block.lane_manifests = Arc::clone(&manifests);
    block.lane_privacy_registry = LanePrivacyRegistry::empty();
    let stx = block.transaction();
    let assignment = single_lane_assignment(&stx.nexus.dataspace_catalog);
    let budget = stx.execution_budget();
    assert!(budget.same_pool(&state.ivm_execution_budget()));
    let retained = budget.reserved_bytes();
    let occupied = budget
        .try_reserve_bytes(budget.limit_bytes() - retained)
        .unwrap();
    let error = super::enforce_lane_policies(&tx, &stx, &assignment).unwrap_err();
    let ExecutionAttemptError::Deferred(original) = error else {
        panic!("unfinished privacy projection became a completed transaction rejection");
    };
    assert!(matches!(
        original.allocation_refusal(),
        Some(iroha_allocation::AllocationRefusal::Capacity { .. })
    ));
    assert!(Arc::ptr_eq(&stx.lane_manifests, &manifests));
    assert!(stx.lane_privacy_registry.is_empty());
    assert_eq!(state.state_view_generation(), generation);
    assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
    drop(occupied);
    assert_eq!(budget.reserved_bytes(), retained);
    super::enforce_lane_policies(&tx, &stx, &assignment)
        .expect("the same signed input retries against its original manifest and pool");
    assert_eq!(
        budget.reserved_bytes(),
        retained,
        "temporary projection refunds at the policy boundary"
    );
    assert_eq!(tx.encode_wire_v1().unwrap(), wire);
    assert!(
        stx.lane_privacy_registry.is_empty(),
        "policy does not install its disposable projection"
    );
    assert_eq!(state.state_view_generation(), generation);
}

#[test]
fn stateful_privacy_refusal_retains_original_cause_and_forbids_settlement() {
    let (world, authority, keypair) = world_with_authority("wonderland");
    let state = State::new(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let mut status = lane_manifest_status(TestLaneId::SINGLE, None, GovernanceRules::default());
    status.privacy_commitments = vec![LanePrivacyCommitment::merkle(
        LaneCommitmentId::new(7),
        MerkleCommitment::from_root_bytes([0x11; 32], 4),
    )];
    let manifests = Arc::new(LaneManifestRegistry::from_statuses(BTreeMap::from([(
        TestLaneId::SINGLE,
        status,
    )])));
    let mut metadata = Metadata::default();
    metadata.insert("expires_at_height".parse().unwrap(), Json::new(5_u64));
    metadata.insert("tx_sequence".parse().unwrap(), Json::new(1_u64));
    let tx = TransactionBuilder::new(
        test_network_id(),
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_metadata(metadata)
    .with_instructions([Log::new(Level::INFO, "original privacy cause".into())])
    .sign(keypair.private_key());
    let generation = state.state_view_generation();
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
    block.lane_manifests = Arc::clone(&manifests);
    block.lane_privacy_registry = LanePrivacyRegistry::empty();
    let mut overlay = block.transaction();
    let native = crate::queue::RoutingDecision::new(TestLaneId::SINGLE, TestDataSpaceId::UNIVERSAL);
    let accepted = AcceptedTransaction::new_unchecked(Cow::Borrowed(&tx));
    let policy_route = CapturedNetworkPolicyRoute::for_component(&accepted, &overlay, native)
        .expect("capture the actual physical policy before exhausting its original pool");
    let budget = overlay.execution_budget();
    let retained = budget.reserved_bytes();
    let occupied = budget
        .try_reserve_bytes(budget.limit_bytes() - retained)
        .unwrap();
    let assignment = single_lane_assignment(&overlay.nexus.dataspace_catalog);
    let expected = super::enforce_lane_policies(&tx, &overlay, &assignment).unwrap_err();
    let ExecutionAttemptError::Deferred(expected) = expected else {
        panic!("the actual privacy owner must refuse at this cut");
    };
    let Some(iroha_allocation::AllocationRefusal::Capacity {
        requested_bytes: expected_bytes,
        release: expected_release,
        ..
    }) = expected.allocation_refusal()
    else {
        panic!("actual owner did not retain the original capacity source");
    };
    let error =
        StateBlock::validate_stateful_admission(&tx, &mut overlay, native, &policy_route, None)
            .expect_err("unfinished privacy acquisition must stop admission");
    assert!(matches!(error, TransactionRejectionReason::Validation(_)));
    assert!(overlay.is_execution_deferred());
    let original = overlay
        .take_execution_deferral()
        .expect("caller retains the actual refusal");
    let Some(iroha_allocation::AllocationRefusal::Capacity {
        requested_bytes,
        release,
        ..
    }) = original.allocation_refusal()
    else {
        panic!("stateful admission erased the actual allocation refusal");
    };
    assert_eq!(
        requested_bytes, expected_bytes,
        "the policy owner is the refused demand"
    );
    assert_eq!(
        release, expected_release,
        "actual owner retains the exact original pool observation"
    );
    let waiter_pool = iroha_allocation::AllocationBudget::new(4096);
    let mut prepaid = waiter_pool
        .try_reserve(iroha_allocation::release::ReleaseRegistration::allocation_layout())
        .unwrap();
    let mut registration =
        iroha_allocation::release::ReleaseRegistration::from_reservation(&mut prepaid).unwrap();
    struct Noop;
    impl std::task::Wake for Noop {
        fn wake(self: Arc<Self>) {}
    }
    let waker = std::task::Waker::from(Arc::new(Noop));
    let mut context = std::task::Context::from_waker(&waker);
    assert!(registration.poll_wait(release, &mut context).is_pending());
    assert!(
        overlay.is_execution_deferred(),
        "moving custody cannot allow settlement"
    );
    assert!(overlay.take_execution_deferral().is_none());
    assert!(overlay.ensure_execution_ready().is_err());
    assert!(overlay.world.tx_sequences.get(&authority).is_none());
    assert_eq!(state.state_view_generation(), generation);
    drop(occupied);
    assert!(registration.poll_wait(release, &mut context).is_ready());
    drop(registration);
    drop(prepaid);
    assert_eq!(waiter_pool.reserved_bytes(), 0);
    assert_eq!(budget.reserved_bytes(), retained);
}
