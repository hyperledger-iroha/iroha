use std::{
    borrow::Cow,
    fs::File,
    num::{NonZeroU64, NonZeroUsize},
    path::Path,
    sync::{Arc, Barrier},
};

use super::*;
use crate::{
    block::BlockBuilder,
    query::store::LiveQueryStore,
    state::{
        AssetDefinitionAliasBindingRecord, ContractAliasBindingRecord, derive_validator_key_id,
    },
    sumeragi::consensus::{PERMISSIONED_TAG, Phase, Vote, default_chain_order_hash, vote_preimage},
    tx::AcceptedTransaction,
};
use iroha_config::{
    base::WithOrigin,
    kura::{FsyncMode, InitMode},
    parameters::{
        actual::{Kura as KuraConfig, LaneConfig},
        defaults::kura::{
            BLOCK_SYNC_ROSTER_RETENTION, FSYNC_INTERVAL, MAX_DISK_USAGE_BYTES,
            MERGE_LEDGER_CACHE_CAPACITY, REPLICA_ADVERT_POLICY, ROSTER_SIDECAR_RETENTION,
        },
    },
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature, bls_normal_pop_prove};
use iroha_data_model::{
    ChainId, Level, Registrable,
    account::{
        AccountAlias, AccountAliasDomain, AccountDetails, AccountId, AccountRekeyRecord,
        AccountValue,
    },
    asset::{AssetDefinition, AssetDefinitionAlias, AssetDefinitionId},
    block::{BlockHeader, SignedBlock},
    consensus::{ConsensusKeyStatus, Qc, QcAggregate, VALIDATOR_SET_HASH_VERSION_V1},
    domain::DomainId,
    isi::{Log, space_directory::PublishSpaceDirectoryManifest},
    metadata::Metadata,
    nexus::{
        AssetPermissionManifest, DataSpaceId, LaneCatalog, LaneConfig as ModelLaneConfig,
        ManifestVersion, UniversalAccountId,
    },
    peer::PeerId,
    smart_contract::{ContractAddress, ContractAlias},
    transaction::TransactionBuilder,
};
use iroha_primitives::json::Json;
use nonzero_ext::nonzero;
use tempfile::tempdir;

const TEST_CHUNK_SIZE: NonZeroUsize = nonzero!(1024_usize);

fn dummy_block_hash(marker: u8) -> HashOf<BlockHeader> {
    HashOf::from_untyped_unchecked(Hash::prehashed([marker; 32]))
}

const TEST_CHAIN_ID: &str = "test-chain";
const SMALL_ORDER_ED25519_R: [u8; 32] = [
    1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
const NONCANONICAL_ED25519_R: [u8; 32] = [
    0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f,
];

fn snapshot_test_network_id() -> NetworkId {
    let mut genesis_hash = [0_u8; Hash::LENGTH];
    genesis_hash[Hash::LENGTH - 1] = 1;
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::prehashed(genesis_hash),
    ))
}

fn checked_seeded_keypair(seed: u8, algorithm: Algorithm) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], algorithm)
        .expect("test snapshot seeded keypair should be valid")
}

fn checked_random_snapshot_keypair() -> KeyPair {
    KeyPair::try_random().expect("snapshot fixture key generation should succeed")
}

fn checked_random_snapshot_bls_keypair() -> KeyPair {
    KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
        .expect("snapshot BLS fixture key generation should succeed")
}

fn current_generation_name(store_dir: &Path) -> String {
    let pointer_path = store_dir.join(SNAPSHOT_CURRENT_FILE_NAME);
    let pointer = std::fs::read(&pointer_path).expect("read canonical snapshot pointer");
    parse_snapshot_current_pointer(&pointer, &pointer_path)
        .expect("canonical snapshot pointer must name one generation")
}

fn current_generation_dir(store_dir: &Path) -> PathBuf {
    store_dir
        .join(SNAPSHOT_GENERATIONS_DIR_NAME)
        .join(current_generation_name(store_dir))
}

fn current_generation_artifact(store_dir: &Path, name: &str) -> PathBuf {
    current_generation_dir(store_dir).join(name)
}

