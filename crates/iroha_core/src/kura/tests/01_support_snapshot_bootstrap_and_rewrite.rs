use std::{
    borrow::Cow,
    cell::Cell,
    collections::BTreeMap,
    fs,
    io::{Read, Seek, SeekFrom, Write},
    num::{NonZeroU32, NonZeroUsize},
    path::{Path, PathBuf},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

fn test_network_id(label: &[u8]) -> iroha_data_model::NetworkId {
    iroha_data_model::NetworkId::from_genesis_hash(
        iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(label)),
    )
}

use iroha_config::{
    base::WithOrigin,
    kura::{FsyncMode, InitMode},
    parameters::{
        actual::{Kura as KuraConfig, LaneConfig as RuntimeLaneConfig},
        defaults::kura::{
            BLOCK_SYNC_ROSTER_RETENTION, BLOCKS_IN_MEMORY, FSYNC_INTERVAL,
            MERGE_LEDGER_CACHE_CAPACITY, ROSTER_SIDECAR_RETENTION,
        },
    },
};
use iroha_crypto::{
    Algorithm, Hash, HashOf, KeyPair, Signature, SignatureOf, bls_normal_pop_prove,
};
use iroha_data_model::{
    ChainId, Level,
    account::Account,
    asset::{AssetDefinitionId, AssetId},
    block::{
        BlockExecutionContextBundle, BlockHeader, BlockSignature, CertifiedMergeLedgerReference,
        ExternalExecutionContext,
        consensus::{
            CertPhase, ExecKv, LaneBlockCommitment, LaneBlockDescriptorV1, LaneBlockProposalV1,
            LaneBlockQcV1, SumeragiLanePayloadOwnership,
        },
        consensus_v2::{
            BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
            ExecutionCommitment, GlobalPhase, HeightContext, PROTOCOL_VERSION, PayloadEncoding,
            QuorumCertificate, ValidatorPower, finality::V2FinalityArtifact,
        },
    },
    consensus::{Qc, VALIDATOR_SET_HASH_VERSION_V1},
    domain::{Domain, DomainId},
    isi::{InstructionBox, Log, Upgrade},
    merge::MergeQuorumCertificate,
    nexus::{
        DataSpaceId, LaneCatalog, LaneConfig as ModelLaneConfig, LaneId, LaneStorageProfile,
        LaneVisibility,
    },
    offline::{
        KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4, KagemushaRecursiveSpendArtifactBindingV4,
        KagemushaRecursiveSpendTopUpRequestV4, KagemushaRequestAuthorizationV2,
        KagemushaScaledAmountV2, KagemushaSpendableNoteDescriptorV2,
        KagemushaTopUpShieldEvidenceV2,
    },
    peer::PeerId,
    prelude::{Executor, IvmBytecode},
    proof::{ProofAttachment, ProofBox, VerifyingKeyId},
    transaction::{
        Executable, TransactionBuilder,
        signed::{TransactionEntrypoint, TransactionResult, TransactionResultInner},
    },
    trigger::DataTriggerSequence,
};
use iroha_genesis::{GenesisBuilder, GenesisTopologyEntry};
use iroha_telemetry::metrics::Metrics;
use iroha_test_samples::{
    SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR, gen_account_in,
};
use iroha_version::codec::EncodeVersioned;
use nonzero_ext::nonzero;
use sha2::Digest as _;
use tempfile::TempDir;

use super::*;
use crate::{
    block::{BlockBuilder, ValidBlock},
    governance::manifest::{
        GovernanceRules, LaneManifestRegistry, LaneManifestStatus, ManifestValidatorBinding,
    },
    prelude::{AcceptedTransaction, StateReadOnly, World},
    query::store::LiveQueryStore,
    smartcontracts::Registrable,
    state::State,
    sumeragi::{
        consensus::{PERMISSIONED_TAG, Phase, QcAggregate},
        network_topology::Topology,
    },
};

fn provisional_snapshot_metadata(tag: u8) -> ProvisionalSnapshotBootstrap {
    ProvisionalSnapshotBootstrap {
        hash_only_prefix_height: usize::from(tag),
        bootstrap_lineage_hash: Some(Hash::prehashed([tag; Hash::LENGTH])),
        hash_journal_digest: Some(Hash::prehashed([tag.wrapping_add(1); Hash::LENGTH])),
    }
}

#[test]
fn snapshot_bootstrap_state_blocks_until_successful_finalization() {
    let kura = Kura::blank_kura_for_testing();
    let pending = provisional_snapshot_metadata(7);
    *kura.provisional_snapshot_bootstrap.lock() =
        SnapshotBootstrapRuntimeState::Pending(pending.clone());

    assert_eq!(
        kura.provisional_snapshot_bootstrap_metadata(),
        Some((
            pending.hash_only_prefix_height,
            pending.bootstrap_lineage_hash
        ))
    );
    assert!(kura.provisional_snapshot_bootstrap_pending());
    assert!(matches!(
        kura.ensure_snapshot_bootstrap_authenticated(),
        Err(Error::SnapshotBootstrapAuthenticationPending)
    ));

    assert!(
        kura.provisional_snapshot_bootstrap
            .lock()
            .begin_finalization(&pending)
    );
    assert_eq!(kura.provisional_snapshot_bootstrap_metadata(), None);
    assert!(
        kura.provisional_snapshot_bootstrap_pending(),
        "finalizing remains unauthenticated and must keep every mutation fail-closed"
    );
    assert!(matches!(
        kura.ensure_snapshot_bootstrap_authenticated(),
        Err(Error::SnapshotBootstrapAuthenticationPending)
    ));

    assert!(
        kura.provisional_snapshot_bootstrap
            .lock()
            .finish_finalization()
    );
    assert!(kura.ensure_snapshot_bootstrap_authenticated().is_ok());
    assert!(
        !kura
            .provisional_snapshot_bootstrap
            .lock()
            .finish_finalization(),
        "authenticated state cannot be finalized twice"
    );
}

#[test]
fn snapshot_bootstrap_pending_state_has_one_finalization_claim() {
    let pending = provisional_snapshot_metadata(11);
    let state = Arc::new(Mutex::new(SnapshotBootstrapRuntimeState::Pending(
        pending.clone(),
    )));
    let start = Arc::new(std::sync::Barrier::new(3));
    let mut finalizers = Vec::new();
    for _ in 0..2 {
        let state = Arc::clone(&state);
        let start = Arc::clone(&start);
        let pending = pending.clone();
        finalizers.push(thread::spawn(move || {
            start.wait();
            state.lock().begin_finalization(&pending)
        }));
    }

    start.wait();
    let claims = finalizers
        .into_iter()
        .map(|finalizer| finalizer.join().expect("finalizer thread must not panic"))
        .filter(|claimed| *claimed)
        .count();

    assert_eq!(claims, 1, "exactly one finalizer may claim recovery");
    assert!(matches!(
        *state.lock(),
        SnapshotBootstrapRuntimeState::Finalizing
    ));
    assert!(
        !state.lock().begin_finalization(&pending),
        "Finalizing remains fail-closed until recovery succeeds"
    );
}

#[test]
fn snapshot_finalization_authority_is_instance_bound_and_expires() {
    let first = Kura::blank_kura_for_testing();
    let second = Kura::blank_kura_for_testing();
    let pending = provisional_snapshot_metadata(13);
    *first.provisional_snapshot_bootstrap.lock() =
        SnapshotBootstrapRuntimeState::Pending(pending.clone());
    *second.provisional_snapshot_bootstrap.lock() =
        SnapshotBootstrapRuntimeState::Pending(pending.clone());
    assert!(
        first
            .provisional_snapshot_bootstrap
            .lock()
            .begin_finalization(&pending)
    );
    let authority =
        SnapshotFinalizationMutationAuthority::new(&first).expect("mint finalization token");

    assert!(matches!(
        authority.validate_for(&second),
        Err(Error::SnapshotBootstrapAuthenticationPending)
    ));
    assert!(
        first
            .provisional_snapshot_bootstrap
            .lock()
            .finish_finalization()
    );
    assert!(matches!(
        authority.validate_for(&first),
        Err(Error::SnapshotBootstrapAuthenticationPending)
    ));
}

#[cfg(unix)]
#[test]
fn provisional_snapshot_gate_preserves_tree_across_mutation_families() {
    let kura = Kura::blank_kura_for_testing();
    let store_root = kura.store_root();
    let retired_merge = store_root.join("retired/merge_ledger");
    std::fs::create_dir_all(&retired_merge).expect("create retired purge fixture");
    std::fs::write(retired_merge.join("pending.log"), b"must remain")
        .expect("write retired purge fixture");
    let before = snapshot_regular_test_tree(&store_root);
    let pending = provisional_snapshot_metadata(5);
    *kura.provisional_snapshot_bootstrap.lock() =
        SnapshotBootstrapRuntimeState::Pending(pending.clone());

    let entry_hash =
        HashOf::<MergeLedgerEntry>::from_untyped_unchecked(Hash::prehashed([0xA5; Hash::LENGTH]));
    assert!(matches!(
        kura.remove_pending_certified_merge_entry(entry_hash),
        Err(Error::SnapshotBootstrapAuthenticationPending)
    ));
    assert!(matches!(
        kura.truncate_merge_log_to_len(0),
        Err(Error::SnapshotBootstrapAuthenticationPending)
    ));
    assert!(matches!(
        kura.store_wsv_checkpoint(
            1,
            HashOf::from_untyped_unchecked(Hash::prehashed([0xB5; Hash::LENGTH])),
            Hash::prehashed([0xC5; Hash::LENGTH]),
        ),
        Err(Error::SnapshotBootstrapAuthenticationPending)
    ));
    kura.write_pipeline_metadata(&PipelineRecoverySidecar::new(
        1,
        HashOf::from_untyped_unchecked(Hash::prehashed([0xD5; Hash::LENGTH])),
        PipelineDagSnapshot {
            fingerprint: [0xE5; 32],
            key_count: 0,
        },
        Vec::new(),
    ));
    assert!(matches!(
        kura.purge_retired_segments(),
        Err(Error::SnapshotBootstrapAuthenticationPending)
    ));

    assert!(
        kura.provisional_snapshot_bootstrap
            .lock()
            .begin_finalization(&pending)
    );
    assert!(matches!(
        kura.purge_retired_segments(),
        Err(Error::SnapshotBootstrapAuthenticationPending)
    ));

    assert_eq!(snapshot_regular_test_tree(&store_root), before);
}

#[test]
fn io_error_display_preserves_path_and_underlying_cause() {
    let path = PathBuf::from("lane_geometry_journal.norito");
    let error = Error::IO(
        std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "injected journal denial",
        ),
        path.clone(),
    );
    let rendered = error.to_string();

    assert!(rendered.contains(&format!("{path:?}")), "{rendered}");
    assert!(rendered.contains("injected journal denial"), "{rendered}");
}

#[test]
fn kura_new_rejects_empty_production_store_root_before_persistence_initialization() {
    let temp_dir = TempDir::new().expect("tempdir");
    let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    config.store_dir = WithOrigin::inline(PathBuf::new());

    let err = match Kura::new(&config, &RuntimeLaneConfig::default()) {
        Ok(_) => panic!("empty production Kura root must fail closed"),
        Err(err) => err,
    };
    assert!(matches!(err, Error::EmptyStoreRoot));
    assert!(
        CommitRosterJournal::journal_path(Path::new(""))
            .as_os_str()
            .is_empty(),
        "the regression must cover the previously silent empty journal path"
    );
}

#[test]
fn state_and_kura_share_commit_roster_journal_owner_across_truncation() {
    let kura = Kura::blank_kura_for_testing();
    let state = State::new_for_testing(
        World::default(),
        Arc::clone(&kura),
        LiveQueryStore::start_test(),
    );
    let kura_journal = kura.commit_roster_journal_handle();
    assert!(
        Arc::ptr_eq(&state.commit_roster_journal, &kura_journal),
        "State must retain Kura's exact journal owner, not a cloned snapshot"
    );

    let first_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA1; Hash::LENGTH]));
    let second_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA2; Hash::LENGTH]));
    let (first_qc, first_checkpoint) = sample_commit_roster_tuple(1, first_hash, 0xA1);
    let (second_qc, second_checkpoint) = sample_commit_roster_tuple(2, second_hash, 0xA2);
    {
        let mut journal = kura_journal.write();
        assert!(journal.upsert(first_qc.clone(), first_checkpoint.clone(), None));
        assert!(journal.upsert(second_qc.clone(), second_checkpoint, None));
    }

    assert_eq!(
        state
            .commit_roster_journal
            .read()
            .get(first_qc.height, first_qc.subject_block_hash)
            .map(|snapshot| snapshot.validator_checkpoint),
        Some(first_checkpoint),
        "State must observe a mutation made through Kura's shared handle"
    );
    assert!(
        state
            .commit_roster_journal
            .read()
            .get(second_qc.height, second_qc.subject_block_hash)
            .is_some()
    );

    kura_journal
        .write()
        .truncate_to_height(1)
        .expect("truncate shared commit roster journal");

    let state_journal = state.commit_roster_journal.read();
    assert!(
        state_journal
            .get(first_qc.height, first_qc.subject_block_hash)
            .is_some(),
        "truncation must retain the in-range snapshot"
    );
    assert!(
        state_journal
            .get(second_qc.height, second_qc.subject_block_hash)
            .is_none(),
        "State must immediately observe Kura-side truncation"
    );
}

