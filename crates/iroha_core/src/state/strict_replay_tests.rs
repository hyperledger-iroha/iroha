//! Production-path tests for strict Sumeragi-v2 Kura replay.

use std::{
    collections::BTreeSet,
    num::{NonZeroU64, NonZeroUsize},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use crate::sumeragi::v2_core::{EventTag, Generation};
use iroha_config::parameters::actual::{LaneConfig as RuntimeLaneConfig, Queue as QueueConfig};
use iroha_crypto::{Algorithm, Hash, KeyPair, Signature, SignatureOf};
use iroha_data_model::{
    ChainId, Registrable,
    account::{Account, AccountId},
    block::{
        BlockHeader, BlockSignature, SignedBlock, consensus_v2 as wire,
        consensus_v2::finality::V2FinalityArtifact,
    },
    bridge::SccpOutboundMessageContextV1,
    domain::Domain,
    isi::SetParameter,
    parameter::{Parameter, system::SumeragiParameter},
    peer::PeerId,
    transaction::TransactionBuilder,
};
use mv::storage::StorageReadOnly;
use norito::codec::Encode;

use super::{QueryIndexJournal, QueryProjectionCheckpointJournal, State, World, WorldReadOnly};
use crate::{
    governance::manifest::LaneManifestRegistry,
    kura::{CommitManifest, CommitManifestBindingState, Kura},
    query::store::LiveQueryStore,
    queue::Queue,
    sumeragi::{
        v2_apply::V2ApplyService,
        v2_body_store::{BlockSignaturePolicy, V2BodyStore},
        v2_effects::ApplyTask,
    },
};

const HEIGHT: u64 = 1;

/// Test-only mirror of Kura's private retained SCCP message layout.
#[derive(Clone, Debug, PartialEq, Eq, Encode)]
#[norito(deny_unknown_fields)]
struct CorruptedKuraRetainedSccpMessage {
    commitment_index: u32,
    context: SccpOutboundMessageContextV1,
    payload_bytes: Vec<u8>,
}

/// Test-only mirror used to install a disk-corrupted retained record.
#[derive(Clone, Debug, PartialEq, Eq, Encode)]
#[norito(deny_unknown_fields)]
struct CorruptedKuraRetainedBlockRecord {
    format_version: u16,
    height: u64,
    block_hash: iroha_crypto::HashOf<BlockHeader>,
    block_header: BlockHeader,
    proposal_wire_hash: Hash,
    executed_block_wire_hash: Hash,
    sccp_archive: Vec<CorruptedKuraRetainedSccpMessage>,
}

/// Test-only mirror used to install a disk-corrupted v2 finality envelope.
#[derive(Clone, Debug, PartialEq, Eq, Encode)]
#[norito(deny_unknown_fields)]
struct CorruptedKuraV2FinalityRecord {
    format_version: u16,
    block_header: BlockHeader,
    artifact: V2FinalityArtifact,
}

#[derive(Debug, PartialEq, Eq)]
enum TreeEntry {
    Directory,
    File(Vec<u8>),
    Symlink(PathBuf),
}

fn kura_tree_fingerprint(kura: &Kura) -> Vec<(PathBuf, TreeEntry)> {
    fn visit(root: &Path, path: &Path, entries: &mut Vec<(PathBuf, TreeEntry)>) {
        let mut children = std::fs::read_dir(path)
            .unwrap_or_else(|error| panic!("read Kura tree {}: {error}", path.display()))
            .map(|entry| entry.expect("read Kura tree entry").path())
            .collect::<Vec<_>>();
        children.sort();
        for child in children {
            let relative = child
                .strip_prefix(root)
                .expect("Kura child remains below root")
                .to_path_buf();
            let metadata = std::fs::symlink_metadata(&child)
                .unwrap_or_else(|error| panic!("stat Kura tree {}: {error}", child.display()));
            if metadata.file_type().is_symlink() {
                entries.push((
                    relative,
                    TreeEntry::Symlink(
                        std::fs::read_link(&child).expect("read Kura tree symlink target"),
                    ),
                ));
            } else if metadata.is_dir() {
                entries.push((relative, TreeEntry::Directory));
                visit(root, &child, entries);
            } else if metadata.is_file() {
                entries.push((
                    relative,
                    TreeEntry::File(std::fs::read(&child).expect("read Kura tree file")),
                ));
            } else {
                panic!("unsupported Kura tree entry type: {}", child.display());
            }
        }
    }

    let root = kura.store_root();
    let mut entries = Vec::new();
    visit(&root, &root, &mut entries);
    entries
}

fn seed_recovery_candidates_for_read_only_prevalidation(kura: &Kura) {
    let root = kura.store_root();

    let query_source = root.join("atomic-replay-query-source.norito");
    let mut query = QueryIndexJournal::new(query_source.clone());
    query.set_latest(
        77,
        Some(iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
            b"atomic replay query temp",
        ))),
    );
    query
        .persist()
        .expect("persist query-index recovery candidate");
    std::fs::rename(
        query_source,
        QueryIndexJournal::journal_path(&root).with_extension("norito.tmp"),
    )
    .expect("install query-index recovery candidate");

    let projection_source = root.join("atomic-replay-projection-source.norito");
    let projection = QueryProjectionCheckpointJournal::new(projection_source.clone());
    projection
        .persist()
        .expect("persist projection recovery candidate");
    std::fs::rename(
        projection_source,
        QueryProjectionCheckpointJournal::journal_path(&root).with_extension("norito.tmp"),
    )
    .expect("install projection recovery candidate");

    let merge_tail = root.join("merge_ledger").join("atomic-replay-tail.tmp");
    std::fs::create_dir_all(merge_tail.parent().expect("merge tail parent"))
        .expect("create merge-tail directory");
    std::fs::write(
        &merge_tail,
        b"unpublished merge tail must remain byte-identical",
    )
    .expect("write unpublished merge tail");
}

