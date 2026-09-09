// Actual signed snapshot reader and token-consuming finalizer; no test authority constructor.

struct SignedSnapshotPhysicalFixture {
    _store_directory: TempDir,
    snapshot_directory: TempDir,
    kura: Arc<Kura>,
    policy: SnapshotBootstrapPolicy,
    signing_key: KeyPair,
    network_id: iroha_data_model::NetworkId,
    block_count: BlockCount,
    record: iroha_data_model::block::consensus_v2::SnapshotV2BootstrapRecord,
}

fn signed_snapshot_physical_fixture() -> SignedSnapshotPhysicalFixture {
    use iroha_data_model::consensus::{
        ConsensusKeyId, ConsensusKeyRecord, ConsensusKeyRole, ConsensusKeyStatus,
    };
    use sha2::{Digest as _, Sha256};

    let directory = TempDir::new().unwrap();
    let snapshot_directory = TempDir::new().unwrap();
    let config = kura_config_for_dir(&directory, BLOCKS_IN_MEMORY);
    let catalog = LaneCatalog::default();
    let lane_config = RuntimeLaneConfig::from_catalog(&catalog);
    let (kura, _) = Kura::new_with_configured_lane_catalog(&config, &lane_config, &catalog)
        .expect("open real configured source Kura");
    establish_dummy_store_primary_anchor(&kura);
    let network_id = test_network_id(b"signed-snapshot-physical-finalization");
    let mut keys = (1_u8..=4)
        .map(|seed| KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal).unwrap())
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    let mut world = World::new();
    for (index, key) in keys.iter().enumerate() {
        let id = ConsensusKeyId::new(ConsensusKeyRole::Validator, format!("snapshot{index}"));
        let entry = ConsensusKeyRecord {
            id: id.clone(),
            public_key: key.public_key().clone(),
            pop: Some(bls_normal_pop_prove(key.private_key()).unwrap()),
            activation_height: 0,
            expiry_height: None,
            replaces: None,
            status: ConsensusKeyStatus::Active,
        };
        world.consensus_keys.insert(id.clone(), entry.clone());
        world
            .consensus_keys_by_pk
            .insert(entry.public_key.to_string(), vec![id]);
    }
    // The production snapshot reader restores static policy from runtime defaults.
    // Use those same defaults when signing the context: the general test constructor
    // zeros Nexus fees and changes execution policy, neither of which is snapshot state.
    let state = State::try_new_with_chain_and_network_id_with_default_telemetry(
        world,
        Arc::clone(&kura),
        LiveQueryStore::start_test(),
        ChainId::from("signed-snapshot-physical-finalization"),
        network_id,
    )
    .expect("open source State with the same static policy as the signed snapshot reader");
    state.install_active_lane_markers_for_tests();
    let topology = Topology::new(keys.iter().map(|key| PeerId::new(key.public_key().clone())));
    let mut parent = None;
    for height in 1..=3_u64 {
        let mut valid = ValidBlock::new_dummy_and_modify_header(keys[0].private_key(), |header| {
            header.set_height(std::num::NonZeroU64::new(height).unwrap());
            header.set_prev_block_hash(parent);
            header.creation_time_ms = height;
            header.merkle_root = None;
        });
        valid
            .as_mut()
            .set_transaction_results(Vec::new(), &[], Vec::new())
            .unwrap();
        let block = valid.commit_unchecked().unpack(|_| {});
        parent = Some(block.as_ref().hash());
        kura.store_block(block.clone()).unwrap();
        let mut staged = state.block(block.as_ref().header());
        let _events = staged.apply_without_execution(&block, topology.as_ref().to_owned());
        staged.commit().unwrap();
    }
    let roster = keys
        .iter()
        .map(|key| ValidatorPower {
            validator: PeerId::new(key.public_key().clone()),
            power: 1,
        })
        .collect::<Vec<_>>();
    let (kagemusha_mint_finality_epoch_id, kagemusha_mint_finality_epoch_roster) =
        crate::kagemusha_v1_test_fixtures::mint_finality_roster_and_id(network_id, 0, &roster);
    let context = HeightContext {
        network_id,
        protocol_version: PROTOCOL_VERSION,
        height: 4,
        epoch: 0,
        epoch_end_height: u64::MAX,
        next_epoch_snapshot: None,
        snapshot_bootstrap: Some(
            iroha_data_model::block::consensus_v2::SnapshotBootstrapAnchor {
                snapshot_height: 3,
                snapshot_block_hash: parent.unwrap(),
                snapshot_block_creation_time_ms: 3,
                snapshot_state_hash: crate::snapshot::canonical_state_snapshot_hash(&state),
            },
        ),
        mode: ConsensusMode::Permissioned,
        parent_commit_qc: None,
        quorum: DualQuorum::from_roster(&roster).unwrap(),
        roster,
        kagemusha_mint_finality_epoch_id,
        kagemusha_mint_finality_epoch_roster,
        nexus_amx_context_hash: crate::sumeragi::v2_recovery::committed_nexus_amx_context_hash(
            &state,
        ),
        execution_policy_hash: crate::sumeragi::v2_recovery::committed_execution_policy_hash(
            &state,
        )
        .unwrap(),
        da_layout: DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 1024,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 4096,
            max_chunk_count: 8,
        },
        leader_seed: [0x31; 32],
    };
    let record = iroha_data_model::block::consensus_v2::SnapshotV2BootstrapRecord {
        version: iroha_data_model::block::consensus_v2::SnapshotV2BootstrapRecord::VERSION,
        context,
        validator_set_pops: keys
            .iter()
            .map(|key| bls_normal_pop_prove(key.private_key()).unwrap())
            .collect(),
    };
    record.validate().unwrap();
    assert_eq!(record.context.roster.len(), 4);
    assert!(state.authenticated_snapshot_v2_bootstrap().is_none());
    // Build an untrusted wire candidate. Only the signed reader below may
    // promote it; no State authenticated-lineage setter or payload/token factory is used.
    let signing_key = KeyPair::try_from_seed(vec![0x71; 32], Algorithm::Ed25519).unwrap();
    let payload = crate::snapshot::publish_signed_snapshot_payload_for_physical_test(
        snapshot_directory.path(),
        &state,
        &record,
        &signing_key,
    );
    let policy = SnapshotBootstrapPolicy {
        enabled: true,
        audited_sha256: Some(hex::encode(Sha256::digest(&payload))),
        audited_height: Some(3),
    };
    drop(state);
    drop(kura);
    let (kura, block_count) = Kura::new_with_configured_lane_catalog_and_snapshot_bootstrap(
        &config,
        &lane_config,
        &catalog,
        &policy,
    )
    .expect("reopen through the production read-only provisional constructor");
    assert!(kura.provisional_snapshot_bootstrap_pending());
    assert!(kura.ensure_snapshot_bootstrap_authenticated().is_err());
    assert!(!kura.sumeragi_v2_storage_root().join("contexts").exists());
    SignedSnapshotPhysicalFixture {
        _store_directory: directory,
        snapshot_directory,
        kura,
        policy,
        signing_key,
        network_id,
        block_count,
        record,
    }
}