fn assert_canonical_snapshot_generation(store_dir: &Path) {
    let mut root_entries = std::fs::read_dir(store_dir)
        .expect("read snapshot root")
        .map(|entry| {
            entry
                .expect("read snapshot root entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    root_entries.sort();
    assert_eq!(
        root_entries,
        vec![
            SNAPSHOT_CURRENT_FILE_NAME.to_owned(),
            SNAPSHOT_GENERATIONS_DIR_NAME.to_owned(),
        ],
        "first-release snapshots expose only the atomic pointer and immutable generations"
    );

    let generation_dir = current_generation_dir(store_dir);
    let generation_name = current_generation_name(store_dir);
    let payload = std::fs::read(generation_dir.join(SNAPSHOT_FILE_NAME))
        .expect("read selected snapshot payload");
    assert_eq!(generation_name, hex::encode(Sha256::digest(&payload)));
    assert_eq!(
        std::fs::read(generation_dir.join(SNAPSHOT_DIGEST_FILE_NAME))
            .expect("read selected snapshot digest"),
        format!("{generation_name}\n").as_bytes()
    );
    let mut artifact_names = std::fs::read_dir(&generation_dir)
        .expect("read selected generation")
        .map(|entry| {
            entry
                .expect("read generation entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    artifact_names.sort();
    let mut expected = vec![
        SNAPSHOT_FILE_NAME.to_owned(),
        SNAPSHOT_DIGEST_FILE_NAME.to_owned(),
        SNAPSHOT_SIGNATURE_FILE_NAME.to_owned(),
        SNAPSHOT_MERKLE_FILE_NAME.to_owned(),
    ];
    expected.sort();
    assert_eq!(artifact_names, expected);
}

fn signed_complete_wire_finality_for_snapshot_blocks(
    network_id: &NetworkId,
    blocks: &[Arc<SignedBlock>],
) -> Vec<iroha_data_model::block::consensus_v2::finality::V2FinalityArtifact> {
    use iroha_data_model::block::consensus_v2::{
        BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
        ExecutionCommitment, GlobalPhase, HeightContext, PROTOCOL_VERSION, PayloadEncoding,
        QuorumCertificate, ValidatorPower, finality::V2FinalityArtifact,
    };

    let mut keypairs = (0_u8..4)
        .map(|index| checked_seeded_keypair(0xB0_u8.saturating_add(index), Algorithm::BlsNormal))
        .collect::<Vec<_>>();
    keypairs.sort_by(|left, right| {
        PeerId::new(left.public_key().clone()).cmp(&PeerId::new(right.public_key().clone()))
    });
    let roster = keypairs
        .iter()
        .map(|keypair| ValidatorPower {
            validator: PeerId::new(keypair.public_key().clone()),
            power: 1,
        })
        .collect::<Vec<_>>();
    let validator_set_pops = keypairs
        .iter()
        .map(|keypair| {
            bls_normal_pop_prove(keypair.private_key())
                .expect("derive snapshot-eviction validator PoP")
        })
        .collect::<Vec<_>>();
    let execution_commitment_template = ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new(b"snapshot eviction parent state"),
        Hash::new(b"snapshot eviction post state"),
        Hash::new(b"snapshot eviction ordinary writes"),
        1,
        Hash::new(b"snapshot eviction executed block wire placeholder"),
    );
    let mut parent: Option<V2FinalityArtifact> = None;
    let mut artifacts = Vec::with_capacity(blocks.len());
    for block in blocks {
        let height = block.header().height().get();
        let context = HeightContext {
            network_id: network_id.clone(),
            protocol_version: PROTOCOL_VERSION,
            height,
            epoch: 0,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: ConsensusMode::Permissioned,
            parent_commit_qc: parent.as_ref().map(|artifact| artifact.commit_qc.clone()),
            snapshot_bootstrap: None,
            quorum: DualQuorum::from_roster(&roster).expect("snapshot-eviction fixture quorum"),
            roster: roster.clone(),
            nexus_amx_context_hash: Hash::new(b"snapshot eviction nexus context"),
            execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
            da_layout: DataAvailabilityLayout {
                encoding: PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 512 * 1024,
                max_chunk_count: 1024,
            },
            leader_seed: [0x42; 32],
        };
        let subject = BlockSubject {
            parent_block_hash: block.header().prev_block_hash(),
            block_hash: block.hash(),
            payload_hash: block
                .canonical_proposal_wire_hash()
                .expect("canonical snapshot proposal wire"),
        };
        let mut execution_commitment = execution_commitment_template;
        execution_commitment.executed_block_wire_len = u64::try_from(
            block
                .encode_wire()
                .expect("canonical snapshot executed block wire")
                .len(),
        )
        .expect("snapshot executed block wire length fits u64");
        execution_commitment.executed_block_wire_hash = block
            .executed_block_wire_hash()
            .expect("canonical snapshot executed block wire");
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
            .expect("snapshot-eviction signer preimage");
        let signatures = commit_qc
            .signers
            .iter()
            .map(|index| {
                Signature::try_new(
                    keypairs[usize::try_from(*index).expect("fixture signer index")].private_key(),
                    &preimage,
                )
                .expect("sign snapshot-eviction finality vote")
                .payload()
                .to_vec()
            })
            .collect::<Vec<_>>();
        let signature_refs = signatures.iter().map(Vec::as_slice).collect::<Vec<_>>();
        commit_qc.aggregate_signature =
            iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
                .expect("aggregate snapshot-eviction finality votes");
        let artifact =
            V2FinalityArtifact::new(context, subject, commit_qc, validator_set_pops.clone());
        artifact
            .verify()
            .expect("snapshot-eviction finality fixture verifies");
        parent = Some(artifact.clone());
        artifacts.push(artifact);
    }
    artifacts
}

fn snapshot_gate_fixture() -> (
    State,
    Arc<Kura>,
    Arc<SignedBlock>,
    iroha_data_model::block::consensus_v2::finality::V2FinalityArtifact,
) {
    let kura = Kura::blank_kura_for_testing();
    let mut state = state_factory_with_kura(Arc::clone(&kura));
    let block = signed_block_with_transaction(accepted_log_transaction("snapshot gate"));
    store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block));
    let artifact = signed_complete_wire_finality_for_snapshot_blocks(
        &state.network_id,
        std::slice::from_ref(&block),
    )
    .into_iter()
    .next()
    .expect("one snapshot finality artifact");
    (state, kura, block, artifact)
}

fn store_snapshot_checkpoint_and_manifest(
    state: &State,
    kura: &Kura,
    block: &SignedBlock,
    state_hash: Hash,
    authority: &iroha_data_model::block::consensus_v2::finality::V2FinalityArtifact,
) {
    let height = block.header().height().get();
    kura.store_wsv_checkpoint(height, block.hash(), state_hash)
        .expect("store snapshot gate WSV checkpoint");
    let manifest =
        crate::kura::CommitManifest::new(height, block.hash(), None, None, state_hash, None)
            .with_authenticated_v2_commit_authority(authority);
    kura.store_commit_manifest(manifest)
        .expect("store checkpoint-bound snapshot gate manifest");
    assert_eq!(state.committed_height(), usize::try_from(height).unwrap());
}

fn store_complete_snapshot_commit_evidence(
    state: &State,
    kura: &Kura,
    block: &SignedBlock,
    authority: &iroha_data_model::block::consensus_v2::finality::V2FinalityArtifact,
) {
    let state_hash = canonical_state_snapshot_hash(state);
    store_snapshot_checkpoint_and_manifest(state, kura, block, state_hash, authority);
    let _ = kura
        .store_v2_finality_artifact(authority)
        .expect("persist complete-wire snapshot finality");
}

fn store_complete_snapshot_commit_evidence_for_blocks(
    state: &State,
    kura: &Kura,
    blocks: &[Arc<SignedBlock>],
) {
    let artifacts = signed_complete_wire_finality_for_snapshot_blocks(&state.network_id, blocks);
    let (terminal_artifact, historical_artifacts) = artifacts
        .split_last()
        .expect("snapshot commit evidence requires a terminal block");
    for artifact in historical_artifacts {
        let _ = kura
            .store_v2_finality_artifact(artifact)
            .expect("persist historical complete-wire snapshot finality");
    }
    let terminal_block = blocks
        .last()
        .expect("snapshot commit evidence requires a terminal block");
    store_complete_snapshot_commit_evidence(state, kura, terminal_block, terminal_artifact);
}

fn assert_snapshot_bundle_absent(store_dir: &Path) {
    assert!(
        !store_dir.join(SNAPSHOT_CURRENT_FILE_NAME).exists(),
        "rejected snapshot must not publish a current pointer"
    );
    let generations = store_dir.join(SNAPSHOT_GENERATIONS_DIR_NAME);
    assert!(
        !generations.exists()
            || std::fs::read_dir(&generations)
                .expect("read unpublished generations directory")
                .next()
                .is_none(),
        "rejected snapshot must not leave a selectable immutable generation"
    );
}

#[tokio::test]
async fn bounded_snapshot_reader_rejects_oversized_regular_file() {
    let root = tempdir().expect("tempdir");
    let path = root.path().join("oversized");
    std::fs::write(&path, [0_u8; 9]).expect("write oversized fixture");

    let error = read_bounded_stable_regular_file(&path, 8)
        .expect_err("oversized snapshot artifact must fail before allocation");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
}

#[tokio::test]
async fn bounded_snapshot_reader_rechecks_the_opened_file_length() {
    let error = bounded_snapshot_read_capacity(9, 8)
        .expect_err("growth between path metadata and the opened descriptor must fail");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
}

#[cfg(unix)]
#[tokio::test]
async fn authenticated_bound_payload_rejects_in_place_change_before_decode() {
    let root = tempdir().expect("tempdir");
    let path = root.path().join("snapshot.data");
    std::fs::write(&path, b"canonical").expect("write canonical payload");
    let binding = bind_snapshot_file_handle(&path, 9)
        .expect("bind canonical payload")
        .expect("payload exists");

    std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&path)
        .and_then(|mut file| file.write_all(b"malicious"))
        .expect("replace bytes in the already-open inode");

    assert!(matches!(
        read_bound_snapshot_payload(&binding),
        Err(TryReadError::SnapshotBindingChanged(changed)) if changed == path
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn snapshot_bindings_reject_untrusted_unix_owner_or_mode() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let root = tempdir().expect("tempdir");
    let directory = root.path().join("snapshot");
    std::fs::create_dir(&directory).expect("create snapshot directory");
    let metadata = std::fs::symlink_metadata(&directory).expect("snapshot directory metadata");
    let effective_uid = rustix::process::geteuid().as_raw();
    assert!(snapshot_unix_owner_and_mode_are_trusted(
        metadata.uid(),
        metadata.mode(),
        effective_uid
    ));
    assert!(!snapshot_unix_owner_and_mode_are_trusted(
        metadata.uid().wrapping_add(1),
        metadata.mode(),
        effective_uid
    ));

    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o770))
        .expect("make snapshot directory group-writable");
    assert!(matches!(
        direct_snapshot_directory_identity(&directory),
        Err(TryReadError::SnapshotGenerationInvalid { .. })
    ));

    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
        .expect("restore snapshot directory permissions");
    let artifact = directory.join("artifact");
    std::fs::write(&artifact, b"snapshot").expect("write snapshot artifact");
    std::fs::set_permissions(&artifact, std::fs::Permissions::from_mode(0o660))
        .expect("make snapshot artifact group-writable");
    let error = read_bounded_stable_regular_file(&artifact, 1024)
        .expect_err("group-writable snapshot artifact must fail closed");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
}

#[cfg(unix)]
#[tokio::test]
async fn bounded_snapshot_reader_rejects_symlink_and_hardlink() {
    use std::os::unix::fs::symlink;

    let root = tempdir().expect("tempdir");
    let victim = root.path().join("victim");
    let symlink_path = root.path().join("symlink");
    let hardlink_path = root.path().join("hardlink");
    std::fs::write(&victim, b"sensitive victim bytes").expect("write victim");
    symlink(&victim, &symlink_path).expect("create symlink");
    std::fs::hard_link(&victim, &hardlink_path).expect("create hardlink");

    for path in [&symlink_path, &hardlink_path] {
        let error = read_bounded_stable_regular_file(path, 1024)
            .expect_err("linked snapshot artifact must fail closed");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }
    assert_eq!(
        std::fs::read(&victim).expect("read victim"),
        b"sensitive victim bytes"
    );
}

#[tokio::test]
async fn snapshot_publication_defers_without_checkpoint_and_selects_nothing() {
    let (state, kura, _block, _artifact) = snapshot_gate_fixture();
    let root = tempdir().expect("snapshot gate temp root");
    let store_dir = root.path().join("snapshot");
    let signing_key = checked_random_snapshot_keypair();

    let error = try_write_snapshot(&state, &store_dir, &signing_key, TEST_CHUNK_SIZE)
        .expect_err("a durable body without its checkpoint must defer snapshot publication");
    assert!(matches!(
        error,
        TryWriteError::CommitEvidenceDeferred { .. }
    ));
    assert_snapshot_bundle_absent(&store_dir);
    assert!(
        try_read_snapshot(
            &store_dir,
            &kura,
            LiveQueryStore::start_test,
            BlockCount(1),
            TEST_CHUNK_SIZE,
            signing_key.public_key(),
            &state.network_id,
            &state.zk_snapshot(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::new(<_>::default(), true),
        )
        .is_err(),
        "restart must not select a rejected unpublished generation"
    );
}

#[tokio::test]
async fn snapshot_publication_defers_bound_manifest_without_finality() {
    let (state, kura, block, artifact) = snapshot_gate_fixture();
    let state_hash = canonical_state_snapshot_hash(&state);
    store_snapshot_checkpoint_and_manifest(&state, &kura, &block, state_hash, &artifact);
    let root = tempdir().expect("snapshot gate temp root");
    let store_dir = root.path().join("snapshot");

    let error = try_write_snapshot(
        &state,
        &store_dir,
        &checked_random_snapshot_keypair(),
        TEST_CHUNK_SIZE,
    )
    .expect_err("checkpoint and manifest without finality must defer publication");
    assert!(matches!(
        error,
        TryWriteError::CommitEvidenceDeferred { .. }
    ));
    assert_snapshot_bundle_absent(&store_dir);
}

#[tokio::test]
async fn snapshot_publication_rejects_mismatched_state_hash() {
    let (state, kura, block, artifact) = snapshot_gate_fixture();
    let wrong_state_hash = Hash::new(b"adversarial snapshot state hash");
    store_snapshot_checkpoint_and_manifest(&state, &kura, &block, wrong_state_hash, &artifact);
    let _ = kura
        .store_v2_finality_artifact(&artifact)
        .expect("store exact finality artifact");
    let root = tempdir().expect("snapshot gate temp root");
    let store_dir = root.path().join("snapshot");

    let error = try_write_snapshot(
        &state,
        &store_dir,
        &checked_random_snapshot_keypair(),
        TEST_CHUNK_SIZE,
    )
    .expect_err("a mismatched WSV checkpoint must fail snapshot publication");
    assert!(matches!(error, TryWriteError::CommitEvidence { .. }));
    assert_snapshot_bundle_absent(&store_dir);
}

#[tokio::test]
async fn snapshot_publication_rejects_foreign_manifest_authority() {
    let (state, kura, block, artifact) = snapshot_gate_fixture();
    let foreign_block = signed_block_with_transaction(accepted_log_transaction("foreign"));
    let foreign = signed_complete_wire_finality_for_snapshot_blocks(
        &state.network_id,
        std::slice::from_ref(&foreign_block),
    )
    .into_iter()
    .next()
    .expect("foreign authority artifact");
    let state_hash = canonical_state_snapshot_hash(&state);
    store_snapshot_checkpoint_and_manifest(&state, &kura, &block, state_hash, &foreign);
    let _ = kura
        .store_v2_finality_artifact(&artifact)
        .expect("store exact finality artifact");
    let root = tempdir().expect("snapshot gate temp root");
    let store_dir = root.path().join("snapshot");

    let error = try_write_snapshot(
        &state,
        &store_dir,
        &checked_random_snapshot_keypair(),
        TEST_CHUNK_SIZE,
    )
    .expect_err("a foreign manifest authority must fail snapshot publication");
    assert!(matches!(error, TryWriteError::CommitEvidence { .. }));
    assert_snapshot_bundle_absent(&store_dir);
}

#[tokio::test]
async fn snapshot_publication_accepts_complete_authenticated_tuple() {
    let (mut state, kura, block, artifact) = snapshot_gate_fixture();
    state.nexus.get_mut().autoscale.scale_out_window_blocks =
        std::num::NonZeroU16::new(48).expect("non-zero scale-out window");
    state.nexus.get_mut().autoscale.scale_in_window_blocks =
        std::num::NonZeroU16::new(192).expect("non-zero scale-in window");
    let state_hash = canonical_state_snapshot_hash(&state);
    store_snapshot_checkpoint_and_manifest(&state, &kura, &block, state_hash, &artifact);
    let _ = kura
        .store_v2_finality_artifact(&artifact)
        .expect("store exact finality artifact");
    let root = tempdir().expect("snapshot gate temp root");
    let store_dir = root.path().join("snapshot");
    let signing_key = checked_random_snapshot_keypair();

    try_write_snapshot(&state, &store_dir, &signing_key, TEST_CHUNK_SIZE)
        .expect("complete authenticated commit tuple must permit publication");
    assert_canonical_snapshot_generation(&store_dir);
    SNAPSHOT_HASH_RECONCILIATION_PASSES.with(|passes| passes.set(0));
    let restored = try_read_snapshot(
        &store_dir,
        &kura,
        LiveQueryStore::start_test,
        BlockCount(state.committed_height()),
        TEST_CHUNK_SIZE,
        signing_key.public_key(),
        state.network_id_ref(),
        &state.zk_snapshot(),
        #[cfg(feature = "telemetry")]
        StateTelemetry::new(<_>::default(), true),
    )
    .expect("post-height snapshot must remain exactly restart-readable");
    SNAPSHOT_HASH_RECONCILIATION_PASSES.with(|passes| {
        assert_eq!(
            passes.get(),
            1,
            "authenticated snapshot restart must reconcile the Kura prefix once"
        );
    });
    assert_eq!(
        restored.nexus_snapshot().autoscale.scale_out_window_blocks,
        state.nexus_snapshot().autoscale.scale_out_window_blocks
    );
    assert_eq!(
        restored.nexus_snapshot().autoscale.scale_in_window_blocks,
        state.nexus_snapshot().autoscale.scale_in_window_blocks
    );
    assert_eq!(
        exact_snapshot_payload_bytes(&restored),
        exact_snapshot_payload_bytes(&state),
        "post-height publication must preserve the exact canonical restart payload"
    );
}

#[tokio::test]
async fn snapshot_fixture_key_generation_preserves_algorithm() {
    assert_eq!(
        checked_random_snapshot_keypair().public_key().algorithm(),
        Algorithm::default()
    );
    assert_eq!(
        checked_random_snapshot_bls_keypair()
            .public_key()
            .algorithm(),
        Algorithm::BlsNormal
    );
}

#[tokio::test]
async fn snapshot_bootstrap_policy_requires_exact_canonical_digest_and_height() {
    let digest = "1a0861b04fa35fd0d8ea4c2f38baaa478c7430df3466e9401c53f934671747bd";
    let policy = SnapshotBootstrapPolicy {
        enabled: true,
        audited_sha256: Some(digest.to_owned()),
        audited_height: Some(42),
    };
    assert!(policy.validate().is_ok());
    assert!(policy.authorizes(digest, 42));
    assert!(!policy.authorizes(
        "2a0861b04fa35fd0d8ea4c2f38baaa478c7430df3466e9401c53f934671747bd",
        42
    ));
    assert!(!policy.authorizes(digest, 41));

    let invalid_uppercase = SnapshotBootstrapPolicy {
        audited_sha256: Some(digest.to_ascii_uppercase()),
        ..policy.clone()
    };
    assert!(invalid_uppercase.validate().is_err());

    let disabled = SnapshotBootstrapPolicy::default();
    assert!(disabled.validate().is_ok());
    assert!(!disabled.authorizes(digest, 42));
    let disabled_with_authority = SnapshotBootstrapPolicy {
        enabled: false,
        audited_sha256: Some(digest.to_owned()),
        audited_height: Some(42),
    };
    assert!(disabled_with_authority.validate().is_err());
}

fn state_factory_with_kura_and_chain(kura: Arc<Kura>, chain_id: ChainId) -> State {
    let query_handle = LiveQueryStore::start_test();
    State::new_with_chain(
        crate::queue::tests::world_with_test_domains(),
        kura,
        query_handle,
        chain_id,
    )
}

fn state_factory_with_kura(kura: Arc<Kura>) -> State {
    state_factory_with_kura_and_chain(kura, ChainId::from(TEST_CHAIN_ID))
}

fn state_factory() -> State {
    state_factory_with_kura(Kura::blank_kura_for_testing())
}

fn sccp_registry_for_snapshot_test() -> crate::state::SccpOnChainRegistryV1 {
    let route = iroha_sccp::sccp_exact_evm_governed_route_test_fixture_v1(
        iroha_data_model::bridge::SccpNetworkV1::EthereumSepolia,
        iroha_data_model::bridge::SccpRouteActivationV1::Staged,
    );
    crate::state::SccpOnChainRegistryV1 {
        version: 1,
        lanes: vec![iroha_data_model::bridge::SccpGovernedLaneV1 {
            lane_id: route.lane_id,
            native_trust_anchors: Vec::new(),
            current_native_trust_anchor_hash: None,
            routes: vec![route],
        }],
    }
}

fn state_with_exact_pending_sccp_snapshot_fixture(
    kura: Arc<Kura>,
) -> (
    State,
    iroha_data_model::bridge::SccpOutboundMessageKeyV1,
    iroha_data_model::bridge::SccpOutboundPendingMessageRecordV1,
) {
    let exact = iroha_sccp::sccp_exact_outbound_test_fixture_v1();
    let provisional_finality =
        iroha_sccp::decode_taira_bridge_finality_proof(&exact.bundle.finality_proof)
            .expect("exact provisional SCCP finality fixture decodes");
    let payload_bytes = iroha_sccp::canonical_sccp_payload_bytes(&exact.bundle.payload)
        .expect("exact SCCP payload encodes canonically");
    let instruction = crate::bridge::test_record_sccp_message(payload_bytes.clone());
    assert_eq!(
        instruction.context, exact.bundle.commitment.context,
        "exact snapshot block instruction must preserve the bundle context"
    );
    let transaction_key = checked_seeded_keypair(0x34, Algorithm::Ed25519);
    let authority = AccountId::new(transaction_key.public_key().clone());
    let transaction = TransactionBuilder::new(
        snapshot_test_network_id(),
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(iroha_data_model::transaction::Executable::IvmProved(
        iroha_data_model::transaction::IvmProved {
            bytecode: iroha_data_model::transaction::IvmBytecode::from_compiled(vec![
                0x01, 0x02, 0x03,
            ]),
            overlay: vec![iroha_data_model::isi::InstructionBox::from(instruction)].into(),
            events_commitment: Hash::new(b"snapshot-sccp-events"),
            gas_policy_commitment: Hash::new(b"snapshot-sccp-gas"),
        },
    ))
    .sign(transaction_key.private_key());
    let entry_hash = transaction.hash_as_entrypoint();
    let block_signer = checked_seeded_keypair(0x35, Algorithm::Ed25519);
    let template_header = provisional_finality.block_header;
    let mut provisional_header = iroha_data_model::block::BlockHeader::new(
        template_header.height(),
        template_header.prev_block_hash(),
        None,
        None,
        u64::try_from(template_header.creation_time().as_millis())
            .expect("fixture creation time fits u64"),
        template_header.view_change_index(),
    );
    provisional_header.set_sccp_commitment_root(template_header.sccp_commitment_root());
    let signature = iroha_data_model::block::BlockSignature::new(
        0,
        iroha_crypto::SignatureOf::try_from_hash(
            block_signer.private_key(),
            provisional_header.hash(),
        )
        .expect("sign provisional retained SCCP header"),
    );
    let mut block = SignedBlock::presigned(signature, provisional_header, vec![transaction]);
    block
        .set_transaction_results(
            Vec::new(),
            &[entry_hash],
            vec![iroha_data_model::transaction::TransactionResultInner::Ok(
                iroha_data_model::transaction::DataTriggerSequence::default(),
            )],
        )
        .expect("exact retained SCCP block results");
    assert!(
        provisional_finality
            .finality_artifact
            .validate_for_header(&block.header())
            .is_err(),
        "pre-finalization SCCP artifact must reject the completed snapshot block"
    );
    let signature = iroha_data_model::block::BlockSignature::new(
        0,
        iroha_crypto::SignatureOf::try_from_hash(block_signer.private_key(), block.hash())
            .expect("sign completed retained SCCP header"),
    );
    block
        .replace_signatures([signature].into_iter().collect())
        .expect("replace provisional retained SCCP signature");
    block
        .signatures()
        .next()
        .expect("completed retained SCCP signature")
        .signature()
        .verify_hash(block_signer.public_key(), block.hash())
        .expect("completed retained SCCP signature verifies");
    crate::bridge::validate_sccp_commitment_root_for_signed_block(&block)
        .expect("completed snapshot block authenticates its exact SCCP message");

    let exact = exact.with_finalized_block(&block, None);
    let finality = iroha_sccp::decode_taira_bridge_finality_proof(&exact.bundle.finality_proof)
        .expect("exact completed SCCP finality fixture decodes");
    assert_eq!(block.header(), finality.block_header);
    assert_eq!(block.hash(), finality.finality_artifact.block_hash);
    assert_eq!(
        exact.request.public_inputs.finality_block_hash,
        <[u8; 32]>::from(Hash::from(block.hash()))
    );
    finality
        .finality_artifact
        .validate_for_header(&block.header())
        .expect("completed snapshot SCCP artifact binds the exact block header");
    finality
        .finality_artifact
        .verify()
        .expect("completed snapshot SCCP artifact is cryptographically valid");
    let block = Arc::new(block);
    kura.persist_block_with_retained_archive_for_tests(&block)
        .expect("persist exact SCCP block and archive");
    let _ = kura
        .store_v2_finality_artifact(&finality.finality_artifact)
        .expect("persist exact SCCP finality artifact");

    let mut state = state_factory_with_kura_and_chain(
        Arc::clone(&kura),
        ChainId::from(iroha_sccp::SCCP_TAIRA_CHAIN_ID_V1),
    );
    state.push_block_hash_for_testing(block.hash());
    let (_, source_identity, trust_anchor) =
        iroha_sccp::sccp_native_ethereum_transfer_inbound_test_fixture_v1();
    assert_eq!(
        exact.route.source_identity, source_identity,
        "exact snapshot route and native trust anchor must share one source identity"
    );
    state.set_sccp_registry_for_testing(
        crate::state::ValidatedSccpRegistryV1::try_from_wire(
            iroha_data_model::bridge::SccpRegistryV1 {
                version: 1,
                lanes: vec![iroha_data_model::bridge::SccpGovernedLaneV1 {
                    lane_id: exact.route.lane_id,
                    native_trust_anchors: vec![trust_anchor],
                    current_native_trust_anchor_hash: Some(trust_anchor.anchor_hash),
                    routes: vec![exact.route.clone()],
                }],
            },
        )
        .expect("exact outbound snapshot registry validates"),
    );
    let key = iroha_data_model::bridge::SccpOutboundMessageKeyV1 {
        lane: exact.bundle.commitment.context.lane,
        message_id: exact.bundle.commitment.message_id,
    };
    let record = iroha_data_model::bridge::SccpOutboundPendingMessageRecordV1 {
        destination_binding_hash: exact.bundle.commitment.context.destination_binding_hash,
        route_configuration_hash: exact.bundle.commitment.context.route_configuration_hash,
        payload_hash: exact.bundle.commitment.payload_hash,
        payload_bytes,
        recorded_at_height: 1,
        commitment_index: 0,
    };
    state
        .insert_sccp_outbound_message_for_testing(key.clone(), record.clone())
        .expect("insert canonical SCCP outbound snapshot fixture");
    store_complete_snapshot_commit_evidence(
        &state,
        &kura,
        block.as_ref(),
        &finality.finality_artifact,
    );
    (state, key, record)
}
fn kura_config_for_snapshot_test(store_dir: &Path, blocks_in_memory: NonZeroUsize) -> KuraConfig {
    KuraConfig {
        init_mode: InitMode::Strict,
        store_dir: WithOrigin::inline(store_dir.to_path_buf()),
        max_disk_usage_bytes: MAX_DISK_USAGE_BYTES,
        blocks_in_memory,
        debug_output_new_blocks: false,
        merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
        fsync_mode: FsyncMode::Batched,
        fsync_interval: FSYNC_INTERVAL,
        block_sync_roster_retention: BLOCK_SYNC_ROSTER_RETENTION,
        roster_sidecar_retention: ROSTER_SIDECAR_RETENTION,
        replica_advert: REPLICA_ADVERT_POLICY,
    }
}

fn install_active_space_directory_manifest(
    state: &mut State,
) -> (UniversalAccountId, DataSpaceId, AccountId) {
    let uaid = UniversalAccountId::from_hash(Hash::new(b"snapshot-space-directory"));
    let dataspace = DataSpaceId::new(7);
    let account_id = AccountId::new(checked_random_snapshot_keypair().public_key().clone());
    let details = AccountDetails::new(Metadata::default(), None, Some(uaid), Vec::new());
    state
        .world
        .accounts
        .insert(account_id.clone(), AccountValue::new(details));

    let manifest = AssetPermissionManifest {
        version: ManifestVersion::default(),
        uaid,
        dataspace,
        issued_ms: 1,
        activation_epoch: 1,
        expiry_epoch: None,
        entries: Vec::new(),
    };
    let mut record = crate::nexus::space_directory::SpaceDirectoryManifestRecord::new(manifest);
    record.lifecycle.mark_activated(1);
    let mut set = crate::nexus::space_directory::SpaceDirectoryManifestSet::default();
    set.upsert(record);
    state.world.space_directory_manifests.insert(uaid, set);

    (uaid, dataspace, account_id)
}

fn resource_policy(
    max_decode_depth: usize,
    max_decode_items: usize,
    max_string_bytes: usize,
    max_blob_bytes: usize,
    max_transient_bytes: usize,
) -> SnapshotResourcePolicy {
    SnapshotResourcePolicy {
        max_decode_depth: NonZeroUsize::new(max_decode_depth).expect("non-zero depth"),
        max_decode_items: NonZeroUsize::new(max_decode_items).expect("non-zero item limit"),
        max_string_bytes: NonZeroUsize::new(max_string_bytes).expect("non-zero string limit"),
        max_blob_bytes: NonZeroUsize::new(max_blob_bytes).expect("non-zero blob limit"),
        max_transient_bytes: NonZeroUsize::new(max_transient_bytes)
            .expect("non-zero transient limit"),
    }
}

#[tokio::test]
async fn snapshot_json_scanner_enforces_every_resource_budget() {
    assert_eq!(
        count_borrowed_json_array_items("[0,{\"nested\":true},[]]")
            .expect("borrowed array item count"),
        3
    );
    let generous = usize::MAX / 4;
    let cases = [
        (
            b"[[0]]".as_slice(),
            resource_policy(2, generous, generous, generous, generous),
            "nesting depth",
        ),
        (
            b"[0,1]".as_slice(),
            resource_policy(8, 1, generous, generous, generous),
            "aggregate items",
        ),
        (
            br#""four""#.as_slice(),
            resource_policy(8, generous, 3, generous, generous),
            "JSON string",
        ),
        (
            br#"{"encoded_hex":"000102030405060708090a0b0c"}"#.as_slice(),
            resource_policy(8, generous, 12, 12, generous),
            "decoded blob",
        ),
        (
            b"[0,1,2,3,4,5,6,7,8,9,10,11,12]".as_slice(),
            resource_policy(8, generous, 12, 12, generous),
            "byte-vector blobs",
        ),
        (
            b"[0]".as_slice(),
            resource_policy(8, generous, generous, generous, 1),
            "transient estimate",
        ),
    ];
    for (payload, policy, expected) in cases {
        let error = validate_snapshot_json_resources(payload, policy)
            .expect_err("payload must exceed its configured resource budget");
        match error {
            TryReadError::SnapshotResourceLimit(message) => {
                assert!(
                    message.contains(expected),
                    "unexpected resource error: {message}"
                );
            }
            other => panic!("unexpected resource rejection: {other:?}"),
        }
    }
}

#[tokio::test]
async fn snapshot_json_scanner_rejects_noncanonical_spelling() {
    let policy = SnapshotResourcePolicy::default();
    for payload in [b"{ \"a\":0}".as_slice(), br#""\u0061""#.as_slice()] {
        assert!(matches!(
            validate_snapshot_json_resources(payload, policy),
            Err(TryReadError::NonCanonicalSnapshotPayload)
        ));
    }
}

#[tokio::test]
async fn borrowed_snapshot_wsv_hash_matches_typed_canonical_surface() {
    let state = state_factory();
    let payload = exact_snapshot_payload_bytes(&state);
    validate_snapshot_json_resources(&payload, SnapshotResourcePolicy::default())
        .expect("generated snapshot must satisfy the default resource policy");
    let tree_reference = Hash::new(canonical_state_snapshot_bytes(&state));
    assert_eq!(
        canonical_snapshot_wsv_hash(&payload).expect("borrowed canonical WSV hash"),
        tree_reference,
    );
    assert_eq!(canonical_state_snapshot_hash(&state), tree_reference);
}

#[tokio::test]
async fn borrowed_snapshot_wsv_hash_canonicalizes_json_lexemes() {
    let lexical = br#"{"\u0077orld":{"note":"\u0061","number":1e0}}"#;
    let canonical = br#"{"world":{"note":"a","number":1.0}}"#;
    assert_eq!(
        canonical_snapshot_wsv_hash(lexical).expect("hash lexical snapshot spelling"),
        Hash::new(canonical),
    );

    let lexical_set =
        br#"{"world":{"parameters":{"sumeragi":{"key_allowed_hsm_providers":["\u0078","x"]}}}}"#;
    let canonical_set =
        br#"{"world":{"parameters":{"sumeragi":{"key_allowed_hsm_providers":["x"]}}}}"#;
    assert_eq!(
        canonical_snapshot_wsv_hash(lexical_set).expect("hash lexical set spelling"),
        Hash::new(canonical_set),
    );
}

#[tokio::test]
async fn staged_snapshot_wsv_hash_injects_committed_event_buffer() {
    let staged = br#"{"world":{"accounts":{}}}"#;
    let committed_event_buffer = r#"{"revert":{},"blocks":[]}"#;
    let canonical = br#"{"world":{"accounts":{},"external_event_buf":[]}}"#;
    assert_eq!(
        canonical_snapshot_wsv_hash_with_overrides(
            staged,
            CanonicalWsvOverrides {
                committed_external_event_buf: Some(committed_event_buffer),
            },
        )
        .expect("hash staged snapshot with its committed event buffer"),
        Hash::new(canonical),
    );
}

#[tokio::test]
async fn canonical_wsv_hash_ignores_commit_qc_sidecars() {
    let mut state = state_factory();
    let before = canonical_state_snapshot_bytes_for_tests(&state);
    let key_pair = checked_random_snapshot_bls_keypair();
    let peer = PeerId::new(key_pair.public_key().clone());
    let roster = vec![peer];
    let block_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xC7; Hash::LENGTH]));
    let zero_root = Hash::prehashed([0_u8; Hash::LENGTH]);
    let qc = Qc {
        phase: crate::sumeragi::consensus::Phase::Commit,
        subject_block_hash: block_hash,
        parent_state_root: zero_root,
        post_state_root: zero_root,
        height: 2,
        view: 0,
        epoch: 0,
        chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
        rechain_seq: 0,
        mode_tag: crate::sumeragi::consensus::PERMISSIONED_TAG.to_owned(),
        highest_qc: None,
        validator_set_hash: HashOf::new(&roster),
        validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
        validator_set: roster,
        aggregate: crate::sumeragi::consensus::QcAggregate {
            signers_bitmap: vec![0b0000_0001],
            bls_aggregate_signature: vec![0xAA; 96],
        },
    };

    state.insert_commit_qc_for_testing(block_hash, qc);
    let mut restart_snapshot = String::new();
    serialize_state_snapshot(&state, &mut restart_snapshot, true);
    assert!(
        restart_snapshot.contains("\"commit_qcs\""),
        "restart snapshots must retain the historical commit-QC archive"
    );

    let after = canonical_state_snapshot_bytes_for_tests(&state);
    assert_eq!(
        before, after,
        "commit-QC recovery evidence must not affect replay WSV checkpoints"
    );
    assert_eq!(
        canonical_snapshot_wsv_hash(restart_snapshot.as_bytes())
            .expect("borrowed WSV hashing must redact commit-QC sidecars"),
        Hash::new(&after),
    );
    let canonical_json =
        String::from_utf8(after).expect("canonical state snapshot should be utf8 json");
    assert!(
        !canonical_json.contains("\"commit_qcs\""),
        "canonical WSV checkpoint surface should omit commit-QC sidecars"
    );
}

#[tokio::test]
async fn canonical_wsv_hash_uses_current_mv_cell_values() {
    let state = state_factory();
    let before = canonical_state_snapshot_bytes_for_tests(&state);

    {
        let mut parameters = state.world.parameters.block();
        let current = parameters.get().clone();
        *parameters.get_mut() = current;
        parameters.commit();
    }

    let after = canonical_state_snapshot_bytes_for_tests(&state);
    assert_eq!(
        before, after,
        "MV cell history must not affect replay WSV checkpoints when the current value is unchanged"
    );
    assert_eq!(
        canonical_snapshot_wsv_hash(&exact_snapshot_payload_bytes(&state))
            .expect("borrowed WSV hashing must unwrap MV cells"),
        Hash::new(&after),
    );

    let value = canonical_state_snapshot_value(&state);
    let parameters = value
        .get("world")
        .and_then(|world| world.get("parameters"))
        .and_then(json::Value::as_object)
        .expect("canonical snapshot should contain parameters as a plain object");
    assert!(
        !parameters.contains_key("revert") && !parameters.contains_key("blocks"),
        "canonical WSV checkpoint surface should serialize current cell values"
    );
}

fn test_vrf_epoch_record(epoch: u64) -> iroha_data_model::consensus::VrfEpochRecord {
    iroha_data_model::consensus::VrfEpochRecord {
        epoch,
        seed: [0_u8; 32],
        epoch_length: 1,
        commit_deadline_offset: 0,
        reveal_deadline_offset: 0,
        roster_len: 0,
        finalized: false,
        updated_at_height: 0,
        participants: Vec::new(),
        late_reveals: Vec::new(),
        committed_no_reveal: Vec::new(),
        no_participation: Vec::new(),
        penalties_applied: false,
        penalties_applied_at_height: None,
        validator_election: None,
    }
}

#[tokio::test]
async fn canonical_wsv_hash_ignores_vrf_epoch_sidecars() {
    let state = state_factory();
    let before = canonical_state_snapshot_bytes_for_tests(&state);

    {
        let mut world = state.world.block();
        world.vrf_epochs.insert(0, test_vrf_epoch_record(0));
        world.commit();
    }
    let after = canonical_state_snapshot_bytes_for_tests(&state);

    assert_eq!(
        before, after,
        "VRF epoch sidecars must not affect replay WSV checkpoints"
    );
    assert_eq!(
        canonical_snapshot_wsv_hash(&exact_snapshot_payload_bytes(&state))
            .expect("borrowed WSV hashing must redact VRF sidecars"),
        Hash::new(&after),
    );

    let value = canonical_state_snapshot_value(&state);
    let world = value
        .get("world")
        .and_then(json::Value::as_object)
        .expect("canonical snapshot should contain world as an object");
    assert!(
        !world.contains_key("vrf_epochs"),
        "canonical WSV checkpoint surface should omit VRF epoch sidecars"
    );
}

#[tokio::test]
async fn canonical_wsv_hash_sorts_sumeragi_key_policy_sets() {
    let state = state_factory();

    {
        let mut parameters = state.world.parameters.block();
        parameters.sumeragi.key_allowed_algorithms = vec![
            Algorithm::Secp256k1,
            Algorithm::Ed25519,
            Algorithm::Secp256k1,
        ];
        parameters.sumeragi.key_allowed_hsm_providers = vec![
            "yubihsm".to_owned(),
            "pkcs11".to_owned(),
            "softkey".to_owned(),
            "pkcs11".to_owned(),
        ];
        parameters.commit();
    }
    let first = canonical_state_snapshot_bytes_for_tests(&state);
    let first_payload = exact_snapshot_payload_bytes(&state);
    assert_eq!(
        canonical_snapshot_wsv_hash(&first_payload)
            .expect("borrowed WSV hashing must canonicalize set-like key policy fields"),
        Hash::new(&first),
    );

    {
        let mut parameters = state.world.parameters.block();
        parameters.sumeragi.key_allowed_algorithms = vec![Algorithm::Ed25519, Algorithm::Secp256k1];
        parameters.sumeragi.key_allowed_hsm_providers = vec![
            "pkcs11".to_owned(),
            "softkey".to_owned(),
            "yubihsm".to_owned(),
        ];
        parameters.commit();
    }
    let second = canonical_state_snapshot_bytes_for_tests(&state);
    let second_payload = exact_snapshot_payload_bytes(&state);
    assert_eq!(
        canonical_snapshot_wsv_hash(&second_payload)
            .expect("borrowed WSV hashing must preserve canonical key policy sets"),
        Hash::new(&second),
    );

    assert_eq!(
        first, second,
        "set-like Sumeragi key policy fields must not make WSV checkpoints order-sensitive"
    );

    let value = canonical_state_snapshot_value(&state);
    let providers = value
        .get("world")
        .and_then(|world| world.get("parameters"))
        .and_then(|parameters| parameters.get("sumeragi"))
        .and_then(|sumeragi| sumeragi.get("key_allowed_hsm_providers"))
        .and_then(json::Value::as_array)
        .expect("canonical snapshot should contain normalized HSM providers");
    let providers = providers
        .iter()
        .map(|value| match value {
            json::Value::String(provider) => provider.as_str(),
            _ => panic!("HSM provider should serialize as a string"),
        })
        .collect::<Vec<_>>();
    assert_eq!(providers, ["pkcs11", "softkey", "yubihsm"]);
}

#[tokio::test]
async fn canonical_state_snapshot_ignores_consensus_evidence_caches() {
    let state = state_factory();
    let expected = canonical_state_snapshot_bytes_for_tests(&state);

    let keypair = checked_random_snapshot_bls_keypair();
    let peer = PeerId::new(keypair.public_key().clone());
    let roster = vec![peer.clone()];
    let block_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA1; Hash::LENGTH]));
    let commit_qc = crate::sumeragi::consensus::Qc {
        phase: crate::sumeragi::consensus::Phase::Commit,
        subject_block_hash: block_hash,
        parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
        post_state_root: Hash::prehashed([1u8; Hash::LENGTH]),
        height: 2,
        view: 1,
        epoch: 0,
        chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
        rechain_seq: 0,
        mode_tag: crate::sumeragi::consensus::PERMISSIONED_TAG.to_string(),
        highest_qc: None,
        validator_set_hash: HashOf::new(&roster),
        validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
        validator_set: roster,
        aggregate: crate::sumeragi::consensus::QcAggregate {
            signers_bitmap: vec![0b0000_0001],
            bls_aggregate_signature: Vec::new(),
        },
    };
    let vrf_epoch = iroha_data_model::consensus::VrfEpochRecord {
        epoch: 0,
        seed: [0x42; 32],
        epoch_length: 10,
        commit_deadline_offset: 2,
        reveal_deadline_offset: 4,
        roster_len: 1,
        finalized: false,
        updated_at_height: 2,
        participants: Vec::new(),
        late_reveals: Vec::new(),
        committed_no_reveal: Vec::new(),
        no_participation: Vec::new(),
        penalties_applied: false,
        penalties_applied_at_height: None,
        validator_election: None,
    };

    {
        let mut world = state.world.block();
        world
            .commit_qcs_mut_for_testing()
            .insert(block_hash, commit_qc);
        world
            .vrf_epochs_mut_for_testing()
            .insert(vrf_epoch.epoch, vrf_epoch);
        world.commit();
    }
    {
        let mut commit_topology = state.commit_topology.block();
        commit_topology.push(peer.clone());
        commit_topology.commit();
    }
    {
        let mut prev_commit_topology = state.prev_commit_topology.block();
        prev_commit_topology.push(peer);
        prev_commit_topology.commit();
    }

    assert_eq!(
        canonical_state_snapshot_bytes_for_tests(&state),
        expected,
        "consensus evidence caches must not perturb canonical replay checkpoints"
    );
}