struct StateFingerprint {
    snapshot: Vec<u8>,
    height: usize,
    tip: Option<iroha_crypto::HashOf<BlockHeader>>,
    commit_roster_handle:
        Arc<parking_lot::RwLock<crate::commit_roster_journal::CommitRosterJournal>>,
    commit_rosters: Vec<crate::commit_roster_journal::CommitRosterSnapshot>,
    merge_entries: Vec<iroha_data_model::merge::MergeLedgerEntry>,
}

#[derive(Debug, PartialEq, Eq)]
struct RuntimeStateFingerprint {
    commit_rosters: Vec<crate::commit_roster_journal::CommitRosterSnapshot>,
    merge_entries: Vec<iroha_data_model::merge::MergeLedgerEntry>,
    runtime_debug: String,
}

impl RuntimeStateFingerprint {
    fn capture(state: &State) -> Self {
        Self {
            commit_rosters: state.commit_roster_journal.read().snapshots(),
            merge_entries: state
                .merge_ledger
                .snapshot()
                .iter()
                .map(|entry| entry.as_ref().clone())
                .collect(),
            runtime_debug: format!(
                "merge={:?}|da={:?}|confidential={:?}|receipt={:?}|shard={:?}|pin={:?}|relays={:?}|settled={:?}|nexus={:?}|incarnations={:?}|lineage={:?}|activations={:?}",
                state.merge_admission.read(),
                state.da_commitments.read(),
                state.da_confidential_compute.read(),
                state.da_receipt_cursors.read(),
                state.da_shard_cursors.read(),
                state.da_pin_intents.read(),
                state.lane_relays.read(),
                state.settled_nexus_fee_receipts.read(),
                state.nexus.read(),
                state.lane_incarnations.read(),
                state.lane_incarnation_lineage.read(),
                state.lane_incarnation_activation_heights.read(),
            ),
        }
    }
}

impl StateFingerprint {
    fn capture(state: &State) -> Self {
        Self {
            snapshot: crate::snapshot::canonical_state_snapshot_bytes_for_tests(state),
            height: state.committed_height(),
            tip: state.latest_block_hash_fast(),
            commit_roster_handle: Arc::clone(&state.commit_roster_journal),
            commit_rosters: state.commit_roster_journal.read().snapshots(),
            merge_entries: state
                .merge_ledger
                .snapshot()
                .iter()
                .map(|entry| entry.as_ref().clone())
                .collect(),
        }
    }

    fn assert_unchanged(&self, state: &State) {
        assert_eq!(
            state.committed_height(),
            self.height,
            "committed height changed"
        );
        assert_eq!(
            state.latest_block_hash_fast(),
            self.tip,
            "canonical tip changed"
        );
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_bytes_for_tests(state),
            self.snapshot,
            "rejected replay changed the canonical WSV bytes"
        );
        assert!(
            Arc::ptr_eq(&state.commit_roster_journal, &self.commit_roster_handle),
            "rejected replay replaced the live commit-roster authority"
        );
        assert_eq!(
            state.commit_roster_journal.read().snapshots(),
            self.commit_rosters,
            "rejected replay mutated the live commit-roster cache"
        );
        let merge_entries = state
            .merge_ledger
            .snapshot()
            .iter()
            .map(|entry| entry.as_ref().clone())
            .collect::<Vec<_>>();
        assert_eq!(
            merge_entries, self.merge_entries,
            "rejected replay published merge-ledger cache state"
        );
    }
}

struct StrictReplayFixture {
    chain_id: ChainId,
    genesis_account: AccountId,
    genesis_key: KeyPair,
    keys: Vec<KeyPair>,
    context: wire::HeightContext,
    block: SignedBlock,
    artifact: wire::finality::V2FinalityArtifact,
    manifest: CommitManifest,
    checkpoint_hash: Hash,
    expected_snapshot: Vec<u8>,
    kura: Arc<Kura>,
    materialized_state: Arc<State>,
    apply_service: V2ApplyService,
}

struct TwoBlockReplayFixture {
    first: StrictReplayFixture,
    second_context: wire::HeightContext,
    second_block: SignedBlock,
    second_artifact: wire::finality::V2FinalityArtifact,
    second_checkpoint_hash: Hash,
}

impl StrictReplayFixture {
    fn new() -> Self {
        let chain_id: ChainId = "strict-production-v2-replay".into();
        let mut keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("derive deterministic BLS validator key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let roster = keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let mut context = wire::HeightContext {
            chain_id: chain_id.clone(),
            protocol_version: wire::PROTOCOL_VERSION,
            height: HEIGHT,
            epoch: 0,
            epoch_end_height: u64::MAX,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("derive fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"strict replay fixture pending state"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::Plain,
                chunk_size_bytes: 2 * 1024 * 1024,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 2 * 1024 * 1024,
                max_chunk_count: 1,
            },
            leader_seed: [0; 32],
        };
        let leader = context.leader(0);
        let genesis_key = KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::Ed25519)
            .expect("derive deterministic genesis authority key");
        let genesis_account = AccountId::new(genesis_key.public_key().clone());
        let kura = Kura::blank_kura_for_testing();
        let state = Arc::new(Self::new_state(
            Arc::clone(&kura),
            chain_id.clone(),
            genesis_account.clone(),
        ));
        context.nexus_amx_context_hash =
            crate::sumeragi::v2_recovery::committed_nexus_amx_context_hash(state.as_ref());
        context.validate().expect("validate fixture context");
        assert_eq!(context.leader(0), leader, "fixture must freeze its leader");

