#[test]
fn installing_manifests_populates_privacy_registry() {
    let fixture_budget = iroha_allocation::AllocationBudget::new(1024 * 1024);
    use iroha_crypto::privacy::{LaneCommitmentId, LanePrivacyCommitment, MerkleCommitment};
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Queue::test(config_factory(), &time_source);
    let mut statuses = BTreeMap::new();
    statuses.insert(
        LaneId::SINGLE,
        LaneManifestStatus {
            lane: LaneId::SINGLE,
            alias: "default".to_string(),
            dataspace: DataSpaceId::UNIVERSAL,
            visibility: LaneVisibility::Public,
            storage: LaneStorageProfile::CommitmentOnly,
            governance: None,
            manifest_path: Some(PathBuf::from("/tmp/privacy.json")),
            governance_rules: None,
            privacy_commitments: vec![LanePrivacyCommitment::merkle(
                LaneCommitmentId::new(7),
                MerkleCommitment::from_root_bytes([0x11; 32], 4),
            )],
        },
    );
    let manifests = Arc::new(LaneManifestRegistry::from_statuses(statuses));
    queue
        .install_lane_manifests_for_testing(&manifests, &fixture_budget)
        .expect("original fixture pool admits privacy projection");
    let registry = queue.lane_privacy_registry();
    assert!(
        registry
            .lane(LaneId::SINGLE)
            .expect("lane registry entry")
            .get(LaneCommitmentId::new(7))
            .is_some()
    );
}
#[tokio::test]
async fn governance_manifest_allows_multisig_propose_envelope_from_live_signer() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let signer_key = checked_random_queue_keypair();
    let signer_id = AccountId::new(signer_key.public_key().clone());
    let cosigner_key = checked_random_queue_keypair();
    let cosigner_id = AccountId::new(cosigner_key.public_key().clone());
    let validator_key = checked_random_queue_keypair();
    let validator_id = AccountId::new(validator_key.public_key().clone());
    let multisig_key = checked_random_queue_keypair();
    let multisig_id = AccountId::new(multisig_key.public_key().clone());
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("static domain");
    let mut multisig_metadata = Metadata::default();
    multisig_metadata.insert(
        crate::smartcontracts::isi::multisig::spec_key(),
        Json::new(MultisigSpec {
            signatories: BTreeMap::from([(signer_id.clone(), 1), (cosigner_id.clone(), 1)]),
            quorum: nonzero!(2_u16),
            transaction_ttl_ms: nonzero!(
                iroha_executor_data_model::isi::multisig::DEFAULT_MULTISIG_TTL_MS
            ),
        }),
    );
    let domain = Domain::new(domain_id.clone()).build(&signer_id);
    let signer = Account::new(signer_id.clone()).build(&signer_id);
    let cosigner = Account::new(cosigner_id.clone()).build(&cosigner_id);
    let validator = Account::new(validator_id.clone()).build(&validator_id);
    let multisig = Account::new(multisig_id.clone())
        .with_metadata(multisig_metadata)
        .build(&multisig_id);
    let world = World::with([domain], [signer, cosigner, validator, multisig], []);
    // Explicit Global metadata belongs to this admission component fixture;
    // it does not authenticate signed genesis, native history or publication.
    let mut parameters = crate::state::parameter_cell_block_for_testing(&world)
        .expect("funded original parameter-only fixture block");
    crate::state::set_parameter_cell_for_testing(
        &mut parameters,
        crate::sumeragi::lanes::routing::test_support::metadata(
            iroha_data_model::block::consensus::SumeragiRootScope::Global,
        ),
        world.sccp_committee_generations.allocation_budget(),
    )
    .expect("valid fixture parameters");
    parameters.commit();
    let mut state = State::new(
        world_with_explicit_queue_fees(world, zero_rate_routing_fees()),
        kura,
        query_handle,
    );
    {
        let nexus = state.nexus.get_mut();
        nexus.lane_config =
            iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
    }
    let state = Arc::new(state);
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let mut statuses = BTreeMap::new();
    let rules = GovernanceRules {
        validators: vec![validator_id.clone()],
        quorum: Some(3),
        ..GovernanceRules::default()
    };
    let status = LaneManifestStatus {
        lane: LaneId::SINGLE,
        alias: "centralbank".to_string(),
        dataspace: DataSpaceId::UNIVERSAL,
        visibility: LaneVisibility::Public,
        storage: LaneStorageProfile::FullReplica,
        governance: Some("parliament".to_string()),
        manifest_path: Some(PathBuf::from("/tmp/manifest.json")),
        governance_rules: Some(rules),
        privacy_commitments: Vec::new(),
    };
    statuses.insert(LaneId::SINGLE, status);
    let manifests = Arc::new(LaneManifestRegistry::from_statuses(statuses));
    queue
        .reconfigure_nexus_with_state(&state.nexus_snapshot(), &state, None)
        .expect("original State pool admits Queue privacy projection");
    queue
        .install_lane_manifests_for_testing(&manifests, &state.ivm_execution_budget())
        .expect("original fixture pool admits privacy projection");
    let tx = accepted_tx_with(
        signer_id,
        &signer_key,
        &time_source,
        vec![InstructionBox::from(MultisigPropose::new(
            multisig_id,
            iroha_executor_data_model::isi::multisig::MultisigProposalNonce::new(
                *iroha_crypto::HashOf::new(&vec![InstructionBox::from(Log::new(
                    Level::INFO,
                    "multisig envelope".into(),
                ))])
                .as_ref(),
            )
            .unwrap(),
            vec![InstructionBox::from(Log::new(
                Level::INFO,
                "multisig envelope".into(),
            ))],
            None,
        ))],
        Metadata::default(),
    );
    queue
        .push(tx, state.view())
        .expect("multisig propose envelopes from live signers should bypass lane-validator gating");
}
#[allow(clippy::too_many_lines)]
#[tokio::test]
async fn lane_compliance_policy_blocks_transactions() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let metrics = Arc::new(Metrics::default());
    #[cfg(feature = "telemetry")]
    let state = Arc::new(State::with_telemetry(
        world_with_test_domains(),
        kura.clone(),
        query_handle.clone(),
        StateTelemetry::new(metrics.clone(), true),
    ));
    #[cfg(not(feature = "telemetry"))]
    let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let (allowed_id, allowed_keypair) = gen_account_in("wonderland");
    register_test_authority(&state, &allowed_id);
    let (denied_id, denied_keypair) = gen_account_in("wonderland");
    register_test_authority(&state, &denied_id);
    let (confidential_id, confidential_keypair) = gen_account_in("wonderland");
    register_test_authority(&state, &confidential_id);
    // Build a Merkle commitment + witness for privacy-gated policies
    let first_leaf = [0x01_u8; 32];
    let proof = MerkleProof::from_audit_path_bytes(0, vec![[0x02_u8; 32]]);
    let merkle_root = MerkleWitness::new(first_leaf, proof.clone())
        .implied_root(8)
        .expect("valid lane proof must produce a root");
    let merkle_commitment = LanePrivacyCommitment::merkle(
        LaneCommitmentId::new(9),
        MerkleCommitment::new(merkle_root, 8),
    );
    let queue_inner = Queue::test(config_factory(), &time_source);
    let policy = LaneCompliancePolicy {
        id: LaneCompliancePolicyId::new(Hash::prehashed([0xAA; 32])),
        version: 1,
        lane_id: LaneId::SINGLE,
        dataspace_id: DataSpaceId::UNIVERSAL,
        jurisdiction: JurisdictionSet::default(),
        deny: vec![LaneComplianceRule {
            selector: ParticipantSelector {
                account: Some(denied_id.clone()),
                ..ParticipantSelector::default()
            },
            reason_code: Some("denied account".to_string()),
            jurisdiction_override: JurisdictionSet::default(),
        }],
        allow: vec![
            LaneComplianceRule {
                selector: ParticipantSelector {
                    account: Some(allowed_id.clone()),
                    ..ParticipantSelector::default()
                },
                reason_code: Some("allowed account".to_string()),
                jurisdiction_override: JurisdictionSet::default(),
            },
            LaneComplianceRule {
                selector: ParticipantSelector {
                    account: Some(confidential_id.clone()),
                    privacy_commitments_any_of: vec![LaneCommitmentId::new(9)],
                    ..ParticipantSelector::default()
                },
                reason_code: Some("confidential allowed".to_string()),
                jurisdiction_override: JurisdictionSet::default(),
            },
        ],
        transfer_limits: Vec::new(),
        audit_controls: AuditControls::default(),
        metadata: Metadata::default(),
    };
    let engine = LaneComplianceEngine::from_policies(vec![policy], false).expect("engine");
    *queue_inner.lane_compliance.write() = Some(Arc::new(engine));
    let queue = Arc::new(queue_inner);
    let mut statuses = BTreeMap::new();
    statuses.insert(
        LaneId::SINGLE,
        LaneManifestStatus {
            lane: LaneId::SINGLE,
            alias: "confidential".to_string(),
            dataspace: DataSpaceId::UNIVERSAL,
            visibility: LaneVisibility::Public,
            storage: LaneStorageProfile::CommitmentOnly,
            governance: None,
            manifest_path: Some(PathBuf::from("/tmp/privacy.json")),
            governance_rules: None,
            privacy_commitments: vec![merkle_commitment],
        },
    );
    let manifests = Arc::new(LaneManifestRegistry::from_statuses(statuses));
    queue
        .install_lane_manifests_for_testing(&manifests, &state.ivm_execution_budget())
        .expect("original fixture pool admits privacy projection");
    *queue.lane_privacy_registry.write() = LanePrivacyRegistry::empty();
    assert!(
        queue.lane_privacy_registry().is_empty(),
        "test uses a stale cache"
    );
    let denied_tx = accepted_tx_by(denied_id.clone(), &denied_keypair, &time_source);
    let err = queue
        .push(denied_tx, state.view())
        .expect_err("denied account should be rejected");
    assert!(matches!(err.err, Error::LaneComplianceDenied { .. }));
    let allowed_tx = accepted_tx_by(allowed_id.clone(), &allowed_keypair, &time_source);
    queue
        .push(allowed_tx, state.view())
        .expect("allowed account should pass");
    // Confidential account is denied without a privacy witness.
    let confidential_tx =
        accepted_tx_by(confidential_id.clone(), &confidential_keypair, &time_source);
    let err = queue
        .push(confidential_tx, state.view())
        .expect_err("missing privacy proof should be rejected");
    assert!(matches!(
        err.err,
        Error::LanePrivacyProofRejected { .. } | Error::LaneComplianceDenied { .. }
    ));
    // A valid Merkle witness satisfies the privacy policy, but its advisory
    // transaction-proof carrier has no authoritative execution consumer.
    let proof_attachment = ProofAttachment {
        backend: Ident::from_str("halo2/ipa").expect("ident"),
        proof: ProofBox {
            backend: Ident::from_str("halo2/ipa").expect("ident"),
            bytes: vec![0xAA],
        },
        vk_ref: iroha_data_model::proof::VerifyingKeyId::new("halo2/ipa", "privacy"),
        vk_commitment: None,
        envelope_hash: None,
        lane_privacy: Some(LanePrivacyProof {
            commitment_id: LaneCommitmentId::new(9),
            witness: LanePrivacyWitness::Merkle(LanePrivacyMerkleWitness {
                leaf: first_leaf,
                proof,
            }),
        }),
    };
    let attachments = vec![proof_attachment];
    let confidential_tx = signed_tx_with_attachments(
        confidential_id.clone(),
        &confidential_keypair,
        &time_source,
        vec![sample_unregister_instruction()],
        Metadata::default(),
        Some(attachments.clone()),
    );
    confidential_tx
        .validate_signed_attachments_v1()
        .expect("canonical signed carrier and statement bindings");
    confidential_tx
        .verify_signature()
        .expect("genuine signature");
    let original_wire = confidential_tx.encode_wire_v1().unwrap();
    assert!(matches!(
        try_accept_signed_queue_tx(confidential_tx.clone(), &time_source),
        Err(crate::tx::AcceptTransactionFail::SignedAttachments(
            iroha_data_model::transaction::signed_attachments::SignedAttachmentErrorV1::UnsupportedOperation { action_index: 0 }
        ))
    ));
    assert_eq!(confidential_tx.encode_wire_v1().unwrap(), original_wire);

    // Exercise the real manifest snapshot, privacy verifier and compliance
    // engine directly. This is policy-component evidence, not queue admission
    // of the unsupported carrier and not an unchecked AcceptedTransaction.
    let privacy_proofs = confidential_tx
        .lane_privacy_proofs_v1()
        .expect("read the original signed privacy witness");
    let (snapshot, _, _) = queue
        .lane_manifest_admission_snapshot(
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
            &BTreeSet::from([LaneId::SINGLE]),
        )
        .expect("actual current manifest cut");
    let registry =
        LanePrivacyRegistry::try_from_manifest_registry(&snapshot, &state.ivm_execution_budget())
            .expect("original fixture pool admits the stale privacy graph");
    let verified = verify_lane_privacy_proofs(&registry, LaneId::SINGLE, &privacy_proofs)
        .expect("manifest-backed privacy proof passes despite an empty local cache");
    assert_eq!(verified, BTreeSet::from([LaneCommitmentId::new(9)]));
    assert!(matches!(
        verify_lane_privacy_proofs(
            &queue.lane_privacy_registry(),
            LaneId::SINGLE,
            &privacy_proofs,
        ),
        Err(crate::interlane::LanePrivacyRegistryError::LaneMissing { lane_id })
            if lane_id == LaneId::SINGLE
    ));
    let engine = queue.lane_compliance.read().clone().expect("policy engine");
    let mut context =
        LaneComplianceContext::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL, &confidential_id);
    context.lane_privacy_registry = Some(registry);
    context.verified_privacy_commitments = &verified;
    assert!(matches!(
        engine.evaluate(&context),
        LaneComplianceEvaluation::Allowed(_)
    ));

    let mut revoked_status = manifests.status(LaneId::SINGLE).unwrap().clone();
    revoked_status.privacy_commitments = vec![LanePrivacyCommitment::merkle(
        LaneCommitmentId::new(10),
        MerkleCommitment::new(merkle_root, 8),
    )];
    let revoked = Arc::new(LaneManifestRegistry::from_statuses(BTreeMap::from([(
        LaneId::SINGLE,
        revoked_status,
    )])));
    queue
        .install_lane_manifests_for_testing(&revoked, &state.ivm_execution_budget())
        .expect("original fixture pool admits privacy projection");
    *queue.lane_privacy_registry.write() =
        LanePrivacyRegistry::try_from_manifest_registry(&manifests, &state.ivm_execution_budget())
            .expect("original fixture pool admits the stale privacy graph");
    let mut revoked_metadata = Metadata::default();
    revoked_metadata.insert(
        "revoked_privacy_test".parse().expect("metadata key"),
        iroha_primitives::json::Json::new(true),
    );
    let unregistered_proof = signed_tx_with_attachments(
        confidential_id.clone(),
        &confidential_keypair,
        &time_source,
        vec![sample_unregister_instruction()],
        revoked_metadata,
        Some(attachments),
    );
    let revoked_wire = unregistered_proof.encode_wire_v1().unwrap();
    assert!(matches!(
        try_accept_signed_queue_tx(unregistered_proof.clone(), &time_source),
        Err(crate::tx::AcceptTransactionFail::SignedAttachments(
            iroha_data_model::transaction::signed_attachments::SignedAttachmentErrorV1::UnsupportedOperation { action_index: 0 }
        ))
    ));
    let privacy_proofs = unregistered_proof
        .lane_privacy_proofs_v1()
        .expect("same canonical witness under the re-signed metadata");
    let (snapshot, _, _) = queue
        .lane_manifest_admission_snapshot(
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
            &BTreeSet::from([LaneId::SINGLE]),
        )
        .expect("actual manifest cut after revocation");
    let registry =
        LanePrivacyRegistry::try_from_manifest_registry(&snapshot, &state.ivm_execution_budget())
            .expect("original fixture pool admits the stale privacy graph");
    assert_eq!(
        verify_lane_privacy_proofs(&registry, LaneId::SINGLE, &privacy_proofs),
        Err(
            crate::interlane::LanePrivacyRegistryError::UnknownCommitment {
                lane_id: LaneId::SINGLE,
                commitment_id: LaneCommitmentId::new(9),
            }
        ),
        "the old commitment must fail against the new manifest despite the stale cache"
    );
    assert_eq!(
        verify_lane_privacy_proofs(
            &queue.lane_privacy_registry(),
            LaneId::SINGLE,
            &privacy_proofs,
        )
        .expect("the forged stale cache still contains the revoked commitment"),
        BTreeSet::from([LaneCommitmentId::new(9)])
    );
    let no_verified_commitments = BTreeSet::new();
    let mut context =
        LaneComplianceContext::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL, &confidential_id);
    context.lane_privacy_registry = Some(registry);
    context.verified_privacy_commitments = &no_verified_commitments;
    assert!(matches!(
        engine.evaluate(&context),
        LaneComplianceEvaluation::Denied(_)
    ));
    assert_eq!(unregistered_proof.encode_wire_v1().unwrap(), revoked_wire);
    unregistered_proof
        .verify_signature()
        .expect("original revoked carrier remains unchanged");
}