fn read_signed_snapshot_physical_fixture(
    fixture: &SignedSnapshotPhysicalFixture,
    verification_key: &iroha_crypto::PublicKey,
    policy: &SnapshotBootstrapPolicy,
) -> std::result::Result<State, crate::snapshot::TryReadError> {
    crate::snapshot::try_read_snapshot_with_bootstrap_policy(
        fixture.snapshot_directory.path(),
        &fixture.kura,
        LiveQueryStore::start_test,
        fixture.block_count,
        nonzero!(1024_usize),
        iroha_config::parameters::defaults::snapshot::MAX_PAYLOAD_BYTES,
        iroha_config::parameters::actual::SnapshotResourcePolicy::default(),
        verification_key,
        &fixture.network_id,
        &crate::state::default_zk_config(),
        policy,
        #[cfg(feature = "telemetry")]
        crate::telemetry::StateTelemetry::default(),
    )
}

fn authenticate_signed_snapshot_physical_fixture(
    fixture: &SignedSnapshotPhysicalFixture,
) -> crate::sumeragi::AuthenticatedV2SnapshotStartup {
    let state = read_signed_snapshot_physical_fixture(
        fixture,
        fixture.signing_key.public_key(),
        &fixture.policy,
    )
    .expect("verify actual signature, signed manifest, Merkle payload and exact audited lineage");
    assert_eq!(
        state.authenticated_snapshot_v2_bootstrap(),
        Some(&fixture.record)
    );
    let plan = crate::sumeragi::plan_v2_startup_replay(&fixture.kura).unwrap();
    assert_eq!(plan.audited_bootstrap_prefix_height(), 3);
    assert_eq!(plan.first_full_body_height(), None);
    let authorization =
        crate::sumeragi::authenticate_v2_snapshot_startup(&fixture.kura, &state, &plan)
            .unwrap()
            .expect("the public exact-boundary verifier must mint the consumed token");
    assert_eq!(authorization.mode(), fixture.record.context.mode);
    authorization
}

