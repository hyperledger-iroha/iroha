//! Canonical construction of immutable Sumeragi v2 height contexts.
//!
//! The reducer never reads mutable world state. Genesis inputs and finalized
//! epoch snapshots enter here once, and every non-boundary successor carries
//! the previous frozen election inputs unchanged.

use std::collections::BTreeMap;

use iroha_crypto::{Algorithm, Hash};
use iroha_data_model::{
    ChainId, block::consensus_v2 as wire, isi::RegisterPeerWithPop,
    nexus::PublicLaneValidatorStatus, peer::PeerId, transaction::Executable,
};
use iroha_genesis::GenesisBlock;
use mv::storage::StorageReadOnly;
use norito::codec::Encode;
use thiserror::Error;

use super::{stake_snapshot::strict_v2_voting_roster, v2::VerifiedHeightContext};
use crate::state::{
    StateBlock, WorldReadOnly, live_consensus_key_pop_for_peer,
    public_lane_validator_record_matches_key,
};

/// Verified height-one inputs retained until the production reducer opens its
/// safety WAL.
#[derive(Clone)]
pub struct GenesisV2Bootstrap {
    verified_context: VerifiedHeightContext,
}

impl GenesisV2Bootstrap {
    /// Borrow the exact signed-and-staged height context for diagnostics.
    #[must_use]
    pub fn context(&self) -> &wire::HeightContext {
        self.verified_context.context()
    }

    pub(crate) fn into_verified_context(self) -> VerifiedHeightContext {
        self.verified_context
    }
}

/// Extract the only voting roster source accepted at fresh genesis: signed
/// `RegisterPeerWithPop` instructions in the genesis body.
///
/// Plain peer registrations are observers and are intentionally absent.
pub fn signed_genesis_voting_peers(
    genesis: &GenesisBlock,
) -> Result<Vec<PeerId>, V2GenesisBootstrapError> {
    Ok(signed_genesis_validator_pops(genesis)?
        .into_keys()
        .collect())
}

