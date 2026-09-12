//! Bounded untrusted transport replies for independently verified hardware stream-token issuance.

use iroha_crypto::zeroize_value_for_confidential_discard;
use sorafs_manifest::{
    StreamTokenBodyV1,
    signer::{
        custody::{SIGNER_CUSTODY_MAX_BYTES_V1, SignerCustodyAnchorV1},
        stream_token::{SIGNER_STREAM_TOKEN_RECEIPT_MAX_BYTES_V1, SignerStreamTokenExpectedV1},
        stream_token_evidence::{
            SIGNER_STREAM_TOKEN_EVIDENCE_MAX_BYTES_V1, SignerStreamTokenObservationRequestV1,
        },
    },
};
use std::fmt;
use thiserror::Error;

/// Fixed public transport failures; no provider message, key, payload or path is exposed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum StreamTokenHardwareCallErrorV1 {
    /// The operation was unavailable before a mutating request was accepted.
    #[error("stream-token hardware runtime unavailable")]
    Unavailable,
    /// The service refused the exact prepared operation.
    #[error("stream-token hardware runtime refused request")]
    Refused,
    /// Sign may have durably completed; only the same operation's read-only recovery is allowed.
    #[error("stream-token hardware completion is ambiguous")]
    AmbiguousCompletion,
    /// A transport reply exceeded its bounds or had the wrong closed response form.
    #[error("stream-token hardware response is invalid")]
    InvalidResponse,
}

/// Opaque hardware service client with exactly one mutating method and explicit read-only recovery.
///
/// The configured service independently derives and compares the same operation and provider
/// scope. Credentials, bounded deadlines and transport admission belong to the adapter. Neither
/// this client nor its raw output can qualify custody, finality or completed-operation authority.
pub trait StreamTokenHardwareClientV1: Send + Sync {
    /// Exact configured production hardware runtime handle, without credentials.
    fn handle(&self) -> &str;
    /// Sign the exact independently prepared body once; never retry after ambiguous completion.
    fn sign(
        &self,
        expected: &SignerStreamTokenExpectedV1,
        body: &StreamTokenBodyV1,
    ) -> Result<StreamTokenHardwareReceiptV1, StreamTokenHardwareCallErrorV1>;
    /// Read only the same prepared operation's immutable receipt; never reserve, commit or sign.
    fn recover(
        &self,
        expected: &SignerStreamTokenExpectedV1,
        body: &StreamTokenBodyV1,
    ) -> Result<StreamTokenHardwareReceiptV1, StreamTokenHardwareCallErrorV1>;
}

/// Separate observer transport; authority comes only from independently pinned signed evidence.
///
/// A new genuine finalized read must follow each caller-created query. Cached pre-provider or
/// startup state cannot be timestamp-refreshed into a later phase. The adapter must bound its
/// complete canonical transport envelope before allocation, in addition to these leaf ceilings.
pub trait StreamTokenStateObserverClientV1: Send + Sync {
    /// Exact independently configured credential-free observer routing handle.
    fn handle(&self) -> &str;
    /// Execute one read-only query, preserving every retained challenge, phase and subject field.
    fn observe(
        &self,
        request: &SignerStreamTokenObservationRequestV1,
    ) -> Result<StreamTokenObserverReplyV1, StreamTokenHardwareCallErrorV1>;
}

struct UntrustedBytes(Vec<u8>);
impl UntrustedBytes {
    fn bounded(bytes: Vec<u8>, maximum: usize) -> Result<Self, StreamTokenHardwareCallErrorV1> {
        let candidate = Self(bytes);
        if candidate.0.is_empty() || candidate.0.len() > maximum {
            return Err(StreamTokenHardwareCallErrorV1::InvalidResponse);
        }
        Ok(candidate)
    }
}
impl Drop for UntrustedBytes {
    fn drop(&mut self) {
        zeroize_value_for_confidential_discard(&mut self.0);
    }
}

/// Untrusted bounded receipt bytes; shape and all authority checks remain with shared verifiers.
///
/// No Clone, wire decoder, default or verified-result conversion is provided. Unreleased signature
/// copies are scrubbed when this wrapper is dropped on any path.
pub struct StreamTokenHardwareReceiptV1(UntrustedBytes);
impl StreamTokenHardwareReceiptV1 {
    /// Retain one nonempty receipt within the shared exact canonical-frame byte ceiling.
    ///
    /// # Errors
    /// Rejects empty/oversized transport input without qualifying its content.
    pub fn new(bytes: Vec<u8>) -> Result<Self, StreamTokenHardwareCallErrorV1> {
        UntrustedBytes::bounded(bytes, SIGNER_STREAM_TOKEN_RECEIPT_MAX_BYTES_V1).map(Self)
    }
    /// Borrow the untrusted exact receipt bytes for canonical public verification.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.0.0
    }
}
impl fmt::Debug for StreamTokenHardwareReceiptV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("StreamTokenHardwareReceiptV1")
            .finish_non_exhaustive()
    }
}

