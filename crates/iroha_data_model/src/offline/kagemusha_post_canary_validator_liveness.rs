//! Challenge-bound proof that every qualified Kagemusha validator responded after a canary.
//!
//! A node signature authenticates the node identity, durable tip, genesis, and
//! Sumeragi status. The issuer-signed observation times and HTTPS origins are
//! trusted-host observations: the node signature does not authenticate them.

use crate::{
    block::{BlockHeader, consensus_v2::ConsensusMode},
    bridge::{
        BridgeFinalityAttestationV1, BridgeFinalityProof, BridgeFinalityVerifier,
        verify_bridge_finality_proof,
    },
    peer::PeerId,
    transaction::SignedTransaction,
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, PublicKey, SignatureOf};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

use super::{
    kagemusha_canary_evidence::KagemushaV4VerifiedTairaCanaryEvidenceV1,
    kagemusha_promotion_receipt::{
        KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1,
        KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_BYTES, KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT,
        KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION, KagemushaExactBytesDigestV1,
        KagemushaV4ActivationReceiptExpectationsV1, KagemushaV4PromotionBindingV1,
    },
    kagemusha_runtime_effective_config_projection::KagemushaV4RuntimeEffectiveConfigProjectionV1,
};

/// Maximum canonical bytes accepted for one signed liveness challenge.
pub const KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_CHALLENGE_MAX_BYTES: usize = 1024 * 1024;
/// Maximum canonical bytes accepted for one complete liveness artifact.
pub const KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_EVIDENCE_MAX_BYTES: usize = 64 * 1024 * 1024;
/// Maximum canonical bytes accepted for one Torii attestation response.
pub const KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_ATTESTATION_MAX_BYTES: usize =
    8 * 1024 * 1024;
/// Maximum UTF-8 bytes accepted for one canonical Torii origin.
pub const KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_ORIGIN_MAX_BYTES: usize = 512;
// Canonical evidence nests four signed status/finality proof graphs. Keep that
// reviewed, bounded decoder off ordinary 2 MiB service and test stacks.
const KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_DECODE_STACK_BYTES: usize = 8 * 1024 * 1024;
/// Maximum issuer-authorized collection interval.
pub const KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_MAX_INTERVAL_MS: u64 = 5 * 60 * 1_000;
/// Maximum elapsed time for one validator request.
pub const KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_MAX_RESPONSE_MS: u64 = 60 * 1_000;
/// Schema id of the exact canary anchor nested in a liveness challenge.
pub const KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_CANARY_ANCHOR_SCHEMA: &str =
    "iroha.kagemusha.v4.post_canary_validator_liveness_canary_anchor.v1";
/// Schema id of an issuer-signed challenge body.
pub const KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_CHALLENGE_BODY_SCHEMA: &str =
    "iroha.kagemusha.v4.post_canary_validator_liveness_challenge_body.v1";
/// Schema id of an issuer-signed challenge.
pub const KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_CHALLENGE_SCHEMA: &str =
    "iroha.kagemusha.v4.post_canary_validator_liveness_challenge.v1";
/// Schema id of one validator observation.
pub const KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_OBSERVATION_SCHEMA: &str =
    "iroha.kagemusha.v4.post_canary_validator_liveness_observation.v1";
/// Schema id of the complete liveness evidence body.
pub const KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_EVIDENCE_BODY_SCHEMA: &str =
    "iroha.kagemusha.v4.post_canary_validator_liveness_evidence_body.v1";
/// Schema id of the signed liveness artifact.
pub const KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_EVIDENCE_SCHEMA: &str =
    "iroha.kagemusha.v4.post_canary_validator_liveness_evidence.v1";
/// Domain separator for the issuer's pre-collection challenge signature.
pub const KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_CHALLENGE_SIGNATURE_DOMAIN: &[u8] =
    b"iroha:kagemusha:v4:post-canary-validator-liveness-challenge:v1\0";
/// Domain separator used to derive the exact Torii challenge header.
pub const KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_ENDPOINT_CHALLENGE_DOMAIN: &[u8] =
    b"iroha:kagemusha:v4:post-canary-validator-liveness-endpoint-challenge:v1\0";
/// Domain separator for the issuer's complete evidence signature.
pub const KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_EVIDENCE_SIGNATURE_DOMAIN: &[u8] =
    b"iroha:kagemusha:v4:post-canary-validator-liveness-evidence:v1\0";

/// Exact authenticated canary values against which liveness is measured.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4PostCanaryValidatorLivenessCanaryAnchorV1 {
    /// Exact anchor schema.
    pub schema: String,
    /// Anchor version.
    pub version: u16,
    /// Exact issuer-signed activation-finality receipt identity.
    pub activation_finality_receipt: KagemushaExactBytesDigestV1,
    /// Exact controller-signed canary authorization identity.
    pub canary_authorization: KagemushaExactBytesDigestV1,
    /// Payload-only canary transaction intent.
    pub canary_transaction_intent: HashOf<SignedTransaction>,
    /// Exact authorization-bearing canary transaction wire identity.
    pub canary_transaction_wire: KagemushaExactBytesDigestV1,
    /// Authenticated finalized canary carrier height.
    pub canary_finalized_height: u64,
    /// Authenticated finalized canary carrier hash.
    pub canary_finalized_block_hash: HashOf<BlockHeader>,
    /// Authenticated canary block-header creation time.
    pub canary_finalized_block_time_unix_ms: u64,
}

impl KagemushaV4PostCanaryValidatorLivenessCanaryAnchorV1 {
    fn validate_structure(
        &self,
    ) -> Result<(), KagemushaV4PostCanaryValidatorLivenessValidationError> {
        if self.schema != KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_CANARY_ANCHOR_SCHEMA
            || self.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
            || self.canary_finalized_height == 0
            || self.canary_finalized_block_time_unix_ms == 0
            || hash_is_zero(self.canary_transaction_intent.as_ref())
            || hash_is_zero(self.canary_finalized_block_hash.as_ref().as_ref())
        {
            return Err(
                KagemushaV4PostCanaryValidatorLivenessValidationError::InvalidField(
                    "post_canary_liveness.canary_anchor",
                ),
            );
        }
        validate_identity_digest(
            self.activation_finality_receipt,
            KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_BYTES,
            "post_canary_liveness.activation_finality_receipt",
        )?;
        validate_identity_digest(
            self.canary_authorization,
            KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_EVIDENCE_MAX_BYTES,
            "post_canary_liveness.canary_authorization",
        )?;
        validate_identity_digest(
            self.canary_transaction_wire,
            KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_EVIDENCE_MAX_BYTES,
            "post_canary_liveness.canary_transaction_wire",
        )
    }
}

/// One validator identity and the exact Torii origin challenged for it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4PostCanaryValidatorLivenessTargetV1 {
    /// Qualified BLS validator identity expected to sign the response.
    pub validator_id: PeerId,
    /// Exact lower-case HTTPS DNS origin queried for this validator.
    pub canonical_torii_origin: String,
}

impl KagemushaV4PostCanaryValidatorLivenessTargetV1 {
    fn validate_structure(
        &self,
    ) -> Result<(), KagemushaV4PostCanaryValidatorLivenessValidationError> {
        if self.validator_id.public_key().try_algorithm() != Ok(Algorithm::BlsNormal) {
            return Err(
                KagemushaV4PostCanaryValidatorLivenessValidationError::InvalidField(
                    "post_canary_liveness.target.validator_id",
                ),
            );
        }
        validate_liveness_torii_origin(&self.canonical_torii_origin)
    }
}

/// Issuer-signed, canary-bound challenge fixed before validator collection.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4PostCanaryValidatorLivenessChallengeBodyV1 {
    /// Exact challenge-body schema.
    pub schema: String,
    /// Challenge-body version.
    pub version: u16,
    /// Complete authenticated promotion and consensus identity.
    pub binding: KagemushaV4PromotionBindingV1,
    /// Exact canary values that every response must have observed.
    pub canary_anchor: KagemushaV4PostCanaryValidatorLivenessCanaryAnchorV1,
    /// Exact qualified validators and their independently queried origins.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_array"))]
    pub targets:
        [KagemushaV4PostCanaryValidatorLivenessTargetV1; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
    /// Independent activation-receipt issuer authorizing this collection.
    pub issuer: PublicKey,
    /// Unpredictable non-zero per-collection nonce.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub nonce: [u8; 32],
    /// Issuer-declared start of the collection interval.
    pub issued_at_unix_ms: u64,
    /// Exclusive end of the collection interval.
    pub expires_at_unix_ms: u64,
}

impl KagemushaV4PostCanaryValidatorLivenessChallengeBodyV1 {
    /// Return the domain-separated typed hash signed before collection.
    #[must_use]
    pub fn signing_hash(&self) -> HashOf<Self> {
        HashOf::from_untyped_unchecked(Hash::new_from_chunks(&[
            KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_CHALLENGE_SIGNATURE_DOMAIN,
            &self.encode(),
        ]))
    }