        let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(32);
        let queue = Arc::new(Queue::from_config(
            QueueConfig::default(),
            events_sender.clone(),
        ));
        let pops = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("derive validator proof of possession")
            })
            .collect::<Vec<_>>();
        let service = V2ApplyService::new(
            Arc::clone(&state),
            queue,
            Arc::clone(&kura),
            None,
            None,
            chain_id.clone(),
            Duration::from_secs(1),
            genesis_account.clone(),
            events_sender,
            pops,
        );

        let transaction = TransactionBuilder::new(
            chain_id.clone(),
            genesis_account.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::MaxClockDriftMs(100),
        ))])
        .sign(genesis_key.private_key());
        let creation_time_ms = (transaction.creation_time() + Duration::from_millis(1))
            .as_millis()
            .try_into()
            .expect("fixture creation time fits u64");
        let mut header = BlockHeader::new(
            NonZeroU64::new(HEIGHT).expect("non-zero height"),
            None,
            None,
            None,
            creation_time_ms,
            0,
        );
        let confidential_features = {
            let view = state.view();
            let digest = crate::state::compute_confidential_feature_digest(
                view.world(),
                &view.zk,
                view.sccp_registry.as_ref(),
                HEIGHT,
            );
            (!digest.is_empty()).then_some(digest)
        };
        header.set_confidential_features(confidential_features);
        let mut builder = iroha_data_model::block::builder::BlockBuilder::new(header);
        builder.push_transaction(transaction);
        builder.set_da_proof_policies(Some(crate::da::active_proof_policy_bundle_at_height(
            &state.nexus_snapshot(),
            HEIGHT,
        )));
        let body = builder
            .try_build_with_signature(0, genesis_key.private_key())
            .expect("sign canonical fixture block")
            .canonical_resultless_proposal();
        let canonical_wire = body.encode_wire().expect("encode canonical fixture block");
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: body.hash(),
            payload_hash: Hash::new(&canonical_wire),
        };
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: HEIGHT,
            view: 0,
        };
        let payload_manifest = wire::PayloadManifest::derive(
            &context,
            round,
            subject,
            u64::try_from(canonical_wire.len()).expect("body size fits u64"),
            std::slice::from_ref(&canonical_wire),
        )
        .expect("derive exact payload manifest");
        let execution_commitment = service
            .validate_candidate(&context, &body)
            .expect("derive exact execution commitment");
        let mut certificate = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: Vec::new(),
        };
        Self::resign_certificate(&mut certificate, &keys);

        let body_root = tempfile::tempdir().expect("create exact-body store");
        let mut body_store = V2BodyStore::open_with_policy(
            body_root.path(),
            context.clone(),
            BlockSignaturePolicy::GenesisAuthority(genesis_key.public_key().clone()),
        )
        .expect("open exact-body store");
        let durable = body_store
            .store(payload_manifest, canonical_wire)
            .expect("persist exact canonical body");
        let validated = body_store
            .validate(&durable, |candidate| {
                service.validate_candidate(&context, candidate)
            })
            .expect("persist exact validation receipt");
        let task = ApplyTask::for_test(
            1,
            EventTag::new(1, 0, Generation::new(1)),
            subject,
            certificate,
            validated,
        );
        let _completion = service
            .execute(&context, &mut body_store, &task)
            .expect("materialize the exact production durable tuple");

        let block = kura
            .get_block(NonZeroUsize::new(1).expect("non-zero height"))
            .expect("read committed canonical block")
            .as_ref()
            .clone();
        let artifact = kura
            .v2_finality_artifact(HEIGHT)
            .expect("read finality artifact")
            .expect("finality artifact exists");
        let manifest = kura
            .commit_manifest(HEIGHT)
            .expect("read commit manifest")
            .expect("commit manifest exists");
        let checkpoint_hash = kura
            .wsv_checkpoint(HEIGHT)
            .expect("read WSV checkpoint")
            .expect("WSV checkpoint exists")
            .state_hash();
        let expected_snapshot =
            crate::snapshot::canonical_state_snapshot_bytes_for_tests(state.as_ref());

        Self {
            chain_id,
            genesis_account,
            genesis_key,
            keys,
            context,
            block,
            artifact,
            manifest,
            checkpoint_hash,
            expected_snapshot,
            kura,
            materialized_state: state,
            apply_service: service,
        }
    }

    fn into_two_block(self) -> TwoBlockReplayFixture {
        let second_context = wire::HeightContext {
            chain_id: self.chain_id.clone(),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 2,
            epoch: 0,
            epoch_end_height: u64::MAX,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: Some(self.artifact.commit_qc.clone()),
            snapshot_bootstrap: None,
            quorum: self.context.quorum,
            roster: self.context.roster.clone(),
            nexus_amx_context_hash: crate::sumeragi::v2_recovery::committed_nexus_amx_context_hash(
                self.materialized_state.as_ref(),
            ),
            da_layout: self.context.da_layout,
            leader_seed: [0; 32],
        };
        second_context
            .validate()
            .expect("validate height-two fixture context");
        let second_leader = second_context.leader(0);
        let second_leader_index =
            usize::try_from(second_leader).expect("height-two leader index fits usize");

        let creation_time_ms = (self.block.header().creation_time() + Duration::from_secs(1))
            .as_millis()
            .try_into()
            .expect("height-two creation time fits u64");
        let mut header = BlockHeader::new(
            NonZeroU64::new(2).expect("non-zero height"),
            Some(self.block.hash()),
            None,
            None,
            creation_time_ms,
            0,
        );
        let confidential_features = {
            let view = self.materialized_state.view();
            let digest = crate::state::compute_confidential_feature_digest(
                view.world(),
                &view.zk,
                view.sccp_registry.as_ref(),
                2,
            );
            (!digest.is_empty()).then_some(digest)
        };
        header.set_confidential_features(confidential_features);
        let mut builder = iroha_data_model::block::builder::BlockBuilder::new(header);
        builder.set_da_proof_policies(Some(crate::da::active_proof_policy_bundle_at_height(
            &self.materialized_state.nexus_snapshot(),
            2,
        )));
        let body = builder
            .try_build_with_signature(
                u64::from(second_leader),
                self.keys[second_leader_index].private_key(),
            )
            .expect("sign canonical height-two heartbeat")
            .canonical_resultless_proposal();
        let canonical_wire = body
            .encode_wire()
            .expect("encode canonical height-two heartbeat");
        let subject = wire::BlockSubject {
            parent_block_hash: Some(self.block.hash()),
            block_hash: body.hash(),
            payload_hash: Hash::new(&canonical_wire),
        };
        let round = wire::ConsensusRound {
            context_id: second_context.id(),
            height: 2,
            view: 0,
        };
        let payload_manifest = wire::PayloadManifest::derive(
            &second_context,
            round,
            subject,
            u64::try_from(canonical_wire.len()).expect("height-two body size fits u64"),
            std::slice::from_ref(&canonical_wire),
        )
        .expect("derive height-two payload manifest");
        let execution_commitment = self
            .apply_service
            .validate_candidate(&second_context, &body)
            .expect("derive height-two execution commitment");
        let mut certificate = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: Vec::new(),
        };
        Self::resign_certificate(&mut certificate, &self.keys);

        let body_root = tempfile::tempdir().expect("create height-two exact-body store");
        let mut body_store = V2BodyStore::open_with_policy(
            body_root.path(),
            second_context.clone(),
            BlockSignaturePolicy::RotatingLeader,
        )
        .expect("open height-two exact-body store");
        let durable = body_store
            .store(payload_manifest, canonical_wire)
            .expect("persist exact height-two body");
        let validated = body_store
            .validate(&durable, |candidate| {
                self.apply_service
                    .validate_candidate(&second_context, candidate)
            })
            .expect("persist height-two validation receipt");
        let task = ApplyTask::for_test(
            2,
            EventTag::new(2, 0, Generation::new(1)),
            subject,
            certificate,
            validated,
        );
        let _completion = self
            .apply_service
            .execute(&second_context, &mut body_store, &task)
            .expect("materialize exact height-two durable tuple");
        let second_block = self
            .kura
            .get_block(NonZeroUsize::new(2).expect("non-zero height"))
            .expect("read committed height-two block")
            .as_ref()
            .clone();
        let second_artifact = self
            .kura
            .v2_finality_artifact(2)
            .expect("read height-two finality")
            .expect("height-two finality exists");
        let second_checkpoint_hash = self
            .kura
            .wsv_checkpoint(2)
            .expect("read height-two checkpoint")
            .expect("height-two checkpoint exists")
            .state_hash();
        TwoBlockReplayFixture {
            first: self,
            second_context,
            second_block,
            second_artifact,
            second_checkpoint_hash,
        }
    }

    fn new_state(kura: Arc<Kura>, chain_id: ChainId, genesis_account: AccountId) -> State {
        let genesis_domain =
            Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone()).build(&genesis_account);
        let account = Account::new(genesis_account.clone()).build(&genesis_account);
        let state = State::new_with_chain_for_testing(
            World::with([genesis_domain], [account], []),
            kura,
            LiveQueryStore::start_test(),
            chain_id,
        );
        let nexus = state.nexus_snapshot();
        let lane_manifests =
            Arc::new(LaneManifestRegistry::empty().rebind(&nexus.lane_catalog, &nexus.governance));
        state.install_lane_manifests(&lane_manifests);
        {
            let mut parameters = state.world.parameters.block();
            parameters.sumeragi.block_cadence_ms =
                NonZeroU64::new(1_000).expect("fixture cadence is non-zero");
            parameters.sumeragi.key_require_hsm = false;
            parameters.commit();
        }
        state
    }

    fn replay_state(&self, kura: Arc<Kura>) -> State {
        Self::new_state(kura, self.chain_id.clone(), self.genesis_account.clone())
    }

    fn resign_certificate(certificate: &mut wire::QuorumCertificate, keys: &[KeyPair]) {
        let signatures = certificate
            .signers
            .iter()
            .map(|index| {
                let preimage = wire::Vote {
                    round: certificate.round,
                    proposal_round: certificate.proposal_round,
                    phase: certificate.phase,
                    subject: certificate.subject,
                    execution_commitment: certificate.execution_commitment,
                    signer: *index,
                    signature: Vec::new(),
                }
                .signature_preimage();
                Signature::try_new(
                    keys[usize::try_from(*index).expect("signer index fits usize")].private_key(),
                    &preimage,
                )
                .expect("sign Commit vote")
                .payload()
                .to_vec()
            })
            .collect::<Vec<_>>();
        certificate.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
            &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        )
        .expect("aggregate Commit votes");
    }

    fn exact_kura_copy(&self) -> Arc<Kura> {
        self.kura_with_block_and_artifact(self.block.clone(), self.artifact.clone())
    }

    fn kura_with_block_and_artifact(
        &self,
        block: SignedBlock,
        artifact: wire::finality::V2FinalityArtifact,
    ) -> Arc<Kura> {
        let kura = Kura::blank_kura_for_testing();
        kura.store_block(Arc::new(block.clone()))
            .expect("store forked canonical block");
        kura.store_wsv_checkpoint(HEIGHT, block.hash(), self.checkpoint_hash)
            .expect("store forked checkpoint");
        let manifest =
            CommitManifest::new(HEIGHT, block.hash(), None, None, self.checkpoint_hash, None)
                .with_authenticated_v2_commit_authority(&artifact);
        kura.store_commit_manifest(manifest)
            .expect("store forked manifest");
        let _receipt = kura
            .store_v2_finality_artifact(&artifact)
            .expect("store forked finality artifact");
        kura
    }

    fn fork_with_signature(&self, index: u64, private_key: &iroha_crypto::PrivateKey) -> Arc<Kura> {
        let mut block = self.block.clone();
        let signature = BlockSignature::new(
            index,
            SignatureOf::try_from_hash(private_key, block.header().hash())
                .expect("sign forked block header"),
        );
        block
            .replace_signatures(BTreeSet::from([signature]))
            .expect("replace forked signature set");
        let mut artifact = self.artifact.clone();
        artifact.subject.payload_hash = block
            .canonical_proposal_wire_hash()
            .expect("encode forked canonical proposal");
        artifact.commit_qc.subject = artifact.subject;
        artifact
            .commit_qc
            .execution_commitment
            .executed_block_wire_hash = block
            .executed_block_wire_hash()
            .expect("encode forked executed block");
        Self::resign_certificate(&mut artifact.commit_qc, &self.keys);
        self.kura_with_block_and_artifact(block, artifact)
    }

    fn fork_with_malformed_sccp_root(&self) -> Arc<Kura> {
        let mut block = self.block.clone();
        block.set_sccp_commitment_root(Some([0xA7; 32]));
        let signature = BlockSignature::new(
            0,
            SignatureOf::try_from_hash(self.genesis_key.private_key(), block.header().hash())
                .expect("sign malformed-SCCP block header"),
        );
        block
            .replace_signatures(BTreeSet::from([signature]))
            .expect("replace malformed-SCCP block signature");

        let mut artifact = self.artifact.clone();
        artifact.block_hash = block.hash();
        artifact.subject.block_hash = block.hash();
        artifact.subject.payload_hash = block
            .canonical_proposal_wire_hash()
            .expect("encode malformed-SCCP proposal");
        artifact.commit_qc.subject = artifact.subject;
        artifact
            .commit_qc
            .execution_commitment
            .executed_block_wire_hash = block
            .executed_block_wire_hash()
            .expect("encode malformed-SCCP executed block");
        Self::resign_certificate(&mut artifact.commit_qc, &self.keys);

        // Production finality publication intentionally rejects this tuple while preparing the
        // retained archive. Install the mutually correlated bytes through test-only corruption
        // hooks so strict replay, rather than the writer, remains the component under test.
        let kura = Kura::blank_kura_for_testing();
        kura.store_block(Arc::new(block.clone()))
            .expect("store malformed-SCCP canonical block");
        kura.store_wsv_checkpoint(HEIGHT, block.hash(), self.checkpoint_hash)
            .expect("store malformed-SCCP checkpoint");
        let manifest =
            CommitManifest::new(HEIGHT, block.hash(), None, None, self.checkpoint_hash, None)
                .with_authenticated_v2_commit_authority(&artifact);
        kura.store_commit_manifest(manifest)
            .expect("store malformed-SCCP manifest");

        let blocks_dir = RuntimeLaneConfig::default()
            .primary()
            .blocks_dir(&kura.store_root());
        let retained_dir = blocks_dir.join("retained_blocks");
        std::fs::create_dir_all(&retained_dir).expect("create retained-block directory");
        let retained = CorruptedKuraRetainedBlockRecord {
            format_version: 2,
            height: HEIGHT,
            block_hash: block.hash(),
            block_header: block.header(),
            proposal_wire_hash: block
                .canonical_proposal_wire_hash()
                .expect("encode malformed-SCCP proposal"),
            executed_block_wire_hash: block
                .executed_block_wire_hash()
                .expect("encode malformed-SCCP executed block"),
            sccp_archive: Vec::new(),
        };
        std::fs::write(
            retained_dir.join(format!("{HEIGHT:020}.norito")),
            retained.encode(),
        )
        .expect("install malformed retained SCCP archive");

        let finality_dir = blocks_dir.join("v2_finality");
        std::fs::create_dir_all(&finality_dir).expect("create v2-finality directory");
        let finality = CorruptedKuraV2FinalityRecord {
            format_version: 2,
            block_header: block.header(),
            artifact,
        };
        kura.overwrite_v2_finality_bytes_for_tests(HEIGHT, &finality.encode())
            .expect("install malformed-SCCP finality envelope");
        kura
    }

    fn overwrite_correlated_artifact(
        &self,
        kura: &Kura,
        artifact: wire::finality::V2FinalityArtifact,
    ) {
        let manifest = CommitManifest::new(
            HEIGHT,
            self.block.hash(),
            None,
            None,
            self.checkpoint_hash,
            None,
        )
        .with_authenticated_v2_commit_authority(&artifact);
        kura.overwrite_commit_manifest_without_binding_for_tests(&manifest)
            .expect("overwrite correlated manifest");
        kura.overwrite_wsv_checkpoint_without_validation_for_tests(
            HEIGHT,
            self.checkpoint_hash,
            Some(&manifest),
        )
        .expect("overwrite correlated checkpoint binding");
        kura.overwrite_v2_finality_without_validation_for_tests(HEIGHT, artifact)
            .expect("overwrite correlated finality artifact");
    }

    fn assert_rejected_without_mutation(&self, kura: Arc<Kura>, expected_error: &str) {
        let mut replay_state = self.replay_state(Arc::clone(&kura));
        let before = StateFingerprint::capture(&replay_state);
        let error = super::replay_blocks_from_kura_range(&kura, &mut replay_state, 1, 1)
            .expect_err("strict replay must reject the corrupted tuple");
        let diagnostic = format!("{error:?}");
        assert!(
            diagnostic.contains(expected_error),
            "unexpected replay error: {diagnostic}; expected fragment: {expected_error}"
        );
        before.assert_unchanged(&replay_state);
    }
}

