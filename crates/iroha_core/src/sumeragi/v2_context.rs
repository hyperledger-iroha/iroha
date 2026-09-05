//! Canonical construction of immutable Sumeragi v2 height contexts.
//!
//! The reducer never reads mutable world state. Genesis inputs and finalized
//! epoch snapshots enter here once, and every non-boundary successor carries
//! the previous frozen election inputs unchanged.
use super::{
    stake_snapshot::{StrictV2StakeSnapshotError, strict_v2_voting_roster},
    v2::VerifiedHeightContext,
};
use crate::{
    beacon::{
        GlobalThresholdBeaconSessionBindingV1, global_threshold_beacon_npos_successor_seed_v1,
        validate_global_threshold_beacon_session_v1,
        validate_persisted_global_threshold_beacon_pulse_v1,
        verify_finalized_global_threshold_beacon_pulse_v1,
    },
    smartcontracts::isi::staking::validator_election_eligible_at_height,
    state::{
        GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, StateBlock, StateReadOnly, WorldReadOnly,
        epoch_validator_peer_ids_from_world_with_seed, live_consensus_key_pop_for_peer_with_role,
        nexus_active_lane_ids, public_lane_validator_record_matches_key,
    },
};
use iroha_crypto::{Algorithm, Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    block::{SignedBlock, consensus_v2 as wire},
    consensus::ConsensusKeyRole,
    isi::RegisterBox,
    isi::kagemusha_v1::KagemushaMintFinalityEpochRosterV1,
    parameter::system::ConsensusHandshakeMetadata,
    peer::PeerId,
    transaction::Executable,
};
use iroha_genesis::GenesisBlock;
use mv::storage::StorageReadOnly;
use norito::codec::Encode;
use std::collections::BTreeMap;
use thiserror::Error;