fn snapshot_physical_scope_counts(kura: &Kura) -> IndexResourceCounts {
    kura.physical_resource_scope()
        .unwrap()
        .observe(kura.evidence_resource_limits())
        .unwrap()
}

fn snapshot_physical_assert_actual(kura: &Kura) -> IndexResourceCounts {
    let actual = snapshot_physical_scope_counts(kura);
    for family in PHYSICAL_RESOURCE_FAMILIES {
        assert_eq!(
            kura.resource_inventory
                .component_usage_for_tests(family)
                .unwrap(),
            actual[family as usize],
            "{family:?}"
        );
    }
    actual
}

fn snapshot_physical_seed_measured_test_baseline(kura: &Kura) -> IndexResourceCounts {
    let counts = snapshot_physical_scope_counts(kura);
    // This initializes only the isolated measurement fixture, not runtime
    // storage authority. Pending still rejects every unauthenticated mutation.
    assert!(kura.provisional_snapshot_bootstrap_pending());
    kura.resource_inventory
        .initialize(
            kura.resource_inventory.reconciliation_generation().unwrap(),
            &PHYSICAL_RESOURCE_FAMILIES
                .iter()
                .map(|family| (*family, counts[*family as usize]))
                .collect::<Vec<_>>(),
        )
        .unwrap();
    assert!(kura.ensure_snapshot_bootstrap_authenticated().is_err());
    snapshot_physical_assert_actual(kura)
}

#[test]
fn signed_snapshot_finalizer_accounts_deferred_metadata_before_authenticated_reaudit() {
    let fixture = signed_snapshot_physical_fixture();
    let authorization = authenticate_signed_snapshot_physical_fixture(&fixture);
    let kura = &fixture.kura;
    let merge = kura.active_merge_path.lock().clone();
    assert_eq!(fs::metadata(&merge).unwrap().len(), 0);
    // An incomplete two-byte frame has no QC/entry authority and is recovered
    // by the real MergeLedgerLog reader inside the authorized finalizer.
    fs::write(&merge, [0x09_u8, 0x00]).unwrap();
    let checkpoint = kura.wsv_checkpoint_dir().join(format!("{:020}.norito", 4));
    let manifest = kura.commit_manifest_dir().join(format!("{:020}.norito", 4));
    fs::create_dir_all(checkpoint.parent().unwrap()).unwrap();
    fs::create_dir_all(manifest.parent().unwrap()).unwrap();
    fs::write(&checkpoint, [0xA1_u8; 9]).unwrap();
    fs::write(&manifest, [0xA2_u8; 23]).unwrap();
    let before = snapshot_physical_seed_measured_test_baseline(kura);
    kura.arm_snapshot_finalization_resource_observation_for_test();
    kura.finalize_authenticated_snapshot_bootstrap(authorization)
        .unwrap();
    assert!(!kura.provisional_snapshot_bootstrap_pending());
    assert!(kura.ensure_snapshot_bootstrap_authenticated().is_ok());
    assert!(kura.sumeragi_v2_storage_root().join("contexts").exists());
    assert_eq!(fs::metadata(&merge).unwrap().len(), 0);
    assert!(!checkpoint.exists());
    assert!(!manifest.exists());
    // The method automatically re-audits after authentication. This per-owner
    // read-only observation precedes that recount and cannot conceal a missing guard.
    let observed = kura.take_snapshot_finalization_resource_observation_for_test();
    let immediate = observed
        .inventory
        .expect("all immediate physical families remain valid");
    let measured = observed
        .measured
        .expect("independent managed-scope observation succeeds");
    assert_eq!(immediate, measured);
    let after = snapshot_physical_assert_actual(kura);
    assert_eq!(after, immediate);
    assert_eq!(
        before[ResourceFamily::StorageBytes as usize].storage_bytes
            - after[ResourceFamily::StorageBytes as usize].storage_bytes,
        34
    );
    assert_eq!(
        before[ResourceFamily::EvidenceKeyRecords as usize].persisted_entries
            - after[ResourceFamily::EvidenceKeyRecords as usize].persisted_entries,
        2
    );
    kura.reconcile_physical_resource_inventory().unwrap();
    assert_eq!(snapshot_physical_assert_actual(kura), after);
}

