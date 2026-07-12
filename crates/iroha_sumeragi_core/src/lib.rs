//! Pure, persistence-aware state machine for Sumeragi protocol version 2.
//!
//! This crate deliberately owns no networking, cryptography, executor, clock,
//! or filesystem implementation. Adapters authenticate inbound messages and
//! execute the effects returned by [`Reducer`]. The reducer serializes all
//! consensus state changes and makes the durability boundary explicit: a vote
//! or timeout is never signed until its corresponding [`WalRecord`] has been
//! acknowledged, and a decided block is never applied before its decision is
//! durable.
//!
//! The types use only deterministic standard-library collections and integer
//! arithmetic. This keeps the executable transition relation small enough to
//! serve as both the production core and the target of deductive verification.

#[cfg(all(verus_only, feature = "verus"))]
use vstd::prelude::*;

mod quorum;
#[macro_use]
mod refinement;
mod reducer;
mod types;
#[cfg(all(verus_only, feature = "verus"))]
mod verus_proofs;
mod wal;

pub use quorum::{Quorum, QuorumError};
pub use reducer::{
    BodyState, DurableCommitReceipt, Effect, EquivocationKind, Event, FinalizedHeight,
    IgnoreReason, Reducer, ReducerError, SignableMessage, StepDisposition, StepOutcome,
};
pub use types::{
    CertificateRef, ChainId, ConsensusMessageV2, ContextId, Digest, EventTag, Generation,
    HeightContext, HeightContextError, OpaqueSignature, PROTOCOL_VERSION_V2, PayloadChunk,
    PayloadManifest, Phase, Proposal, ProposalJustification, QuorumCertificate, Round,
    SignatureShare, SignedProposal, SignedTimeoutVote, SignedVote, Subject, TimeoutCertificate,
    TimeoutSignatureGroup, TimeoutVote, Validator, ValidatorId, Vote, VotingMode, VotingPower,
};
pub use wal::{
    DurableState, EncodedWalFrame, PersistenceId, RecoveredWalRecord, ReplayError,
    SAFETY_WAL_FILE_HEADER_LEN, SAFETY_WAL_FILE_MAGIC, SAFETY_WAL_FORMAT_VERSION,
    SAFETY_WAL_FRAME_HEADER_LEN, SAFETY_WAL_FRAME_MAGIC, SAFETY_WAL_HASH_LEN,
    SAFETY_WAL_MAX_RECORD_BYTES, WalAppendError, WalAppendIo, WalAppendReceipt, WalAppendState,
    WalCodecError, WalEntry, WalFileHasher, WalFileIdentity, WalFileRecovery, WalFrameCorruption,
    WalHeaderCorruption, WalIdentityField, WalIoStage, WalRecord, WalRetirementAuthorization,
    encode_wal_file_header, encode_wal_frame, recover_wal_file,
};

#[cfg(test)]
mod tests;
