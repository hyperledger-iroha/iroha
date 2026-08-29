//! Adaptive threshold-BLS verification for Parliament timelock release.
//!
//! A [`TleKeySessionId`] names the long-lived, independently generated TLE
//! threshold key. It is deliberately distinct from the per-ballot
//! `governance::TleSessionId`. The persisted state in this module contains only
//! public DKG broadcasts, verification shares, transcript bindings, and public
//! release signatures. Dealer polynomials, recipient contributions, aggregate
//! signing shares, and proof nonces have no serializable representation here.
//!
//! Release shares are admitted only for the exact [`TleReleaseIdentityV1`] and
//! only after its target finalized height. Combining a canonical threshold
//! subset produces the unique standard BLS group signature; no signer bitmap
//! enters the final release record.
//!
//! “Adaptive” names the three-scalar Das--Ren protocol profile. It does not
//! assert a generic or standard-assumption adaptive-security theorem; the
//! precise model, cumulative corruption bound, lack of proactive refresh, and
//! 2026 key-uniqueness caveat are documented by
//! [`iroha_crypto::threshold_bls`].

use iroha_crypto::{
    threshold_bls::{
        AdaptiveThresholdBlsParameters, AdaptiveThresholdBlsPublicTranscript,
        AdaptiveThresholdBlsSecretShare, DasRenDealerCommitment, DasRenPartialSignature,
        ThresholdBlsError, ThresholdBlsSession, ThresholdBlsSignature, TleReleasePurpose,
        ValidatedDealerCommitment,
    },
    tle::{TleError, TleIdentitySecretKeyV1, TleMasterPublicKey, TleReleaseIdentityV1},
};
pub use iroha_data_model::governance::types::TleKeySessionId;
use iroha_data_model::governance::types::{
    BallotAttemptId, BallotAttemptStatusV1, GovernanceAttemptId, GovernanceAttemptStatusV1,
};
use mv::storage::StorageReadOnly;
use norito::{
    NoritoDeserialize, NoritoSerialize,
    derive::{JsonDeserialize, JsonSerialize},
};
use sha2::{Digest as _, Sha256};
use thiserror::Error;
use zeroize::Zeroizing;

use crate::{
    governance::timed_ovn::{
        TimedOvnEvidenceError, TimedOvnLifecycleStateV1, TimedOvnReleaseIdentityPublicV1,
    },
    state::{StateReadOnly, WorldReadOnly as _},
};

mod casting;
mod custody;
#[cfg(feature = "test-network-parliament-signers")]
#[doc(hidden)]
pub mod parliament_test_network_signer;
mod runtime;
pub(crate) use casting::derive_parliament_timed_ovn_casting_snapshot_v1;
pub use casting::{
    AuthorizedTimedOvnCastingContextV1, PARLIAMENT_TIMED_OVN_CASTING_CONTEXT_ARCHIVE_MAX_BYTES_V1,
    PARLIAMENT_TIMED_OVN_CASTING_CONTEXT_ARCHIVE_VERSION_V1,
    ParliamentTimedOvnCastingContextArchiveV1, ParliamentTimedOvnCastingPhaseV1,
    TimedOvnCastingArchiveValidationErrorV1, TimedOvnCastingAuthorizationErrorV1,
    ValidatedParliamentTimedOvnCastingContextArchiveV1,
    authorize_parliament_timed_ovn_casting_context_v1,
};
pub use custody::{RuntimeTleReleaseShareCustodyV1, TleReleaseShareCustodyErrorV1};
pub use runtime::{TleReleaseCoordinatorErrorV1, TleReleaseCoordinatorV1};

/// Fixed version of the public TLE key-session adapter.
pub const TLE_KEY_SESSION_ADAPTER_VERSION_V1: u16 = 1;
/// Fixed version of the authenticated-broker public release projection.
pub const TLE_AUTHORIZED_RELEASE_PROJECTION_VERSION_V1: u16 = 1;
/// Exact byte length of the V1 application identity payload.
pub const TLE_AUTHORIZED_RELEASE_IDENTITY_PAYLOAD_BYTES_V1: usize = 243;

/// Public coefficient commitments and constant-term proof for one qualified dealer.
#[derive(
    Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct TleAdaptiveDealerCommitmentV1 {
    /// Canonical one-based dealer index.
    pub dealer_index: u16,
    /// Exact degree-`f` triple-generator coefficient commitments.
    pub coefficient_commitments: Vec<[u8; 96]>,
    /// Schnorr commitment proving knowledge of the unblinded constant term.
    pub constant_pok_commitment: [u8; 96],
    /// Canonical big-endian Schnorr response scalar.
    pub constant_pok_response: [u8; 32],
}

impl TleAdaptiveDealerCommitmentV1 {
    fn from_validated(dealer: &ValidatedDealerCommitment<TleReleasePurpose>) -> Self {
        Self {
            dealer_index: dealer.dealer_index(),
            coefficient_commitments: dealer
                .coefficients()
                .iter()
                .map(|coefficient| *coefficient.as_bytes())
                .collect(),
            constant_pok_commitment: *dealer.constant_proof().commitment_bytes(),
            constant_pok_response: *dealer.constant_proof().response_bytes(),
        }
    }
}

/// One public composite verification share in a finalized adaptive TLE transcript.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
)]
pub struct TleAdaptivePublicShareV1 {
    /// Canonical one-based participant index.
    pub index: u16,
    /// Purpose- and roster-bound canonical seat digest.
    pub participant_hash: [u8; 32],
    /// Canonical compressed composite commitment `g^s h^r v^u`.
    pub public_key_share: [u8; 96],
}

/// Canonical, public-only state for one finalized adaptive TLE key session.
///
/// The qualified dealer commitments are retained so a restart can reconstruct
/// and revalidate the complete cryptographic transcript instead of trusting
/// cached public-key bytes.
#[derive(
    Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct TleKeySessionPublicStateV1 {
    /// Fixed adapter version.
    pub version: u16,
    /// Long-lived, purpose-distinct TLE threshold key identifier.
    pub key_session_id: TleKeySessionId,
    /// Canonical network/genesis binding.
    pub network_id: [u8; 32],
    /// Hash of the exact ordered threshold committee roster.
    pub roster_hash: [u8; 32],
    /// Exact `3f + 1` committee size.
    pub committee_size: u16,
    /// Exact `f + 1` release threshold.
    pub threshold: u16,
    /// Purpose- and session-derived independent Pedersen generator `h`.
    pub generator_h: [u8; 96],
    /// Purpose- and session-derived independent Pedersen generator `v`.
    pub generator_v: [u8; 96],
    /// Strictly increasing qualified dealer indices.
    pub qualified_dealers: Vec<u16>,
    /// Proof-validated public broadcasts aligned exactly with `qualified_dealers`.
    pub qualified_dealer_commitments: Vec<TleAdaptiveDealerCommitmentV1>,
    /// Consensus event hash binding complaints, responses, and qualification.
    pub dkg_event_hash: [u8; 32],
    /// Standard-generator aggregate group public key.
    pub group_public_key: [u8; 96],
    /// Complete canonical sequence of composite participant verification shares.
    pub public_shares: Vec<TleAdaptivePublicShareV1>,
    /// Commitment to the complete verified adaptive transcript.
    pub transcript_hash: [u8; 32],
}

impl TleKeySessionPublicStateV1 {
    /// Reconstruct and cryptographically validate this public-only state.
    ///
    /// # Errors
    ///
    /// Returns [`TleReleaseAdapterError`] for a wrong version, malformed DKG
    /// proof, noncanonical qualified set, or any cached transcript mismatch.
    pub fn validate(self) -> Result<ValidatedTleKeySessionV1, TleReleaseAdapterError> {
        ValidatedTleKeySessionV1::from_public_state(self)
    }
}

/// Runtime-validated adaptive TLE key session.
///
/// This value is deliberately not serializable. Persistence uses
/// [`TleKeySessionPublicStateV1`] and reconstructs this authenticated runtime
/// object by replaying every public proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTleKeySessionV1 {
    state: TleKeySessionPublicStateV1,
    transcript: AdaptiveThresholdBlsPublicTranscript<TleReleasePurpose>,
}

impl ValidatedTleKeySessionV1 {
    /// Finalize a canonical qualified-dealer set into public-only state.
    ///
    /// # Errors
    ///
    /// Returns [`TleReleaseAdapterError`] for malformed session bindings,
    /// insufficient or reordered dealers, or a failed adaptive transcript.
    pub fn from_qualified_dealers(
        session: ThresholdBlsSession<TleReleasePurpose>,
        validated_dealers: &[ValidatedDealerCommitment<TleReleasePurpose>],
        qualified_dealers: &[u16],
        dkg_event_hash: [u8; 32],
    ) -> Result<Self, TleReleaseAdapterError> {
        let parameters = AdaptiveThresholdBlsParameters::derive(&session)?;
        let transcript = AdaptiveThresholdBlsPublicTranscript::from_qualified_dealers(
            &parameters,
            validated_dealers,
            qualified_dealers,
            dkg_event_hash,
        )?;
        transcript.ensure_adaptive_protocol_ready()?;
        let key_session_id = TleKeySessionId::new(*session.session_id());
        let state = TleKeySessionPublicStateV1 {
            version: TLE_KEY_SESSION_ADAPTER_VERSION_V1,
            key_session_id,
            network_id: *session.network_id(),
            roster_hash: *session.roster_hash(),
            committee_size: session.committee_size(),
            threshold: session.threshold(),
            generator_h: *parameters.h_bytes(),
            generator_v: *parameters.v_bytes(),
            qualified_dealers: qualified_dealers.to_vec(),
            qualified_dealer_commitments: validated_dealers
                .iter()
                .map(TleAdaptiveDealerCommitmentV1::from_validated)
                .collect(),
            dkg_event_hash,
            group_public_key: *transcript.group_public_key().as_bytes(),
            public_shares: transcript
                .public_shares()
                .iter()
                .map(|share| TleAdaptivePublicShareV1 {
                    index: share.index(),
                    participant_hash: *share.participant_hash(),
                    public_key_share: *share.as_bytes(),
                })
                .collect(),
            transcript_hash: *transcript.transcript_hash(),
        };
        Ok(Self { state, transcript })
    }