fn sample_space_directory_manifest() -> AssetPermissionManifest {
    AssetPermissionManifest {
        version: ManifestVersion::default(),
        uaid: UniversalAccountId::from_hash(Hash::new(b"snapshot-legacy-manifest")),
        dataspace: DataSpaceId::new(11),
        issued_ms: 1,
        activation_epoch: 1,
        expiry_epoch: None,
        entries: Vec::new(),
    }
}

fn insert_account_with_uaid(state: &mut State, uaid: UniversalAccountId) -> AccountId {
    let account_id = AccountId::new(checked_random_snapshot_keypair().public_key().clone());
    let details = AccountDetails::new(Metadata::default(), None, Some(uaid), Vec::new());
    state
        .world
        .accounts
        .insert(account_id.clone(), AccountValue::new(details));
    account_id
}

fn accepted_manifest_transaction() -> AcceptedTransaction<'static> {
    let key_pair = checked_seeded_keypair(0x31, Algorithm::Ed25519);
    let authority = AccountId::new(key_pair.public_key().clone());
    let transaction = TransactionBuilder::new(
        snapshot_test_network_id(),
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([PublishSpaceDirectoryManifest {
        manifest: sample_space_directory_manifest(),
    }])
    .sign(key_pair.private_key());
    AcceptedTransaction::new_unchecked(Cow::Owned(transaction))
}