#[test]
fn signed_snapshot_finalizer_partial_recovery_failure_invalidates_every_physical_family() {
    let fixture = signed_snapshot_physical_fixture();
    let authorization = authenticate_signed_snapshot_physical_fixture(&fixture);
    let kura = &fixture.kura;
    let merge = kura.active_merge_path.lock().clone();
    fs::write(&merge, [0x09_u8, 0x00]).unwrap();
    let manifest = kura.commit_manifest_dir().join(format!("{:020}.norito", 1));
    fs::create_dir_all(manifest.parent().unwrap()).unwrap();
    fs::write(&manifest, [0xA3_u8; 23]).unwrap();
    snapshot_physical_seed_measured_test_baseline(kura);
    assert!(matches!(
        kura.finalize_authenticated_snapshot_bootstrap(authorization),
        Err(Error::NoritoFrame(_))
    ));
    assert_eq!(
        fs::metadata(&merge).unwrap().len(),
        0,
        "real earlier deferred recovery completed"
    );
    assert_eq!(fs::read(&manifest).unwrap(), [0xA3_u8; 23]);
    assert!(kura.provisional_snapshot_bootstrap_pending());
    assert!(kura.canonical_storage_poisoned.load(Ordering::Acquire));
    assert!(!kura.sumeragi_v2_storage_root().join("contexts").exists());
    for family in PHYSICAL_RESOURCE_FAMILIES {
        assert!(
            kura.resource_inventory
                .component_usage_for_tests(family)
                .is_err(),
            "{family:?}"
        );
    }
    assert!(kura.reconcile_physical_resource_inventory().is_err());
}

#[test]
fn snapshot_signature_rejection_cannot_mint_finalization_authority_or_change_physical_counts() {
    let fixture = signed_snapshot_physical_fixture();
    let wrong_key = KeyPair::try_from_seed(vec![0x72; 32], Algorithm::Ed25519).unwrap();
    let before = snapshot_physical_seed_measured_test_baseline(&fixture.kura);
    // Disable the separate exact-digest exception for this signature negative.
    // The positive uses the matching real signature; no signature bypass is tested as success.
    assert!(matches!(
        read_signed_snapshot_physical_fixture(
            &fixture,
            wrong_key.public_key(),
            &SnapshotBootstrapPolicy::default()
        ),
        Err(crate::snapshot::TryReadError::SignatureInvalid(_))
    ));
    assert!(fixture.kura.provisional_snapshot_bootstrap_pending());
    assert!(
        !fixture
            .kura
            .canonical_storage_poisoned
            .load(Ordering::Acquire)
    );
    assert!(
        !fixture
            .kura
            .sumeragi_v2_storage_root()
            .join("contexts")
            .exists()
    );
    assert_eq!(snapshot_physical_assert_actual(&fixture.kura), before);
    assert!(
        fixture
            .kura
            .reconcile_physical_resource_inventory()
            .is_err()
    );
}
