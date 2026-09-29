//! Pure canonical epoch schedule types and validation shared by node and proof readers.
use crate::{
    block::{SignedBlock, consensus::is_valid_committee_size},
    isi::RegisterBox,
    transaction::Executable,
};
use iroha_crypto::Algorithm;
use iroha_model_base::peer::PeerId;
use iroha_sumeragi::{
    api::ConfigError,
    types::{Committee, CommitteeError, PublicKey},
};
use std::collections::BTreeMap;
use thiserror::Error;

/// Portable projection of an unavailable or corrupt committed schedule source.
///
/// This records local recovery failures without depending on a node storage reader.
/// Resource admission and physical allocation refusals use the separate schedule variants;
/// callers must preserve those refusals before projecting a committed-source failure.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ScheduleSourceError {
    /// The height is zero or above this view's committed height.
    #[error("height {height} is not committed in this view")]
    NotCommitted {
        /// Requested height.
        height: u64,
    },
    /// Kura does not hold the block this view committed at the height.
    #[error("Kura does not hold the block this view committed at {height}")]
    NotInView {
        /// Requested height.
        height: u64,
    },
    /// The block carries no commit certificate.
    #[error("the block at {height} carries no commit certificate")]
    MissingCertificate {
        /// Requested height.
        height: u64,
    },
    /// A certificate part is not one canonical frame, or has the wrong shape.
    #[error("the commit certificate at {height} is malformed: {reason}")]
    Malformed {
        /// Requested height.
        height: u64,
        /// What is wrong.
        reason: String,
    },
    /// The certified header or `CommitQC` does not belong to the stored block at this height.
    #[error("the certified header does not match the block at {height}")]
    HeaderMismatch {
        /// Requested height.
        height: u64,
    },
    /// The result preimage does not hash to the certified result.
    #[error("the result preimage at {height} does not hash to the certified result")]
    ResultMismatch {
        /// Requested height.
        height: u64,
    },
    /// The certified result does not commit the stored result-bearing block.
    #[error("the certified result at {height} does not commit the stored block")]
    ExecutionMismatch {
        /// Requested height.
        height: u64,
    },
    /// The certificate names another chain instance.
    #[error("the block at {height} is certified for another chain instance")]
    WrongInstance {
        /// Requested height.
        height: u64,
    },
    /// The block does not extend the block before it.
    #[error("the block at {height} does not extend its parent")]
    Discontinuous {
        /// Requested height.
        height: u64,
    },
    /// Kura's genesis is not this view's network genesis.
    #[error("Kura's genesis is not this view's network genesis")]
    ForeignGenesis,
    /// The committee of the height cannot be formed.
    #[error("no committee for height {height}: {reason}")]
    Committee {
        /// Requested height.
        height: u64,
        /// What is wrong.
        reason: String,
    },
    /// The `CommitQC` does not verify under the committee of its height.
    #[error("the commit certificate at {height} does not verify: {error:?}")]
    Certificate {
        /// Requested height.
        height: u64,
        /// The failed check.
        error: iroha_sumeragi::crypto::CertError,
    },
}