#[test]
fn kura_startup_rejects_existing_invalid_commit_roster_journals() {
    let valid = CommitRosterJournal::empty_payload_bytes_for_version(2);
    let truncated = valid[..valid.len().saturating_sub(1)].to_vec();
    let unsupported = CommitRosterJournal::empty_payload_bytes_for_version(u32::MAX);
    for (label, bytes, unsupported_version) in [
        ("corrupt", b"not-a-norito-journal".to_vec(), false),
        ("truncated", truncated, false),
        ("unsupported", unsupported, true),
    ] {
        let temp_dir = TempDir::new().expect("tempdir");
        let path = CommitRosterJournal::journal_path(temp_dir.path());
        let generations = path.join("generations");
        fs::create_dir_all(&generations).expect("create commit-roster generations");
        let digest = hex::encode(sha2::Sha256::digest(&bytes));
        fs::write(generations.join(format!("{digest}.norito")), bytes)
            .expect("write invalid commit-roster generation");
        fs::write(path.join("current"), format!("{digest}\n"))
            .expect("publish invalid commit-roster generation");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let err = match Kura::new(&config, &RuntimeLaneConfig::default()) {
            Ok(_) => panic!("{label} journal must abort Kura startup"),
            Err(err) => err,
        };
        match err {
            Error::CommitRosterJournal(CommitRosterJournalError::UnsupportedVersion { .. })
                if unsupported_version => {}
            Error::CommitRosterJournal(CommitRosterJournalError::Decode { .. })
                if !unsupported_version => {}
            other => panic!("unexpected startup error for {label} journal: {other:?}"),
        }
    }
}

#[test]
fn kura_startup_rejects_corrupt_rollback_intent_without_mutating_block_boundary() {
    let temp_dir = TempDir::new().expect("tempdir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) = Kura::new(&config, &lane_config).expect("initial Kura");
    store_dummy_blocks(&kura, 2);
    let blocks_root = kura.active_blocks_dir.lock().clone();
    drop(kura);

    let intent_path = Kura::rollback_intent_path(&blocks_root);
    fs::write(&intent_path, b"corrupt rollback intent").expect("write corrupt intent");
    sync_dir(&blocks_root).expect("sync corrupt intent marker");
    let err = match Kura::new(&config, &lane_config) {
        Ok(_) => panic!("corrupt rollback intent must block startup"),
        Err(err) => err,
    };
    assert!(matches!(err, Error::RollbackIntentInvalid { .. }));
    assert!(
        intent_path.exists(),
        "unrecoverable intent must remain durable for operator diagnosis"
    );
    let mut store = BlockStore::new(&blocks_root);
    assert_eq!(store.read_index_count().expect("index count"), 2);
    assert_eq!(store.read_hashes_count().expect("hash count"), 2);
}

#[test]
fn kura_startup_promotes_synced_temporary_rollback_intent_and_completes() {
    let temp_dir = TempDir::new().expect("tempdir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) = Kura::new(&config, &lane_config).expect("initial Kura");
    let hashes = store_dummy_blocks(&kura, 2);
    let blocks_root = kura.active_blocks_dir.lock().clone();
    drop(kura);

    // Dummy blocks do not carry merge-ledger entries, so the sparse merge boundary is zero.
    let intent = KuraRollbackIntent::new_with_merge_entries(2, 1, 0, Some(hashes[0]));
    let intent_path = Kura::rollback_intent_path(&blocks_root);
    let temp_path = intent_path.with_extension("norito.tmp");
    fs::write(
        &temp_path,
        norito::to_bytes(&intent).expect("encode rollback intent"),
    )
    .expect("write temporary rollback intent");
    fs::File::open(&temp_path)
        .expect("open temporary intent")
        .sync_data()
        .expect("sync temporary intent");
    sync_dir(&blocks_root).expect("sync temporary intent directory entry");

    let (reopened, BlockCount(block_count)) = Kura::new(&config, &lane_config)
        .expect("startup should promote and complete valid temporary intent");
    assert_eq!(block_count, 1);
    assert_eq!(
        reopened.block_hash_at_height(nonzero!(1_usize)),
        Some(hashes[0])
    );
    assert!(!intent_path.exists());
    assert!(!temp_path.exists());
}

fn offline_top_up_entrypoint_for_index(
    request_operation_id: [u8; 32],
    authorization_operation_id: [u8; 32],
) -> TransactionEntrypoint {
    offline_top_up_entrypoint_for_index_with_outer_authority(
        request_operation_id,
        authorization_operation_id,
        &SAMPLE_GENESIS_ACCOUNT_KEYPAIR,
    )
}

fn offline_top_up_entrypoint_for_index_with_outer_authority(
    request_operation_id: [u8; 32],
    authorization_operation_id: [u8; 32],
    outer_authority: &KeyPair,
) -> TransactionEntrypoint {
    let network_id = test_network_id(b"kura-offline-operation-index-network");
    let domain_id = DomainId::try_new("offline", "index").expect("fixture domain id");
    let definition = AssetDefinitionId::derive_from_components(
        domain_id,
        "cash".parse().expect("fixture asset definition name"),
    );
    let amount = KagemushaScaledAmountV2 {
        atomic_units: 7,
        scale: 0,
    };
    let request = KagemushaRecursiveSpendTopUpRequestV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
        asset: AssetId::new(definition.clone(), SAMPLE_GENESIS_ACCOUNT_ID.clone()),
        amount,
        current_note: KagemushaSpendableNoteDescriptorV2 {
            network_id,
            asset: definition.clone(),
            note_commitment: [0x31; 32],
            spend_nullifier: [0x32; 32],
            amount,
        },
        shield_evidence: KagemushaTopUpShieldEvidenceV2 {
            initial_root: [0x35; 32],
            finalized_root: [0x36; 32],
            leaf_index: 0,
            proof: {
                let mut attachment = ProofAttachment::new_ref(
                    crate::zk::ZK_BACKEND_HALO2_IPA.into(),
                    ProofBox::new(crate::zk::ZK_BACKEND_HALO2_IPA.to_owned(), vec![0x37]),
                    VerifyingKeyId::new(
                        crate::zk::ZK_BACKEND_HALO2_IPA,
                        "kagemusha-topup-shield-v2",
                    ),
                );
                attachment.vk_commitment = Some([0x38; 32]);
                attachment
            },
        },
        artifact_binding: KagemushaRecursiveSpendArtifactBindingV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
            generation: "kura-operation-index-fixture".to_owned(),
            manifest_sha256: [0x39; 32],
        },
        operation_id: request_operation_id,
        authorization: KagemushaRequestAuthorizationV2 {
            authority: SAMPLE_GENESIS_ACCOUNT_ID.clone(),
            device_id: "kura-operation-index-device".to_owned(),
            asset_definition_id: definition,
            operation_id: authorization_operation_id,
            issued_at_ms: 1,
            expires_at_ms: u64::MAX,
            nonce: [0x33; 32],
            payload_digest: [0x34; 32],
            registration_hash: [0x35; 32],
            hardware_assertion:
                iroha_data_model::offline::KagemushaOnlineHardwareAssertionV1::AndroidKeyMint(
                    iroha_data_model::offline::KagemushaAndroidKeyMintHardwareAssertionV1 {
                        signature:
                            iroha_data_model::offline::KagemushaDeviceSignatureV2::from_raw_bytes(
                                &[1_u8; 64],
                            )
                            .expect("fixture hardware signature"),
                    },
                ),
        },
    };
    let outer_authority_id = AccountId::new(outer_authority.public_key().clone());
    let transaction = TransactionBuilder::new(
        network_id,
        outer_authority_id,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(Executable::Batch(
        vec![
            iroha_data_model::transaction::ExecutableBatchItem::Instruction(InstructionBox::from(
                TopUpKagemushaRecursiveV4::new(request),
            )),
        ]
        .into(),
    ))
    .sign(outer_authority.private_key());
    TransactionEntrypoint::External(transaction)
}

fn merge_entry_with_indexed_entrypoint(entrypoint: TransactionEntrypoint) -> MergeLedgerEntry {
    let entrypoint_hashes = vec![Hash::from(entrypoint.hash())];
    let results = vec![TransactionResult::from(Ok(DataTriggerSequence::default()))];
    let result_hashes = results
        .iter()
        .map(|result| Hash::from(result.hash()))
        .collect::<Vec<_>>();
    let validator = KeyPair::try_from_seed(vec![0xD1; 32], Algorithm::BlsNormal)
        .expect("derive deterministic Kura merge-index fixture validator");
    let validator_set = vec![PeerId::new(validator.public_key().clone())];
    let validator_count =
        u32::try_from(validator_set.len()).expect("fixture validator count fits u32");
    let min_quorum = DualQuorum::count_threshold(validator_count)
        .expect("non-empty fixture validator set has a quorum");
    let lane_incarnation = Hash::new(b"kura-index-refresh-lane-incarnation");
    let mut descriptor = LaneBlockDescriptorV1 {
        lane_id: LaneId::SINGLE,
        dataspace_id: DataSpaceId::UNIVERSAL,
        lane_incarnation,
        proposal_height: 2,
        previous_lane_block_height: 0,
        previous_lane_block_descriptor_hash: None,
        lane_block_height: 1,
        lane_block_view: 0,
        subject_hash: Hash::new(b"kura-index-refresh-subject"),
        payload_ownership_hash: Hash::new(b"kura-index-refresh-ownership"),
        rbc_instance_hash: Hash::new(b"kura-index-refresh-rbc"),
        accepted_candidate_indices: vec![0],
        accepted_transaction_hashes: entrypoint_hashes.clone(),
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set: validator_set.clone(),
        validator_count,
        min_quorum,
        qc_mode_tag: "kura-index-refresh-test".to_owned(),
        descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
    };
    descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
    let mut proposal = LaneBlockProposalV1 {
        descriptor,
        proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
        payload_block_hint: None,
    };
    proposal.proposal_hash = proposal.computed_proposal_hash();
    crate::lane_consensus::validate_lane_block_proposal(&proposal)
        .expect("merge-index fixture proposal must satisfy production ingress validation");
    let lane_qc = |phase| LaneBlockQcV1 {
        body: proposal.vote_body(phase),
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set: validator_set.clone(),
        signers_bitmap: Vec::new(),
        bls_aggregate_signature: Vec::new(),
        payload_availability_qc: None,
    };
    let prepare_qc = lane_qc(CertPhase::Prepare);
    let commit_qc = lane_qc(CertPhase::Commit);
    let settlement_commitment = LaneBlockCommitment {
        block_height: 1,
        lane_id: LaneId::SINGLE,
        lane_incarnation,
        dataspace_id: DataSpaceId::UNIVERSAL,
        tx_count: 0,
        total_local_amount: "0".parse().expect("valid settlement quantity"),
        total_xor_due: "0".parse().expect("valid settlement quantity"),
        total_xor_after_haircut: "0".parse().expect("valid settlement quantity"),
        total_xor_variance: "0".parse().expect("valid settlement quantity"),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    };
    let execution = MergeLaneExecution {
        source_bundle: vec![1],
        source_bundle_hash: Hash::new(b"kura-index-refresh-source"),
        proposal: proposal.clone(),
        origin_proposal: proposal,
        prepare_qc,
        commit_qc,
        signer_proofs: Vec::new(),
        autonomous_network_id: test_network_id(b"kura-index-refresh-genesis"),
        autonomous_epoch: 0,
        autonomous_payload_hash: Hash::new(b"kura-index-refresh-payload"),
        entrypoint_hashes,
        entrypoints: vec![entrypoint],
        reservation_keys: vec![vec![1]],
        routing_plans: vec![vec![2]],
        native_amx_receipts: vec![None],
        result_hashes,
        results,
        settlement_hash: iroha_data_model::nexus::compute_settlement_hash(&settlement_commitment)
            .expect("fixture settlement hashes canonically"),
        settlement_commitment,
    };
    let lanes = vec![execution];
    let entrypoint_merkle_root = crate::merge::merge_execution_entrypoint_merkle_root(&lanes)
        .expect("fixture has an entrypoint");
    let result_merkle_root =
        crate::merge::merge_execution_result_merkle_root(&lanes).expect("fixture has a result");
    let base_state_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"kura-index-refresh-base-state"));
    let write_set_root = Hash::new(b"kura-index-refresh-write-set");
    let mut batch = MergeExecutionBatch {
        version: 1,
        base_state_height: 1,
        base_state_hash,
        application_block_header: BlockHeader::new(
            nonzero!(2_u64),
            Some(HashOf::from_untyped_unchecked(Hash::new(
                b"kura-index-refresh-parent",
            ))),
            None,
            None,
            1,
            0,
        ),
        execution_root: crate::merge::merge_execution_root(&lanes),
        lanes,
        entrypoint_count: 1,
        entrypoint_merkle_root,
        result_merkle_root,
        application_write_set_root: Hash::new(b"kura-index-refresh-application-write-set"),
        write_set_root,
        expected_post_state_hash: crate::merge::merge_expected_post_state_hash(
            1,
            base_state_hash,
            write_set_root,
        ),
        batch_hash: Hash::prehashed([0; Hash::LENGTH]),
    };
    batch.batch_hash = crate::merge::merge_execution_batch_hash(&batch);
    let mut entry = sample_merge_entry(1);
    entry.execution_batch = Some(batch);
    entry
}