macro_rules! strict_replay_test {
    ($name:ident, $body:block) => {
        #[test]
        fn $name() {
            let handle = crate::sumeragi::sumeragi_thread_builder(concat!(
                "strict-production-replay-",
                stringify!($name)
            ))
            .spawn(move || $body)
            .expect("spawn strict replay test on a consensus-sized stack");
            if let Err(payload) = handle.join() {
                std::panic::resume_unwind(payload);
            }
        }
    };
}

strict_replay_test!(production_replay_accepts_the_exact_durable_v2_tuple, {
    let fixture = StrictReplayFixture::new();
    let mut replay_state = fixture.replay_state(Arc::clone(&fixture.kura));
    super::replay_blocks_from_kura_range(&fixture.kura, &mut replay_state, 1, 1)
        .expect("the exact production tuple replays");

    assert_eq!(replay_state.committed_height(), 1);
    assert_eq!(
        replay_state.latest_block_hash_fast(),
        Some(fixture.block.hash())
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_bytes_for_tests(&replay_state),
        fixture.expected_snapshot,
        "replay must reconstruct the exact committed WSV"
    );
    assert_eq!(
        RuntimeStateFingerprint::capture(&replay_state),
        RuntimeStateFingerprint::capture(fixture.materialized_state.as_ref()),
        "receipt publication must reproduce ordinary sequential runtime indexes"
    );
    let durable = fixture
        .kura
        .get_block(NonZeroUsize::new(1).expect("non-zero height"))
        .expect("read durable block");
    assert!(
        durable.has_results(),
        "the durable fixture must carry execution results"
    );
    assert!(
        durable
            .committed_fragment_count()
            .is_some_and(|count| count > 0),
        "the durable fixture must exercise a non-zero committed fragment count"
    );
    let proposal = durable.canonical_resultless_proposal();
    assert!(proposal.is_resultless_proposal());
    assert_eq!(proposal.hash(), durable.hash());
    assert_eq!(proposal.committed_fragment_count(), None);
    assert_eq!(
        durable.encode_wire().expect("encode durable block"),
        fixture.block.encode_wire().expect("encode fixture block")
    );
    assert_eq!(
        fixture.artifact.subject.payload_hash,
        durable
            .canonical_proposal_wire_hash()
            .expect("encode durable proposal block")
    );
    assert_eq!(
        fixture
            .artifact
            .commit_qc
            .execution_commitment
            .executed_block_wire_hash,
        durable
            .executed_block_wire_hash()
            .expect("encode durable executed block")
    );
    assert!(
        fixture
            .manifest
            .binds_authenticated_v2_commit_authority(&fixture.artifact)
    );
    assert_eq!(
        fixture
            .kura
            .commit_manifest_binding_state(&fixture.manifest)
            .expect("verify manifest binding"),
        CommitManifestBindingState::Bound
    );
    assert!(
        replay_state
            .world_view()
            .commit_qcs()
            .get(&fixture.block.hash())
            .is_none(),
        "exact v2 replay must not populate the legacy WSV commit-QC archive"
    );
    assert!(
        replay_state
            .commit_roster_snapshot_for_block(HEIGHT, fixture.block.hash())
            .is_none(),
        "exact v2 replay must not populate the legacy commit-roster journal"
    );
    assert!(
        fixture.kura.read_roster_metadata(HEIGHT).is_none(),
        "exact v2 replay must not require or synthesize a legacy roster sidecar"
    );
});