    fn validate_structure(
        &self,
    ) -> Result<(), KagemushaV4PostCanaryValidatorLivenessValidationError> {
        if self.schema != KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_CHALLENGE_BODY_SCHEMA
            || self.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
            || !supports_issuer_signature_algorithm(&self.issuer)
            || self.issuer == self.binding.promotion_controller
            || self.nonce == [0; 32]
            || self.issued_at_unix_ms <= self.canary_anchor.canary_finalized_block_time_unix_ms
            || self.expires_at_unix_ms <= self.issued_at_unix_ms
            || self
                .expires_at_unix_ms
                .saturating_sub(self.issued_at_unix_ms)
                > KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_MAX_INTERVAL_MS
        {
            return Err(
                KagemushaV4PostCanaryValidatorLivenessValidationError::InvalidField(
                    "post_canary_liveness.challenge_body",
                ),
            );
        }
        self.binding.validate().map_err(|_| {
            KagemushaV4PostCanaryValidatorLivenessValidationError::InvalidField(
                "post_canary_liveness.binding",
            )
        })?;
        self.canary_anchor.validate_structure()?;
        let mut previous = None;
        for (index, target) in self.targets.iter().enumerate() {
            target.validate_structure()?;
            if previous.is_some_and(|id: &PeerId| id >= &target.validator_id)
                || target.validator_id.public_key() == &self.issuer
                || self.targets[..index]
                    .iter()
                    .any(|prior| prior.canonical_torii_origin == target.canonical_torii_origin)
            {
                return Err(
                    KagemushaV4PostCanaryValidatorLivenessValidationError::InvalidField(
                        "post_canary_liveness.targets",
                    ),
                );
            }
            previous = Some(&target.validator_id);
        }
        Ok(())
    }
}

/// Signed collection challenge whose exact bytes derive the Torii header value.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4PostCanaryValidatorLivenessChallengeV1 {
    /// Exact challenge schema.
    pub schema: String,
    /// Challenge version.
    pub version: u16,
    /// Pre-collection statement.
    pub body: KagemushaV4PostCanaryValidatorLivenessChallengeBodyV1,
    /// Issuer signature over `body.signing_hash()`.
    pub signature: SignatureOf<KagemushaV4PostCanaryValidatorLivenessChallengeBodyV1>,
}

impl KagemushaV4PostCanaryValidatorLivenessChallengeV1 {
    /// Validate and sign one challenge before querying any validator.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4PostCanaryValidatorLivenessValidationError`] for a malformed,
    /// signer-mismatched, or oversized challenge.
    pub fn try_sign(
        body: KagemushaV4PostCanaryValidatorLivenessChallengeBodyV1,
        issuer: &KeyPair,
    ) -> Result<Self, KagemushaV4PostCanaryValidatorLivenessValidationError> {
        body.validate_structure()?;
        enforce_artifact_size(
            &body,
            KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_CHALLENGE_MAX_BYTES,
        )?;
        if issuer.public_key() != &body.issuer {
            return Err(KagemushaV4PostCanaryValidatorLivenessValidationError::SignerMismatch);
        }
        let signature = SignatureOf::try_from_hash(issuer.private_key(), body.signing_hash())
            .map_err(|_| KagemushaV4PostCanaryValidatorLivenessValidationError::Signature)?;
        let challenge = Self {
            schema: KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_CHALLENGE_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            body,
            signature,
        };
        challenge.verify_structure_and_signature()?;
        Ok(challenge)
    }

    /// Decode one exact canonical bounded challenge.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4PostCanaryValidatorLivenessValidationError`] for empty,
    /// oversized, non-canonical, malformed, or unsigned bytes.
    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, KagemushaV4PostCanaryValidatorLivenessValidationError> {
        check_input_size(
            bytes,
            KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_CHALLENGE_MAX_BYTES,
        )?;
        let challenge: Self = norito::decode_canonical_with_limits(
            bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .map_err(|_| KagemushaV4PostCanaryValidatorLivenessValidationError::Decode)?;
        if norito::encode_canonical(&challenge)
            .map_err(|_| KagemushaV4PostCanaryValidatorLivenessValidationError::Encode)?
            != bytes
        {
            return Err(KagemushaV4PostCanaryValidatorLivenessValidationError::Decode);
        }
        challenge.verify_structure_and_signature()?;
        Ok(challenge)
    }

    /// Derive the 32-byte challenge sent in `X-Iroha-Finality-Challenge`.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4PostCanaryValidatorLivenessValidationError`] if the
    /// signed challenge is invalid or cannot be canonically encoded.
    pub fn endpoint_challenge(
        &self,
    ) -> Result<[u8; 32], KagemushaV4PostCanaryValidatorLivenessValidationError> {
        self.verify_structure_and_signature()?;
        let bytes = norito::encode_canonical(self)
            .map_err(|_| KagemushaV4PostCanaryValidatorLivenessValidationError::Encode)?;
        let challenge = *Hash::new_from_chunks(&[
            KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_ENDPOINT_CHALLENGE_DOMAIN,
            &bytes,
        ])
        .as_ref();
        if challenge == [0; 32] {
            return Err(KagemushaV4PostCanaryValidatorLivenessValidationError::Challenge);
        }
        Ok(challenge)
    }

    /// Verify the challenge against authenticated activation and canary inputs before dispatch.
    ///
    /// The returned bytes are the exact lowercase-hex payload source for
    /// `X-Iroha-Finality-Challenge` on every validator request.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4PostCanaryValidatorLivenessValidationError`] when the
    /// challenge targets, issuer, promotion, canary capability, or canary proof differ.
    pub fn verify_bound(
        &self,
        expectations: &KagemushaV4ActivationReceiptExpectationsV1,
        verified_canary: &KagemushaV4VerifiedTairaCanaryEvidenceV1,
        expected_canary: &KagemushaV4PostCanaryValidatorLivenessCanaryAnchorV1,
        canary_finality_proof: &BridgeFinalityProof,
    ) -> Result<[u8; 32], KagemushaV4PostCanaryValidatorLivenessValidationError> {
        let trust = LivenessTrust::from_expectations(
            expectations,
            verified_canary,
            expected_canary,
            canary_finality_proof,
        )?;
        verify_challenge_with_trust(self, &trust)
    }

    fn verify_structure_and_signature(
        &self,
    ) -> Result<(), KagemushaV4PostCanaryValidatorLivenessValidationError> {
        enforce_artifact_size(
            self,
            KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_CHALLENGE_MAX_BYTES,
        )?;
        self.body.validate_structure()?;
        if self.schema != KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_CHALLENGE_SCHEMA
            || self.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
        {
            return Err(KagemushaV4PostCanaryValidatorLivenessValidationError::Challenge);
        }
        verify_typed_signature(&self.signature, &self.body.issuer, self.body.signing_hash())
    }
}

/// One exact host observation of a validator's challenge-bound Torii response.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4PostCanaryValidatorLivenessObservationV1 {
    /// Exact observation schema.
    pub schema: String,
    /// Observation version.
    pub version: u16,
    /// Precommitted validator identity and queried HTTPS origin.
    pub target: KagemushaV4PostCanaryValidatorLivenessTargetV1,
    /// Trusted-host time immediately before dispatching the HTTP request.
    pub request_started_at_unix_ms: u64,
    /// Trusted-host time immediately after receiving the complete response.
    pub response_completed_at_unix_ms: u64,
    /// Exact canonical Norito response identity.
    pub attestation_response_norito: KagemushaExactBytesDigestV1,
    /// Node-signed challenge-bound durable-tip statement.
    pub attestation: BridgeFinalityAttestationV1,
}

impl KagemushaV4PostCanaryValidatorLivenessObservationV1 {
    fn validate_structure(
        &self,
    ) -> Result<(), KagemushaV4PostCanaryValidatorLivenessValidationError> {
        if self.schema != KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_OBSERVATION_SCHEMA
            || self.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
            || self.request_started_at_unix_ms == 0
            || self.response_completed_at_unix_ms < self.request_started_at_unix_ms
            || self
                .response_completed_at_unix_ms
                .saturating_sub(self.request_started_at_unix_ms)
                > KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_MAX_RESPONSE_MS
        {
            return Err(KagemushaV4PostCanaryValidatorLivenessValidationError::Observation);
        }
        self.target.validate_structure()?;
        validate_identity_digest(
            self.attestation_response_norito,
            KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_ATTESTATION_MAX_BYTES,
            "post_canary_liveness.attestation_response_norito",
        )?;
        enforce_artifact_size(
            &self.attestation,
            KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_ATTESTATION_MAX_BYTES,
        )?;
        let exact_attestation = norito::encode_canonical(&self.attestation)
            .map_err(|_| KagemushaV4PostCanaryValidatorLivenessValidationError::Encode)?;
        if !self
            .attestation_response_norito
            .matches_bytes(&exact_attestation)
        {
            return Err(KagemushaV4PostCanaryValidatorLivenessValidationError::Observation);
        }
        Ok(())
    }
}

/// Issuer-signed body proving four independent validator responses.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4PostCanaryValidatorLivenessEvidenceBodyV1 {
    /// Exact evidence-body schema.
    pub schema: String,
    /// Evidence-body version.
    pub version: u16,
    /// Issuer-signed challenge fixed before collection.
    pub challenge: KagemushaV4PostCanaryValidatorLivenessChallengeV1,
    /// Exact challenge sent to all four validator endpoints.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub endpoint_challenge: [u8; 32],
    /// Exactly four observations in qualified validator order.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_array"))]
    pub observations: [KagemushaV4PostCanaryValidatorLivenessObservationV1;
        KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
    /// One shared immediate-successor chain from the canary to the highest observed tip.
    pub post_canary_finality_proof_chain: Vec<BridgeFinalityProof>,
}

