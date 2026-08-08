//! Canonical construction of immutable Sumeragi v2 height contexts.
//!
//! The reducer never reads mutable world state. Genesis inputs and finalized
//! epoch snapshots enter here once, and every non-boundary successor carries
//! the previous frozen election inputs unchanged.

use std::collections::BTreeMap;

use iroha_crypto::{Algorithm, Hash};
use iroha_data_model::{
    NetworkId, block::consensus_v2 as wire, isi::RegisterBox, nexus::PublicLaneValidatorStatus,
    peer::PeerId, transaction::Executable,
};
use iroha_genesis::GenesisBlock;
use mv::storage::StorageReadOnly;
use norito::codec::Encode;
use thiserror::Error;

use super::{
    stake_snapshot::{StrictV2StakeSnapshotError, strict_v2_voting_roster},
    v2::VerifiedHeightContext,
};
use crate::state::{
    StateBlock, StateReadOnly, WorldReadOnly, epoch_validator_peer_ids_from_world,
    live_consensus_key_pop_for_peer, nexus_active_lane_ids,
    public_lane_validator_record_matches_key,
};

/// Verified height-one inputs retained until the production reducer opens its
/// safety WAL.
#[derive(Clone)]
pub struct GenesisV2Bootstrap {
    verified_context: VerifiedHeightContext,
    staged_nexus_amx_context: StagedGenesisNexusAmxContext,
}

/// Non-forgeable proof that one Nexus/AMX projection was recomputed from the
/// validated, uncommitted genesis overlay.
///
/// The field is private so only [`freeze_staged_genesis_v2`] can mint this
/// token. The height runner consumes it at the sole pre-commit boundary where
/// committed state cannot yet contain the signed genesis projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StagedGenesisNexusAmxContext {
    hash: Hash,
}

impl StagedGenesisNexusAmxContext {
    /// Return the exact projection authenticated by staged genesis execution.
    pub(crate) const fn hash(self) -> Hash {
        self.hash
    }

    /// Construct an authenticated projection token for boundary unit tests.
    #[cfg(test)]
    pub(super) const fn for_test(hash: Hash) -> Self {
        Self { hash }
    }
}

impl GenesisV2Bootstrap {
    /// Borrow the exact signed-and-staged height context for diagnostics.
    #[must_use]
    pub fn context(&self) -> &wire::HeightContext {
        self.verified_context.context()
    }