fn accepted_log_transaction(message: &str) -> AcceptedTransaction<'static> {
    let key_pair = checked_seeded_keypair(0x32, Algorithm::Ed25519);
    let authority = AccountId::new(key_pair.public_key().clone());
    let transaction = TransactionBuilder::new(
        snapshot_test_network_id(),
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, message.to_owned())])
    .sign(key_pair.private_key());
    AcceptedTransaction::new_unchecked(Cow::Owned(transaction))
}

fn signed_block_with_transaction(transaction: AcceptedTransaction<'static>) -> Arc<SignedBlock> {
    signed_block_after_transaction(transaction, None)
}

fn signed_block_after_transaction(
    transaction: AcceptedTransaction<'static>,
    latest_block: Option<&SignedBlock>,
) -> Arc<SignedBlock> {
    let block_signer = checked_seeded_keypair(0x33, Algorithm::BlsNormal);
    Arc::new(
        BlockBuilder::new(vec![transaction])
            .chain(0, latest_block)
            .sign(block_signer.private_key())
            .unpack(|_| {})
            .into(),
    )
}

fn legacy_snapshot_bytes_without_space_directory_section(state: &State) -> Vec<u8> {
    let mut payload = String::new();
    serialize_state_snapshot(state, &mut payload, false);
    payload.into_bytes()
}