enum ObserverBytes {
    Current {
        record: UntrustedBytes,
        observation: UntrustedBytes,
    },
    Completed {
        observation: UntrustedBytes,
    },
}
/// Closed current-only/completed raw observer reply with private bounded byte buffers.
pub struct StreamTokenObserverReplyV1(ObserverBytes);
impl StreamTokenObserverReplyV1 {
    /// Retain the current-only attestation record and independently signed state observation.
    ///
    /// # Errors
    /// Rejects either empty or oversized leaf; performs no custody or observation verification.
    pub fn current(
        record: Vec<u8>,
        observation: Vec<u8>,
    ) -> Result<Self, StreamTokenHardwareCallErrorV1> {
        // Wrap both immediately so every early error scrubs both buffers.
        let record = UntrustedBytes(record);
        let observation = UntrustedBytes(observation);
        if record.0.is_empty()
            || record.0.len() > SIGNER_CUSTODY_MAX_BYTES_V1
            || observation.0.is_empty()
            || observation.0.len() > SIGNER_STREAM_TOKEN_EVIDENCE_MAX_BYTES_V1
        {
            return Err(StreamTokenHardwareCallErrorV1::InvalidResponse);
        }
        Ok(Self(ObserverBytes::Current {
            record,
            observation,
        }))
    }
    /// Retain one completed-operation observation; no fabricated current-only record is accepted.
    ///
    /// # Errors
    /// Rejects empty or oversized observation input, without creating verified authority.
    pub fn completed(observation: Vec<u8>) -> Result<Self, StreamTokenHardwareCallErrorV1> {
        UntrustedBytes::bounded(observation, SIGNER_STREAM_TOKEN_EVIDENCE_MAX_BYTES_V1)
            .map(|observation| Self(ObserverBytes::Completed { observation }))
    }
    /// Borrow raw current-only record/observation leaves, or None for the other response form.
    #[must_use]
    pub fn current_evidence(&self) -> Option<(&[u8], &[u8])> {
        match &self.0 {
            ObserverBytes::Current {
                record,
                observation,
            } => Some((&record.0, &observation.0)),
            ObserverBytes::Completed { .. } => None,
        }
    }
    /// Borrow the raw completed observation, or None for the current-only response form.
    #[must_use]
    pub fn completed_observation(&self) -> Option<&[u8]> {
        match &self.0 {
            ObserverBytes::Completed { observation } => Some(&observation.0),
            ObserverBytes::Current { .. } => None,
        }
    }
}
impl fmt::Debug for StreamTokenObserverReplyV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("StreamTokenObserverReplyV1")
            .finish_non_exhaustive()
    }
}

/// Independent deployment-approved public custody floor, bound to complete runtime config pins.
///
/// This is an explicit trust input supplied by governed runtime construction, never a provider
/// reply, decoded receipt or verified marker. It must represent genuine finalized per-role state;
/// Torii additionally validates its chain association against its own durable Core history.
#[derive(Clone, Copy)]
pub struct StreamTokenApprovedCustodyAnchorV1 {
    config_digest: [u8; 32],
    anchor: SignerCustodyAnchorV1,
}
impl StreamTokenApprovedCustodyAnchorV1 {
    /// Bind an independently approved nonzero full custody floor to exact configured public pins.
    ///
    /// # Errors
    /// Rejects an empty binding or anchor. Construction alone does not prove finality or authority.
    pub fn new(
        config_digest: [u8; 32],
        anchor: SignerCustodyAnchorV1,
    ) -> Result<Self, StreamTokenHardwareCallErrorV1> {
        if config_digest == [0; 32]
            || anchor.height == 0
            || anchor.block_hash == [0; 32]
            || anchor.state_digest == [0; 32]
        {
            return Err(StreamTokenHardwareCallErrorV1::InvalidResponse);
        }
        Ok(Self {
            config_digest,
            anchor,
        })
    }
    /// Exact canonical complete public-config digest approved with this floor.
    #[must_use]
    pub const fn config_digest(&self) -> [u8; 32] {
        self.config_digest
    }
    /// Exact independently approved finalized per-role custody anchor.
    #[must_use]
    pub const fn anchor(&self) -> SignerCustodyAnchorV1 {
        self.anchor
    }
}
impl fmt::Debug for StreamTokenApprovedCustodyAnchorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("StreamTokenApprovedCustodyAnchorV1")
            .finish_non_exhaustive()
    }
}