/// Why schedule preparation failed. Admission and allocator refusals are local deferrals,
/// never invalid-block evidence; all other variants describe deterministic protocol violations.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ScheduleError {
    /// An exact committed local source needs recovery before this attempt can continue.
    #[error(transparent)]
    CommittedSource(#[from] ScheduleSourceError),
    /// Full authenticated epoch, boundary or canonical source is invalid.
    #[error("invalid native epoch: {0}")]
    Epoch(String),
    /// The original finite resource pool refused this local attempt.
    #[error(transparent)]
    Admission(#[from] mv::allocation::AllocationRefusal),
    /// Physical allocation failed after the exact original pool admitted its layout.
    #[error("native schedule allocator refused {requested_bytes} prepaid bytes")]
    Allocator {
        /// Exact failed requested allocation size.
        requested_bytes: usize,
    },
    /// The global voting roster is not exactly `3f + 1`, with `1 <= f <= 10`.
    #[error("global committee must have exactly 3f + 1 validators (4..=31), got {validators}")]
    InvalidCommitteeSize {
        /// Actual number of supplied voting members, without observers or deduplication.
        validators: usize,
    },
    /// Stored membership is not in the canonical order used by signer indexes.
    #[error("global committee members are not in canonical consensus-key order")]
    NonCanonicalCommittee,
    /// The retained roster does not have exactly one proof per ordered member.
    #[error("scheduled committee has {validators} members but {proofs} proofs of possession")]
    ProofCount {
        /// Number of ordered validator members.
        validators: usize,
        /// Number of retained proofs.
        proofs: usize,
    },
    /// An original proof does not authenticate its exact ordered BLS key.
    #[error("scheduled committee proof of possession is invalid at member {index}")]
    InvalidProofOfPossession {
        /// Index in the canonical ordered committee.
        index: usize,
    },
    /// A committee key is not a BLS-normal public key the core accepts.
    #[error("committee member {0} is not a BLS-normal consensus key")]
    NotBlsNormal(String),
    /// The committee cannot be formed (empty, too large, duplicate or malformed keys).
    #[error("invalid committee: {0}")]
    Committee(CommitteeError),
    /// The chain parameters fail §9.4 validation.
    #[error("invalid chain parameters: {0}")]
    Params(ConfigError),
    /// The signed NPoS policy contradicts the current consensus epoch authority.
    #[error("NPoS epoch_length_blocks must equal the signed Sumeragi epoch_length_blocks")]
    EpochPolicyMismatch,
    /// The stored schedule is not empty or three consecutive heights.
    #[error("the stored consensus schedule is malformed")]
    Malformed,
    /// The stored schedule does not end right before the height being scheduled.
    #[error("block {height} cannot advance a schedule ending at {last_scheduled:?}")]
    NotConsecutive {
        /// Height of the executed block.
        height: u64,
        /// Last height the stored schedule covers.
        last_scheduled: Option<u64>,
    },
    /// A block below the genesis height.
    #[error("block {height} is below the genesis height {genesis_height}")]
    BeforeGenesis {
        /// Height of the executed block.
        height: u64,
        /// Genesis height.
        genesis_height: u64,
    },
    /// A demotion-window change after genesis.
    #[error("the Sumeragi demotion window is a genesis constant")]
    GenesisOnly,
    /// A height does not fit `u64`.
    #[error("height overflow")]
    HeightOverflow,
    /// The block's execution did not reach the output seal, so the requested step never ran.
    #[error("the block's execution did not advance the schedule")]
    NotAdvanced,
}

/// Construct a global committee under the first-release voting geometry. This boundary is
/// shared by live scheduling, signed genesis, and historical certificate verification; the
/// generic consensus core's wider committee domain cannot broaden global authority.
pub fn global_committee(keys: Vec<PublicKey>) -> Result<Committee, ScheduleError> {
    if !is_valid_committee_size(keys.len()) {
        return Err(ScheduleError::InvalidCommitteeSize {
            validators: keys.len(),
        });
    }
    Committee::new(keys).map_err(ScheduleError::Committee)
}

/// The core consensus key of a validator peer: the raw 48-byte BLS-normal public key (§3.1).
///
/// # Errors
/// [`ScheduleError::NotBlsNormal`] for another algorithm or a malformed key.
pub fn consensus_key(peer: &PeerId) -> Result<PublicKey, ScheduleError> {
    let not_bls = || ScheduleError::NotBlsNormal(peer.to_string());
    let (algorithm, payload) = peer.public_key().try_to_bytes().map_err(|_| not_bls())?;
    if algorithm != Algorithm::BlsNormal {
        return Err(not_bls());
    }
    PublicKey::new(payload.to_vec()).map_err(|_| not_bls())
}

/// Why a genesis block defines no committee.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum GenesisCommitteeError {
    /// A genesis transaction is not an instruction list.
    #[error("genesis transactions must be instruction lists")]
    UnsupportedExecutable,
    /// A registered validator key is not BLS-normal.
    #[error("genesis validator {0} is not a BLS-normal key")]
    NonBlsValidator(String),
    /// A registered validator's proof of possession does not verify.
    #[error("genesis validator {0} has an invalid proof of possession")]
    InvalidProofOfPossession(String),
    /// A validator is registered twice.
    #[error("genesis registers validator {0} more than once")]
    DuplicateValidator(String),
    /// The validators do not form an exact first-release global committee.
    #[error("genesis validators do not form a global committee: {0}")]
    Committee(ScheduleError),
}

/// The validators signed into genesis with their proofs of possession, **not verified**: for
/// readers that independently verify each proof before granting signing authority.
///
/// # Errors
/// A non-instruction transaction, a non-BLS-normal key or a duplicate.
pub fn genesis_registrations(
    genesis: &SignedBlock,
) -> Result<BTreeMap<PeerId, Vec<u8>>, GenesisCommitteeError> {
    let mut validators = BTreeMap::new();
    for transaction in genesis.external_transactions() {
        let Executable::Instructions(instructions) = transaction.instructions() else {
            return Err(GenesisCommitteeError::UnsupportedExecutable);
        };
        for register in instructions.iter().filter_map(|instruction| {
            match instruction.as_any().downcast_ref::<RegisterBox>()? {
                RegisterBox::Peer(register) => Some(register),
                _ => None,
            }
        }) {
            let name = || register.peer.to_string();
            if register.peer.public_key().try_algorithm() != Ok(Algorithm::BlsNormal) {
                return Err(GenesisCommitteeError::NonBlsValidator(name()));
            }
            if validators
                .insert(register.peer.clone(), register.pop.clone())
                .is_some()
            {
                return Err(GenesisCommitteeError::DuplicateValidator(name()));
            }
        }
    }
    Ok(validators)
}