fn merge_entry_with_indexed_reservation(
    epoch: u64,
    salt: u8,
) -> (
    MergeLedgerEntry,
    HashOf<SignedTransaction>,
    LaneQueueReservationKeyV2,
) {
    let entrypoint = offline_top_up_entrypoint_for_index([salt; 32], [salt.saturating_add(1); 32]);
    let accepted = AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(entrypoint.clone()));
    let transaction_hash = accepted.hash();
    let mut entry = merge_entry_with_indexed_entrypoint(entrypoint.clone());
    entry.epoch_id = epoch;
    let execution = entry
        .execution_batch
        .as_mut()
        .and_then(|batch| batch.lanes.first_mut())
        .expect("reservation fixture has one execution");
    let descriptor = &execution.proposal.descriptor;
    let routing_plan = RoutingPlan::single(crate::queue::RoutingDecision::new(
        descriptor.lane_id,
        descriptor.dataspace_id,
    ));
    let reservation = LaneQueueReservationKeyV2 {
        version: LaneQueueReservationKeyV2::VERSION,
        signed_transaction_hash: transaction_hash,
        entrypoint_hash: entrypoint.hash(),
        queue_plan_admission_binding_hash: Hash::new_from_chunks(&[
            b"kura-queue-plan-admission-binding",
            &[salt],
        ]),
        routing_plan_digest: routing_plan.digest(),
        coordinator_leg: routing_plan.coordinator_leg(),
        lane_id: descriptor.lane_id,
        dataspace_id: descriptor.dataspace_id,
        lane_incarnation: descriptor.lane_incarnation,
        proposal_height: descriptor.proposal_height,
        lane_block_height: descriptor.lane_block_height,
        lane_block_view: descriptor.lane_block_view,
        reservation_owner_hash: Hash::new([salt]),
        proposal_identity_hash: execution.proposal.proposal_hash,
    };
    execution.reservation_keys =
        vec![norito::to_bytes(&reservation).expect("encode canonical indexed reservation fixture")];
    execution.routing_plans =
        vec![norito::to_bytes(&routing_plan).expect("encode canonical routing fixture")];
    (entry, transaction_hash, reservation)
}

fn store_indexed_reservation_carrier(
    kura: &Kura,
    salt: u8,
) -> (
    HashOf<SignedTransaction>,
    LaneQueueReservationKeyV2,
    MergeLedgerFrameIndex,
) {
    let mut blocks = DummyBlocks::new();
    let genesis = blocks.next();
    let raw_carrier = blocks.next();
    let (mut entry, transaction_hash, reservation) = merge_entry_with_indexed_reservation(1, salt);
    let batch = entry
        .execution_batch
        .as_mut()
        .expect("reservation fixture has an execution batch");
    batch.application_block_header =
        crate::merge::merge_application_header_from_carrier(&raw_carrier.header());
    batch.batch_hash = crate::merge::merge_execution_batch_hash(batch);
    let descriptor = batch
        .lanes
        .first()
        .expect("reservation fixture has one lane execution")
        .proposal
        .descriptor
        .clone();
    let lane_entry = kura
        .lane_storage_entry(descriptor.lane_id)
        .expect("reservation fixture targets an active lane");
    kura.install_lane_incarnation_marker_for_test(&lane_entry, descriptor.lane_incarnation, 0)
        .expect("install reservation fixture lane incarnation");
    let mut executed_carrier = raw_carrier.as_ref().clone();
    attach_ok_results_to_block(&mut executed_carrier);
    let carrier = bind_merge_entry_to_carrier(Arc::new(executed_carrier), &mut entry);
    assert!(
        carrier.has_results(),
        "a canonical reservation carrier must contain execution results"
    );
    assert_eq!(
        carrier.results().count(),
        carrier.external_entrypoints_cloned().count(),
        "the reservation carrier must contain one result per ordinary entrypoint"
    );
    assert_eq!(
        entry
            .execution_batch
            .as_ref()
            .expect("reservation fixture has an execution batch")
            .application_block_header,
        crate::merge::merge_application_header_from_carrier(&carrier.header()),
        "the reservation batch must bind the canonical stripped carrier header"
    );
    kura.store_block(genesis)
        .expect("store reservation genesis");
    kura.store_block_with_merge_entry(carrier, &entry)
        .expect("store reservation merge carrier");
    let _ = persist_v2_finality_chain_through(kura, nonzero!(2_usize));
    let frame = kura.merge_log.lock().frames_by_epoch[&1];
    (transaction_hash, reservation, frame)
}

fn v2_finality_fixture_keys() -> Vec<KeyPair> {
    let mut keypairs = (0_u8..4)
        .map(|index| {
            KeyPair::try_from_seed(
                vec![0xA0_u8.saturating_add(index); 32],
                Algorithm::BlsNormal,
            )
            .expect("derive deterministic Kura finality BLS fixture key")
        })
        .collect::<Vec<_>>();
    keypairs.sort_by(|left, right| {
        PeerId::new(left.public_key().clone()).cmp(&PeerId::new(right.public_key().clone()))
    });
    keypairs
}

fn v2_finality_fixture_execution_commitment() -> ExecutionCommitment {
    ExecutionCommitment::new_without_merge_carrier(
        Hash::new(b"kura finality parent state"),
        Hash::new(b"kura finality post state"),
        Hash::new(b"kura finality ordinary writes"),
        None,
        0,
        1,
        Hash::new(b"Kura fixture executed block wire placeholder"),
    )
    .expect("canonical Kura finality fixture execution commitment")
}

fn v2_finality_artifact_for_block_with_keys(
    block: &SignedBlock,
    parent: Option<&V2FinalityArtifact>,
    keypairs: &[KeyPair],
    execution_commitment: ExecutionCommitment,
) -> V2FinalityArtifact {
    let merge_carrier = block
        .execution_context()
        .and_then(|bundle| bundle.merge_entry.as_ref())
        .map(|reference| {
            iroha_data_model::block::consensus_v2::MergeCarrierCommitmentV1::new(
                reference.entry_hash,
            )
        });
    v2_finality_artifact_for_block_with_keys_and_merge_carrier(
        block,
        parent,
        keypairs,
        execution_commitment,
        merge_carrier,
    )
}