/// Verified height-one inputs retained until the production reducer opens its
/// safety WAL.
pub struct GenesisV2Bootstrap {
    verified_context: VerifiedHeightContext,
    staged_nexus_amx_context: StagedGenesisNexusAmxContext,
    authenticated_genesis: AuthenticatedGenesisBodyV1,
}
/// Move-only signed genesis body authenticated by the height-one bootstrap.
///
/// The signed block is copied only while [`freeze_staged_genesis_v2`] still
/// owns the validated genesis wrapper. Production recovery then moves this
/// seal into the lifecycle launch; no raw signed block can be substituted at
/// that boundary.
#[must_use = "authenticated genesis must remain sealed until lifecycle launch"]
pub(in crate::sumeragi) struct AuthenticatedGenesisBodyV1 {
    signed_block: SignedBlock,
    authority: iroha_crypto::PublicKey,
}
impl AuthenticatedGenesisBodyV1 {
    fn authenticate(genesis: &GenesisBlock) -> Result<Self, V2GenesisBootstrapError> {
        let mut transactions = genesis.0.external_transactions();
        let first = transactions
            .next()
            .ok_or(V2GenesisBootstrapError::MissingGenesisAuthority)?;
        let authority = first
            .authority()
            .try_signatory()
            .cloned()
            .ok_or(V2GenesisBootstrapError::NonCanonicalGenesisAuthority)?;
        if transactions
            .any(|transaction| transaction.authority().try_signatory() != Some(&authority))
        {
            return Err(V2GenesisBootstrapError::NonCanonicalGenesisAuthority);
        }
        Ok(Self {
            signed_block: genesis.0.clone(),
            authority,
        })
    }
    /// Borrow the exact authenticated body for executor installation.
    pub(in crate::sumeragi) const fn signed_block(&self) -> &SignedBlock {
        &self.signed_block
    }
    /// Compare the retained canonical genesis authority with recovery's key.
    pub(in crate::sumeragi) fn authorizes(&self, authority: &iroha_crypto::PublicKey) -> bool {
        &self.authority == authority
    }
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
    /// Borrow the exact PoPs authenticated with the staged height-one roster.
    #[must_use]
    pub fn proofs_of_possession(&self) -> &[Vec<u8>] {
        self.verified_context.proofs_of_possession()
    }
    pub(in crate::sumeragi) fn into_parts(
        self,
    ) -> (
        VerifiedHeightContext,
        StagedGenesisNexusAmxContext,
        AuthenticatedGenesisBodyV1,
    ) {
        (
            self.verified_context,
            self.staged_nexus_amx_context,
            self.authenticated_genesis,
        )
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
    let signed_network_id = NetworkId::from_genesis_hash(genesis.0.hash());
    if context.network_id != signed_network_id {
        return Err(V2GenesisBootstrapError::FinalityVotingAuthorityMismatch);
    }
    let metadata = iroha_genesis::signed_genesis_consensus_metadata(&genesis.0)
        .map_err(|error| V2GenesisBootstrapError::Context(error.to_string()))?;
    if context.mode != wire::ConsensusMode::from(metadata.mode)
        || context.da_layout != metadata.sumeragi_v2.da_layout
        || *context.nexus_amx_context_hash.as_ref() != metadata.sumeragi_v2.nexus_amx_context_hash
        || *context.execution_policy_hash.as_ref() != metadata.sumeragi_v2.execution_policy_hash
    {
        return Err(V2GenesisBootstrapError::FinalityVotingAuthorityMismatch);
    }
    let (signed_mint_roster, signed_next_mint_roster) =
        bind_signed_mint_finality_rosters(&metadata, signed_network_id)?;
    if signed_mint_roster != context.kagemusha_mint_finality_epoch_roster
        || signed_next_mint_roster
            != context
                .next_epoch_snapshot
                .as_ref()
                .map(|snapshot| snapshot.kagemusha_mint_finality_epoch_roster.clone())
    {
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

fn bind_signed_mint_finality_rosters(
    metadata: &ConsensusHandshakeMetadata,
    network_id: NetworkId,
) -> Result<
    (
        KagemushaMintFinalityEpochRosterV1,
        Option<KagemushaMintFinalityEpochRosterV1>,
    ),
    V2GenesisBootstrapError,
> {
    let bind = |template: &iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityEpochRosterTemplateV1|
     -> Result<KagemushaMintFinalityEpochRosterV1, V2GenesisBootstrapError> {
        let roster = template
            .bind_network_id(network_id)
            .map_err(|error| V2GenesisBootstrapError::Context(error.to_string()))?;
        crate::zk::kagemusha_v1_recursion::validate_kagemusha_mint_finality_roster_keys_v1(
            &roster,
        )
        .map_err(|error| V2GenesisBootstrapError::Context(error.to_string()))?;
        Ok(roster)
    };
    let epoch_roster = bind(&metadata.kagemusha_mint_finality.epoch_roster)?;
    let next_epoch_roster = metadata
        .kagemusha_mint_finality
        .next_epoch_roster
        .as_ref()
        .map(bind)
        .transpose()?;
    Ok((epoch_roster, next_epoch_roster))
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
    let expected = freeze_staged_genesis_v2(genesis, staged, context.mode)?;
    let (verified, _, _) = expected.into_parts();
    if verified.context() != context || verified.proofs_of_possession() != validator_set_pops {
        return Err(V2GenesisBootstrapError::FinalityVotingAuthorityMismatch);
    }
    Ok(())
}
/// Freeze and cryptographically verify the height-one context from a validated
/// but uncommitted genesis state block.
///
/// The function decodes the Sumeragi and Kagemusha inputs directly from the
/// signed genesis handshake. It never accepts either from mutable runtime
/// configuration or an attacker-controlled persisted height context.
pub fn freeze_staged_genesis_v2(
    genesis: &GenesisBlock,
    staged: &StateBlock<'_>,
    mode: wire::ConsensusMode,
) -> Result<GenesisV2Bootstrap, V2GenesisBootstrapError> {
    let signed_metadata = iroha_genesis::signed_genesis_consensus_metadata(&genesis.0)
        .map_err(|error| V2GenesisBootstrapError::Context(error.to_string()))?;
    if wire::ConsensusMode::from(signed_metadata.mode) != mode {
        return Err(V2GenesisBootstrapError::SignedConsensusModeMismatch);
    }
    let signed_parameters = signed_metadata.sumeragi_v2;
    signed_parameters.validate()?;
    let authenticated_genesis = AuthenticatedGenesisBodyV1::authenticate(genesis)?;
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
        let staged_pop = live_consensus_key_pop_for_peer_with_role(
            staged_world,
            voter,
            1,
            ConsensusKeyRole::Validator,
        )
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
        wire::ConsensusMode::Npos => strict_v2_voting_roster(staged_world, &voters, None, 1)
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
    if network_id != NetworkId::from_genesis_hash(genesis.0.hash()) {
        return Err(V2GenesisBootstrapError::StagedNetworkIdMismatch);
    }
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
    let (signed_mint_roster, signed_next_mint_roster) =
        bind_signed_mint_finality_rosters(&signed_metadata, network_id)?;
    if signed_mint_roster.epoch != 0
        || signed_mint_roster.validators.len() != roster.len()
        || signed_mint_roster
            .validators
            .iter()
            .zip(&roster)
            .any(|(mint, consensus)| mint.validator != consensus.validator)
    {
        return Err(V2GenesisBootstrapError::InvalidSignedMintFinalityRoster);
    }
    let election = FrozenElectionInputs {
        epoch: 0,
        kagemusha_mint_finality_epoch_roster: signed_mint_roster,
        epoch_end_height,
        mode,
        roster,
        leader_seed,
    };
    let next_epoch_snapshot = if election.epoch_end_height == 1 {
        let roster = signed_next_mint_roster.ok_or_else(|| {
            V2GenesisBootstrapError::Context(
                V2ContextBuildError::MissingNextKagemushaMintFinalityEpochId.to_string(),
            )
        })?;
        Some(
            finalized_next_epoch_snapshot_with_roster(staged, &network_id, 1, &election, roster)
                .map_err(|error| V2GenesisBootstrapError::Context(error.to_string()))?,
        )
    } else {
        if signed_next_mint_roster.is_some() {
            return Err(V2GenesisBootstrapError::Context(
                V2ContextBuildError::UnexpectedNextKagemushaMintFinalityEpochRoster.to_string(),
            ));
        }
        None
    };
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
        authenticated_genesis,
    })
}
/// Extract the canonical voting identities and exact PoPs signed into genesis.
pub fn signed_genesis_validator_pops(
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
/// canonically ordered public-lane validator records whose retained tenure
/// contains height one, and the complete retained lane-incarnation lineage,
/// including retired lane identifiers.
#[must_use]
pub fn staged_genesis_nexus_amx_context_hash(staged: &StateBlock<'_>) -> Hash {
    const GENESIS_CONTEXT_HEIGHT: wire::Height = 1;
    let eligible_validators = staged
        .world()
        .public_lane_validators()
        .iter()
        .filter(|(key, record)| public_lane_validator_record_matches_key(key, record))
        .filter(|(_, record)| validator_election_eligible_at_height(record, GENESIS_CONTEXT_HEIGHT))
        .map(|(key, record)| (key.clone(), record.clone()))
        .collect::<Vec<_>>();
    let retained_lane_lineage = staged
        .lane_incarnation_lineage_for_snapshot()
        .iter()
        .map(
            |(&lane_id, lineage)| iroha_config::parameters::actual::SumeragiV2LaneLifecycleEntry {
                lane_id,
                generation: lineage.generation,
                incarnation: lineage.incarnation,
                activation_height: lineage.activation_height,
            },
        )
        .collect::<Vec<_>>();
    iroha_config::parameters::actual::sumeragi_v2_nexus_amx_context_hash(
        &staged.nexus,
        &staged.pipeline,
        &eligible_validators,
        &retained_lane_lineage,
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
/// Returns an error if the Nexus policy has no authenticated runtime policy set.
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
    /// Signed genesis omitted the canonical root transaction authority.
    #[error("Sumeragi v2 genesis has no canonical root authority")]
    MissingGenesisAuthority,
    /// Signed genesis transactions do not share one canonical single-key authority.
    #[error("Sumeragi v2 genesis root authority is not one canonical single key")]
    NonCanonicalGenesisAuthority,
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
    /// The mode supplied by staged validation disagrees with signed metadata.
    #[error("staged Sumeragi v2 mode differs from signed genesis metadata")]
    SignedConsensusModeMismatch,
    /// The signed networkless Pasta template disagrees with the exact frozen voter roster.
    #[error("signed Kagemusha mint-finality roster differs from the frozen consensus roster")]
    InvalidSignedMintFinalityRoster,
    /// Staged execution used a network identity other than the final signed genesis hash.
    #[error("staged network identity differs from the final signed genesis hash")]
    StagedNetworkIdMismatch,
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
    /// Identifier of the paired-Pasta mint-finality roster for this epoch.
    pub kagemusha_mint_finality_epoch_roster:
        iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityEpochRosterV1,
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
    let quorum = inputs.election.quorum()?;
    let kagemusha_mint_finality_epoch_id = inputs
        .election
        .kagemusha_mint_finality_epoch_roster
        .finality_epoch_id()
        .map_err(|_| V2ContextBuildError::InvalidKagemushaMintFinalityEpochRoster)?;
    let context = wire::HeightContext {
        network_id: inputs.network_id,
        protocol_version: wire::PROTOCOL_VERSION,
        height: 1,
        epoch: inputs.election.epoch,
        kagemusha_mint_finality_epoch_id,
        kagemusha_mint_finality_epoch_roster: inputs.election.kagemusha_mint_finality_epoch_roster,
        epoch_end_height: inputs.election.epoch_end_height,
        next_epoch_snapshot: inputs.next_epoch_snapshot,
        mode: inputs.election.mode,
        parent_commit_qc: None,
        snapshot_bootstrap: None,
        quorum,
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
    let quorum = election.quorum()?;
    let kagemusha_mint_finality_epoch_id = election
        .kagemusha_mint_finality_epoch_roster
        .finality_epoch_id()
        .map_err(|_| V2ContextBuildError::InvalidKagemushaMintFinalityEpochRoster)?;
    let context = wire::HeightContext {
        network_id: parent.height_context.network_id,
        protocol_version: wire::PROTOCOL_VERSION,
        height,
        epoch: election.epoch,
        kagemusha_mint_finality_epoch_id,
        kagemusha_mint_finality_epoch_roster: election.kagemusha_mint_finality_epoch_roster,
        epoch_end_height: election.epoch_end_height,
        next_epoch_snapshot,
        mode: election.mode,
        parent_commit_qc: Some(parent.commit_qc.clone()),
        snapshot_bootstrap: None,
        quorum,
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
            kagemusha_mint_finality_epoch_roster: snapshot
                .kagemusha_mint_finality_epoch_roster
                .clone(),
            epoch_end_height: snapshot.epoch_end_height,
            mode: snapshot.mode,
            roster: snapshot.roster.clone(),
            leader_seed: snapshot.leader_seed,
        },
        None => FrozenElectionInputs {
            epoch: parent.height_context.epoch,
            kagemusha_mint_finality_epoch_roster: parent
                .height_context
                .kagemusha_mint_finality_epoch_roster
                .clone(),
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

/// Resolve the exact finalized global-beacon pulse authorized to seed one NPoS
/// successor epoch.
///
/// The pulse must be the unique history tail finalized in the last committed
/// pre-boundary block. It must authenticate the immediately preceding canonical
/// block hash, the key session active at the pulse height, and the exact network. Full
/// threshold-BLS verification is repeated at this consensus consumption
/// boundary so restored or directly seeded state cannot turn a shape-valid
/// pulse into authoritative entropy.
pub(crate) fn finalized_global_beacon_npos_successor_seed_from_sources(
    world: &impl WorldReadOnly,
    block_hashes: &[HashOf<iroha_data_model::block::BlockHeader>],
    network_id: &NetworkId,
    boundary_height: wire::Height,
    successor_epoch: u64,
) -> Result<[u8; 32], V2ContextBuildError> {
    let pulse_height = boundary_height
        .checked_sub(1)
        .ok_or(V2ContextBuildError::InvalidPreBoundaryBeaconPulse)?;
    let anchor_height = pulse_height
        .checked_sub(1)
        .ok_or(V2ContextBuildError::InvalidPreBoundaryBeaconPulse)?;

    let mut exact_height = world
        .global_beacon_pulses()
        .iter()
        .filter(|(_, pulse)| pulse.height == pulse_height);
    let (storage_key, pulse) = exact_height
        .next()
        .ok_or(V2ContextBuildError::MissingPreBoundaryBeaconPulse)?;
    if exact_height.next().is_some()
        || storage_key != &pulse.pulse_id
        || world
            .global_beacon_pulses()
            .iter()
            .any(|(_, candidate)| candidate.height > pulse_height)
    {
        return Err(V2ContextBuildError::InvalidPreBoundaryBeaconPulse);
    }

    let committed_height = u64::try_from(block_hashes.len())
        .map_err(|_| V2ContextBuildError::InvalidPreBoundaryBeaconPulse)?;
    let anchor_index = usize::try_from(anchor_height)
        .ok()
        .and_then(|height| height.checked_sub(1))
        .ok_or(V2ContextBuildError::InvalidPreBoundaryBeaconPulse)?;
    let anchor_hash = block_hashes
        .get(anchor_index)
        .copied()
        .ok_or(V2ContextBuildError::InvalidPreBoundaryBeaconPulse)?;
    let expected_anchor = iroha_data_model::consensus::GlobalThresholdBeaconChainAnchorV1 {
        height: anchor_height,
        block_hash: anchor_hash,
    };
    if committed_height != pulse_height
        || pulse.network_id != *network_id
        || pulse.finalized_chain_anchor != expected_anchor
    {
        return Err(V2ContextBuildError::InvalidPreBoundaryBeaconPulse);
    }

    let key_session = world
        .global_beacon_key_sessions()
        .get(&pulse.session_id)
        .ok_or(V2ContextBuildError::InvalidPreBoundaryBeaconPulse)?;
    if !key_session.is_active_at(pulse.height) {
        return Err(V2ContextBuildError::InvalidPreBoundaryBeaconPulse);
    }
    let binding = GlobalThresholdBeaconSessionBindingV1 {
        network_id: *network_id,
        session_id: pulse.session_id,
        roster_hash: pulse.roster_hash,
        transcript_hash: pulse.transcript_hash,
    };
    let validated_session =
        validate_global_threshold_beacon_session_v1(key_session.session.clone(), &binding)
            .map_err(|_| V2ContextBuildError::InvalidPreBoundaryBeaconPulse)?;

    let verified_link = verify_finalized_global_threshold_beacon_pulse_v1(
        &validated_session,
        pulse,
        expected_anchor,
    )
    .map_err(|_| V2ContextBuildError::InvalidPreBoundaryBeaconPulse)?;
    let canonical_link = validate_persisted_global_threshold_beacon_pulse_v1(pulse)
        .map_err(|_| V2ContextBuildError::InvalidPreBoundaryBeaconPulse)?;
    if verified_link != canonical_link
        || world
            .global_beacon_latest_pulse()
            .get(&GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY)
            != Some(&verified_link)
    {
        return Err(V2ContextBuildError::InvalidPreBoundaryBeaconPulse);
    }

    let seed =
        global_threshold_beacon_npos_successor_seed_v1(pulse, boundary_height, successor_epoch);
    if seed == [0; 32] {
        return Err(V2ContextBuildError::InvalidPreBoundaryBeaconPulse);
    }
    Ok(seed)
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
    let kagemusha_mint_finality_epoch_roster = state
        .world()
        .kagemusha_mint_finality_next_epoch_parameter()
        .ok_or(V2ContextBuildError::MissingNextKagemushaMintFinalityEpochId)?
        .roster;
    finalized_next_epoch_snapshot_with_roster(
        state,
        network_id,
        height,
        election,
        kagemusha_mint_finality_epoch_roster,
    )
    .map(Some)
}

fn finalized_next_epoch_snapshot_with_roster(
    state: &impl StateReadOnly,
    network_id: &NetworkId,
    height: wire::Height,
    election: &FrozenElectionInputs,
    kagemusha_mint_finality_epoch_roster: iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityEpochRosterV1,
) -> Result<wire::finality::FinalizedNextEpochSnapshot, V2ContextBuildError> {
    let successor_height = height
        .checked_add(1)
        .ok_or(V2ContextBuildError::HeightOverflow)?;
    let epoch = election
        .epoch
        .checked_add(1)
        .ok_or(V2ContextBuildError::EpochOverflow)?;
    if kagemusha_mint_finality_epoch_roster.network_id != *network_id
        || kagemusha_mint_finality_epoch_roster.epoch != epoch
    {
        return Err(V2ContextBuildError::InvalidKagemushaMintFinalityEpochRoster);
    }
    let kagemusha_mint_finality_epoch_id = kagemusha_mint_finality_epoch_roster
        .finality_epoch_id()
        .map_err(|_| V2ContextBuildError::InvalidKagemushaMintFinalityEpochRoster)?;
    let npos_params = if election.mode == wire::ConsensusMode::Npos {
        Some(
            super::v2_npos::committed_epoch_length_blocks(state.world()).map_err(|error| {
                match error {
                    super::v2_npos::V2NposError::MissingCommittedParameters => {
                        V2ContextBuildError::MissingNposParameters
                    }
                    _ => V2ContextBuildError::InvalidNposParameters,
                }
            })?,
        )
    } else {
        None
    };
    let authenticated_npos_seed = if npos_params.is_some() {
        Some(finalized_global_beacon_npos_successor_seed_from_sources(
            state.world(),
            state.block_hashes(),
            network_id,
            height,
            epoch,
        )?)
    } else {
        None
    };
    let roster = match election.mode {
        wire::ConsensusMode::Permissioned => election.roster.clone(),
        wire::ConsensusMode::Npos => {
            let elected = epoch_validator_peer_ids_from_world_with_seed(
                state.world(),
                state.commit_topology().iter().cloned(),
                successor_height,
                state.nexus(),
                epoch,
                authenticated_npos_seed.expect(
                    "NPoS branch authenticates the pre-boundary beacon before roster selection",
                ),
            )
            .ok_or(V2ContextBuildError::MissingFinalizedEpochRoster)?;
            let active_lanes = nexus_active_lane_ids(state.nexus());
            strict_v2_voting_roster(
                state.world(),
                &elected,
                Some(&active_lanes),
                successor_height,
            )?
        }
    };
    if kagemusha_mint_finality_epoch_roster.validators.len() != roster.len()
        || kagemusha_mint_finality_epoch_roster
            .validators
            .iter()
            .zip(&roster)
            .any(|(mint, consensus)| mint.validator != consensus.validator)
        || crate::zk::kagemusha_v1_recursion::validate_kagemusha_mint_finality_roster_keys_v1(
            &kagemusha_mint_finality_epoch_roster,
        )
        .is_err()
    {
        return Err(V2ContextBuildError::InvalidKagemushaMintFinalityEpochRoster);
    }
    let quorum = wire::DualQuorum::from_roster(&roster)?;
    let validator_set_pops = roster
        .iter()
        .map(|entry| {
            live_consensus_key_pop_for_peer_with_role(
                state.world(),
                &entry.validator,
                successor_height,
                ConsensusKeyRole::Validator,
            )
            .ok_or(V2ContextBuildError::MissingNextEpochProofOfPossession)
        })
        .collect::<Result<Vec<_>, _>>()?;
    wire::finality::verify_validator_power_roster_pops(&roster, &validator_set_pops)
        .map_err(V2ContextBuildError::NextEpochCryptography)?;
    let epoch_end_height = match election.mode {
        wire::ConsensusMode::Permissioned => u64::MAX,
        wire::ConsensusMode::Npos => {
            let epoch_length = npos_params.expect(
                "NPoS branch validates the committed schedule before snapshot construction",
            );
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
    Ok(wire::finality::FinalizedNextEpochSnapshot {
        epoch,
        kagemusha_mint_finality_epoch_id,
        kagemusha_mint_finality_epoch_roster,
        epoch_end_height,
        mode: election.mode,
        roster,
        validator_set_pops,
        quorum,
        leader_seed,
    })
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
    /// An epoch boundary omitted its committed next paired-Pasta roster.
    #[error(
        "Sumeragi v2 epoch boundary is missing the committed Kagemusha V1 next-roster parameter"
    )]
    MissingNextKagemushaMintFinalityEpochId,
    /// Signed genesis supplied a next roster at a height which is not an epoch boundary.
    #[error("signed Sumeragi v2 genesis supplied an unused next Kagemusha V1 roster")]
    UnexpectedNextKagemushaMintFinalityEpochRoster,
    /// A supplied paired-Pasta roster is malformed or disagrees with the elected epoch.
    #[error("invalid Kagemusha mint-finality epoch roster")]
    InvalidKagemushaMintFinalityEpochRoster,
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
    /// The finalized state before an NPoS boundary omitted the exact global
    /// threshold-beacon pulse required for successor entropy.
    #[error("Sumeragi v2 NPoS boundary is missing its finalized pre-boundary beacon pulse")]
    MissingPreBoundaryBeaconPulse,
    /// The retained pre-boundary pulse is inconsistent with the exact network,
    /// active key session, finalized-chain anchor, height, or threshold signature.
    #[error("Sumeragi v2 NPoS boundary has an invalid finalized beacon pulse")]
    InvalidPreBoundaryBeaconPulse,
    /// Exact NPoS voting-power extraction failed.
    #[error(transparent)]
    Stake(#[from] StrictV2StakeSnapshotError),
    /// Selected epoch end precedes the height it would govern.
    #[error("Sumeragi v2 epoch end precedes its successor height")]
    EpochEndBeforeSuccessor,
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        ChainId, NetworkId,
        account::AccountId,
        block::{BlockHeader, SignedBlock},
        consensus::{ConsensusKeyId, ConsensusKeyRecord, ConsensusKeyRole, ConsensusKeyStatus},
        isi::{RegisterCommitteePeerWithPop, RegisterPeerWithPop, SetParameter},
        metadata::Metadata,
        nexus::{
            DataSpaceCatalog, DataSpaceId, DataSpaceMetadata, LaneId, PublicLaneValidatorRecord,
            PublicLaneValidatorStatus,
        },
        parameter::{
            Parameter,
            custom::CustomParameter,
            system::{
                ConsensusFingerprint, ConsensusHandshakeMetadata, SumeragiConsensusMode,
                SumeragiNposParameters, consensus_metadata,
            },
        },
        peer::PeerId,
        prelude::{InstructionBox, TransactionBuilder},
    };
    use iroha_genesis::GenesisBlock;
    use iroha_primitives::{json::Json, numeric::Quantity};
    use std::num::NonZeroU64;
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
        let network_id = test_network_id(0x41);
        let kagemusha_mint_finality_epoch_roster =
            crate::kagemusha_v1_test_fixtures::mint_finality_roster(network_id, 4, &roster);
        let next_epoch_snapshot = (end == 1).then(|| {
            let next_roster = roster.clone();
            let (kagemusha_mint_finality_epoch_id, kagemusha_mint_finality_epoch_roster) =
                crate::kagemusha_v1_test_fixtures::mint_finality_roster_and_id(
                    network_id,
                    5,
                    &next_roster,
                );
            wire::finality::FinalizedNextEpochSnapshot {
                epoch: 5,
                kagemusha_mint_finality_epoch_id,
                kagemusha_mint_finality_epoch_roster,
                epoch_end_height: 5,
                mode,
                quorum: wire::DualQuorum::from_roster(&next_roster).expect("next quorum"),
                validator_set_pops: vec![vec![0x43]; next_roster.len()],
                roster: next_roster,
                leader_seed: [0x42; 32],
            }
        });
        build_genesis_height_context(GenesisContextInputs {
            network_id,
            election: FrozenElectionInputs {
                epoch: 4,
                kagemusha_mint_finality_epoch_roster,
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
        signed_roster_genesis_with_extra(voters, duplicate_first, corrupt_first_pop, Vec::new())
    }
    fn signed_roster_genesis_with_extra(
        voters: &[KeyPair],
        duplicate_first: bool,
        corrupt_first_pop: bool,
        extra_instructions: Vec<InstructionBox>,
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
        instructions.extend(extra_instructions);
        if voters.len() == 4 && !duplicate_first && !corrupt_first_pop {
            let mut roster = voters
                .iter()
                .map(|key| wire::ValidatorPower {
                    validator: PeerId::new(key.public_key().clone()),
                    power: 1,
                })
                .collect::<Vec<_>>();
            roster.sort_by(|left, right| left.validator.cmp(&right.validator));
            let metadata = ConsensusHandshakeMetadata {
                mode: SumeragiConsensusMode::Permissioned,
                block_cadence_ms: NonZeroU64::new(1_000).expect("non-zero test cadence"),
                wire_protocol_version: u32::from(wire::PROTOCOL_VERSION),
                consensus_fingerprint: ConsensusFingerprint::new([0xA5; 32]),
                kagemusha_mint_finality:
                    crate::kagemusha_v1_test_fixtures::mint_finality_genesis_parameters(&roster),
                sumeragi_v2: crate::kagemusha_v1_test_fixtures::genesis_context_parameters(),
            };
            metadata
                .validate()
                .expect("valid signed genesis consensus metadata fixture");
            let metadata = norito::json::value::to_value(&metadata)
                .expect("serialize signed genesis consensus metadata fixture");
            let metadata = Json::from_norito_value_ref(&metadata)
                .expect("encode signed genesis consensus metadata fixture");
            instructions.push(InstructionBox::from(SetParameter::new(Parameter::Custom(
                CustomParameter::new(consensus_metadata::handshake_meta_id(), metadata),
            ))));
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
    fn authenticated_genesis_body_retains_exact_block_and_authority() {
        let voter = KeyPair::try_from_seed(vec![0x31; 32], Algorithm::BlsNormal)
            .expect("deterministic BLS voter");
        let genesis = signed_roster_genesis(std::slice::from_ref(&voter), false, false);
        let authority =
            KeyPair::try_from_seed(b"v2-context-genesis-authority".to_vec(), Algorithm::Ed25519)
                .expect("deterministic genesis authority");
        let foreign = KeyPair::try_from_seed(
            b"v2-context-foreign-genesis-authority".to_vec(),
            Algorithm::Ed25519,
        )
        .expect("deterministic foreign authority");
        let authenticated = AuthenticatedGenesisBodyV1::authenticate(&genesis)
            .expect("validated genesis seals its exact signed body");
        assert_eq!(authenticated.signed_block(), &genesis.0);
        assert!(authenticated.authorizes(authority.public_key()));
        assert!(!authenticated.authorizes(foreign.public_key()));
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
    fn signed_genesis_roster_ignores_proof_bound_committee_peers() {
        let voters = [0x61_u8, 0x62, 0x63, 0x64].map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic BLS voter")
        });
        let committee = KeyPair::try_from_seed(vec![0x65; 32], Algorithm::BlsNormal)
            .expect("deterministic BLS committee peer");
        let committee_peer = PeerId::new(committee.public_key().clone());
        let committee_pop = iroha_crypto::bls_normal_pop_prove(committee.private_key())
            .expect("committee PoP fixture");
        let genesis = signed_roster_genesis_with_extra(
            &voters,
            false,
            false,
            vec![InstructionBox::from(RegisterCommitteePeerWithPop::new(
                committee_peer.clone(),
                committee_pop,
            ))],
        );

        let observed = signed_genesis_voting_peers(&genesis).expect("signed global roster");
        let mut expected = voters
            .iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect::<Vec<_>>();
        expected.sort();
        assert_eq!(observed, expected);
        assert!(!observed.contains(&committee_peer));
        assert!(
            !signed_genesis_validator_pops(&genesis)
                .expect("signed validator PoPs")
                .contains_key(&committee_peer),
            "committee peer registrations must never widen the signed global voter roster"
        );
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
            let network_id = NetworkId::from_genesis_hash(genesis.0.hash());
            let (kagemusha_mint_finality_epoch_id, kagemusha_mint_finality_epoch_roster) =
                crate::kagemusha_v1_test_fixtures::mint_finality_roster_and_id(
                    network_id, 0, &roster,
                );
            let signed_parameters = crate::kagemusha_v1_test_fixtures::genesis_context_parameters();
            let context = wire::HeightContext {
                network_id,
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
                kagemusha_mint_finality_epoch_id,
                kagemusha_mint_finality_epoch_roster,
                nexus_amx_context_hash: Hash::prehashed(signed_parameters.nexus_amx_context_hash),
                execution_policy_hash: Hash::prehashed(signed_parameters.execution_policy_hash),
                da_layout: signed_parameters.da_layout,
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
            freeze_staged_genesis_v2(&genesis, &staged, wire::ConsensusMode::Permissioned,),
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
            activation_height: 1,
            deactivation_height: None,
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
    fn staged_context_hash_with_record(record: PublicLaneValidatorRecord) -> Hash {
        let state = lane_hash_world(&[]);
        let mut block = state.block(BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero test height"),
            None,
            None,
            None,
            0,
            0,
        ));
        block
            .world
            .public_lane_validators
            .insert((record.lane_id, record.validator.clone()), record);
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
    fn staged_genesis_hash_uses_height_one_half_open_validator_tenure() {
        let peer = PeerId::new(
            KeyPair::try_from_seed(vec![0x64; 32], Algorithm::BlsNormal)
                .expect("validator")
                .public_key()
                .clone(),
        );
        let lane = LaneId::new(3);
        let empty_hash = staged_context_hash(&lane_hash_world(&[]));
        let mut record = lane_record(&peer, lane, 7);

        record.status = PublicLaneValidatorStatus::PendingActivation(1);
        assert_ne!(
            staged_context_hash_with_record(record.clone()),
            empty_hash,
            "a due pending label cannot suppress height-one tenure"
        );

        record.status = PublicLaneValidatorStatus::Exiting(u64::MAX);
        record.deactivation_height = Some(2);
        assert_ne!(
            staged_context_hash_with_record(record.clone()),
            empty_hash,
            "an exiting label cannot suppress retained height-one tenure"
        );

        record.status = PublicLaneValidatorStatus::Slashed(Hash::new(b"height-one slash"));
        assert_ne!(
            staged_context_hash_with_record(record.clone()),
            empty_hash,
            "a slashed label cannot suppress retained height-one tenure"
        );

        record.status = PublicLaneValidatorStatus::Exiting(u64::MAX);
        record.deactivation_height = Some(1);
        assert_eq!(
            staged_context_hash_with_record(record),
            empty_hash,
            "the deactivation boundary is exclusive"
        );
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
            execution_commitment:
                wire::ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
                    Hash::new(b"context fixture parent state"),
                    Hash::new(b"context fixture post state"),
                    Hash::new(b"context fixture ordinary writes"),
                    1,
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
        let next_epoch = parent_context.epoch + 1;
        let (kagemusha_mint_finality_epoch_id, kagemusha_mint_finality_epoch_roster) =
            crate::kagemusha_v1_test_fixtures::mint_finality_roster_and_id(
                parent_context.network_id,
                next_epoch,
                &next_roster,
            );
        let snapshot = wire::finality::FinalizedNextEpochSnapshot {
            epoch: next_epoch,
            kagemusha_mint_finality_epoch_id,
            kagemusha_mint_finality_epoch_roster,
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
        let next_epoch = boundary_context.epoch + 1;
        let (kagemusha_mint_finality_epoch_id, kagemusha_mint_finality_epoch_roster) =
            crate::kagemusha_v1_test_fixtures::mint_finality_roster_and_id(
                boundary_context.network_id,
                next_epoch,
                &boundary_context.roster,
            );
        let snapshot = wire::finality::FinalizedNextEpochSnapshot {
            epoch: next_epoch,
            kagemusha_mint_finality_epoch_id,
            kagemusha_mint_finality_epoch_roster,
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
            live_consensus_key_pop_for_peer_with_role(
                expiring_view.world(),
                expiring_peer,
                BOUNDARY_HEIGHT,
                ConsensusKeyRole::Validator,
            )
            .is_some(),
            "fixture key must still authenticate the boundary height"
        );
        assert!(
            live_consensus_key_pop_for_peer_with_role(
                expiring_view.world(),
                expiring_peer,
                SUCCESSOR_HEIGHT,
                ConsensusKeyRole::Validator,
            )
            .is_none(),
            "a key is expired at its exclusive expiry height"
        );
        let scheduled_peer = &roster[1].validator;
        assert!(
            live_consensus_key_pop_for_peer_with_role(
                expiring_view.world(),
                scheduled_peer,
                BOUNDARY_HEIGHT,
                ConsensusKeyRole::Validator,
            )
            .is_none(),
            "a scheduled key must not activate early"
        );
        assert!(
            live_consensus_key_pop_for_peer_with_role(
                expiring_view.world(),
                scheduled_peer,
                SUCCESSOR_HEIGHT,
                ConsensusKeyRole::Validator,
            )
            .is_some(),
            "Pending is a durable schedule and becomes live at activation height"
        );
        let kagemusha_mint_finality_epoch_roster =
            crate::kagemusha_v1_test_fixtures::mint_finality_roster(
                *expiring_view.network_id(),
                4,
                &roster,
            );
        let election = FrozenElectionInputs {
            epoch: 4,
            kagemusha_mint_finality_epoch_roster,
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
    fn npos_boundary_fails_closed_without_finalized_pre_boundary_beacon_pulse() {
        const BOUNDARY_HEIGHT: u64 = 7;
        let chain_id = ChainId::from("v2-npos-missing-pre-boundary-record");
        let world = World::new();
        {
            let mut block = world.block();
            let mut params = SumeragiNposParameters::default();
            params.epoch_length_blocks = NonZeroU64::new(7).expect("non-zero epoch");
            params.evidence_horizon_blocks = 14;
            params.slashing_delay_blocks = 7;
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
        let election_roster = roster(&[1, 1, 1, 1]);
        let election = FrozenElectionInputs {
            epoch: 3,
            kagemusha_mint_finality_epoch_roster:
                crate::kagemusha_v1_test_fixtures::mint_finality_roster(
                    *view.network_id(),
                    3,
                    &election_roster,
                ),
            epoch_end_height: BOUNDARY_HEIGHT,
            mode: wire::ConsensusMode::Npos,
            roster: election_roster,
            leader_seed: [0x63; 32],
        };
        assert_eq!(
            finalized_next_epoch_snapshot(&view, view.network_id(), BOUNDARY_HEIGHT, &election,),
            Err(V2ContextBuildError::MissingPreBoundaryBeaconPulse)
        );
    }
    #[test]
    fn genesis_rejects_non_unit_consensus_power() {
        let network_id = test_network_id(0x43);
        let election_roster = roster(&[1, 2, 1, 1]);
        let error = build_genesis_height_context(GenesisContextInputs {
            network_id,
            election: FrozenElectionInputs {
                epoch: 0,
                kagemusha_mint_finality_epoch_roster:
                    crate::kagemusha_v1_test_fixtures::mint_finality_roster(
                        network_id,
                        0,
                        &election_roster,
                    ),
                epoch_end_height: 10,
                mode: wire::ConsensusMode::Permissioned,
                roster: election_roster,
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
        let mut parent_context =
            genesis(wire::ConsensusMode::Permissioned, &[1, 1, 1, 1], u64::MAX);
        parent_context.height = u64::MAX - 1;

        let mut grandparent_commit_qc = artifact(
            genesis(wire::ConsensusMode::Permissioned, &[1, 1, 1, 1], u64::MAX),
            None,
        )
        .commit_qc;
        grandparent_commit_qc.round.height = u64::MAX - 2;
        grandparent_commit_qc.proposal_round = grandparent_commit_qc.round;
        grandparent_commit_qc.signers = vec![0, 1, 2];
        parent_context.parent_commit_qc = Some(grandparent_commit_qc);

        let mut parent = artifact(parent_context, None);
        parent.commit_qc.signers = vec![0, 1, 2];
        assert_eq!(parent.height, u64::MAX - 1);
        parent
            .validate()
            .expect("MAX-1 finality artifact must be structurally valid");

        let terminal =
            build_successor_height_context(&parent, Hash::new(b"terminal nexus AMX context"), None)
                .expect("terminal successor context");
        assert_eq!(terminal.height, u64::MAX);
        assert_eq!(terminal.epoch_end_height, u64::MAX);
        assert_eq!(terminal.next_epoch_snapshot, None);
        terminal
            .validate()
            .expect("the full terminal height context must validate");
    }
}
