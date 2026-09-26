//! Startup of the Sumeragi driver (`specs/sumeragi.md` §7.4, §10, §12.1).
//!
//! Genesis (iroha height 1, the core's genesis height `g`) is applied locally, without a
//! consensus round: every node executes the same signed genesis block to the same state. Its
//! result `R_g` (the core's `tip.result` at `g`) is kept durably in the genesis frame as a
//! result-only certificate (no core header, no CommitQC), so a restart reads it back instead of
//! recomputing it. On restart genesis is re-executed from Kura and must reproduce that result;
//! later blocks are replayed through the executor, each checked against its certified result.

use iroha_data_model::{
    account::AccountId,
    block::{CommitCertificate, SignedBlock},
    parameter::system::ConsensusMode,
};
use iroha_model_base::peer::PeerId;
use iroha_primitives::time::TimeSource;
use iroha_sumeragi::types::Hash32;

use super::{
    commitment::{execution_result, result_of_preimage},
    network_topology::Topology,
    schedule,
};
use crate::{
    block::ValidBlock,
    state::State,
};

/// The genesis height: iroha's genesis block is height 1.
pub const GENESIS_HEIGHT: u64 = 1;

/// Why startup failed.
#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    /// The genesis block is invalid.
    #[error("genesis is invalid: {0}")]
    InvalidGenesis(String),
    /// The genesis committee or schedule is invalid.
    #[error("genesis schedule is invalid: {0}")]
    Schedule(String),
    /// Genesis re-executed to a different result than the stored one.
    #[error("genesis re-execution diverges from the stored result")]
    GenesisDiverged,
    /// A local failure (storage, state publication).
    #[error("local failure: {0}")]
    Local(String),
}

/// The core's view of the applied genesis: its block hash and result (`Init.tip` at `g`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenesisTip {
    /// Core hash of genesis: the bytes of iroha's genesis block hash.
    pub block_hash: Hash32,
    /// `R_g`.
    pub result: Hash32,
}

/// The core hash of an iroha block: the bytes of its header hash.
#[must_use]
pub fn core_hash_of(block: &SignedBlock) -> Hash32 {
    let hash: iroha_crypto::Hash = block.hash().into();
    Hash32(*hash.as_ref())
}

/// Execute and apply `genesis` to the empty `state`.
///
/// `stored` is `None` for a fresh chain (the genesis frame is then written to Kura with its
/// result-only certificate) and the genesis frame read back from Kura on a restart (its
/// certificate must reproduce).
///
/// # Errors
/// Invalid genesis, an invalid genesis schedule, a divergent re-execution, or a local
/// failure.
pub fn apply_genesis(
    state: &State,
    genesis: SignedBlock,
    genesis_account: &AccountId,
    consensus_mode: ConsensusMode,
    stored: Option<&CommitCertificate>,
) -> Result<GenesisTip, StartupError> {
    let committee = genesis_committee_peers(&genesis)?;
    let topology = Topology::new(committee.clone());
    let block_hash = core_hash_of(&genesis);
    let (valid, mut overlay) = ValidBlock::validate_sumeragi_genesis(
        genesis,
        &topology,
        genesis_account,
        &TimeSource::new_system(),
        state,
        consensus_mode,
    )
    .unpack(|_| {})
    .map_err(|(_, error)| StartupError::InvalidGenesis(error.to_string()))?;
    let next = overlay
        .take_sumeragi_schedule()
        .and_then(|next| next.height_config())
        .map_err(|error| StartupError::Schedule(error.to_string()))?;
    let witness = overlay
        .take_exec_witness()
        .ok_or_else(|| StartupError::Local("genesis witness was not captured".into()))?;
    let (_, preimage, result) = execution_result(&witness, valid.as_ref(), &next)
        .map_err(|error| StartupError::InvalidGenesis(error.to_string()))?;
    let certificate = CommitCertificate::new(Vec::new(), Vec::new(), preimage);
    let committed = valid.commit_unchecked().unpack(|_| {});
    match stored {
        Some(stored) => {
            if stored != &certificate {
                return Err(StartupError::GenesisDiverged);
            }
        }
        None => {
            let frame = committed
                .as_ref()
                .clone()
                .with_commit_certificate(Some(certificate.clone()));
            state
                .kura()
                .store_block(frame)
                .map_err(|error| StartupError::Local(error.to_string()))?;
        }
    }
    overlay
        .authorize_sumeragi_output_publication(&committed, &witness, &certificate)
        .map_err(StartupError::Local)?;
    overlay
        .apply_without_execution_with_sumeragi_commit(&committed, &certificate, committee)
        .map_err(|error| StartupError::Local(error.to_string()))?;
    overlay
        .commit()
        .map_err(|error| StartupError::Local(error.to_string()))?;
    Ok(GenesisTip { block_hash, result })
}

/// The genesis tip recorded in Kura, if genesis is stored.
#[must_use]
pub fn stored_genesis(state: &State) -> Option<(SignedBlock, CommitCertificate, GenesisTip)> {
    let block = state
        .kura()
        .get_block(core::num::NonZeroUsize::new(1).expect("non-zero"))?;
    let certificate = block.commit_certificate()?.clone();
    let tip = GenesisTip {
        block_hash: core_hash_of(&block),
        result: result_of_preimage(&certificate.result_preimage),
    };
    Some((block.canonical_resultless_proposal(), certificate, tip))
}

/// The validators of a signed genesis, in canonical order (`C_g`).
///
/// # Errors
/// The genesis does not register a valid committee.
pub fn genesis_committee_peers(genesis: &SignedBlock) -> Result<Vec<PeerId>, StartupError> {
    let genesis = iroha_genesis::GenesisBlock(genesis.clone());
    let validators = schedule::genesis_validators(&genesis)
        .map_err(|error| StartupError::Schedule(error.to_string()))?;
    let committee = schedule::genesis_committee(&genesis)
        .map_err(|error| StartupError::Schedule(error.to_string()))?;
    let mut peers = validators.into_keys().collect::<Vec<_>>();
    peers.sort_by_key(|peer| {
        schedule::consensus_key(peer)
            .ok()
            .and_then(|key| committee.index_of(&key))
    });
    Ok(peers)
}

/// The height the state has applied.
#[must_use]
pub fn applied_height(state: &State) -> u64 {
    state.view().height() as u64
}
