//! One prepared token, one mutating call, and independently authenticated release fences.

use super::{
    StreamTokenApprovedCustodyAnchorV1, StreamTokenHardwareCallErrorV1,
    StreamTokenHardwareClientV1, StreamTokenHardwarePinsV1, StreamTokenIssuerError,
    StreamTokenStateObserverClientV1,
    hardware_finality::{FinalityFloorV1, HardwareFinalityV1},
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
};
use std::{
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) trait HardwareClockV1: Send + Sync {
    fn now_unix_ms(&self) -> Result<u64, StreamTokenIssuerError>;
    fn challenge(&self) -> Result<[u8; 32], StreamTokenIssuerError>;
}
pub(super) struct SystemHardwareClockV1;
impl HardwareClockV1 for SystemHardwareClockV1 {
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
pub(super) struct HardwareDriverV1 {
    pins: StreamTokenHardwarePinsV1,
    client: Arc<dyn StreamTokenHardwareClientV1>,
    observer: Arc<dyn StreamTokenStateObserverClientV1>,
    finality: Arc<dyn HardwareFinalityV1>,
    clock: Arc<dyn HardwareClockV1>,
    history: Mutex<History>,
}
impl HardwareDriverV1 {
    pub(super) fn new(
        pins: StreamTokenHardwarePinsV1,
        client: Arc<dyn StreamTokenHardwareClientV1>,
        observer: Arc<dyn StreamTokenStateObserverClientV1>,
        approved: StreamTokenApprovedCustodyAnchorV1,
        finality: Arc<dyn HardwareFinalityV1>,
        clock: Arc<dyn HardwareClockV1>,
    ) -> Result<Self, StreamTokenIssuerError> {
        if approved.config_digest() != pins.config_digest() {
            return Err(StreamTokenIssuerError::HardwareBindingMismatch);
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
        driver.current(Phase::Startup)?;
        Ok(driver)
    }
    pub(super) fn pins(&self) -> &StreamTokenHardwarePinsV1 {
        &self.pins
    }
    pub(super) fn now_unix_ms(&self) -> Result<u64, StreamTokenIssuerError> {
        let mut history = self
            .history
            .lock()
            .map_err(|_| StreamTokenIssuerError::HardwareStateChanged)?;
        let now = self.clock.now_unix_ms()?;
        if now < history.trusted_at {
            return Err(StreamTokenIssuerError::HardwareClockRollback);
        }
        history.trusted_at = now;
        Ok(now)
    }
    fn check_handles(&self) -> Result<(), StreamTokenIssuerError> {
        if self.client.handle() != self.pins.binding().runtime_handle
            || self.observer.handle() != self.pins.observer_handle()
        {
            return Err(StreamTokenIssuerError::HardwareBindingMismatch);
        }
        Ok(())
    }
    fn query_floor(&self) -> Result<QueryFloor, StreamTokenIssuerError> {
        self.check_handles()?;
        let now = self.now_unix_ms()?;
        let history = self
            .history
            .lock()
            .map_err(|_| StreamTokenIssuerError::HardwareStateChanged)?;
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
        historical: &[FinalityFloorV1],
        token_expiry: Option<u64>,
    ) -> Result<(), StreamTokenIssuerError> {
        self.check_handles()?;
        let candidate = custody.current_anchor();
        let observed_at = observation.observed_at_unix_ms;
        let mut history = self
            .history
            .lock()
            .map_err(|_| StreamTokenIssuerError::HardwareStateChanged)?;
        let now = self.clock.now_unix_ms()?;
        if now < history.trusted_at
            || observed_at < history.observed_at
            || observed_at > now
            || candidate.height < history.anchor.height
            || (candidate.height == history.anchor.height && candidate != history.anchor)
        {
            return Err(StreamTokenIssuerError::HardwareStateChanged);
        }
        // Reobserve local finality after evidence authentication and while serializing publication
        // against concurrent successful observations. No runtime client I/O occurs under this lock.
        self.finality
            .validate(floor.minimum, candidate, floor.finality, historical)?;
        self.finality
            .validate(history.anchor, candidate, floor.finality, historical)?;
        self.check_handles()?;
        // Time is sampled again after potentially expensive durable finality reads. Every bound
        // below belongs to the same canonical observation already authenticated by the sole full
        // verifier; no Expected is reconstructed or reused to extend its lifetime.
        let now = self.clock.now_unix_ms()?;
        if now < history.trusted_at || now < custody.verified_at_unix_ms() {
            return Err(StreamTokenIssuerError::HardwareClockRollback);
        }
        if now >= custody.statement().expires_at_unix_ms
            || now >= self.pins.custody_trust().active_until_unix_ms
            || now >= self.pins.observer_trust().active_until_unix_ms
            || now >= observation.expires_at_unix_ms
            || now < observed_at
            || now - observed_at > self.pins.custody_trust().max_anchor_age_ms
            || now - observed_at > self.pins.observer_trust().max_state_age_ms
            || token_expiry.is_some_and(|expires| now >= expires)
        {
            return Err(evidence_error());
        }
        history.anchor = candidate;
        history.observed_at = observed_at;
        history.trusted_at = now;
        Ok(())
    }
    fn current(
        &self,
        phase: Phase,
    ) -> Result<VerifiedStreamTokenSignerQualificationV1, StreamTokenIssuerError> {
        let floor = self.query_floor()?;
        let attempt = SignerStreamTokenObservationExpectedV1::current(
            self.pins.binding(),
            phase,
            floor.challenge,
            floor.minimum,
            floor.not_before,
        )
        .map_err(|_| evidence_error())?;
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
                .map_err(|_| evidence_error())?;
        let verified = verify_stream_token_signer_current_evidence_v1(
            record,
            observation,
            self.pins.binding(),
            self.pins.custody_trust(),
            self.pins.observer_trust(),
            attempt,
            self.now_unix_ms()?,
        )
        .map_err(|_| evidence_error())?;
        self.accept(
            verified.custody(),
            &decoded_observation.body,
            &floor,
            &[historical_block(verified.custody().statement().anchor)],
            None,
        )?;
        Ok(verified)
    }
    pub(super) fn sign(
        &self,
        body: StreamTokenBodyV1,
    ) -> Result<StreamTokenV1, StreamTokenIssuerError> {
        let prepared = SignerStreamTokenExpectedV1::new(&body, self.pins.binding())
            .map_err(|_| evidence_error())?;
        let original = self.current(Phase::BeforeProvider)?;
        self.check_handles()?;
        // The exact body and prepared operation survive an ambiguous result. Recovery is one
        // read-only lookup, never a second reservation, signature or HTTP issuance.
        let raw_receipt = match self.client.sign(&prepared, &body) {
            Ok(receipt) => receipt,
            Err(StreamTokenHardwareCallErrorV1::AmbiguousCompletion) => {
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
        drop(claims);
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
        .map_err(|_| evidence_error())?;
        let after_reply = self
            .observer
            .observe(after_attempt.request())
            .map_err(map_call_error)?;
        self.check_handles()?;
        let after_observation = after_reply
            .completed_observation()
            .ok_or_else(evidence_error)?;
        let after_decoded =
            SignerStreamTokenStateObservationV1::decode_canonical(after_observation)
                .map_err(|_| evidence_error())?;
        let after = verify_stream_token_signer_completed_observation_v1(
            raw_receipt.bytes(),
            after_observation,
            pending.token(),
            &prepared,
            self.pins.binding(),
            self.pins.custody_trust(),
            self.pins.observer_trust(),
            after_attempt,
            self.now_unix_ms()?,
        )
        .map_err(|_| evidence_error())?;
        if !after.custody().continues_active_state(original.custody()) {
            return Err(StreamTokenIssuerError::HardwareStateChanged);
        }
        let expiry = pending
            .token()
            .body
            .ttl_epoch
            .checked_mul(1_000)
            .ok_or(StreamTokenIssuerError::TimeOverflow)?;
        self.accept(
            after.custody(),
            &after_decoded.body,
            &after_floor,
            &[
                historical_block(after.custody().statement().anchor),
                historical_block(signing_anchor),
                FinalityFloorV1 {
                    height: after.completion().anchor.height,
                    block_hash: after.completion().anchor.block_hash,
                },
            ],
            Some(expiry),
        )?;

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
        .map_err(|_| evidence_error())?;
        let release_reply = self
            .observer
            .observe(release_attempt.request())
            .map_err(map_call_error)?;
        self.check_handles()?;
        let release_observation = release_reply
            .completed_observation()
            .ok_or_else(evidence_error)?;
        let release_decoded =
            SignerStreamTokenStateObservationV1::decode_canonical(release_observation)
                .map_err(|_| evidence_error())?;
        let released = verify_stream_token_signer_evidence_v1(
            raw_receipt.bytes(),
            release_observation,
            pending.token(),
            &prepared,
            self.pins.binding(),
            self.pins.custody_trust(),
            self.pins.observer_trust(),
            release_attempt,
            self.now_unix_ms()?,
        )
        .map_err(|_| evidence_error())?;
        if !released.custody().continues_active_state(after.custody())
            || !released
                .custody()
                .continues_active_state(original.custody())
        {
            return Err(StreamTokenIssuerError::HardwareStateChanged);
        }
        self.accept(
            released.custody(),
            &release_decoded.body,
            &release_floor,
            &[
                historical_block(released.custody().statement().anchor),
                historical_block(signing_anchor),
                FinalityFloorV1 {
                    height: released.completion().anchor.height,
                    block_hash: released.completion().anchor.block_hash,
                },
            ],
            Some(expiry),
        )?;
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
const fn evidence_error() -> StreamTokenIssuerError {
    StreamTokenIssuerError::HardwareEvidenceInvalid
}
const fn map_call_error(error: StreamTokenHardwareCallErrorV1) -> StreamTokenIssuerError {
    match error {
        StreamTokenHardwareCallErrorV1::Unavailable
        | StreamTokenHardwareCallErrorV1::AmbiguousCompletion => {
            StreamTokenIssuerError::RuntimeSignerUnavailable
        }
        StreamTokenHardwareCallErrorV1::Refused => StreamTokenIssuerError::RuntimeSignerRefused,
        StreamTokenHardwareCallErrorV1::InvalidResponse => {
            StreamTokenIssuerError::RuntimeSignerOutputInvalid
        }
    }
}

fn historical_block(anchor: SignerCustodyAnchorV1) -> FinalityFloorV1 {
    FinalityFloorV1 {
        height: anchor.height,
        block_hash: anchor.block_hash,
    }
}
