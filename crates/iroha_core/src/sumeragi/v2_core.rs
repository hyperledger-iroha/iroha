//! Pure persistence-aware Sumeragi v2 reducer embedded in `iroha_core`.
//!
//! The package-local modules below are the authoritative dependency-free
//! transition relation used by production, simulation, and formal refinement.
//! The excluded `iroha_sumeragi_core` crate is only a verification harness over
//! these sources, so publishing `iroha_core` never depends on files outside its
//! own package root.

// These three source modules also form the public API of the standalone
// `iroha_sumeragi_core` verification crate. Some public accessors are not used
// by the private embedded adapter, so its compilation cannot observe their
// external consumers.
#[allow(dead_code)]
mod quorum;
#[macro_use]
mod refinement;
#[allow(dead_code)]
mod reducer;
#[allow(dead_code)]
mod types;
mod wal;

pub(crate) use quorum::{Quorum, QuorumError};
#[cfg(test)]
pub(crate) use reducer::EquivocationKind;
pub(crate) use reducer::{
    BodyState, DurableCommitReceipt, Effect, EquivocationEvidence, Event, IgnoreReason, Reducer,
    ReducerError, SignableMessage, StepDisposition, StepOutcome,
};
pub(crate) use types::{
    CertificateRef, ChainId, ConsensusMessageV2, ContextId, Digest, EventTag, Generation,
    HeightContext, HeightContextError, OpaqueSignature, PayloadManifest, Phase, Proposal,
    ProposalJustification, QuorumCertificate, Round, SignatureShare, SignedProposal,
    SignedTimeoutVote, SignedVote, Subject, TimeoutCertificate, TimeoutSignatureGroup, TimeoutVote,
    Validator, ValidatorId, Vote, VotingMode, VotingPower,
};
pub(crate) use wal::{DurableState, PersistenceId, ReplayError, WalEntry, WalRecord};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod network_simulation;