impl KagemushaV4PostCanaryValidatorLivenessEvidenceBodyV1 {
    /// Return the domain-separated typed hash signed by the independent issuer.
    #[must_use]
    pub fn signing_hash(&self) -> HashOf<Self> {
        HashOf::from_untyped_unchecked(Hash::new_from_chunks(&[
            KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_EVIDENCE_SIGNATURE_DOMAIN,
            &self.encode(),
        ]))
    }

    fn validate_structure(
        &self,
    ) -> Result<(), KagemushaV4PostCanaryValidatorLivenessValidationError> {
        if self.schema != KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_EVIDENCE_BODY_SCHEMA
            || self.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
            || self.endpoint_challenge == [0; 32]
            || self.post_canary_finality_proof_chain.len()
                > KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1
        {
            return Err(
                KagemushaV4PostCanaryValidatorLivenessValidationError::InvalidField(
                    "post_canary_liveness.evidence_body",
                ),
            );
        }
        self.challenge.verify_structure_and_signature()?;
        if self.endpoint_challenge != self.challenge.endpoint_challenge()? {
            return Err(KagemushaV4PostCanaryValidatorLivenessValidationError::Challenge);
        }
        for (observation, target) in self.observations.iter().zip(&self.challenge.body.targets) {
            observation.validate_structure()?;
            if observation.target != *target {
                return Err(KagemushaV4PostCanaryValidatorLivenessValidationError::Observation);
            }
        }
        Ok(())
    }
}

/// Complete issuer-signed evidence that all four validators responded after the canary.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4PostCanaryValidatorLivenessEvidenceV1 {
    /// Exact evidence schema.
    pub schema: String,
    /// Evidence version.
    pub version: u16,
    /// Complete signed statement.
    pub body: KagemushaV4PostCanaryValidatorLivenessEvidenceBodyV1,
    /// Independent issuer signature over `body.signing_hash()`.
    pub signature: SignatureOf<KagemushaV4PostCanaryValidatorLivenessEvidenceBodyV1>,
}

impl KagemushaV4PostCanaryValidatorLivenessEvidenceV1 {
    /// Verify every observation and finality proof, then sign the complete artifact.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4PostCanaryValidatorLivenessValidationError`] for any
    /// malformed, untrusted, stale, signer-mismatched, non-final, or oversized input.
    pub fn try_sign(
        body: KagemushaV4PostCanaryValidatorLivenessEvidenceBodyV1,
        issuer: &KeyPair,
        expectations: &KagemushaV4ActivationReceiptExpectationsV1,
        verified_canary: &KagemushaV4VerifiedTairaCanaryEvidenceV1,
        expected_canary: &KagemushaV4PostCanaryValidatorLivenessCanaryAnchorV1,
        canary_finality_proof: &BridgeFinalityProof,
    ) -> Result<Self, KagemushaV4PostCanaryValidatorLivenessValidationError> {
        let trust = LivenessTrust::from_expectations(
            expectations,
            verified_canary,
            expected_canary,
            canary_finality_proof,
        )?;
        Self::try_sign_with_trust(body, issuer, &trust)
    }

    /// Decode one exact canonical bounded liveness artifact.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4PostCanaryValidatorLivenessValidationError`] for empty,
    /// oversized, non-canonical, or structurally invalid bytes.
    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, KagemushaV4PostCanaryValidatorLivenessValidationError> {
        check_input_size(
            bytes,
            KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_EVIDENCE_MAX_BYTES,
        )?;
        let evidence: Self = std::thread::scope(|scope| {
            let decoder = std::thread::Builder::new()
                .name("kagemusha-liveness-decode".to_owned())
                .stack_size(KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_DECODE_STACK_BYTES)
                .spawn_scoped(scope, || {
                    norito::decode_canonical_with_limits(
                        bytes,
                        norito::canonical_decode_limits(bytes.len()),
                    )
                    .map_err(|_| KagemushaV4PostCanaryValidatorLivenessValidationError::Decode)
                })
                .map_err(|_| KagemushaV4PostCanaryValidatorLivenessValidationError::Decode)?;
            decoder
                .join()
                .map_err(|_| KagemushaV4PostCanaryValidatorLivenessValidationError::Decode)?
        })?;
        evidence.body.validate_structure()?;
        if evidence.schema != KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_EVIDENCE_SCHEMA
            || evidence.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
            || norito::encode_canonical(&evidence)
                .map_err(|_| KagemushaV4PostCanaryValidatorLivenessValidationError::Encode)?
                != bytes
        {
            return Err(KagemushaV4PostCanaryValidatorLivenessValidationError::Decode);
        }
        Ok(evidence)
    }

    /// Verify exact encoding, activation trust, issuer signatures, four node signatures,
    /// genesis identities, and the common post-canary finality chain.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4PostCanaryValidatorLivenessValidationError`] on any
    /// exact-byte, trust, challenge, observation, signature, or finality mismatch.
    pub fn verify_exact(
        &self,
        exact_evidence_bytes: &[u8],
        expectations: &KagemushaV4ActivationReceiptExpectationsV1,
        verified_canary: &KagemushaV4VerifiedTairaCanaryEvidenceV1,
        expected_canary: &KagemushaV4PostCanaryValidatorLivenessCanaryAnchorV1,
        canary_finality_proof: &BridgeFinalityProof,
    ) -> Result<
        KagemushaV4VerifiedPostCanaryValidatorLivenessEvidenceV1,
        KagemushaV4PostCanaryValidatorLivenessValidationError,
    > {
        check_input_size(
            exact_evidence_bytes,
            KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_EVIDENCE_MAX_BYTES,
        )?;
        let trust = LivenessTrust::from_expectations(
            expectations,
            verified_canary,
            expected_canary,
            canary_finality_proof,
        )?;
        self.verify_exact_with_trust(exact_evidence_bytes, &trust)
    }

    fn verify_exact_with_trust(
        &self,
        exact_evidence_bytes: &[u8],
        trust: &LivenessTrust<'_>,
    ) -> Result<
        KagemushaV4VerifiedPostCanaryValidatorLivenessEvidenceV1,
        KagemushaV4PostCanaryValidatorLivenessValidationError,
    > {
        check_input_size(
            exact_evidence_bytes,
            KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_EVIDENCE_MAX_BYTES,
        )?;
        enforce_artifact_size(
            self,
            KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_EVIDENCE_MAX_BYTES,
        )?;
        self.body.validate_structure()?;
        if self.schema != KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_EVIDENCE_SCHEMA
            || self.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
            || norito::encode_canonical(self)
                .map_err(|_| KagemushaV4PostCanaryValidatorLivenessValidationError::Encode)?
                != exact_evidence_bytes
        {
            return Err(KagemushaV4PostCanaryValidatorLivenessValidationError::Decode);
        }
        verify_typed_signature(&self.signature, trust.issuer, self.body.signing_hash())?;
        verify_evidence_body_with_trust(&self.body, trust)
    }

    fn try_sign_with_trust(
        body: KagemushaV4PostCanaryValidatorLivenessEvidenceBodyV1,
        issuer: &KeyPair,
        trust: &LivenessTrust<'_>,
    ) -> Result<Self, KagemushaV4PostCanaryValidatorLivenessValidationError> {
        body.validate_structure()?;
        enforce_artifact_size(
            &body,
            KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_EVIDENCE_MAX_BYTES,
        )?;
        verify_evidence_body_with_trust(&body, trust)?;
        if issuer.public_key() != trust.issuer || issuer.public_key() != &body.challenge.body.issuer
        {
            return Err(KagemushaV4PostCanaryValidatorLivenessValidationError::SignerMismatch);
        }
        let signature = SignatureOf::try_from_hash(issuer.private_key(), body.signing_hash())
            .map_err(|_| KagemushaV4PostCanaryValidatorLivenessValidationError::Signature)?;
        let evidence = Self {
            schema: KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_EVIDENCE_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            body,
            signature,
        };
        enforce_artifact_size(
            &evidence,
            KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_EVIDENCE_MAX_BYTES,
        )?;
        Ok(evidence)
    }
}