fn funded_privacy_manifest_fixture() -> LaneManifestRegistryHandle {
    use iroha_data_model::nexus::{
        NativeLaneManifestV1, NativeLaneMerkleCommitmentV1, NativeLanePrivacyCommitmentV1,
    };

    // Lifecycle rebinding consumes the frozen source, so the component fixture
    // authors canonical manifest bytes before its temporary directory is removed.
    let nexus = Nexus::default();
    let directory = tempdir().expect("privacy manifest source directory");
    let alias = &nexus.lane_catalog.lanes()[0].alias;
    let manifest = NativeLaneManifestV1 {
        lane: Some(alias.clone()),
        version: Some(NativeLaneManifestV1::VERSION),
        privacy_commitments: Some(vec![NativeLanePrivacyCommitmentV1 {
            id: Some(7),
            scheme: Some("merkle".to_owned()),
            merkle: Some(NativeLaneMerkleCommitmentV1 {
                root: Some("11".repeat(32)),
                max_depth: Some(4),
            }),
        }]),
        ..NativeLaneManifestV1::default()
    };
    fs::write(
        directory.path().join(format!("{alias}.manifest.json")),
        norito::json::to_vec(&manifest).expect("canonical privacy manifest"),
    )
    .expect("write privacy manifest source");
    let mut registry = nexus.registry;
    registry.manifest_directory = Some(directory.path().to_path_buf());
    Arc::new(LaneManifestRegistry::from_config(
        &nexus.lane_catalog,
        &nexus.governance,
        &registry,
    ))
}