strict_replay_test!(
    production_replay_consumes_preinstalled_lane_manifest_snapshot,
    {
        let fixture = StrictReplayFixture::new();
        let mut replay_state = fixture.replay_state(Arc::clone(&fixture.kura));
        replay_state.install_lane_manifests(&Arc::new(LaneManifestRegistry::empty()));
        let before = StateFingerprint::capture(&replay_state);

        let error = super::replay_blocks_from_kura_range(&fixture.kura, &mut replay_state, 1, 1)
            .expect_err("replay must reject a durable block when its lane is absent");
        let diagnostic = format!("{error:?}");
        assert!(
            diagnostic.contains("first transaction error: tx#0"),
            "replay rejection must identify the first failed transaction: {diagnostic}"
        );
        assert!(
            diagnostic.contains("lane 0 is absent from the installed manifest registry snapshot"),
            "replay rejection must expose the missing registry binding: {diagnostic}"
        );
        before.assert_unchanged(&replay_state);

        let nexus = replay_state.nexus_snapshot();
        let frozen =
            Arc::new(LaneManifestRegistry::empty().rebind(&nexus.lane_catalog, &nexus.governance));
        replay_state.install_lane_manifests(&frozen);
        super::replay_blocks_from_kura_range(&fixture.kura, &mut replay_state, 1, 1)
            .expect("the identical durable block replays after the lane snapshot is installed");

        assert_eq!(replay_state.committed_height(), 1);
        assert_eq!(
            replay_state.latest_block_hash_fast(),
            Some(fixture.block.hash())
        );
    }
);