/// Capability returned only after complete four-validator liveness verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KagemushaV4VerifiedPostCanaryValidatorLivenessEvidenceV1 {
    promotion_id: [u8; 32],
    canary_transaction_intent: HashOf<SignedTransaction>,
    canary_finalized_height: u64,
    canary_finalized_block_hash: HashOf<BlockHeader>,
    endpoint_challenge: [u8; 32],
    validator_ids: [PeerId; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
    observed_tip_heights: [u64; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
    highest_observed_tip_height: u64,
}

impl KagemushaV4VerifiedPostCanaryValidatorLivenessEvidenceV1 {
    /// Return the authenticated promotion id.
    #[must_use]
    pub const fn promotion_id(&self) -> [u8; 32] {
        self.promotion_id
    }

    /// Return the authenticated canary transaction intent.
    #[must_use]
    pub const fn canary_transaction_intent(&self) -> HashOf<SignedTransaction> {
        self.canary_transaction_intent
    }

    /// Return the authenticated canary carrier height.
    #[must_use]
    pub const fn canary_finalized_height(&self) -> u64 {
        self.canary_finalized_height
    }

    /// Return the authenticated canary carrier hash.
    #[must_use]
    pub const fn canary_finalized_block_hash(&self) -> HashOf<BlockHeader> {
        self.canary_finalized_block_hash
    }

    /// Return the common challenge signed independently by every validator.
    #[must_use]
    pub const fn endpoint_challenge(&self) -> [u8; 32] {
        self.endpoint_challenge
    }

    /// Return all four authenticated validator identities in qualification order.
    #[must_use]
    pub const fn validator_ids(&self) -> &[PeerId; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT] {
        &self.validator_ids
    }

    /// Return each validator's authenticated durable-tip height.
    #[must_use]
    pub const fn observed_tip_heights(&self) -> &[u64; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT] {
        &self.observed_tip_heights
    }

    /// Return the terminal height of the verified shared proof chain.
    #[must_use]
    pub const fn highest_observed_tip_height(&self) -> u64 {
        self.highest_observed_tip_height
    }
}

struct LivenessTrust<'a> {
    binding: &'a KagemushaV4PromotionBindingV1,
    issuer: &'a PublicKey,
    validator_ids: [PeerId; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
    runtime: &'a KagemushaV4RuntimeEffectiveConfigProjectionV1,
    canary_anchor: &'a KagemushaV4PostCanaryValidatorLivenessCanaryAnchorV1,
    canary_finality_proof: &'a BridgeFinalityProof,
}

impl<'a> LivenessTrust<'a> {
    fn from_expectations(
        expectations: &'a KagemushaV4ActivationReceiptExpectationsV1,
        verified_canary: &KagemushaV4VerifiedTairaCanaryEvidenceV1,
        canary_anchor: &'a KagemushaV4PostCanaryValidatorLivenessCanaryAnchorV1,
        canary_finality_proof: &'a BridgeFinalityProof,
    ) -> Result<Self, KagemushaV4PostCanaryValidatorLivenessValidationError> {
        let bodies = expectations.validator_bodies();
        let runtime = &bodies[0].runtime_effective_config;
        runtime.validate().map_err(|_| {
            KagemushaV4PostCanaryValidatorLivenessValidationError::ActivationBinding
        })?;
        let validator_ids = std::array::from_fn(|index| bodies[index].validator_id.clone());
        if bodies.iter().enumerate().any(|(index, body)| {
            body.binding != *expectations.binding()
                || body.runtime_effective_config != *runtime
                || body.validator_id != runtime.validators[index].validator_id
        }) {
            return Err(KagemushaV4PostCanaryValidatorLivenessValidationError::ActivationBinding);
        }
        if verified_canary.promotion_id() != expectations.binding().promotion_id
            || verified_canary.activation_expectations_artifact()
                != expectations.activation_expectations_artifact()
            || verified_canary.activation_transaction_intent()
                != expectations.activation_transaction_intent()
            || canary_anchor.activation_finality_receipt
                != verified_canary.activation_finality_receipt()
            || canary_anchor.canary_authorization != verified_canary.authorization_identity()
            || canary_anchor.canary_transaction_intent
                != verified_canary.canary_transaction_intent()
            || canary_anchor.canary_transaction_wire != verified_canary.canary_transaction_wire()
            || canary_anchor.canary_finalized_height != verified_canary.finalized_height()
            || canary_anchor.canary_finalized_block_hash != verified_canary.finalized_block_hash()
        {
            return Err(KagemushaV4PostCanaryValidatorLivenessValidationError::CanaryBinding);
        }
        let trust = Self {
            binding: expectations.binding(),
            issuer: expectations.receipt_issuer(),
            validator_ids,
            runtime,
            canary_anchor,
            canary_finality_proof,
        };
        validate_canary_finality_anchor(&trust)?;
        Ok(trust)
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "one ordered verifier preserves fail-closed precedence across all four attestations and the shared proof chain"
)]
fn verify_evidence_body_with_trust(
    body: &KagemushaV4PostCanaryValidatorLivenessEvidenceBodyV1,
    trust: &LivenessTrust<'_>,
) -> Result<
    KagemushaV4VerifiedPostCanaryValidatorLivenessEvidenceV1,
    KagemushaV4PostCanaryValidatorLivenessValidationError,
> {
    body.validate_structure()?;
    validate_canary_finality_anchor(trust)?;
    let expected_challenge = verify_challenge_with_trust(&body.challenge, trust)?;
    if body.endpoint_challenge != expected_challenge {
        return Err(KagemushaV4PostCanaryValidatorLivenessValidationError::Challenge);
    }
    let challenge = &body.challenge.body;

    let canary_height = trust.canary_anchor.canary_finalized_height;
    let mut observed_tip_heights = [0_u64; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT];
    let mut highest_tip = canary_height;
    let expected_genesis = trust.runtime.genesis_expected_hash;
    let expected_config_fingerprint = trust.runtime.sumeragi_config_fingerprint;
    let zero = Hash::prehashed([0; Hash::LENGTH]);
    let mut common_genesis = None;
    let mut common_build_fingerprint = None;

    for (index, observation) in body.observations.iter().enumerate() {
        if observation.request_started_at_unix_ms < challenge.issued_at_unix_ms
            || observation.response_completed_at_unix_ms >= challenge.expires_at_unix_ms
        {
            return Err(KagemushaV4PostCanaryValidatorLivenessValidationError::TimeWindow);
        }
        let attestation = &observation.attestation;
        attestation
            .verify()
            .map_err(|_| KagemushaV4PostCanaryValidatorLivenessValidationError::Observation)?;
        let attestation_body = &attestation.body;
        if attestation_body.challenge != body.endpoint_challenge
            || attestation_body.network_id != trust.binding.network_id
            || attestation_body.node_id != trust.validator_ids[index]
            || observation.target.validator_id != trust.validator_ids[index]
            || attestation_body.genesis_block_hash != expected_genesis
            || attestation_body.status.config_fingerprint != expected_config_fingerprint
            || attestation_body.status.build_fingerprint == zero
        {
            return Err(KagemushaV4PostCanaryValidatorLivenessValidationError::Observation);
        }
        match &common_genesis {
            Some(proof) if proof != &attestation_body.genesis_finality_proof => {
                return Err(KagemushaV4PostCanaryValidatorLivenessValidationError::Genesis);
            }
            None => {
                validate_finality_corridor(&attestation_body.genesis_finality_proof, trust)?;
                verify_bridge_finality_proof(
                    &attestation_body.genesis_finality_proof,
                    &trust.binding.network_id,
                )
                .map_err(|_| KagemushaV4PostCanaryValidatorLivenessValidationError::Genesis)?;
                common_genesis = Some(attestation_body.genesis_finality_proof.clone());
            }
            Some(_) => {}
        }
        match common_build_fingerprint {
            Some(fingerprint) if fingerprint != attestation_body.status.build_fingerprint => {
                return Err(KagemushaV4PostCanaryValidatorLivenessValidationError::Observation);
            }
            None => common_build_fingerprint = Some(attestation_body.status.build_fingerprint),
            Some(_) => {}
        }
        let tip_height = attestation_body.finality_proof.finality_artifact.height;
        if tip_height < canary_height {
            return Err(KagemushaV4PostCanaryValidatorLivenessValidationError::Finality);
        }
        observed_tip_heights[index] = tip_height;
        highest_tip = highest_tip.max(tip_height);
    }

    let proof_count = highest_tip
        .checked_sub(canary_height)
        .and_then(|count| usize::try_from(count).ok())
        .ok_or(KagemushaV4PostCanaryValidatorLivenessValidationError::Finality)?;
    if proof_count != body.post_canary_finality_proof_chain.len()
        || proof_count > KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1
    {
        return Err(KagemushaV4PostCanaryValidatorLivenessValidationError::Finality);
    }
    let mut verifier = BridgeFinalityVerifier::with_context(
        trust.binding.network_id,
        trust.canary_finality_proof.finality_artifact.context_id(),
    );
    verifier
        .verify(trust.canary_finality_proof)
        .map_err(|_| KagemushaV4PostCanaryValidatorLivenessValidationError::Finality)?;
    for proof in &body.post_canary_finality_proof_chain {
        validate_finality_corridor(proof, trust)?;
        verifier
            .verify(proof)
            .map_err(|_| KagemushaV4PostCanaryValidatorLivenessValidationError::Finality)?;
    }
    for observation in &body.observations {
        let tip = &observation.attestation.body.finality_proof;
        let expected = if tip.finality_artifact.height == canary_height {
            trust.canary_finality_proof
        } else {
            let offset = tip
                .finality_artifact
                .height
                .checked_sub(canary_height)
                .and_then(|distance| distance.checked_sub(1))
                .and_then(|offset| usize::try_from(offset).ok())
                .ok_or(KagemushaV4PostCanaryValidatorLivenessValidationError::Finality)?;
            body.post_canary_finality_proof_chain
                .get(offset)
                .ok_or(KagemushaV4PostCanaryValidatorLivenessValidationError::Finality)?
        };
        if tip != expected {
            return Err(KagemushaV4PostCanaryValidatorLivenessValidationError::Finality);
        }
    }

    Ok(KagemushaV4VerifiedPostCanaryValidatorLivenessEvidenceV1 {
        promotion_id: trust.binding.promotion_id,
        canary_transaction_intent: trust.canary_anchor.canary_transaction_intent,
        canary_finalized_height: canary_height,
        canary_finalized_block_hash: trust.canary_anchor.canary_finalized_block_hash,
        endpoint_challenge: body.endpoint_challenge,
        validator_ids: trust.validator_ids.clone(),
        observed_tip_heights,
        highest_observed_tip_height: highest_tip,
    })
}

fn verify_challenge_with_trust(
    challenge: &KagemushaV4PostCanaryValidatorLivenessChallengeV1,
    trust: &LivenessTrust<'_>,
) -> Result<[u8; 32], KagemushaV4PostCanaryValidatorLivenessValidationError> {
    challenge.verify_structure_and_signature()?;
    if challenge.body.binding != *trust.binding
        || challenge.body.canary_anchor != *trust.canary_anchor
        || challenge.body.issuer != *trust.issuer
        || challenge
            .body
            .targets
            .iter()
            .zip(&trust.validator_ids)
            .any(|(target, expected)| &target.validator_id != expected)
    {
        return Err(KagemushaV4PostCanaryValidatorLivenessValidationError::ActivationBinding);
    }
    challenge.endpoint_challenge()
}

fn validate_canary_finality_anchor(
    trust: &LivenessTrust<'_>,
) -> Result<(), KagemushaV4PostCanaryValidatorLivenessValidationError> {
    trust.canary_anchor.validate_structure()?;
    let proof = trust.canary_finality_proof;
    let block_time = u64::try_from(proof.block_header.creation_time().as_millis())
        .map_err(|_| KagemushaV4PostCanaryValidatorLivenessValidationError::CanaryBinding)?;
    if proof.finality_artifact.height != trust.canary_anchor.canary_finalized_height
        || proof.finality_artifact.block_hash != trust.canary_anchor.canary_finalized_block_hash
        || proof.block_header.hash() != trust.canary_anchor.canary_finalized_block_hash
        || block_time != trust.canary_anchor.canary_finalized_block_time_unix_ms
    {
        return Err(KagemushaV4PostCanaryValidatorLivenessValidationError::CanaryBinding);
    }
    validate_finality_corridor(proof, trust)?;
    verify_bridge_finality_proof(proof, &trust.binding.network_id)
        .map_err(|_| KagemushaV4PostCanaryValidatorLivenessValidationError::Finality)
}

fn validate_finality_corridor(
    proof: &BridgeFinalityProof,
    trust: &LivenessTrust<'_>,
) -> Result<(), KagemushaV4PostCanaryValidatorLivenessValidationError> {
    let context = &proof.finality_artifact.height_context;
    let runtime = trust.runtime;
    if context.network_id != trust.binding.network_id
        || context.mode != ConsensusMode::Permissioned
        || context.nexus_amx_context_hash
            != Hash::prehashed(runtime.genesis_context.nexus_amx_context_hash)
        || context.execution_policy_hash != trust.binding.execution_policy_hash
        || context.da_layout != runtime.genesis_context.da_layout
        || context.snapshot_bootstrap.is_some()
        || context.roster.len() != KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT
        || proof.finality_artifact.validator_set_pops.len()
            != KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT
        || context
            .roster
            .iter()
            .zip(&trust.validator_ids)
            .any(|(member, expected)| member.power != 1 || &member.validator != expected)
        || proof
            .finality_artifact
            .validator_set_pops
            .iter()
            .zip(&runtime.validators)
            .any(|(actual, expected)| actual != &expected.bls_pop)
    {
        return Err(KagemushaV4PostCanaryValidatorLivenessValidationError::Finality);
    }
    Ok(())
}

fn validate_liveness_torii_origin(
    origin: &str,
) -> Result<(), KagemushaV4PostCanaryValidatorLivenessValidationError> {
    if origin.is_empty()
        || origin.len() > KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_ORIGIN_MAX_BYTES
        || !origin.is_ascii()
        || origin != origin.to_ascii_lowercase()
    {
        return Err(
            KagemushaV4PostCanaryValidatorLivenessValidationError::InvalidField(
                "post_canary_liveness.canonical_torii_origin",
            ),
        );
    }
    let Some(authority) = origin.strip_prefix("https://") else {
        return Err(
            KagemushaV4PostCanaryValidatorLivenessValidationError::InvalidField(
                "post_canary_liveness.canonical_torii_origin",
            ),
        );
    };
    if authority.is_empty()
        || authority
            .chars()
            .any(|character| matches!(character, '/' | '?' | '#' | '@' | '[' | ']'))
    {
        return Err(
            KagemushaV4PostCanaryValidatorLivenessValidationError::InvalidField(
                "post_canary_liveness.canonical_torii_origin",
            ),
        );
    }
    let (host, port) = authority
        .rsplit_once(':')
        .map_or((authority, None), |(host, port)| (host, Some(port)));
    if host.is_empty()
        || host.len() > 253
        || host.starts_with('.')
        || host.ends_with('.')
        || host.parse::<std::net::IpAddr>().is_ok()
        || host.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
        || port.is_some_and(|port_text| {
            port_text.parse::<u16>().map_or(true, |port| {
                port == 0 || port == 443 || port_text != port.to_string()
            })
        })
    {
        return Err(
            KagemushaV4PostCanaryValidatorLivenessValidationError::InvalidField(
                "post_canary_liveness.canonical_torii_origin",
            ),
        );
    }
    Ok(())
}

fn validate_identity_digest(
    digest: KagemushaExactBytesDigestV1,
    maximum: usize,
    field: &'static str,
) -> Result<(), KagemushaV4PostCanaryValidatorLivenessValidationError> {
    digest
        .validate()
        .map_err(|_| KagemushaV4PostCanaryValidatorLivenessValidationError::InvalidField(field))?;
    if digest.byte_len
        > u64::try_from(maximum).expect("liveness byte limits fit the digest length field")
    {
        return Err(KagemushaV4PostCanaryValidatorLivenessValidationError::InvalidField(field));
    }
    Ok(())
}

fn supports_issuer_signature_algorithm(signer: &PublicKey) -> bool {
    matches!(
        signer.try_algorithm(),
        Ok(Algorithm::Ed25519 | Algorithm::MlDsa | Algorithm::BlsNormal)
    )
}

fn verify_typed_signature<T>(
    signature: &SignatureOf<T>,
    signer: &PublicKey,
    signing_hash: HashOf<T>,
) -> Result<(), KagemushaV4PostCanaryValidatorLivenessValidationError> {
    match signer.try_algorithm() {
        Ok(Algorithm::Ed25519) => {
            iroha_crypto::ed25519_parse_signature(signature.payload())
                .map_err(|_| KagemushaV4PostCanaryValidatorLivenessValidationError::Signature)?;
        }
        Ok(Algorithm::MlDsa) => {
            iroha_crypto::mldsa65_parse_signature(signature.payload())
                .map_err(|_| KagemushaV4PostCanaryValidatorLivenessValidationError::Signature)?;
        }
        Ok(Algorithm::BlsNormal) => {}
        _ => return Err(KagemushaV4PostCanaryValidatorLivenessValidationError::Signature),
    }
    signature
        .verify_hash(signer, signing_hash)
        .map_err(|_| KagemushaV4PostCanaryValidatorLivenessValidationError::Signature)
}

fn hash_is_zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

fn enforce_artifact_size<T: norito::NoritoSerialize>(
    value: &T,
    maximum: usize,
) -> Result<(), KagemushaV4PostCanaryValidatorLivenessValidationError> {
    let _canonical_flags =
        norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let actual = norito::core::encoded_frame_len(value)
        .map_err(|_| KagemushaV4PostCanaryValidatorLivenessValidationError::Encode)?;
    if actual > maximum {
        return Err(
            KagemushaV4PostCanaryValidatorLivenessValidationError::Size { actual, maximum },
        );
    }
    Ok(())
}

fn check_input_size(
    bytes: &[u8],
    maximum: usize,
) -> Result<(), KagemushaV4PostCanaryValidatorLivenessValidationError> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(
            KagemushaV4PostCanaryValidatorLivenessValidationError::Size {
                actual: bytes.len(),
                maximum,
            },
        );
    }
    Ok(())
}