#[test]
fn funded_privacy_manifest_retains_frozen_source_and_original_commitment() {
    let manifests = funded_privacy_manifest_fixture();
    manifests
        .validate_materialized_source_projection()
        .expect("privacy fixture retains its canonical source");
    let nexus = Nexus::default();
    let rebound = manifests.rebind(&nexus.lane_catalog, &nexus.governance);
    rebound
        .validate_materialized_authority_for_catalog(&nexus.lane_catalog, &nexus.governance)
        .expect("privacy fixture can rebind after source directory removal");
    let status = rebound.status(LaneId::SINGLE).expect("privacy lane");
    assert!(
        !status
            .manifest_path
            .as_ref()
            .expect("original path")
            .exists()
    );
    assert_eq!(
        status.privacy_commitments,
        vec![LanePrivacyCommitment::merkle(
            LaneCommitmentId::new(7),
            MerkleCommitment::from_root_bytes([0x11; 32], 4),
        )]
    );
}

#[test]
fn admission_privacy_cache_requires_original_pool_and_preserves_owner_on_refusal() {
    let (_clock, time) = TimeSource::new_mock(Duration::default());
    let queue = Queue::test(config_factory(), &time);
    let original_pool = iroha_allocation::AllocationBudget::new(1024 * 1024);
    let sibling_pool = iroha_allocation::AllocationBudget::new(original_pool.limit_bytes());
    let manifests = funded_privacy_manifest_fixture();
    let first = queue
        .admission_privacy_registry_for(
            &manifests,
            &original_pool,
            &mut QueuePrivacyRetirements::default(),
        )
        .unwrap();
    let original_bytes = original_pool.reserved_bytes();
    assert!(original_bytes > 0);
    assert!(first.belongs_to(&original_pool));
    assert!(!first.belongs_to(&sibling_pool));
    assert!(LanePrivacyRegistry::ptr_eq(
        &first,
        &queue
            .admission_privacy_registry_for(
                &manifests,
                &original_pool,
                &mut QueuePrivacyRetirements::default()
            )
            .unwrap(),
    ));
    assert_eq!(original_pool.reserved_bytes(), original_bytes);

    let occupied = sibling_pool
        .try_reserve_bytes(sibling_pool.limit_bytes())
        .unwrap();
    let refused = queue
        .admission_privacy_registry_for(
            &manifests,
            &sibling_pool,
            &mut QueuePrivacyRetirements::default(),
        )
        .expect_err("same manifest identity cannot spend a different pool's cached owner");
    assert!(matches!(
        refused.allocation_refusal(),
        Some(iroha_allocation::AllocationRefusal::Capacity { .. })
    ));
    {
        let cache = queue.admission_privacy_registry.lock();
        let (source, retained) = cache.as_ref().unwrap();
        assert!(Arc::ptr_eq(source, &manifests));
        assert!(LanePrivacyRegistry::ptr_eq(retained, &first));
    }
    assert_eq!(original_pool.reserved_bytes(), original_bytes);
    assert_eq!(sibling_pool.reserved_bytes(), sibling_pool.limit_bytes());
    drop(occupied);
    let retried = queue
        .admission_privacy_registry_for(
            &manifests,
            &sibling_pool,
            &mut QueuePrivacyRetirements::default(),
        )
        .unwrap();
    assert!(retried.belongs_to(&sibling_pool));
    assert!(!retried.belongs_to(&original_pool));
    assert!(!LanePrivacyRegistry::ptr_eq(&first, &retried));
    assert_eq!(first, retried);
    assert_eq!(
        original_pool.reserved_bytes(),
        original_bytes,
        "old reader retains its original charge"
    );
    drop(first);
    assert_eq!(original_pool.reserved_bytes(), 0);
    let retry_bytes = sibling_pool.reserved_bytes();
    assert!(retry_bytes > 0);
    drop(queue);
    assert_eq!(
        sibling_pool.reserved_bytes(),
        retry_bytes,
        "last sampled owner still retains its graph"
    );
    drop(retried);
    assert_eq!(sibling_pool.reserved_bytes(), 0);
}