fn exact_snapshot_payload_bytes(state: &State) -> Vec<u8> {
    let mut payload = String::new();
    serialize_state_snapshot(state, &mut payload, true);
    payload.into_bytes()
}

fn publish_test_snapshot_generation(
    store_dir: &std::path::Path,
    bytes: &[u8],
    key_pair: &KeyPair,
) -> (StableSnapshotFileIdentity, PublishedSnapshotGeneration) {
    std::fs::create_dir_all(store_dir).expect("snapshot dir");
    let digest_bytes = Sha256::digest(bytes);
    let digest_vec = digest_bytes.to_vec();
    let digest_hex = hex::encode(&digest_vec);
    let digest_line = format!("{digest_hex}\n").into_bytes();
    let signature = Signature::try_new(key_pair.private_key(), &digest_vec)
        .expect("checked snapshot signature");
    let signature_hex = hex::encode(signature.payload()).into_bytes();
    let merkle = SnapshotMerkleMetadata::from_bytes(bytes, TEST_CHUNK_SIZE);
    let merkle_bytes = json::to_json(&merkle)
        .expect("canonical snapshot merkle")
        .into_bytes();
    let merkle_limit = SNAPSHOT_MERKLE_FIXED_OVERHEAD_BYTES.saturating_add(
        u64::try_from(merkle.leaf_hashes_hex.len())
            .unwrap_or(u64::MAX)
            .saturating_mul(SNAPSHOT_MERKLE_BYTES_PER_LEAF),
    );
    let store_identity = direct_snapshot_directory_identity(store_dir).expect("bind snapshot root");
    let generation = publish_immutable_snapshot_generation(
        store_dir,
        store_identity,
        &digest_hex,
        bytes,
        &digest_line,
        &signature_hex,
        &merkle_bytes,
        merkle_limit,
        key_pair.public_key(),
    )
    .expect("publish immutable test generation");
    (store_identity, generation)
}