/// Freeze and cryptographically verify the height-one context from a validated
/// but uncommitted genesis state block.
///
/// `signed_parameters` must be decoded from the signed genesis handshake
/// metadata. The function does not accept runtime consensus configuration.
pub fn freeze_staged_genesis_v2(
    genesis: &GenesisBlock,
    staged: &StateBlock<'_>,
    mode: wire::ConsensusMode,
    signed_parameters: wire::SumeragiV2GenesisContextParameters,
) -> Result<GenesisV2Bootstrap, V2GenesisBootstrapError> {
    signed_parameters.validate()?;
    let staged_world = staged.world();
    let signed_pops = signed_genesis_validator_pops(genesis)?;
    if signed_pops.is_empty() {
        return Err(V2GenesisBootstrapError::EmptyVotingRoster);
    }

    let voters = signed_pops.keys().cloned().collect::<Vec<_>>();
    for voter in &voters {
        if !staged_world.peers().iter().any(|peer| peer == voter) {
            return Err(V2GenesisBootstrapError::VoterMissingFromStagedWorld);
        }
        let staged_pop = live_consensus_key_pop_for_peer(staged_world, voter, 1)
            .ok_or(V2GenesisBootstrapError::MissingLiveConsensusKey)?;
        if signed_pops.get(voter) != Some(&staged_pop) {
            return Err(V2GenesisBootstrapError::ProofOfPossessionMismatch);
        }
    }

    let roster = match mode {
        wire::ConsensusMode::Permissioned => voters
            .iter()
            .cloned()
            .map(|validator| wire::ValidatorPower {
                validator,
                power: 1,
            })
            .collect(),
        wire::ConsensusMode::Npos => strict_v2_voting_roster(staged_world, &voters, None)
            .map_err(|error| V2GenesisBootstrapError::Stake(error.to_string()))?,
    };
    let proofs_of_possession = roster
        .iter()
        .map(|entry| {
            signed_pops
                .get(&entry.validator)
                .cloned()
                .ok_or(V2GenesisBootstrapError::ProofOfPossessionMismatch)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let first_transaction = genesis
        .0
        .external_transactions()
        .next()
        .ok_or(V2GenesisBootstrapError::MissingTransaction)?;
    let chain_id = first_transaction.chain().clone();
    if genesis
        .0
        .external_transactions()
        .any(|transaction| transaction.chain() != &chain_id)
    {
        return Err(V2GenesisBootstrapError::MixedChainIds);
    }

    let (epoch_end_height, leader_seed) = match mode {
        wire::ConsensusMode::Permissioned => {
            let mut seed_preimage = b"sumeragi-v2:permissioned-leader-seed".to_vec();
            seed_preimage.extend_from_slice(&chain_id.encode());
            let seed: [u8; 32] = Hash::new(seed_preimage).into();
            (u64::MAX, seed)
        }
        wire::ConsensusMode::Npos => {
            let parameters = staged_world
                .sumeragi_npos_parameters()
                .ok_or(V2GenesisBootstrapError::MissingNposParameters)?;
            let epoch_length = parameters.epoch_length_blocks();
            if epoch_length == 0 {
                return Err(V2GenesisBootstrapError::InvalidEpochLength);
            }
            (epoch_length, parameters.epoch_seed())
        }
    };

    let context = build_genesis_height_context(GenesisContextInputs {
        chain_id,
        election: FrozenElectionInputs {
            epoch: 0,
            epoch_end_height,
            mode,
            roster,
            leader_seed,
        },
        nexus_amx_context_hash: verify_staged_nexus_amx_context_hash(
            staged,
            signed_parameters.nexus_amx_context_hash,
        )?,
        da_layout: signed_parameters.da_layout,
    })
    .map_err(|error| V2GenesisBootstrapError::Context(error.to_string()))?;
    let verified_context = VerifiedHeightContext::genesis(context, proofs_of_possession)
        .map_err(|error| V2GenesisBootstrapError::Adapter(error.to_string()))?;
    Ok(GenesisV2Bootstrap { verified_context })
}

fn signed_genesis_validator_pops(
    genesis: &GenesisBlock,
) -> Result<BTreeMap<PeerId, Vec<u8>>, V2GenesisBootstrapError> {
    let mut validators = BTreeMap::new();
    for transaction in genesis.0.external_transactions() {
        let Executable::Instructions(instructions) = transaction.instructions() else {
            return Err(V2GenesisBootstrapError::UnsupportedGenesisExecutable);
        };
        for register in instructions
            .iter()
            .filter_map(|instruction| instruction.as_any().downcast_ref::<RegisterPeerWithPop>())
        {
            if register.peer.public_key().try_algorithm() != Ok(Algorithm::BlsNormal) {
                return Err(V2GenesisBootstrapError::NonBlsValidator);
            }
            iroha_crypto::bls_normal_pop_verify(register.peer.public_key(), &register.pop)
                .map_err(|_| V2GenesisBootstrapError::InvalidProofOfPossession)?;
            if validators
                .insert(register.peer.clone(), register.pop.clone())
                .is_some()
            {
                return Err(V2GenesisBootstrapError::DuplicateValidator);
            }
        }
    }
    Ok(validators)
}

/// Compute the canonical Nexus/AMX commitment from a validated genesis state
/// block without committing that block. The projection binds every Nexus and
/// deterministic AMX input used by proposal assembly or validation, plus the
/// canonically ordered active public-lane validator records.
#[must_use]
pub fn staged_genesis_nexus_amx_context_hash(staged: &StateBlock<'_>) -> Hash {
    let active_validators = staged
        .world()
        .public_lane_validators()
        .iter()
        .filter(|(key, record)| public_lane_validator_record_matches_key(key, record))
        .filter(|(_, record)| matches!(record.status, PublicLaneValidatorStatus::Active))
        .map(|(key, record)| (key.clone(), record.clone()))
        .collect::<Vec<_>>();
    let lane_lifecycle = staged
        .nexus
        .lane_catalog
        .lanes()
        .iter()
        .map(
            |lane| iroha_config::parameters::actual::SumeragiV2LaneLifecycleEntry {
                lane_id: lane.id,
                incarnation: *staged
                    .lane_incarnations
                    .get(&lane.id)
                    .expect("validated staged genesis has every active lane incarnation"),
                activation_height: *staged
                    .lane_incarnation_activation_heights
                    .get(&lane.id)
                    .expect("validated staged genesis has every lane activation height"),
            },
        )
        .collect::<Vec<_>>();
    iroha_config::parameters::actual::sumeragi_v2_nexus_amx_context_hash(
        &staged.nexus,
        &staged.pipeline,
        &active_validators,
        &lane_lifecycle,
    )
}

fn verify_staged_nexus_amx_context_hash(
    staged: &StateBlock<'_>,
    signed_hash: [u8; 32],
) -> Result<Hash, V2GenesisBootstrapError> {
    let staged = staged_genesis_nexus_amx_context_hash(staged);
    let signed = Hash::prehashed(signed_hash);
    if staged != signed {
        return Err(V2GenesisBootstrapError::NexusAmxContextHashMismatch { signed, staged });
    }
    Ok(signed)
}

/// Failure to derive an exact fresh-genesis reducer bootstrap.
#[derive(Debug, Error)]
pub enum V2GenesisBootstrapError {
    /// Genesis contains no transaction from which to derive the chain id.
    #[error("Sumeragi v2 genesis contains no transaction")]
    MissingTransaction,
    /// Genesis transactions disagree on the signed chain id.
    #[error("Sumeragi v2 genesis transactions contain different chain ids")]
    MixedChainIds,
    /// A genesis transaction uses an executable form that cannot define the
    /// deterministic bootstrap roster.
    #[error("Sumeragi v2 genesis transactions must contain instruction batches")]
    UnsupportedGenesisExecutable,
    /// No signed validator/PoP entries were present.
    #[error("Sumeragi v2 genesis voting roster is empty")]
    EmptyVotingRoster,
    /// One signed validator was repeated.
    #[error("Sumeragi v2 genesis repeats a validator")]
    DuplicateValidator,
    /// Voting validators must use BLS-normal keys.
    #[error("Sumeragi v2 genesis validator key is not BLS-normal")]
    NonBlsValidator,
    /// A signed proof of possession failed verification.
    #[error("Sumeragi v2 genesis contains an invalid proof of possession")]
    InvalidProofOfPossession,
    /// A signed voter was not created by staged execution.
    #[error("Sumeragi v2 signed voter is absent from the staged world")]
    VoterMissingFromStagedWorld,
    /// A signed voter has no live staged consensus key.
    #[error("Sumeragi v2 signed voter has no live staged consensus key")]
    MissingLiveConsensusKey,
    /// Staged and signed PoPs or their canonical order differ.
    #[error("Sumeragi v2 staged proof of possession does not match signed genesis")]
    ProofOfPossessionMismatch,
    /// NPoS mode omitted its signed on-chain election parameters.
    #[error("Sumeragi v2 NPoS genesis is missing election parameters")]
    MissingNposParameters,
    /// NPoS epoch length must be positive.
    #[error("Sumeragi v2 NPoS genesis epoch length must be positive")]
    InvalidEpochLength,
    /// Runtime-injected or otherwise drifted Nexus state differs from the
    /// commitment in signed genesis metadata.
    #[error(
        "Sumeragi v2 signed Nexus/AMX context hash {signed} does not match staged state {staged}"
    )]
    NexusAmxContextHashMismatch {
        /// Hash embedded in signed genesis.
        signed: Hash,
        /// Hash recomputed from the validated staged world.
        staged: Hash,
    },
    /// Exact NPoS power extraction failed.
    #[error("failed to freeze the Sumeragi v2 NPoS stake snapshot: {0}")]
    Stake(String),
    /// Height context construction failed.
    #[error("failed to construct the Sumeragi v2 height context: {0}")]
    Context(String),
    /// Signed context parameters were malformed.
    #[error(transparent)]
    Wire(#[from] wire::ValidationError),
    /// BLS context verification failed.
    #[error("failed to verify the Sumeragi v2 height context: {0}")]
    Adapter(String),
}

/// Fully frozen election inputs accepted by the context builder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FrozenElectionInputs {
    /// Election epoch number.
    pub epoch: u64,
    /// Final height governed by this snapshot.
    pub epoch_end_height: wire::Height,
    /// Genesis-selected consensus mode.
    pub mode: wire::ConsensusMode,
    /// Strictly ordered voting roster with exact powers; observers are absent.
    pub roster: Vec<wire::ValidatorPower>,
    /// Finalized leader seed for deterministic rotation.
    pub leader_seed: [u8; 32],
}