fn v2_finality_artifact_for_block_with_keys_and_merge_carrier(
    block: &SignedBlock,
    parent: Option<&V2FinalityArtifact>,
    keypairs: &[KeyPair],
    mut execution_commitment: ExecutionCommitment,
    merge_carrier: Option<iroha_data_model::block::consensus_v2::MergeCarrierCommitmentV1>,
) -> V2FinalityArtifact {
    let roster = keypairs
        .iter()
        .map(|keypair| ValidatorPower {
            validator: PeerId::new(keypair.public_key().clone()),
            power: 1,
        })
        .collect::<Vec<_>>();
    let height = block.header().height().get();
    assert_eq!(
        parent.map_or(1, |artifact| artifact.height.saturating_add(1)),
        height,
        "fixture finality artifacts must form a contiguous chain"
    );
    let context = HeightContext {
        network_id: test_network_id(b"kura-v2-finality-test"),
        protocol_version: PROTOCOL_VERSION,
        height,
        epoch: 0,
        epoch_end_height: 100,
        next_epoch_snapshot: None,
        mode: ConsensusMode::Permissioned,
        parent_commit_qc: parent.map(|artifact| artifact.commit_qc.clone()),
        snapshot_bootstrap: None,
        quorum: DualQuorum::from_roster(&roster).expect("valid fixture quorum"),
        roster,
        nexus_amx_context_hash: Hash::new(b"kura finality nexus amx context"),
        execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
        da_layout: DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 1024,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 4096,
            max_chunk_count: 8,
        },
        leader_seed: [0x42; 32],
    };
    let executed_block_wire = block.encode_wire().expect("canonical executed block wire");
    execution_commitment.executed_block_wire_len =
        u64::try_from(executed_block_wire.len()).expect("fixture wire length fits u64");
    execution_commitment.executed_block_wire_hash = Hash::new(&executed_block_wire);
    execution_commitment.merge_carrier = merge_carrier;
    let subject = BlockSubject {
        parent_block_hash: block.header().prev_block_hash(),
        block_hash: block.hash(),
        payload_hash: block
            .canonical_proposal_wire_hash()
            .expect("canonical proposal block wire"),
    };
    let round = ConsensusRound {
        context_id: context.id(),
        height,
        view: block.header().view_change_index(),
    };
    let mut commit_qc = QuorumCertificate {
        round,
        proposal_round: round,
        phase: GlobalPhase::Commit,
        subject,
        execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![1],
    };
    let preimage = commit_qc
        .signer_preimage(&context, 0)
        .expect("valid Kura finality fixture signer");
    let signatures = commit_qc
        .signers
        .iter()
        .map(|index| {
            Signature::try_new(
                keypairs[usize::try_from(*index).expect("fixture signer index")].private_key(),
                &preimage,
            )
            .expect("sign Kura finality fixture vote")
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    let signature_refs = signatures.iter().map(Vec::as_slice).collect::<Vec<_>>();
    commit_qc.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
        .expect("aggregate Kura finality fixture votes");
    let validator_set_pops = keypairs
        .iter()
        .map(|keypair| {
            bls_normal_pop_prove(keypair.private_key()).expect("derive Kura finality fixture PoP")
        })
        .collect();
    let artifact = V2FinalityArtifact::new(context, subject, commit_qc, validator_set_pops);
    artifact
        .verify()
        .expect("Kura finality fixture is cryptographically valid");
    artifact
}

fn v2_finality_artifact_for_block(block: &SignedBlock) -> V2FinalityArtifact {
    assert_eq!(
        block.header().height().get(),
        1,
        "single-artifact fixture requires a genesis-height block"
    );
    v2_finality_artifact_for_block_with_keys(
        block,
        None,
        &v2_finality_fixture_keys(),
        v2_finality_fixture_execution_commitment(),
    )
}

fn v2_finality_artifact_for_block_with_execution(
    block: &SignedBlock,
    execution_commitment: ExecutionCommitment,
) -> V2FinalityArtifact {
    assert_eq!(
        block.header().height().get(),
        1,
        "single-artifact fixture requires a genesis-height block"
    );
    v2_finality_artifact_for_block_with_keys(
        block,
        None,
        &v2_finality_fixture_keys(),
        execution_commitment,
    )
}

fn v2_finality_artifacts_for_chain(blocks: &[Arc<SignedBlock>]) -> Vec<V2FinalityArtifact> {
    let keypairs = v2_finality_fixture_keys();
    let mut artifacts = Vec::with_capacity(blocks.len());
    for block in blocks {
        let artifact = v2_finality_artifact_for_block_with_keys(
            block,
            artifacts.last(),
            &keypairs,
            v2_finality_fixture_execution_commitment(),
        );
        artifacts.push(artifact);
    }
    artifacts
}

pub(super) fn persist_v2_finality_chain_through(
    kura: &Kura,
    height: NonZeroUsize,
) -> Vec<V2FinalityArtifact> {
    let blocks = (1..=height.get())
        .map(|height| {
            kura.get_block_without_merge_sidecar(
                NonZeroUsize::new(height).expect("fixture height is non-zero"),
            )
            .expect("fixture canonical block body is available")
        })
        .collect::<Vec<_>>();
    let artifacts = v2_finality_artifacts_for_chain(&blocks);
    for artifact in &artifacts {
        let _ = kura
            .store_v2_finality_artifact(artifact)
            .expect("persist exact fixture finality chain");
    }
    artifacts
}

fn retained_archive_sccp_payload(nonce: u64) -> iroha_sccp::SccpPayloadV1 {
    iroha_sccp::SccpPayloadV1::Transfer(iroha_sccp::TransferPayloadV1 {
        version: 1,
        source_domain: iroha_sccp::SCCP_DOMAIN_SORA,
        dest_domain: iroha_sccp::SCCP_DOMAIN_ETH,
        nonce,
        route_revision: 1,
        asset_home_domain: iroha_sccp::SCCP_DOMAIN_SORA,
        asset_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        asset_id: b"xor".to_vec(),
        amount: 77,
        sender_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        sender: b"sora:retained-archive".to_vec(),
        recipient_codec: iroha_sccp::SCCP_CODEC_EVM_ADDRESS20,
        recipient: [0x22; 20].to_vec(),
        route_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        route_id: iroha_sccp::SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1
            .as_bytes()
            .to_vec(),
    })
}

fn retained_archive_empty_block(previous: Option<&SignedBlock>) -> Arc<SignedBlock> {
    Arc::new(
        BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
            .chain(0, previous)
            .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
            .unpack(|_| {})
            .into(),
    )
}

fn retained_archive_sccp_block(
    previous: &SignedBlock,
    payloads: &[iroha_sccp::SccpPayloadV1],
) -> Arc<SignedBlock> {
    let mut commitments = Vec::new();
    let records = payloads
        .iter()
        .map(|payload| {
            let payload_bytes = iroha_sccp::canonical_sccp_payload_bytes(payload)
                .expect("retained archive payload encodes canonically");
            let record = crate::bridge::test_record_sccp_message(payload_bytes);
            let validated = crate::bridge::validate_recorded_sccp_message_payload_bytes(
                record.context,
                &record.payload_bytes,
            )
            .expect("retained archive SCCP fixture validates");
            commitments.push(validated.commitment);
            record
        })
        .collect::<Vec<_>>();
    let transaction = TransactionBuilder::new(
        test_network_id(b"kura-retained-sccp-archive"),
        SAMPLE_GENESIS_ACCOUNT_ID.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions(records)
    .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
    let accepted = vec![AcceptedTransaction::new_unchecked(Cow::Owned(transaction))];
    let root = iroha_sccp::commitment_merkle_root(&commitments);
    let mut block: SignedBlock = BlockBuilder::new(accepted)
        .chain(0, Some(previous))
        .with_sccp_commitment_root(root)
        .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
        .unpack(|_| {})
        .into();
    attach_ok_results_to_block(&mut block);
    crate::bridge::validate_sccp_commitment_root_for_signed_block(&block)
        .expect("retained archive block commits its successful SCCP records");
    Arc::new(block)
}

fn store_retained_archive_chain(
    kura: &Kura,
) -> (Vec<Arc<SignedBlock>>, Vec<iroha_sccp::SccpPayloadV1>) {
    let genesis = retained_archive_empty_block(None);
    let mut payloads = vec![
        retained_archive_sccp_payload(41),
        retained_archive_sccp_payload(7),
    ];
    let first_id = crate::bridge::test_sccp_outbound_message_key(&payloads[0]).message_id;
    let second_id = crate::bridge::test_sccp_outbound_message_key(&payloads[1]).message_id;
    if first_id < second_id {
        payloads.swap(0, 1);
    }
    let sccp = retained_archive_sccp_block(&genesis, &payloads);
    let third = retained_archive_empty_block(Some(&sccp));
    let fourth = retained_archive_empty_block(Some(&third));
    let blocks = vec![genesis, sccp, third, fourth];
    for block in &blocks {
        kura.store_block(Arc::clone(block))
            .expect("store retained archive fixture block");
    }
    (blocks, payloads)
}

fn replace_v2_finality_record_artifact(path: &Path, artifact: V2FinalityArtifact) {
    let bytes = std::fs::read(path).expect("read Kura finality record");
    let mut cursor = bytes.as_slice();
    let mut record =
        KuraV2FinalityRecord::decode_all(&mut cursor).expect("decode Kura finality record");
    record.artifact = artifact;
    std::fs::write(path, record.encode()).expect("replace Kura finality record artifact");
}

fn assert_v2_commit_receipt_matches_artifact(
    receipt: &KuraV2CommitReceipt,
    artifact: &V2FinalityArtifact,
) {
    assert_eq!(receipt.height(), artifact.height);
    assert_eq!(receipt.block_hash(), artifact.block_hash);
    assert_eq!(receipt.context_id(), artifact.context_id());
    assert_eq!(receipt.subject(), artifact.subject);
    assert_eq!(receipt.certificate(), artifact.commit_qc.as_ref());
    assert_eq!(receipt.artifact_hash(), HashOf::new(artifact));
}

fn kagemusha_topup_witness(
    operation_id: [u8; 32],
    anchor_digest: [u8; 32],
) -> (ExecWitness, ExecutionCommitment) {
    let mut key = vec![crate::sumeragi::smt::KAGEMUSHA_V4_TOPUP_ANCHOR_WITNESS_KEY_TAG];
    key.extend_from_slice(&operation_id);
    let receiver_snapshot =
        iroha_data_model::offline::KagemushaActiveReceiverSnapshotV1::unavailable(
            1,
            1,
            b"Kura top-up fixture has no governed receiver policy",
        )
        .expect("valid unavailable receiver snapshot");
    let validation_fee_snapshot =
        iroha_data_model::validation_fee::ValidationFeePolicySnapshotCommitmentV1::from_registry(
            1, None,
        );
    let witness = ExecWitness {
        reads: Vec::new(),
        writes: vec![
            ExecKv {
                key,
                value: anchor_digest.to_vec(),
            },
            ExecKv {
                key: iroha_data_model::offline::KAGEMUSHA_ACTIVE_RECEIVER_WITNESS_KEY_V1.to_vec(),
                value: norito::to_bytes(&receiver_snapshot.commitment)
                    .expect("encode receiver snapshot commitment"),
            },
            ExecKv {
                key: iroha_data_model::validation_fee::VALIDATION_FEE_POLICY_WITNESS_KEY_V1
                    .to_vec(),
                value: norito::to_bytes(&validation_fee_snapshot)
                    .expect("encode validation-fee snapshot commitment"),
            },
        ],
        fastpq_transcripts: Vec::new(),
        fastpq_batches: Vec::new(),
    };
    let manifest = crate::sumeragi::exec::NativeAmxApplicationManifestV1::empty(
        1,
        Hash::new(b"Kura top-up fixture executed block wire placeholder"),
    );
    let commitment =
        crate::sumeragi::exec::execution_commitment_from_witness_for_tests(&witness, &manifest)
            .expect("derive top-up execution commitment");
    (witness, commitment)
}

fn kagemusha_receiver_only_witness(
    height: u64,
    evaluated_at_ms: u64,
) -> (ExecWitness, ExecutionCommitment) {
    let receiver_snapshot =
        iroha_data_model::offline::KagemushaActiveReceiverSnapshotV1::unavailable(
            height,
            evaluated_at_ms,
            b"Kura fixture has no governed receiver policy",
        )
        .expect("valid unavailable receiver snapshot");
    let validation_fee_snapshot =
        iroha_data_model::validation_fee::ValidationFeePolicySnapshotCommitmentV1::from_registry(
            height, None,
        );
    let witness = ExecWitness {
        reads: Vec::new(),
        writes: vec![
            ExecKv {
                key: iroha_data_model::offline::KAGEMUSHA_ACTIVE_RECEIVER_WITNESS_KEY_V1.to_vec(),
                value: norito::to_bytes(&receiver_snapshot.commitment)
                    .expect("encode receiver snapshot commitment"),
            },
            ExecKv {
                key: iroha_data_model::validation_fee::VALIDATION_FEE_POLICY_WITNESS_KEY_V1
                    .to_vec(),
                value: norito::to_bytes(&validation_fee_snapshot)
                    .expect("encode validation-fee snapshot commitment"),
            },
        ],
        fastpq_transcripts: Vec::new(),
        fastpq_batches: Vec::new(),
    };
    let manifest = crate::sumeragi::exec::NativeAmxApplicationManifestV1::empty(
        1,
        Hash::new(b"Kura receiver-only fixture executed block wire placeholder"),
    );
    let commitment =
        crate::sumeragi::exec::execution_commitment_from_witness_for_tests(&witness, &manifest)
            .expect("derive receiver-only execution commitment");
    (witness, commitment)
}

#[test]
fn checked_keypair_helpers_preserve_requested_algorithm() {
    assert_eq!(checked_keypair().algorithm(), Algorithm::default());
    assert_eq!(
        checked_keypair_with_algorithm(Algorithm::BlsNormal).algorithm(),
        Algorithm::BlsNormal
    );
}

#[test]
fn blank_kura_for_testing_uses_isolated_canonical_primary_storage() {
    let kura = Kura::blank_kura_for_testing();
    let block_store_path = kura.block_store.lock().path_to_blockchain.clone();
    let active_blocks_path = kura.active_blocks_dir.lock().clone();
    let active_merge_path = kura.active_merge_path.lock().clone();
    let lane_config = RuntimeLaneConfig::default();
    let expected_blocks = lane_config.primary().blocks_dir(kura.store_root());
    let expected_merge = lane_config.primary().merge_log_path(kura.store_root());

    assert!(
        block_store_path.is_absolute(),
        "test Kura block store must live under an isolated temporary directory"
    );
    assert_ne!(
        block_store_path,
        std::env::current_dir().expect("current directory"),
        "test Kura must not write blocks.* into the crate working directory"
    );
    assert_eq!(
        block_store_path, expected_blocks,
        "test Kura must use the canonical primary-lane block geometry"
    );
    assert_eq!(
        active_blocks_path, expected_blocks,
        "active block storage must match the canonical primary-lane geometry"
    );
    assert_eq!(
        active_merge_path, expected_merge,
        "test Kura must use the canonical primary-lane merge geometry"
    );
    assert!(
        expected_blocks.is_dir(),
        "canonical primary block directory must exist"
    );
    for name in [
        DATA_FILE_NAME,
        INDEX_FILE_NAME,
        HASHES_FILE_NAME,
        COUNT_FILE_NAME,
    ] {
        let path = expected_blocks.join(name);
        let metadata =
            std::fs::symlink_metadata(&path).expect("inspect blank canonical journal file");
        assert!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "blank test Kura must initialize canonical journal file {name}"
        );
        if name != COUNT_FILE_NAME {
            assert_eq!(
                metadata.len(),
                0,
                "blank canonical journal file {name} must be empty"
            );
        }
    }
    assert_eq!(
        kura.exact_durable_blocks_count()
            .expect("read blank durable height"),
        0
    );
    assert!(
        expected_merge.is_file(),
        "canonical primary merge ledger must exist"
    );
}

#[test]
fn blank_kura_applies_staged_pre_genesis_nexus_geometry() {
    let kura = Kura::blank_kura_for_testing();
    let store_root = kura.store_root().to_path_buf();
    let lane_zero = ModelLaneConfig::default();
    let lane_one = ModelLaneConfig {
        id: LaneId::new(1),
        alias: "staged-secondary".to_owned(),
        ..ModelLaneConfig::default()
    };
    let catalog = LaneCatalog::new(nonzero!(2_u32), vec![lane_zero, lane_one])
        .expect("staged two-lane catalog");
    let lane_config = RuntimeLaneConfig::from_catalog(&catalog);
    let mut state = State::new_with_chain_for_testing(
        World::default(),
        Arc::clone(&kura),
        LiveQueryStore::start_test(),
        ChainId::from("staged-genesis-geometry"),
    );

    state
        .set_nexus(iroha_config::parameters::actual::Nexus {
            enabled: true,
            lane_catalog: catalog.clone(),
            configured_lane_catalog: catalog,
            lane_config: lane_config.clone(),
            ..Default::default()
        })
        .expect("fresh staged state must extend authenticated primary geometry");

    for entry in lane_config.entries() {
        assert!(entry.blocks_dir(&store_root).is_dir());
        assert!(entry.merge_log_path(&store_root).is_file());
    }
}

#[test]
fn temporary_configured_kura_owns_authenticated_storage_lifetime() {
    let configured = LaneCatalog::default();
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    let ignored_store = PathBuf::from("must-not-be-used-by-temporary-kura");
    let config = kura_config_for_path(&ignored_store, BLOCKS_IN_MEMORY);

    let kura = Kura::new_temporary_with_configured_lane_catalog(&config, &lane_config, &configured)
        .expect("initialize temporary authenticated Kura");
    let store_root = kura.store_root().to_path_buf();
    assert!(store_root.is_dir());
    assert_ne!(store_root, ignored_store);
    assert!(lane_config.primary().blocks_dir(&store_root).is_dir());
    assert!(lane_config.primary().merge_log_path(&store_root).is_file());

    drop(kura);
    assert!(
        !store_root.exists(),
        "temporary authenticated Kura must remove its storage when its final owner drops"
    );
}

#[test]
fn store_root_lock_rejects_a_second_live_kura_and_releases_on_drop() {
    let temp_dir = TempDir::new().expect("create Kura store root");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = RuntimeLaneConfig::default();
    let (first, _) = Kura::new(&config, &lane_config).expect("open first Kura");
    let expected_lock_path = std::fs::canonicalize(temp_dir.path())
        .expect("canonical Kura root")
        .join(STORE_ROOT_LOCK_FILE_NAME);

    assert!(matches!(
        Kura::new(&config, &lane_config),
        Err(Error::Locked(path)) if path == expected_lock_path
    ));

    drop(first);
    let (reopened, _) = Kura::new(&config, &lane_config)
        .expect("the OS lock must be released when the first Kura is dropped");
    drop(reopened);
}

#[cfg(unix)]
#[test]
fn store_root_lock_rejects_a_symlink_without_touching_its_target() {
    use std::os::unix::fs::symlink;

    let temp_dir = TempDir::new().expect("create Kura store root");
    let victim = temp_dir.path().join("lock-victim");
    let victim_bytes = b"must remain untouched";
    std::fs::write(&victim, victim_bytes).expect("create lock victim");
    let lock_path = temp_dir.path().join(STORE_ROOT_LOCK_FILE_NAME);
    symlink(&victim, &lock_path).expect("plant lockfile symlink");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);

    assert!(matches!(
        Kura::new(&config, &RuntimeLaneConfig::default()),
        Err(Error::IO(error, observed_path))
            if error.kind() == ErrorKind::InvalidData && observed_path == lock_path
    ));
    assert_eq!(
        std::fs::read(&victim).expect("read lock victim"),
        victim_bytes
    );
}

#[cfg(unix)]
#[test]
fn store_root_lock_canonicalizes_a_symlinked_root_for_all_kura_paths() {
    use std::os::unix::fs::symlink;

    let real_root = TempDir::new().expect("create real Kura root");
    let alias_parent = TempDir::new().expect("create Kura alias parent");
    let alias_root = alias_parent.path().join("kura-alias");
    symlink(real_root.path(), &alias_root).expect("create Kura root alias");
    let canonical_root = std::fs::canonicalize(real_root.path()).expect("canonical real root");
    let mut alias_config = kura_config_for_dir(&real_root, BLOCKS_IN_MEMORY);
    alias_config.store_dir = WithOrigin::inline(alias_root);
    let real_config = kura_config_for_dir(&real_root, BLOCKS_IN_MEMORY);
    let lane_config = RuntimeLaneConfig::default();

    let (aliased, _) = Kura::new(&alias_config, &lane_config).expect("open aliased Kura root");
    assert_eq!(aliased.store_root, canonical_root);
    assert!(
        aliased
            .active_blocks_dir
            .lock()
            .starts_with(&canonical_root)
    );
    assert!(matches!(
        Kura::new(&real_config, &lane_config),
        Err(Error::Locked(path))
            if path == canonical_root.join(STORE_ROOT_LOCK_FILE_NAME)
    ));

    drop(aliased);
    let (reopened, _) = Kura::new(&real_config, &lane_config)
        .expect("canonical root must reopen after aliased owner drops");
    drop(reopened);
}