    fn from_public_state(
        state: TleKeySessionPublicStateV1,
    ) -> Result<Self, TleReleaseAdapterError> {
        if state.version != TLE_KEY_SESSION_ADAPTER_VERSION_V1 {
            return Err(TleReleaseAdapterError::UnsupportedVersion);
        }
        if is_zero(state.key_session_id.as_bytes()) {
            return Err(TleReleaseAdapterError::ZeroKeySessionId);
        }
        let key_session_id = state.key_session_id;
        let session = ThresholdBlsSession::<TleReleasePurpose>::new(
            state.network_id,
            key_session_id.into_bytes(),
            state.roster_hash,
            state.committee_size,
            state.threshold,
        )?;
        let parameters = AdaptiveThresholdBlsParameters::derive(&session)?;
        if state.generator_h != *parameters.h_bytes() || state.generator_v != *parameters.v_bytes()
        {
            return Err(TleReleaseAdapterError::GeneratorMismatch);
        }
        if state.qualified_dealers.len() != state.qualified_dealer_commitments.len() {
            return Err(TleReleaseAdapterError::TranscriptMismatch);
        }
        let validated_dealers = state
            .qualified_dealer_commitments
            .iter()
            .zip(&state.qualified_dealers)
            .map(|(dealer, qualified_index)| {
                if dealer.dealer_index != *qualified_index {
                    return Err(ThresholdBlsError::NonCanonicalQualifiedSet);
                }
                DasRenDealerCommitment::verify(
                    &parameters,
                    dealer.dealer_index,
                    &dealer.coefficient_commitments,
                    dealer.constant_pok_commitment,
                    dealer.constant_pok_response,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let transcript = AdaptiveThresholdBlsPublicTranscript::from_qualified_dealers(
            &parameters,
            &validated_dealers,
            &state.qualified_dealers,
            state.dkg_event_hash,
        )?;
        transcript.ensure_adaptive_protocol_ready()?;
        let reconstructed_shares = transcript
            .public_shares()
            .iter()
            .map(|share| TleAdaptivePublicShareV1 {
                index: share.index(),
                participant_hash: *share.participant_hash(),
                public_key_share: *share.as_bytes(),
            })
            .collect::<Vec<_>>();
        if state.group_public_key != *transcript.group_public_key().as_bytes()
            || state.public_shares != reconstructed_shares
            || state.transcript_hash != *transcript.transcript_hash()
            || state.dkg_event_hash != *transcript.dkg_event_hash()
        {
            return Err(TleReleaseAdapterError::TranscriptMismatch);
        }
        Ok(Self { state, transcript })
    }

    /// Borrow the canonical public-only persistence state.
    #[must_use]
    pub const fn public_state(&self) -> &TleKeySessionPublicStateV1 {
        &self.state
    }

    /// Borrow the verified adaptive cryptographic transcript.
    #[must_use]
    pub const fn transcript(&self) -> &AdaptiveThresholdBlsPublicTranscript<TleReleasePurpose> {
        &self.transcript
    }

    /// Return the typed threshold-release master public key.
    #[must_use]
    pub const fn master_public_key(&self) -> TleMasterPublicKey {
        TleMasterPublicKey::from_threshold_key(*self.transcript.group_public_key())
    }

    /// Convert and verify one locally produced adaptive partial release.
    ///
    /// The returned record contains public proof material only. Consensus must
    /// still authenticate the sender-to-index mapping before admitting it.
    ///
    /// # Errors
    ///
    /// Returns [`TleReleaseAdapterError`] for an early release, wrong identity,
    /// transcript mismatch, or invalid partial proof.
    pub fn encode_partial_release(
        &self,
        identity: &TleReleaseIdentityV1,
        finalized_height: u64,
        partial: &DasRenPartialSignature<TleReleasePurpose>,
    ) -> Result<TlePartialReleaseShareV1, TleReleaseAdapterError> {
        let identity_digest = self.validate_release_identity(identity, finalized_height)?;
        self.transcript
            .verify_partial_signature(&identity.payload_bytes(), partial)?;
        let (z_s, z_r, z_u) = partial.response_bytes();
        Ok(TlePartialReleaseShareV1 {
            key_session_id: self.state.key_session_id,
            identity_digest,
            participant_index: partial.index(),
            sigma: *partial.sigma_bytes(),
            proof_x: *partial.proof_x_bytes(),
            proof_y: *partial.proof_y_bytes(),
            z_s: *z_s,
            z_r: *z_r,
            z_u: *z_u,
        })
    }

    /// Parse and verify one public partial release for the exact future identity.
    ///
    /// Sender authentication and the sender-to-participant-index mapping remain
    /// consensus responsibilities; this method verifies the cryptographic seat.
    ///
    /// # Errors
    ///
    /// Returns [`TleReleaseAdapterError`] for an early/wrong release, malformed
    /// point or scalar, replayed session, or failed adaptive proof.
    pub fn verify_partial_release(
        &self,
        identity: &TleReleaseIdentityV1,
        finalized_height: u64,
        record: &TlePartialReleaseShareV1,
    ) -> Result<VerifiedTlePartialReleaseShareV1, TleReleaseAdapterError> {
        let identity_digest = self.validate_release_identity(identity, finalized_height)?;
        if record.key_session_id != self.state.key_session_id
            || record.identity_digest != identity_digest
        {
            return Err(TleReleaseAdapterError::ReleaseBindingMismatch);
        }
        let partial = DasRenPartialSignature::from_bytes(
            self.state.key_session_id.into_bytes(),
            record.participant_index,
            record.sigma,
            record.proof_x,
            record.proof_y,
            record.z_s,
            record.z_r,
            record.z_u,
        )?;
        self.transcript
            .verify_partial_signature(&identity.payload_bytes(), &partial)?;
        Ok(VerifiedTlePartialReleaseShareV1 {
            record: record.clone(),
            transcript_hash: self.state.transcript_hash,
            partial,
        })
    }

    /// Verify and combine canonical public partial-release records.
    ///
    /// # Errors
    ///
    /// Returns [`TleReleaseAdapterError`] for an invalid share or a subset that
    /// is insufficient, duplicated, reordered, or bound to another identity.
    pub fn combine_partial_releases(
        &self,
        identity: &TleReleaseIdentityV1,
        finalized_height: u64,
        records: &[TlePartialReleaseShareV1],
    ) -> Result<TleFinalReleaseSignatureV1, TleReleaseAdapterError> {
        let verified = records
            .iter()
            .map(|record| self.verify_partial_release(identity, finalized_height, record))
            .collect::<Result<Vec<_>, _>>()?;
        self.combine_verified_partial_releases(identity, finalized_height, &verified)
    }

    /// Combine already verified shares and final-verify the unique release signature.
    ///
    /// # Errors
    ///
    /// Returns [`TleReleaseAdapterError`] for a cross-transcript wrapper,
    /// insufficient/noncanonical subset, or invalid final BLS signature.
    pub fn combine_verified_partial_releases(
        &self,
        identity: &TleReleaseIdentityV1,
        finalized_height: u64,
        shares: &[VerifiedTlePartialReleaseShareV1],
    ) -> Result<TleFinalReleaseSignatureV1, TleReleaseAdapterError> {
        let identity_digest = self.validate_release_identity(identity, finalized_height)?;
        for share in shares {
            if share.transcript_hash != self.state.transcript_hash
                || share.record.key_session_id != self.state.key_session_id
                || share.record.identity_digest != identity_digest
            {
                return Err(TleReleaseAdapterError::ReleaseBindingMismatch);
            }
        }
        let partials = shares.iter().map(|share| share.partial).collect::<Vec<_>>();
        let signature = self
            .transcript
            .combine_partial_signatures(&identity.payload_bytes(), &partials)?;
        let record = TleFinalReleaseSignatureV1 {
            key_session_id: self.state.key_session_id,
            identity_digest,
            signature: *signature.as_bytes(),
        };
        self.verify_final_release(identity, finalized_height, &record)?;
        Ok(record)
    }

    /// Verify a public final release against the exact future identity.
    ///
    /// # Errors
    ///
    /// Returns [`TleReleaseAdapterError`] for an early/wrong release,
    /// malformed signature, or failed final pairing verification.
    pub fn verify_final_release(
        &self,
        identity: &TleReleaseIdentityV1,
        finalized_height: u64,
        record: &TleFinalReleaseSignatureV1,
    ) -> Result<(), TleReleaseAdapterError> {
        let identity_digest = self.validate_release_identity(identity, finalized_height)?;
        if record.key_session_id != self.state.key_session_id
            || record.identity_digest != identity_digest
        {
            return Err(TleReleaseAdapterError::ReleaseBindingMismatch);
        }
        let signature = ThresholdBlsSignature::<TleReleasePurpose>::from_bytes(
            self.state.key_session_id.into_bytes(),
            &record.signature,
        )?;
        self.transcript
            .verify_final_signature(&identity.payload_bytes(), &signature)?;
        // Reuse the TLE identity-key verifier as a second typed binding check.
        // The zeroizing owner is immediately dropped; persistence retains only
        // the public final signature record.
        let release_key = TleIdentitySecretKeyV1::from_threshold_signature(
            self.master_public_key(),
            identity,
            &record.signature,
        )?;
        drop(release_key);
        Ok(())
    }

    /// Construct the zeroizing release key used by the folded aggregate opener.
    ///
    /// The returned runtime value has no serialization API. Callers must not
    /// persist or log it; the canonical persisted artifact is `record`.
    ///
    /// # Errors
    ///
    /// Returns [`TleReleaseAdapterError`] unless the target height has been
    /// finalized and the exact public final signature verifies.
    pub fn release_key_for_opening(
        &self,
        identity: &TleReleaseIdentityV1,
        finalized_height: u64,
        record: &TleFinalReleaseSignatureV1,
    ) -> Result<TleIdentitySecretKeyV1, TleReleaseAdapterError> {
        self.verify_final_release(identity, finalized_height, record)?;
        Ok(TleIdentitySecretKeyV1::from_threshold_signature(
            self.master_public_key(),
            identity,
            &record.signature,
        )?)
    }

    fn validate_release_identity(
        &self,
        identity: &TleReleaseIdentityV1,
        finalized_height: u64,
    ) -> Result<[u8; 32], TleReleaseAdapterError> {
        if identity.session() != self.transcript.session()
            || identity.session().session_id() != self.state.key_session_id.as_bytes()
        {
            return Err(TleReleaseAdapterError::ReleaseBindingMismatch);
        }
        if finalized_height < identity.target_finalized_height() {
            return Err(TleReleaseAdapterError::ReleaseHeightNotReached);
        }
        Ok(Sha256::digest(identity.release_message()?).into())
    }
}

/// Public-only wire projection of one Core-authorized TLE release.
///
/// This type carries no secret material, provider handle, or signing
/// capability. It exists solely so an authenticated local runtime broker can
/// revalidate the exact public session and release statement supplied by the
/// daemon. Cryptographic validation does **not** prove that a projection came
/// from committed state; broker transport must admit it only from the scoped
/// daemon session, and the daemon must construct it through
/// [`AuthorizedTleReleaseContextV1::broker_projection_v1`].
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct AuthorizedTleReleaseProjectionV1 {
    /// Fixed projection layout version.
    pub version: u16,
    /// Exact ballot attempt authorized by Core.
    pub ballot_attempt_id: BallotAttemptId,
    /// Inclusive final height of the aggregate-opening window.
    pub opening_deadline_height: u64,
    /// Finalized height observed by the authorizing committed view.
    pub finalized_height: u64,
    /// Complete proof-carrying public threshold-key transcript.
    pub key_session: TleKeySessionPublicStateV1,
    /// Frozen public timed-OVN release identity.
    pub public_release_identity: TimedOvnReleaseIdentityPublicV1,
    /// Exact fixed-size application identity payload.
    pub identity_payload: [u8; TLE_AUTHORIZED_RELEASE_IDENTITY_PAYLOAD_BYTES_V1],
    /// SHA-256 of the exact threshold-session-framed release message.
    pub identity_digest: [u8; 32],
}

impl AuthorizedTleReleaseProjectionV1 {
    /// Reconstruct and validate every public cryptographic and height binding.
    ///
    /// The result is intentionally distinct from
    /// [`AuthorizedTleReleaseContextV1`]. A valid wire projection is not a Core
    /// authorization capability and cannot be converted into one.
    ///
    /// # Errors
    ///
    /// Returns a closed error for a wrong version, malformed public DKG state,
    /// inconsistent identity, invalid height window, payload mismatch, or
    /// digest mismatch.
    pub fn validate(self) -> Result<ValidatedTleReleaseProjectionV1, TleReleaseProjectionErrorV1> {
        if self.version != TLE_AUTHORIZED_RELEASE_PROJECTION_VERSION_V1 {
            return Err(TleReleaseProjectionErrorV1::UnsupportedVersion);
        }
        if self
            .ballot_attempt_id
            .as_bytes()
            .iter()
            .all(|byte| *byte == 0)
            || self.ballot_attempt_id.as_bytes() != &self.public_release_identity.ballot_attempt_id
            || self.key_session.key_session_id != self.public_release_identity.tle_key_session_id
        {
            return Err(TleReleaseProjectionErrorV1::BindingMismatch);
        }
        if self.finalized_height < self.public_release_identity.target_finalized_height
            || self.finalized_height > self.opening_deadline_height
            || self.opening_deadline_height < self.public_release_identity.target_finalized_height
        {
            return Err(TleReleaseProjectionErrorV1::InvalidHeightWindow);
        }

        let session = self.key_session.clone().validate()?;
        let identity = TleReleaseIdentityV1::new(
            *session.transcript().session(),
            self.public_release_identity.governance_attempt_id,
            self.public_release_identity.body_instance_id,
            self.public_release_identity.ballot_attempt_id,
            self.public_release_identity.survivor_corpus_root,
            self.public_release_identity.no_recovery_root,
            self.public_release_identity.target_finalized_height,
            self.public_release_identity.parameter_hash,
        )?;
        let expected_payload: [u8; TLE_AUTHORIZED_RELEASE_IDENTITY_PAYLOAD_BYTES_V1] = identity
            .payload_bytes()
            .try_into()
            .map_err(|_| TleReleaseProjectionErrorV1::IdentityPayloadMismatch)?;
        if self.identity_payload != expected_payload {
            return Err(TleReleaseProjectionErrorV1::IdentityPayloadMismatch);
        }
        let identity_digest =
            session.validate_release_identity(&identity, self.finalized_height)?;
        if self.identity_digest != identity_digest {
            return Err(TleReleaseProjectionErrorV1::IdentityDigestMismatch);
        }
        Ok(ValidatedTleReleaseProjectionV1 {
            projection: self,
            session,
            identity,
        })
    }
}

/// Revalidated public statement admitted by an authenticated runtime broker.
///
/// The value has no serialization implementation and is not a substitute for
/// Core's opaque committed-state authorization.
#[derive(Debug, Clone)]
pub struct ValidatedTleReleaseProjectionV1 {
    projection: AuthorizedTleReleaseProjectionV1,
    session: ValidatedTleKeySessionV1,
    identity: TleReleaseIdentityV1,
}

impl ValidatedTleReleaseProjectionV1 {
    /// Borrow the complete validated public projection.
    #[must_use]
    pub const fn projection(&self) -> &AuthorizedTleReleaseProjectionV1 {
        &self.projection
    }

    /// Borrow the reconstructed, proof-validated public key session.
    #[must_use]
    pub const fn session(&self) -> &ValidatedTleKeySessionV1 {
        &self.session
    }

    /// Borrow the exact reconstructed threshold release identity.
    #[must_use]
    pub const fn identity(&self) -> &TleReleaseIdentityV1 {
        &self.identity
    }

    /// Return the finalized height carried by the authenticated broker request.
    #[must_use]
    pub const fn finalized_height(&self) -> u64 {
        self.projection.finalized_height
    }
}

/// Closed failures while validating a public authenticated-broker projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TleReleaseProjectionErrorV1 {
    /// The broker projection used an unsupported layout version.
    #[error("unsupported Parliament TLE release projection version")]
    UnsupportedVersion,
    /// Public key-session, ballot, or release-identity bindings disagreed.
    #[error("Parliament TLE release projection binding mismatch")]
    BindingMismatch,
    /// The target, observed, and opening-deadline heights were inconsistent.
    #[error("Parliament TLE release projection height window is invalid")]
    InvalidHeightWindow,
    /// The transmitted fixed-size application payload was not canonical.
    #[error("Parliament TLE release projection identity payload mismatch")]
    IdentityPayloadMismatch,
    /// The transmitted threshold-framed identity digest was not canonical.
    #[error("Parliament TLE release projection identity digest mismatch")]
    IdentityDigestMismatch,
    /// Public key-session or release-identity cryptography was invalid.
    #[error(transparent)]
    Release(#[from] TleReleaseAdapterError),
    /// The typed release identity was invalid.
    #[error(transparent)]
    Identity(#[from] TleError),
}

/// Constructor-authenticated authorization for one committed TLE release share.
///
/// Callers cannot construct this type directly. Core issues it only for a
/// replay-valid, sealed timed-OVN corpus whose Parliament ballot has already
/// consumed its release beacon and entered `Opening`. The current finalized
/// height must also be within the ballot's inclusive opening window.
#[derive(Debug, Clone)]
pub struct AuthorizedTleReleaseContextV1 {
    ballot_attempt_id: BallotAttemptId,
    opening_deadline_height: u64,
    finalized_height: u64,
    public_release_identity: TimedOvnReleaseIdentityPublicV1,
    identity: TleReleaseIdentityV1,
    session: ValidatedTleKeySessionV1,
}

impl AuthorizedTleReleaseContextV1 {
    /// Return the exact committed ballot attempt authorized for release.
    #[must_use]
    pub const fn ballot_attempt_id(&self) -> BallotAttemptId {
        self.ballot_attempt_id
    }

    /// Return the finalized height at which Core authorized the share.
    #[must_use]
    pub const fn finalized_height(&self) -> u64 {
        self.finalized_height
    }

    /// Return the inclusive last height at which opening may complete.
    #[must_use]
    pub const fn opening_deadline_height(&self) -> u64 {
        self.opening_deadline_height
    }

    /// Borrow the bounded public release identity stored with timed-OVN evidence.
    #[must_use]
    pub const fn public_release_identity(&self) -> &TimedOvnReleaseIdentityPublicV1 {
        &self.public_release_identity
    }

    /// Borrow the fully reconstructed threshold-signing identity.
    #[must_use]
    pub const fn identity(&self) -> &TleReleaseIdentityV1 {
        &self.identity
    }

    /// Borrow the proof-revalidated public threshold-key session.
    #[must_use]
    pub const fn session(&self) -> &ValidatedTleKeySessionV1 {
        &self.session
    }

    /// Build the exact public-only request sent to an authenticated runtime broker.
    ///
    /// This is the only production constructor for the broker projection. The
    /// returned data is not itself an authorization capability; the broker must
    /// scope admission to the authenticated daemon connection and revalidate it
    /// before entering a projected signer.
    ///
    /// # Errors
    ///
    /// Returns a closed error if the fixed V1 application payload width or
    /// identity digest cannot be reproduced from this Core authorization.
    pub fn broker_projection_v1(
        &self,
    ) -> Result<AuthorizedTleReleaseProjectionV1, TleReleaseProjectionErrorV1> {
        let identity_payload: [u8; TLE_AUTHORIZED_RELEASE_IDENTITY_PAYLOAD_BYTES_V1] = self
            .identity
            .payload_bytes()
            .try_into()
            .map_err(|_| TleReleaseProjectionErrorV1::IdentityPayloadMismatch)?;
        let identity_digest = self
            .session
            .validate_release_identity(&self.identity, self.finalized_height)?;
        Ok(AuthorizedTleReleaseProjectionV1 {
            version: TLE_AUTHORIZED_RELEASE_PROJECTION_VERSION_V1,
            ballot_attempt_id: self.ballot_attempt_id,
            opening_deadline_height: self.opening_deadline_height,
            finalized_height: self.finalized_height,
            key_session: self.session.public_state().clone(),
            public_release_identity: self.public_release_identity,
            identity_payload,
            identity_digest,
        })
    }
}

/// Authorize a TLE release from one point-in-time committed state view.
///
/// This is the only public constructor for [`AuthorizedTleReleaseContextV1`].
/// It joins the exact Parliament attempt, ballot, timed-OVN lifecycle, and TLE
/// transcript and then replays all public cryptographic evidence. In
/// particular, an `AwaitingRelease` ballot is not enough: the committed release
/// beacon must already have advanced it to `Opening`.
///
/// # Errors
///
/// Returns [`TleReleaseAuthorizationErrorV1`] when any state component is
/// absent, terminal, early, expired, malformed, or cross-bound.
pub fn authorize_parliament_tle_release_v1(
    state: &impl StateReadOnly,
    ballot_attempt_id: BallotAttemptId,
) -> Result<AuthorizedTleReleaseContextV1, TleReleaseAuthorizationErrorV1> {
    let finalized_height = u64::try_from(state.height())
        .map_err(|_| TleReleaseAuthorizationErrorV1::HeightOverflow)?;
    let world = state.world();
    let lifecycle = world
        .timed_ovn_evidence()
        .get(&ballot_attempt_id)
        .ok_or(TleReleaseAuthorizationErrorV1::MissingTimedOvnEvidence)?;
    if lifecycle.ballot_attempt_id() != *ballot_attempt_id.as_bytes() {
        return Err(TleReleaseAuthorizationErrorV1::BindingMismatch);
    }
    let TimedOvnLifecycleStateV1::Sealed(sealed) = lifecycle else {
        return Err(TleReleaseAuthorizationErrorV1::TimedOvnNotSealed);
    };
    let key_session_id = lifecycle.tle_key_session_id();
    let session = world
        .tle_key_sessions()
        .get(&key_session_id)
        .cloned()
        .ok_or(TleReleaseAuthorizationErrorV1::MissingKeySession)?
        .validate()?;
    let validated_evidence = sealed.clone().validate(&session)?;
    let identity = *validated_evidence.release_identity();
    let public_release_identity = sealed.release_identity;

    let governance_attempt_id = GovernanceAttemptId::new(lifecycle.session().governance_attempt_id);
    let attempt = world
        .parliament_attempts()
        .get(&governance_attempt_id)
        .ok_or(TleReleaseAuthorizationErrorV1::MissingGovernanceAttempt)?;
    attempt
        .validate()
        .map_err(|_| TleReleaseAuthorizationErrorV1::InvalidParliamentState)?;
    if attempt.attempt().status != GovernanceAttemptStatusV1::Active {
        return Err(TleReleaseAuthorizationErrorV1::GovernanceAttemptNotActive);
    }
    let ballot = attempt
        .ballot(&ballot_attempt_id)
        .ok_or(TleReleaseAuthorizationErrorV1::MissingBallot)?;
    if ballot.attempt().status != BallotAttemptStatusV1::Opening {
        return Err(TleReleaseAuthorizationErrorV1::BallotNotOpening);
    }

    let target_height = lifecycle.target_finalized_height();
    if attempt.proposal_content_id().as_bytes() != &lifecycle.session().proposal_content_id
        || ballot.attempt().body_instance_id.as_bytes() != &lifecycle.session().body_instance_id
        || ballot.tle_key_session_id() != Some(key_session_id)
        || ballot.release_height() != Some(target_height)
        || identity.governance_attempt_id() != governance_attempt_id.as_bytes()
        || identity.body_instance_id() != ballot.attempt().body_instance_id.as_bytes()
        || identity.ballot_attempt_id() != ballot_attempt_id.as_bytes()
        || identity.target_finalized_height() != target_height
    {
        return Err(TleReleaseAuthorizationErrorV1::BindingMismatch);
    }
    if finalized_height < target_height {
        return Err(TleReleaseAuthorizationErrorV1::ReleaseHeightNotReached);
    }
    let opening_deadline_height = ballot.opening_deadline_height();
    if finalized_height > opening_deadline_height {
        return Err(TleReleaseAuthorizationErrorV1::OpeningDeadlinePassed);
    }
    session.validate_release_identity(&identity, finalized_height)?;

    Ok(AuthorizedTleReleaseContextV1 {
        ballot_attempt_id,
        opening_deadline_height,
        finalized_height,
        public_release_identity,
        identity,
        session,
    })
}

/// Fail-closed reasons Core could not authorize a runtime TLE release share.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TleReleaseAuthorizationErrorV1 {
    /// The committed state height cannot be represented by the v1 wire type.
    #[error("committed state height does not fit the TLE release protocol")]
    HeightOverflow,
    /// No public timed-OVN lifecycle exists for the requested ballot.
    #[error("timed-OVN evidence is missing for the ballot")]
    MissingTimedOvnEvidence,
    /// The timed-OVN lifecycle has not reached its complete sealed corpus.
    #[error("timed-OVN evidence is not sealed for release")]
    TimedOvnNotSealed,
    /// The referenced public TLE key transcript is absent.
    #[error("TLE key session is missing")]
    MissingKeySession,
    /// The embedded governance attempt is absent.
    #[error("Parliament governance attempt is missing")]
    MissingGovernanceAttempt,
    /// The embedded ballot attempt is absent from its governance attempt.
    #[error("Parliament ballot attempt is missing")]
    MissingBallot,
    /// The reducer state failed its complete deterministic invariant check.
    #[error("Parliament reducer state is invalid")]
    InvalidParliamentState,
    /// The governance attempt is terminal rather than active.
    #[error("Parliament governance attempt is not active")]
    GovernanceAttemptNotActive,
    /// The release beacon has not advanced this ballot into `Opening`.
    #[error("Parliament ballot is not in the opening phase")]
    BallotNotOpening,
    /// Two committed objects disagree on an immutable release binding.
    #[error("Parliament TLE release state has inconsistent bindings")]
    BindingMismatch,
    /// The target finalized height has not yet been reached.
    #[error("TLE release target finalized height has not been reached")]
    ReleaseHeightNotReached,
    /// The inclusive aggregate-opening deadline has elapsed.
    #[error("Parliament aggregate-opening deadline has passed")]
    OpeningDeadlinePassed,
    /// The public threshold-key transcript is malformed or inconsistent.
    #[error(transparent)]
    KeySession(#[from] TleReleaseAdapterError),
    /// The sealed timed-OVN corpus does not replay exactly.
    #[error(transparent)]
    TimedOvn(#[from] TimedOvnEvidenceError),
}

/// Runtime-only owner capable of producing one authorized adaptive TLE release share.
///
/// Implementations are injected by the deployment's secure runtime boundary.
/// Accepting only [`AuthorizedTleReleaseContextV1`] keeps caller-supplied
/// identities outside the signing boundary and prevents a node from becoming
/// a generic threshold-BLS signing oracle. Private DKG components must never
/// enter configuration, World state, logs, or wire DTOs.
///
/// A production provider may own multiple retiring and active key-session
/// shares. It must select only by the authorized context's exact
/// `key_session_id` and retain every retiring share through the last committed
/// ballot opening deadline that references it. The single-session
/// [`InMemoryTlePartialReleaseSignerV1`] is only a software adapter and test
/// provider, not a global key-rotation policy.
pub trait TlePartialReleaseSignerV1: Send + Sync {
    /// Attest non-secret custody for one exact public session and participant seat.
    ///
    /// Implementations must perform a live lookup in the same custody object later
    /// used by [`Self::sign_partial_release`]; constructing an attestation from
    /// public state alone is not proof of custody.
    ///
    /// # Errors
    ///
    /// Returns a closed capability error when the provider cannot perform the
    /// lookup or does not own the exact session and seat.
    fn attest_partial_release_capability(
        &self,
        session: &ValidatedTleKeySessionV1,
        expected_participant_index: u16,
    ) -> Result<TlePartialReleaseCapabilityAttestationV1, TlePartialReleaseCapabilityErrorV1>;

    /// Sign the exact Core-authorized committed future identity.
    ///
    /// # Errors
    ///
    /// Returns a non-secret diagnostic when the requested key session is not
    /// owned by this provider, the target height has not been finalized, or
    /// the secure runtime cannot produce a valid share.
    fn sign_partial_release(
        &self,
        context: &AuthorizedTleReleaseContextV1,
    ) -> Result<TlePartialReleaseShareV1, String>;
}

/// Non-secret readiness attestation for one runtime provider's exact TLE share.
///
/// The fields identify only public transcript material and a public one-based
/// committee seat. Callers must exact-match the returned value against their
/// committed session and expected local seat. The value is intentionally not
/// serializable as a ledger object; broker adapters use their own authenticated
/// runtime wire envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TlePartialReleaseCapabilityAttestationV1 {
    key_session_id: TleKeySessionId,
    transcript_hash: [u8; 32],
    participant_index: u16,
}

impl TlePartialReleaseCapabilityAttestationV1 {
    /// Construct the exact public attestation expected for a validated session seat.
    ///
    /// # Errors
    ///
    /// Returns [`TlePartialReleaseCapabilityErrorV1::InvalidRequest`] when the
    /// one-based seat does not exist in the validated public transcript.
    pub fn for_validated_session(
        session: &ValidatedTleKeySessionV1,
        participant_index: u16,
    ) -> Result<Self, TlePartialReleaseCapabilityErrorV1> {
        if !session
            .public_state()
            .public_shares
            .iter()
            .any(|share| share.index == participant_index)
        {
            return Err(TlePartialReleaseCapabilityErrorV1::InvalidRequest);
        }
        Ok(Self {
            key_session_id: session.public_state().key_session_id,
            transcript_hash: session.public_state().transcript_hash,
            participant_index,
        })
    }

    /// Return the exact public key-session identifier.
    #[must_use]
    pub const fn key_session_id(self) -> TleKeySessionId {
        self.key_session_id
    }

    /// Return the exact validated public-transcript hash.
    #[must_use]
    pub const fn transcript_hash(self) -> [u8; 32] {
        self.transcript_hash
    }

    /// Return the exact one-based participant seat.
    #[must_use]
    pub const fn participant_index(self) -> u16 {
        self.participant_index
    }

    /// Return whether this attestation exactly matches a committed session seat.
    #[must_use]
    pub fn matches(self, session: &ValidatedTleKeySessionV1, participant_index: u16) -> bool {
        self.key_session_id == session.public_state().key_session_id
            && self.transcript_hash == session.public_state().transcript_hash
            && self.participant_index == participant_index
    }
}

/// Closed failure classes for non-signing Parliament TLE custody attestation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TlePartialReleaseCapabilityErrorV1 {
    /// The secure runtime or authenticated lookup is temporarily unavailable.
    #[error("Parliament TLE release capability attestation is unavailable")]
    Unavailable,
    /// The provider does not own the exact requested session and participant seat.
    #[error("Parliament TLE release capability is not owned")]
    NotOwned,
    /// The requested one-based participant seat is absent from the public transcript.
    #[error("Parliament TLE release capability request is invalid")]
    InvalidRequest,
}

/// Runtime broker backend capable of signing one revalidated public projection.
///
/// This is a deliberately separate surface from [`TlePartialReleaseSignerV1`].
/// A [`ValidatedTleReleaseProjectionV1`] proves only that its public transcript,
/// identity, and height bindings are internally valid; it does not prove that
/// the request came from committed state. Implementations must therefore be
/// reachable only through an authenticated broker session scoped to an Iroha
/// daemon. The daemon must independently verify every returned public share.
pub trait TleProjectedPartialReleaseSignerV1: Send + Sync {
    /// Sign the exact public statement revalidated at the broker boundary.
    ///
    /// # Errors
    ///
    /// Returns a non-secret diagnostic when the requested key session is not
    /// owned, the height binding is invalid, or the secure runtime cannot
    /// produce a valid proof-carrying share.
    fn sign_projected_partial_release(
        &self,
        projection: &ValidatedTleReleaseProjectionV1,
    ) -> Result<TlePartialReleaseShareV1, String>;
}

/// Process-local zeroizing software owner for one adaptive TLE signing share.
///
/// This adapter is for deployments whose secure runtime unwraps a share into
/// process memory. It deliberately has no `Clone`, `Debug`, byte export, or
/// serialization implementation. In-process HSM/KMS integrations implement
/// [`TlePartialReleaseSignerV1`] directly; backends reached through the
/// authenticated runtime broker implement
/// [`TleProjectedPartialReleaseSignerV1`].
pub struct InMemoryTlePartialReleaseSignerV1 {
    session: ValidatedTleKeySessionV1,
    share: AdaptiveThresholdBlsSecretShare<TleReleasePurpose>,
}

impl InMemoryTlePartialReleaseSignerV1 {
    /// Move one validated adaptive share into the live signer.
    ///
    /// # Errors
    ///
    /// Returns an adapter error when the share belongs to another public
    /// session or transcript.
    pub fn from_validated_share(
        session: ValidatedTleKeySessionV1,
        share: AdaptiveThresholdBlsSecretShare<TleReleasePurpose>,
    ) -> Result<Self, TleReleaseAdapterError> {
        let mut hasher = Sha256::new();
        hasher.update(b"iroha.parliament.tle-release.runtime-share-import.v1\0");
        hasher.update(session.public_state().key_session_id.as_bytes());
        hasher.update(session.public_state().transcript_hash);
        let import_challenge: [u8; 32] = hasher.finalize().into();
        let partial = share.sign_payload(session.transcript(), &import_challenge)?;
        session
            .transcript()
            .verify_partial_signature(&import_challenge, &partial)?;
        Ok(Self { session, share })
    }

    /// Import three sealed scalar components and consume their zeroizing buffer.
    ///
    /// # Errors
    ///
    /// Returns an adapter error if the public transcript or secret share does
    /// not match the frozen participant seat.
    pub fn from_components(
        public_state: TleKeySessionPublicStateV1,
        participant_index: u16,
        components: Zeroizing<[[u8; 32]; 3]>,
    ) -> Result<Self, TleReleaseAdapterError> {
        let session = public_state.validate()?;
        let share = AdaptiveThresholdBlsSecretShare::from_components(
            session.transcript(),
            participant_index,
            components[0],
            components[1],
            components[2],
        )?;
        Self::from_validated_share(session, share)
    }

    /// Return the one-based frozen DKG participant seat.
    #[must_use]
    pub const fn participant_index(&self) -> u16 {
        self.share.index()
    }

    /// Return the exact public key-session identifier owned by this adapter.
    ///
    /// This exposes only the already-public transcript identifier. The share,
    /// participant inventory, and scalar components remain inaccessible.
    #[must_use]
    pub const fn key_session_id(&self) -> TleKeySessionId {
        self.session.public_state().key_session_id
    }

    fn sign_validated_release(
        &self,
        session: &ValidatedTleKeySessionV1,
        identity: &TleReleaseIdentityV1,
        finalized_height: u64,
    ) -> Result<TlePartialReleaseShareV1, String> {
        if session.public_state() != self.session.public_state() {
            return Err("requested TLE key session does not match the sealed share".to_owned());
        }
        session
            .validate_release_identity(identity, finalized_height)
            .map_err(|error| format!("TLE release identity rejected: {error}"))?;
        let partial = self
            .share
            .sign_payload(self.session.transcript(), &identity.payload_bytes())
            .map_err(|error| format!("adaptive TLE partial signing failed: {error}"))?;
        session
            .encode_partial_release(identity, finalized_height, &partial)
            .map_err(|error| format!("adaptive TLE partial validation failed: {error}"))
    }
}

impl TlePartialReleaseSignerV1 for InMemoryTlePartialReleaseSignerV1 {
    fn attest_partial_release_capability(
        &self,
        session: &ValidatedTleKeySessionV1,
        expected_participant_index: u16,
    ) -> Result<TlePartialReleaseCapabilityAttestationV1, TlePartialReleaseCapabilityErrorV1> {
        if session.public_state() != self.session.public_state()
            || expected_participant_index != self.share.index()
        {
            return Err(TlePartialReleaseCapabilityErrorV1::NotOwned);
        }
        TlePartialReleaseCapabilityAttestationV1::for_validated_session(
            session,
            expected_participant_index,
        )
    }

    fn sign_partial_release(
        &self,
        context: &AuthorizedTleReleaseContextV1,
    ) -> Result<TlePartialReleaseShareV1, String> {
        self.sign_validated_release(
            context.session(),
            context.identity(),
            context.finalized_height(),
        )
    }
}

impl TleProjectedPartialReleaseSignerV1 for InMemoryTlePartialReleaseSignerV1 {
    fn sign_projected_partial_release(
        &self,
        projection: &ValidatedTleReleaseProjectionV1,
    ) -> Result<TlePartialReleaseShareV1, String> {
        self.sign_validated_release(
            projection.session(),
            projection.identity(),
            projection.finalized_height(),
        )
    }
}

/// Public adaptive partial release and representation proof.
#[derive(
    Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct TlePartialReleaseShareV1 {
    /// Long-lived TLE key session.
    pub key_session_id: TleKeySessionId,
    /// SHA-256 of the exact typed future release message.
    pub identity_digest: [u8; 32],
    /// Canonical one-based threshold participant index.
    pub participant_index: u16,
    /// Canonical adaptive partial signature in G1.
    pub sigma: [u8; 48],
    /// Triple-generator representation-proof commitment in G2.
    pub proof_x: [u8; 96],
    /// Message-representation proof commitment in G1.
    pub proof_y: [u8; 48],
    /// Standard-generator proof response.
    pub z_s: [u8; 32],
    /// `h`/independent-message proof response.
    pub z_r: [u8; 32],
    /// `v` proof response.
    pub z_u: [u8; 32],
}

/// Constructor-authenticated partial release wrapper.
///
/// Every field is public proof material, but the wrapper is intentionally not
/// serializable: wire input must pass [`ValidatedTleKeySessionV1::verify_partial_release`]
/// after each restart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedTlePartialReleaseShareV1 {
    record: TlePartialReleaseShareV1,
    transcript_hash: [u8; 32],
    partial: DasRenPartialSignature<TleReleasePurpose>,
}

impl VerifiedTlePartialReleaseShareV1 {
    /// Borrow the canonical public wire record.
    #[must_use]
    pub const fn record(&self) -> &TlePartialReleaseShareV1 {
        &self.record
    }
}

/// Unique public final threshold release signature for one future identity.
///
/// No reconstruction subset or signer bitmap is serialized.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
)]
pub struct TleFinalReleaseSignatureV1 {
    /// Long-lived TLE key session.
    pub key_session_id: TleKeySessionId,
    /// SHA-256 of the exact typed future release message.
    pub identity_digest: [u8; 32],
    /// Canonical standard BLS group signature in G1.
    pub signature: [u8; 48],
}