fn write_snapshot_bundle_from_bytes(store_dir: &std::path::Path, bytes: &[u8], key_pair: &KeyPair) {
    let (store_identity, generation) = publish_test_snapshot_generation(store_dir, bytes, key_pair);
    publish_snapshot_current_pointer(
        store_dir,
        store_identity,
        &generation,
        defaults::snapshot::MAX_PAYLOAD_BYTES,
        TEST_CHUNK_SIZE,
        key_pair.public_key(),
    )
    .expect("publish canonical test pointer");
}

fn store_block_and_mark_state_height(state: &mut State, kura: &Arc<Kura>, block: Arc<SignedBlock>) {
    kura.store_block(Arc::clone(&block)).expect("store block");
    state.push_block_hash_for_testing(block.hash());
}

fn signed_commit_qc_for_snapshot(
    network_id: &NetworkId,
    block_hash: HashOf<BlockHeader>,
    height: u64,
    validator: &KeyPair,
) -> Qc {
    let validator_set = vec![PeerId::new(validator.public_key().clone())];
    let zero_root = Hash::prehashed([0; Hash::LENGTH]);
    let vote = Vote {
        phase: Phase::Commit,
        block_hash,
        parent_state_root: zero_root,
        post_state_root: zero_root,
        height,
        view: 0,
        epoch: 0,
        chain_order_hash: default_chain_order_hash(),
        rechain_seq: 0,
        highest_qc: None,
        signer: 0,
        bls_sig: Vec::new(),
    };
    let preimage = vote_preimage(network_id, PERMISSIONED_TAG, &vote);
    let signature = Signature::try_new(validator.private_key(), &preimage)
        .expect("snapshot commit vote signature");
    let aggregate = iroha_crypto::bls_normal_aggregate_signatures(&[signature.payload()])
        .expect("snapshot aggregate commit signature");
    Qc {
        phase: Phase::Commit,
        subject_block_hash: block_hash,
        parent_state_root: zero_root,
        post_state_root: zero_root,
        height,
        view: 0,
        epoch: 0,
        chain_order_hash: default_chain_order_hash(),
        rechain_seq: 0,
        mode_tag: PERMISSIONED_TAG.to_owned(),
        highest_qc: None,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set,
        aggregate: QcAggregate {
            signers_bitmap: vec![1],
            bls_aggregate_signature: aggregate,
        },
    }
}