#[test]
fn v2_finality_artifact_roundtrips_with_unforgeable_receipt() {
    let kura = Kura::blank_kura_for_testing();
    let block = DummyBlocks::new().next();
    kura.store_block(Arc::clone(&block))
        .expect("store canonical block");
    let artifact = v2_finality_artifact_for_block(&block);

    let receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("persist finality artifact");

    assert_eq!(receipt.height(), artifact.height);
    assert_eq!(receipt.block_hash(), block.hash());
    assert_eq!(receipt.context_id(), artifact.context_id());
    assert_eq!(receipt.subject(), artifact.subject);
    assert_eq!(receipt.certificate(), artifact.commit_qc.as_ref());
    assert_eq!(receipt.artifact_hash(), HashOf::new(&artifact));
    assert_eq!(
        kura.v2_finality_artifact(artifact.height)
            .expect("read finality artifact"),
        Some(artifact.clone())
    );
    let (recovered, recovered_receipt) = kura
        .v2_finality_artifact_with_receipt(artifact.height)
        .expect("recover artifact and receipt")
        .expect("artifact exists");
    assert_eq!(recovered, artifact);
    assert_eq!(recovered_receipt.height(), receipt.height());
    assert_eq!(recovered_receipt.block_hash(), receipt.block_hash());
    assert_eq!(recovered_receipt.context_id(), receipt.context_id());
    assert_eq!(recovered_receipt.subject(), receipt.subject());
    assert_eq!(recovered_receipt.certificate(), receipt.certificate());
    assert_eq!(recovered_receipt.artifact_hash(), receipt.artifact_hash());
    assert!(kura.v2_finality_artifact_path(artifact.height).is_file());
    assert!(
        !kura
            .v2_finality_artifact_path(artifact.height)
            .with_extension("norito.tmp")
            .exists(),
        "successful atomic write must not leave a temporary artifact"
    );
}

#[test]
fn kagemusha_topup_witness_stage_promotes_only_after_exact_finality_persistence() {
    let kura = Kura::blank_kura_for_testing();
    let block = DummyBlocks::new().next();
    let operation_id = [0xA5; 32];
    let anchor_digest = [0x5B; 32];
    let (witness, mut execution_commitment) = kagemusha_topup_witness(operation_id, anchor_digest);
    execution_commitment.executed_block_wire_len = u64::try_from(
        block
            .encode_wire()
            .expect("canonical top-up fixture wire")
            .len(),
    )
    .expect("canonical top-up fixture wire length fits u64");
    execution_commitment.executed_block_wire_hash = block
        .executed_block_wire_hash()
        .expect("canonical top-up fixture executed wire");
    let artifact = v2_finality_artifact_for_block_with_execution(&block, execution_commitment);

    kura.stage_kagemusha_topup_finality_sidecar(
        artifact.height,
        artifact.block_hash,
        &witness,
        execution_commitment,
    )
    .expect("durably stage top-up witness projection");
    assert!(
        kura.kagemusha_topup_finality_staging_path(artifact.height)
            .is_file()
    );
    assert!(
        !kura
            .kagemusha_topup_finality_sidecar_path(artifact.height)
            .exists(),
        "a witness stage must not be exposed as finalized"
    );

    kura.store_block(Arc::clone(&block))
        .expect("store canonical block");
    let receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("persist exact finality artifact");
    assert!(
        !kura
            .kagemusha_topup_finality_sidecar_path(artifact.height)
            .exists(),
        "persisting finality alone must not silently publish witness bytes"
    );

    kura.promote_kagemusha_topup_finality_sidecar(&artifact, &receipt)
        .expect("promote exact staged sidecar");
    assert!(
        !kura
            .kagemusha_topup_finality_staging_path(artifact.height)
            .exists()
    );
    assert!(
        kura.kagemusha_topup_finality_sidecar_path(artifact.height)
            .is_file()
    );
    let proof = kura
        .kagemusha_topup_finality_proof_v2(artifact.height, operation_id)
        .expect("read compact proof")
        .expect("operation is present");
    assert_eq!(proof.anchor.topup_operation_id, operation_id);
    assert_eq!(proof.anchor.anchor_digest, anchor_digest);
    assert_eq!(proof.commit_qc.certificate, artifact.commit_qc);
    assert_eq!(
        proof.commit_qc.height_context.context_id,
        artifact.context_id()
    );
    proof.validate_structure().expect("durable proof structure");

    kura.promote_kagemusha_topup_finality_sidecar(&artifact, &receipt)
        .expect("promotion is idempotent after a crash/retry");
}

#[test]
fn active_receiver_witness_survives_finality_promotion_restart_and_retry() {
    let temp_dir = TempDir::new().expect("create persistent Kura root");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) = Kura::new(&config, &lane_config).expect("open persistent Kura");
    let block = DummyBlocks::new().next();
    let (witness, mut execution_commitment) = kagemusha_topup_witness([0xC1; 32], [0xC2; 32]);
    execution_commitment.executed_block_wire_len = u64::try_from(
        block
            .encode_wire()
            .expect("canonical receiver fixture wire")
            .len(),
    )
    .expect("canonical receiver fixture wire length fits u64");
    execution_commitment.executed_block_wire_hash = block
        .executed_block_wire_hash()
        .expect("canonical receiver fixture executed wire");
    let artifact = v2_finality_artifact_for_block_with_execution(&block, execution_commitment);

    kura.stage_kagemusha_topup_finality_sidecar(
        artifact.height,
        artifact.block_hash,
        &witness,
        execution_commitment,
    )
    .expect("stage receiver witness before finality");
    kura.store_block(Arc::clone(&block))
        .expect("persist authoritative block");
    let receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("persist authoritative finality artifact");
    kura.promote_kagemusha_topup_finality_sidecar(&artifact, &receipt)
        .expect("promote receiver witness after finality");
    let expected = kura
        .kagemusha_active_receiver_witness_proof_v1(artifact.height)
        .expect("read promoted receiver witness")
        .expect("receiver witness exists");
    assert!(expected.verify(execution_commitment.ordinary_writes_root));
    kura.promote_kagemusha_topup_finality_sidecar(&artifact, &receipt)
        .expect("exact promotion retry is idempotent");
    drop(kura);

    let (reopened, _) = Kura::new(&config, &lane_config).expect("reopen persistent Kura");
    let recovered = reopened
        .kagemusha_active_receiver_witness_proof_v1(artifact.height)
        .expect("read receiver witness after restart")
        .expect("receiver witness survives restart");
    assert_eq!(recovered, expected);
    assert!(recovered.verify(execution_commitment.ordinary_writes_root));
}

#[test]
fn kagemusha_topup_stage_rejects_commitment_and_path_substitution() {
    let kura = Kura::blank_kura_for_testing();
    let block = DummyBlocks::new().next();
    let operation_id = [0xA5; 32];
    let (witness, mut execution_commitment) = kagemusha_topup_witness(operation_id, [0x5B; 32]);
    execution_commitment.executed_block_wire_len = u64::try_from(
        block
            .encode_wire()
            .expect("canonical top-up fixture wire")
            .len(),
    )
    .expect("canonical top-up fixture wire length fits u64");
    execution_commitment.executed_block_wire_hash = block
        .executed_block_wire_hash()
        .expect("canonical top-up fixture executed wire");
    let mut mismatched = execution_commitment;
    mismatched.ordinary_writes_root = Hash::new(b"substituted ordinary root");
    assert!(matches!(
        kura.stage_kagemusha_topup_finality_sidecar(1, block.hash(), &witness, mismatched,),
        Err(Error::KagemushaActiveReceiverFinalitySidecar(_))
    ));
    assert!(
        !kura.kagemusha_topup_finality_staging_path(1).exists(),
        "a commitment mismatch must not leave a stage"
    );

    kura.stage_kagemusha_topup_finality_sidecar(1, block.hash(), &witness, execution_commitment)
        .expect("stage canonical witness");
    let stage_path = kura.kagemusha_topup_finality_staging_path(1);
    let (mut staged, _) = kura
        .decode_staged_kagemusha_topup_finality(&stage_path)
        .expect("read stage")
        .expect("stage exists");
    staged.leaves[0].anchor_digest[0] ^= 1;
    std::fs::write(&stage_path, staged.encode()).expect("substitute staged path leaf");
    kura.store_block(Arc::clone(&block))
        .expect("store canonical block");
    let artifact = v2_finality_artifact_for_block_with_execution(&block, execution_commitment);
    let receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("persist finality artifact");
    assert!(matches!(
        kura.promote_kagemusha_topup_finality_sidecar(&artifact, &receipt),
        Err(Error::KagemushaTopUpFinalitySidecar(_))
    ));
    assert!(
        !kura.kagemusha_topup_finality_sidecar_path(1).exists(),
        "mutated witness-derived path must never be promoted"
    );
}

#[test]
fn immutable_sidecar_publication_never_clobbers_a_racing_destination() {
    let kura = Kura::blank_kura_for_testing();
    let directory = kura.v2_finality_artifact_dir();
    create_dir_all_with_context(&directory).expect("create immutable sidecar directory");
    let path = directory.join("race.norito");
    std::fs::write(&path, b"attacker-won-the-race").expect("publish racing destination");

    assert!(
        !kura
            .write_atomic_synced_noclobber(&path, b"candidate")
            .expect("no-clobber race is not an I/O failure")
    );
    assert_eq!(
        std::fs::read(&path).expect("read racing destination"),
        b"attacker-won-the-race",
        "immutable publication must not overwrite a destination created after lookup"
    );
}

#[test]
fn non_topup_finality_rejects_orphan_staged_and_final_sidecars() {
    let kura = Kura::blank_kura_for_testing();
    let block = DummyBlocks::new().next();
    let (receiver_witness, mut execution_commitment) =
        kagemusha_receiver_only_witness(block.header().height().get(), 1);
    execution_commitment.executed_block_wire_len = u64::try_from(
        block
            .encode_wire()
            .expect("canonical receiver-only fixture wire")
            .len(),
    )
    .expect("canonical receiver-only fixture wire length fits u64");
    execution_commitment.executed_block_wire_hash = block
        .executed_block_wire_hash()
        .expect("canonical receiver-only fixture executed wire");
    let artifact = v2_finality_artifact_for_block_with_execution(&block, execution_commitment);
    kura.stage_kagemusha_topup_finality_sidecar(
        artifact.height,
        artifact.block_hash,
        &receiver_witness,
        execution_commitment,
    )
    .expect("stage mandatory receiver witness for non-top-up block");
    kura.store_block(Arc::clone(&block))
        .expect("store canonical block");
    let receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("persist non-top-up finality");

    let staging_dir = kura.kagemusha_topup_finality_staging_dir();
    create_dir_all_with_context(&staging_dir).expect("create staging directory");
    let staging_path = kura.kagemusha_topup_finality_staging_path(artifact.height);
    std::fs::write(&staging_path, b"orphan-stage").expect("write orphan stage");
    assert!(matches!(
        kura.promote_kagemusha_topup_finality_sidecar(&artifact, &receipt),
        Err(Error::KagemushaTopUpFinalitySidecar(_))
    ));

    std::fs::remove_file(&staging_path).expect("remove orphan stage");
    let final_dir = kura.kagemusha_topup_finality_sidecar_dir();
    create_dir_all_with_context(&final_dir).expect("create final directory");
    let final_path = kura.kagemusha_topup_finality_sidecar_path(artifact.height);
    std::fs::write(&final_path, b"orphan-final").expect("write orphan final sidecar");
    assert!(matches!(
        kura.promote_kagemusha_topup_finality_sidecar(&artifact, &receipt),
        Err(Error::KagemushaTopUpFinalitySidecar(_))
    ));
}

#[test]
fn finality_cache_rejects_a_path_swap_between_decode_and_verification() {
    let kura = Kura::blank_kura_for_testing();
    let block = DummyBlocks::new().next();
    kura.store_block(Arc::clone(&block))
        .expect("store canonical block");
    let artifact = v2_finality_artifact_for_block(&block);
    let receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("persist finality artifact");
    assert_v2_commit_receipt_matches_artifact(&receipt, &artifact);
    let path = kura.v2_finality_artifact_path(artifact.height);
    let directory = kura.v2_finality_artifact_dir();
    let (decoded, read_identity) = kura
        .decode_v2_finality_record_at(&path, &directory)
        .expect("decode stable sidecar")
        .expect("sidecar exists");
    let replacement = path.with_extension("replacement");
    std::fs::write(&replacement, decoded.encode()).expect("write equal-byte replacement");
    std::fs::remove_file(&path).expect("remove decoded path before identity swap");
    std::fs::rename(&replacement, &path).expect("swap path identity");

    assert!(matches!(
        kura.verify_v2_finality_artifact_at(
            &path,
            &directory,
            &decoded.artifact,
            &read_identity,
        ),
        Err(Error::IO(error, _)) if error.kind() == ErrorKind::InvalidData
    ));
}