/// Errors returned by the public TLE release adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TleReleaseAdapterError {
    /// A decoded public state advertised another adapter version.
    #[error("unsupported TLE key-session adapter version")]
    UnsupportedVersion,
    /// The long-lived key-session identifier was the all-zero placeholder.
    #[error("TLE key-session identifier must be non-zero")]
    ZeroKeySessionId,
    /// Persisted independent generators did not match deterministic derivation.
    #[error("TLE key-session adaptive generators do not match the typed session")]
    GeneratorMismatch,
    /// Cached public fields did not reconstruct to the committed transcript.
    #[error("TLE key-session public transcript mismatch")]
    TranscriptMismatch,
    /// A partial or final release was bound to another key session or identity.
    #[error("TLE release is bound to another key session or future identity")]
    ReleaseBindingMismatch,
    /// The identity's target finalized height has not yet been reached.
    #[error("TLE release target finalized height has not been reached")]
    ReleaseHeightNotReached,
    /// Adaptive threshold-BLS validation failed.
    #[error(transparent)]
    Threshold(#[from] ThresholdBlsError),
    /// Timelock identity validation failed.
    #[error(transparent)]
    Tle(#[from] TleError),
}

fn is_zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

#[cfg(test)]
pub(crate) mod tests {
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, TLE_KEY_SESSION_SINGLETON_KEY, World},
    };
    use iroha_crypto::threshold_bls::{
        AdaptiveThresholdBlsSecretShare, DasRenDealerSecret, TleReleasePurpose,
    };
    use norito::codec::{DecodeAll as _, Encode as _};
    use rand::{SeedableRng as _, rngs::StdRng};
    use std::sync::Arc;

    use super::*;

    fn binding(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    struct Fixture {
        session: ThresholdBlsSession<TleReleasePurpose>,
        validated: ValidatedTleKeySessionV1,
        dealer_secrets: Vec<DasRenDealerSecret<TleReleasePurpose>>,
        dealers: Vec<ValidatedDealerCommitment<TleReleasePurpose>>,
    }

    fn fixture_for_binding(network_id: [u8; 32], key_byte: u8, roster_hash: [u8; 32]) -> Fixture {
        let session = ThresholdBlsSession::<TleReleasePurpose>::new(
            network_id,
            binding(key_byte),
            roster_hash,
            4,
            2,
        )
        .expect("session");
        let parameters = AdaptiveThresholdBlsParameters::derive(&session).expect("parameters");
        let mut rng = StdRng::from_seed([key_byte.wrapping_add(5); 32]);
        let mut dealer_secrets = Vec::new();
        let mut dealers = Vec::new();
        for index in 1_u16..=3 {
            let (secret, dealer) =
                DasRenDealerSecret::generate_with_rng(&parameters, index, &mut rng)
                    .expect("dealer");
            dealer_secrets.push(secret);
            dealers.push(dealer);
        }
        let validated = ValidatedTleKeySessionV1::from_qualified_dealers(
            session,
            &dealers,
            &[1, 2, 3],
            binding(4),
        )
        .expect("validated key session");
        Fixture {
            session,
            validated,
            dealer_secrets,
            dealers,
        }
    }

    fn fixture_for_key(key_byte: u8) -> Fixture {
        fixture_for_binding(binding(1), key_byte, binding(3))
    }

    /// Build a deterministic public TLE session bound to an exact consensus context.
    pub(crate) fn public_key_session_fixture_for_context_v1(
        network_id: [u8; 32],
        key_byte: u8,
        roster_hash: [u8; 32],
    ) -> TleKeySessionPublicStateV1 {
        fixture_for_binding(network_id, key_byte, roster_hash)
            .validated
            .public_state()
            .clone()
    }

    fn fixture() -> Fixture {
        fixture_for_key(2)
    }

    fn identity(session: ThresholdBlsSession<TleReleasePurpose>) -> TleReleaseIdentityV1 {
        TleReleaseIdentityV1::new(
            session,
            binding(10),
            binding(11),
            binding(12),
            binding(13),
            binding(14),
            100,
            binding(15),
        )
        .expect("identity")
    }

    fn authorized_context(
        session: ValidatedTleKeySessionV1,
        identity: TleReleaseIdentityV1,
        finalized_height: u64,
    ) -> AuthorizedTleReleaseContextV1 {
        AuthorizedTleReleaseContextV1 {
            ballot_attempt_id: BallotAttemptId::new(binding(12)),
            opening_deadline_height: 110,
            finalized_height,
            public_release_identity: TimedOvnReleaseIdentityPublicV1 {
                tle_key_session_id: session.public_state().key_session_id,
                governance_attempt_id: binding(10),
                body_instance_id: binding(11),
                ballot_attempt_id: binding(12),
                survivor_corpus_root: binding(13),
                no_recovery_root: binding(14),
                target_finalized_height: 100,
                parameter_hash: binding(15),
            },
            identity,
            session,
        }
    }

    fn runtime_signer(
        fixture: &Fixture,
        participant_index: u16,
    ) -> InMemoryTlePartialReleaseSignerV1 {
        let parameters = *fixture.validated.transcript().parameters();
        let private_shares = fixture
            .dealer_secrets
            .iter()
            .zip(&fixture.dealers)
            .map(|(secret, dealer)| {
                secret
                    .private_share(&parameters, dealer, participant_index)
                    .expect("private contribution")
            })
            .collect::<Vec<_>>();
        let signing_share = AdaptiveThresholdBlsSecretShare::from_dealer_shares(
            fixture.validated.transcript(),
            &private_shares,
        )
        .expect("signing share");
        InMemoryTlePartialReleaseSignerV1::from_validated_share(
            fixture.validated.clone(),
            signing_share,
        )
        .expect("runtime signer")
    }

    #[test]
    fn public_state_roundtrips_and_revalidates_every_proof() {
        let fixture = fixture();
        let encoded = fixture.validated.public_state().encode();
        let decoded = TleKeySessionPublicStateV1::decode_all(&mut encoded.as_slice())
            .expect("decode public state");
        let restored = decoded.validate().expect("revalidate public state");
        assert_eq!(restored.public_state(), fixture.validated.public_state());

        let mut tampered = fixture.validated.public_state().clone();
        tampered.qualified_dealer_commitments[0].constant_pok_response[31] ^= 1;
        assert!(tampered.validate().is_err());
    }

    #[test]
    fn exact_identity_partials_combine_without_a_subset_bitmap() {
        let fixture = fixture();
        let identity = identity(fixture.session);
        let parameters = *fixture.validated.transcript().parameters();
        let mut rng = StdRng::from_seed([8; 32]);
        let mut records = Vec::new();
        for recipient in 1_u16..=2 {
            let private_shares = fixture
                .dealer_secrets
                .iter()
                .zip(&fixture.dealers)
                .map(|(secret, dealer)| {
                    secret
                        .private_share(&parameters, dealer, recipient)
                        .expect("private contribution")
                })
                .collect::<Vec<_>>();
            let signing_share = AdaptiveThresholdBlsSecretShare::from_dealer_shares(
                fixture.validated.transcript(),
                &private_shares,
            )
            .expect("signing share");
            let partial = signing_share
                .sign_payload_with_rng(
                    fixture.validated.transcript(),
                    &identity.payload_bytes(),
                    &mut rng,
                )
                .expect("partial");
            records.push(
                fixture
                    .validated
                    .encode_partial_release(&identity, 100, &partial)
                    .expect("public partial"),
            );
        }

        let final_release = fixture
            .validated
            .combine_partial_releases(&identity, 100, &records)
            .expect("final release");
        assert_eq!(
            fixture
                .validated
                .verify_final_release(&identity, 100, &final_release),
            Ok(())
        );
        let _release_key = fixture
            .validated
            .release_key_for_opening(&identity, 100, &final_release)
            .expect("zeroizing release key");
        assert_eq!(
            fixture
                .validated
                .verify_final_release(&identity, 99, &final_release),
            Err(TleReleaseAdapterError::ReleaseHeightNotReached)
        );

        let wrong_identity = TleReleaseIdentityV1::new(
            fixture.session,
            binding(10),
            binding(11),
            binding(99),
            binding(13),
            binding(14),
            100,
            binding(15),
        )
        .expect("wrong identity");
        assert_eq!(
            fixture
                .validated
                .verify_final_release(&wrong_identity, 100, &final_release),
            Err(TleReleaseAdapterError::ReleaseBindingMismatch)
        );
    }

    #[test]
    fn runtime_signer_accepts_only_an_authorized_context_and_rechecks_height() {
        let fixture = fixture();
        let identity = identity(fixture.session);
        let signer = runtime_signer(&fixture, 1);
        assert_eq!(signer.participant_index(), 1);

        let attestation = signer
            .attest_partial_release_capability(&fixture.validated, 1)
            .expect("exact imported share must attest its public session and seat");
        assert!(attestation.matches(&fixture.validated, 1));
        assert_eq!(
            attestation.key_session_id(),
            fixture.validated.public_state().key_session_id
        );
        assert_eq!(
            attestation.transcript_hash(),
            fixture.validated.public_state().transcript_hash
        );
        assert_eq!(attestation.participant_index(), 1);
        assert_eq!(
            signer.attest_partial_release_capability(&fixture.validated, 2),
            Err(TlePartialReleaseCapabilityErrorV1::NotOwned)
        );
        let other = fixture_for_key(0x91);
        assert_eq!(
            signer.attest_partial_release_capability(&other.validated, 1),
            Err(TlePartialReleaseCapabilityErrorV1::NotOwned)
        );

        let early_context = authorized_context(fixture.validated.clone(), identity, 99);
        let early = signer
            .sign_partial_release(&early_context)
            .expect_err("runtime signer must reject a pre-target release");
        assert!(early.contains("target finalized height"));

        let context = authorized_context(fixture.validated.clone(), identity, 100);
        let partial = signer
            .sign_partial_release(&context)
            .expect("target-height partial release");
        assert_eq!(partial.participant_index, 1);
        fixture
            .validated
            .verify_partial_release(&identity, 100, &partial)
            .expect("runtime signer output re-verifies independently");
    }

    #[test]
    fn authenticated_broker_projection_roundtrips_and_signs_after_revalidation() {
        let fixture = fixture();
        let release_identity = identity(fixture.session);
        let context = authorized_context(fixture.validated.clone(), release_identity, 100);
        let projection = context
            .broker_projection_v1()
            .expect("opaque Core context projects to bounded public wire data");
        assert_eq!(
            projection.identity_payload.len(),
            TLE_AUTHORIZED_RELEASE_IDENTITY_PAYLOAD_BYTES_V1
        );
        let encoded = projection.encode();
        let decoded = AuthorizedTleReleaseProjectionV1::decode_all(&mut encoded.as_slice())
            .expect("decode broker projection");
        let validated = decoded
            .validate()
            .expect("revalidate exact public transcript and release statement");

        let signer = runtime_signer(&fixture, 1);
        let partial = signer
            .sign_projected_partial_release(&validated)
            .expect("authenticated broker backend signs validated projection");
        assert_eq!(partial.participant_index, 1);
        context
            .session()
            .verify_partial_release(context.identity(), context.finalized_height(), &partial)
            .expect("daemon independently verifies broker output");

        let other = fixture_for_key(22);
        let other_context =
            authorized_context(other.validated.clone(), identity(other.session), 100);
        let other_projection = other_context
            .broker_projection_v1()
            .expect("project another valid key session")
            .validate()
            .expect("validate another key session projection");
        let error = signer
            .sign_projected_partial_release(&other_projection)
            .expect_err("one-session signer must reject a valid cross-session request");
        assert!(error.contains("does not match the sealed share"));
    }

    #[test]
    fn authenticated_broker_projection_rejects_tampered_public_bindings() {
        let fixture = fixture();
        let identity = identity(fixture.session);
        let context = authorized_context(fixture.validated, identity, 100);
        let projection = context
            .broker_projection_v1()
            .expect("valid base projection");

        let mut wrong_ballot = projection.clone();
        wrong_ballot.ballot_attempt_id = BallotAttemptId::new(binding(99));
        assert_eq!(
            wrong_ballot.validate().expect_err("ballot substitution"),
            TleReleaseProjectionErrorV1::BindingMismatch
        );

        let mut wrong_payload = projection.clone();
        wrong_payload.identity_payload[0] ^= 1;
        assert_eq!(
            wrong_payload.validate().expect_err("payload substitution"),
            TleReleaseProjectionErrorV1::IdentityPayloadMismatch
        );

        let mut wrong_digest = projection.clone();
        wrong_digest.identity_digest[0] ^= 1;
        assert_eq!(
            wrong_digest.validate().expect_err("digest substitution"),
            TleReleaseProjectionErrorV1::IdentityDigestMismatch
        );

        let mut expired = projection;
        expired.finalized_height = expired.opening_deadline_height.saturating_add(1);
        assert_eq!(
            expired.validate().expect_err("expired projection"),
            TleReleaseProjectionErrorV1::InvalidHeightWindow
        );
    }

    #[test]
    fn coordinator_fails_closed_without_a_signer_and_reverifies_positive_output() {
        let fixture = fixture();
        let identity = identity(fixture.session);
        let context = authorized_context(fixture.validated.clone(), identity, 100);

        let absent = TleReleaseCoordinatorV1::without_signer();
        assert!(!absent.signer_is_available());
        assert_eq!(
            absent.request_authorized_partial_release(&context),
            Err(TleReleaseCoordinatorErrorV1::SignerUnavailable)
        );

        let coordinator =
            TleReleaseCoordinatorV1::from_signer(Arc::new(runtime_signer(&fixture, 1)));
        assert!(coordinator.signer_is_available());
        let partial = coordinator
            .request_authorized_partial_release(&context)
            .expect("independently verified runtime partial");
        assert_eq!(partial.participant_index, 1);
        context
            .session()
            .verify_partial_release(context.identity(), context.finalized_height(), &partial)
            .expect("coordinator output must reverify outside the signer");
    }

    #[test]
    fn runtime_custody_selects_multiple_sessions_and_retires_unreferenced_share() {
        let first = fixture_for_key(2);
        let second = fixture_for_key(22);
        let first_context =
            authorized_context(first.validated.clone(), identity(first.session), 100);
        let second_context =
            authorized_context(second.validated.clone(), identity(second.session), 100);
        let custody = Arc::new(RuntimeTleReleaseShareCustodyV1::new());
        custody
            .insert_validated_share(runtime_signer(&first, 1))
            .expect("insert first live session");
        custody
            .insert_validated_share(runtime_signer(&second, 2))
            .expect("insert rotating session");
        assert_eq!(
            custody.insert_validated_share(runtime_signer(&second, 2)),
            Err(TleReleaseShareCustodyErrorV1::SessionAlreadyPresent)
        );
        assert!(
            custody
                .attest_partial_release_capability(&first.validated, 1)
                .expect("custody attests the exact first session seat")
                .matches(&first.validated, 1)
        );
        assert!(
            custody
                .attest_partial_release_capability(&second.validated, 2)
                .expect("custody attests the exact rotating session seat")
                .matches(&second.validated, 2)
        );
        assert_eq!(
            custody.attest_partial_release_capability(&second.validated, 1),
            Err(TlePartialReleaseCapabilityErrorV1::NotOwned)
        );

        let signer: Arc<dyn TlePartialReleaseSignerV1> = custody.clone();
        let coordinator = TleReleaseCoordinatorV1::from_signer(signer);
        assert_eq!(
            coordinator
                .request_authorized_partial_release(&first_context)
                .expect("first session partial")
                .participant_index,
            1
        );
        assert_eq!(
            coordinator
                .request_authorized_partial_release(&second_context)
                .expect("second session partial")
                .participant_index,
            2
        );
        let projected_second = second_context
            .broker_projection_v1()
            .expect("project second session")
            .validate()
            .expect("validate second-session broker projection");
        assert_eq!(
            custody
                .sign_projected_partial_release(&projected_second)
                .expect("custody selects the projected key session")
                .participant_index,
            2
        );

        let first_key_session_id = first.validated.public_state().key_session_id;
        let second_key_session_id = second.validated.public_state().key_session_id;
        let mut world = World::new();
        world
            .tle_key_sessions
            .insert(first_key_session_id, first.validated.public_state().clone());
        world.tle_key_sessions.insert(
            second_key_session_id,
            second.validated.public_state().clone(),
        );
        world
            .tle_active_key_session
            .insert(TLE_KEY_SESSION_SINGLETON_KEY, second_key_session_id);
        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        custody
            .retire_session(&state.query_view(), first_key_session_id)
            .expect("unreferenced session retires and zeroizes");
        assert_eq!(
            coordinator.request_authorized_partial_release(&first_context),
            Err(TleReleaseCoordinatorErrorV1::SignerFailed)
        );
        coordinator
            .request_authorized_partial_release(&second_context)
            .expect("unretired rotating session remains available");
        assert_eq!(
            custody.retire_session(&state.query_view(), second_key_session_id),
            Err(TleReleaseShareCustodyErrorV1::SessionStillRequired)
        );
        assert_eq!(
            custody.retire_session(&state.query_view(), first_key_session_id),
            Err(TleReleaseShareCustodyErrorV1::SessionNotPresent)
        );
    }

    #[test]
    fn runtime_custody_rejects_retirement_through_max_committed_retry_deadline() {
        use crate::state::WorldReadOnly as _;

        let retiring = fixture_for_key(32);
        let successor = fixture_for_key(42);
        let retiring_id = retiring.validated.public_state().key_session_id;
        let successor_id = successor.validated.public_state().key_session_id;
        let custody = RuntimeTleReleaseShareCustodyV1::new();
        custody
            .insert_validated_share(runtime_signer(&retiring, 1))
            .expect("insert retiring runtime share");

        let retaining_attempt =
            crate::governance::parliament::tests::tle_key_session_retention_attempt_fixture_v1(
                retiring_id,
            );
        let attempt_id = retaining_attempt.attempt().id;
        let mut world = World::new();
        world
            .tle_key_sessions
            .insert(retiring_id, retiring.validated.public_state().clone());
        world
            .tle_key_sessions
            .insert(successor_id, successor.validated.public_state().clone());
        world
            .tle_active_key_session
            .insert(TLE_KEY_SESSION_SINGLETON_KEY, successor_id);
        world
            .parliament_attempts
            .insert(attempt_id, retaining_attempt);
        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let view = state.query_view();
        assert_eq!(
            view.world()
                .tle_key_session_retention_deadline_v1(retiring_id)
                .expect("validate every committed Parliament attempt"),
            Some(62)
        );
        assert_eq!(
            custody.retire_session(&view, retiring_id),
            Err(TleReleaseShareCustodyErrorV1::SessionStillRequired)
        );
    }

    #[test]
    fn runtime_custody_rejects_invalid_component_import_without_inventory_output() {
        let fixture = fixture();
        let custody = RuntimeTleReleaseShareCustodyV1::new();
        assert_eq!(
            custody.import_components(
                fixture.validated.public_state().clone(),
                1,
                Zeroizing::new([[0; 32]; 3]),
            ),
            Err(TleReleaseShareCustodyErrorV1::InvalidShare)
        );

        let state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        assert_eq!(
            custody.import_committed_components(
                &state.query_view(),
                fixture.validated.public_state().key_session_id,
                1,
                Zeroizing::new([[0; 32]; 3]),
            ),
            Err(TleReleaseShareCustodyErrorV1::SessionNotCommitted)
        );
    }

    #[test]
    fn coordinator_canonicalizes_partials_into_the_existing_finalize_transition() {
        use iroha_data_model::isi::governance::ParliamentLifecycleTransitionV1;

        let fixture = fixture();
        let identity = identity(fixture.session);
        let context = authorized_context(fixture.validated.clone(), identity, 100);
        let first = TleReleaseCoordinatorV1::from_signer(Arc::new(runtime_signer(&fixture, 1)))
            .request_authorized_partial_release(&context)
            .expect("first partial");
        let second = TleReleaseCoordinatorV1::from_signer(Arc::new(runtime_signer(&fixture, 2)))
            .request_authorized_partial_release(&context)
            .expect("second partial");
        let coordinator = TleReleaseCoordinatorV1::without_signer();

        let transition = coordinator
            .combine_authorized_partial_releases(&context, &[second.clone(), first.clone()])
            .expect("canonical final release transition");
        let ParliamentLifecycleTransitionV1::FinalizeOpenedBallot(payload) = transition else {
            panic!("coordinator must emit only FinalizeOpenedBallot")
        };
        assert_eq!(payload.ballot_attempt_id, context.ballot_attempt_id());
        let final_release = TleFinalReleaseSignatureV1 {
            key_session_id: payload.final_release.key_session_id,
            identity_digest: payload.final_release.identity_digest,
            signature: payload.final_release.signature,
        };
        context
            .session()
            .verify_final_release(
                context.identity(),
                context.finalized_height(),
                &final_release,
            )
            .expect("combined transition must carry the unique final signature");

        assert_eq!(
            coordinator.combine_authorized_partial_releases(&context, &[first.clone(), first]),
            Err(TleReleaseCoordinatorErrorV1::InvalidPartialSet)
        );
    }

    #[test]
    fn coordinator_discards_signer_diagnostics_and_invalid_public_output() {
        struct FailingSigner;

        impl TlePartialReleaseSignerV1 for FailingSigner {
            fn attest_partial_release_capability(
                &self,
                _session: &ValidatedTleKeySessionV1,
                _expected_participant_index: u16,
            ) -> Result<TlePartialReleaseCapabilityAttestationV1, TlePartialReleaseCapabilityErrorV1>
            {
                Err(TlePartialReleaseCapabilityErrorV1::NotOwned)
            }

            fn sign_partial_release(
                &self,
                _context: &AuthorizedTleReleaseContextV1,
            ) -> Result<TlePartialReleaseShareV1, String> {
                Err("secret-provider-handle-and-share-metadata".to_owned())
            }
        }

        struct InvalidSigner(TlePartialReleaseShareV1);

        impl TlePartialReleaseSignerV1 for InvalidSigner {
            fn attest_partial_release_capability(
                &self,
                _session: &ValidatedTleKeySessionV1,
                _expected_participant_index: u16,
            ) -> Result<TlePartialReleaseCapabilityAttestationV1, TlePartialReleaseCapabilityErrorV1>
            {
                Err(TlePartialReleaseCapabilityErrorV1::NotOwned)
            }

            fn sign_partial_release(
                &self,
                _context: &AuthorizedTleReleaseContextV1,
            ) -> Result<TlePartialReleaseShareV1, String> {
                Ok(self.0.clone())
            }
        }

        let fixture = fixture();
        let identity = identity(fixture.session);
        let context = authorized_context(fixture.validated.clone(), identity, 100);
        let failing = TleReleaseCoordinatorV1::from_signer(Arc::new(FailingSigner));
        let error = failing
            .request_authorized_partial_release(&context)
            .expect_err("provider failure must stay closed");
        assert_eq!(error, TleReleaseCoordinatorErrorV1::SignerFailed);
        assert!(!error.to_string().contains("secret-provider"));

        let mut partial = runtime_signer(&fixture, 1)
            .sign_partial_release(&context)
            .expect("valid base partial");
        partial.sigma[0] ^= 1;
        let invalid = TleReleaseCoordinatorV1::from_signer(Arc::new(InvalidSigner(partial)));
        assert_eq!(
            invalid.request_authorized_partial_release(&context),
            Err(TleReleaseCoordinatorErrorV1::InvalidSignerOutput)
        );
    }

    #[test]
    fn release_authorization_rejects_an_uncommitted_ballot() {
        let state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        assert_eq!(
            authorize_parliament_tle_release_v1(
                &state.query_view(),
                BallotAttemptId::new(binding(12)),
            )
            .expect_err("an arbitrary ballot must not reach the signer"),
            TleReleaseAuthorizationErrorV1::MissingTimedOvnEvidence
        );
    }
}
