//! Native signed-genesis merge authority, without an obsolete global height context.

use crate::state::{StateBlock, StateReadOnly};
use iroha_crypto::Hash;
use iroha_data_model::{
    NetworkId, block::SignedBlock, parameter::system::ConsensusMode,
    sumeragi::epoch::ValidatorEpochContextV1,
};
use iroha_genesis::GenesisBlock;

/// Move-only authority of the exact signed and successfully executed genesis.
/// Its fields cannot be decoded or publicly constructed; they grant no post-genesis finality.
#[must_use = "retain exact genesis authority through launch and original-facts admission"]
pub struct GenesisMergeAuthority {
    genesis: SignedBlock,
    epoch: ValidatorEpochContextV1,
    catalog_hash: Hash,
    active_lanes: Vec<iroha_data_model::merge::MergeLaneBinding>,
    lane_authority_catalog: iroha_data_model::merge::MergeLaneAuthorityCatalogV1,
}
impl GenesisMergeAuthority {
    /// Exact signed native authority generation and genesis epoch authorization.
    pub fn epoch(&self) -> &ValidatorEpochContextV1 {
        &self.epoch
    }
    /// Borrow the exact signed body. Its mutable post-execution R is not a genesis signature.
    pub fn genesis(&self) -> &SignedBlock {
        &self.genesis
    }
    /// Native genesis consensus identity, equal to the network-pinned signed header hash.
    pub fn consensus_hash(&self) -> Hash {
        Hash::from(self.genesis.hash())
    }
    /// Same sanitized catalog commitment used by actual merge planning.
    pub const fn catalog_hash(&self) -> Hash {
        self.catalog_hash
    }
    /// Production-ordered lane, incarnation and activation bindings.
    pub fn active_lanes(&self) -> &[iroha_data_model::merge::MergeLaneBinding] {
        &self.active_lanes
    }
    /// Exact route committees resolved from the successfully executed genesis.
    pub fn lane_authority_catalog(&self) -> &iroha_data_model::merge::MergeLaneAuthorityCatalogV1 {
        &self.lane_authority_catalog
    }
}

/// Exact failure at the signed-genesis authority boundary.
#[derive(Debug, thiserror::Error)]
pub enum GenesisMergeAuthorityError {
    /// Signature, complete epoch, mode or executed registration disagrees with signed genesis.
    #[error("invalid native genesis authority: {0}")]
    Authority(String),
    /// The requested execution mode differs from the independently signed metadata.
    #[error("genesis consensus mode differs from signed metadata")]
    SignedConsensusModeMismatch,
    /// The overlay belongs to a different genesis network.
    #[error("staged genesis network differs from signed genesis")]
    StagedNetworkIdMismatch,
    /// The supplied overlay is not the exact signed genesis execution.
    #[error("merge authority requires the exact staged height-one genesis header and network")]
    StagedHeaderMismatch,
    /// Nexus, execution policy or signed mandatory RS16 differs from the actual staged source.
    #[error("executed genesis policy differs from the independently signed projection")]
    PolicyMismatch,
    /// Actual merge committee, incarnation or activation projection is inconsistent.
    #[error(transparent)]
    Merge(#[from] crate::state::MergeLedgerCommitError),
}

/// Freeze native merge authority from actual signed genesis and its still-owned overlay.
///
/// # Errors
/// Rejects a substituted network/header, unsupported signed epoch, mismatched complete voting
/// registrations/PoPs, Nexus/execution policy drift or invalid actual route custody.
pub fn freeze_genesis_merge_authority(
    genesis: &GenesisBlock,
    staged: &StateBlock<'_>,
    mode: ConsensusMode,
) -> Result<GenesisMergeAuthority, GenesisMergeAuthorityError> {
    if !genesis.0.header().is_genesis() || staged._curr_block != genesis.0.header() {
        return Err(GenesisMergeAuthorityError::StagedHeaderMismatch);
    }
    if *staged.network_id() != NetworkId::from_genesis_hash(genesis.0.hash()) {
        return Err(GenesisMergeAuthorityError::StagedNetworkIdMismatch);
    }
    let epoch =
        super::epoch::genesis_epoch(&genesis.0).map_err(GenesisMergeAuthorityError::Authority)?;
    if epoch.mode != mode {
        return Err(GenesisMergeAuthorityError::SignedConsensusModeMismatch);
    }
    super::schedule::validate_executed_genesis(staged.world(), &epoch)
        .map_err(|error| GenesisMergeAuthorityError::Authority(error.to_string()))?;
    let metadata = iroha_genesis::signed_genesis_consensus_metadata(&genesis.0)
        .map_err(|error| GenesisMergeAuthorityError::Authority(error.to_string()))?;
    let nexus = super::genesis_meta::staged_genesis_nexus_amx_context_hash(staged);
    let policy = super::genesis_meta::staged_genesis_execution_policy_hash(staged)
        .map_err(|error| GenesisMergeAuthorityError::Authority(error.to_string()))?;
    if nexus.as_ref() != &metadata.sumeragi_v2.nexus_amx_context_hash
        || policy.as_ref() != &metadata.sumeragi_v2.execution_policy_hash
    {
        return Err(GenesisMergeAuthorityError::PolicyMismatch);
    }
    let (catalog_hash, active_lanes, lane_authority_catalog) =
        staged.staged_genesis_merge_authority_snapshot()?;
    Ok(GenesisMergeAuthority {
        genesis: genesis.0.clone(),
        epoch,
        catalog_hash,
        active_lanes,
        lane_authority_catalog,
    })
}