impl FrozenElectionInputs {
    fn quorum(&self) -> Result<wire::DualQuorum, V2ContextBuildError> {
        wire::DualQuorum::from_roster(&self.roster).map_err(V2ContextBuildError::Wire)
    }
}

/// Consensus-relevant immutable inputs selected at genesis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GenesisContextInputs {
    /// Fresh chain identifier.
    pub chain_id: ChainId,
    /// Initial finalized election snapshot.
    pub election: FrozenElectionInputs,
    /// Nexus/AMX consensus-context commitment at genesis.
    pub nexus_amx_context_hash: Hash,
    /// Mandatory deterministic data-availability layout.
    pub da_layout: wire::DataAvailabilityLayout,
}

/// Build the only valid height-one context from genesis-frozen inputs.
pub(crate) fn build_genesis_height_context(
    inputs: GenesisContextInputs,
) -> Result<wire::HeightContext, V2ContextBuildError> {
    if inputs.election.epoch_end_height < 1 {
        return Err(V2ContextBuildError::EpochEndBeforeSuccessor);
    }
    let context = wire::HeightContext {
        chain_id: inputs.chain_id,
        protocol_version: wire::PROTOCOL_VERSION,
        height: 1,
        epoch: inputs.election.epoch,
        epoch_end_height: inputs.election.epoch_end_height,
        mode: inputs.election.mode,
        parent_commit_qc: None,
        quorum: inputs.election.quorum()?,
        roster: inputs.election.roster,
        nexus_amx_context_hash: inputs.nexus_amx_context_hash,
        da_layout: inputs.da_layout,
        leader_seed: inputs.election.leader_seed,
    };
    context.validate()?;
    Ok(context)
}

