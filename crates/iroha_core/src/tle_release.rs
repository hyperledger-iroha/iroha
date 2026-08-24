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

use iroha_crypto::{
    threshold_bls::{
        AdaptiveThresholdBlsParameters, AdaptiveThresholdBlsPublicTranscript,
        DasRenDealerCommitment, DasRenPartialSignature, ThresholdBlsError, ThresholdBlsSession,
        ThresholdBlsSignature, TleReleasePurpose, ValidatedDealerCommitment,
    },
    tle::{TleError, TleIdentitySecretKeyV1, TleMasterPublicKey, TleReleaseIdentityV1},
};
pub use iroha_data_model::governance::types::TleKeySessionId;
use norito::{
    NoritoDeserialize, NoritoSerialize,
    derive::{JsonDeserialize, JsonSerialize},
};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

/// Fixed version of the public TLE key-session adapter.
pub const TLE_KEY_SESSION_ADAPTER_VERSION_V1: u16 = 1;

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
            parameters,
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
            parameters,
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
mod tests {
    use iroha_crypto::threshold_bls::{
        AdaptiveThresholdBlsSecretShare, DasRenDealerSecret, TleReleasePurpose,
    };
    use norito::codec::{DecodeAll as _, Encode as _};
    use rand::{SeedableRng as _, rngs::StdRng};

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

    fn fixture() -> Fixture {
        let session =
            ThresholdBlsSession::<TleReleasePurpose>::new(binding(1), binding(2), binding(3), 4, 2)
                .expect("session");
        let parameters = AdaptiveThresholdBlsParameters::derive(&session).expect("parameters");
        let mut rng = StdRng::from_seed([7; 32]);
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
}