#[test]
fn v2_finality_crypto_cache_is_bounded_to_an_exact_immutable_sidecar_identity() {
    let kura = Kura::blank_kura_for_testing();
    let block = DummyBlocks::new().next();
    kura.store_block(Arc::clone(&block))
        .expect("store canonical block");
    let artifact = v2_finality_artifact_for_block(&block);
    assert_eq!(
        kura.v2_finality_crypto_verifications
            .load(Ordering::Relaxed),
        0
    );

    let initial_receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("persist and verify finality artifact");
    assert_v2_commit_receipt_matches_artifact(&initial_receipt, &artifact);
    assert_eq!(
        kura.v2_finality_crypto_verifications
            .load(Ordering::Relaxed),
        1
    );
    for _ in 0..8 {
        assert_eq!(
            kura.v2_finality_artifact(artifact.height)
                .expect("cached finality read"),
            Some(artifact.clone())
        );
    }
    let repeated_receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("idempotent store reuses verified immutable identity");
    assert_v2_commit_receipt_matches_artifact(&repeated_receipt, &artifact);
    assert_eq!(
        repeated_receipt.artifact_hash(),
        initial_receipt.artifact_hash()
    );
    assert_eq!(
        kura.v2_finality_crypto_verifications
            .load(Ordering::Relaxed),
        1,
        "unchanged immutable bytes must receive one cryptographic pass total"
    );

    let path = kura.v2_finality_artifact_path(artifact.height);
    let mut forged = artifact.clone();
    forged.commit_qc.aggregate_signature[0] ^= 0x80;
    replace_v2_finality_record_artifact(&path, forged);
    assert!(matches!(
        kura.v2_finality_artifact(artifact.height),
        Err(Error::V2FinalityCryptography(_))
    ));
    assert_eq!(
        kura.v2_finality_crypto_verifications
            .load(Ordering::Relaxed),
        2,
        "replacement identity must invalidate the successful cache entry"
    );

    replace_v2_finality_record_artifact(&path, artifact.clone());
    assert_eq!(
        kura.v2_finality_artifact(artifact.height)
            .expect("reverify restored sidecar"),
        Some(artifact)
    );
    assert_eq!(
        kura.v2_finality_crypto_verifications
            .load(Ordering::Relaxed),
        3,
        "restored bytes require a fresh successful cryptographic pass"
    );
}

#[test]
fn v2_finality_crypto_cache_uses_fixed_lru_capacity() {
    let kura = Kura::blank_kura_for_testing();
    let store_root = kura.store_root();
    let identity_path = store_root.join("v2-finality-cache-lru-identity");
    std::fs::write(&identity_path, b"stable identity").expect("write cache identity file");
    let stable_metadata = || {
        kura.regular_sidecar_metadata(&identity_path, &store_root)
            .expect("read cache identity metadata")
            .expect("cache identity file exists")
    };
    let artifact_hash = HashOf::<V2FinalityArtifact>::from_untyped_unchecked(Hash::new(
        b"v2 finality cache artifact",
    ));
    let bytes_hash = |height: u64| Hash::new(height.to_le_bytes());
    let capacity =
        u64::try_from(V2_FINALITY_VERIFICATION_CACHE_CAPACITY).expect("cache capacity fits u64");

    for height in 1..=capacity {
        kura.remember_verified_v2_finality(
            height,
            artifact_hash,
            bytes_hash(height),
            stable_metadata(),
        );
    }
    assert_eq!(
        kura.v2_finality_verification_cache.lock().len(),
        V2_FINALITY_VERIFICATION_CACHE_CAPACITY
    );

    let metadata = stable_metadata();
    assert!(
        kura.v2_finality_cache_hit(1, artifact_hash, bytes_hash(1), &metadata),
        "reading the oldest entry must promote it to most-recently used"
    );
    let inserted_height = capacity.saturating_add(1);
    kura.remember_verified_v2_finality(
        inserted_height,
        artifact_hash,
        bytes_hash(inserted_height),
        metadata,
    );

    let cache = kura.v2_finality_verification_cache.lock();
    assert_eq!(cache.len(), V2_FINALITY_VERIFICATION_CACHE_CAPACITY);
    assert!(cache.iter().any(|entry| entry.height == 1));
    assert!(cache.iter().all(|entry| entry.height != 2));
    assert_eq!(
        cache.back().map(|entry| entry.height),
        Some(inserted_height)
    );
}

#[test]
fn startup_finality_inventory_reuses_more_than_the_runtime_lru_without_reverification() {
    let kura = Kura::blank_kura_for_testing();
    let artifact_count = V2_FINALITY_VERIFICATION_CACHE_CAPACITY.saturating_add(1);
    let mut generator = DummyBlocks::new();
    let blocks = (0..artifact_count)
        .map(|_| generator.next())
        .collect::<Vec<_>>();
    for block in &blocks {
        kura.store_block(Arc::clone(block))
            .expect("store startup-inventory fixture block");
    }
    let artifacts = v2_finality_artifacts_for_chain(&blocks);
    for artifact in &artifacts {
        let _receipt = kura
            .store_v2_finality_artifact(artifact)
            .expect("store startup-inventory fixture finality");
    }

    kura.clear_v2_finality_verification_cache_for_test();
    kura.reset_v2_finality_crypto_verifications_for_test();
    let inventory = kura
        .validate_v2_finality_inventory_on_startup()
        .expect("audit complete startup finality inventory");
    assert_eq!(
        kura.v2_finality_crypto_verifications_for_test(),
        artifact_count,
        "the startup audit must verify every artifact exactly once"
    );
    kura.install_v2_startup_finality_verification_inventory(inventory);
    kura.refresh_v2_startup_replay_auxiliary_binding()
        .expect("bind startup auxiliary sidecar identities");
    kura.clear_v2_finality_verification_cache_for_test();

    let session = kura
        .begin_v2_startup_finality_verification()
        .expect("bind startup inventory")
        .expect("startup inventory is reusable");
    kura.reset_startup_replay_historical_payload_reads_for_test();
    for _ in 0..2 {
        for artifact in &artifacts {
            let observed = session
                .finality_projection(artifact.height)
                .expect("audited finality projection exists");
            assert_eq!(observed.height, artifact.height);
            assert_eq!(observed.block_hash, artifact.block_hash);
            assert_eq!(
                observed.commit_qc_hash,
                Hash::new(artifact.commit_qc.encode())
            );
        }
    }
    let _binding = session
        .storage_binding()
        .expect("startup identities remain exact");
    assert_eq!(
        kura.v2_finality_crypto_verifications_for_test(),
        artifact_count,
        "an O(H) startup capability must not thrash the fixed 64-entry runtime LRU"
    );
    assert_eq!(
        kura.startup_replay_historical_payload_reads_for_test(),
        0,
        "projection reuse must not reopen finality or retained-block payloads"
    );
    drop(session);

    kura.finish_v2_startup_finality_verification();
    kura.clear_v2_finality_verification_cache_for_test();
    kura.v2_finality_artifact(1)
        .expect("ordinary finality read after startup cleanup")
        .expect("height-one finality exists");
    assert_eq!(
        kura.v2_finality_crypto_verifications_for_test(),
        artifact_count.saturating_add(1),
        "cleanup must restore fixed-LRU runtime verification behavior"
    );
}

#[test]
fn startup_lane_geometry_refresh_reuses_authenticated_replay_inventory() {
    let kura = Kura::blank_kura_for_testing();
    let block = DummyBlocks::new().next();
    kura.store_block(Arc::clone(&block))
        .expect("store startup lane-geometry fixture block");
    let artifact = v2_finality_artifact_for_block(&block);
    let _receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("store startup lane-geometry fixture finality");
    let checkpoint_hash = Hash::new(b"startup lane-geometry fixture checkpoint");
    kura.store_wsv_checkpoint(1, block.hash(), checkpoint_hash)
        .expect("store startup lane-geometry fixture checkpoint");
    kura.store_commit_manifest(
        CommitManifest::new(1, block.hash(), None, None, checkpoint_hash, None)
            .with_authenticated_v2_commit_authority(&artifact),
    )
    .expect("store startup lane-geometry fixture manifest");

    kura.clear_v2_finality_verification_cache_for_test();
    kura.reset_v2_finality_crypto_verifications_for_test();
    let inventory = kura
        .validate_v2_finality_inventory_on_startup()
        .expect("audit startup lane-geometry fixture");
    assert_eq!(
        kura.v2_finality_crypto_verifications_for_test(),
        1,
        "the authenticated startup audit verifies the fixture exactly once"
    );
    kura.install_v2_startup_finality_verification_inventory(inventory);

    // Production Kura opens only the authenticated primary lane. State startup then
    // publishes the remaining configured lane directories before replay planning.
    let lane_config = two_lane_runtime_config();
    let secondary = lane_config
        .entry(LaneId::from(1))
        .expect("two-lane fixture contains its secondary lane");
    let secondary_artifacts = Kura::lane_artifact_dir(&secondary.blocks_dir(&kura.store_root()));
    std::fs::create_dir_all(&secondary_artifacts)
        .expect("publish secondary lane artifact directory");
    kura.replace_lane_storage_entries_for_test(&lane_config);

    kura.clear_v2_finality_verification_cache_for_test();
    kura.reset_startup_replay_historical_payload_reads_for_test();
    kura.refresh_v2_startup_replay_auxiliary_binding()
        .expect("refresh only post-geometry auxiliary identities");
    let plan = crate::sumeragi::plan_v2_startup_replay(kura.as_ref())
        .expect("reuse authenticated audit after lane geometry setup");

    assert_eq!(plan.durable_height(), 1);
    assert_eq!(
        kura.v2_finality_crypto_verifications_for_test(),
        1,
        "post-geometry planning must not run a second finality audit"
    );
    assert_eq!(
        kura.startup_replay_historical_payload_reads_for_test(),
        0,
        "post-geometry planning must reuse in-memory replay projections"
    );
}

#[test]
fn startup_lane_geometry_refresh_replaces_contracted_lane_auxiliary_identities() {
    let kura = Kura::blank_kura_for_testing();
    let block = DummyBlocks::new().next();
    kura.store_block(Arc::clone(&block))
        .expect("store contraction fixture block");
    let artifact = v2_finality_artifact_for_block(&block);
    let _receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("store contraction fixture finality");
    let checkpoint_hash = Hash::new(b"startup contraction fixture checkpoint");
    kura.store_wsv_checkpoint(1, block.hash(), checkpoint_hash)
        .expect("store contraction fixture checkpoint");
    kura.store_commit_manifest(
        CommitManifest::new(1, block.hash(), None, None, checkpoint_hash, None)
            .with_authenticated_v2_commit_authority(&artifact),
    )
    .expect("store contraction fixture manifest");

    let configured = two_lane_runtime_config();
    let retired = configured
        .entry(LaneId::from(1))
        .expect("two-lane fixture contains its secondary lane");
    let retired_artifacts = Kura::lane_artifact_dir(&retired.blocks_dir(&kura.store_root()));
    let retired_historical =
        Kura::historical_autonomous_recovery_directory_for_entry(retired, &kura.store_root());
    fs::create_dir_all(&retired_artifacts)
        .expect("publish configured secondary lane artifact directory");
    kura.replace_lane_storage_entries_for_test(&configured);

    let verified = kura
        .validate_v2_finality_inventory_on_startup()
        .expect("audit configured startup topology");
    kura.install_v2_startup_finality_verification_inventory(verified);
    kura.refresh_v2_startup_replay_auxiliary_binding()
        .expect("bind configured startup lane identities");

    let checkpoint_dir = kura.wsv_checkpoint_dir();
    let manifest_dir = kura.commit_manifest_dir();
    let (checkpoint_identity, manifest_identity) = {
        let installed = kura.v2_startup_finality_verification_inventory.lock();
        let inventory = installed.as_ref().expect("startup inventory is installed");
        assert!(
            inventory
                .auxiliary_sidecars
                .contains_key(&retired_artifacts)
                && inventory
                    .auxiliary_sidecars
                    .contains_key(&retired_historical),
            "the pre-restore inventory must cover the soon-to-be-retired lane",
        );
        (
            {
                let checkpoint = inventory
                    .auxiliary_sidecars
                    .get(&checkpoint_dir)
                    .expect("checkpoint identity is bound");
                assert_eq!(
                    checkpoint.files.len(),
                    1,
                    "contraction must preserve a populated checkpoint identity",
                );
                checkpoint.clone()
            },
            {
                let manifest = inventory
                    .auxiliary_sidecars
                    .get(&manifest_dir)
                    .expect("manifest identity is bound");
                assert_eq!(
                    manifest.files.len(),
                    1,
                    "contraction must preserve a populated manifest identity",
                );
                manifest.clone()
            },
        )
    };

    let contracted_catalog = LaneCatalog::new(nonzero!(1_u32), vec![ModelLaneConfig::default()])
        .expect("single-lane restored catalog");
    let contracted = RuntimeLaneConfig::from_catalog(&contracted_catalog);
    kura.replace_lane_storage_entries_for_test(&contracted);
    kura.refresh_v2_startup_replay_auxiliary_binding()
        .expect("replace stale auxiliary identities after topology contraction");

    let current_lane_auxiliary = kura
        .capture_v2_startup_replay_lane_auxiliary_sidecars()
        .expect("capture contracted lane identities");
    {
        let installed = kura.v2_startup_finality_verification_inventory.lock();
        let inventory = installed
            .as_ref()
            .expect("startup inventory remains installed");
        assert!(
            !inventory
                .auxiliary_sidecars
                .contains_key(&retired_artifacts)
                && !inventory
                    .auxiliary_sidecars
                    .contains_key(&retired_historical),
            "a contracted lane must not remain authorized merely because its storage still exists",
        );
        assert_eq!(
            inventory.lane_auxiliary_directories,
            current_lane_auxiliary
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>(),
            "the tracked lane-derived subset must exactly match the restored catalog",
        );
        assert!(
            Kura::stable_sidecar_directory_inventory_unchanged(
                &checkpoint_identity,
                inventory
                    .auxiliary_sidecars
                    .get(&checkpoint_dir)
                    .expect("checkpoint identity remains bound"),
            ) && Kura::stable_sidecar_directory_inventory_unchanged(
                &manifest_identity,
                inventory
                    .auxiliary_sidecars
                    .get(&manifest_dir)
                    .expect("manifest identity remains bound"),
            ),
            "lane reconciliation must preserve non-lane replay evidence exactly",
        );
    }

    let session = kura
        .begin_v2_startup_finality_verification()
        .expect("open contracted startup verification session")
        .expect("contracted startup inventory is reusable");
    let binding = session
        .storage_binding()
        .expect("mint exact contracted storage binding");
    drop(session);
    kura.validate_v2_startup_replay_storage_binding(&binding)
        .expect("contracted storage binding matches the restored catalog");
}