/// Failure while validating four-validator post-canary liveness evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum KagemushaV4PostCanaryValidatorLivenessValidationError {
    /// One named structural field is invalid.
    #[error("invalid Kagemusha post-canary liveness field: {0}")]
    InvalidField(&'static str),
    /// The supplied signing key differs from the authenticated issuer.
    #[error("Kagemusha post-canary liveness signer does not match the issuer")]
    SignerMismatch,
    /// Canonical encoding failed.
    #[error("failed to encode canonical Kagemusha post-canary liveness evidence")]
    Encode,
    /// Canonical bounded decoding failed.
    #[error("failed to decode canonical Kagemusha post-canary liveness evidence")]
    Decode,
    /// An artifact violates its exact canonical byte ceiling.
    #[error("Kagemusha post-canary liveness artifact is {actual} bytes; maximum is {maximum}")]
    Size {
        /// Actual exact byte length.
        actual: usize,
        /// Maximum permitted byte length.
        maximum: usize,
    },
    /// An issuer signature is malformed or invalid.
    #[error("invalid Kagemusha post-canary liveness issuer signature")]
    Signature,
    /// The signed challenge is malformed or mismatched.
    #[error("invalid Kagemusha post-canary liveness challenge")]
    Challenge,
    /// The activation-qualified binding, issuer, or validator set differs.
    #[error("Kagemusha post-canary liveness differs from activation expectations")]
    ActivationBinding,
    /// The supplied canary proof differs from the exact authenticated canary anchor.
    #[error("invalid Kagemusha post-canary liveness canary binding")]
    CanaryBinding,
    /// A trusted-host observation is malformed or disagrees with its node response.
    #[error("invalid Kagemusha post-canary validator observation")]
    Observation,
    /// A request or response falls outside the exclusive signed collection window.
    #[error("Kagemusha post-canary validator observation is outside the collection window")]
    TimeWindow,
    /// A signed genesis identity or proof is invalid or inconsistent.
    #[error("invalid Kagemusha post-canary validator genesis proof")]
    Genesis,
    /// The canary anchor or common successor chain is invalid.
    #[error("invalid Kagemusha post-canary validator finality chain")]
    Finality,
}

