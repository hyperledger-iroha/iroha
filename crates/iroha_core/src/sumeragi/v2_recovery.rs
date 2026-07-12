//! Crash-safe selection of the one active Sumeragi v2 height context.
//!
//! A fresh chain consumes the signed, staged genesis bootstrap. Restart first
//! inspects Kura's immutable finality sidecars: a missing sidecar at the durable
//! tip means application/finality for that exact height must resume, while a
//! present sidecar authorizes construction of exactly one successor context.
//! Context records are persisted before the height WAL is opened.

use std::num::NonZeroUsize;

use iroha_crypto::{Hash, HashOf, PublicKey};
use iroha_data_model::{
    block::{BlockHeader, consensus_v2 as wire},
    nexus::PublicLaneValidatorStatus,
};
use mv::storage::StorageReadOnly;
use thiserror::Error;

use super::{
    v2::{AdapterError, VerifiedHeightContext},
    v2_body_store::BlockSignaturePolicy,
    v2_context::{
        GenesisV2Bootstrap, V2ContextBuildError, build_successor_height_context_from_state,
    },
    v2_context_store::{PersistedHeightContext, V2ContextStore, V2ContextStoreError},
};
use crate::{
    kura::{Kura, KuraV2CommitReceipt},
    state::{State, WorldReadOnly, public_lane_validator_record_matches_key},
};

/// Fully verified active-height inputs selected before network ingress opens.
pub(crate) struct RecoveredV2Height {
    verified_context: VerifiedHeightContext,
    context_store: V2ContextStore,
    signature_policy: BlockSignaturePolicy,
    pending_kura_apply: Option<PendingKuraApply>,
}

/// Canonical Kura tip which WAL/body replay must bind before ingress opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub(crate) struct PendingKuraApply {
    context_id: wire::HeightContextId,
    height: wire::Height,
    block_hash: HashOf<BlockHeader>,
}

impl PendingKuraApply {
    /// Construct a pending-tip expectation for boundary unit tests.
    #[cfg(test)]
    pub(crate) const fn for_test(
        context_id: wire::HeightContextId,
        height: wire::Height,
        block_hash: HashOf<BlockHeader>,
    ) -> Self {
        Self {
            context_id,
            height,
            block_hash,
        }
    }

    /// Frozen context identifier expected from the replayed Decision record.
    pub(crate) const fn context_id(self) -> wire::HeightContextId {
        self.context_id
    }

    /// Interrupted application height.
    pub(crate) const fn height(self) -> wire::Height {
        self.height
    }

    /// Canonical block already durable in Kura.
    pub(crate) const fn block_hash(self) -> HashOf<BlockHeader> {
        self.block_hash
    }
}

impl RecoveredV2Height {
    /// Borrow the exact verified context selected for this process lifetime.
    #[cfg(test)]
    pub(crate) const fn verified_context(&self) -> &VerifiedHeightContext {
        &self.verified_context
    }

    /// Return the Kura tip which reducer/body replay must prove exact before
    /// the caller opens network ingress.
    pub(crate) const fn pending_kura_apply(&self) -> Option<PendingKuraApply> {
        self.pending_kura_apply
    }

    /// Consume recovery output into the height runner's owned parts.
    pub(crate) fn into_parts(
        self,
    ) -> (VerifiedHeightContext, V2ContextStore, BlockSignaturePolicy) {
        (
            self.verified_context,
            self.context_store,
            self.signature_policy,
        )
    }
}