#[test]
fn startup_lane_geometry_refresh_replaces_relabelled_lane_auxiliary_identities() {
    let kura = Kura::blank_kura_for_testing();
    let configured = two_lane_runtime_config();
    let previous = configured
        .entry(LaneId::from(1))
        .expect("two-lane fixture contains its secondary lane");
    let previous_blocks = previous.blocks_dir(&kura.store_root());
    let previous_artifacts = Kura::lane_artifact_dir(&previous_blocks);
    let previous_historical =
        Kura::historical_autonomous_recovery_directory_for_entry(previous, &kura.store_root());
    fs::create_dir_all(&previous_historical)
        .expect("publish pre-relabel historical recovery directory");
    let evidence_name = "durable-lane-evidence.norito";
    fs::write(
        previous_artifacts.join(evidence_name),
        b"relabelled lane evidence",
    )
    .expect("publish pre-relabel lane evidence");
    kura.replace_lane_storage_entries_for_test(&configured);

    let verified = kura
        .validate_v2_finality_inventory_on_startup()
        .expect("audit configured startup topology");
    kura.install_v2_startup_finality_verification_inventory(verified);
    kura.refresh_v2_startup_replay_auxiliary_binding()
        .expect("bind pre-relabel startup lane identities");

    let checkpoint_dir = kura.wsv_checkpoint_dir();
    let manifest_dir = kura.commit_manifest_dir();
    let (checkpoint_identity, manifest_identity) = {
        let installed = kura.v2_startup_finality_verification_inventory.lock();
        let inventory = installed.as_ref().expect("startup inventory is installed");
        (
            inventory
                .auxiliary_sidecars
                .get(&checkpoint_dir)
                .expect("checkpoint identity is bound")
                .clone(),
            inventory
                .auxiliary_sidecars
                .get(&manifest_dir)
                .expect("manifest identity is bound")
                .clone(),
        )
    };

    let relabelled_lane = ModelLaneConfig {
        id: LaneId::from(1),
        alias: "gamma".to_owned(),
        ..ModelLaneConfig::default()
    };
    let relabelled_catalog = LaneCatalog::new(
        nonzero!(2_u32),
        vec![ModelLaneConfig::default(), relabelled_lane],
    )
    .expect("relabelled restored catalog");
    let relabelled = RuntimeLaneConfig::from_catalog(&relabelled_catalog);
    let current = relabelled
        .entry(LaneId::from(1))
        .expect("relabelled catalog contains its secondary lane");
    let current_blocks = current.blocks_dir(&kura.store_root());
    let current_artifacts = Kura::lane_artifact_dir(&current_blocks);
    let current_historical =
        Kura::historical_autonomous_recovery_directory_for_entry(current, &kura.store_root());
    fs::rename(&previous_blocks, &current_blocks)
        .expect("move secondary storage to its authenticated relabelled path");
    kura.replace_lane_storage_entries_for_test(&relabelled);
    kura.refresh_v2_startup_replay_auxiliary_binding()
        .expect("replace stale auxiliary identities after lane relabel");

    let current_lane_auxiliary = kura
        .capture_v2_startup_replay_lane_auxiliary_sidecars()
        .expect("capture relabelled lane identities");
    {
        let installed = kura.v2_startup_finality_verification_inventory.lock();
        let inventory = installed
            .as_ref()
            .expect("startup inventory remains installed");
        assert!(
            !inventory
                .auxiliary_sidecars
                .contains_key(&previous_artifacts)
                && !inventory
                    .auxiliary_sidecars
                    .contains_key(&previous_historical),
            "pre-relabel paths must not remain authorized after their topology identity moves",
        );
        assert!(
            inventory
                .auxiliary_sidecars
                .contains_key(&current_artifacts)
                && inventory
                    .auxiliary_sidecars
                    .contains_key(&current_historical),
            "the relabelled lane paths must be authorized",
        );
        assert!(
            inventory
                .auxiliary_sidecars
                .get(&current_artifacts)
                .is_some_and(|lane| lane
                    .files
                    .contains_key(&current_artifacts.join(evidence_name))),
            "durable lane evidence moved by the relabel must remain identity-bound",
        );
        assert_eq!(
            inventory.lane_auxiliary_directories,
            current_lane_auxiliary
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>(),
            "the tracked lane-derived subset must exactly match the relabelled catalog",
        );
        assert!(
            Kura::stable_sidecar_directory_inventory_unchanged(
                &checkpoint_identity,
                inventory
                    .auxiliary_sidecars
                    .get(&checkpoint_dir)
                    .expect("checkpoint identity remains bound"),
            ) && Kura::stable_sidecar_directory_inventory_unchanged(
                &manifest_identity,
                inventory
                    .auxiliary_sidecars
                    .get(&manifest_dir)
                    .expect("manifest identity remains bound"),
            ),
            "lane relabel reconciliation must preserve non-lane replay evidence exactly",
        );
    }

    let session = kura
        .begin_v2_startup_finality_verification()
        .expect("open relabelled startup verification session")
        .expect("relabelled startup inventory is reusable");
    let binding = session
        .storage_binding()
        .expect("mint exact relabelled storage binding");
    drop(session);
    kura.validate_v2_startup_replay_storage_binding(&binding)
        .expect("relabelled storage binding matches the restored catalog");
}

#[test]
fn startup_replay_binding_covers_the_recognized_historical_recovery_namespace() {
    let kura = Kura::blank_kura_for_testing();
    let lane = kura
        .lane_storage_entries
        .lock()
        .values()
        .next()
        .cloned()
        .expect("blank Kura has a primary lane");
    let historical =
        Kura::historical_autonomous_recovery_directory_for_entry(&lane, &kura.store_root());
    fs::create_dir(&historical).expect("create recognized historical recovery namespace");

    let empty = kura
        .capture_v2_startup_replay_lane_auxiliary_sidecars()
        .expect("capture empty recognized historical namespace");
    let empty_historical = empty
        .get(&historical)
        .expect("recognized historical namespace has its own inventory");
    assert!(
        empty_historical.directory.metadata.is_some() && empty_historical.files.is_empty(),
        "an empty direct namespace must be accepted and identity-bound",
    );

    let record_path = historical.join(format!(
        "{}.norito",
        "0".repeat(Hash::LENGTH.saturating_mul(2))
    ));
    let record_bytes = b"startup replay historical recovery identity";
    fs::write(&record_path, record_bytes).expect("write bounded historical recovery fixture");
    let verified = kura
        .validate_v2_finality_inventory_on_startup()
        .expect("audit empty canonical chain");
    kura.install_v2_startup_finality_verification_inventory(verified);
    kura.refresh_v2_startup_replay_auxiliary_binding()
        .expect("bind recognized historical recovery namespace");
    {
        let installed = kura.v2_startup_finality_verification_inventory.lock();
        let historical_inventory = installed
            .as_ref()
            .and_then(|inventory| inventory.auxiliary_sidecars.get(&historical))
            .expect("installed startup inventory covers historical recovery namespace");
        assert!(
            historical_inventory.files.contains_key(&record_path),
            "the immutable recovery record identity must be bound",
        );
    }

    let session = kura
        .begin_v2_startup_finality_verification()
        .expect("open startup verification session")
        .expect("installed startup inventory is reusable");
    let binding = session
        .storage_binding()
        .expect("mint startup storage binding");
    drop(session);

    let displaced = kura
        .store_root()
        .join("historical-recovery-displaced-for-test");
    fs::rename(&historical, &displaced).expect("displace bound historical namespace");
    fs::create_dir(&historical).expect("replace historical namespace");
    fs::write(&record_path, record_bytes).expect("replace recovery bytes at the same path");
    kura.validate_v2_startup_replay_storage_binding(&binding)
        .expect_err("namespace inode replacement after binding must fail");
}

#[test]
fn startup_replay_auxiliary_capture_rejects_configured_historical_byte_overflow() {
    let temp = TempDir::new().expect("temporary configured startup Kura root");
    let config = kura_config_for_dir(&temp, BLOCKS_IN_MEMORY);
    let initial_limits = SumeragiV2RuntimeLimits::default();
    let (kura, _) = open_configured_kura_with_pending_limits(&config, &initial_limits)
        .expect("open default-bound startup Kura");
    publish_configured_catalog_baseline(&kura, &LaneCatalog::default());
    let genesis = DummyBlocks::new().next();
    kura.store_block(genesis)
        .expect("store configured-overflow canonical genesis");
    let _ = persist_v2_finality_chain_through(&kura, nonzero!(1_usize));
    let lane = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("configured Kura has its primary lane");
    let historical =
        Kura::historical_autonomous_recovery_directory_for_entry(&lane, &kura.store_root());
    fs::create_dir_all(&historical)
        .expect("publish configured-limit historical recovery namespace");

    let lower_limit = V2_PENDING_CONTROL_SIDECAR_BYTES_MIN;
    let first_len = lower_limit / 2;
    let second_len = lower_limit.saturating_sub(first_len).saturating_add(1);
    for (stem, byte, length) in [("a", b'a', first_len), ("b", b'b', second_len)] {
        let path = historical.join(format!(
            "{}.norito",
            stem.repeat(Hash::LENGTH.saturating_mul(2))
        ));
        fs::write(path, vec![byte; length])
            .expect("write individually bounded historical recovery record");
    }
    kura.capture_v2_startup_replay_lane_auxiliary_sidecars()
        .expect("the default aggregate bound accepts the two-record fixture");
    drop(kura);

    let mut tightened_limits = initial_limits;
    tightened_limits.pending_control_sidecar_bytes =
        NonZeroUsize::new(lower_limit).expect("configured lower byte limit is non-zero");
    let error = open_configured_kura_with_pending_limits(&config, &tightened_limits)
        .expect_err("startup auxiliary capture must enforce the configured aggregate bound");
    assert!(
        error
            .to_string()
            .contains("historical autonomous recovery bytes exceed their hard bound"),
        "configured historical recovery overflow must fail closed: {error}",
    );
}

#[test]
fn startup_replay_binding_rejects_unknown_nested_lane_artifact_directories() {
    let kura = Kura::blank_kura_for_testing();
    let lane = kura
        .lane_storage_entries
        .lock()
        .values()
        .next()
        .cloned()
        .expect("blank Kura has a primary lane");
    let lane_artifacts = Kura::lane_artifact_dir(&lane.blocks_dir(&kura.store_root()));
    let unexpected = lane_artifacts.join("unexpected_nested_namespace");
    fs::create_dir(&unexpected).expect("create unexpected nested lane-artifact directory");
    kura.capture_v2_startup_replay_lane_auxiliary_sidecars()
        .expect_err("an unknown nested lane-artifact directory must fail closed");
}

#[test]
fn startup_finality_parallel_batch_reports_the_lowest_corrupt_height() {
    let kura = Kura::blank_kura_for_testing();
    let artifact_count = V2_FINALITY_STARTUP_VERIFICATION_BATCH_SIZE.saturating_add(1);
    let mut generator = DummyBlocks::new();
    let blocks = (0..artifact_count)
        .map(|_| generator.next())
        .collect::<Vec<_>>();
    for block in &blocks {
        kura.store_block(Arc::clone(block))
            .expect("store deterministic-batch fixture block");
    }
    let artifacts = v2_finality_artifacts_for_chain(&blocks);
    for artifact in &artifacts {
        let _receipt = kura
            .store_v2_finality_artifact(artifact)
            .expect("store deterministic-batch fixture finality");
    }
    let lower_path = kura.v2_finality_artifact_path(2);
    let higher_path = kura.v2_finality_artifact_path(5);
    for path in [&higher_path, &lower_path] {
        let bytes = std::fs::read(path).expect("read finality record to corrupt height");
        let mut cursor = bytes.as_slice();
        let mut record =
            KuraV2FinalityRecord::decode_all(&mut cursor).expect("decode finality record");
        record.artifact.height = record.artifact.height.saturating_add(100);
        std::fs::write(path, record.encode()).expect("write canonical corrupt record");
    }

    assert!(matches!(
        kura.validate_v2_finality_inventory_on_startup(),
        Err(Error::IO(error, path))
            if error.kind() == ErrorKind::InvalidData && path == lower_path
    ));
}