    pub(crate) fn into_parts(self) -> (VerifiedHeightContext, StagedGenesisNexusAmxContext) {
        (self.verified_context, self.staged_nexus_amx_context)
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

/// Verify that persisted height-one finality consumes the exact voting
/// authority signed into canonical genesis.
///
/// A finality artifact can prove that its PoPs and CommitQC are internally
/// valid, but that proof is self-referential until its ordered validators and
/// aligned PoP bytes are compared with an external trust root. This check
/// supplies that root from the signed `RegisterPeerWithPop` instructions.
///
/// # Errors
///
/// Returns [`V2GenesisBootstrapError`] when genesis does not define a valid
/// voting authority, the height context is not a valid genesis context, or
/// the persisted authority differs in any validator or PoP byte.
pub fn validate_signed_genesis_v2_authority(
    genesis: &GenesisBlock,
    context: &wire::HeightContext,
    validator_set_pops: &[Vec<u8>],
) -> Result<(), V2GenesisBootstrapError> {
    let signed = signed_genesis_validator_pops(genesis)?;
    if signed.is_empty() {
        return Err(V2GenesisBootstrapError::EmptyVotingRoster);
    }
    let exact_authority = signed.len() == context.roster.len()
        && context.roster.len() == validator_set_pops.len()
        && signed
            .iter()
            .zip(context.roster.iter().zip(validator_set_pops.iter()))
            .all(
                |((signed_validator, signed_pop), (persisted_validator, persisted_pop))| {
                    signed_validator == &persisted_validator.validator
                        && signed_pop == persisted_pop
                        && (context.mode != wire::ConsensusMode::Permissioned
                            || persisted_validator.power == 1)
                },
            );
    if !exact_authority {
        return Err(V2GenesisBootstrapError::FinalityVotingAuthorityMismatch);
    }
    // Establish the signed-genesis trust root before performing any
    // self-contained validation over the persisted, attacker-controlled
    // context. Besides preserving the precise authority-mismatch diagnostic,
    // this keeps unauthorised validator sets out of the more expensive
    // cryptographic validation path.
    VerifiedHeightContext::genesis(context.clone(), validator_set_pops.to_vec())
        .map_err(|error| V2GenesisBootstrapError::Adapter(error.to_string()))?;
    Ok(())
}

/// Verify the complete persisted height-one election against deterministically
/// executed signed genesis state.
///
/// The cheaper pre-replay check in [`validate_signed_genesis_v2_authority`]
/// anchors validator identities and PoPs before an untrusted finality roster is
/// used. This second check runs after genesis execution and additionally binds
/// NPoS powers, quorum, epoch bounds, leader seed, next-epoch snapshot, and the
/// staged Nexus/AMX projection to their signed state derivation.
///
/// # Errors
///
/// Returns [`V2GenesisBootstrapError`] when recomputing the canonical genesis
/// context fails or any persisted context/PoP byte differs.
pub(crate) fn validate_staged_genesis_v2_authority(
    genesis: &GenesisBlock,
    staged: &StateBlock<'_>,
    context: &wire::HeightContext,
    validator_set_pops: &[Vec<u8>],
) -> Result<(), V2GenesisBootstrapError> {
    let expected = freeze_staged_genesis_v2(
        genesis,
        staged,
        context.mode,
        wire::SumeragiV2GenesisContextParameters {
            da_layout: context.da_layout,
            nexus_amx_context_hash: *context.nexus_amx_context_hash.as_ref(),
            execution_policy_hash: *context.execution_policy_hash.as_ref(),
        },
    )?;
    let (verified, _) = expected.into_parts();
    if verified.context() != context || verified.proofs_of_possession() != validator_set_pops {
        return Err(V2GenesisBootstrapError::FinalityVotingAuthorityMismatch);
    }
    Ok(())
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

    let network_id = staged.network_id;

    let (epoch_end_height, leader_seed) = match mode {
        wire::ConsensusMode::Permissioned => {
            let mut seed_preimage = b"sumeragi-v2:permissioned-leader-seed".to_vec();
            seed_preimage.extend_from_slice(&network_id.encode());
            let seed: [u8; 32] = Hash::new(seed_preimage).into();
            (u64::MAX, seed)
        }
        wire::ConsensusMode::Npos => {
            let parameters = staged_world
                .sumeragi_npos_parameters()
                .ok_or(V2GenesisBootstrapError::MissingNposParameters)?;
            let epoch_length = parameters.epoch_length_blocks().get();
            (epoch_length, parameters.epoch_seed())
        }
    };

    let election = FrozenElectionInputs {
        epoch: 0,
        epoch_end_height,
        mode,
        roster,
        leader_seed,
    };
    let next_epoch_snapshot = finalized_next_epoch_snapshot(staged, &network_id, 1, &election)
        .map_err(|error| V2GenesisBootstrapError::Context(error.to_string()))?;
    let staged_nexus_amx_context_hash =
        verify_staged_nexus_amx_context_hash(staged, signed_parameters.nexus_amx_context_hash)?;
    let staged_execution_policy_hash =
        verify_staged_execution_policy_hash(staged, signed_parameters.execution_policy_hash)?;
    let context = build_genesis_height_context(GenesisContextInputs {
        network_id,
        election,
        next_epoch_snapshot,
        nexus_amx_context_hash: staged_nexus_amx_context_hash,
        execution_policy_hash: staged_execution_policy_hash,
        da_layout: signed_parameters.da_layout,
    })
    .map_err(|error| V2GenesisBootstrapError::Context(error.to_string()))?;
    let verified_context = VerifiedHeightContext::genesis(context, proofs_of_possession)
        .map_err(|error| V2GenesisBootstrapError::Adapter(error.to_string()))?;
    Ok(GenesisV2Bootstrap {
        verified_context,
        staged_nexus_amx_context: StagedGenesisNexusAmxContext {
            hash: staged_nexus_amx_context_hash,
        },
    })
}

fn signed_genesis_validator_pops(
    genesis: &GenesisBlock,
) -> Result<BTreeMap<PeerId, Vec<u8>>, V2GenesisBootstrapError> {
    let mut validators = BTreeMap::new();
    for transaction in genesis.0.external_transactions() {
        let Executable::Instructions(instructions) = transaction.instructions() else {
            return Err(V2GenesisBootstrapError::UnsupportedGenesisExecutable);
        };
        for register in instructions.iter().filter_map(|instruction| {
            let RegisterBox::Peer(register) = instruction.as_any().downcast_ref::<RegisterBox>()?
            else {
                return None;
            };
            Some(register)
        }) {
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

/// Compute the canonical V1 execution policy from a validated, uncommitted genesis block.
///
/// # Errors
///
/// Returns an error if an enabled Nexus policy has no authenticated runtime policy set.
pub fn staged_genesis_execution_policy_hash(
    staged: &StateBlock<'_>,
) -> Result<Hash, V2GenesisBootstrapError> {
    staged
        .execution_policy_digest_v1()
        .map(Hash::prehashed)
        .map_err(|error| V2GenesisBootstrapError::ExecutionPolicy(error.to_string()))
}

fn verify_staged_execution_policy_hash(
    staged: &StateBlock<'_>,
    signed_hash: [u8; 32],
) -> Result<Hash, V2GenesisBootstrapError> {
    let staged = staged_genesis_execution_policy_hash(staged)?;
    let signed = Hash::prehashed(signed_hash);
    if staged != signed {
        return Err(V2GenesisBootstrapError::ExecutionPolicyHashMismatch { signed, staged });
    }
    Ok(signed)
}

/// Failure to derive an exact fresh-genesis reducer bootstrap.
#[derive(Debug, Error)]
pub enum V2GenesisBootstrapError {
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
    /// Persisted height-one finality names another voting authority.
    #[error("Sumeragi v2 height-one finality voting authority differs from signed genesis")]
    FinalityVotingAuthorityMismatch,
    /// NPoS mode omitted its signed on-chain election parameters.
    #[error("Sumeragi v2 NPoS genesis is missing election parameters")]
    MissingNposParameters,
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
    /// Runtime-injected or otherwise drifted execution policy differs from signed genesis.
    #[error(
        "Sumeragi v2 signed execution-policy hash {signed} does not match staged state {staged}"
    )]
    ExecutionPolicyHashMismatch {
        /// Hash embedded in signed genesis.
        signed: Hash,
        /// Hash recomputed from the validated staged execution policy.
        staged: Hash,
    },
    /// The staged execution policy could not be represented canonically.
    #[error("failed to derive the staged execution-policy identity: {0}")]
    ExecutionPolicy(String),
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
    /// Exact genesis-derived network identity.
    pub network_id: NetworkId,
    /// Initial finalized election snapshot.
    pub election: FrozenElectionInputs,
    /// Old-roster-authenticated transition when height one ends epoch zero.
    pub next_epoch_snapshot: Option<wire::finality::FinalizedNextEpochSnapshot>,
    /// Nexus/AMX consensus-context commitment at genesis.
    pub nexus_amx_context_hash: Hash,
    /// Canonical V1 boot execution-policy identity.
    pub execution_policy_hash: Hash,
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
        network_id: inputs.network_id,
        protocol_version: wire::PROTOCOL_VERSION,
        height: 1,
        epoch: inputs.election.epoch,
        epoch_end_height: inputs.election.epoch_end_height,
        next_epoch_snapshot: inputs.next_epoch_snapshot,
        mode: inputs.election.mode,
        parent_commit_qc: None,
        snapshot_bootstrap: None,
        quorum: inputs.election.quorum()?,
        roster: inputs.election.roster,
        nexus_amx_context_hash: inputs.nexus_amx_context_hash,
        execution_policy_hash: inputs.execution_policy_hash,
        da_layout: inputs.da_layout,
        leader_seed: inputs.election.leader_seed,
    };
    context.validate()?;
    Ok(context)
}

/// Build the unique successor of one structurally valid finalized artifact.
///
/// At an epoch boundary, all election inputs (including the next epoch end)
/// come only from the parent-authenticated snapshot. Away from a boundary the
/// old election inputs are copied exactly.
pub(crate) fn build_successor_height_context(
    parent: &wire::finality::V2FinalityArtifact,
    nexus_amx_context_hash: Hash,
    next_epoch_snapshot: Option<wire::finality::FinalizedNextEpochSnapshot>,
) -> Result<wire::HeightContext, V2ContextBuildError> {
    parent.validate()?;
    let height = parent
        .height
        .checked_add(1)
        .ok_or(V2ContextBuildError::HeightOverflow)?;
    let election = successor_election_inputs(parent, height)?;
    if election.epoch_end_height < height {
        return Err(V2ContextBuildError::EpochEndBeforeSuccessor);
    }

    let context = wire::HeightContext {
        network_id: parent.height_context.network_id,
        protocol_version: wire::PROTOCOL_VERSION,
        height,
        epoch: election.epoch,
        epoch_end_height: election.epoch_end_height,
        next_epoch_snapshot,
        mode: election.mode,
        parent_commit_qc: Some(parent.commit_qc.clone()),
        snapshot_bootstrap: None,
        quorum: election.quorum()?,
        roster: election.roster,
        nexus_amx_context_hash,
        execution_policy_hash: parent.height_context.execution_policy_hash,
        da_layout: parent.height_context.da_layout,
        leader_seed: election.leader_seed,
    };
    context.validate()?;
    Ok(context)
}

fn successor_election_inputs(
    parent: &wire::finality::V2FinalityArtifact,
    height: wire::Height,
) -> Result<FrozenElectionInputs, V2ContextBuildError> {
    let election = match parent.height_context.next_epoch_snapshot.as_ref() {
        Some(snapshot) => FrozenElectionInputs {
            epoch: snapshot.epoch,
            epoch_end_height: snapshot.epoch_end_height,
            mode: snapshot.mode,
            roster: snapshot.roster.clone(),
            leader_seed: snapshot.leader_seed,
        },
        None => FrozenElectionInputs {
            epoch: parent.height_context.epoch,
            epoch_end_height: parent.height_context.epoch_end_height,
            mode: parent.height_context.mode,
            roster: parent.height_context.roster.clone(),
            leader_seed: parent.height_context.leader_seed,
        },
    };
    if election.epoch_end_height < height {
        return Err(V2ContextBuildError::EpochEndBeforeSuccessor);
    }
    Ok(election)
}

/// Build and freeze the unique successor directly from finalized pre-state.
pub(crate) fn build_successor_height_context_from_state(
    parent: &wire::finality::V2FinalityArtifact,
    state: &impl StateReadOnly,
    nexus_amx_context_hash: Hash,
) -> Result<wire::HeightContext, V2ContextBuildError> {
    parent.validate()?;
    let height = parent
        .height
        .checked_add(1)
        .ok_or(V2ContextBuildError::HeightOverflow)?;
    let election = successor_election_inputs(parent, height)?;
    let next_epoch_snapshot =
        finalized_next_epoch_snapshot(state, &parent.height_context.network_id, height, &election)?;
    build_successor_height_context(parent, nexus_amx_context_hash, next_epoch_snapshot)
}

/// Derive the complete transition committed by the old roster at an epoch's
/// final height.
///
/// The source is the committed state *before* any candidate at `height` is
/// executed. This makes the transition available when the height context is
/// frozen and therefore includes it in every Prepare/Commit vote's context
/// identifier. Transactions in the boundary block take effect only for later
/// elections, never retroactively for the imminent validator set.
pub(crate) fn finalized_next_epoch_snapshot(
    state: &impl StateReadOnly,
    network_id: &NetworkId,
    height: wire::Height,
    election: &FrozenElectionInputs,
) -> Result<Option<wire::finality::FinalizedNextEpochSnapshot>, V2ContextBuildError> {
    // The terminal height is also necessarily the end of its epoch, but it
    // has no representable successor whose election could consume a frozen
    // snapshot.
    if height == wire::Height::MAX {
        return Ok(None);
    }
    if height != election.epoch_end_height {
        return Ok(None);
    }
    let successor_height = height
        .checked_add(1)
        .ok_or(V2ContextBuildError::HeightOverflow)?;
    let epoch = election
        .epoch
        .checked_add(1)
        .ok_or(V2ContextBuildError::EpochOverflow)?;
    let npos_params = if election.mode == wire::ConsensusMode::Npos {
        Some(
            super::v2_npos::committed_epoch_params(state.world()).map_err(|error| match error {
                super::v2_npos::V2NposError::MissingCommittedParameters => {
                    V2ContextBuildError::MissingNposParameters
                }
                _ => V2ContextBuildError::InvalidNposParameters,
            })?,
        )
    } else {
        None
    };
    let authenticated_npos_seed = if let Some(params) = npos_params {
        let record = state
            .world()
            .vrf_epochs()
            .get(&election.epoch)
            .ok_or(V2ContextBuildError::MissingPreBoundaryVrfRecord)?;
        Some(
            super::v2_npos::authenticated_successor_seed(
                network_id,
                election.epoch,
                election.epoch_end_height,
                election.leader_seed,
                &election.roster,
                params,
                record,
            )
            .map_err(|_| V2ContextBuildError::InvalidPreBoundaryVrfRecord)?,
        )
    } else {
        None
    };
    let roster = match election.mode {
        wire::ConsensusMode::Permissioned => election.roster.clone(),
        wire::ConsensusMode::Npos => {
            let elected = epoch_validator_peer_ids_from_world(
                state.world(),
                state.commit_topology().iter().cloned(),
                successor_height,
                state.nexus(),
                epoch,
            )
            .ok_or(V2ContextBuildError::MissingFinalizedEpochRoster)?;
            let active_lanes = state
                .nexus()
                .enabled
                .then(|| nexus_active_lane_ids(state.nexus()));
            strict_v2_voting_roster(state.world(), &elected, active_lanes.as_ref())?
        }
    };
    let quorum = wire::DualQuorum::from_roster(&roster)?;
    let validator_set_pops = roster
        .iter()
        .map(|entry| {
            live_consensus_key_pop_for_peer(state.world(), &entry.validator, successor_height)
                .ok_or(V2ContextBuildError::MissingNextEpochProofOfPossession)
        })
        .collect::<Result<Vec<_>, _>>()?;
    wire::finality::verify_validator_power_roster_pops(&roster, &validator_set_pops)
        .map_err(V2ContextBuildError::NextEpochCryptography)?;
    let epoch_end_height = match election.mode {
        wire::ConsensusMode::Permissioned => u64::MAX,
        wire::ConsensusMode::Npos => {
            let epoch_length = npos_params
                .expect("NPoS branch validates the committed schedule before snapshot construction")
                .epoch_length_blocks;
            height
                .checked_add(epoch_length)
                .ok_or(V2ContextBuildError::HeightOverflow)?
        }
    };
    let leader_seed = match election.mode {
        wire::ConsensusMode::Permissioned => {
            let mut preimage = b"sumeragi-v2:permissioned-next-epoch".to_vec();
            preimage.extend_from_slice(&election.leader_seed);
            preimage.extend_from_slice(&height.to_le_bytes());
            Hash::new(preimage).into()
        }
        wire::ConsensusMode::Npos => authenticated_npos_seed
            .expect("NPoS branch authenticates the pre-boundary seed before roster selection"),
    };
    Ok(Some(wire::finality::FinalizedNextEpochSnapshot {
        epoch,
        epoch_end_height,
        mode: election.mode,
        roster,
        validator_set_pops,
        quorum,
        leader_seed,
    }))
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
    /// The current election epoch cannot be incremented.
    #[error("Sumeragi v2 epoch overflows u64")]
    EpochOverflow,
    /// NPoS state has no finalized roster for the imminent epoch.
    #[error("Sumeragi v2 NPoS boundary is missing its finalized next-epoch roster")]
    MissingFinalizedEpochRoster,
    /// One selected next-epoch validator has no live pre-boundary PoP.
    #[error("Sumeragi v2 next-epoch roster is missing a live proof of possession")]
    MissingNextEpochProofOfPossession,
    /// A selected next-epoch key or proof failed BLS verification.
    #[error("invalid Sumeragi v2 next-epoch roster cryptography: {0}")]
    NextEpochCryptography(wire::finality::V2QuorumCertificateVerificationError),
    /// NPoS boundary state omitted its finalized epoch parameters.
    #[error("Sumeragi v2 NPoS boundary is missing on-chain parameters")]
    MissingNposParameters,
    /// NPoS boundary parameters do not reserve finalized pre-state after the
    /// reveal cutoff.
    #[error("Sumeragi v2 NPoS boundary has an invalid committed epoch schedule")]
    InvalidNposParameters,
    /// The finalized state before an NPoS boundary omitted its authenticated
    /// current-epoch VRF record.
    #[error("Sumeragi v2 NPoS boundary is missing its authenticated pre-boundary VRF record")]
    MissingPreBoundaryVrfRecord,
    /// The retained pre-boundary record is inconsistent with the frozen
    /// epoch, roster, window schedule, or authenticated observations.
    #[error("Sumeragi v2 NPoS boundary has an invalid authenticated VRF record")]
    InvalidPreBoundaryVrfRecord,
    /// Exact NPoS voting-power extraction failed.
    #[error(transparent)]
    Stake(#[from] StrictV2StakeSnapshotError),
    /// Selected epoch end precedes the height it would govern.
    #[error("Sumeragi v2 epoch end precedes its successor height")]
    EpochEndBeforeSuccessor,
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        ChainId, NetworkId,
        account::AccountId,
        block::{BlockHeader, SignedBlock},
        consensus::{ConsensusKeyId, ConsensusKeyRecord, ConsensusKeyRole, ConsensusKeyStatus},
        isi::RegisterPeerWithPop,
        metadata::Metadata,
        nexus::{
            DataSpaceCatalog, DataSpaceId, DataSpaceMetadata, LaneId, PublicLaneValidatorRecord,
            PublicLaneValidatorStatus,
        },
        parameter::system::SumeragiNposParameters,
        peer::PeerId,
        prelude::{InstructionBox, TransactionBuilder},
    };
    use iroha_genesis::GenesisBlock;
    use iroha_primitives::numeric::Quantity;

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    fn test_network_id(seed: u8) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([seed; Hash::LENGTH]),
        ))
    }

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
        let roster = roster(powers);
        let next_epoch_snapshot = (end == 1).then(|| {
            let next_roster = roster.clone();
            wire::finality::FinalizedNextEpochSnapshot {
                epoch: 5,
                epoch_end_height: 5,
                mode,
                quorum: wire::DualQuorum::from_roster(&next_roster).expect("next quorum"),
                validator_set_pops: vec![vec![0x43]; next_roster.len()],
                roster: next_roster,
                leader_seed: [0x42; 32],
            }
        });
        build_genesis_height_context(GenesisContextInputs {
            network_id: test_network_id(0x41),
            election: FrozenElectionInputs {
                epoch: 4,
                epoch_end_height: end,
                mode,
                roster,
                leader_seed: [0x41; 32],
            },
            next_epoch_snapshot,
            nexus_amx_context_hash: Hash::new(b"genesis nexus amx context"),
            execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 4096,
                max_chunk_count: 8,
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
        let transaction = TransactionBuilder::new_genesis(
            AccountId::new(authority.public_key().clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
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
    fn persisted_genesis_finality_authority_is_rooted_in_signed_genesis() {
        let voters = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic signed genesis voter")
            })
            .collect::<Vec<_>>();
        let genesis = signed_roster_genesis(&voters, false, false);
        let authority = |keys: &[KeyPair]| {
            let mut ordered = keys.iter().collect::<Vec<_>>();
            ordered.sort_by(|left, right| left.public_key().cmp(right.public_key()));
            let roster = ordered
                .iter()
                .map(|key| wire::ValidatorPower {
                    validator: PeerId::new(key.public_key().clone()),
                    power: 1,
                })
                .collect::<Vec<_>>();
            let validator_set_pops = ordered
                .iter()
                .map(|key| {
                    iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("valid finality PoP")
                })
                .collect::<Vec<_>>();
            let context = wire::HeightContext {
                network_id: test_network_id(0x42),
                protocol_version: wire::PROTOCOL_VERSION,
                height: 1,
                epoch: 0,
                epoch_end_height: u64::MAX,
                next_epoch_snapshot: None,
                mode: wire::ConsensusMode::Permissioned,
                parent_commit_qc: None,
                snapshot_bootstrap: None,
                quorum: wire::DualQuorum::from_roster(&roster).expect("canonical quorum"),
                roster,
                nexus_amx_context_hash: Hash::new(b"signed genesis finality authority"),
                execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
                da_layout: wire::DataAvailabilityLayout {
                    encoding: wire::PayloadEncoding::ReedSolomon16,
                    chunk_size_bytes: 1024,
                    data_shards: 1,
                    parity_shards: 1,
                    max_payload_size_bytes: 4096,
                    max_chunk_count: 8,
                },
                leader_seed: [0xA7; 32],
            };
            (context, validator_set_pops)
        };
        let (signed_context, signed_pops) = authority(&voters);
        validate_signed_genesis_v2_authority(&genesis, &signed_context, &signed_pops)
            .expect("the exact signed authority must be accepted");

        let mut forged_power_context = signed_context.clone();
        forged_power_context.roster[0].power = 1_000;
        forged_power_context.quorum =
            wire::DualQuorum::from_roster(&forged_power_context.roster).expect("forged quorum");
        assert!(matches!(
            validate_signed_genesis_v2_authority(&genesis, &forged_power_context, &signed_pops,),
            Err(V2GenesisBootstrapError::FinalityVotingAuthorityMismatch)
        ));

        let attacker_keys = (81_u8..=84)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic attacker voter")
            })
            .collect::<Vec<_>>();
        let (attacker_context, attacker_pops) = authority(&attacker_keys);
        assert!(matches!(
            validate_signed_genesis_v2_authority(&genesis, &attacker_context, &attacker_pops,),
            Err(V2GenesisBootstrapError::FinalityVotingAuthorityMismatch)
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
            total_stake: Quantity::from(stake),
            self_stake: Quantity::from(stake),
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
    fn staged_execution_policy_hash_rejects_process_local_drift() {
        let baseline = lane_hash_world(&[]);
        let staged = baseline.block(BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero test height"),
            None,
            None,
            None,
            0,
            0,
        ));
        let expected =
            staged_genesis_execution_policy_hash(&staged).expect("derive baseline policy");
        assert_eq!(
            verify_staged_execution_policy_hash(&staged, expected.into())
                .expect("matching signed policy"),
            expected
        );
        drop(staged);

        let mut drifted = baseline;
        let mut pipeline = drifted.pipeline_snapshot();
        pipeline.overlay_max_bytes = pipeline.overlay_max_bytes.saturating_add(1);
        drifted.set_pipeline(pipeline);
        let staged = drifted.block(BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero test height"),
            None,
            None,
            None,
            0,
            0,
        ));
        assert!(matches!(
            verify_staged_execution_policy_hash(&staged, expected.into()),
            Err(V2GenesisBootstrapError::ExecutionPolicyHashMismatch { .. })
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
        mut context: wire::HeightContext,
        next: Option<wire::finality::FinalizedNextEpochSnapshot>,
    ) -> wire::finality::V2FinalityArtifact {
        context.next_epoch_snapshot = next;
        context.validate().expect("valid artifact context");
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
            proposal_round: wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 0,
            },
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment: wire::ExecutionCommitment::without_topups(
                Hash::new(b"context fixture parent state"),
                Hash::new(b"context fixture post state"),
                Hash::new(b"context fixture ordinary writes"),
                Hash::new(b"context fixture executed block wire"),
            ),
            signers: vec![0, 1, 2, 3],
            aggregate_signature: vec![0xA5; 48],
        };
        let validator_set_pops = vec![vec![0xA6]; context.roster.len()];
        wire::finality::V2FinalityArtifact::new(context, subject, commit_qc, validator_set_pops)
    }

    #[test]
    fn non_boundary_successor_copies_frozen_election_inputs_exactly() {
        let parent_context = genesis(wire::ConsensusMode::Npos, &[1, 1, 1, 1], 3);
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
        let parent_context = genesis(wire::ConsensusMode::Npos, &[1, 1, 1, 1], 1);
        let next_roster = roster(&[1, 1, 1, 1]);
        let snapshot = wire::finality::FinalizedNextEpochSnapshot {
            epoch: parent_context.epoch + 1,
            epoch_end_height: 5,
            mode: parent_context.mode,
            quorum: wire::DualQuorum::from_roster(&next_roster).expect("next quorum"),
            validator_set_pops: vec![vec![0x78]; next_roster.len()],
            roster: next_roster.clone(),
            leader_seed: [0x77; 32],
        };
        let parent = artifact(parent_context, Some(snapshot));
        let successor = build_successor_height_context(&parent, Hash::new(b"next lanes"), None)
            .expect("epoch successor");
        assert_eq!(successor.height, 2);
        assert_eq!(successor.epoch, 5);
        assert_eq!(successor.epoch_end_height, 5);
        assert_eq!(successor.roster, next_roster);
        assert_eq!(successor.leader_seed, [0x77; 32]);
    }

    #[test]
    fn successor_epoch_end_and_pops_come_only_from_the_authenticated_parent() {
        let non_boundary = artifact(genesis(wire::ConsensusMode::Npos, &[1, 1, 1, 1], 3), None);
        let unchanged = build_successor_height_context(&non_boundary, Hash::new(b"lanes"), None)
            .expect("non-boundary successor");
        assert_eq!(unchanged.epoch_end_height, 3);
        assert_eq!(unchanged.roster, non_boundary.height_context.roster);

        let boundary_context = genesis(wire::ConsensusMode::Npos, &[1, 1, 1, 1], 1);
        let next_pops = vec![vec![0x1A]; boundary_context.roster.len()];
        let snapshot = wire::finality::FinalizedNextEpochSnapshot {
            epoch: boundary_context.epoch + 1,
            epoch_end_height: 9,
            mode: boundary_context.mode,
            roster: boundary_context.roster.clone(),
            quorum: boundary_context.quorum,
            validator_set_pops: next_pops.clone(),
            leader_seed: [0x19; 32],
        };
        let boundary = artifact(boundary_context, Some(snapshot));
        let rotated = build_successor_height_context(&boundary, Hash::new(b"lanes"), None)
            .expect("boundary successor");
        assert_eq!(rotated.epoch_end_height, 9);
        assert_eq!(
            boundary
                .height_context
                .next_epoch_snapshot
                .as_ref()
                .expect("boundary snapshot")
                .validator_set_pops,
            next_pops
        );
    }

    #[test]
    fn next_epoch_snapshot_obeys_successor_key_activation_and_expiry() {
        const BOUNDARY_HEIGHT: u64 = 7;
        const SUCCESSOR_HEIGHT: u64 = BOUNDARY_HEIGHT + 1;

        let mut keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic BLS validator")
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

        let chain_id = ChainId::from("v2-expiry-boundary-test");
        let state_with_lifecycle = |expire_first: bool| {
            let mut world = World::new();
            for (index, key) in keys.iter().enumerate() {
                let id =
                    ConsensusKeyId::new(ConsensusKeyRole::Validator, format!("validator{index}"));
                let record = ConsensusKeyRecord {
                    id: id.clone(),
                    public_key: key.public_key().clone(),
                    pop: Some(
                        iroha_crypto::bls_normal_pop_prove(key.private_key())
                            .expect("valid BLS proof of possession"),
                    ),
                    activation_height: if index == 1 { SUCCESSOR_HEIGHT } else { 0 },
                    expiry_height: (expire_first && index == 0).then_some(SUCCESSOR_HEIGHT),
                    hsm: None,
                    replaces: None,
                    status: if index == 1 {
                        ConsensusKeyStatus::Pending
                    } else {
                        ConsensusKeyStatus::Active
                    },
                };
                world.consensus_keys.insert(id.clone(), record.clone());
                world
                    .consensus_keys_by_pk
                    .insert(record.public_key.to_string(), vec![id]);
            }
            State::new_with_chain_for_testing(
                world,
                Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test(),
                chain_id.clone(),
            )
        };

        let expiring_state = state_with_lifecycle(true);
        let expiring_view = expiring_state.view();
        let expiring_peer = &roster[0].validator;
        assert!(
            live_consensus_key_pop_for_peer(expiring_view.world(), expiring_peer, BOUNDARY_HEIGHT)
                .is_some(),
            "fixture key must still authenticate the boundary height"
        );
        assert!(
            live_consensus_key_pop_for_peer(expiring_view.world(), expiring_peer, SUCCESSOR_HEIGHT)
                .is_none(),
            "a key is expired at its exclusive expiry height"
        );
        let scheduled_peer = &roster[1].validator;
        assert!(
            live_consensus_key_pop_for_peer(expiring_view.world(), scheduled_peer, BOUNDARY_HEIGHT)
                .is_none(),
            "a scheduled key must not activate early"
        );
        assert!(
            live_consensus_key_pop_for_peer(
                expiring_view.world(),
                scheduled_peer,
                SUCCESSOR_HEIGHT
            )
            .is_some(),
            "Pending is a durable schedule and becomes live at activation height"
        );

        let election = FrozenElectionInputs {
            epoch: 4,
            epoch_end_height: BOUNDARY_HEIGHT,
            mode: wire::ConsensusMode::Permissioned,
            roster,
            leader_seed: [0x72; 32],
        };
        assert!(matches!(
            finalized_next_epoch_snapshot(
                &expiring_view,
                expiring_view.network_id(),
                BOUNDARY_HEIGHT,
                &election,
            ),
            Err(V2ContextBuildError::MissingNextEpochProofOfPossession)
        ));

        let activating_state = state_with_lifecycle(false);
        let activating_view = activating_state.view();
        let snapshot = finalized_next_epoch_snapshot(
            &activating_view,
            activating_view.network_id(),
            BOUNDARY_HEIGHT,
            &election,
        )
        .expect("derive successor snapshot")
        .expect("boundary snapshot");
        assert_eq!(snapshot.roster, election.roster);
        assert_eq!(snapshot.validator_set_pops.len(), election.roster.len());
        wire::finality::verify_validator_power_roster_pops(
            &snapshot.roster,
            &snapshot.validator_set_pops,
        )
        .expect("newly activated key is cryptographically admitted");
    }

    #[test]
    fn npos_boundary_fails_closed_without_authenticated_pre_boundary_record() {
        const BOUNDARY_HEIGHT: u64 = 7;
        let chain_id = ChainId::from("v2-npos-missing-pre-boundary-record");
        let world = World::new();
        {
            let mut block = world.block();
            let mut params = SumeragiNposParameters::default();
            params.epoch_length_blocks = NonZeroU64::new(7).expect("non-zero epoch");
            params.vrf_commit_window_blocks = 2;
            params.vrf_reveal_window_blocks = 2;
            block.parameters.get_mut().custom.insert(
                SumeragiNposParameters::parameter_id(),
                params.into_custom_parameter(),
            );
            block.commit();
        }
        let state = State::new_with_chain_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            chain_id.clone(),
        );
        let view = state.view();
        let election = FrozenElectionInputs {
            epoch: 3,
            epoch_end_height: BOUNDARY_HEIGHT,
            mode: wire::ConsensusMode::Npos,
            roster: roster(&[1, 1, 1, 1]),
            leader_seed: [0x63; 32],
        };

        assert_eq!(
            finalized_next_epoch_snapshot(&view, view.network_id(), BOUNDARY_HEIGHT, &election,),
            Err(V2ContextBuildError::MissingPreBoundaryVrfRecord)
        );
    }

    #[test]
    fn genesis_rejects_non_unit_consensus_power() {
        let error = build_genesis_height_context(GenesisContextInputs {
            network_id: test_network_id(0x43),
            election: FrozenElectionInputs {
                epoch: 0,
                epoch_end_height: 10,
                mode: wire::ConsensusMode::Permissioned,
                roster: roster(&[1, 2, 1, 1]),
                leader_seed: [0; 32],
            },
            next_epoch_snapshot: None,
            nexus_amx_context_hash: Hash::new(b"nexus amx context"),
            execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 4096,
                max_chunk_count: 8,
            },
        })
        .expect_err("consensus power must be one");
        assert!(matches!(
            error,
            V2ContextBuildError::Wire(wire::ValidationError::VotingPowerNotOne)
        ));
    }

    #[test]
    fn terminal_height_never_derives_an_unrepresentable_epoch_snapshot() {
        let chain_id = ChainId::from("terminal-v2-context-builder-test");
        let state = State::new_with_chain_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            chain_id.clone(),
        );
        let view = state.view();
        let election = FrozenElectionInputs {
            epoch: u64::MAX,
            epoch_end_height: u64::MAX,
            mode: wire::ConsensusMode::Permissioned,
            roster: Vec::new(),
            leader_seed: [0x7A; 32],
        };

        assert_eq!(
            finalized_next_epoch_snapshot(&view, view.network_id(), u64::MAX, &election),
            Ok(None),
            "terminal construction must not inspect or increment next-epoch state"
        );
    }
}