/// Select and verify the only active v2 height after a fresh start or crash.
///
/// The caller must invoke this before opening consensus ingress. A context is
/// never inferred from mutable local configuration: height one comes from
/// signed genesis, and every successor is checked against the durable parent
/// artifact and current finalized state.
pub(crate) fn recover_active_height(
    kura: &Kura,
    state: &State,
    fresh_genesis: Option<GenesisV2Bootstrap>,
    genesis_public_key: PublicKey,
) -> Result<RecoveredV2Height, V2RecoveryError> {
    let storage_root = kura.sumeragi_v2_storage_root();
    let context_store = V2ContextStore::open(&storage_root)?;
    let durable_height = u64::try_from(kura.durable_blocks_count())?;
    let state_height = u64::try_from(state.committed_height())?;

    if durable_height == 0 {
        if state_height != 0 {
            return Err(V2RecoveryError::StateKuraMismatch {
                state_height,
                durable_height,
            });
        }
        let verified_context = fresh_genesis.ok_or(V2RecoveryError::MissingFreshGenesis)?;
        let verified_context = verified_context.into_verified_context();
        context_store.persist(&PersistedHeightContext::from_verified(&verified_context))?;
        return Ok(RecoveredV2Height {
            verified_context,
            context_store,
            signature_policy: BlockSignaturePolicy::GenesisAuthority(genesis_public_key),
            pending_kura_apply: None,
        });
    }

    if state_height > durable_height || durable_height.saturating_sub(state_height) > 1 {
        return Err(V2RecoveryError::StateKuraMismatch {
            state_height,
            durable_height,
        });
    }
    verify_state_kura_prefix(kura, state, state_height)?;

    if let Some((parent_artifact, parent_receipt)) =
        kura.v2_finality_artifact_with_receipt(durable_height)?
    {
        if state_height != durable_height {
            return Err(V2RecoveryError::FinalityAheadOfState {
                finality_height: durable_height,
                state_height,
            });
        }
        let verified_context =
            build_verified_successor(state, &context_store, &parent_artifact, &parent_receipt)?;
        return Ok(RecoveredV2Height {
            verified_context,
            context_store,
            signature_policy: BlockSignaturePolicy::RotatingLeader,
            pending_kura_apply: None,
        });
    }

    // A canonical block without its v2 sidecar is the deliberate crash window
    // between Kura/WSV application and finality-artifact persistence. Resume
    // exactly that height from its already-persisted context and WAL.
    let record = context_store
        .load(durable_height)?
        .ok_or(V2RecoveryError::MissingActiveContext(durable_height))?;
    let verified_context =
        verify_persisted_height(kura, state, &context_store, record, durable_height)?;
    let signature_policy = if durable_height == 1 {
        BlockSignaturePolicy::GenesisAuthority(genesis_public_key)
    } else {
        BlockSignaturePolicy::RotatingLeader
    };
    let durable_index = NonZeroUsize::new(usize::try_from(durable_height)?)
        .ok_or(V2RecoveryError::MissingKuraPrefix(durable_height))?;
    let block_hash = kura
        .get_durable_block_hash(durable_index)
        .ok_or(V2RecoveryError::MissingKuraPrefix(durable_height))?;
    let pending_kura_apply = Some(PendingKuraApply {
        context_id: verified_context.context().id(),
        height: durable_height,
        block_hash,
    });
    Ok(RecoveredV2Height {
        verified_context,
        context_store,
        signature_policy,
        pending_kura_apply,
    })
}

fn verify_state_kura_prefix(
    kura: &Kura,
    state: &State,
    state_height: u64,
) -> Result<(), V2RecoveryError> {
    let Some(nonzero_height) = NonZeroUsize::new(usize::try_from(state_height)?) else {
        return Ok(());
    };
    let state_hash = state
        .committed_block_hashes_snapshot()
        .last()
        .copied()
        .ok_or(V2RecoveryError::MissingStateTip(state_height))?;
    let kura_hash = kura
        .get_durable_block_hash(nonzero_height)
        .ok_or(V2RecoveryError::MissingKuraPrefix(state_height))?;
    if state_hash != kura_hash {
        return Err(V2RecoveryError::StateKuraHashMismatch {
            height: state_height,
            state_hash,
            kura_hash,
        });
    }
    Ok(())
}

/// Build or reopen the unique successor of one just-finalized height and
/// persist its immutable context before its safety WAL is opened.
pub(crate) fn build_verified_successor(
    state: &State,
    context_store: &V2ContextStore,
    parent_artifact: &wire::finality::V2FinalityArtifact,
    parent_receipt: &KuraV2CommitReceipt,
) -> Result<VerifiedHeightContext, V2RecoveryError> {
    let parent_height = parent_artifact.height;
    let parent_record = context_store
        .load(parent_height)?
        .ok_or(V2RecoveryError::MissingParentContext(parent_height))?;
    if parent_record.context() != &parent_artifact.height_context {
        return Err(V2RecoveryError::ParentContextMismatch(parent_height));
    }
    let target_height = parent_height
        .checked_add(1)
        .ok_or(V2RecoveryError::HeightOverflow)?;
    let state_view = state.view();
    let expected = build_successor_height_context_from_state(
        parent_artifact,
        &state_view,
        committed_nexus_amx_context_hash(state),
    )?;
    if expected.height != target_height {
        return Err(V2RecoveryError::HeightOverflow);
    }
    let record = match context_store.load(target_height)? {
        Some(record) => {
            if record.context() != &expected {
                return Err(V2RecoveryError::ConflictingDerivedContext(target_height));
            }
            record
        }
        None => {
            let proofs = successor_proofs_of_possession(parent_artifact);
            let verified = VerifiedHeightContext::successor(
                expected,
                proofs,
                parent_artifact,
                parent_receipt,
                parent_record.proofs_of_possession(),
            )?;
            context_store.persist(&PersistedHeightContext::from_verified(&verified))?;
            return Ok(verified);
        }
    };
    VerifiedHeightContext::successor(
        record.context().clone(),
        record.proofs_of_possession().to_vec(),
        parent_artifact,
        parent_receipt,
        parent_record.proofs_of_possession(),
    )
    .map_err(Into::into)
}

