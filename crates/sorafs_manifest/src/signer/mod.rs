//! Sole public SoraFS signer qualification and operation-receipt contract.
//!
//! Trust, time, finalized active custody and completed-operation state are supplied independently
//! of candidate receipts. Runtime signer transport, credentials, exclusive reservations and
//! durable state adapters belong to the daemon, never this pure verification layer.
//! TODO: Complete the production consumer hard cut and real signer/state adapters; this public
//! verifier alone is not deployment qualification and does not upgrade software receipts.

pub mod custody;
/// Governed signer custody policy and state, excluding application operations.
pub mod custody_control;
pub mod final_promotion;
pub mod protocol;
pub mod receipt;
pub mod release_evidence;
pub mod state_observation;
pub mod stream_token;
pub mod stream_token_evidence;
