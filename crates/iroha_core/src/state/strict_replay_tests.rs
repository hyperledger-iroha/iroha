//! Production-path tests for strict Sumeragi-v2 Kura replay.
use super::{QueryIndexJournal, QueryProjectionCheckpointJournal, State, World, WorldReadOnly};
use crate::sumeragi::v2_core::{EventTag, Generation};
use crate::{
    governance::manifest::LaneManifestRegistry,
    kura::{CommitManifest, CommitManifestBindingState, Kura},
    query::store::LiveQueryStore,
    queue::Queue,
    sumeragi::{
        v2_apply::V2ApplyService,
        v2_body_store::{BlockSignaturePolicy, V2BodyStore},
        v2_chunks::encode_payload,
        v2_effects::ApplyTask,
    },
};
use iroha_config::parameters::actual::{LaneConfig as RuntimeLaneConfig, Queue as QueueConfig};
use iroha_crypto::{Algorithm, Hash, KeyPair, Signature, SignatureOf};
use iroha_data_model::{
    ChainId, HasMetadata, Registrable,
    account::{Account, AccountId},
    block::{
        BlockHeader, BlockSignature, CertifiedMergeLedgerReference, SignedBlock,
        consensus_v2 as wire, consensus_v2::finality::V2FinalityArtifact,
    },
    bridge::SccpOutboundMessageContextV1,
    domain::Domain,
    parameter::{Parameter, system::SumeragiParameter},
    peer::PeerId,
};
use iroha_primitives::time::TimeSource;
use norito::codec::Encode;
use std::{
    collections::BTreeSet,
    num::{NonZeroU64, NonZeroUsize},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
const HEIGHT: u64 = 1;
/// Test-only mirror of Kura's private retained SCCP message layout.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::state::strict_replay_tests::CorruptedKuraRetainedSccpMessage")]
#[derive(Clone, Debug, PartialEq, Eq, Encode)]
#[norito(deny_unknown_fields)]
struct CorruptedKuraRetainedSccpMessage {
    commitment_index: u32,
    context: SccpOutboundMessageContextV1,
    payload_bytes: Vec<u8>,
}
/// Test-only mirror used to install a disk-corrupted retained record.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::state::strict_replay_tests::CorruptedKuraRetainedBlockRecord")]
#[derive(Clone, Debug, PartialEq, Eq, Encode)]
#[norito(deny_unknown_fields)]
struct CorruptedKuraRetainedBlockRecord {
    format_version: u16,
    height: u64,
    block_hash: iroha_crypto::HashOf<BlockHeader>,
    block_header: BlockHeader,
    proposal_wire_hash: Hash,
    executed_block_wire_len: u64,
    executed_block_wire_hash: Hash,
    merge_reference: Option<CertifiedMergeLedgerReference>,
    sccp_archive: Vec<CorruptedKuraRetainedSccpMessage>,
}
/// Test-only mirror used to install a disk-corrupted v2 finality envelope.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::state::strict_replay_tests::CorruptedKuraV2FinalityRecord")]
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
    merge_entries: Vec<iroha_data_model::merge::MergeLedgerEntry>,
}
#[derive(Debug, PartialEq, Eq)]
struct RuntimeStateFingerprint {
    merge_entries: Vec<iroha_data_model::merge::MergeLedgerEntry>,
    runtime_debug: String,
}
impl RuntimeStateFingerprint {
    fn capture(state: &State) -> Self {
        Self {
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
#[derive(Clone, Copy)]
struct ReplayFixtureOptions {
    mode: wire::ConsensusMode,
    npos_seed: [u8; 32],
    seed_space_directory: bool,
    install_compliance: bool,
}
impl Default for ReplayFixtureOptions {
    fn default() -> Self {
        Self {
            mode: wire::ConsensusMode::Permissioned,
            npos_seed: [1; 32],
            seed_space_directory: false,
            install_compliance: false,
        }
    }
}
pub(super) struct AppliedReplayBlock {
    pub(super) context: wire::HeightContext,
    pub(super) block: SignedBlock,
    pub(super) artifact: wire::finality::V2FinalityArtifact,
    pub(super) checkpoint_hash: Hash,
}
pub(super) struct StrictReplayFixture {
    chain_id: ChainId,
    pub(super) genesis_account: AccountId,
    genesis_key: KeyPair,
    pub(super) keys: Vec<KeyPair>,
    pub(super) context: wire::HeightContext,
    pub(super) block: SignedBlock,
    pub(super) artifact: wire::finality::V2FinalityArtifact,
    manifest: CommitManifest,
    pub(super) checkpoint_hash: Hash,
    expected_snapshot: Vec<u8>,
    pub(super) kura: Arc<Kura>,
    pub(super) materialized_state: Arc<State>,
    queue: Arc<Queue>,
    options: ReplayFixtureOptions,
    apply_service: V2ApplyService,
}
pub(super) struct TwoBlockReplayFixture {
    pub(super) first: StrictReplayFixture,
    pub(super) second_context: wire::HeightContext,
    pub(super) second_block: SignedBlock,
    pub(super) second_artifact: wire::finality::V2FinalityArtifact,
    pub(super) second_checkpoint_hash: Hash,
}
impl StrictReplayFixture {
    pub(super) fn new() -> Self {
        Self::new_with_options(ReplayFixtureOptions::default(), Vec::new())
    }
    fn new_with_compliance() -> Self {
        Self::new_with_options(
            ReplayFixtureOptions {
                install_compliance: true,
                ..Default::default()
            },
            Vec::new(),
        )
    }
    pub(super) fn new_npos() -> Self {
        Self::new_with_options(
            ReplayFixtureOptions {
                mode: wire::ConsensusMode::Npos,
                ..Default::default()
            },
            Vec::new(),
        )
    }
    pub(super) fn new_with_space_directory() -> Self {
        Self::new_with_options(
            ReplayFixtureOptions {
                seed_space_directory: true,
                ..Default::default()
            },
            Vec::new(),
        )
    }
    pub(super) fn new_with_genesis_instructions(
        instructions: Vec<iroha_data_model::isi::InstructionBox>,
    ) -> Self {
        Self::new_with_options(ReplayFixtureOptions::default(), instructions)
    }
    fn staking_asset_definition() -> iroha_data_model::asset::AssetDefinitionId {
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            iroha_genesis::GENESIS_DOMAIN_ID.clone(),
            "replay_stake".parse().expect("fixture staking asset name"),
        )
    }
    fn new_with_options(
        options: ReplayFixtureOptions,
        instructions: Vec<iroha_data_model::isi::InstructionBox>,
    ) -> Self {
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
        let genesis_key = KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::Ed25519)
            .expect("derive deterministic genesis authority key");
        let genesis_account = AccountId::new(genesis_key.public_key().clone());
        let topology = crate::sumeragi::network_topology::Topology::new(
            roster.iter().map(|entry| entry.validator.clone()),
        );
        let mut genesis_builder =
            iroha_genesis::GenesisBuilder::new_without_executor(chain_id.clone(), ".")
                .with_sumeragi_v2_context_parameters(
                    wire::SumeragiV2GenesisContextParameters::recommended(),
                )
                .with_kagemusha_mint_finality_genesis_parameters(
                    crate::kagemusha_v1_test_fixtures::mint_finality_genesis_parameters(&roster),
                )
                .with_block_cadence_ms(NonZeroU64::new(1_000).expect("non-zero fixture cadence"))
                .set_topology(
                    keys.iter()
                        .map(|key| {
                            iroha_genesis::GenesisTopologyEntry::new(
                                PeerId::new(key.public_key().clone()),
                                iroha_crypto::bls_normal_pop_prove(key.private_key())
                                    .expect("derive validator proof of possession"),
                            )
                        })
                        .collect::<Vec<_>>(),
                )
                .append_parameter(Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(100)));
        if options.mode == wire::ConsensusMode::Npos {
            // NPoS eligibility is executed from signed genesis: actual asset minting,
            // escrow custody, registration and activation replace seeded validator rows.
            use iroha_data_model::asset::{AssetBalancePolicy, AssetDefinition, AssetId};
            use iroha_data_model::isi::{
                ActivatePublicLaneValidator, Mint, Register, RegisterPublicLaneValidator,
            };
            let definition = Self::staking_asset_definition();
            let npos = iroha_data_model::parameter::system::SumeragiNposParameters {
                epoch_seed: options.npos_seed,
                ..Default::default()
            };
            genesis_builder = genesis_builder
                .next_transaction()
                .append_parameter(Parameter::Custom(npos.into_custom_parameter()))
                .append_instruction(Register::asset_definition(AssetDefinition::numeric(
                    definition.clone(),
                    "replay stake".to_owned(),
                    AssetBalancePolicy::Global,
                    None,
                )));
            for entry in &roster {
                let validator = AccountId::new(entry.validator.public_key().clone());
                genesis_builder = genesis_builder
                    .append_instruction(Register::account(Account::new(validator.clone())))
                    .append_instruction(Mint::asset_quantity(
                        1_000_u64,
                        AssetId::of(definition.clone(), validator.clone()),
                    ))
                    .append_instruction(RegisterPublicLaneValidator::new(
                        iroha_data_model::nexus::LaneId::SINGLE,
                        validator.clone(),
                        entry.validator.clone(),
                        validator.clone(),
                        iroha_primitives::numeric::Quantity::from(1_000_u64),
                        iroha_data_model::metadata::Metadata::default(),
                    ))
                    .append_instruction(ActivatePublicLaneValidator::new(
                        iroha_data_model::nexus::LaneId::SINGLE,
                        validator,
                    ));
            }
        }
        if !instructions.is_empty() {
            genesis_builder = genesis_builder.next_transaction();
            for instruction in instructions {
                genesis_builder = genesis_builder.append_instruction(instruction);
            }
        }
        let template = genesis_builder
            .build_raw()
            .expect("build complete strict-replay genesis manifest");
        // As in the production signer, stage a provisional signed genesis to derive
        // its network-independent commitments, then sign the final complete manifest.
        let policy_state = Self::new_state(
            Self::fresh_kura(options),
            chain_id.clone(),
            crate::sumeragi::synthetic_network_id("strict-replay-policy-preview"),
            genesis_account.clone(),
            &roster,
            options,
        );
        let confidential_policy_hash = {
            let view = policy_state.view();
            crate::state::compute_confidential_feature_digest(
                view.world(),
                &view.zk,
                view.sccp_registry.as_ref(),
                HEIGHT,
            )
            .zk_policy_hash
        };
        let policies =
            crate::da::active_proof_policy_bundle_at_height(&policy_state.nexus_snapshot(), HEIGHT);
        let sign = |manifest: iroha_genesis::RawGenesisTransaction| {
            manifest
                .with_consensus_mode(options.mode.into())
                .with_consensus_meta()
                .build_and_sign_with_da_proof_policies_and_confidential_policy_hash_at(
                    &genesis_key,
                    Some(policies.clone()),
                    confidential_policy_hash,
                    1_000,
                )
                .expect("sign complete canonical strict-replay genesis")
        };
        let provisional = sign(template.clone());
        let staging_state = Self::new_state(
            Self::fresh_kura(options),
            chain_id.clone(),
            iroha_data_model::NetworkId::from_genesis_hash(provisional.0.hash()),
            genesis_account.clone(),
            &roster,
            options,
        );
        let parameters = {
            let mut voting_block = None;
            let (_valid, staged) =
                crate::block::ValidBlock::validate_signed_genesis_keep_voting_block(
                    provisional.0,
                    &topology,
                    &genesis_account,
                    &TimeSource::new_system(),
                    &staging_state,
                    &mut voting_block,
                    options.mode,
                )
                .unpack(|_| {})
                .unwrap_or_else(|(_block, error)| {
                    panic!("stage complete strict-replay genesis: {error}")
                });
            let mut parameters = template.sumeragi_v2_context_parameters();
            parameters.nexus_amx_context_hash =
                crate::sumeragi::staged_genesis_nexus_amx_context_hash(&staged).into();
            parameters.execution_policy_hash =
                crate::sumeragi::staged_genesis_execution_policy_hash(&staged)
                    .expect("derive strict-replay genesis execution policy")
                    .into();
            parameters
        };
        let genesis = sign(template.with_sumeragi_v2_context_parameters(parameters));
        let network_id = iroha_data_model::NetworkId::from_genesis_hash(genesis.0.hash());
        let kura = Self::fresh_kura(options);
        let state = Arc::new(Self::new_state(
            Arc::clone(&kura),
            chain_id.clone(),
            network_id,
            genesis_account.clone(),
            &roster,
            options,
        ));
        let bootstrap = {
            let mut voting_block = None;
            let (_valid, staged) =
                crate::block::ValidBlock::validate_signed_genesis_keep_voting_block(
                    genesis.0.clone(),
                    &topology,
                    &genesis_account,
                    &TimeSource::new_system(),
                    state.as_ref(),
                    &mut voting_block,
                    options.mode,
                )
                .unpack(|_| {})
                .unwrap_or_else(|(_block, error)| {
                    panic!("stage final strict-replay genesis: {error}")
                });
            crate::sumeragi::freeze_staged_genesis_v2(&genesis, &staged, options.mode)
                .expect("freeze exact signed and staged strict-replay genesis authority")
        };
        let context = bootstrap.context().clone();
        let pops = bootstrap.proofs_of_possession().to_vec();
        assert_eq!(context.network_id, network_id);
        assert_eq!(context.roster, roster);
        let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(32);
        let queue = Arc::new(Queue::from_config(
            QueueConfig::default(),
            events_sender.clone(),
        ));
        let service = V2ApplyService::new(
            Arc::clone(&state),
            Arc::clone(&queue),
            Arc::clone(&kura),
            None,
            None,
            Duration::from_secs(1),
            genesis_account.clone(),
            events_sender,
            pops,
        );
        let body = genesis.0.canonical_resultless_proposal();
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
        let payload_manifest = encode_payload(&context, round, subject, &canonical_wire)
            .expect("encode exact replay payload")
            .manifest()
            .clone();
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
            queue,
            options,
            apply_service: service,
        }
    }
    pub(super) fn into_two_block(mut self) -> TwoBlockReplayFixture {
        let second = self.append_metadata_block();
        TwoBlockReplayFixture {
            first: self,
            second_context: second.context,
            second_block: second.block,
            second_artifact: second.artifact,
            second_checkpoint_hash: second.checkpoint_hash,
        }
    }
    pub(super) fn successor_context(&self) -> wire::HeightContext {
        let parent_height = self.materialized_state.committed_height();
        let parent = self
            .kura
            .v2_finality_artifact(u64::try_from(parent_height).expect("parent height fits u64"))
            .expect("read exact parent finality")
            .expect("parent finality exists");
        crate::sumeragi::v2_context::build_successor_height_context_from_state(
            &parent,
            &self.materialized_state.view(),
            crate::sumeragi::v2_recovery::committed_nexus_amx_context_hash(
                self.materialized_state.as_ref(),
            ),
        )
        .expect("derive unique successor from committed parent authority")
    }
    pub(super) fn append_metadata_block(&mut self) -> AppliedReplayBlock {
        self.append_metadata_block_at_view(0)
    }
    pub(super) fn append_metadata_block_at_view(&mut self, view: u64) -> AppliedReplayBlock {
        let height = u64::try_from(self.materialized_state.committed_height())
            .expect("fixture height fits u64")
            + 1;
        let parent = self
            .kura
            .get_block(
                NonZeroUsize::new(usize::try_from(height - 1).expect("parent index"))
                    .expect("parent height"),
            )
            .expect("parent block");
        let transaction = iroha_data_model::transaction::TransactionBuilder::new_with_time_source(
            self.context.network_id,
            self.genesis_account.clone(),
            &TimeSource::new_fixed(parent.header().creation_time()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([iroha_data_model::isi::SetKeyValue::domain(
            iroha_genesis::GENESIS_DOMAIN_ID.clone(),
            "strict_replay_effect".parse().expect("effect key"),
            height,
        )])
        .sign(self.genesis_key.private_key());
        let applied = self.append_transaction_at_view(transaction, view);
        let effect_key: iroha_model_base::name::Name =
            "strict_replay_effect".parse().expect("effect key");
        assert_eq!(
            self.materialized_state
                .world_view()
                .domain(&iroha_genesis::GENESIS_DOMAIN_ID)
                .expect("genesis domain")
                .metadata()
                .get(&effect_key),
            Some(&iroha_primitives::json::Json::from(height)),
            "real Apply must retain the intended successor mutation"
        );
        applied
    }
    pub(super) fn append_instructions(
        &mut self,
        authority: &AccountId,
        key: &iroha_crypto::PrivateKey,
        instructions: Vec<iroha_data_model::isi::InstructionBox>,
    ) -> AppliedReplayBlock {
        let parent = self
            .kura
            .get_block(
                NonZeroUsize::new(self.materialized_state.committed_height())
                    .expect("parent height"),
            )
            .expect("parent block");
        let transaction = iroha_data_model::transaction::TransactionBuilder::new_with_time_source(
            self.context.network_id,
            authority.clone(),
            &TimeSource::new_fixed(parent.header().creation_time()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions(instructions)
        .sign(key);
        self.append_transaction_at_view(transaction, 0)
    }
    fn append_transaction_at_view(
        &mut self,
        transaction: iroha_data_model::transaction::SignedTransaction,
        view: u64,
    ) -> AppliedReplayBlock {
        let second_context = self.successor_context();
        let height = second_context.height;
        let parent = self
            .kura
            .get_block(
                NonZeroUsize::new(usize::try_from(height - 1).expect("parent height fits usize"))
                    .expect("parent height is non-zero"),
            )
            .expect("exact parent block exists");
        second_context
            .validate()
            .expect("validate successor fixture context");
        let second_leader = second_context.leader(view);
        let second_leader_index =
            usize::try_from(second_leader).expect("successor leader index fits usize");
        let creation_time_ms = (parent.header().creation_time() + Duration::from_secs(1))
            .as_millis()
            .try_into()
            .expect("successor creation time fits u64");
        let mut header = BlockHeader::new(
            NonZeroU64::new(height).expect("non-zero height"),
            Some(parent.hash()),
            None,
            None,
            creation_time_ms,
            view,
        );
        let confidential_features = {
            let view = self.materialized_state.view();
            let digest = crate::state::compute_confidential_feature_digest(
                view.world(),
                &view.zk,
                view.sccp_registry.as_ref(),
                height,
            );
            (!digest.is_empty()).then_some(digest)
        };
        header.set_confidential_features(confidential_features);
        let accepted = crate::prelude::AcceptedTransaction::new_unchecked(std::borrow::Cow::Owned(
            transaction.clone(),
        ));
        let routing_plan = self
            .queue
            .route_plan_with_state(&accepted, self.materialized_state.as_ref())
            .expect("resolve successor metadata mutation route");
        let route = routing_plan.coordinator_route();
        let entrypoint_hash = Hash::from(accepted.hash_as_entrypoint());
        let lane_plan = crate::sumeragi::lane_planner::prepare_v2_lane_payload_plan(
            self.materialized_state.as_ref(),
            self.kura.as_ref(),
            &second_context,
            view,
            &second_context.roster[second_leader_index].validator,
            std::slice::from_ref(&route),
            std::slice::from_ref(&entrypoint_hash),
        )
        .expect("derive successor lane authority and predecessor");
        assert!(
            lane_plan.unavailable_indices.is_empty(),
            "successor production requires the previous ordinary lane certificate and receipt: {:?}",
            lane_plan.unavailable_indices,
        );
        let lane_proposals = lane_plan.proposals;
        assert_eq!(lane_plan.ownerships.len(), 1);
        let execution_context = iroha_data_model::block::BlockExecutionContextBundle::new(vec![
            crate::queue::execution_context_for_routing_plan(
                transaction.hash_as_entrypoint(),
                &routing_plan,
            ),
        ])
        .with_lane_payload_ownerships(lane_plan.ownerships);
        let mut builder = iroha_data_model::block::builder::BlockBuilder::new(header);
        builder.push_transaction(transaction);
        builder.set_execution_context(Some(execution_context));
        builder.set_da_proof_policies(Some(crate::da::active_proof_policy_bundle_at_height(
            &self.materialized_state.nexus_snapshot(),
            height,
        )));
        let body = builder
            .try_build_with_signature(
                u64::from(second_leader),
                self.keys[second_leader_index].private_key(),
            )
            .expect("sign canonical successor mutation")
            .canonical_resultless_proposal();
        let canonical_wire = body
            .encode_wire()
            .expect("encode canonical successor mutation");
        let subject = wire::BlockSubject {
            parent_block_hash: Some(parent.hash()),
            block_hash: body.hash(),
            payload_hash: Hash::new(&canonical_wire),
        };
        let round = wire::ConsensusRound {
            context_id: second_context.id(),
            height,
            view,
        };
        let payload_manifest = encode_payload(&second_context, round, subject, &canonical_wire)
            .expect("encode exact successor replay payload")
            .manifest()
            .clone();
        let execution_commitment = self
            .apply_service
            .validate_candidate(&second_context, &body)
            .expect("derive successor execution commitment");
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
        let body_root = tempfile::tempdir().expect("create successor exact-body store");
        let mut body_store = V2BodyStore::open_with_policy(
            body_root.path(),
            second_context.clone(),
            BlockSignaturePolicy::RotatingLeader,
        )
        .expect("open successor exact-body store");
        let durable = body_store
            .store(payload_manifest, canonical_wire)
            .expect("persist exact successor body");
        let validated = body_store
            .validate(&durable, |candidate| {
                self.apply_service
                    .validate_candidate(&second_context, candidate)
            })
            .expect("persist successor validation receipt");
        let task = ApplyTask::for_test(
            height,
            EventTag::new(height, view, Generation::new(1)),
            subject,
            certificate,
            validated,
        );
        let _completion = self
            .apply_service
            .execute(&second_context, &mut body_store, &task)
            .expect("materialize exact successor durable tuple");
        let second_block = self
            .kura
            .get_block(
                NonZeroUsize::new(usize::try_from(height).expect("height fits usize"))
                    .expect("non-zero height"),
            )
            .expect("read committed successor block")
            .as_ref()
            .clone();
        let second_artifact = self
            .kura
            .v2_finality_artifact(height)
            .expect("read successor finality")
            .expect("successor finality exists");
        let second_checkpoint_hash = self
            .kura
            .wsv_checkpoint(height)
            .expect("read successor checkpoint")
            .expect("successor checkpoint exists")
            .state_hash();
        // Global Apply writes canonical lane ownership, but the ordinary lane
        // certificates and application receipt are independently durable. Close
        // that real protocol boundary before asking the producer for a successor.
        self.finalize_applied_lane_proposals(&second_block, lane_proposals);
        assert_eq!(second_block.results().len(), 1);
        assert!(second_block.results().all(|result| result.as_ref().is_ok()));
        AppliedReplayBlock {
            context: second_context,
            block: second_block,
            artifact: second_artifact,
            checkpoint_hash: second_checkpoint_hash,
        }
    }
    fn finalize_applied_lane_proposals(
        &self,
        block: &SignedBlock,
        proposals: Vec<iroha_data_model::block::consensus::LaneBlockProposalV1>,
    ) {
        use iroha_data_model::block::consensus::{CertPhase, LaneBlockProposalPayloadHintV1};
        for mut proposal in proposals {
            proposal.payload_block_hint = Some(LaneBlockProposalPayloadHintV1 {
                proposal_height: block.header().height().get(),
                proposal_view: block.header().view_change_index(),
                proposal_block_hash: block.hash(),
            });
            crate::lane_consensus::validate_lane_block_proposal(&proposal)
                .expect("planner produced the exact canonical ordinary lane proposal");
            let committee_keys = proposal
                .descriptor
                .validator_set
                .iter()
                .map(|peer| {
                    self.keys
                        .iter()
                        .find(|key| key.public_key() == peer.public_key())
                        .expect("fixture owns the exact lane committee key")
                })
                .collect::<Vec<_>>();
            let sign_qc = |phase| {
                let body = proposal.vote_body(phase);
                let votes = committee_keys
                    .iter()
                    .map(|key| crate::lane_consensus::LaneBlockVoteV1 {
                        body: body.clone(),
                        signer: PeerId::new(key.public_key().clone()),
                        bls_signature: Signature::try_new(
                            key.private_key(),
                            &body.signature_preimage(),
                        )
                        .expect("sign the applied lane's exact vote body")
                        .payload()
                        .to_vec(),
                        payload_availability_vote: None,
                    })
                    .collect::<Vec<_>>();
                crate::lane_consensus::aggregate_lane_block_votes_to_qc(
                    body,
                    proposal.descriptor.validator_set.clone(),
                    &votes,
                )
                .expect("exact committee votes form an authenticated lane QC")
            };
            let session = crate::lane_consensus::CommittedLaneBlockSession {
                prepare_qc: sign_qc(CertPhase::Prepare),
                commit_qc: sign_qc(CertPhase::Commit),
                proposal,
            };
            // The QC contains its exact quorum subset, even when every committee
            // member voted. Persist PoPs for the union of actual certificate
            // signers, rather than adding non-signing committee members.
            let qc_signer_keys = [&session.prepare_qc, &session.commit_qc]
                .into_iter()
                .flat_map(|qc| {
                    qc.validator_set
                        .iter()
                        .enumerate()
                        .filter_map(|(index, peer)| {
                            (qc.signers_bitmap[index / 8] & (1_u8 << (index % 8)) != 0)
                                .then(|| peer.public_key().clone())
                        })
                })
                .collect::<BTreeSet<_>>();
            let signer_pops = qc_signer_keys
                .iter()
                .map(|public_key| {
                    let key = committee_keys
                        .iter()
                        .find(|key| key.public_key() == public_key)
                        .expect("fixture owns each selected QC signer");
                    (
                        key.public_key().clone(),
                        iroha_crypto::bls_normal_pop_prove(key.private_key())
                            .expect("derive lane signer proof of possession"),
                    )
                })
                .collect();
            self.materialized_state
                .persist_committed_lane_block_session_lifecycle_bound(&session, &signer_pops)
                .expect("persist signed ordinary lane finality under current lifecycle authority");
            self.kura
                .persist_lane_block_application_receipt(&session.proposal)
                .expect("derive application receipt from exact canonical committed results");
            assert!(
                self.materialized_state
                    .certified_lane_block_session_is_applied_or_snapshot_anchored(&session)
                    .expect("authenticate finalized ordinary lane application"),
                "the finalized fixture must authorize its next producer slot",
            );
        }
    }
    fn fresh_kura(options: ReplayFixtureOptions) -> Arc<Kura> {
        if options.mode != wire::ConsensusMode::Npos {
            return Kura::blank_kura_for_testing();
        }
        // The NPoS staking policy is startup configuration. Open its immutable
        // configured catalog before State publishes that configuration.
        let nexus = iroha_config::parameters::actual::Nexus::default();
        let config = iroha_config::parameters::actual::Kura {
            init_mode: iroha_config::kura::InitMode::Strict,
            store_dir: iroha_config::base::WithOrigin::inline(std::path::PathBuf::new()),
            max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: iroha_config::parameters::defaults::kura::BLOCKS_IN_MEMORY,
            lane_history_retention:
                iroha_config::parameters::defaults::kura::LANE_HISTORY_RETENTION,
            fastpq_artifacts: iroha_config::parameters::defaults::kura::FASTPQ_ARTIFACT_POLICY,
            replica_advert: iroha_config::parameters::defaults::kura::REPLICA_ADVERT_POLICY,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity:
                iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Batched,
            fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
        };
        Kura::new_temporary_with_configured_lane_catalog(
            &config,
            &iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog),
            &nexus.lane_catalog,
        )
        .expect("open exact configured NPoS fixture catalog before State startup")
    }
    fn new_state(
        kura: Arc<Kura>,
        chain_id: ChainId,
        network_id: iroha_data_model::NetworkId,
        genesis_account: AccountId,
        roster: &[wire::ValidatorPower],
        options: ReplayFixtureOptions,
    ) -> State {
        let genesis_domain =
            Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone()).build(&genesis_account);
        let account = Account::new(genesis_account.clone()).build(&genesis_account);
        let mut state = State::new_with_chain_and_network_id_for_testing(
            World::with([genesis_domain], [account], []),
            kura,
            LiveQueryStore::start_test(),
            chain_id,
            network_id,
        );
        if options.mode == wire::ConsensusMode::Npos {
            let mut nexus = state.nexus_snapshot();
            nexus.staking.stake_asset_id = Self::staking_asset_definition().to_string();
            nexus.staking.stake_escrow_account_id = genesis_account.to_string();
            nexus.staking.slash_sink_account_id = genesis_account.to_string();
            // Follow the configured-primary startup sequence for fresh signing
            // states and empty-state replay against already populated Kura.
            state
                .prepare_configured_primary_geometry_anchor(&nexus.configured_lane_catalog)
                .expect("authenticate the NPoS configured primary before publication");
            state
                .restore_kura_lane_segments_before_startup_replay()
                .expect("recover the exact configured NPoS lane geometry");
            state
                .set_nexus_from_config(nexus)
                .expect("install the same NPoS startup policy before signing, Apply and replay");
        }
        if options.install_compliance {
            state.install_lane_compliance_engine(Some(Arc::new(
                crate::compliance::LaneComplianceEngine::from_policies(Vec::new(), true)
                    .expect("construct exact fixture compliance policy before genesis signing"),
            )));
        }
        if options.seed_space_directory {
            super::replay_validation_tests::seed_space_directory_manifest_for_retired_checkpoint_test(
                &state, iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
            );
        }
        let nexus = state.nexus_snapshot();
        let mut statuses = LaneManifestRegistry::empty()
            .rebind(&nexus.lane_catalog, &nexus.governance)
            .statuses()
            .into_iter()
            .map(|status| (status.lane, status))
            .collect::<std::collections::BTreeMap<_, _>>();
        let lane = nexus
            .lane_catalog
            .lanes()
            .iter()
            .find(|lane| lane.id == iroha_data_model::nexus::LaneId::SINGLE)
            .expect("strict replay fixture has the primary lane");
        let validators = roster
            .iter()
            .map(|entry| AccountId::new(entry.validator.public_key().clone()))
            .collect::<Vec<_>>();
        let validator_bindings = validators
            .iter()
            .zip(roster)
            .map(
                |(validator, entry)| crate::governance::manifest::ManifestValidatorBinding {
                    validator: validator.clone(),
                    peer_id: entry.validator.clone(),
                    torii_url: None,
                },
            )
            .collect();
        statuses.insert(
            lane.id,
            crate::governance::manifest::LaneManifestStatus {
                lane: lane.id,
                alias: lane.alias.clone(),
                dataspace: lane.dataspace_id,
                visibility: lane.visibility,
                storage: lane.storage.clone(),
                governance: lane.governance.clone(),
                manifest_path: Some(PathBuf::from("/tmp/strict-replay-lane-manifest.json")),
                governance_rules: Some(crate::governance::manifest::GovernanceRules {
                    validators,
                    validator_bindings,
                    ..Default::default()
                }),
                privacy_commitments: Vec::new(),
            },
        );
        state.install_lane_manifests(&Arc::new(LaneManifestRegistry::from_statuses(statuses)));
        {
            let mut parameters = state.world.parameters.block();
            parameters.sumeragi.block_cadence_ms =
                NonZeroU64::new(1_000).expect("fixture cadence is non-zero");
            parameters.commit();
        }
        state
    }
    pub(super) fn replay_state(&self, kura: Arc<Kura>) -> State {
        Self::new_state(
            kura,
            self.chain_id.clone(),
            self.context.network_id.clone(),
            self.genesis_account.clone(),
            &self.context.roster,
            self.options,
        )
    }
    pub(super) fn resign_certificate(certificate: &mut wire::QuorumCertificate, keys: &[KeyPair]) {
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
    pub(super) fn exact_kura_copy(&self) -> Arc<Kura> {
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
    pub(super) fn fork_with_signature(
        &self,
        index: u64,
        private_key: &iroha_crypto::PrivateKey,
    ) -> Arc<Kura> {
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
            .executed_block_wire_len = u64::try_from(
            block
                .encode_wire()
                .expect("encode forked executed block")
                .len(),
        )
        .expect("forked executed block length fits u64");
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
            .executed_block_wire_len = u64::try_from(
            block
                .encode_wire()
                .expect("encode malformed-SCCP executed block")
                .len(),
        )
        .expect("malformed-SCCP executed block length fits u64");
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
            format_version: 3,
            height: HEIGHT,
            block_hash: block.hash(),
            block_header: block.header(),
            proposal_wire_hash: block
                .canonical_proposal_wire_hash()
                .expect("encode malformed-SCCP proposal"),
            executed_block_wire_len: u64::try_from(
                block
                    .encode_wire()
                    .expect("encode malformed-SCCP executed block")
                    .len(),
            )
            .expect("malformed-SCCP executed block length fits u64"),
            executed_block_wire_hash: block
                .executed_block_wire_hash()
                .expect("encode malformed-SCCP executed block"),
            merge_reference: None,
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
            format_version: 3,
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
});
strict_replay_test!(
    production_replay_consumes_preinstalled_lane_manifest_snapshot,
    {
        let fixture = StrictReplayFixture::new();
        let mut replay_state = fixture.replay_state(Arc::clone(&fixture.kura));
        let frozen = replay_state.lane_manifests.read().clone();
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
        let fixture = StrictReplayFixture::new_with_compliance().into_two_block();
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
        let compliance_engine = replay_state
            .lane_compliance_engine()
            .expect("replay uses the same configured compliance policy signed into genesis");
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