fn verify_persisted_height(
    kura: &Kura,
    state: &State,
    context_store: &V2ContextStore,
    record: PersistedHeightContext,
    height: wire::Height,
) -> Result<VerifiedHeightContext, V2RecoveryError> {
    if height == 1 {
        return VerifiedHeightContext::genesis(
            record.context().clone(),
            record.proofs_of_possession().to_vec(),
        )
        .map_err(Into::into);
    }
    let parent_height = height
        .checked_sub(1)
        .ok_or(V2RecoveryError::HeightOverflow)?;
    let (parent_artifact, parent_receipt) = kura
        .v2_finality_artifact_with_receipt(parent_height)?
        .ok_or(V2RecoveryError::MissingParentFinality(parent_height))?;
    let parent_record = context_store
        .load(parent_height)?
        .ok_or(V2RecoveryError::MissingParentContext(parent_height))?;
    if parent_record.context() != &parent_artifact.height_context {
        return Err(V2RecoveryError::ParentContextMismatch(parent_height));
    }

    // Before state application, the successor projection is still
    // recomputable and must match the immutable record. After state application
    // the record is the only pre-state snapshot; the matching WAL, body marker,
    // and canonical Kura block complete the crash-recovery binding.
    let state_height = u64::try_from(state.committed_height())?;
    if state_height.saturating_add(1) == height {
        let state_view = state.view();
        let expected = build_successor_height_context_from_state(
            &parent_artifact,
            &state_view,
            committed_nexus_amx_context_hash(state),
        )?;
        if record.context() != &expected {
            return Err(V2RecoveryError::ConflictingDerivedContext(height));
        }
    }
    VerifiedHeightContext::successor(
        record.context().clone(),
        record.proofs_of_possession().to_vec(),
        &parent_artifact,
        &parent_receipt,
        parent_record.proofs_of_possession(),
    )
    .map_err(Into::into)
}

fn successor_proofs_of_possession(parent: &wire::finality::V2FinalityArtifact) -> Vec<Vec<u8>> {
    parent
        .height_context
        .next_epoch_snapshot
        .as_ref()
        .map_or_else(
            || parent.validator_set_pops.clone(),
            |snapshot| snapshot.validator_set_pops.clone(),
        )
}

pub(crate) fn committed_nexus_amx_context_hash(state: &State) -> Hash {
    let view = state.view();
    let active_validators = view
        .world()
        .public_lane_validators()
        .iter()
        .filter(|(key, record)| public_lane_validator_record_matches_key(key, record))
        .filter(|(_, record)| matches!(record.status, PublicLaneValidatorStatus::Active))
        .map(|(key, record)| (key.clone(), record.clone()))
        .collect::<Vec<_>>();
    let lane_lifecycle = view
        .nexus
        .lane_catalog
        .lanes()
        .iter()
        .map(
            |lane| iroha_config::parameters::actual::SumeragiV2LaneLifecycleEntry {
                lane_id: lane.id,
                incarnation: *view
                    .lane_incarnations
                    .get(&lane.id)
                    .expect("validated state view has every active lane incarnation"),
                activation_height: *view
                    .lane_incarnation_activation_heights
                    .get(&lane.id)
                    .expect("validated state view has every lane activation height"),
            },
        )
        .collect::<Vec<_>>();
    iroha_config::parameters::actual::sumeragi_v2_nexus_amx_context_hash(
        &view.nexus,
        &view.pipeline,
        &active_validators,
        &lane_lifecycle,
    )
}