/// Build the unique successor of one structurally valid finalized artifact.
///
/// At an epoch boundary, `next_epoch_end_height` is mandatory and election
/// inputs come only from the artifact's finalized snapshot. Away from a
/// boundary it must be absent and the old election inputs are copied exactly.
pub(crate) fn build_successor_height_context(
    parent: &wire::finality::V2FinalityArtifact,
    nexus_amx_context_hash: Hash,
    next_epoch_end_height: Option<wire::Height>,
) -> Result<wire::HeightContext, V2ContextBuildError> {
    parent.validate()?;
    let height = parent
        .height
        .checked_add(1)
        .ok_or(V2ContextBuildError::HeightOverflow)?;

    let election = match (parent.next_epoch_snapshot.as_ref(), next_epoch_end_height) {
        (Some(snapshot), Some(epoch_end_height)) => FrozenElectionInputs {
            epoch: snapshot.epoch,
            epoch_end_height,
            mode: snapshot.mode,
            roster: snapshot.roster.clone(),
            leader_seed: snapshot.leader_seed,
        },
        (None, None) => FrozenElectionInputs {
            epoch: parent.height_context.epoch,
            epoch_end_height: parent.height_context.epoch_end_height,
            mode: parent.height_context.mode,
            roster: parent.height_context.roster.clone(),
            leader_seed: parent.height_context.leader_seed,
        },
        (Some(_), None) => return Err(V2ContextBuildError::MissingNextEpochEnd),
        (None, Some(_)) => return Err(V2ContextBuildError::UnexpectedNextEpochEnd),
    };
    if election.epoch_end_height < height {
        return Err(V2ContextBuildError::EpochEndBeforeSuccessor);
    }

    let context = wire::HeightContext {
        chain_id: parent.height_context.chain_id.clone(),
        protocol_version: wire::PROTOCOL_VERSION,
        height,
        epoch: election.epoch,
        epoch_end_height: election.epoch_end_height,
        mode: election.mode,
        parent_commit_qc: Some(parent.commit_qc.clone()),
        quorum: election.quorum()?,
        roster: election.roster,
        nexus_amx_context_hash,
        da_layout: parent.height_context.da_layout,
        leader_seed: election.leader_seed,
    };
    context.validate()?;
    Ok(context)
}