#[cfg(test)]
pub(super) mod tests {
    use std::num::NonZeroU64;

    use crate::{
        ChainId, NetworkId,
        block::consensus_v2::{
            self as wire, BlockSubject, ConsensusRound, DataAvailabilityLayout, DualQuorum,
            ExecutionCommitment, GlobalPhase, HeightContext, PayloadEncoding, QuorumCertificate,
            SumeragiV2BodyState, SumeragiV2CommitQcStatus, SumeragiV2GenesisContextParameters,
            SumeragiV2HeightContextStatus, SumeragiV2LivenessStatus, SumeragiV2Status,
            SumeragiV2StatusPhase, ValidatorPower, Vote, finality::V2FinalityArtifact,
        },
        bridge::{
            BRIDGE_FINALITY_ATTESTATION_VERSION_V1, BRIDGE_FINALITY_PROOF_VERSION_V2,
            BridgeFinalityAttestationBodyV1,
        },
        offline::KagemushaV4RuntimeValidatorProjectionV1,
    };
    use iroha_crypto::{Signature, SignatureOf};

    use super::*;

    const LIVENESS_TEST_STACK_BYTES: usize = 16 * 1024 * 1024;

    fn run_liveness_test(test: impl FnOnce() + Send + 'static) {
        let result = std::thread::Builder::new()
            .name("kagemusha-post-canary-liveness-test".to_owned())
            .stack_size(LIVENESS_TEST_STACK_BYTES)
            .spawn(test)
            .expect("spawn Kagemusha liveness test with reviewed stack")
            .join();
        if let Err(payload) = result {
            std::panic::resume_unwind(payload);
        }
    }

    macro_rules! liveness_test {
        ($name:ident, $body:ident) => {
            #[test]
            fn $name() {
                run_liveness_test($body);
            }
        };
    }

    struct Fixture {
        binding: KagemushaV4PromotionBindingV1,
        runtime: KagemushaV4RuntimeEffectiveConfigProjectionV1,
        issuer: KeyPair,
        validator_keys: Vec<KeyPair>,
        genesis: BridgeFinalityProof,
        canary: BridgeFinalityProof,
        tip: BridgeFinalityProof,
        canary_anchor: KagemushaV4PostCanaryValidatorLivenessCanaryAnchorV1,
    }