/// Fail-closed active-height selection error.
#[derive(Debug, Error)]
pub(crate) enum V2RecoveryError {
    /// Kura operation failed.
    #[error(transparent)]
    Kura(#[from] crate::kura::Error),
    /// Immutable context-store operation failed.
    #[error(transparent)]
    ContextStore(#[from] V2ContextStoreError),
    /// Height context construction failed.
    #[error(transparent)]
    Context(#[from] V2ContextBuildError),
    /// Cryptographic context verification failed.
    #[error(transparent)]
    Adapter(#[from] AdapterError),
    /// Local storage height cannot be represented on the wire.
    #[error(transparent)]
    Integer(#[from] std::num::TryFromIntError),
    /// Empty Kura/WSV startup did not carry the signed genesis bootstrap.
    #[error("fresh Sumeragi v2 storage is missing its signed genesis bootstrap")]
    MissingFreshGenesis,
    /// State and Kura heights cannot arise from one interrupted apply.
    #[error(
        "Sumeragi v2 WSV height {state_height} is inconsistent with Kura height {durable_height}"
    )]
    StateKuraMismatch {
        /// Committed WSV height.
        state_height: u64,
        /// Durable canonical Kura height.
        durable_height: u64,
    },
    /// WSV height is non-zero but its committed hash journal has no tip.
    #[error("Sumeragi v2 WSV hash journal is missing its height {0} tip")]
    MissingStateTip(u64),
    /// Kura does not contain the WSV prefix height despite compatible counts.
    #[error("Sumeragi v2 Kura is missing the WSV prefix at height {0}")]
    MissingKuraPrefix(u64),
    /// WSV and Kura have different canonical hashes at their common prefix.
    #[error(
        "Sumeragi v2 WSV/Kura hash mismatch at height {height}: WSV {state_hash}, Kura {kura_hash}"
    )]
    StateKuraHashMismatch {
        /// Highest height applied to WSV.
        height: u64,
        /// WSV's committed block hash.
        state_hash: iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>,
        /// Kura's durable block hash.
        kura_hash: iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>,
    },
    /// A finality artifact exists for state which was not committed.
    #[error("Sumeragi v2 finality height {finality_height} is ahead of WSV height {state_height}")]
    FinalityAheadOfState {
        /// Durable sidecar height.
        finality_height: u64,
        /// Committed WSV height.
        state_height: u64,
    },
    /// Interrupted active height has no immutable context record.
    #[error("missing Sumeragi v2 active context at height {0}")]
    MissingActiveContext(wire::Height),
    /// Durable parent artifact has no matching immutable context record.
    #[error("missing Sumeragi v2 parent context at height {0}")]
    MissingParentContext(wire::Height),
    /// Interrupted successor lacks its durable parent finality artifact.
    #[error("missing Sumeragi v2 parent finality artifact at height {0}")]
    MissingParentFinality(wire::Height),
    /// Parent record and finality artifact disagree.
    #[error("Sumeragi v2 parent context record differs from finality at height {0}")]
    ParentContextMismatch(wire::Height),
    /// Persisted successor differs from the unique projection of finalized state.
    #[error("persisted Sumeragi v2 context conflicts with finalized state at height {0}")]
    ConflictingDerivedContext(wire::Height),
    /// Height arithmetic overflowed.
    #[error("Sumeragi v2 height overflow")]
    HeightOverflow,
}

#[cfg(test)]
mod tests {
    use std::{num::NonZeroU64, sync::Arc};

    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
    use iroha_data_model::{
        ChainId,
        block::{BlockHeader, SignedBlock, consensus_v2 as wire},
        consensus::{ConsensusKeyId, ConsensusKeyRecord, ConsensusKeyRole, ConsensusKeyStatus},
        peer::PeerId,
    };

    use super::{V2RecoveryError, recover_active_height, successor_proofs_of_possession};
    use crate::{
        block::{CommittedBlock, ValidBlock},
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
        sumeragi::{
            network_topology::Topology,
            v2::VerifiedHeightContext,
            v2_context_store::{PersistedHeightContext, V2ContextStore},
        },
    };

