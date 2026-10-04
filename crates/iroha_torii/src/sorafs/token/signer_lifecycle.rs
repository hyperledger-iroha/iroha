//! One prepared token, one mutating call, and independently authenticated release fences.

#[path = "signer_completed_phase.rs"]
mod completed_phase;

use super::{
    StreamTokenApprovedCustodyAnchorV1, StreamTokenIssuerError, StreamTokenSignerCallErrorV1,
    StreamTokenSignerClientV1, StreamTokenSignerPinsV1, StreamTokenStateObserverClientV1,
    signer_finality::{FinalityFloorV1, HistoricalFinalityV1, SignerFinalityV1},
};
use ed25519_dalek::VerifyingKey;
use iroha_crypto::zeroize_value_for_confidential_discard;
use rand::{rand_core::TryRngCore, rngs::OsRng};
use sorafs_manifest::{
    StreamTokenBodyV1, StreamTokenV1,
    signer::{
        custody::{SignerCustodyAnchorV1, VerifiedSignerCustodyV1},
        stream_token::{SignerStreamTokenExpectedV1, SignerStreamTokenReceiptV1},
        stream_token_evidence::{
            SignerStreamTokenObservationExpectedV1, SignerStreamTokenObservationPhaseV1 as Phase,
            SignerStreamTokenStateObservationBodyV1, SignerStreamTokenStateObservationV1,
            VerifiedStreamTokenSignerQualificationV1,
            verify_stream_token_signer_completed_observation_v1,
            verify_stream_token_signer_current_evidence_v1, verify_stream_token_signer_evidence_v1,
        },
    },
    token::STREAM_TOKEN_MAX_FUTURE_SKEW_SECS_V1,
};
use std::{
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) trait SignerClockV1: Send + Sync {
    fn now_unix_ms(&self) -> Result<u64, StreamTokenIssuerError>;
    fn challenge(&self) -> Result<[u8; 32], StreamTokenIssuerError>;
}
pub(super) struct SystemSignerClockV1;
impl SignerClockV1 for SystemSignerClockV1 {
    fn now_unix_ms(&self) -> Result<u64, StreamTokenIssuerError> {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| StreamTokenIssuerError::TimeOverflow)?;
        u64::try_from(duration.as_millis()).map_err(|_| StreamTokenIssuerError::TimeOverflow)
    }
    fn challenge(&self) -> Result<[u8; 32], StreamTokenIssuerError> {
        let mut challenge = [0; 32];
        OsRng
            .try_fill_bytes(&mut challenge)
            .map_err(|_| StreamTokenIssuerError::RuntimeSignerUnavailable)?;
        if challenge == [0; 32] {
            return Err(StreamTokenIssuerError::RuntimeSignerUnavailable);
        }
        Ok(challenge)
    }
}
struct History {
    anchor: SignerCustodyAnchorV1,
    observed_at: u64,
    trusted_at: u64,
}
struct QueryFloor {
    minimum: SignerCustodyAnchorV1,
    finality: FinalityFloorV1,
    not_before: u64,
    challenge: [u8; 32],
}
pub(super) struct SignerDriverV1 {
    pins: StreamTokenSignerPinsV1,
    client: Arc<dyn StreamTokenSignerClientV1>,
    observer: Arc<dyn StreamTokenStateObserverClientV1>,
    finality: Arc<dyn SignerFinalityV1>,
    clock: Arc<dyn SignerClockV1>,
    history: Mutex<History>,
}
impl SignerDriverV1 {
    pub(super) fn new(
        pins: StreamTokenSignerPinsV1,
        client: Arc<dyn StreamTokenSignerClientV1>,
        observer: Arc<dyn StreamTokenStateObserverClientV1>,
        approved: StreamTokenApprovedCustodyAnchorV1,
        finality: Arc<dyn SignerFinalityV1>,
        clock: Arc<dyn SignerClockV1>,
    ) -> Result<Self, StreamTokenIssuerError> {
        if approved.config_digest() != pins.config_digest() {
            return Err(StreamTokenIssuerError::SignerBindingMismatch);
        }
        let driver = Self {
            pins,
            client,
            observer,
            finality,
            clock,
            history: Mutex::new(History {
                anchor: approved.anchor(),
                observed_at: 0,
                trusted_at: 0,
            }),
        };
        driver.current(Phase::Startup, None)?;
        Ok(driver)
    }
    pub(super) fn pins(&self) -> &StreamTokenSignerPinsV1 {
        &self.pins
    }
    pub(super) fn now_unix_ms(&self) -> Result<u64, StreamTokenIssuerError> {
        let mut history = self
            .history
            .lock()
            .map_err(|_| StreamTokenIssuerError::SignerStateChanged)?;
        let now = self.clock.now_unix_ms()?;
        if now < history.trusted_at {
            return Err(StreamTokenIssuerError::SignerClockRollback);
        }
        history.trusted_at = now;
        Ok(now)
    }
    pub(super) fn require_completed_proof_source(&self) -> Result<(), StreamTokenIssuerError> {
        self.finality.require_completed_proof_source()
    }
    fn check_handles(&self) -> Result<(), StreamTokenIssuerError> {
        if self.client.handle() != self.pins.binding().runtime_handle
            || self.observer.handle() != self.pins.observer_handle()
        {
            return Err(StreamTokenIssuerError::SignerBindingMismatch);
        }
        Ok(())
    }
    fn query_floor(&self) -> Result<QueryFloor, StreamTokenIssuerError> {
        self.check_handles()?;
        let now = self.now_unix_ms()?;
        let history = self
            .history
            .lock()
            .map_err(|_| StreamTokenIssuerError::SignerStateChanged)?;
        let minimum = history.anchor;
        let not_before = now.max(history.trusted_at).max(history.observed_at);
        drop(history);
        let finality = self.finality.capture(minimum)?;
        let challenge = self.clock.challenge()?;
        if challenge == [0; 32] {
            return Err(StreamTokenIssuerError::RuntimeSignerUnavailable);
        }
        Ok(QueryFloor {
            minimum,
            finality,
            not_before,
            challenge,
        })
    }
    fn accept(
        &self,
        custody: &VerifiedSignerCustodyV1,
        observation: &SignerStreamTokenStateObservationBodyV1,
        floor: &QueryFloor,
        historical: &[HistoricalFinalityV1],
        token_window: Option<(u64, u64)>,
        completed: Option<&super::signer_completed_finality::CompletedFinalityV1>,
    ) -> Result<u64, StreamTokenIssuerError> {
        self.check_handles()?;
        let candidate = custody.current_anchor();
        let observed_at = observation.observed_at_unix_ms;
        let mut history = self
            .history
            .lock()
            .map_err(|_| StreamTokenIssuerError::SignerStateChanged)?;
        let now = self.clock.now_unix_ms()?;
        if now < history.trusted_at
            || observed_at < history.observed_at
            || observed_at > now
            || candidate.height < history.anchor.height
            || (candidate.height == history.anchor.height && candidate != history.anchor)
        {
            return Err(StreamTokenIssuerError::SignerStateChanged);
        }
        // Reobserve local finality after evidence authentication and while serializing publication
        // against concurrent successful observations. No runtime client I/O occurs under this lock.
        self.finality.validate(
            floor.minimum,
            candidate,
            floor.finality,
            historical,
            observation,
            completed,
        )?;
        self.finality.validate(
            history.anchor,
            candidate,
            floor.finality,
            historical,
            observation,
            completed,
        )?;
        self.check_handles()?;
        // Time is sampled again after potentially expensive durable finality reads. Every bound
        // below belongs to the same canonical observation already authenticated by the sole full
        // verifier; no Expected is reconstructed or reused to extend its lifetime.
        let now = self.clock.now_unix_ms()?;
        if now < history.trusted_at || now < custody.verified_at_unix_ms() {
            return Err(StreamTokenIssuerError::SignerClockRollback);
        }
        let earliest = now
            .checked_sub(self.pins.clock_uncertainty_ms())
            .ok_or_else(evidence_error)?;
        let latest = now
            .checked_add(self.pins.clock_uncertainty_ms())
            .ok_or_else(evidence_error)?;
        if earliest < custody.statement().issued_at_unix_ms
            || earliest < self.pins.custody_trust().active_from_unix_ms
            || earliest < self.pins.observer_trust().active_from_unix_ms
            || latest >= custody.statement().expires_at_unix_ms
            || latest >= self.pins.custody_trust().active_until_unix_ms
            || latest >= self.pins.observer_trust().active_until_unix_ms
            || latest >= observation.expires_at_unix_ms
            || now < observed_at
            || latest - observed_at > self.pins.custody_trust().max_anchor_age_ms
            || latest - observed_at > self.pins.observer_trust().max_state_age_ms
            || token_window.is_some_and(|(issued, expires)| {
                latest >= expires
                    || issued
                        > (earliest / 1_000).saturating_add(STREAM_TOKEN_MAX_FUTURE_SKEW_SECS_V1)
            })
        {
            return Err(evidence_error());
        }
        history.anchor = candidate;
        history.observed_at = observed_at;
        history.trusted_at = now;
        Ok(now)
    }
    fn current(
        &self,
        phase: Phase,
        token_window: Option<(u64, u64)>,
    ) -> Result<(VerifiedStreamTokenSignerQualificationV1, u64), StreamTokenIssuerError> {
        let floor = self.query_floor()?;
        let mut attempt = SignerStreamTokenObservationExpectedV1::current(
            self.pins.binding(),
            phase,
            floor.challenge,
            floor.minimum,
            floor.not_before,
        )
        .map_err(|error| evidence_admission_error(&error))?;
        // `attempt` is owned by this invocation and is dropped on transport failure; never cached,
        // reconstructed from a response or reused after any validation outcome.
        let reply = self
            .observer
            .observe(attempt.request())
            .map_err(map_call_error)?;
        self.check_handles()?;
        let (record, observation) = reply.current_evidence().ok_or_else(evidence_error)?;
        let decoded_observation =
            SignerStreamTokenStateObservationV1::decode_canonical(observation)
                .map_err(|error| evidence_admission_error(&error))?;
        let verified = verify_stream_token_signer_current_evidence_v1(
            record,
            observation,
            self.pins.binding(),
            self.pins.custody_trust(),
            self.pins.observer_trust(),
            &mut attempt,
            self.now_unix_ms()?,
        )
        .map_err(|error| evidence_admission_error(&error))?;
        let validated_at = self.accept(
            verified.custody(),
            &decoded_observation.body,
            &floor,
            &[HistoricalFinalityV1::Custody(
                verified.custody().statement().anchor,
            )],
            token_window,
            None,
        )?;
        Ok((verified, validated_at))
    }
    /// Authorize one new admission from fresh current custody and local finalized history.
    /// The returned time is sampled after observer I/O and both finality checks. This does not
    /// promise globally latest control state or cancellation of previously admitted streams.
    pub(super) fn before_admission(
        &self,
        body: &StreamTokenBodyV1,
    ) -> Result<u64, StreamTokenIssuerError> {
        // This pure preparation validates the bounded body and its exact provider/key generation;
        // it neither reserves an operation nor invokes the signing provider.
        SignerStreamTokenExpectedV1::new(body, self.pins.binding())
            .map_err(|_| StreamTokenIssuerError::SignerBindingMismatch)?;
        let expiry = body
            .ttl_epoch
            .checked_mul(1_000)
            .ok_or(StreamTokenIssuerError::TimeOverflow)?;
        let (_, validated_at) =
            self.current(Phase::BeforeAdmission, Some((body.issued_at, expiry)))?;
        Ok(validated_at)
    }
    pub(super) fn sign(
        &self,
        body: StreamTokenBodyV1,
    ) -> Result<StreamTokenV1, StreamTokenIssuerError> {
        let prepared = SignerStreamTokenExpectedV1::new(&body, self.pins.binding())
            .map_err(|_| evidence_error())?;
        // Production cannot make a signer reservation while the completed-operation proof
        // required for release has no authoritative source. This is before provider I/O.
        self.require_completed_proof_source()?;
        let (original, _) = self.current(Phase::BeforeProvider, None)?;
        self.check_handles()?;
        // The exact body and prepared operation survive an ambiguous result. Recovery is one
        // read-only lookup, never a second reservation, signature or HTTP issuance.
        let raw_receipt = match self.client.sign(&prepared, &body) {
            Ok(receipt) => receipt,
            Err(StreamTokenSignerCallErrorV1::AmbiguousCompletion) => {
                self.check_handles()?;
                self.client
                    .recover(&prepared, &body)
                    .map_err(map_call_error)?
            }
            Err(error) => return Err(map_call_error(error)),
        };
        self.check_handles()?;
        let claims = ReceiptClaims(
            SignerStreamTokenReceiptV1::decode_canonical(raw_receipt.bytes())
                .map_err(|_| evidence_error())?,
        );
        let mut signature = claims
            .0
            .role_signature_claim()
            .map_err(|_| evidence_error())?;
        let mut pending = PendingToken(Some(StreamTokenV1 {
            body,
            signature: signature.to_vec(),
        }));
        zeroize_value_for_confidential_discard(&mut signature);
        let signing_anchor = claims.0.provenance.signing_anchor;
        let key = self.pins.binding().public_key.to_bytes().1;
        let key_bytes: [u8; 32] = key.try_into().map_err(|_| evidence_error())?;
        let verifier = VerifyingKey::from_bytes(&key_bytes).map_err(|_| evidence_error())?;
        pending
            .token()
            .verify(&verifier)
            .map_err(|_| StreamTokenIssuerError::RuntimeSignerOutputInvalid)?;

        let after_floor = self.query_floor()?;
        let after_attempt = SignerStreamTokenObservationExpectedV1::completed(
            raw_receipt.bytes(),
            pending.token(),
            &prepared,
            self.pins.binding(),
            Phase::AfterCommit,
            after_floor.challenge,
            after_floor.minimum,
            after_floor.not_before,
        )
        .map_err(|error| evidence_admission_error(&error))?;
        let after_native = self.finality.prepare_completed_check(
            &claims.0,
            Phase::AfterCommit,
            self.observer.as_ref(),
        )?;
        let after_reply = self
            .observer
            .observe(after_attempt.request())
            .map_err(map_call_error)?;
        let after = completed_phase::Received::new(
            self,
            completed_phase::Inputs {
                receipt: &raw_receipt,
                token: &pending,
                prepared: &prepared,
                original: original.custody(),
                previous: None,
                signing_anchor,
            },
            after_floor,
            after_attempt,
            after_reply,
            after_native,
        )
        .authenticate_waiting()?
        .verify_native()?
        .accept()?;

        let release_floor = self.query_floor()?;
        let release_attempt = SignerStreamTokenObservationExpectedV1::completed(
            raw_receipt.bytes(),
            pending.token(),
            &prepared,
            self.pins.binding(),
            Phase::BeforeRelease,
            release_floor.challenge,
            release_floor.minimum,
            release_floor.not_before,
        )
        .map_err(|error| evidence_admission_error(&error))?;
        let release_native = self.finality.prepare_completed_check(
            &claims.0,
            Phase::BeforeRelease,
            self.observer.as_ref(),
        )?;
        let release_reply = self
            .observer
            .observe(release_attempt.request())
            .map_err(map_call_error)?;
        let release = completed_phase::Received::new(
            self,
            completed_phase::Inputs {
                receipt: &raw_receipt,
                token: &pending,
                prepared: &prepared,
                original: original.custody(),
                previous: Some(after.custody()),
                signing_anchor,
            },
            release_floor,
            release_attempt,
            release_reply,
            release_native,
        )
        .authenticate_waiting()?
        .verify_native()?
        .accept()?;
        // Retire both borrowed phase owners before discharging this exact pending token.
        drop(release);
        drop(after);
        // The only escape of the pending signature follows the fresh exact completed observation.
        pending.0.take().ok_or_else(evidence_error)
    }
}
struct ReceiptClaims(SignerStreamTokenReceiptV1);
impl Drop for ReceiptClaims {
    fn drop(&mut self) {
        for value in &mut self.0.signatures {
            zeroize_value_for_confidential_discard(&mut value.signature);
        }
    }
}
struct PendingToken(Option<StreamTokenV1>);
impl PendingToken {
    fn token(&self) -> &StreamTokenV1 {
        self.0
            .as_ref()
            .expect("pending token remains owned before release")
    }
}
impl Drop for PendingToken {
    fn drop(&mut self) {
        if let Some(token) = &mut self.0 {
            zeroize_value_for_confidential_discard(&mut token.signature);
        }
    }
}
// Fixed service classification outside the retained completed-reply owner. There is no retry
// here: current-only attempts retire, and pre-reply completed custody remains an explicit gate.
fn evidence_admission_error(
    error: &sorafs_manifest::signer::stream_token_evidence::SignerStreamTokenEvidenceAdmissionErrorV1,
) -> StreamTokenIssuerError {
    if error.is_retryable() {
        StreamTokenIssuerError::RuntimeSignerUnavailable
    } else {
        evidence_error()
    }
}
const fn evidence_error() -> StreamTokenIssuerError {
    StreamTokenIssuerError::SignerEvidenceInvalid
}
const fn map_call_error(error: StreamTokenSignerCallErrorV1) -> StreamTokenIssuerError {
    match error {
        StreamTokenSignerCallErrorV1::Unavailable
        | StreamTokenSignerCallErrorV1::AmbiguousCompletion => {
            StreamTokenIssuerError::RuntimeSignerUnavailable
        }
        StreamTokenSignerCallErrorV1::Refused => StreamTokenIssuerError::RuntimeSignerRefused,
        StreamTokenSignerCallErrorV1::InvalidResponse => {
            StreamTokenIssuerError::RuntimeSignerOutputInvalid
        }
    }
}
