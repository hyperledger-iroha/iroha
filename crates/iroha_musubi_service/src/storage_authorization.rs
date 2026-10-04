//! Borrowed storage requests authenticated by the private service's sole route owner.

use super::{
    MusubiPublicationRuntimeAuthorizationV1, MusubiPublicationServiceErrorCodeV1,
    MusubiPublicationServiceErrorV1, MusubiStorageCoordinationRequestV1,
};
use std::time::{Duration, Instant};

/// One authenticated storage request and its original call bounds.
///
/// Only the private service constructs this value, after verifying the publisher authorization,
/// exact canonical request, staging receipt and immutable request bindings. It contains no
/// replay-sensitive authorization header, signature or signing key. It proves neither native
/// finality nor permission to spend the storage operator's funds.
///
/// The backend must retain its first accepted operation's finite lifetime and operator fee
/// limits before preparing paid work. A later authenticated call may observe that operation;
/// it must not replace those retained limits with this call's later expiry.
pub struct VerifiedStorageCoordinationRequestV1<'a> {
    request: &'a MusubiStorageCoordinationRequestV1,
    canonical_request_digest: [u8; 32],
    authorization_issued_at_ms: u64,
    authorization_expires_at_ms: u64,
    observed_at_unix_ms: u64,
    deadline: Instant,
}

impl<'a> VerifiedStorageCoordinationRequestV1<'a> {
    // Sole production caller is handle_storage_coordination after its complete verification.
    pub(super) fn from_verified(
        request: &'a MusubiStorageCoordinationRequestV1,
        canonical_request_digest: [u8; 32],
        authorization: &MusubiPublicationRuntimeAuthorizationV1,
        observed_at_unix_ms: u64,
        observation_started: Instant,
    ) -> Result<Self, MusubiPublicationServiceErrorV1> {
        let expired = || {
            MusubiPublicationServiceErrorV1::retryable(
                MusubiPublicationServiceErrorCodeV1::AuthorizationExpired,
            )
        };
        let remaining = authorization
            .payload
            .expires_at_ms
            .checked_sub(observed_at_unix_ms)
            .filter(|remaining| *remaining > 0)
            .ok_or_else(expired)?;
        // Anchor before the durable UTC sample, so clock I/O, decoding, signature verification
        // and journal admission all consume this call's original monotonic interval.
        let deadline = observation_started
            .checked_add(Duration::from_millis(remaining))
            .filter(|deadline| *deadline > Instant::now())
            .ok_or_else(expired)?;
        Ok(Self {
            request,
            canonical_request_digest,
            authorization_issued_at_ms: authorization.payload.issued_at_ms,
            authorization_expires_at_ms: authorization.payload.expires_at_ms,
            observed_at_unix_ms,
            deadline,
        })
    }

    /// Borrow the exact canonical request authenticated by the publisher.
    #[must_use]
    pub const fn request(&self) -> &MusubiStorageCoordinationRequestV1 {
        self.request
    }

    /// Domain-separated digest of the exact canonical request bytes.
    #[must_use]
    pub const fn canonical_request_digest(&self) -> [u8; 32] {
        self.canonical_request_digest
    }

    /// Original publisher authorization issuance, in Unix milliseconds.
    #[must_use]
    pub const fn authorization_issued_at_ms(&self) -> u64 {
        self.authorization_issued_at_ms
    }

    /// Original publisher authorization expiry, in Unix milliseconds.
    #[must_use]
    pub const fn authorization_expires_at_ms(&self) -> u64 {
        self.authorization_expires_at_ms
    }

    /// Non-regressing service clock sample used to authenticate this call.
    #[must_use]
    pub const fn observed_at_unix_ms(&self) -> u64 {
        self.observed_at_unix_ms
    }

    /// Original monotonic deadline; consumers must recheck it before every effect.
    ///
    /// This bound cannot refresh a previously retained operation's lifetime, and is deliberately
    /// conservative at the transport protocol's inclusive expiry boundary.
    #[must_use]
    pub const fn deadline(&self) -> Instant {
        self.deadline
    }
}