fn model_rotated_disabled_removed_validator(
    state: &mut State,
    historical_validator: &KeyPair,
) -> Vec<u8> {
    let historical_pop = iroha_crypto::bls_normal_pop_prove(historical_validator.private_key())
        .expect("historical validator PoP");
    state.world.register_validator_pop_for_testing(
        historical_validator.public_key().clone(),
        historical_pop.clone(),
    );
    let replacement = checked_random_snapshot_bls_keypair();
    let replacement_pop = iroha_crypto::bls_normal_pop_prove(replacement.private_key())
        .expect("replacement validator PoP");
    state
        .world
        .register_validator_pop_for_testing(replacement.public_key().clone(), replacement_pop);

    let historical_id = derive_validator_key_id(historical_validator.public_key());
    let replacement_id = derive_validator_key_id(replacement.public_key());
    let mut world = state.world.block();
    let mut historical_record = world
        .consensus_keys
        .get(&historical_id)
        .cloned()
        .expect("historical consensus record");
    historical_record.status = ConsensusKeyStatus::Disabled;
    historical_record.expiry_height = Some(2);
    world
        .consensus_keys
        .insert(historical_id.clone(), historical_record);
    let mut replacement_record = world
        .consensus_keys
        .get(&replacement_id)
        .cloned()
        .expect("replacement consensus record");
    replacement_record.replaces = Some(historical_id);
    world
        .consensus_keys
        .insert(replacement_id, replacement_record);
    world.commit();

    assert!(
        state
            .world
            .peers
            .view()
            .iter()
            .all(|peer| peer.public_key() != historical_validator.public_key()),
        "historical validator must be absent from the live peer roster"
    );
    historical_pop
}