#[test]
fn queue_privacy_install_refusal_keeps_manifest_cache_and_retained_graph() {
    let (_clock, time) = TimeSource::new_mock(Duration::default());
    let queue = Queue::test(config_factory(), &time);
    let budget = iroha_allocation::AllocationBudget::new(1024 * 1024);
    let original = funded_privacy_manifest_fixture();
    queue
        .install_lane_manifests_for_testing(&original, &budget)
        .unwrap();
    let installed = queue.lane_privacy_registry();
    let cached = queue
        .admission_privacy_registry_for(&original, &budget, &mut QueuePrivacyRetirements::default())
        .unwrap();
    let retained = budget.reserved_bytes();
    assert!(retained > 0);
    let replacement = funded_privacy_manifest_fixture();
    let occupied = budget
        .try_reserve_bytes(budget.limit_bytes() - retained)
        .unwrap();
    let refused = queue
        .install_lane_manifests_for_testing(&replacement, &budget)
        .unwrap_err();
    assert!(matches!(
        refused.allocation_refusal(),
        Some(iroha_allocation::AllocationRefusal::Capacity { .. })
    ));
    assert!(Arc::ptr_eq(&queue.lane_manifests.read(), &original));
    assert!(LanePrivacyRegistry::ptr_eq(
        &queue.lane_privacy_registry(),
        &installed
    ));
    {
        let cache = queue.admission_privacy_registry.lock();
        let (source, retained) = cache.as_ref().unwrap();
        assert!(Arc::ptr_eq(source, &original));
        assert!(LanePrivacyRegistry::ptr_eq(retained, &cached));
    }
    assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
    drop(occupied);
    assert_eq!(budget.reserved_bytes(), retained);
    queue
        .install_lane_manifests_for_testing(&replacement, &budget)
        .unwrap();
    assert!(Arc::ptr_eq(&queue.lane_manifests.read(), &replacement));
    assert!(!LanePrivacyRegistry::ptr_eq(
        &queue.lane_privacy_registry(),
        &installed
    ));
    assert_eq!(queue.lane_privacy_registry(), installed);
    drop(queue);
    drop(installed);
    drop(cached);
    assert_eq!(budget.reserved_bytes(), 0);
}