strict_replay_test!(
    production_replay_rejects_missing_and_mismatched_sidecars_atomically,
    {
        let fixture = StrictReplayFixture::new();

        let kura = fixture.exact_kura_copy();
        kura.remove_wsv_checkpoint_without_binding_for_tests(HEIGHT)
            .expect("remove checkpoint");
        fixture.assert_rejected_without_mutation(kura, "missing WSV checkpoint");

        let kura = fixture.exact_kura_copy();
        let forged_checkpoint = Hash::new(b"forged strict replay WSV");
        let forged_manifest = CommitManifest::new(
            HEIGHT,
            fixture.block.hash(),
            None,
            None,
            forged_checkpoint,
            None,
        )
        .with_authenticated_v2_commit_authority(&fixture.artifact);
        kura.overwrite_commit_manifest_without_binding_for_tests(&forged_manifest)
            .expect("forge correlated checkpoint manifest");
        kura.overwrite_wsv_checkpoint_without_validation_for_tests(
            HEIGHT,
            forged_checkpoint,
            Some(&forged_manifest),
        )
        .expect("forge checkpoint state hash");
        fixture.assert_rejected_without_mutation(kura, "WSV checkpoint mismatch");

        let kura = fixture.exact_kura_copy();
        kura.remove_commit_manifest_without_binding_for_tests(HEIGHT)
            .expect("remove commit manifest");
        fixture.assert_rejected_without_mutation(kura, "manifest is missing");

        let kura = fixture.exact_kura_copy();
        let mismatched_manifest = CommitManifest::new(
            HEIGHT,
            fixture.block.hash(),
            None,
            None,
            Hash::new(b"forged manifest checkpoint"),
            None,
        )
        .with_authenticated_v2_commit_authority(&fixture.artifact);
        kura.overwrite_commit_manifest_without_binding_for_tests(&mismatched_manifest)
            .expect("overwrite commit manifest");
        fixture
            .assert_rejected_without_mutation(kura, "commit manifest WSV checkpoint hash mismatch");

        let kura = fixture.exact_kura_copy();
        kura.remove_v2_finality_without_binding_for_tests(HEIGHT)
            .expect("remove finality artifact");
        fixture.assert_rejected_without_mutation(kura, "missing verified v2 finality artifact");

        let kura = fixture.exact_kura_copy();
        let mut mismatched_finality = fixture.artifact.clone();
        mismatched_finality.block_hash =
            iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"another canonical block"));
        kura.overwrite_v2_finality_without_validation_for_tests(HEIGHT, mismatched_finality)
            .expect("overwrite mismatched finality");
        fixture.assert_rejected_without_mutation(kura, "failed to verify v2 finality");
    }
);