#[test]
fn startup_finality_mismatch_retains_the_original_binding_until_refresh() {
    let kura = Kura::blank_kura_for_testing();
    let block = DummyBlocks::new().next();
    kura.store_block(Arc::clone(&block))
        .expect("store startup-mismatch fixture block");
    let artifact = v2_finality_artifact_for_block(&block);
    let _receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("store startup-mismatch fixture finality");
    kura.clear_v2_finality_verification_cache_for_test();
    let inventory = kura
        .validate_v2_finality_inventory_on_startup()
        .expect("audit startup-mismatch fixture");
    kura.install_v2_startup_finality_verification_inventory(inventory);
    let path = kura.v2_finality_artifact_path(artifact.height);
    let bytes = std::fs::read(&path).expect("read validated finality bytes");
    std::fs::write(&path, bytes).expect("replace finality bytes in place");

    kura.clear_v2_finality_verification_cache_for_test();
    assert_eq!(
        kura.v2_finality_artifact(artifact.height)
            .expect("fully reverify equal in-place bytes"),
        Some(artifact)
    );
    assert_eq!(
        kura.v2_startup_finality_inventory_len_for_test(),
        1,
        "an identity mismatch must retain the original evidence for later binding checks"
    );
    assert!(
        kura.begin_v2_startup_finality_verification()
            .expect("reject stale startup identity")
            .is_none(),
        "begin must force a full refresh after any validated identity changes"
    );
}

#[test]
fn v2_finality_cache_invalidates_same_inode_parent_path_relocation() {
    let kura = Kura::blank_kura_for_testing();
    let block = DummyBlocks::new().next();
    kura.store_block(Arc::clone(&block))
        .expect("store canonical block");
    let artifact = v2_finality_artifact_for_block(&block);
    let _commit_receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("persist and verify finality artifact");
    assert_eq!(
        kura.v2_finality_crypto_verifications
            .load(Ordering::Relaxed),
        1
    );

    let directory = kura.v2_finality_artifact_dir();
    let path = kura.v2_finality_artifact_path(artifact.height);
    let before = kura
        .regular_sidecar_metadata(&path, &directory)
        .expect("read original sidecar metadata")
        .expect("original sidecar exists");
    let relocated_directory = directory.with_file_name("v2_finality_relocated");
    std::fs::rename(&directory, &relocated_directory)
        .expect("relocate the complete finality directory");
    let relocated_path = relocated_directory.join(
        path.file_name()
            .expect("canonical finality path has a file name"),
    );
    let relocated = kura
        .regular_sidecar_metadata(&relocated_path, &relocated_directory)
        .expect("read relocated sidecar metadata")
        .expect("relocated sidecar exists");
    assert!(Kura::sidecar_metadata_same_object(
        &before.file,
        &relocated.file
    ));
    assert!(Kura::sidecar_metadata_same_object(
        &before.directory,
        &relocated.directory
    ));
    assert_ne!(before.canonical_path, relocated.canonical_path);

    let (record, read_identity) = kura
        .decode_v2_finality_record_at(&relocated_path, &relocated_directory)
        .expect("decode relocated finality sidecar")
        .expect("relocated finality sidecar exists");
    kura.verify_v2_finality_artifact_at(
        &relocated_path,
        &relocated_directory,
        &record.artifact,
        &read_identity,
    )
    .expect("reverify relocated finality sidecar");
    assert_eq!(
        kura.v2_finality_crypto_verifications
            .load(Ordering::Relaxed),
        2,
        "a persistent path relocation must invalidate the old cache identity"
    );

    let (record, read_identity) = kura
        .decode_v2_finality_record_at(&relocated_path, &relocated_directory)
        .expect("decode stable relocated finality sidecar")
        .expect("stable relocated finality sidecar exists");
    kura.verify_v2_finality_artifact_at(
        &relocated_path,
        &relocated_directory,
        &record.artifact,
        &read_identity,
    )
    .expect("reuse relocated finality cache identity");
    assert_eq!(
        kura.v2_finality_crypto_verifications
            .load(Ordering::Relaxed),
        2,
        "repeated reads at the same relocated path must reuse one successful pass"
    );
}

#[test]
fn bridge_and_sccp_proof_builders_reuse_exact_finality_sidecar_verification() {
    let kura = Kura::blank_kura_for_testing();
    let block = DummyBlocks::new().next();
    kura.store_block(Arc::clone(&block))
        .expect("store canonical block");
    let artifact = v2_finality_artifact_for_block(&block);
    let _receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("persist and verify finality artifact");
    assert_eq!(
        kura.v2_finality_crypto_verifications
            .load(Ordering::Relaxed),
        1
    );

    let state = State::new_with_chain_and_network_id_for_testing(
        World::default(),
        Arc::clone(&kura),
        LiveQueryStore::start_test(),
        ChainId::from("kura-v2-finality-test"),
        artifact.height_context.network_id,
    );
    for _ in 0..4 {
        let proof = crate::bridge::build_finality_proof(&state, artifact.height)
            .expect("build bridge proof from verified Kura finality");
        assert_eq!(proof.finality_artifact, artifact);
        let bundle = crate::bridge::build_finality_bundle(&state, artifact.height)
            .expect("build bridge bundle from verified Kura finality");
        assert_eq!(bundle.finality_proof.finality_artifact, artifact);
        assert!(
            crate::bridge::validated_sccp_finalized_messages_at_height(&state, artifact.height,)
                .expect("build SCCP finality projection from verified Kura finality")
                .is_none(),
            "a block without an SCCP commitment has no finalized SCCP projection"
        );
    }
    assert_eq!(
        kura.v2_finality_crypto_verifications
            .load(Ordering::Relaxed),
        1,
        "bridge and SCCP proof construction must not repeat Kura's BLS verification"
    );
}

#[test]
fn v2_finality_persistence_never_waits_for_the_block_store_writer_lock() {
    let kura = Kura::blank_kura_for_testing();
    let block = DummyBlocks::new().next();
    kura.store_block(Arc::clone(&block))
        .expect("store canonical block");
    let artifact = v2_finality_artifact_for_block(&block);
    let writer_guard = kura.block_store_write_lock.lock();
    let worker_kura = Arc::clone(&kura);
    let (result_tx, result_rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        result_tx
            .send(
                worker_kura
                    .store_v2_finality_artifact(&artifact)
                    .map(|_| ()),
            )
            .expect("report finality persistence result");
    });

    let result = result_rx.recv_timeout(Duration::from_secs(5));
    drop(writer_guard);
    if matches!(&result, Err(RecvTimeoutError::Timeout)) {
        let _ = result_rx.recv_timeout(Duration::from_secs(5));
        worker.join().expect("join finality persistence worker");
        panic!(
            "v2 finality persistence waited for block_store_write_lock and can deadlock with a block-data owner"
        );
    }
    result
        .expect("finality worker did not disconnect")
        .expect("persist finality while writer lock is independently held");
    worker.join().expect("join finality persistence worker");
}

#[test]
fn v2_finality_artifact_is_immutable_after_first_durable_write() {
    let kura = Kura::blank_kura_for_testing();
    let block = DummyBlocks::new().next();
    kura.store_block(Arc::clone(&block))
        .expect("store canonical block");
    let artifact = v2_finality_artifact_for_block(&block);
    let first_receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("persist finality artifact");
    let repeated_receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("identical artifact is idempotent");
    assert_eq!(
        repeated_receipt.artifact_hash(),
        first_receipt.artifact_hash()
    );

    let conflicting = v2_finality_artifact_for_block_with_execution(
        &block,
        ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"conflicting Kura finality parent state"),
            Hash::new(b"conflicting Kura finality post state"),
            Hash::new(b"conflicting Kura finality ordinary writes"),
            1,
            Hash::new(b"conflicting Kura finality executed block wire"),
        ),
    );
    conflicting
        .verify()
        .expect("conflicting fixture is independently cryptographically valid");
    assert_ne!(conflicting, artifact);
    assert!(matches!(
        kura.store_v2_finality_artifact(&conflicting),
        Err(Error::ConflictingV2FinalityArtifact { height: 1 })
    ));
    assert_eq!(
        kura.v2_finality_artifact(1)
            .expect("read immutable original"),
        Some(artifact)
    );
}

#[test]
fn v2_finality_store_and_read_fail_closed_while_prune_poisoned() {
    let kura = Kura::blank_kura_for_testing();
    let block = DummyBlocks::new().next();
    kura.store_block(Arc::clone(&block))
        .expect("store canonical block");
    let artifact = v2_finality_artifact_for_block(&block);
    let original_receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("persist finality artifact");

    kura.prune_recovery_required.store(true, Ordering::Release);
    assert!(matches!(
        kura.store_v2_finality_artifact(&artifact),
        Err(Error::PruneRecoveryRequired)
    ));
    assert!(matches!(
        kura.v2_finality_artifact(artifact.height),
        Err(Error::PruneRecoveryRequired)
    ));
    assert!(matches!(
        kura.v2_finality_artifact_with_receipt(artifact.height),
        Err(Error::PruneRecoveryRequired)
    ));

    kura.prune_recovery_required.store(false, Ordering::Release);
    assert_eq!(
        kura.v2_finality_artifact(artifact.height)
            .expect("read original after clearing test poison"),
        Some(artifact.clone()),
        "poisoned operations must not mutate the durable artifact"
    );
    let (recovered, recovered_receipt) = kura
        .v2_finality_artifact_with_receipt(artifact.height)
        .expect("recover original after clearing test poison")
        .expect("artifact remains present");
    assert_eq!(recovered, artifact);
    assert_eq!(
        recovered_receipt.artifact_hash(),
        original_receipt.artifact_hash()
    );
}

#[test]
fn v2_finality_record_rejects_a_substituted_retained_header() {
    let kura = Kura::blank_kura_for_testing();
    let block = DummyBlocks::new().next();
    kura.store_block(Arc::clone(&block))
        .expect("store canonical block");
    let artifact = v2_finality_artifact_for_block(&block);
    let _receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("persist finality artifact");
    let path = kura.v2_finality_artifact_path(1);
    let bytes = std::fs::read(&path).expect("read Kura finality record");
    let mut cursor = bytes.as_slice();
    let mut record =
        KuraV2FinalityRecord::decode_all(&mut cursor).expect("decode Kura finality record");
    let substitute: SignedBlock =
        ValidBlock::new_dummy_and_modify_header(checked_keypair().private_key(), |header| {
            header.set_height(nonzero!(1_u64));
            header.set_prev_block_hash(None);
            header.set_view_change_index(header.view_change_index().saturating_add(1));
        })
        .into();
    record.block_header = substitute.header();
    std::fs::write(&path, record.encode()).expect("substitute retained header");

    assert!(matches!(
        kura.v2_finality_artifact(1),
        Err(Error::BlockHeightConflict {
            height: 1,
            expected,
            actual,
        }) if expected == block.hash() && actual == substitute.hash()
    ));
}

#[test]
fn canonical_finality_path_rejects_raw_artifact_shape_on_read_and_restart() {
    let temp_dir = TempDir::new().expect("create Kura root");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    {
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
        let block = store_dummy_block_arcs(&kura, 1)
            .pop()
            .expect("canonical block");
        let artifact = v2_finality_artifact_for_block(block.as_ref());
        let _ = kura
            .store_v2_finality_artifact(&artifact)
            .expect("persist canonical private finality envelope");
        fs::write(kura.v2_finality_artifact_path(1), artifact.encode())
            .expect("replace private envelope with raw public artifact");

        assert!(
            kura.v2_finality_artifact(1).is_err(),
            "the canonical private path must not accept the raw public artifact shape"
        );
    }

    assert!(
        Kura::new(&config, &RuntimeLaneConfig::default()).is_err(),
        "startup must reject a raw artifact at the canonical private-record path"
    );
}

#[test]
fn canonical_finality_path_rejects_wrong_private_record_version_on_read_and_restart() {
    let temp_dir = TempDir::new().expect("create Kura root");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    {
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
        let block = store_dummy_block_arcs(&kura, 1)
            .pop()
            .expect("canonical block");
        let artifact = v2_finality_artifact_for_block(block.as_ref());
        let _ = kura
            .store_v2_finality_artifact(&artifact)
            .expect("persist canonical private finality envelope");
        let path = kura.v2_finality_artifact_path(1);
        let bytes = fs::read(&path).expect("read canonical private finality envelope");
        let mut input = bytes.as_slice();
        let mut record = KuraV2FinalityRecord::decode_all(&mut input)
            .expect("decode canonical private finality envelope");
        record.format_version = KURA_V2_FINALITY_RECORD_VERSION.saturating_add(1);
        fs::write(&path, record.encode()).expect("write unsupported private-record version");

        assert!(matches!(
            kura.v2_finality_artifact(1),
            Err(Error::IO(error, _)) if error.kind() == ErrorKind::InvalidData
        ));
    }

    assert!(matches!(
        Kura::new(&config, &RuntimeLaneConfig::default()),
        Err(Error::IO(error, _)) if error.kind() == ErrorKind::InvalidData
    ));
}

#[test]
fn canonical_finality_path_rejects_legacy_v2_private_record_on_read_and_restart() {
    let temp_dir = TempDir::new().expect("create Kura root");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    {
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
        let block = store_dummy_block_arcs(&kura, 1)
            .pop()
            .expect("canonical block");
        let artifact = v2_finality_artifact_for_block(block.as_ref());
        let _ = kura
            .store_v2_finality_artifact(&artifact)
            .expect("persist canonical private finality envelope");
        let path = kura.v2_finality_artifact_path(1);
        let bytes = fs::read(&path).expect("read canonical private finality envelope");
        let mut input = bytes.as_slice();
        let mut record = KuraV2FinalityRecord::decode_all(&mut input)
            .expect("decode canonical private finality envelope");
        record.format_version = 2;
        fs::write(&path, record.encode()).expect("write legacy v2 private record");

        assert!(matches!(
            kura.v2_finality_artifact(1),
            Err(Error::IO(error, _)) if error.kind() == ErrorKind::InvalidData
        ));
    }

    assert!(matches!(
        Kura::new(&config, &RuntimeLaneConfig::default()),
        Err(Error::IO(error, _)) if error.kind() == ErrorKind::InvalidData
    ));
}