struct QueuePrivacyRefundObserver {
    queue: std::sync::Weak<Queue>,
    expected: LaneManifestRegistryHandle,
    wakes: AtomicUsize,
}

impl std::task::Wake for QueuePrivacyRefundObserver {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        let queue = self.queue.upgrade().expect("live Queue during publication");
        let manifests = queue
            .lane_manifests
            .try_write()
            .expect("refund callback follows the manifest writer");
        let privacy = queue
            .lane_privacy_registry
            .try_write()
            .expect("refund callback follows the privacy writer");
        let cache = queue
            .admission_privacy_registry
            .try_lock()
            .expect("refund callback follows the admission cache writer");
        assert!(Arc::ptr_eq(&manifests, &self.expected));
        let (source, owner) = cache.as_ref().expect("published admission owner");
        assert!(Arc::ptr_eq(source, &self.expected));
        assert!(LanePrivacyRegistry::ptr_eq(&privacy, owner));
        self.wakes.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn queue_privacy_publication_refunds_after_all_original_writers_release() {
    let (_clock, time) = TimeSource::new_mock(Duration::default());
    let queue = Arc::new(Queue::test(config_factory(), &time));
    let budget = iroha_allocation::AllocationBudget::new(1024 * 1024);
    let original = funded_privacy_manifest_fixture();
    queue
        .install_lane_manifests_for_testing(&original, &budget)
        .unwrap();
    let retained = budget.reserved_bytes();
    assert!(retained > 0);
    let wait = match budget.try_reserve_bytes(budget.limit_bytes()).unwrap_err() {
        iroha_allocation::AllocationRefusal::Capacity { release, .. } => release,
        other => panic!("expected occupied original pool: {other:?}"),
    };
    // The waiter has its own explicit fixture pool, separate from graph custody.
    let waiter_pool = iroha_allocation::AllocationBudget::new(4096);
    let mut prepaid = waiter_pool
        .try_reserve(iroha_allocation::release::ReleaseRegistration::allocation_layout())
        .unwrap();
    let mut registration =
        iroha_allocation::release::ReleaseRegistration::from_reservation(&mut prepaid).unwrap();
    let replacement = funded_privacy_manifest_fixture();
    let observer = Arc::new(QueuePrivacyRefundObserver {
        queue: Arc::downgrade(&queue),
        expected: Arc::clone(&replacement),
        wakes: AtomicUsize::new(0),
    });
    let waker = std::task::Waker::from(Arc::clone(&observer));
    assert!(
        registration
            .poll_wait(&wait, &mut std::task::Context::from_waker(&waker),)
            .is_pending()
    );
    queue
        .install_lane_manifests_for_testing(&replacement, &budget)
        .unwrap();
    assert!(observer.wakes.load(Ordering::SeqCst) > 0);
    assert_eq!(
        budget.reserved_bytes(),
        retained,
        "one graph remains after replacement"
    );
    drop(registration);
    drop(prepaid);
    assert_eq!(waiter_pool.reserved_bytes(), 0);
    drop(queue);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn queue_reconfiguration_and_route_preserve_accepted_owners_on_privacy_refusal() {
    // Keep unrelated fixture retirements from reopening the measured pool.
    let _fixture_retirement = crossbeam_epoch::pin();
    let state = State::new(
        world_with_test_domains(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let (_clock, time) = TimeSource::new_mock(Duration::default());
    let queue = Queue::test(config_factory(), &time);
    let manifests = funded_privacy_manifest_fixture();
    queue.install_lane_manifests_with_state_for_testing(&manifests, &state);
    let nexus = state.nexus_snapshot();
    let view = state.view();
    let tx = accepted_tx_by_someone(&time);
    // Model an accepted Queue cache that must catch up to the observed State.
    queue.routing_policy.write().default_lane = LaneId::new(99);
    let old_policy = queue.routing_policy.read().clone();
    let old_router = Arc::clone(&queue.router.read());
    let old_lanes = Arc::clone(&queue.lane_catalog.read());
    let old_dataspaces = Arc::clone(&queue.dataspace_catalog.read());
    let old_privacy = queue.lane_privacy_registry();
    let generation = state.state_view_generation();
    let budget = state.ivm_execution_budget();
    let retained = budget.reserved_bytes();
    let occupied = budget
        .try_reserve_bytes(budget.limit_bytes() - retained)
        .unwrap();
    for refusal in [
        queue.reconfigure_nexus(&nexus, &view, None).unwrap_err(),
        queue
            .reconfigure_nexus_with_state(&nexus, &state, None)
            .unwrap_err(),
        queue
            .reconfigure_nexus_with_state_if_needed(&nexus, &state, None)
            .unwrap_err(),
    ] {
        assert!(matches!(
            refusal.allocation_refusal(),
            Some(iroha_allocation::AllocationRefusal::Capacity { .. })
        ));
        assert!(Arc::ptr_eq(&queue.lane_manifests.read(), &manifests));
        assert!(LanePrivacyRegistry::ptr_eq(
            &queue.lane_privacy_registry(),
            &old_privacy
        ));
        assert!(Arc::ptr_eq(&queue.router.read(), &old_router));
        assert!(Arc::ptr_eq(&queue.lane_catalog.read(), &old_lanes));
        assert!(Arc::ptr_eq(
            &queue.dataspace_catalog.read(),
            &old_dataspaces
        ));
        assert_eq!(*queue.routing_policy.read(), old_policy);
        let cache = queue.admission_privacy_registry.lock();
        let (source, owner) = cache.as_ref().unwrap();
        assert!(Arc::ptr_eq(source, &manifests));
        assert!(LanePrivacyRegistry::ptr_eq(owner, &old_privacy));
    }
    let route_error = queue.route_plan_with_state(&tx, &state).unwrap_err();
    assert!(
        matches!(route_error, RoutingResolveError::Deferred(ref original)
        if matches!(original.allocation_refusal(), Some(iroha_allocation::AllocationRefusal::Capacity { .. })))
    );
    assert_eq!(state.state_view_generation(), generation);
    assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
    drop(occupied);
    assert!(
        queue
            .reconfigure_nexus_with_state_if_needed(&nexus, &state, None)
            .expect("retry against the same original pool")
    );
    assert!(queue.nexus_routing_matches(&nexus));
    assert_eq!(queue.lane_privacy_registry(), old_privacy);
    assert!(queue.lane_privacy_registry().belongs_to(&budget));
    assert_eq!(state.state_view_generation(), generation);
}

struct ForeignQueuePrivacyRefundObserver {
    queue: std::sync::Weak<Queue>,
    state: Arc<State>,
    expected_cache_source: Option<LaneManifestRegistryHandle>,
    wakes: AtomicUsize,
    failed_probes: AtomicUsize,
}

impl std::task::Wake for ForeignQueuePrivacyRefundObserver {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        // Record contention instead of panicking in a callback during unwind.
        self.wakes.fetch_add(1, Ordering::SeqCst);
        let Ok(_lifecycle) = self.state.try_lock_lane_lifecycle_work_admission() else {
            self.failed_probes.fetch_or(1, Ordering::SeqCst);
            return;
        };
        let queue = self.queue.upgrade().expect("live Queue during refund");
        let Some(manifests) = queue.lane_manifests.try_write() else {
            self.failed_probes.fetch_or(2, Ordering::SeqCst);
            return;
        };
        let Some(privacy) = queue.lane_privacy_registry.try_write() else {
            self.failed_probes.fetch_or(4, Ordering::SeqCst);
            return;
        };
        let Some(cache) = queue.admission_privacy_registry.try_lock() else {
            self.failed_probes.fetch_or(8, Ordering::SeqCst);
            return;
        };
        let valid = cache.as_ref().is_some_and(|(source, owner)| {
            owner.belongs_to(&self.state.ivm_execution_budget())
                && if let Some(expected) = &self.expected_cache_source {
                    Arc::ptr_eq(source, expected)
                } else {
                    Arc::ptr_eq(source, &manifests) && LanePrivacyRegistry::ptr_eq(owner, &privacy)
                }
        });
        if !valid {
            self.failed_probes.fetch_or(16, Ordering::SeqCst);
        }
    }
}

fn privacy_refund_registration(
    budget: &iroha_allocation::AllocationBudget,
    observer: &Arc<ForeignQueuePrivacyRefundObserver>,
) -> (
    iroha_allocation::AllocationBudget,
    iroha_allocation::release::ReleaseRegistration,
) {
    assert!(
        budget.reserved_bytes() > 0,
        "the Queue is the only graph owner"
    );
    let wait = match budget.try_reserve_bytes(budget.limit_bytes()).unwrap_err() {
        iroha_allocation::AllocationRefusal::Capacity { release, .. } => release,
        other => panic!("expected the actual occupied foreign pool: {other:?}"),
    };
    let waiter_pool = iroha_allocation::AllocationBudget::new(4096);
    let mut prepaid = waiter_pool
        .try_reserve(iroha_allocation::release::ReleaseRegistration::allocation_layout())
        .unwrap();
    let mut registration =
        iroha_allocation::release::ReleaseRegistration::from_reservation(&mut prepaid).unwrap();
    let waker = std::task::Waker::from(Arc::clone(observer));
    assert!(
        registration
            .poll_wait(&wait, &mut std::task::Context::from_waker(&waker))
            .is_pending()
    );
    (waiter_pool, registration)
}

#[test]
fn foreign_privacy_publication_refunds_after_actual_route_lifecycle_fence() {
    let (state, time) = current_admission_queue_fixture();
    let manifests = funded_privacy_manifest_fixture();
    drop(state.install_lane_manifests_for_testing(&manifests));
    let state = Arc::new(state);
    let queue = Arc::new(Queue::test(config_factory(), &time));
    let foreign_pool = iroha_allocation::AllocationBudget::new(1024 * 1024);
    queue
        .install_lane_manifests_for_testing(&manifests, &foreign_pool)
        .unwrap();
    // No external privacy clone survives: both final graph references are Queue-owned.
    queue.routing_policy.write().default_lane = LaneId::new(99);
    let observer = Arc::new(ForeignQueuePrivacyRefundObserver {
        queue: Arc::downgrade(&queue),
        state: Arc::clone(&state),
        expected_cache_source: None,
        wakes: AtomicUsize::new(0),
        failed_probes: AtomicUsize::new(0),
    });
    let (waiter_pool, registration) = privacy_refund_registration(&foreign_pool, &observer);
    let tx = accepted_tx_by_someone(&time);
    queue
        .route_plan_with_state(&tx, &state)
        .expect("actual route refresh succeeds");
    assert!(observer.wakes.load(Ordering::SeqCst) > 0);
    assert_eq!(
        observer.failed_probes.load(Ordering::SeqCst),
        0,
        "callback sees released lifecycle/Queue guards and the exact new owner pair"
    );
    assert_eq!(foreign_pool.reserved_bytes(), 0);
    assert!(queue.nexus_routing_matches(&state.nexus_snapshot()));
    drop(registration);
    assert_eq!(waiter_pool.reserved_bytes(), 0);
}

#[test]
fn foreign_privacy_cache_refunds_after_actual_admission_success_and_rejection() {
    for reject in [false, true] {
        let (mut state, time) = current_admission_queue_fixture();
        let tx = accepted_tx_by_someone(&time);
        register_accepted_tx_authority_for_queue_test(&mut state, &tx);
        let state = Arc::new(state);
        let queue = Arc::new(Queue::test(config_factory(), &time));
        queue
            .reconfigure_nexus_with_state(&state.nexus_snapshot(), &state, None)
            .unwrap();
        let manifests = funded_privacy_manifest_fixture();
        queue
            .install_lane_manifests_for_testing(&manifests, &state.ivm_execution_budget())
            .unwrap();
        if reject {
            let policy = LaneCompliancePolicy {
                id: LaneCompliancePolicyId::new(Hash::prehashed([0xBA; 32])),
                version: 1,
                lane_id: LaneId::SINGLE,
                dataspace_id: DataSpaceId::UNIVERSAL,
                jurisdiction: JurisdictionSet::default(),
                deny: vec![LaneComplianceRule {
                    selector: ParticipantSelector {
                        account: Some(AccountId::new(ALICE_KEYPAIR.public_key().clone())),
                        ..ParticipantSelector::default()
                    },
                    reason_code: Some("post-cache rejection control".to_owned()),
                    jurisdiction_override: JurisdictionSet::default(),
                }],
                allow: Vec::new(),
                transfer_limits: Vec::new(),
                audit_controls: AuditControls::default(),
                metadata: Metadata::default(),
            };
            *queue.lane_compliance.write() = Some(Arc::new(
                LaneComplianceEngine::from_policies(vec![policy], false).unwrap(),
            ));
        }
        let foreign_pool = iroha_allocation::AllocationBudget::new(1024 * 1024);
        drop(
            queue
                .admission_privacy_registry_for(
                    &manifests,
                    &foreign_pool,
                    &mut QueuePrivacyRetirements::default(),
                )
                .unwrap(),
        );
        let observer = Arc::new(ForeignQueuePrivacyRefundObserver {
            queue: Arc::downgrade(&queue),
            state: Arc::clone(&state),
            expected_cache_source: Some(Arc::clone(&manifests)),
            wakes: AtomicUsize::new(0),
            failed_probes: AtomicUsize::new(0),
        });
        let (waiter_pool, registration) = privacy_refund_registration(&foreign_pool, &observer);
        let original = tx.entrypoint_bytes().to_vec();
        let result = queue.push_with_lane_with_state(tx, &state);
        if reject {
            let failure =
                result.expect_err("actual compliance rejection follows cache replacement");
            assert!(matches!(failure.err, Error::LaneComplianceDenied { .. }));
            assert_eq!(
                failure.tx.entrypoint_bytes().as_slice(),
                original.as_slice()
            );
            assert_eq!(queue.active_len(), 0);
        } else {
            result.expect("actual admission succeeds after the cache replacement");
            assert_eq!(queue.active_len(), 1);
        }
        assert!(observer.wakes.load(Ordering::SeqCst) > 0);
        assert_eq!(
            observer.failed_probes.load(Ordering::SeqCst),
            0,
            "callback sees released lifecycle/Queue guards and the exact new owner pair"
        );
        assert_eq!(foreign_pool.reserved_bytes(), 0);
        drop(registration);
        assert_eq!(waiter_pool.reserved_bytes(), 0);
    }
}

#[test]
fn foreign_privacy_retirement_survives_error_and_unwind_after_both_replacements() {
    // The actual replacement methods receive custody owned before the genuine
    // lifecycle guard. Exercise cleanup after their return as well as on unwind.
    for publish in [false, true] {
        for unwind in [false, true] {
            let (state, time) = current_admission_queue_fixture();
            let manifests = funded_privacy_manifest_fixture();
            drop(state.install_lane_manifests_for_testing(&manifests));
            let state = Arc::new(state);
            let queue = Arc::new(Queue::test(config_factory(), &time));
            let foreign_pool = iroha_allocation::AllocationBudget::new(1024 * 1024);
            if publish {
                queue
                    .install_lane_manifests_for_testing(&manifests, &foreign_pool)
                    .unwrap();
                queue.routing_policy.write().default_lane = LaneId::new(99);
            } else {
                drop(
                    queue
                        .admission_privacy_registry_for(
                            &manifests,
                            &foreign_pool,
                            &mut QueuePrivacyRetirements::default(),
                        )
                        .unwrap(),
                );
            }
            let observer = Arc::new(ForeignQueuePrivacyRefundObserver {
                queue: Arc::downgrade(&queue),
                state: Arc::clone(&state),
                expected_cache_source: (!publish).then(|| Arc::clone(&manifests)),
                wakes: AtomicUsize::new(0),
                failed_probes: AtomicUsize::new(0),
            });
            let (waiter_pool, registration) = privacy_refund_registration(&foreign_pool, &observer);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut retired = QueuePrivacyRetirements::default();
                let _lifecycle = state.lock_lane_lifecycle_work_admission();
                if publish {
                    queue
                        .sync_nexus_routing_with_state(&state, &mut retired)
                        .unwrap();
                } else {
                    drop(
                        queue
                            .admission_privacy_registry_for(
                                &manifests,
                                &state.ivm_execution_budget(),
                                &mut retired,
                            )
                            .unwrap(),
                    );
                }
                assert_eq!(observer.wakes.load(Ordering::SeqCst), 0);
                assert!(foreign_pool.reserved_bytes() > 0);
                assert!(state.try_lock_lane_lifecycle_work_admission().is_err());
                assert!(!unwind, "intentional post-replacement unwind");
                Err::<(), _>("intentional post-replacement error")
            }));
            if unwind {
                assert!(result.is_err());
            } else {
                assert_eq!(result.unwrap(), Err("intentional post-replacement error"));
            }
            assert!(observer.wakes.load(Ordering::SeqCst) > 0);
            assert_eq!(
                observer.failed_probes.load(Ordering::SeqCst),
                0,
                "callback sees released lifecycle/Queue guards and the exact new owner pair"
            );
            assert_eq!(foreign_pool.reserved_bytes(), 0);
            drop(registration);
            assert_eq!(waiter_pool.reserved_bytes(), 0);
        }
    }
}