strict_replay_test!(
    production_replay_rejects_correlated_finality_forgeries_atomically,
    {
        let fixture = StrictReplayFixture::new();

        let kura = fixture.exact_kura_copy();
        let mut missing_pop = fixture.artifact.clone();
        missing_pop.validator_set_pops[0].clear();
        kura.overwrite_v2_finality_without_validation_for_tests(HEIGHT, missing_pop)
            .expect("overwrite missing PoP");
        fixture.assert_rejected_without_mutation(kura, "failed to verify v2 finality");

        let kura = fixture.exact_kura_copy();
        let mut mismatched_pop = fixture.artifact.clone();
        mismatched_pop.validator_set_pops.swap(0, 1);
        kura.overwrite_v2_finality_without_validation_for_tests(HEIGHT, mismatched_pop)
            .expect("overwrite mismatched PoP");
        fixture.assert_rejected_without_mutation(kura, "failed to verify v2 finality");

        let kura = fixture.exact_kura_copy();
        let mut duplicate_signers = fixture.artifact.clone();
        duplicate_signers.commit_qc.signers = vec![0, 0, 2];
        kura.overwrite_v2_finality_without_validation_for_tests(HEIGHT, duplicate_signers)
            .expect("overwrite duplicate certificate signer");
        fixture.assert_rejected_without_mutation(kura, "failed to verify v2 finality");

        let kura = fixture.exact_kura_copy();
        let mut wrong_wire = fixture.artifact.clone();
        wrong_wire.subject.payload_hash = Hash::new(b"forged canonical SignedBlockWire");
        wrong_wire.commit_qc.subject = wrong_wire.subject;
        StrictReplayFixture::resign_certificate(&mut wrong_wire.commit_qc, &fixture.keys);
        fixture.overwrite_correlated_artifact(kura.as_ref(), wrong_wire);
        fixture.assert_rejected_without_mutation(kura, "canonical proposal wire image");

        let kura = fixture.exact_kura_copy();
        let mut wrong_executed_wire = fixture.artifact.clone();
        wrong_executed_wire
            .commit_qc
            .execution_commitment
            .executed_block_wire_hash = Hash::new(b"forged executed SignedBlockWire");
        StrictReplayFixture::resign_certificate(&mut wrong_executed_wire.commit_qc, &fixture.keys);
        fixture.overwrite_correlated_artifact(kura.as_ref(), wrong_executed_wire);
        fixture.assert_rejected_without_mutation(kura, "executed block wire image");

        let kura = fixture.exact_kura_copy();
        let mut wrong_execution = fixture.artifact.clone();
        wrong_execution
            .commit_qc
            .execution_commitment
            .parent_state_root = Hash::new(b"forged but structurally valid execution commitment");
        StrictReplayFixture::resign_certificate(&mut wrong_execution.commit_qc, &fixture.keys);
        fixture.overwrite_correlated_artifact(kura.as_ref(), wrong_execution);
        fixture.assert_rejected_without_mutation(kura, "execution commitment differs");
    }
);