/// Canonical height-context construction failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum V2ContextBuildError {
    /// A wire context or roster is structurally invalid.
    #[error("invalid Sumeragi v2 context input: {0}")]
    Wire(#[from] wire::ValidationError),
    /// Parent finality artifact is structurally invalid.
    #[error("invalid Sumeragi v2 parent finality artifact: {0}")]
    Finality(#[from] wire::finality::V2FinalityValidationError),
    /// Parent height cannot be incremented.
    #[error("Sumeragi v2 height overflow")]
    HeightOverflow,
    /// A finalized epoch snapshot omitted the next epoch's end height.
    #[error("Sumeragi v2 epoch transition is missing its next end height")]
    MissingNextEpochEnd,
    /// A non-boundary height attempted to alter the current epoch end.
    #[error("Sumeragi v2 non-boundary successor supplied a new epoch end")]
    UnexpectedNextEpochEnd,
    /// Selected epoch end precedes the height it would govern.
    #[error("Sumeragi v2 epoch end precedes its successor height")]
    EpochEndBeforeSuccessor,
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        account::AccountId,
        block::{BlockHeader, SignedBlock},
        isi::RegisterPeerWithPop,
        metadata::Metadata,
        nexus::{
            DataSpaceCatalog, DataSpaceId, DataSpaceMetadata, LaneId, PublicLaneValidatorRecord,
            PublicLaneValidatorStatus,
        },
        peer::PeerId,
        prelude::{InstructionBox, TransactionBuilder},
    };
    use iroha_genesis::GenesisBlock;
    use iroha_primitives::numeric::Numeric;

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    fn roster(powers: &[u64]) -> Vec<wire::ValidatorPower> {
        let mut entries = powers
            .iter()
            .enumerate()
            .map(|(index, power)| {
                let seed = u8::try_from(index + 1).expect("small fixture roster");
                let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                    .expect("deterministic key");
                wire::ValidatorPower {
                    validator: PeerId::new(key.public_key().clone()),
                    power: *power,
                }
            })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| left.validator.cmp(&right.validator));
        entries
    }

    fn genesis(mode: wire::ConsensusMode, powers: &[u64], end: u64) -> wire::HeightContext {
        build_genesis_height_context(GenesisContextInputs {
            chain_id: "v2-context-builder-test".into(),
            election: FrozenElectionInputs {
                epoch: 4,
                epoch_end_height: end,
                mode,
                roster: roster(powers),
                leader_seed: [0x41; 32],
            },
            nexus_amx_context_hash: Hash::new(b"genesis nexus amx context"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::Plain,
                chunk_size_bytes: 1024,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 4096,
                max_chunk_count: 4,
            },
        })
        .expect("valid genesis context")
    }

    fn signed_roster_genesis(
        voters: &[KeyPair],
        duplicate_first: bool,
        corrupt_first_pop: bool,
    ) -> GenesisBlock {
        let authority =
            KeyPair::try_from_seed(b"v2-context-genesis-authority".to_vec(), Algorithm::Ed25519)
                .expect("deterministic genesis authority");
        let mut instructions = voters
            .iter()
            .map(|key| {
                let mut pop =
                    iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("BLS PoP fixture");
                if corrupt_first_pop && key.public_key() == voters[0].public_key() {
                    pop[0] ^= 0x80;
                }
                InstructionBox::from(RegisterPeerWithPop::new(
                    PeerId::new(key.public_key().clone()),
                    pop,
                ))
            })
            .collect::<Vec<_>>();
        if duplicate_first {
            let pop = iroha_crypto::bls_normal_pop_prove(voters[0].private_key())
                .expect("duplicate PoP fixture");
            instructions.push(InstructionBox::from(RegisterPeerWithPop::new(
                PeerId::new(voters[0].public_key().clone()),
                pop,
            )));
        }
        let transaction = TransactionBuilder::new(
            "v2-context-signed-roster-test".into(),
            AccountId::new(authority.public_key().clone()),
        )
        .with_instructions(instructions)
        .sign(authority.private_key());
        GenesisBlock(SignedBlock::genesis(
            vec![transaction],
            authority.private_key(),
            None,
            None,
        ))
    }

    #[test]
    fn signed_genesis_roster_is_canonical_and_excludes_non_voters() {
        let voters = [3_u8, 1, 2].map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic BLS voter")
        });
        let genesis = signed_roster_genesis(&voters, false, false);
        let observed = signed_genesis_voting_peers(&genesis).expect("signed roster");
        let mut expected = voters
            .iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect::<Vec<_>>();
        expected.sort();
        assert_eq!(observed, expected);
        assert_eq!(observed.len(), voters.len());
    }

    #[test]
    fn signed_genesis_roster_rejects_duplicate_or_invalid_pop() {
        let voter = KeyPair::try_from_seed(vec![0x51; 32], Algorithm::BlsNormal)
            .expect("deterministic BLS voter");
        assert!(matches!(
            signed_genesis_voting_peers(&signed_roster_genesis(
                std::slice::from_ref(&voter),
                true,
                false,
            )),
            Err(V2GenesisBootstrapError::DuplicateValidator)
        ));
        assert!(matches!(
            signed_genesis_voting_peers(&signed_roster_genesis(
                std::slice::from_ref(&voter),
                false,
                true,
            )),
            Err(V2GenesisBootstrapError::InvalidProofOfPossession)
        ));
    }

    #[test]
    fn staged_genesis_rejects_an_empty_signed_voting_roster() {
        let genesis = signed_roster_genesis(&[], false, false);
        let state = lane_hash_world(&[]);
        let staged = state.block(BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero test height"),
            None,
            None,
            None,
            0,
            0,
        ));
        assert!(matches!(
            freeze_staged_genesis_v2(
                &genesis,
                &staged,
                wire::ConsensusMode::Permissioned,
                wire::SumeragiV2GenesisContextParameters::recommended(),
            ),
            Err(V2GenesisBootstrapError::EmptyVotingRoster)
        ));
    }

    fn lane_record(peer: &PeerId, lane: LaneId, stake: u64) -> PublicLaneValidatorRecord {
        let validator = AccountId::new(peer.public_key().clone());
        PublicLaneValidatorRecord {
            lane_id: lane,
            validator: validator.clone(),
            peer_id: peer.clone(),
            stake_account: validator,
            total_stake: Numeric::from(stake),
            self_stake: Numeric::from(stake),
            metadata: Metadata::default(),
            status: PublicLaneValidatorStatus::Active,
            activation_epoch: None,
            activation_height: None,
            last_reward_epoch: None,
        }
    }

    fn lane_hash_world(records: &[(LaneId, PeerId, u64)]) -> State {
        let world = World::default();
        {
            let mut block = world.public_lane_validators.block();
            for (lane, peer, stake) in records {
                let record = lane_record(peer, *lane, *stake);
                block.insert((*lane, record.validator.clone()), record);
            }
            block.commit();
        }
        State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        )
    }

    fn staged_context_hash(state: &State) -> Hash {
        let block = state.block(BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero test height"),
            None,
            None,
            None,
            0,
            0,
        ));
        staged_genesis_nexus_amx_context_hash(&block)
    }

    #[test]
    fn staged_lane_hash_is_order_independent_and_change_sensitive() {
        let peer_a = PeerId::new(
            KeyPair::try_from_seed(vec![0x61; 32], Algorithm::BlsNormal)
                .expect("peer a")
                .public_key()
                .clone(),
        );
        let peer_b = PeerId::new(
            KeyPair::try_from_seed(vec![0x62; 32], Algorithm::BlsNormal)
                .expect("peer b")
                .public_key()
                .clone(),
        );
        let state_ab = lane_hash_world(&[
            (LaneId::new(1), peer_a.clone(), 7),
            (LaneId::new(2), peer_b.clone(), 5),
        ]);
        let state_ba = lane_hash_world(&[
            (LaneId::new(2), peer_b.clone(), 5),
            (LaneId::new(1), peer_a.clone(), 7),
        ]);
        let changed = lane_hash_world(&[(LaneId::new(1), peer_a, 8), (LaneId::new(2), peer_b, 5)]);
        let hash = staged_context_hash(&state_ab);
        assert_eq!(hash, staged_context_hash(&state_ba));
        assert_ne!(hash, staged_context_hash(&changed));
        let staged = state_ab.block(BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero test height"),
            None,
            None,
            None,
            0,
            0,
        ));
        assert_eq!(
            verify_staged_nexus_amx_context_hash(&staged, hash.into())
                .expect("signed canonical hash"),
            hash
        );
        assert!(matches!(
            verify_staged_nexus_amx_context_hash(&staged, [0; 32]),
            Err(V2GenesisBootstrapError::NexusAmxContextHashMismatch { .. })
        ));
    }

    #[test]
    fn staged_lane_hash_binds_catalog_routing_and_amx_policy() {
        let peer = PeerId::new(
            KeyPair::try_from_seed(vec![0x63; 32], Algorithm::BlsNormal)
                .expect("peer")
                .public_key()
                .clone(),
        );
        let records = [(LaneId::SINGLE, peer, 9)];
        let baseline = lane_hash_world(&records);
        let mut changed_catalog = lane_hash_world(&records);
        let mut nexus = iroha_config::parameters::actual::Nexus::default();
        nexus.enabled = true;
        nexus.dataspace_catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: DataSpaceId::new(7),
                alias: "runtime-only-extra".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("valid runtime catalog");
        changed_catalog
            .set_nexus(nexus)
            .expect("install unrelated runtime catalog");

        assert_ne!(
            baseline.view().world().dataspace_catalog(),
            changed_catalog.view().world().dataspace_catalog(),
        );
        assert_ne!(
            staged_context_hash(&baseline),
            staged_context_hash(&changed_catalog),
            "dataspace catalog changes must alter the signed height context",
        );

        let mut changed_amx = lane_hash_world(&records);
        let mut pipeline = changed_amx.pipeline_snapshot();
        pipeline.amx_group_budget_ms = pipeline.amx_group_budget_ms.saturating_add(1);
        changed_amx.set_pipeline(pipeline);
        assert_ne!(
            staged_context_hash(&baseline),
            staged_context_hash(&changed_amx),
            "AMX policy changes must alter the signed height context",
        );
    }

    fn artifact(
        context: wire::HeightContext,
        next: Option<wire::finality::FinalizedNextEpochSnapshot>,
    ) -> wire::finality::V2FinalityArtifact {
        let subject = wire::BlockSubject {
            parent_block_hash: context
                .parent_commit_qc
                .as_ref()
                .map(|certificate| certificate.subject.block_hash),
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new([u8::try_from(
                context.height,
            )
            .unwrap_or(0)])),
            payload_hash: Hash::new([0x52, u8::try_from(context.height).unwrap_or(0)]),
        };
        let commit_qc = wire::QuorumCertificate {
            round: wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 0,
            },
            phase: wire::GlobalPhase::Commit,
            subject,
            signers: vec![0, 1, 2, 3],
            aggregate_signature: vec![0xA5; 48],
        };
        wire::finality::V2FinalityArtifact::new(context, subject, commit_qc, next)
    }

    #[test]
    fn non_boundary_successor_copies_frozen_election_inputs_exactly() {
        let parent_context = genesis(wire::ConsensusMode::Npos, &[7, 5, 3, 1], 3);
        let parent = artifact(parent_context.clone(), None);
        let successor = build_successor_height_context(&parent, Hash::new(b"next lanes"), None)
            .expect("successor context");
        assert_eq!(successor.height, 2);
        assert_eq!(successor.epoch, parent_context.epoch);
        assert_eq!(successor.epoch_end_height, parent_context.epoch_end_height);
        assert_eq!(successor.roster, parent_context.roster);
        assert_eq!(successor.quorum, parent_context.quorum);
        assert_eq!(successor.leader_seed, parent_context.leader_seed);
        assert_eq!(successor.parent_commit_qc, Some(parent.commit_qc));
    }

    #[test]
    fn boundary_successor_uses_only_the_finalized_next_epoch_snapshot() {
        let parent_context = genesis(wire::ConsensusMode::Npos, &[7, 5, 3, 1], 1);
        let next_roster = roster(&[2, 4, 6, 8]);
        let snapshot = wire::finality::FinalizedNextEpochSnapshot {
            epoch: parent_context.epoch + 1,
            mode: parent_context.mode,
            quorum: wire::DualQuorum::from_roster(&next_roster).expect("next quorum"),
            roster: next_roster.clone(),
            leader_seed: [0x77; 32],
        };
        let parent = artifact(parent_context, Some(snapshot));
        let successor = build_successor_height_context(&parent, Hash::new(b"next lanes"), Some(5))
            .expect("epoch successor");
        assert_eq!(successor.height, 2);
        assert_eq!(successor.epoch, 5);
        assert_eq!(successor.epoch_end_height, 5);
        assert_eq!(successor.roster, next_roster);
        assert_eq!(successor.leader_seed, [0x77; 32]);
    }

    #[test]
    fn epoch_end_argument_is_present_only_at_a_certified_boundary() {
        let non_boundary = artifact(genesis(wire::ConsensusMode::Npos, &[4, 3, 2, 1], 3), None);
        assert_eq!(
            build_successor_height_context(&non_boundary, Hash::new(b"lanes"), Some(8)),
            Err(V2ContextBuildError::UnexpectedNextEpochEnd)
        );

        let boundary_context = genesis(wire::ConsensusMode::Npos, &[4, 3, 2, 1], 1);
        let snapshot = wire::finality::FinalizedNextEpochSnapshot {
            epoch: boundary_context.epoch + 1,
            mode: boundary_context.mode,
            roster: boundary_context.roster.clone(),
            quorum: boundary_context.quorum,
            leader_seed: [0x19; 32],
        };
        let boundary = artifact(boundary_context, Some(snapshot));
        assert_eq!(
            build_successor_height_context(&boundary, Hash::new(b"lanes"), None),
            Err(V2ContextBuildError::MissingNextEpochEnd)
        );
    }

    #[test]
    fn permissioned_genesis_rejects_non_unit_power() {
        let error = build_genesis_height_context(GenesisContextInputs {
            chain_id: "bad-permissioned-context".into(),
            election: FrozenElectionInputs {
                epoch: 0,
                epoch_end_height: 10,
                mode: wire::ConsensusMode::Permissioned,
                roster: roster(&[1, 2, 1, 1]),
                leader_seed: [0; 32],
            },
            nexus_amx_context_hash: Hash::new(b"nexus amx context"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::Plain,
                chunk_size_bytes: 1024,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 4096,
                max_chunk_count: 4,
            },
        })
        .expect_err("permissioned power must be one");
        assert!(matches!(
            error,
            V2ContextBuildError::Wire(wire::ValidationError::PermissionedPowerNotOne)
        ));
    }
}