#[test]
fn batch_privacy_retirement_storage_uses_exact_input_count_and_original_pool() {
    // Keep unrelated fixture retirements from reopening the measured pool.
    let _fixture_retirement = crossbeam_epoch::pin();
    let (mut state, time) = current_admission_queue_fixture();
    let tx = accepted_tx_by_someone(&time);
    register_accepted_tx_authority_for_queue_test(&mut state, &tx);
    let queue = Queue::test(config_factory(), &time);
    let plan = queue.route_plan_with_state(&tx, &state).unwrap();
    let original = tx.entrypoint_bytes().to_vec();
    let budget = state.ivm_execution_budget();
    let prior = budget.reserved_bytes();
    let occupied = budget
        .try_reserve_bytes(budget.limit_bytes() - prior)
        .unwrap();
    assert_eq!(
        queue
            .push_batch_with_lane_with_state_and_routing_plans(Vec::new(), &state)
            .unwrap(),
        0
    );
    let expected = budget
        .try_reserve(std::alloc::Layout::array::<QueuePrivacyRetirements>(2).unwrap())
        .unwrap_err();
    let failure = queue
        .push_batch_with_lane_with_state_and_routing_plans(
            vec![(tx.clone(), plan.clone()), (tx.clone(), plan.clone())],
            &state,
        )
        .unwrap_err();
    let Error::Deferred(original_refusal) = failure.err else {
        panic!("typed storage refusal");
    };
    assert_eq!(original_refusal.allocation_refusal(), Some(&expected));
    assert!(matches!(original_refusal.allocation_refusal(),
        Some(iroha_allocation::AllocationRefusal::Capacity { requested_bytes, .. })
        if *requested_bytes == std::alloc::Layout::array::<QueuePrivacyRetirements>(2).unwrap().size()));
    assert_eq!(
        failure.tx.entrypoint_bytes().as_slice(),
        original.as_slice()
    );
    assert_eq!(queue.active_len(), 0);
    assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
    drop(occupied);
    assert_eq!(budget.reserved_bytes(), prior);
    let retry = queue
        .push_batch_with_lane_with_state_and_routing_plans(
            vec![(tx.clone(), plan.clone()), (tx, plan)],
            &state,
        )
        .expect_err("same two-input batch admits storage and reaches its original duplicate");
    assert!(matches!(retry.err, Error::IsInQueue));
    assert_eq!(retry.tx.entrypoint_bytes().as_slice(), original.as_slice());
    assert_eq!(queue.active_len(), 1);
}