strict_replay_test!(
    production_replay_rejects_bad_block_signer_identity_atomically,
    {
        let fixture = StrictReplayFixture::new();

        let out_of_range = fixture.fork_with_signature(
            u64::try_from(fixture.context.roster.len()).expect("roster length fits u64"),
            fixture.keys[0].private_key(),
        );
        fixture.assert_rejected_without_mutation(out_of_range, "signatures");

        let rogue = KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::BlsNormal)
            .expect("derive rogue signer");
        let wrong_key = fixture.fork_with_signature(0, rogue.private_key());
        fixture.assert_rejected_without_mutation(wrong_key, "signatures");
    }
);

strict_replay_test!(
    production_replay_returns_error_for_malformed_sccp_root_without_panicking,
    {
        let fixture = StrictReplayFixture::new();
        let malformed = fixture.fork_with_malformed_sccp_root();
        fixture.assert_rejected_without_mutation(malformed, "SCCP");
    }
);

strict_replay_test!(
    production_replay_range_is_atomic_when_height_two_fails_late,
    {
        let fixture = StrictReplayFixture::new().into_two_block();
        assert_eq!(fixture.second_context.height, 2);
        let forged_checkpoint = Hash::new(b"late height-two replay failure");
        assert_ne!(forged_checkpoint, fixture.second_checkpoint_hash);
        let forged_manifest = CommitManifest::new(
            2,
            fixture.second_block.hash(),
            None,
            None,
            forged_checkpoint,
            None,
        )
        .with_authenticated_v2_commit_authority(&fixture.second_artifact);
        fixture
            .first
            .kura
            .overwrite_commit_manifest_without_binding_for_tests(&forged_manifest)
            .expect("forge height-two manifest");
        fixture
            .first
            .kura
            .overwrite_wsv_checkpoint_without_validation_for_tests(
                2,
                forged_checkpoint,
                Some(&forged_manifest),
            )
            .expect("forge correlated height-two checkpoint");
        let mut replay_state = fixture.first.replay_state(Arc::clone(&fixture.first.kura));
        let compliance_engine = Arc::new(
            crate::compliance::LaneComplianceEngine::from_policies(Vec::new(), true)
                .expect("construct non-empty replay compliance handle"),
        );
        replay_state.install_lane_compliance_engine(Some(Arc::clone(&compliance_engine)));
        seed_recovery_candidates_for_read_only_prevalidation(fixture.first.kura.as_ref());
        let kura_before = kura_tree_fingerprint(fixture.first.kura.as_ref());
        let ivm_cache_before = ivm::ivm_cache::cache_limits();
        let prover_threads_before = ivm::zk::prover_threads();
        let tiered_worker_spawns_before =
            super::TIERED_SNAPSHOT_WORKER_SPAWNS.with(std::cell::Cell::get);
        #[cfg(feature = "sm")]
        let sm2_distid_before = iroha_crypto::sm::Sm2PublicKey::default_distid();

        let isolated_probe =
            super::isolated_state_for_replay_prevalidation(&replay_state, &fixture.first.kura)
                .expect("construct read-only atomic replay State");
        assert!(
            isolated_probe
                .query_handle
                .shares_store_with(&replay_state.query_handle),
            "replay probe must reuse the inert live query handle instead of spawning a service"
        );
        assert!(
            isolated_probe
                .pipeline_parallelism
                .shares_pool_with(&replay_state.pipeline_parallelism),
            "replay probe must share the live pipeline pool instead of spawning Rayon workers"
        );
        let isolated_compliance = isolated_probe
            .lane_compliance_engine()
            .expect("isolated replay keeps configured compliance");
        assert!(
            Arc::ptr_eq(&isolated_compliance, &compliance_engine),
            "replay probe must preserve the exact consensus-critical compliance engine"
        );
        assert_eq!(
            ivm::ivm_cache::cache_limits(),
            ivm_cache_before,
            "replay probe construction must not reconfigure the global IVM cache"
        );
        assert_eq!(
            ivm::zk::prover_threads(),
            prover_threads_before,
            "replay probe construction must not reconfigure prover workers"
        );
        assert_eq!(
            super::TIERED_SNAPSHOT_WORKER_SPAWNS.with(std::cell::Cell::get),
            tiered_worker_spawns_before,
            "replay probe construction must not spawn a tiered snapshot worker"
        );
        #[cfg(feature = "sm")]
        assert_eq!(
            iroha_crypto::sm::Sm2PublicKey::default_distid(),
            sm2_distid_before,
            "replay probe construction must not change the process-wide SM2 distid"
        );
        assert!(
            !Arc::ptr_eq(
                &replay_state.commit_roster_journal,
                &isolated_probe.commit_roster_journal,
            ),
            "atomic replay must not share the live commit-roster cache"
        );
        assert_eq!(
            isolated_probe.commit_roster_journal.read().snapshots(),
            replay_state.commit_roster_journal.read().snapshots(),
            "isolated replay must start from an exact commit-roster snapshot"
        );
        assert_eq!(
            kura_tree_fingerprint(fixture.first.kura.as_ref()),
            kura_before,
            "constructing isolated replay State must not recover or mutate Kura"
        );
        drop(isolated_probe);

        let before = StateFingerprint::capture(&replay_state);
        let error =
            super::replay_blocks_from_kura_range(&fixture.first.kura, &mut replay_state, 1, 2)
                .expect_err("late height-two corruption must reject the complete replay range");
        let diagnostic = format!("{error:?}");
        assert!(
            diagnostic.contains("block #2 WSV checkpoint mismatch"),
            "unexpected late replay failure: {diagnostic}"
        );
        before.assert_unchanged(&replay_state);
        assert_eq!(
            kura_tree_fingerprint(fixture.first.kura.as_ref()),
            kura_before,
            "atomic dry-run rejection must not promote temp journals, recover merge tails, or mutate any Kura byte"
        );
    }
);