    fn verified_context() -> (VerifiedHeightContext, Vec<KeyPair>) {
        let mut keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic BLS key")
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
        let context = wire::HeightContext {
            chain_id: ChainId::from("sumeragi-v2-recovery-test"),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: u64::MAX,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"recovery fixture Nexus/AMX"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::Plain,
                chunk_size_bytes: 1024,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 4096,
                max_chunk_count: 4,
            },
            leader_seed: [0x31; 32],
        };
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("BLS proof of possession")
            })
            .collect();
        (
            VerifiedHeightContext::genesis(context, proofs).expect("verified context"),
            keys,
        )
    }

    fn state_for(kura: &Arc<Kura>, chain_id: ChainId) -> State {
        State::new_with_chain_for_testing(
            World::new(),
            Arc::clone(kura),
            LiveQueryStore::start_test(),
            chain_id,
        )
    }

    fn state_with_consensus_keys(kura: &Arc<Kura>, chain_id: ChainId, keys: &[KeyPair]) -> State {
        let mut world = World::new();
        for (index, key) in keys.iter().enumerate() {
            let id = ConsensusKeyId::new(ConsensusKeyRole::Validator, format!("validator{index}"));
            let record = ConsensusKeyRecord {
                id: id.clone(),
                public_key: key.public_key().clone(),
                pop: Some(
                    iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("BLS proof of possession"),
                ),
                activation_height: 0,
                expiry_height: None,
                hsm: None,
                replaces: None,
                status: ConsensusKeyStatus::Active,
            };
            world.consensus_keys.insert(id.clone(), record.clone());
            world
                .consensus_keys_by_pk
                .insert(record.public_key.to_string(), vec![id]);
        }
        State::new_with_chain_for_testing(
            world,
            Arc::clone(kura),
            LiveQueryStore::start_test(),
            chain_id,
        )
    }

    fn dummy_block(
        key: &KeyPair,
        height: u64,
        parent: Option<HashOf<BlockHeader>>,
    ) -> CommittedBlock {
        dummy_block_with_time(key, height, parent, height)
    }

    fn dummy_block_with_time(
        key: &KeyPair,
        height: u64,
        parent: Option<HashOf<BlockHeader>>,
        creation_time_ms: u64,
    ) -> CommittedBlock {
        let valid = ValidBlock::new_dummy_and_modify_header(key.private_key(), |header| {
            header.set_height(NonZeroU64::new(height).expect("non-zero height"));
            header.set_prev_block_hash(parent);
            header.creation_time_ms = creation_time_ms;
            header.merkle_root = None;
        });
        valid.commit_unchecked().unpack(|_| {})
    }

    fn commit_to_state(state: &State, block: &CommittedBlock, context: &wire::HeightContext) {
        let topology = Topology::new(context.roster.iter().map(|entry| entry.validator.clone()));
        let mut state_block = state.block(block.as_ref().header());
        let _events = state_block.apply_without_execution(block, topology.as_ref().to_owned());
        state_block.commit().expect("commit synthetic state block");
    }

    fn artifact_for(
        context: wire::HeightContext,
        block: &SignedBlock,
    ) -> wire::finality::V2FinalityArtifact {
        let subject = wire::BlockSubject {
            parent_block_hash: block.header().prev_block_hash(),
            block_hash: block.hash(),
            payload_hash: Hash::new(block.encode_wire().expect("canonical block wire")),
        };
        let commit_qc = wire::QuorumCertificate {
            round: wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 0,
            },
            phase: wire::GlobalPhase::Commit,
            subject,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xB4; 48],
        };
        let validator_set_pops = vec![vec![0xB5]; context.roster.len()];
        wire::finality::V2FinalityArtifact::new(context, subject, commit_qc, validator_set_pops)
    }

    fn authenticated_artifact_for(
        context: wire::HeightContext,
        block: &SignedBlock,
        keys: &[KeyPair],
    ) -> wire::finality::V2FinalityArtifact {
        let subject = wire::BlockSubject {
            parent_block_hash: block.header().prev_block_hash(),
            block_hash: block.hash(),
            payload_hash: Hash::new(block.encode_wire().expect("canonical block wire")),
        };
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let unsigned_vote = wire::Vote {
            round,
            phase: wire::GlobalPhase::Commit,
            subject,
            signer: 0,
            signature: Vec::new(),
        };
        let preimage = unsigned_vote.signature_preimage();
        let shares = keys[..3]
            .iter()
            .map(|key| {
                Signature::new(key.private_key(), &preimage)
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let commit_qc = wire::QuorumCertificate {
            round,
            phase: wire::GlobalPhase::Commit,
            subject,
            signers: vec![0, 1, 2],
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
                .expect("aggregate CommitQC"),
        };
        let validator_set_pops = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("fixture validator PoP")
            })
            .collect();
        wire::finality::V2FinalityArtifact::new(context, subject, commit_qc, validator_set_pops)
    }

    #[test]
    fn successor_pops_are_copied_only_from_the_durable_parent_artifact() {
        let (verified, current_keys) = verified_context();
        let current_context = verified.context().clone();
        let block = dummy_block(&current_keys[0], current_context.height, None);

        let parent =
            authenticated_artifact_for(current_context.clone(), block.as_ref(), &current_keys);
        parent.verify().expect("authenticated non-boundary parent");
        assert_eq!(
            successor_proofs_of_possession(&parent),
            parent.validator_set_pops,
            "non-boundary recovery must retain the exact historical PoP bytes"
        );

        let mut next_keys = (21_u8..=24)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic next-epoch BLS key")
            })
            .collect::<Vec<_>>();
        next_keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let next_roster = next_keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let next_pops = next_keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("valid next-epoch PoP")
            })
            .collect::<Vec<_>>();

        let mut boundary_context = current_context;
        boundary_context.epoch_end_height = boundary_context.height;
        boundary_context.next_epoch_snapshot = Some(wire::finality::FinalizedNextEpochSnapshot {
            epoch: boundary_context.epoch + 1,
            epoch_end_height: u64::MAX,
            mode: boundary_context.mode,
            quorum: wire::DualQuorum::from_roster(&next_roster).expect("valid next-epoch quorum"),
            roster: next_roster,
            validator_set_pops: next_pops.clone(),
            leader_seed: [0x73; 32],
        });
        let boundary_parent =
            authenticated_artifact_for(boundary_context, block.as_ref(), &current_keys);
        boundary_parent
            .verify()
            .expect("old roster authenticates the complete boundary snapshot");
        assert_eq!(
            successor_proofs_of_possession(&boundary_parent),
            next_pops,
            "boundary recovery must use the authenticated successor PoPs"
        );
        assert_ne!(
            successor_proofs_of_possession(&boundary_parent),
            boundary_parent.validator_set_pops,
            "next-epoch PoPs must not be reconstructed from the current roster"
        );
    }

    #[test]
    fn durable_block_before_wsv_reopens_only_its_persisted_height_context() {
        let (verified, keys) = verified_context();
        let context = verified.context().clone();
        let kura = Kura::blank_kura_for_testing();
        let state = state_for(&kura, context.chain_id.clone());
        let block = dummy_block(&keys[0], 1, None);
        kura.store_block(block.clone())
            .expect("persist canonical block");
        let store =
            V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("open context store");
        store
            .persist(&PersistedHeightContext::from_verified(&verified))
            .expect("persist active context");

        let recovered =
            recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone())
                .expect("resume interrupted height");
        assert_eq!(recovered.verified_context().context(), &context);
        let pending = recovered
            .pending_kura_apply()
            .expect("durable tip requires replay binding");
        assert_eq!(pending.context_id(), context.id());
        assert_eq!(pending.height(), 1);
        assert_eq!(pending.block_hash(), block.as_ref().hash());
        assert_eq!(state.committed_height(), 0);
        assert_eq!(kura.durable_blocks_count(), 1);
    }

    #[test]
    fn wsv_before_finality_reopens_same_height_without_reapplying() {
        let (verified, keys) = verified_context();
        let context = verified.context().clone();
        let kura = Kura::blank_kura_for_testing();
        let state = state_for(&kura, context.chain_id.clone());
        let block = dummy_block(&keys[0], 1, None);
        kura.store_block(block.clone())
            .expect("persist canonical block");
        commit_to_state(&state, &block, &context);
        let store =
            V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("open context store");
        store
            .persist(&PersistedHeightContext::from_verified(&verified))
            .expect("persist active context");

        let recovered =
            recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone())
                .expect("resume finality sidecar window");
        assert_eq!(recovered.verified_context().context(), &context);
        assert_eq!(
            recovered
                .pending_kura_apply()
                .expect("missing finality requires replay binding")
                .block_hash(),
            block.as_ref().hash()
        );
        assert_eq!(state.committed_height(), 1);
        assert!(
            kura.v2_finality_artifact(1)
                .expect("read finality")
                .is_none()
        );
    }

    #[test]
    fn finality_ahead_of_wsv_fails_closed() {
        let (verified, keys) = verified_context();
        let context = verified.context().clone();
        let kura = Kura::blank_kura_for_testing();
        let state = state_for(&kura, context.chain_id.clone());
        let block = dummy_block(&keys[0], 1, None);
        kura.store_block(block.clone())
            .expect("persist canonical block");
        let artifact = artifact_for(context.clone(), block.as_ref());
        let _receipt = kura
            .store_v2_finality_artifact(&artifact)
            .expect("persist finality");

        assert!(matches!(
            recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone(),),
            Err(V2RecoveryError::FinalityAheadOfState {
                finality_height: 1,
                state_height: 0,
            })
        ));
    }

    #[test]
    fn parent_finality_and_immutable_context_mismatch_fails_closed() {
        let (verified, keys) = verified_context();
        let context = verified.context().clone();
        let kura = Kura::blank_kura_for_testing();
        let state = state_for(&kura, context.chain_id.clone());
        let block = dummy_block(&keys[0], 1, None);
        kura.store_block(block.clone())
            .expect("persist canonical block");
        commit_to_state(&state, &block, &context);
        let artifact = artifact_for(context.clone(), block.as_ref());
        let _receipt = kura
            .store_v2_finality_artifact(&artifact)
            .expect("persist finality");

        let mut different = context;
        different.leader_seed[0] ^= 0x80;
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("BLS proof of possession")
            })
            .collect();
        let different = VerifiedHeightContext::genesis(different, proofs)
            .expect("different context is independently valid");
        let store =
            V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("open context store");
        store
            .persist(&PersistedHeightContext::from_verified(&different))
            .expect("persist mismatching context");

        assert!(matches!(
            recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone(),),
            Err(V2RecoveryError::ParentContextMismatch(1))
        ));
    }

    #[test]
    fn missing_context_for_interrupted_durable_block_fails_closed() {
        let (verified, keys) = verified_context();
        let context = verified.context().clone();
        let kura = Kura::blank_kura_for_testing();
        let state = state_for(&kura, context.chain_id);
        kura.store_block(dummy_block(&keys[0], 1, None))
            .expect("persist canonical block");

        assert!(matches!(
            recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone(),),
            Err(V2RecoveryError::MissingActiveContext(1))
        ));
    }

    #[test]
    fn equal_wsv_and_kura_heights_with_different_hashes_fail_closed() {
        let (verified, keys) = verified_context();
        let context = verified.context().clone();
        let kura = Kura::blank_kura_for_testing();
        let state = state_for(&kura, context.chain_id);
        let state_block = dummy_block_with_time(&keys[0], 1, None, 1);
        let kura_block = dummy_block_with_time(&keys[0], 1, None, 2);
        assert_ne!(state_block.as_ref().hash(), kura_block.as_ref().hash());
        commit_to_state(&state, &state_block, verified.context());
        kura.store_block(kura_block)
            .expect("persist conflicting Kura tip");

        assert!(matches!(
            recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone(),),
            Err(V2RecoveryError::StateKuraHashMismatch { height: 1, .. })
        ));
    }

    #[test]
    fn finalized_tip_derives_one_idempotent_successor_context() {
        let (verified, keys) = verified_context();
        let context = verified.context().clone();
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_consensus_keys(&kura, context.chain_id.clone(), &keys);
        let block = dummy_block(&keys[0], 1, None);
        kura.store_block(block.clone())
            .expect("persist canonical block");
        commit_to_state(&state, &block, &context);
        let artifact = authenticated_artifact_for(context.clone(), block.as_ref(), &keys);
        let _receipt = kura
            .store_v2_finality_artifact(&artifact)
            .expect("persist finality");
        let store =
            V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("open context store");
        store
            .persist(&PersistedHeightContext::from_verified(&verified))
            .expect("persist parent context");

        let first =
            recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone())
                .expect("derive successor");
        assert_eq!(first.verified_context().context().height, 2);
        assert_eq!(
            first.verified_context().context().parent_commit_qc,
            Some(artifact.commit_qc.clone())
        );
        assert!(first.pending_kura_apply().is_none());
        let first_context = first.verified_context().context().clone();
        drop(first);

        let repeated =
            recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone())
                .expect("reopen identical successor");
        assert_eq!(repeated.verified_context().context(), &first_context);
        assert!(repeated.pending_kura_apply().is_none());
    }
}