    impl Fixture {
        #[expect(
            clippy::too_many_lines,
            reason = "the self-contained fixture constructs one exact promotion, runtime, and three-proof chain"
        )]
        fn new() -> Self {
            let mut validator_keys = [0x91_u8, 0x92, 0x93, 0x94]
                .into_iter()
                .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
                .collect::<Vec<_>>();
            validator_keys.sort_by(|left, right| {
                PeerId::new(left.public_key().clone()).cmp(&PeerId::new(right.public_key().clone()))
            });
            let network_id =
                NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
                    Hash::new(b"post-canary liveness fixture network"),
                ));
            let da_layout = DataAvailabilityLayout {
                encoding: PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 4096,
                max_chunk_count: 8,
            };
            let nexus_amx_context_hash = Hash::new(b"post-canary liveness nexus context");
            let execution_policy_hash = Hash::new(b"post-canary liveness execution policy");
            let genesis = make_finality_proof(
                1,
                None,
                &validator_keys,
                network_id,
                da_layout,
                nexus_amx_context_hash,
                execution_policy_hash,
            );
            let canary = make_finality_proof(
                2,
                Some(&genesis),
                &validator_keys,
                network_id,
                da_layout,
                nexus_amx_context_hash,
                execution_policy_hash,
            );
            let tip = make_finality_proof(
                3,
                Some(&canary),
                &validator_keys,
                network_id,
                da_layout,
                nexus_amx_context_hash,
                execution_policy_hash,
            );
            let controller = KeyPair::from_seed(vec![0x81; 32], Algorithm::Ed25519);
            let issuer = KeyPair::from_seed(vec![0x82; 32], Algorithm::Ed25519);
            let binding = KagemushaV4PromotionBindingV1 {
                promotion_controller: controller.public_key().clone(),
                promotion_reservation: exact_digest(b"signed promotion reservation"),
                promotion_id: [0x31; 32],
                network_id,
                reviewed_source_closure_descriptor_sha256: [0x32; 32],
                manifest_sha256: [0x33; 32],
                release_record_sha256: [0x34; 32],
                release_policy_source: exact_digest(b"release policy source"),
                device_attestation_policy_norito: exact_digest(b"device attestation policy"),
                signed_genesis: exact_digest(b"signed genesis"),
                catalog_consensus_policy_digest: [0x35; 32],
                execution_policy_hash,
            };
            let validators = std::array::from_fn(|index| {
                let key = &validator_keys[index];
                KagemushaV4RuntimeValidatorProjectionV1 {
                    validator_id: PeerId::new(key.public_key().clone()),
                    public_address: format!("127.0.0.1:{}", 15_000 + index)
                        .parse()
                        .expect("fixture validator address"),
                    bls_pop: iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("fixture validator PoP"),
                }
            });
            let runtime = KagemushaV4RuntimeEffectiveConfigProjectionV1 {
                chain: ChainId::from("post-canary-liveness-fixture"),
                chain_discriminant: 77,
                is_validator: true,
                genesis_public_key: KeyPair::from_seed(vec![0x83; 32], Algorithm::Ed25519)
                    .public_key()
                    .clone(),
                genesis_expected_hash: genesis.block_header.hash(),
                validators,
                sumeragi_config_fingerprint: Hash::new(b"post-canary liveness config"),
                genesis_context: SumeragiV2GenesisContextParameters {
                    da_layout,
                    nexus_amx_context_hash: *nexus_amx_context_hash.as_ref(),
                    execution_policy_hash: *execution_policy_hash.as_ref(),
                },
                kagemusha_max_decoded_bytes: 64 * 1024 * 1024,
            };
            let canary_anchor = KagemushaV4PostCanaryValidatorLivenessCanaryAnchorV1 {
                schema: KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_CANARY_ANCHOR_SCHEMA.to_owned(),
                version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
                activation_finality_receipt: exact_digest(b"activation finality receipt"),
                canary_authorization: exact_digest(b"canary authorization"),
                canary_transaction_intent: HashOf::from_untyped_unchecked(Hash::new(
                    b"canary transaction intent",
                )),
                canary_transaction_wire: exact_digest(b"signed canary transaction wire"),
                canary_finalized_height: canary.finality_artifact.height,
                canary_finalized_block_hash: canary.finality_artifact.block_hash,
                canary_finalized_block_time_unix_ms: u64::try_from(
                    canary.block_header.creation_time().as_millis(),
                )
                .expect("fixture block time"),
            };
            binding.validate().expect("valid fixture binding");
            runtime.validate().expect("valid fixture runtime");
            Self {
                binding,
                runtime,
                issuer,
                validator_keys,
                genesis,
                canary,
                tip,
                canary_anchor,
            }
        }

        fn trust(&self) -> LivenessTrust<'_> {
            LivenessTrust {
                binding: &self.binding,
                issuer: self.issuer.public_key(),
                validator_ids: std::array::from_fn(|index| {
                    PeerId::new(self.validator_keys[index].public_key().clone())
                }),
                runtime: &self.runtime,
                canary_anchor: &self.canary_anchor,
                canary_finality_proof: &self.canary,
            }
        }

        fn challenge_body(&self) -> KagemushaV4PostCanaryValidatorLivenessChallengeBodyV1 {
            let targets =
                std::array::from_fn(|index| KagemushaV4PostCanaryValidatorLivenessTargetV1 {
                    validator_id: PeerId::new(self.validator_keys[index].public_key().clone()),
                    canonical_torii_origin: format!("https://validator-{index}.example.test"),
                });
            let issued_at_unix_ms = self.canary_anchor.canary_finalized_block_time_unix_ms + 1;
            KagemushaV4PostCanaryValidatorLivenessChallengeBodyV1 {
                schema: KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_CHALLENGE_BODY_SCHEMA
                    .to_owned(),
                version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
                binding: self.binding.clone(),
                canary_anchor: self.canary_anchor.clone(),
                targets,
                issuer: self.issuer.public_key().clone(),
                nonce: [0xA5; 32],
                issued_at_unix_ms,
                expires_at_unix_ms: issued_at_unix_ms
                    + KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_MAX_INTERVAL_MS,
            }
        }

        fn evidence_body(&self) -> KagemushaV4PostCanaryValidatorLivenessEvidenceBodyV1 {
            let challenge = KagemushaV4PostCanaryValidatorLivenessChallengeV1::try_sign(
                self.challenge_body(),
                &self.issuer,
            )
            .expect("sign fixture challenge");
            let endpoint_challenge = challenge
                .endpoint_challenge()
                .expect("derive fixture endpoint challenge");
            let observations = std::array::from_fn(|index| {
                let tip = if index < 2 { &self.canary } else { &self.tip };
                let attestation = make_attestation(
                    &self.validator_keys[index],
                    endpoint_challenge,
                    &self.genesis,
                    tip,
                    &self.runtime,
                );
                let exact_attestation =
                    norito::encode_canonical(&attestation).expect("canonical fixture attestation");
                KagemushaV4PostCanaryValidatorLivenessObservationV1 {
                    schema: KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_OBSERVATION_SCHEMA
                        .to_owned(),
                    version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
                    target: challenge.body.targets[index].clone(),
                    request_started_at_unix_ms: challenge.body.issued_at_unix_ms
                        + 10
                        + u64::try_from(index).expect("small fixture index"),
                    response_completed_at_unix_ms: challenge.body.issued_at_unix_ms
                        + 20
                        + u64::try_from(index).expect("small fixture index"),
                    attestation_response_norito: exact_digest(&exact_attestation),
                    attestation,
                }
            });
            KagemushaV4PostCanaryValidatorLivenessEvidenceBodyV1 {
                schema: KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_EVIDENCE_BODY_SCHEMA.to_owned(),
                version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
                challenge,
                endpoint_challenge,
                observations,
                post_canary_finality_proof_chain: vec![self.tip.clone()],
            }
        }
    }

    pub fn signed_liveness_evidence_fixture() -> KagemushaV4PostCanaryValidatorLivenessEvidenceV1 {
        let fixture = Fixture::new();
        let body = fixture.evidence_body();
        KagemushaV4PostCanaryValidatorLivenessEvidenceV1::try_sign_with_trust(
            body,
            &fixture.issuer,
            &fixture.trust(),
        )
        .expect("signed liveness wire fixture")
    }

    fn exact_digest(bytes: &[u8]) -> KagemushaExactBytesDigestV1 {
        KagemushaExactBytesDigestV1::from_bytes(bytes).expect("non-empty fixture bytes")
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the fixture exposes every consensus identity that the evidence must pin"
    )]
    fn make_finality_proof(
        height: u64,
        parent: Option<&BridgeFinalityProof>,
        keys: &[KeyPair],
        network_id: NetworkId,
        da_layout: DataAvailabilityLayout,
        nexus_amx_context_hash: Hash,
        execution_policy_hash: Hash,
    ) -> BridgeFinalityProof {
        let roster = keys
            .iter()
            .map(|key| ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let validator_set_pops = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("derive fixture proof of possession")
            })
            .collect::<Vec<_>>();
        let parent_hash = parent.map(|proof| proof.finality_artifact.block_hash);
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("non-zero fixture height"),
            parent_hash,
            None,
            None,
            height * 1_000,
            0,
        );
        let context = HeightContext {
            network_id,
            protocol_version: wire::PROTOCOL_VERSION,
            height,
            epoch: 0,
            epoch_end_height: 10,
            next_epoch_snapshot: None,
            mode: ConsensusMode::Permissioned,
            parent_commit_qc: parent.map(|proof| proof.finality_artifact.commit_qc.clone()),
            snapshot_bootstrap: None,
            roster,
            quorum: DualQuorum::from_roster(
                &keys
                    .iter()
                    .map(|key| ValidatorPower {
                        validator: PeerId::new(key.public_key().clone()),
                        power: 1,
                    })
                    .collect::<Vec<_>>(),
            )
            .expect("valid fixture quorum"),
            nexus_amx_context_hash,
            execution_policy_hash,
            da_layout,
            leader_seed: [0x5A; 32],
        };
        let height_bytes = height.to_le_bytes();
        let subject = BlockSubject {
            parent_block_hash: parent_hash,
            block_hash: header.hash(),
            payload_hash: Hash::new_from_chunks(&[b"liveness fixture payload", &height_bytes]),
        };
        let round = ConsensusRound {
            context_id: context.id(),
            height,
            view: 0,
        };
        let execution_commitment = ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new_from_chunks(&[b"liveness fixture parent state", &height_bytes]),
            Hash::new_from_chunks(&[b"liveness fixture post state", &height_bytes]),
            Hash::new_from_chunks(&[b"liveness fixture writes", &height_bytes]),
            1,
            Hash::new_from_chunks(&[b"liveness fixture block wire", &height_bytes]),
        );
        let signers = vec![0, 1, 2];
        let mut commit_qc = QuorumCertificate {
            round,
            proposal_round: round,
            phase: GlobalPhase::Commit,
            subject,
            execution_commitment,
            signers: signers.clone(),
            aggregate_signature: vec![1],
        };
        let preimage = Vote {
            round,
            proposal_round: round,
            phase: GlobalPhase::Commit,
            subject,
            execution_commitment,
            signer: 0,
            signature: Vec::new(),
        }
        .signature_preimage();
        let signatures = signers
            .iter()
            .map(|index| {
                Signature::try_new(
                    keys[usize::try_from(*index).expect("fixture signer index")].private_key(),
                    &preimage,
                )
                .expect("sign fixture commit vote")
                .payload()
                .to_vec()
            })
            .collect::<Vec<_>>();
        let signature_refs = signatures.iter().map(Vec::as_slice).collect::<Vec<_>>();
        commit_qc.aggregate_signature =
            iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
                .expect("aggregate fixture commit votes");
        let artifact = V2FinalityArtifact::new(context, subject, commit_qc, validator_set_pops);
        let proof = BridgeFinalityProof {
            version: BRIDGE_FINALITY_PROOF_VERSION_V2,
            block_header: header,
            finality_artifact: artifact,
        };
        verify_bridge_finality_proof(&proof, &network_id).expect("valid fixture proof");
        proof
    }

    fn make_attestation(
        signer: &KeyPair,
        challenge: [u8; 32],
        genesis: &BridgeFinalityProof,
        tip: &BridgeFinalityProof,
        runtime: &KagemushaV4RuntimeEffectiveConfigProjectionV1,
    ) -> BridgeFinalityAttestationV1 {
        let artifact = &tip.finality_artifact;
        let context = &artifact.height_context;
        let node_id = PeerId::new(signer.public_key().clone());
        let signed_power = artifact
            .commit_qc
            .signers
            .iter()
            .map(|index| context.roster[usize::try_from(*index).expect("signer index")].power)
            .sum();
        let status = SumeragiV2Status {
            protocol_version: wire::PROTOCOL_VERSION,
            node_fingerprint: Hash::new(node_id.encode()),
            build_fingerprint: Hash::new(b"post-canary liveness fixture build"),
            config_fingerprint: runtime.sumeragi_config_fingerprint,
            restart_required: false,
            height_context_id: context.id(),
            height: artifact.height,
            view: artifact.commit_qc.round.view,
            phase: SumeragiV2StatusPhase::PendingApply,
            leader: context.leader(artifact.commit_qc.round.view),
            locked_prepare_qc: None,
            highest_prepare_qc: None,
            last_timeout_certificate: None,
            body_state: SumeragiV2BodyState::Applied,
            pending_persistence_id: None,
            last_committed_height: artifact.height,
            last_committed_subject: Some(artifact.subject),
            height_context: SumeragiV2HeightContextStatus {
                epoch: context.epoch,
                epoch_end_height: context.epoch_end_height,
                mode: context.mode,
                epoch_seed: context.leader_seed,
                validator_count: u32::try_from(context.roster.len()).expect("validator count"),
                quorum: context.quorum,
            },
            last_commit_qc: Some(SumeragiV2CommitQcStatus {
                certificate: artifact.commit_qc.as_ref(),
                validator_count: u32::try_from(context.roster.len()).expect("validator count"),
                signer_count: u32::try_from(artifact.commit_qc.signers.len())
                    .expect("signer count"),
                min_signers: context.quorum.min_signers,
                signed_power,
                total_power: context.quorum.total_power,
            }),
            liveness: SumeragiV2LivenessStatus::default(),
        };
        let body = BridgeFinalityAttestationBodyV1 {
            version: BRIDGE_FINALITY_ATTESTATION_VERSION_V1,
            challenge,
            network_id: context.network_id,
            node_id,
            node_fingerprint: status.node_fingerprint,
            genesis_block_hash: genesis.block_header.hash(),
            genesis_finality_proof: genesis.clone(),
            status,
            finality_proof: tip.clone(),
        };
        let signature = SignatureOf::try_from_hash(signer.private_key(), body.signing_hash())
            .expect("sign fixture attestation");
        let attestation = BridgeFinalityAttestationV1 { body, signature };
        attestation.verify().expect("valid fixture attestation");
        attestation
    }

    fn refresh_observation_attestation(
        observation: &mut KagemushaV4PostCanaryValidatorLivenessObservationV1,
        signer: &KeyPair,
    ) {
        observation.attestation.signature = SignatureOf::try_from_hash(
            signer.private_key(),
            observation.attestation.body.signing_hash(),
        )
        .expect("re-sign hostile attestation");
        let bytes = norito::encode_canonical(&observation.attestation)
            .expect("canonical hostile attestation");
        observation.attestation_response_norito = exact_digest(&bytes);
    }

    fn exact_four_validator_evidence_roundtrips_and_verifies_body() {
        let fixture = Fixture::new();
        let trust = fixture.trust();
        let body = fixture.evidence_body();
        let challenge_bytes =
            norito::encode_canonical(&body.challenge).expect("canonical signed fixture challenge");
        let decoded_challenge =
            KagemushaV4PostCanaryValidatorLivenessChallengeV1::decode_canonical(&challenge_bytes)
                .expect("decode exact signed challenge");
        assert_eq!(
            decoded_challenge
                .endpoint_challenge()
                .expect("derive decoded challenge"),
            body.endpoint_challenge
        );

        let evidence = KagemushaV4PostCanaryValidatorLivenessEvidenceV1::try_sign_with_trust(
            body,
            &fixture.issuer,
            &trust,
        )
        .expect("sign valid four-validator evidence");
        let bytes = norito::encode_canonical(&evidence).expect("canonical liveness evidence");
        let decoded = KagemushaV4PostCanaryValidatorLivenessEvidenceV1::decode_canonical(&bytes)
            .expect("decode exact liveness evidence");
        let verified = decoded
            .verify_exact_with_trust(&bytes, &trust)
            .expect("verify exact four-validator evidence");
        assert_eq!(verified.promotion_id(), fixture.binding.promotion_id);
        assert_eq!(verified.canary_finalized_height(), 2);
        assert_eq!(verified.highest_observed_tip_height(), 3);
        assert_eq!(verified.observed_tip_heights(), &[2, 2, 3, 3]);
        assert_eq!(verified.validator_ids().len(), 4);
    }

    fn copied_validator_response_does_not_prove_four_independent_nodes_body() {
        let fixture = Fixture::new();
        let trust = fixture.trust();
        let mut body = fixture.evidence_body();
        body.observations[3].attestation = body.observations[0].attestation.clone();
        let bytes = norito::encode_canonical(&body.observations[3].attestation)
            .expect("canonical copied attestation");
        body.observations[3].attestation_response_norito = exact_digest(&bytes);
        assert_eq!(
            KagemushaV4PostCanaryValidatorLivenessEvidenceV1::try_sign_with_trust(
                body,
                &fixture.issuer,
                &trust,
            ),
            Err(KagemushaV4PostCanaryValidatorLivenessValidationError::Observation)
        );
    }

    fn stale_or_different_node_challenge_is_rejected_body() {
        let fixture = Fixture::new();
        let trust = fixture.trust();
        let mut body = fixture.evidence_body();
        body.observations[2].attestation.body.challenge = [0xE1; 32];
        refresh_observation_attestation(&mut body.observations[2], &fixture.validator_keys[2]);
        assert_eq!(
            KagemushaV4PostCanaryValidatorLivenessEvidenceV1::try_sign_with_trust(
                body,
                &fixture.issuer,
                &trust,
            ),
            Err(KagemushaV4PostCanaryValidatorLivenessValidationError::Observation)
        );
    }

    fn attestation_structure_cannot_hide_an_invalid_finality_qc_body() {
        let fixture = Fixture::new();
        let trust = fixture.trust();
        let mut body = fixture.evidence_body();
        let signature = &mut body.post_canary_finality_proof_chain[0]
            .finality_artifact
            .commit_qc
            .aggregate_signature;
        signature[0] ^= 0x01;
        let hostile_tip = body.post_canary_finality_proof_chain[0].clone();
        for index in 2..4 {
            let observation = &mut body.observations[index];
            observation.attestation.body.finality_proof = hostile_tip.clone();
            observation
                .attestation
                .body
                .status
                .last_commit_qc
                .as_mut()
                .expect("fixture commit summary")
                .certificate = hostile_tip.finality_artifact.commit_qc.as_ref();
            refresh_observation_attestation(observation, &fixture.validator_keys[index]);
            observation
                .attestation
                .verify()
                .expect("hostile attestation remains structurally and node-signature valid");
        }
        assert_eq!(
            KagemushaV4PostCanaryValidatorLivenessEvidenceV1::try_sign_with_trust(
                body,
                &fixture.issuer,
                &trust,
            ),
            Err(KagemushaV4PostCanaryValidatorLivenessValidationError::Finality)
        );
    }

    fn shared_chain_must_reach_the_highest_observed_tip_exactly_body() {
        let fixture = Fixture::new();
        let trust = fixture.trust();
        let mut body = fixture.evidence_body();
        body.post_canary_finality_proof_chain.clear();
        assert_eq!(
            KagemushaV4PostCanaryValidatorLivenessEvidenceV1::try_sign_with_trust(
                body,
                &fixture.issuer,
                &trust,
            ),
            Err(KagemushaV4PostCanaryValidatorLivenessValidationError::Finality)
        );
    }

    fn response_expiry_is_exclusive_body() {
        let fixture = Fixture::new();
        let trust = fixture.trust();
        let mut body = fixture.evidence_body();
        body.observations[1].request_started_at_unix_ms = body.challenge.body.expires_at_unix_ms;
        body.observations[1].response_completed_at_unix_ms = body.challenge.body.expires_at_unix_ms;
        assert_eq!(
            KagemushaV4PostCanaryValidatorLivenessEvidenceV1::try_sign_with_trust(
                body,
                &fixture.issuer,
                &trust,
            ),
            Err(KagemushaV4PostCanaryValidatorLivenessValidationError::TimeWindow)
        );
    }

    fn challenge_rejects_noncanonical_or_duplicate_origins_and_retroactive_time_body() {
        let fixture = Fixture::new();
        let mut body = fixture.challenge_body();
        body.targets[0].canonical_torii_origin = "https://127.0.0.1".to_owned();
        assert!(
            KagemushaV4PostCanaryValidatorLivenessChallengeV1::try_sign(body, &fixture.issuer)
                .is_err()
        );

        let mut body = fixture.challenge_body();
        body.targets[1].canonical_torii_origin = body.targets[0].canonical_torii_origin.clone();
        assert!(
            KagemushaV4PostCanaryValidatorLivenessChallengeV1::try_sign(body, &fixture.issuer)
                .is_err()
        );

        let mut body = fixture.challenge_body();
        body.issued_at_unix_ms = fixture.canary_anchor.canary_finalized_block_time_unix_ms;
        assert!(
            KagemushaV4PostCanaryValidatorLivenessChallengeV1::try_sign(body, &fixture.issuer)
                .is_err()
        );
    }

    fn challenge_rejects_issuer_role_overlap_and_wrong_private_key_body() {
        let fixture = Fixture::new();
        let mut controller_overlap = fixture.challenge_body();
        let controller = KeyPair::from_seed(vec![0x81; 32], Algorithm::Ed25519);
        controller_overlap.issuer = controller.public_key().clone();
        assert!(
            KagemushaV4PostCanaryValidatorLivenessChallengeV1::try_sign(
                controller_overlap,
                &controller,
            )
            .is_err()
        );

        let mut validator_overlap = fixture.challenge_body();
        validator_overlap.issuer = fixture.validator_keys[0].public_key().clone();
        assert!(
            KagemushaV4PostCanaryValidatorLivenessChallengeV1::try_sign(
                validator_overlap,
                &fixture.validator_keys[0],
            )
            .is_err()
        );

        let wrong_issuer = KeyPair::from_seed(vec![0x84; 32], Algorithm::Ed25519);
        assert_eq!(
            KagemushaV4PostCanaryValidatorLivenessChallengeV1::try_sign(
                fixture.challenge_body(),
                &wrong_issuer,
            ),
            Err(KagemushaV4PostCanaryValidatorLivenessValidationError::SignerMismatch)
        );
    }

    fn exact_decoders_reject_trailing_bytes_and_outer_signature_splice_body() {
        let fixture = Fixture::new();
        let trust = fixture.trust();
        let evidence = KagemushaV4PostCanaryValidatorLivenessEvidenceV1::try_sign_with_trust(
            fixture.evidence_body(),
            &fixture.issuer,
            &trust,
        )
        .expect("sign fixture evidence");
        let bytes = norito::encode_canonical(&evidence).expect("canonical fixture evidence");
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(
            KagemushaV4PostCanaryValidatorLivenessEvidenceV1::decode_canonical(&trailing).is_err()
        );

        let mut spliced = evidence;
        spliced.body.observations[0].request_started_at_unix_ms += 1;
        let spliced_bytes =
            norito::encode_canonical(&spliced).expect("canonical spliced fixture evidence");
        assert_eq!(
            spliced.verify_exact_with_trust(&spliced_bytes, &trust),
            Err(KagemushaV4PostCanaryValidatorLivenessValidationError::Signature)
        );
    }

    liveness_test!(
        exact_four_validator_evidence_roundtrips_and_verifies,
        exact_four_validator_evidence_roundtrips_and_verifies_body
    );
    liveness_test!(
        copied_validator_response_does_not_prove_four_independent_nodes,
        copied_validator_response_does_not_prove_four_independent_nodes_body
    );
    liveness_test!(
        stale_or_different_node_challenge_is_rejected,
        stale_or_different_node_challenge_is_rejected_body
    );
    liveness_test!(
        attestation_structure_cannot_hide_an_invalid_finality_qc,
        attestation_structure_cannot_hide_an_invalid_finality_qc_body
    );
    liveness_test!(
        shared_chain_must_reach_the_highest_observed_tip_exactly,
        shared_chain_must_reach_the_highest_observed_tip_exactly_body
    );
    liveness_test!(
        response_expiry_is_exclusive,
        response_expiry_is_exclusive_body
    );
    liveness_test!(
        challenge_rejects_noncanonical_or_duplicate_origins_and_retroactive_time,
        challenge_rejects_noncanonical_or_duplicate_origins_and_retroactive_time_body
    );
    liveness_test!(
        challenge_rejects_issuer_role_overlap_and_wrong_private_key,
        challenge_rejects_issuer_role_overlap_and_wrong_private_key_body
    );
    liveness_test!(
        exact_decoders_reject_trailing_bytes_and_outer_signature_splice,
        exact_decoders_reject_trailing_bytes_and_outer_signature_splice_body
    );
}
