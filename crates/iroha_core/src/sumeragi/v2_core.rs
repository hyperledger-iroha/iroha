//! Pure persistence-aware Sumeragi v2 reducer embedded in `iroha_core`.
//!
//! The package-local modules below are the authoritative dependency-free
//! transition relation used by production, simulation, and formal refinement.
//! The excluded `iroha_sumeragi_core` crate is only a verification harness over
//! these sources, so publishing `iroha_core` never depends on files outside its
//! own package root.

mod quorum;
#[macro_use]
mod refinement;
mod reducer;
mod types;
mod wal;

pub(crate) use quorum::{Quorum, QuorumError};
pub(crate) use reducer::{
    BodyState, DurableCommitReceipt, Effect, EquivocationEvidence, EquivocationKind, Event,
    FinalizedHeight, IgnoreReason, Reducer, ReducerError, SignableMessage, StepDisposition,
    StepOutcome,
};
pub(crate) use types::{
    CertificateRef, ChainId, ConsensusMessageV2, ContextId, Digest, EventTag, Generation,
    HeightContext, HeightContextError, OpaqueSignature, PROTOCOL_VERSION_V2, PayloadChunk,
    PayloadManifest, Phase, Proposal, ProposalJustification, QuorumCertificate, Round,
    SignatureShare, SignedProposal, SignedTimeoutVote, SignedVote, Subject, TimeoutCertificate,
    TimeoutSignatureGroup, TimeoutVote, Validator, ValidatorId, Vote, VotingMode, VotingPower,
};
pub(crate) use wal::{DurableState, PersistenceId, ReplayError, WalEntry, WalRecord};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod network_simulation;