#[test]
fn batch_foreign_privacy_refunds_after_accepted_prefix_failure_releases_lifecycle() {
    let (mut state, time) = current_admission_queue_fixture();
    let tx = accepted_tx_by_someone(&time);
    register_accepted_tx_authority_for_queue_test(&mut state, &tx);
    let state = Arc::new(state);
    let queue = Arc::new(Queue::test(config_factory(), &time));
    queue
        .reconfigure_nexus_with_state(&state.nexus_snapshot(), &state, None)
        .unwrap();
    let manifests = funded_privacy_manifest_fixture();
    queue
        .install_lane_manifests_for_testing(&manifests, &state.ivm_execution_budget())
        .unwrap();
    let plan = queue.route_plan_with_state(&tx, &state).unwrap();
    let foreign_pool = iroha_allocation::AllocationBudget::new(1024 * 1024);
    drop(
        queue
            .admission_privacy_registry_for(
                &manifests,
                &foreign_pool,
                &mut QueuePrivacyRetirements::default(),
            )
            .unwrap(),
    );
    let observer = Arc::new(ForeignQueuePrivacyRefundObserver {
        queue: Arc::downgrade(&queue),
        state: Arc::clone(&state),
        expected_cache_source: Some(Arc::clone(&manifests)),
        wakes: AtomicUsize::new(0),
        failed_probes: AtomicUsize::new(0),
    });
    let (waiter_pool, registration) = privacy_refund_registration(&foreign_pool, &observer);
    let original = tx.entrypoint_bytes().to_vec();
    let failure = queue
        .push_batch_with_lane_with_state_and_routing_plans(
            vec![(tx.clone(), plan.clone()), (tx, plan)],
            &state,
        )
        .expect_err("the second identical input is already in the accepted prefix");
    assert!(matches!(failure.err, Error::IsInQueue));
    assert_eq!(
        failure.tx.entrypoint_bytes().as_slice(),
        original.as_slice()
    );
    assert_eq!(queue.active_len(), 1);
    assert!(observer.wakes.load(Ordering::SeqCst) > 0);
    assert_eq!(
        observer.failed_probes.load(Ordering::SeqCst),
        0,
        "callback sees released lifecycle/Queue guards and the exact new owner pair"
    );
    assert_eq!(foreign_pool.reserved_bytes(), 0);
    drop(registration);
    assert_eq!(waiter_pool.reserved_bytes(), 0);
}
