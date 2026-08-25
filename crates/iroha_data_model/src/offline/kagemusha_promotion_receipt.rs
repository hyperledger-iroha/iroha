//! Fail-closed Kagemusha V4 validator qualification and activation receipts.
//!
//! These types close the release-record publication cycle without claiming to
//! operate a validator or signer. A caller must separately collect four
//! validator signatures, submit the exact governed activation transaction, and
//! capture its result-bearing finalized block. Validation binds those artifacts
//! together; it never creates, publishes, submits, or activates them.

use crate::{
    NetworkId,
    account::{AccountId, MultisigPolicy},
    block::{
        SignedBlock, decode_framed_signed_block,
        proofs::{AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1, TrustedBlockProofAnchor},
    },
    bridge::{BridgeFinalityProof, BridgeFinalityVerifier, verify_bridge_finality_proof},
    isi::{Instruction as _, offline::ActivateKagemushaRecursiveReleaseV4},
    offline::OfflineDeviceAttestationPolicy,
    peer::PeerId,
    query::CommittedTransaction,
    transaction::{Executable, SignedTransaction, TransactionEntrypoint},
};
#[cfg(feature = "json")]
use base64::{Engine as _, engine::general_purpose::STANDARD};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, PublicKey, SignatureOf};
use iroha_schema::IntoSchema;
use norito::{
    codec::{Decode, Encode},
    core as ncore,
};
use sha2::{Digest as _, Sha256};
use std::fmt;
use thiserror::Error;

use super::KagemushaV4RuntimeEffectiveConfigProjectionV1;

/// Exact first-release validator count required by Kagemusha activation.
pub const KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT: usize = 4;
/// Minimum distinct governance signers required by the V4 activation corridor.
pub const KAGEMUSHA_V4_ACTIVATION_GOVERNANCE_MIN_SIGNERS: usize = 2;
/// Maximum immediate-successor proofs retained after the trusted anchor.
pub const KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1: usize = 4096;
/// Maximum canonical Norito bytes accepted for one activation receipt.
pub const KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_BYTES: usize = 64 * 1024 * 1024;
/// Maximum canonical Norito bytes accepted for one signed promotion reservation.
pub const KAGEMUSHA_V4_PROMOTION_RESERVATION_MAX_BYTES: usize = 1024 * 1024;
/// Maximum exact JSON bytes accepted for one promotion-scoped catalog-revalidation receipt.
pub const KAGEMUSHA_V4_CATALOG_REVALIDATION_RECEIPT_MAX_BYTES: usize = 256 * 1024;
/// Maximum lifetime of one signed validator-qualification authorization.
pub const KAGEMUSHA_V4_VALIDATOR_QUALIFICATION_MAX_LIFETIME_MS: u64 = 5 * 60 * 1_000;
/// Maximum tolerated future skew for a catalog-revalidation receipt issuer.
pub const KAGEMUSHA_V4_CATALOG_REVALIDATION_MAX_CLOCK_SKEW_MS: u64 = 30 * 1_000;
/// Maximum canonical Norito bytes accepted for one signed activation-expectations artifact.
pub const KAGEMUSHA_V4_ACTIVATION_EXPECTATIONS_MAX_BYTES: usize = 64 * 1024 * 1024;
/// Maximum UTF-8 bytes accepted for the GitHub `owner/repository` provenance field.
pub const KAGEMUSHA_V4_GITHUB_REPOSITORY_MAX_BYTES: usize = 255;
/// Maximum UTF-8 bytes accepted for the immutable GitHub workflow reference.
pub const KAGEMUSHA_V4_GITHUB_WORKFLOW_REF_MAX_BYTES: usize = 1024;
/// Maximum raw JSON request bytes accepted by the bounded receipt decoder.
///
/// This is an ingress ceiling, not a peak-memory guarantee. The embedded block
/// parser independently rejects a base64 token whose decoded form exceeds the
/// 32 MiB authenticated-block limit before allocating decoded storage.
pub const KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_JSON_BYTES: usize = 96 * 1024 * 1024;
/// Schema id of a validator qualification body.
pub const KAGEMUSHA_V4_VALIDATOR_QUALIFICATION_SEAL_BODY_SCHEMA: &str =
    "iroha.kagemusha.v4.validator_qualification_seal_body.v1";
/// Schema id of a signed validator qualification seal.
pub const KAGEMUSHA_V4_VALIDATOR_QUALIFICATION_SEAL_SCHEMA: &str =
    "iroha.kagemusha.v4.validator_qualification_seal.v1";
/// Schema id of an activation-finality receipt body.
pub const KAGEMUSHA_V4_ACTIVATION_FINALITY_RECEIPT_BODY_SCHEMA: &str =
    "iroha.kagemusha.v4.activation_finality_receipt_body.v1";
/// Schema id of a signed activation-finality receipt.
pub const KAGEMUSHA_V4_ACTIVATION_FINALITY_RECEIPT_SCHEMA: &str =
    "iroha.kagemusha.v4.activation_finality_receipt.v1";
/// Schema id of a root-custodied promotion reservation body.
pub const KAGEMUSHA_V4_PROMOTION_RESERVATION_BODY_SCHEMA: &str =
    "iroha.kagemusha.v4.promotion_reservation_body.v1";
/// Schema id of a signed root-custodied promotion reservation.
pub const KAGEMUSHA_V4_PROMOTION_RESERVATION_SCHEMA: &str =
    "iroha.kagemusha.v4.promotion_reservation.v1";
/// Schema id of a root-custodied activation-expectations body.
pub const KAGEMUSHA_V4_ACTIVATION_EXPECTATIONS_BODY_SCHEMA: &str =
    "iroha.kagemusha.v4.activation_receipt_expectations_body.v1";
/// Schema id of a signed root-custodied activation-expectations artifact.
pub const KAGEMUSHA_V4_ACTIVATION_EXPECTATIONS_SCHEMA: &str =
    "iroha.kagemusha.v4.activation_receipt_expectations.v1";
/// Current validator qualification and activation receipt version.
pub const KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION: u16 = 1;
/// Domain separator for validator qualification signatures.
pub const KAGEMUSHA_V4_VALIDATOR_QUALIFICATION_SIGNATURE_DOMAIN: &[u8] =
    b"iroha:kagemusha:v4:validator-qualification-seal:v1\0";
/// Domain separator for durable activation-finality receipt signatures.
pub const KAGEMUSHA_V4_ACTIVATION_FINALITY_SIGNATURE_DOMAIN: &[u8] =
    b"iroha:kagemusha:v4:activation-finality-receipt:v1\0";
/// Domain separator for root-controller promotion-reservation signatures.
pub const KAGEMUSHA_V4_PROMOTION_RESERVATION_SIGNATURE_DOMAIN: &[u8] =
    b"iroha:kagemusha:v4:promotion-reservation:v1\0";
/// Domain separator for root-controller activation-expectations signatures.
pub const KAGEMUSHA_V4_ACTIVATION_EXPECTATIONS_SIGNATURE_DOMAIN: &[u8] =
    b"iroha:kagemusha:v4:activation-receipt-expectations:v1\0";
/// Domain tag used to derive a promotion id from immutable GitHub run provenance.
pub const KAGEMUSHA_V4_GITHUB_PROMOTION_RUN_ID_DOMAIN: &[u8] =
    b"iroha.kagemusha.github-promotion-run.v1";

/// Exact byte identity used for an externally held immutable input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaExactBytesDigestV1 {
    /// Exact byte length.
    pub byte_len: u64,
    /// SHA-256 of the exact bytes.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub sha256: [u8; 32],
}

impl KagemushaExactBytesDigestV1 {
    /// Derive the exact byte identity of a non-empty input.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] when the input is
    /// empty or its length does not fit the wire field.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, KagemushaPromotionReceiptValidationError> {
        if bytes.is_empty() {
            return Err(KagemushaPromotionReceiptValidationError::InvalidField(
                "exact_bytes.byte_len",
            ));
        }
        let byte_len = u64::try_from(bytes.len()).map_err(|_| {
            KagemushaPromotionReceiptValidationError::InvalidField("exact_bytes.byte_len")
        })?;
        Ok(Self {
            byte_len,
            sha256: Sha256::digest(bytes).into(),
        })
    }

    /// Validate the non-empty, non-zero identity shape.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] for a zero length
    /// or all-zero digest.
    pub fn validate(&self) -> Result<(), KagemushaPromotionReceiptValidationError> {
        if self.byte_len == 0 || self.sha256 == [0; 32] {
            return Err(KagemushaPromotionReceiptValidationError::InvalidField(
                "exact_bytes",
            ));
        }
        Ok(())
    }

    /// Return whether this identity describes the supplied exact bytes.
    #[must_use]
    pub fn matches_bytes(&self, bytes: &[u8]) -> bool {
        u64::try_from(bytes.len()) == Ok(self.byte_len)
            && <[u8; 32]>::from(Sha256::digest(bytes)) == self.sha256
    }
}

/// Immutable GitHub Actions run provenance used to reserve one promotion id.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4GitHubPromotionRunV1 {
    /// Exact `owner/repository` spelling reported by GitHub Actions.
    pub repository: String,
    /// Immutable workflow ref including the workflow path and resolved ref.
    pub workflow_ref: String,
    /// Exact 20-byte workflow commit SHA.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub workflow_sha: [u8; 20],
    /// GitHub Actions run id.
    pub run_id: u64,
    /// Positive GitHub Actions run attempt.
    pub run_attempt: u32,
}

impl KagemushaV4GitHubPromotionRunV1 {
    /// Derive the deterministic promotion id from the exact provenance tuple.
    ///
    /// Each textual component, including lowercase hexadecimal and decimal
    /// renderings of numeric fields, is terminated by one NUL byte. Structure
    /// validation rejects embedded NUL bytes, making this encoding injective.
    #[must_use]
    pub fn promotion_id(&self) -> [u8; 32] {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut workflow_sha_hex = [0_u8; 40];
        for (index, byte) in self.workflow_sha.iter().copied().enumerate() {
            workflow_sha_hex[index * 2] = HEX[usize::from(byte >> 4)];
            workflow_sha_hex[index * 2 + 1] = HEX[usize::from(byte & 0x0f)];
        }
        let run_id = self.run_id.to_string();
        let run_attempt = self.run_attempt.to_string();
        let mut hasher = Sha256::new();
        for component in [
            KAGEMUSHA_V4_GITHUB_PROMOTION_RUN_ID_DOMAIN,
            self.repository.as_bytes(),
            self.workflow_ref.as_bytes(),
            workflow_sha_hex.as_slice(),
            run_id.as_bytes(),
            run_attempt.as_bytes(),
        ] {
            hasher.update(component);
            hasher.update([0]);
        }
        hasher.finalize().into()
    }

    fn validate(&self) -> Result<(), KagemushaPromotionReceiptValidationError> {
        if self.repository.is_empty()
            || self.repository.len() > KAGEMUSHA_V4_GITHUB_REPOSITORY_MAX_BYTES
            || self.repository.as_bytes().contains(&0)
            || self.workflow_ref.is_empty()
            || self.workflow_ref.len() > KAGEMUSHA_V4_GITHUB_WORKFLOW_REF_MAX_BYTES
            || self.workflow_ref.as_bytes().contains(&0)
            || self.workflow_sha == [0; 20]
            || self.run_id == 0
            || self.run_attempt == 0
        {
            return Err(KagemushaPromotionReceiptValidationError::InvalidField(
                "promotion_reservation.github_run",
            ));
        }
        Ok(())
    }
}

/// Root-controller statement reserving one promotion id and its source custody.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4PromotionReservationBodyV1 {
    /// Exact reservation-body schema.
    pub schema: String,
    /// Reservation-body version.
    pub version: u16,
    /// Independently pinned root promotion-controller key.
    pub promotion_controller: PublicKey,
    /// Immutable GitHub Actions run provenance.
    pub github_run: KagemushaV4GitHubPromotionRunV1,
    /// Deterministic id derived from `github_run`.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub promotion_id: [u8; 32],
    /// Exact genesis-derived network identity reserved for activation.
    pub network_id: NetworkId,
    /// Exact reviewed source-closure descriptor bytes.
    pub reviewed_source_closure_descriptor: KagemushaExactBytesDigestV1,
    /// SHA-256 of the canonical promoted V4 manifest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub manifest_sha256: [u8; 32],
    /// SHA-256 of the canonical promoted V4 release record.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub release_record_sha256: [u8; 32],
    /// Exact canonical promotion-record Norito bytes.
    pub promotion_record_norito: KagemushaExactBytesDigestV1,
    /// Exact root-custodied release-policy source bytes.
    pub release_policy_source: KagemushaExactBytesDigestV1,
    /// Exact signed-genesis source bytes reserved for the ordinary node corridor.
    pub signed_genesis: KagemushaExactBytesDigestV1,
    /// Exact catalog-revalidation receipt JSON bytes.
    pub catalog_revalidation_receipt_json: KagemushaExactBytesDigestV1,
    /// SHA-256 of the receipt's canonical App-Attest release-binding catalog.
    ///
    /// This is deliberately distinct from `catalog_consensus_policy_digest`:
    /// the former binds physical-device evidence and consumption receipts,
    /// while the latter binds consensus release-policy inputs.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub catalog_revalidation_catalog_sha256: [u8; 32],
    /// Deterministic logical digest of the complete ordered Kagemusha catalog.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub catalog_consensus_policy_digest: [u8; 32],
    /// Aggregate protocol execution-policy identity reserved for finality.
    pub execution_policy_hash: Hash,
    /// Exact governed device-attestation policy approved at reservation time.
    pub device_attestation_policy: OfflineDeviceAttestationPolicy,
    /// Root policy-evaluation time in Unix milliseconds.
    pub policy_evaluation_time_ms: u64,
    /// Last Unix millisecond at which a validator may sign this qualification.
    pub validator_qualification_expires_at_unix_ms: u64,
}

impl KagemushaV4PromotionReservationBodyV1 {
    /// Return the domain-separated typed hash signed by the promotion controller.
    #[must_use]
    pub fn signing_hash(&self) -> HashOf<Self> {
        HashOf::from_untyped_unchecked(Hash::new_from_chunks(&[
            KAGEMUSHA_V4_PROMOTION_RESERVATION_SIGNATURE_DOMAIN,
            &self.encode(),
        ]))
    }

    fn validate_structure(&self) -> Result<(), KagemushaPromotionReceiptValidationError> {
        if self.schema != KAGEMUSHA_V4_PROMOTION_RESERVATION_BODY_SCHEMA
            || self.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
            || !matches!(
                self.promotion_controller.try_algorithm(),
                Ok(Algorithm::Ed25519)
            )
            || self.policy_evaluation_time_ms == 0
            || self.validator_qualification_expires_at_unix_ms <= self.policy_evaluation_time_ms
            || self
                .validator_qualification_expires_at_unix_ms
                .saturating_sub(self.policy_evaluation_time_ms)
                > KAGEMUSHA_V4_VALIDATOR_QUALIFICATION_MAX_LIFETIME_MS
        {
            return Err(KagemushaPromotionReceiptValidationError::InvalidField(
                "promotion_reservation.body",
            ));
        }
        self.github_run.validate()?;
        if self.promotion_id != self.github_run.promotion_id()
            || self.manifest_sha256 == [0; 32]
            || self.release_record_sha256 == [0; 32]
            || self.catalog_revalidation_catalog_sha256 == [0; 32]
            || self.catalog_consensus_policy_digest == [0; 32]
            || self.network_id.as_bytes().iter().all(|byte| *byte == 0)
            || self.execution_policy_hash == Hash::prehashed([0; Hash::LENGTH])
        {
            return Err(KagemushaPromotionReceiptValidationError::PromotionProvenance);
        }
        for digest in [
            self.reviewed_source_closure_descriptor,
            self.promotion_record_norito,
            self.release_policy_source,
            self.signed_genesis,
            self.catalog_revalidation_receipt_json,
        ] {
            digest.validate()?;
        }
        if self.catalog_revalidation_receipt_json.byte_len
            > u64::try_from(KAGEMUSHA_V4_CATALOG_REVALIDATION_RECEIPT_MAX_BYTES)
                .expect("catalog receipt bound fits u64")
        {
            return Err(KagemushaPromotionReceiptValidationError::InvalidField(
                "promotion_reservation.catalog_revalidation_receipt_json",
            ));
        }
        Ok(())
    }
}

/// Signed, root-custodied reservation for one promotion id.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4PromotionReservationV1 {
    /// Exact reservation schema.
    pub schema: String,
    /// Reservation version.
    pub version: u16,
    /// Signed reservation statement.
    pub body: KagemushaV4PromotionReservationBodyV1,
    /// Root promotion-controller signature.
    pub signature: SignatureOf<KagemushaV4PromotionReservationBodyV1>,
}

impl KagemushaV4PromotionReservationV1 {
    /// Sign one structurally valid reservation with its declared controller.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] for malformed
    /// provenance, an unsupported or different signer, or an oversized artifact.
    pub fn try_sign(
        body: KagemushaV4PromotionReservationBodyV1,
        controller: &KeyPair,
    ) -> Result<Self, KagemushaPromotionReceiptValidationError> {
        body.validate_structure()?;
        if controller.public_key() != &body.promotion_controller {
            return Err(KagemushaPromotionReceiptValidationError::SignerMismatch(
                "promotion_reservation",
            ));
        }
        let signature = SignatureOf::try_from_hash(controller.private_key(), body.signing_hash())
            .map_err(|_| {
            KagemushaPromotionReceiptValidationError::InvalidSignature("promotion_reservation")
        })?;
        let reservation = Self {
            schema: KAGEMUSHA_V4_PROMOTION_RESERVATION_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            body,
            signature,
        };
        enforce_canonical_artifact_size(
            &reservation,
            KAGEMUSHA_V4_PROMOTION_RESERVATION_MAX_BYTES,
            ArtifactKind::PromotionReservation,
        )?;
        Ok(reservation)
    }

    /// Decode one exact canonical reservation under explicit resource limits.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] for empty,
    /// oversized, non-canonical, or structurally invalid input.
    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, KagemushaPromotionReceiptValidationError> {
        check_artifact_input_size(
            bytes,
            KAGEMUSHA_V4_PROMOTION_RESERVATION_MAX_BYTES,
            ArtifactKind::PromotionReservation,
        )?;
        let reservation: Self = norito::decode_canonical_with_limits(
            bytes,
            artifact_decode_limits(KAGEMUSHA_V4_PROMOTION_RESERVATION_MAX_BYTES, bytes.len()),
        )
        .map_err(|_| KagemushaPromotionReceiptValidationError::ReservationDecode)?;
        reservation.body.validate_structure()?;
        Ok(reservation)
    }

    /// Verify the schema, pinned controller, provenance, and signature.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] on any mismatch.
    pub fn verify(
        &self,
        pinned_controller: &PublicKey,
    ) -> Result<(), KagemushaPromotionReceiptValidationError> {
        if self.schema != KAGEMUSHA_V4_PROMOTION_RESERVATION_SCHEMA
            || self.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
            || &self.body.promotion_controller != pinned_controller
            || !matches!(pinned_controller.try_algorithm(), Ok(Algorithm::Ed25519))
        {
            return Err(KagemushaPromotionReceiptValidationError::PromotionController);
        }
        self.body.validate_structure()?;
        verify_typed_signature_hash(
            &self.signature,
            pinned_controller,
            self.body.signing_hash(),
            "promotion_reservation",
        )
    }

    /// Decode and verify one exact canonical reservation in one fail-closed step.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] on any decode,
    /// provenance, pinned-key, or signature failure.
    pub fn decode_and_verify_canonical(
        bytes: &[u8],
        pinned_controller: &PublicKey,
    ) -> Result<Self, KagemushaPromotionReceiptValidationError> {
        let reservation = Self::decode_canonical(bytes)?;
        reservation.verify(pinned_controller)?;
        Ok(reservation)
    }
}

/// Shared controller, reservation, release, policy, catalog, genesis, and
/// consensus identity.
///
/// Every validator body must carry this value byte-for-byte. Host-local
/// executable, configuration-source, and catalog-seal identities deliberately
/// live outside it because heterogeneous validators need not share those bytes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4PromotionBindingV1 {
    /// Independently pinned root promotion-controller key.
    pub promotion_controller: PublicKey,
    /// Exact canonical bytes of the controller-signed promotion reservation.
    pub promotion_reservation: KagemushaExactBytesDigestV1,
    /// Unique non-zero promotion-run identity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub promotion_id: [u8; 32],
    /// Exact genesis-derived network identity.
    pub network_id: NetworkId,
    /// SHA-256 of the reviewed clean source-closure descriptor.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub reviewed_source_closure_descriptor_sha256: [u8; 32],
    /// SHA-256 of the canonical promoted V4 manifest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub manifest_sha256: [u8; 32],
    /// SHA-256 of the canonical promoted V4 release record.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub release_record_sha256: [u8; 32],
    /// Exact root-custodied release-policy source bytes.
    pub release_policy_source: KagemushaExactBytesDigestV1,
    /// Exact canonical governed device-attestation policy bytes.
    pub device_attestation_policy_norito: KagemushaExactBytesDigestV1,
    /// Exact signed-genesis source bytes for an ordinary genesis-rooted node.
    pub signed_genesis: KagemushaExactBytesDigestV1,
    /// Deterministic logical digest of the complete ordered Kagemusha catalog.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub catalog_consensus_policy_digest: [u8; 32],
    /// Aggregate protocol execution-policy identity authenticated by finality.
    pub execution_policy_hash: Hash,
}

impl KagemushaV4PromotionBindingV1 {
    /// Validate the complete promotion and consensus identity.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] when a controller,
    /// digest, network, or execution-policy identity is malformed.
    pub fn validate(&self) -> Result<(), KagemushaPromotionReceiptValidationError> {
        let required_digests = [
            self.promotion_id,
            self.reviewed_source_closure_descriptor_sha256,
            self.manifest_sha256,
            self.release_record_sha256,
            self.catalog_consensus_policy_digest,
        ];
        if !matches!(
            self.promotion_controller.try_algorithm(),
            Ok(Algorithm::Ed25519)
        ) || required_digests.contains(&[0; 32])
            || self.network_id.as_bytes().iter().all(|byte| *byte == 0)
            || self.execution_policy_hash == Hash::prehashed([0; Hash::LENGTH])
        {
            return Err(KagemushaPromotionReceiptValidationError::InvalidField(
                "promotion_binding",
            ));
        }
        self.promotion_reservation.validate()?;
        self.release_policy_source.validate()?;
        self.device_attestation_policy_norito.validate()?;
        self.signed_genesis.validate()?;
        Ok(())
    }
}

/// One validator's exact signed qualification statement.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4ValidatorQualificationSealBodyV1 {
    /// Exact body schema.
    pub schema: String,
    /// Body version.
    pub version: u16,
    /// Shared promotion and consensus identity.
    pub binding: KagemushaV4PromotionBindingV1,
    /// Validator identity and signing key.
    pub validator_id: PeerId,
    /// Exact validator-local `iroha3d` executable bytes.
    pub iroha3d_executable: KagemushaExactBytesDigestV1,
    /// Exact flattened TOML source bytes read on this validator for source audit.
    pub flattened_toml_config_source: KagemushaExactBytesDigestV1,
    /// Secret-free protocol-effective config derived after every startup overlay.
    pub runtime_effective_config: KagemushaV4RuntimeEffectiveConfigProjectionV1,
    /// Exact host-local canonical catalog qualification-seal bytes.
    pub catalog_qualification_seal: KagemushaExactBytesDigestV1,
}

impl KagemushaV4ValidatorQualificationSealBodyV1 {
    /// Return the domain-separated typed hash signed by the validator.
    #[must_use]
    pub fn signing_hash(&self) -> HashOf<Self> {
        HashOf::from_untyped_unchecked(Hash::new_from_chunks(&[
            KAGEMUSHA_V4_VALIDATOR_QUALIFICATION_SIGNATURE_DOMAIN,
            &self.encode(),
        ]))
    }

    /// Validate non-cryptographic body structure.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] when a schema,
    /// shared binding, validator algorithm, or exact-byte identity is invalid.
    pub fn validate_structure(&self) -> Result<(), KagemushaPromotionReceiptValidationError> {
        if self.schema != KAGEMUSHA_V4_VALIDATOR_QUALIFICATION_SEAL_BODY_SCHEMA
            || self.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
            || !matches!(
                self.validator_id.public_key().try_algorithm(),
                Ok(Algorithm::BlsNormal)
            )
        {
            return Err(KagemushaPromotionReceiptValidationError::InvalidField(
                "validator_qualification.body",
            ));
        }
        self.binding.validate()?;
        self.iroha3d_executable.validate()?;
        self.flattened_toml_config_source.validate()?;
        self.runtime_effective_config.validate()?;
        self.catalog_qualification_seal.validate()?;
        if Hash::prehashed(
            self.runtime_effective_config
                .genesis_context
                .execution_policy_hash,
        ) != self.binding.execution_policy_hash
            || !self
                .runtime_effective_config
                .validators
                .iter()
                .any(|validator| validator.validator_id == self.validator_id)
        {
            return Err(KagemushaPromotionReceiptValidationError::InvalidField(
                "validator_qualification.runtime_effective_config",
            ));
        }
        Ok(())
    }
}

/// Signed qualification seal emitted by one validator host.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4ValidatorQualificationSealV1 {
    /// Exact seal schema.
    pub schema: String,
    /// Seal version.
    pub version: u16,
    /// Signed host statement.
    pub body: KagemushaV4ValidatorQualificationSealBodyV1,
    /// Validator signature over `body.signing_hash()`.
    pub signature: SignatureOf<KagemushaV4ValidatorQualificationSealBodyV1>,
}

impl KagemushaV4ValidatorQualificationSealV1 {
    /// Sign a structurally valid body with its validator key.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] when the body is
    /// invalid, the key does not match `validator_id`, or signing fails.
    pub fn try_sign(
        body: KagemushaV4ValidatorQualificationSealBodyV1,
        key_pair: &KeyPair,
    ) -> Result<Self, KagemushaPromotionReceiptValidationError> {
        body.validate_structure()?;
        if key_pair.public_key() != body.validator_id.public_key() {
            return Err(KagemushaPromotionReceiptValidationError::SignerMismatch(
                "validator_qualification",
            ));
        }
        let signature = SignatureOf::try_from_hash(key_pair.private_key(), body.signing_hash())
            .map_err(|_| {
                KagemushaPromotionReceiptValidationError::InvalidSignature(
                    "validator_qualification",
                )
            })?;
        Ok(Self {
            schema: KAGEMUSHA_V4_VALIDATOR_QUALIFICATION_SEAL_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            body,
            signature,
        })
    }

    /// Validate body structure and the validator signature.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] for any malformed
    /// schema, binding, signer, or signature.
    pub fn verify(&self) -> Result<(), KagemushaPromotionReceiptValidationError> {
        if self.schema != KAGEMUSHA_V4_VALIDATOR_QUALIFICATION_SEAL_SCHEMA
            || self.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
        {
            return Err(KagemushaPromotionReceiptValidationError::InvalidField(
                "validator_qualification.seal",
            ));
        }
        self.body.validate_structure()?;
        verify_typed_signature_hash(
            &self.signature,
            self.body.validator_id.public_key(),
            self.body.signing_hash(),
            "validator_qualification",
        )
    }
}

/// Root-controller statement fixing every input accepted before activation submission.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4ActivationReceiptExpectationsBodyV1 {
    /// Exact expectations-body schema.
    pub schema: String,
    /// Expectations-body version.
    pub version: u16,
    /// Independently pinned root promotion-controller key.
    pub promotion_controller: PublicKey,
    /// Exact bytes of the separately signed promotion reservation.
    pub promotion_reservation: KagemushaExactBytesDigestV1,
    /// Exact shared promotion, release, catalog, and consensus binding.
    pub binding: KagemushaV4PromotionBindingV1,
    /// Independently assigned durable-receipt issuer.
    pub receipt_issuer: PublicKey,
    /// Exact strong multisignature governance account.
    pub governance_authority: AccountId,
    /// Exact canonical governance policy duplicated for explicit review.
    pub governance_multisig_policy: MultisigPolicy,
    /// Four exact validator seals in strict `validator_id` order.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_array"))]
    pub validator_seals:
        [KagemushaV4ValidatorQualificationSealV1; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
    /// Complete directly executable, authorization-bearing activation transaction.
    pub activation_transaction: SignedTransaction,
    /// Exact already-finalized proof captured before submission.
    pub trusted_finality_anchor: BridgeFinalityProof,
}

impl KagemushaV4ActivationReceiptExpectationsBodyV1 {
    /// Return the domain-separated typed hash signed by the promotion controller.
    #[must_use]
    pub fn signing_hash(&self) -> HashOf<Self> {
        HashOf::from_untyped_unchecked(Hash::new_from_chunks(&[
            KAGEMUSHA_V4_ACTIVATION_EXPECTATIONS_SIGNATURE_DOMAIN,
            &self.encode(),
        ]))
    }

    fn validator_bodies(
        &self,
    ) -> [KagemushaV4ValidatorQualificationSealBodyV1; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT]
    {
        core::array::from_fn(|index| self.validator_seals[index].body.clone())
    }

    fn validate_shape(&self) -> Result<(), KagemushaPromotionReceiptValidationError> {
        if self.schema != KAGEMUSHA_V4_ACTIVATION_EXPECTATIONS_BODY_SCHEMA
            || self.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
            || !matches!(
                self.promotion_controller.try_algorithm(),
                Ok(Algorithm::Ed25519)
            )
            || !supports_receipt_signature_algorithm(&self.receipt_issuer)
        {
            return Err(KagemushaPromotionReceiptValidationError::InvalidField(
                "activation_expectations.body",
            ));
        }
        self.promotion_reservation.validate()?;
        self.binding.validate()?;
        Ok(())
    }

    fn validate_trust_chain(
        &self,
    ) -> Result<
        (
            [KagemushaV4ValidatorQualificationSealBodyV1; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
            KagemushaExactBytesDigestV1,
        ),
        KagemushaPromotionReceiptValidationError,
    > {
        self.validate_shape()?;
        let authority_policy = validate_governance_multisig_policy(&self.governance_authority)?;
        if authority_policy != &self.governance_multisig_policy {
            return Err(KagemushaPromotionReceiptValidationError::GovernanceAuthority);
        }
        let validator_bodies = self.validator_bodies();
        verify_kagemusha_v4_validator_qualification_seals(
            &self.validator_seals,
            &validator_bodies,
            &self.binding,
        )?;
        validate_receipt_issuer_role_separation(
            &self.receipt_issuer,
            &self.governance_multisig_policy,
            validator_bodies
                .iter()
                .map(|body| body.validator_id.public_key().clone()),
        )?;
        validate_controller_role_separation(
            &self.promotion_controller,
            &self.receipt_issuer,
            &self.governance_multisig_policy,
            validator_bodies
                .iter()
                .map(|body| body.validator_id.public_key().clone()),
        )?;

        self.activation_transaction
            .verify_signature()
            .map_err(|_| KagemushaPromotionReceiptValidationError::ActivationTransaction)?;
        let transaction_policy =
            validate_governance_multisig_policy(self.activation_transaction.authority())?;
        if transaction_policy != &self.governance_multisig_policy
            || self
                .activation_transaction
                .multisig_signatures()
                .is_none_or(|bundle| {
                    bundle.signatures.len() < KAGEMUSHA_V4_ACTIVATION_GOVERNANCE_MIN_SIGNERS
                })
        {
            return Err(KagemushaPromotionReceiptValidationError::GovernanceAuthority);
        }
        if self.activation_transaction.authority() != &self.governance_authority
            || self.activation_transaction.network_id() != Some(&self.binding.network_id)
        {
            return Err(KagemushaPromotionReceiptValidationError::ActivationTransaction);
        }
        let activation = direct_activation_instruction(&self.activation_transaction)?;
        validate_activation_binding(activation, &self.binding)?;
        if activation.runtime_effective_config_sha256()
            != &validator_bodies[0]
                .runtime_effective_config
                .consensus_sha256()?
        {
            return Err(KagemushaPromotionReceiptValidationError::ActivationPayload);
        }

        verify_bridge_finality_proof(&self.trusted_finality_anchor, &self.binding.network_id)
            .map_err(|_| KagemushaPromotionReceiptValidationError::Finality)?;
        validate_finality_corridor_context(
            &self.trusted_finality_anchor,
            &self.binding,
            &validator_bodies,
        )?;
        let anchor_height = self.trusted_finality_anchor.finality_artifact.height;
        let expiry = self
            .activation_transaction
            .expires_at_height()
            .map_err(|_| KagemushaPromotionReceiptValidationError::ActivationExpiry)?
            .ok_or(KagemushaPromotionReceiptValidationError::ActivationExpiry)?;
        let maximum_expiry = anchor_height
            .checked_add(
                u64::try_from(KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1)
                    .expect("proof-count bound fits u64"),
            )
            .and_then(|height| height.checked_add(1))
            .ok_or(KagemushaPromotionReceiptValidationError::ActivationExpiry)?;
        if expiry <= anchor_height || expiry > maximum_expiry {
            return Err(KagemushaPromotionReceiptValidationError::ActivationExpiry);
        }
        let transaction_wire = self
            .activation_transaction
            .encode_wire_v1()
            .map_err(|_| KagemushaPromotionReceiptValidationError::ActivationTransaction)?;
        let transaction_wire = KagemushaExactBytesDigestV1::from_bytes(&transaction_wire)?;
        Ok((validator_bodies, transaction_wire))
    }

    fn validate_reservation_binding(
        &self,
        reservation: &KagemushaV4PromotionReservationV1,
    ) -> Result<(), KagemushaPromotionReceiptValidationError> {
        let reserved = &reservation.body;
        let device_policy = norito::encode_canonical(&reserved.device_attestation_policy)
            .map_err(|_| KagemushaPromotionReceiptValidationError::ExpectationsEncode)?;
        if reserved.promotion_controller != self.promotion_controller
            || self.binding.promotion_controller != self.promotion_controller
            || self.binding.promotion_reservation != self.promotion_reservation
            || reserved.promotion_id != self.binding.promotion_id
            || reserved.network_id != self.binding.network_id
            || reserved.reviewed_source_closure_descriptor.sha256
                != self.binding.reviewed_source_closure_descriptor_sha256
            || reserved.manifest_sha256 != self.binding.manifest_sha256
            || reserved.release_record_sha256 != self.binding.release_record_sha256
            || reserved.release_policy_source != self.binding.release_policy_source
            || reserved.signed_genesis != self.binding.signed_genesis
            || reserved.catalog_consensus_policy_digest
                != self.binding.catalog_consensus_policy_digest
            || reserved.execution_policy_hash != self.binding.execution_policy_hash
            || !self
                .binding
                .device_attestation_policy_norito
                .matches_bytes(&device_policy)
        {
            return Err(KagemushaPromotionReceiptValidationError::PromotionProvenance);
        }
        Ok(())
    }
}

/// Signed, root-custodied artifact that mints receipt-verification capability.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4ActivationReceiptExpectationsArtifactV1 {
    /// Exact expectations-artifact schema.
    pub schema: String,
    /// Expectations-artifact version.
    pub version: u16,
    /// Signed pre-submission statement.
    pub body: KagemushaV4ActivationReceiptExpectationsBodyV1,
    /// Root promotion-controller signature.
    pub signature: SignatureOf<KagemushaV4ActivationReceiptExpectationsBodyV1>,
}

impl KagemushaV4ActivationReceiptExpectationsArtifactV1 {
    /// Sign an intrinsically valid expectations body with its declared controller.
    ///
    /// Reservation cross-binding is intentionally checked by
    /// [`Self::verify_exact`], which receives the exact reservation bytes.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] for an invalid
    /// trust chain, different signer, or oversized artifact.
    pub fn try_sign(
        body: KagemushaV4ActivationReceiptExpectationsBodyV1,
        controller: &KeyPair,
    ) -> Result<Self, KagemushaPromotionReceiptValidationError> {
        body.validate_trust_chain()?;
        if controller.public_key() != &body.promotion_controller {
            return Err(KagemushaPromotionReceiptValidationError::SignerMismatch(
                "activation_expectations",
            ));
        }
        let signature = SignatureOf::try_from_hash(controller.private_key(), body.signing_hash())
            .map_err(|_| {
            KagemushaPromotionReceiptValidationError::InvalidSignature("activation_expectations")
        })?;
        let artifact = Self {
            schema: KAGEMUSHA_V4_ACTIVATION_EXPECTATIONS_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            body,
            signature,
        };
        enforce_canonical_artifact_size(
            &artifact,
            KAGEMUSHA_V4_ACTIVATION_EXPECTATIONS_MAX_BYTES,
            ArtifactKind::ActivationExpectations,
        )?;
        Ok(artifact)
    }

    /// Decode one exact canonical expectations artifact under explicit limits.
    ///
    /// This does not mint a receipt-verification capability. Call
    /// [`Self::verify_exact`] or [`Self::decode_and_verify_canonical`] with the
    /// pinned controller and exact reservation bytes.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] for empty,
    /// oversized, non-canonical, or structurally invalid input.
    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, KagemushaPromotionReceiptValidationError> {
        check_artifact_input_size(
            bytes,
            KAGEMUSHA_V4_ACTIVATION_EXPECTATIONS_MAX_BYTES,
            ArtifactKind::ActivationExpectations,
        )?;
        let artifact: Self = norito::decode_canonical_with_limits(
            bytes,
            artifact_decode_limits(KAGEMUSHA_V4_ACTIVATION_EXPECTATIONS_MAX_BYTES, bytes.len()),
        )
        .map_err(|_| KagemushaPromotionReceiptValidationError::ExpectationsDecode)?;
        artifact.body.validate_shape()?;
        Ok(artifact)
    }

    /// Authenticate exact artifact and reservation bytes and mint a private capability.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] unless both inputs
    /// are exact canonical artifacts, both root signatures authenticate the
    /// pinned controller, and every seal, transaction, TTL, anchor, corridor,
    /// provenance, digest, and role-separation check succeeds.
    pub fn verify_exact(
        &self,
        exact_artifact_bytes: &[u8],
        pinned_controller: &PublicKey,
        exact_reservation_bytes: &[u8],
    ) -> Result<KagemushaV4ActivationReceiptExpectationsV1, KagemushaPromotionReceiptValidationError>
    {
        check_artifact_input_size(
            exact_artifact_bytes,
            KAGEMUSHA_V4_ACTIVATION_EXPECTATIONS_MAX_BYTES,
            ArtifactKind::ActivationExpectations,
        )?;
        let canonical = norito::encode_canonical(self)
            .map_err(|_| KagemushaPromotionReceiptValidationError::ExpectationsEncode)?;
        if canonical.as_slice() != exact_artifact_bytes {
            return Err(KagemushaPromotionReceiptValidationError::ExpectationsDigest);
        }
        if self.schema != KAGEMUSHA_V4_ACTIVATION_EXPECTATIONS_SCHEMA
            || self.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
            || &self.body.promotion_controller != pinned_controller
            || !matches!(pinned_controller.try_algorithm(), Ok(Algorithm::Ed25519))
        {
            return Err(KagemushaPromotionReceiptValidationError::PromotionController);
        }
        self.body.validate_shape()?;
        verify_typed_signature_hash(
            &self.signature,
            pinned_controller,
            self.body.signing_hash(),
            "activation_expectations",
        )?;

        let reservation = KagemushaV4PromotionReservationV1::decode_and_verify_canonical(
            exact_reservation_bytes,
            pinned_controller,
        )?;
        if !self
            .body
            .promotion_reservation
            .matches_bytes(exact_reservation_bytes)
        {
            return Err(KagemushaPromotionReceiptValidationError::ReservationDigest);
        }
        self.body.validate_reservation_binding(&reservation)?;
        let (validator_bodies, activation_transaction_wire) = self.body.validate_trust_chain()?;

        Ok(KagemushaV4ActivationReceiptExpectationsV1 {
            promotion_controller: self.body.promotion_controller.clone(),
            promotion_reservation: self.body.promotion_reservation,
            activation_expectations_artifact: KagemushaExactBytesDigestV1::from_bytes(
                exact_artifact_bytes,
            )?,
            binding: self.body.binding.clone(),
            receipt_issuer: self.body.receipt_issuer.clone(),
            governance_authority: self.body.governance_authority.clone(),
            governance_multisig_policy: self.body.governance_multisig_policy.clone(),
            validator_seals: self.body.validator_seals.clone(),
            validator_bodies,
            activation_transaction_intent: self.body.activation_transaction.hash(),
            activation_transaction_wire,
            trusted_finality_anchor: self.body.trusted_finality_anchor.clone(),
        })
    }

    /// Decode, authenticate, and mint a receipt-verification capability.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] on any canonical
    /// decode or trust-chain verification failure.
    pub fn decode_and_verify_canonical(
        exact_artifact_bytes: &[u8],
        pinned_controller: &PublicKey,
        exact_reservation_bytes: &[u8],
    ) -> Result<KagemushaV4ActivationReceiptExpectationsV1, KagemushaPromotionReceiptValidationError>
    {
        let artifact = Self::decode_canonical(exact_artifact_bytes)?;
        artifact.verify_exact(
            exact_artifact_bytes,
            pinned_controller,
            exact_reservation_bytes,
        )
    }
}

/// Bounded exact `SignedBlockWire` bytes retained by a durable receipt.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[norito(reuse_archived)]
#[repr(transparent)]
pub struct KagemushaFinalizedBlockWireV1(Vec<u8>);

impl fmt::Debug for KagemushaFinalizedBlockWireV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KagemushaFinalizedBlockWireV1")
            .field("byte_len", &self.0.len())
            .finish()
    }
}

impl KagemushaFinalizedBlockWireV1 {
    /// Construct a non-empty block wire within the 32 MiB authenticated limit.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] when the bytes are
    /// empty or exceed the public authenticated-block ceiling.
    pub fn try_from_bytes(
        bytes: Vec<u8>,
    ) -> Result<Self, KagemushaPromotionReceiptValidationError> {
        if bytes.is_empty() || bytes.len() > AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1 {
            return Err(KagemushaPromotionReceiptValidationError::BlockWireSize {
                actual: bytes.len(),
                maximum: AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1,
            });
        }
        Ok(Self(bytes))
    }

    /// Borrow the exact framed block bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Return the exact byte length.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Return whether the wrapper contains no bytes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl TryFrom<Vec<u8>> for KagemushaFinalizedBlockWireV1 {
    type Error = KagemushaPromotionReceiptValidationError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        Self::try_from_bytes(value)
    }
}

/// Intrinsically bounded contiguous finality successors retained by one receipt.
///
/// The custom Norito decoder inspects the sequence header before decoding the
/// underlying vector, so a hostile count above the first-release limit cannot
/// trigger an oversized proof-vector allocation.
#[derive(Debug, Clone, PartialEq, Eq, IntoSchema)]
#[norito(reuse_archived)]
#[repr(transparent)]
pub struct KagemushaV4ActivationFinalityProofChainV1(Vec<BridgeFinalityProof>);

impl KagemushaV4ActivationFinalityProofChainV1 {
    fn validate_proofs(
        proofs: &[BridgeFinalityProof],
    ) -> Result<(), KagemushaPromotionReceiptValidationError> {
        if proofs.is_empty()
            || proofs.len() > KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1
            || proofs.windows(2).any(|pair| {
                pair[0].finality_artifact.height.checked_add(1)
                    != Some(pair[1].finality_artifact.height)
            })
        {
            return Err(KagemushaPromotionReceiptValidationError::FinalityChain);
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), KagemushaPromotionReceiptValidationError> {
        Self::validate_proofs(&self.0)
    }

    /// Borrow the ordered immediate-successor proofs.
    #[must_use]
    pub fn as_slice(&self) -> &[BridgeFinalityProof] {
        &self.0
    }

    /// Iterate over the ordered immediate-successor proofs.
    pub fn iter(&self) -> std::slice::Iter<'_, BridgeFinalityProof> {
        self.0.iter()
    }

    /// Return the number of retained successor proofs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Return whether no successor proof is retained.
    ///
    /// Valid constructed and decoded values always return `false`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Return the first proof in the contiguous successor chain.
    #[must_use]
    pub fn first(&self) -> Option<&BridgeFinalityProof> {
        self.0.first()
    }

    /// Return the final proof in the contiguous successor chain.
    #[must_use]
    pub fn last(&self) -> Option<&BridgeFinalityProof> {
        self.0.last()
    }

    /// Consume the wrapper and return its ordered successor proofs.
    #[must_use]
    pub fn into_vec(self) -> Vec<BridgeFinalityProof> {
        self.0
    }

    /// Construct an invalid shape solely for fail-closed structural regression tests.
    #[cfg(test)]
    pub(crate) fn from_proofs_unchecked(proofs: Vec<BridgeFinalityProof>) -> Self {
        Self(proofs)
    }
}

impl TryFrom<Vec<BridgeFinalityProof>> for KagemushaV4ActivationFinalityProofChainV1 {
    type Error = KagemushaPromotionReceiptValidationError;

    fn try_from(proofs: Vec<BridgeFinalityProof>) -> Result<Self, Self::Error> {
        Self::validate_proofs(&proofs)?;
        Ok(Self(proofs))
    }
}

impl<'a> IntoIterator for &'a KagemushaV4ActivationFinalityProofChainV1 {
    type Item = &'a BridgeFinalityProof;
    type IntoIter = std::slice::Iter<'a, BridgeFinalityProof>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl norito::NoritoSerialize for KagemushaV4ActivationFinalityProofChainV1 {
    fn serialize(&self, writer: &mut ncore::Encoder<'_>) -> Result<(), ncore::Error> {
        ncore::NoritoSerialize::serialize(&self.0, writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        ncore::NoritoSerialize::encoded_len_hint(&self.0)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        ncore::NoritoSerialize::encoded_len_exact(&self.0)
    }
}

impl<'de> norito::NoritoDeserialize<'de> for KagemushaV4ActivationFinalityProofChainV1 {
    fn deserialize(archived: &'de ncore::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("Kagemusha finality proof chain requires a canonical bounded archive")
    }

    fn try_deserialize(archived: &'de ncore::Archived<Self>) -> Result<Self, ncore::Error> {
        let pointer = core::ptr::from_ref(archived).cast::<u8>();
        let bytes = ncore::payload_slice_from_ptr(pointer)?;
        let (chain, used) = <Self as ncore::DecodeFromSlice>::decode_from_slice(bytes)?;
        if used != bytes.len() {
            return Err(ncore::Error::LengthMismatch);
        }
        Ok(chain)
    }
}

impl<'a> ncore::DecodeFromSlice<'a> for KagemushaV4ActivationFinalityProofChainV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
        let (proof_count, _) = ncore::inspect_seq_len_slice(bytes)?;
        if proof_count == 0 || proof_count > KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1 {
            return Err(ncore::Error::Message(format!(
                "Kagemusha finality proof count {proof_count} is outside 1..={KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1}"
            )));
        }
        let (proofs, used) =
            <Vec<BridgeFinalityProof> as ncore::DecodeFromSlice>::decode_from_slice(bytes)?;
        let chain =
            Self::try_from(proofs).map_err(|error| ncore::Error::Message(error.to_string()))?;
        Ok((chain, used))
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for KagemushaV4ActivationFinalityProofChainV1 {
    fn write_json(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(&self.0, out);
    }

    fn write_json_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        norito::json::JsonSerialize::json_serialize_to(&self.0, out)
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for KagemushaV4ActivationFinalityProofChainV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let proof_count = parser.preflight_array_entries()?;
        if proof_count == 0 || proof_count > KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1 {
            return Err(norito::json::Error::InvalidField {
                field: "finality_proof_chain".to_owned(),
                message: format!(
                    "proof count {proof_count} is outside 1..={KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1}"
                ),
            });
        }
        let mut sequence = norito::json::SeqVisitor::new(parser)?;
        let mut proofs = Vec::new();
        proofs.try_reserve_exact(proof_count).map_err(|_| {
            norito::json::Error::Message("finality proof chain allocation failed".to_owned())
        })?;
        while let Some(proof) = sequence.next_element::<BridgeFinalityProof>()? {
            proofs.push(proof);
        }
        sequence.finish()?;
        Self::try_from(proofs).map_err(|error| norito::json::Error::InvalidField {
            field: "finality_proof_chain".to_owned(),
            message: error.to_string(),
        })
    }

    fn json_from_value(value: &norito::json::Value) -> Result<Self, norito::json::Error> {
        let items = value
            .as_array()
            .ok_or_else(|| norito::json::Error::InvalidField {
                field: "finality_proof_chain".to_owned(),
                message: "expected an array".to_owned(),
            })?;
        if items.is_empty() || items.len() > KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1 {
            return Err(norito::json::Error::InvalidField {
                field: "finality_proof_chain".to_owned(),
                message: format!(
                    "proof count {} is outside 1..={KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1}",
                    items.len()
                ),
            });
        }
        let mut proofs = Vec::new();
        proofs.try_reserve_exact(items.len()).map_err(|_| {
            norito::json::Error::Message("finality proof chain allocation failed".to_owned())
        })?;
        for item in items {
            proofs.push(
                <BridgeFinalityProof as norito::json::JsonDeserialize>::json_from_value(item)?,
            );
        }
        Self::try_from(proofs).map_err(|error| norito::json::Error::InvalidField {
            field: "finality_proof_chain".to_owned(),
            message: error.to_string(),
        })
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for KagemushaFinalizedBlockWireV1 {
    fn write_json(&self, out: &mut String) {
        norito::json::write_base64_json(&self.0, out);
    }

    fn write_json_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        norito::json::write_base64_json_to(&self.0, out)
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for KagemushaFinalizedBlockWireV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let (encoded, decoded_len) =
            bounded_base64_token(parser, AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1)?;
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|error| norito::json::Error::Message(error.to_string()))?;
        if bytes.len() != decoded_len {
            return Err(receipt_json_error(
                "base64 decoder length disagrees with bounded preflight",
            ));
        }
        Self::try_from_bytes(bytes).map_err(|error| receipt_json_error(&error.to_string()))
    }

    fn json_from_value(value: &norito::json::Value) -> Result<Self, norito::json::Error> {
        let encoded = value
            .as_str()
            .ok_or_else(|| receipt_json_error("expected canonical base64 string"))?;
        let decoded_len = canonical_base64_decoded_len(
            encoded,
            AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1,
        )?;
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|error| norito::json::Error::Message(error.to_string()))?;
        if bytes.len() != decoded_len {
            return Err(receipt_json_error(
                "base64 decoder length disagrees with bounded preflight",
            ));
        }
        Self::try_from_bytes(bytes).map_err(|error| receipt_json_error(&error.to_string()))
    }
}

/// Complete signed receipt body for one finalized governed activation.
#[derive(Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4ActivationFinalityReceiptBodyV1 {
    /// Exact receipt-body schema.
    pub schema: String,
    /// Receipt-body version.
    pub version: u16,
    /// Exact signed promotion-reservation bytes authenticated before submission.
    pub promotion_reservation: KagemushaExactBytesDigestV1,
    /// Exact signed activation-expectations bytes authenticated before submission.
    pub activation_expectations_artifact: KagemushaExactBytesDigestV1,
    /// Shared promotion, release, catalog, and consensus binding.
    pub binding: KagemushaV4PromotionBindingV1,
    /// Public key of the independent durable-receipt issuer.
    pub issuer: PublicKey,
    /// Exact governed account which authorized the activation transaction.
    pub governance_authority: AccountId,
    /// Four validator seals in strict `validator_id` order.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_array"))]
    pub validator_seals:
        [KagemushaV4ValidatorQualificationSealV1; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
    /// Payload-only activation intent identity from `SignedTransaction::hash()`.
    pub activation_transaction_intent: HashOf<SignedTransaction>,
    /// Exact authorization-bearing bytes from `SignedTransaction::encode_wire_v1()`.
    pub activation_transaction_wire: KagemushaExactBytesDigestV1,
    /// Committed entrypoint/result and their inclusion paths.
    pub committed_transaction: CommittedTransaction,
    /// Exact result-bearing canonical framed block bytes.
    pub finalized_block_wire: KagemushaFinalizedBlockWireV1,
    /// Separately retained exact identity of `finalized_block_wire`.
    pub finalized_block_wire_digest: KagemushaExactBytesDigestV1,
    /// Contiguous Sumeragi-v2 successors after the pre-submission trusted anchor.
    pub finality_proof_chain: KagemushaV4ActivationFinalityProofChainV1,
}

impl fmt::Debug for KagemushaV4ActivationFinalityReceiptBodyV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KagemushaV4ActivationFinalityReceiptBodyV1")
            .field("schema", &self.schema)
            .field("version", &self.version)
            .field("promotion_reservation", &self.promotion_reservation)
            .field(
                "activation_expectations_artifact",
                &self.activation_expectations_artifact,
            )
            .field("promotion_id", &self.binding.promotion_id)
            .field("issuer", &self.issuer)
            .field("governance_authority", &self.governance_authority)
            .field(
                "activation_transaction_intent",
                &self.activation_transaction_intent,
            )
            .field("finalized_block_wire_len", &self.finalized_block_wire.len())
            .field(
                "finalized_block_wire_digest",
                &self.finalized_block_wire_digest,
            )
            .field(
                "finalized_height",
                &self
                    .finality_proof_chain
                    .last()
                    .map(|proof| proof.finality_artifact.height),
            )
            .field("finality_proof_count", &self.finality_proof_chain.len())
            .finish_non_exhaustive()
    }
}

impl KagemushaV4ActivationFinalityReceiptBodyV1 {
    /// Return the domain-separated typed hash signed by the receipt issuer.
    #[must_use]
    pub fn signing_hash(&self) -> HashOf<Self> {
        HashOf::from_untyped_unchecked(Hash::new_from_chunks(&[
            KAGEMUSHA_V4_ACTIVATION_FINALITY_SIGNATURE_DOMAIN,
            &self.encode(),
        ]))
    }

    fn validate_structure(&self) -> Result<(), KagemushaPromotionReceiptValidationError> {
        if self.schema != KAGEMUSHA_V4_ACTIVATION_FINALITY_RECEIPT_BODY_SCHEMA
            || self.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
            || self.finalized_block_wire.is_empty()
            || self.finalized_block_wire.len() > AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1
        {
            return Err(KagemushaPromotionReceiptValidationError::InvalidField(
                "activation_receipt.body",
            ));
        }
        if !supports_receipt_signature_algorithm(&self.issuer) {
            return Err(KagemushaPromotionReceiptValidationError::InvalidField(
                "activation_receipt.issuer",
            ));
        }
        let governance_policy = validate_governance_multisig_policy(&self.governance_authority)?;
        validate_receipt_issuer_role_separation(
            &self.issuer,
            governance_policy,
            self.validator_seals
                .iter()
                .map(|seal| seal.body.validator_id.public_key().clone()),
        )?;
        self.finality_proof_chain.validate()?;
        self.promotion_reservation.validate()?;
        self.activation_expectations_artifact.validate()?;
        self.binding.validate()?;
        self.activation_transaction_wire.validate()?;
        self.finalized_block_wire_digest.validate()?;
        if !self
            .finalized_block_wire_digest
            .matches_bytes(self.finalized_block_wire.as_bytes())
        {
            return Err(KagemushaPromotionReceiptValidationError::BlockWireDigest);
        }
        if self
            .activation_transaction_intent
            .as_ref()
            .iter()
            .all(|byte| *byte == 0)
        {
            return Err(KagemushaPromotionReceiptValidationError::InvalidField(
                "activation_receipt.transaction_intent",
            ));
        }
        Ok(())
    }
}

/// Signed durable receipt for one exact finalized activation.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4ActivationFinalityReceiptV1 {
    /// Exact receipt schema.
    pub schema: String,
    /// Receipt version.
    pub version: u16,
    /// Signed durable statement.
    pub body: KagemushaV4ActivationFinalityReceiptBodyV1,
    /// Independent issuer signature over `body.signing_hash()`.
    pub signature: SignatureOf<KagemushaV4ActivationFinalityReceiptBodyV1>,
}

/// Private capability returned only after authenticating both root artifacts.
///
/// The fields are deliberately private and this type has no public constructor
/// or Norito decoder. A receipt verifier can therefore receive it only from
/// [`KagemushaV4ActivationReceiptExpectationsArtifactV1::verify_exact`] (or its
/// decode-and-verify convenience wrapper), never from receipt-controlled data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KagemushaV4ActivationReceiptExpectationsV1 {
    promotion_controller: PublicKey,
    promotion_reservation: KagemushaExactBytesDigestV1,
    activation_expectations_artifact: KagemushaExactBytesDigestV1,
    binding: KagemushaV4PromotionBindingV1,
    receipt_issuer: PublicKey,
    governance_authority: AccountId,
    governance_multisig_policy: MultisigPolicy,
    validator_seals:
        [KagemushaV4ValidatorQualificationSealV1; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
    validator_bodies:
        [KagemushaV4ValidatorQualificationSealBodyV1; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
    activation_transaction_intent: HashOf<SignedTransaction>,
    activation_transaction_wire: KagemushaExactBytesDigestV1,
    trusted_finality_anchor: BridgeFinalityProof,
}

impl KagemushaV4ActivationReceiptExpectationsV1 {
    /// Build an intentionally unauthenticated capability only for negative tests.
    #[cfg(test)]
    pub(crate) fn from_unverified_artifact_for_test(
        body: &KagemushaV4ActivationReceiptExpectationsBodyV1,
        exact_artifact_bytes: &[u8],
    ) -> Result<Self, KagemushaPromotionReceiptValidationError> {
        let transaction_wire = body
            .activation_transaction
            .encode_wire_v1()
            .map_err(|_| KagemushaPromotionReceiptValidationError::ActivationTransaction)?;
        Ok(Self {
            promotion_controller: body.promotion_controller.clone(),
            promotion_reservation: body.promotion_reservation,
            activation_expectations_artifact: KagemushaExactBytesDigestV1::from_bytes(
                exact_artifact_bytes,
            )?,
            binding: body.binding.clone(),
            receipt_issuer: body.receipt_issuer.clone(),
            governance_authority: body.governance_authority.clone(),
            governance_multisig_policy: body.governance_multisig_policy.clone(),
            validator_seals: body.validator_seals.clone(),
            validator_bodies: body.validator_bodies(),
            activation_transaction_intent: body.activation_transaction.hash(),
            activation_transaction_wire: KagemushaExactBytesDigestV1::from_bytes(
                &transaction_wire,
            )?,
            trusted_finality_anchor: body.trusted_finality_anchor.clone(),
        })
    }

    /// Return the authenticated root promotion-controller key.
    #[must_use]
    pub const fn promotion_controller(&self) -> &PublicKey {
        &self.promotion_controller
    }

    /// Return the exact authenticated promotion-reservation identity.
    #[must_use]
    pub const fn promotion_reservation(&self) -> KagemushaExactBytesDigestV1 {
        self.promotion_reservation
    }

    /// Return the exact authenticated expectations-artifact identity.
    #[must_use]
    pub const fn activation_expectations_artifact(&self) -> KagemushaExactBytesDigestV1 {
        self.activation_expectations_artifact
    }

    /// Return the authenticated shared promotion binding.
    #[must_use]
    pub const fn binding(&self) -> &KagemushaV4PromotionBindingV1 {
        &self.binding
    }

    /// Return the authenticated independent receipt issuer.
    #[must_use]
    pub const fn receipt_issuer(&self) -> &PublicKey {
        &self.receipt_issuer
    }

    /// Return the authenticated governance authority.
    #[must_use]
    pub const fn governance_authority(&self) -> &AccountId {
        &self.governance_authority
    }

    /// Return the authenticated exact governance policy.
    #[must_use]
    pub const fn governance_multisig_policy(&self) -> &MultisigPolicy {
        &self.governance_multisig_policy
    }

    /// Return the four authenticated validator seals in strict order.
    #[must_use]
    pub const fn validator_seals(
        &self,
    ) -> &[KagemushaV4ValidatorQualificationSealV1; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT] {
        &self.validator_seals
    }

    /// Return the four authenticated validator bodies in strict order.
    #[must_use]
    pub const fn validator_bodies(
        &self,
    ) -> &[KagemushaV4ValidatorQualificationSealBodyV1; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT]
    {
        &self.validator_bodies
    }

    /// Return the authenticated payload-only activation intent.
    #[must_use]
    pub const fn activation_transaction_intent(&self) -> HashOf<SignedTransaction> {
        self.activation_transaction_intent
    }

    /// Return the authenticated authorization-bearing transaction-wire identity.
    #[must_use]
    pub const fn activation_transaction_wire(&self) -> KagemushaExactBytesDigestV1 {
        self.activation_transaction_wire
    }

    /// Return the authenticated pre-submission finality anchor.
    #[must_use]
    pub const fn trusted_finality_anchor(&self) -> &BridgeFinalityProof {
        &self.trusted_finality_anchor
    }

    fn validate(&self) -> Result<(), KagemushaPromotionReceiptValidationError> {
        self.binding.validate()?;
        self.promotion_reservation.validate()?;
        self.activation_expectations_artifact.validate()?;
        self.activation_transaction_wire.validate()?;
        let authority_policy = validate_governance_multisig_policy(&self.governance_authority)?;
        if authority_policy != &self.governance_multisig_policy {
            return Err(KagemushaPromotionReceiptValidationError::GovernanceAuthority);
        }
        if !supports_receipt_signature_algorithm(&self.receipt_issuer)
            || self
                .activation_transaction_intent
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
        {
            return Err(KagemushaPromotionReceiptValidationError::InvalidField(
                "activation_receipt.expectations",
            ));
        }
        let mut previous = None;
        for body in &self.validator_bodies {
            body.validate_structure()?;
            if body.binding != self.binding
                || previous.is_some_and(|validator: &PeerId| validator >= &body.validator_id)
            {
                return Err(KagemushaPromotionReceiptValidationError::ValidatorSet);
            }
            previous = Some(&body.validator_id);
        }
        validate_receipt_issuer_role_separation(
            &self.receipt_issuer,
            &self.governance_multisig_policy,
            self.validator_bodies
                .iter()
                .map(|body| body.validator_id.public_key().clone()),
        )?;
        validate_controller_role_separation(
            &self.promotion_controller,
            &self.receipt_issuer,
            &self.governance_multisig_policy,
            self.validator_bodies
                .iter()
                .map(|body| body.validator_id.public_key().clone()),
        )?;
        verify_kagemusha_v4_validator_qualification_seals(
            &self.validator_seals,
            &self.validator_bodies,
            &self.binding,
        )?;
        validate_finality_corridor_context(
            &self.trusted_finality_anchor,
            &self.binding,
            &self.validator_bodies,
        )?;
        Ok(())
    }

    /// Test-only mutable access for hostile-anchor regression fixtures.
    #[cfg(test)]
    pub(crate) fn trusted_finality_anchor_mut_for_test(&mut self) -> &mut BridgeFinalityProof {
        &mut self.trusted_finality_anchor
    }

    /// Test-only replacement of the receipt issuer for role-splice fixtures.
    #[cfg(test)]
    pub(crate) fn set_receipt_issuer_for_test(&mut self, issuer: PublicKey) {
        self.receipt_issuer = issuer;
    }

    /// Test-only replacement of governance authority for rejection fixtures.
    #[cfg(test)]
    pub(crate) fn set_governance_authority_for_test(&mut self, authority: AccountId) {
        self.governance_authority = authority;
    }

    /// Test-only replacement of governance policy for rejection fixtures.
    #[cfg(test)]
    pub(crate) fn set_governance_multisig_policy_for_test(&mut self, policy: MultisigPolicy) {
        self.governance_multisig_policy = policy;
    }
}

/// Authenticated receipt result returned only after every binding verifies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KagemushaV4VerifiedActivationReceiptV1 {
    /// Exact finalized block height.
    finalized_height: u64,
    /// Hash of the exact finalized block header.
    finalized_block_hash: HashOf<crate::block::BlockHeader>,
    /// Exact payload-only activation transaction intent.
    activation_transaction_intent: HashOf<SignedTransaction>,
}

impl KagemushaV4VerifiedActivationReceiptV1 {
    /// Return the exact finalized block height authenticated by the receipt.
    #[must_use]
    pub const fn finalized_height(&self) -> u64 {
        self.finalized_height
    }

    /// Return the hash of the exact finalized block header authenticated by the receipt.
    #[must_use]
    pub const fn finalized_block_hash(&self) -> HashOf<crate::block::BlockHeader> {
        self.finalized_block_hash
    }

    /// Return the payload-only activation transaction intent authenticated by the receipt.
    #[must_use]
    pub const fn activation_transaction_intent(&self) -> HashOf<SignedTransaction> {
        self.activation_transaction_intent
    }
}

impl KagemushaV4ActivationFinalityReceiptV1 {
    fn verify_envelope_and_expectations(
        &self,
        expectations: &KagemushaV4ActivationReceiptExpectationsV1,
    ) -> Result<(), KagemushaPromotionReceiptValidationError> {
        if self.schema != KAGEMUSHA_V4_ACTIVATION_FINALITY_RECEIPT_SCHEMA
            || self.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
        {
            return Err(KagemushaPromotionReceiptValidationError::InvalidField(
                "activation_receipt",
            ));
        }
        expectations.validate()?;
        self.body.validate_structure()?;
        if self.body.promotion_reservation != expectations.promotion_reservation
            || self.body.activation_expectations_artifact
                != expectations.activation_expectations_artifact
            || self.body.binding != expectations.binding
            || self.body.issuer != expectations.receipt_issuer
            || self.body.governance_authority != expectations.governance_authority
            || self.body.validator_seals != expectations.validator_seals
            || self.body.activation_transaction_intent != expectations.activation_transaction_intent
            || self.body.activation_transaction_wire != expectations.activation_transaction_wire
        {
            return Err(KagemushaPromotionReceiptValidationError::ExpectationMismatch);
        }
        enforce_activation_receipt_frame_size(self)?;
        verify_typed_signature_hash(
            &self.signature,
            &self.body.issuer,
            self.body.signing_hash(),
            "activation_receipt",
        )?;
        verify_kagemusha_v4_validator_qualification_seals(
            &self.body.validator_seals,
            &expectations.validator_bodies,
            &expectations.binding,
        )?;
        Ok(())
    }

    fn verify_committed_activation(
        &self,
        expectations: &KagemushaV4ActivationReceiptExpectationsV1,
    ) -> Result<(SignedBlock, &SignedTransaction, Vec<u8>), KagemushaPromotionReceiptValidationError>
    {
        let block = decode_exact_finalized_block(self.body.finalized_block_wire.as_bytes())?;
        let committed = &self.body.committed_transaction;
        if committed.merge_inclusion.is_some()
            || !committed.verify_inclusion_in_block(&block)
            || committed.result.0.is_err()
            || block
                .entrypoint_hashes()
                .filter(|entry_hash| entry_hash == &committed.entrypoint_hash)
                .count()
                != 1
        {
            return Err(KagemushaPromotionReceiptValidationError::CommittedTransaction);
        }
        let TransactionEntrypoint::External(transaction) = &committed.entrypoint else {
            return Err(KagemushaPromotionReceiptValidationError::ActivationTransaction);
        };
        transaction
            .verify_signature()
            .map_err(|_| KagemushaPromotionReceiptValidationError::ActivationTransaction)?;
        let transaction_policy = validate_governance_multisig_policy(transaction.authority())?;
        if transaction_policy != &expectations.governance_multisig_policy
            || transaction.multisig_signatures().is_none_or(|bundle| {
                bundle.signatures.len() < KAGEMUSHA_V4_ACTIVATION_GOVERNANCE_MIN_SIGNERS
            })
        {
            return Err(KagemushaPromotionReceiptValidationError::GovernanceAuthority);
        }
        if transaction.authority() != &expectations.governance_authority
            || transaction.network_id() != Some(&expectations.binding.network_id)
            || transaction.hash() != expectations.activation_transaction_intent
        {
            return Err(KagemushaPromotionReceiptValidationError::ActivationTransaction);
        }
        let transaction_wire = transaction
            .encode_wire_v1()
            .map_err(|_| KagemushaPromotionReceiptValidationError::ActivationTransaction)?;
        if !expectations
            .activation_transaction_wire
            .matches_bytes(&transaction_wire)
        {
            return Err(KagemushaPromotionReceiptValidationError::ActivationTransaction);
        }
        let activation = direct_activation_instruction(transaction)?;
        validate_activation_binding(activation, &expectations.binding)?;
        Ok((block, transaction, transaction_wire))
    }

    fn verify_finality_and_authorization(
        &self,
        expectations: &KagemushaV4ActivationReceiptExpectationsV1,
        block: &SignedBlock,
        transaction: &SignedTransaction,
        transaction_wire: &[u8],
    ) -> Result<(), KagemushaPromotionReceiptValidationError> {
        let finality_proof = self
            .body
            .finality_proof_chain
            .last()
            .ok_or(KagemushaPromotionReceiptValidationError::FinalityChain)?;
        let artifact = &finality_proof.finality_artifact;
        let anchor_height = expectations
            .trusted_finality_anchor
            .finality_artifact
            .height;
        let proof_count = u64::try_from(self.body.finality_proof_chain.len())
            .map_err(|_| KagemushaPromotionReceiptValidationError::FinalityChain)?;
        if anchor_height.checked_add(proof_count) != Some(artifact.height) {
            return Err(KagemushaPromotionReceiptValidationError::FinalityChain);
        }
        let expiry = transaction
            .expires_at_height()
            .map_err(|_| KagemushaPromotionReceiptValidationError::ActivationExpiry)?
            .ok_or(KagemushaPromotionReceiptValidationError::ActivationExpiry)?;
        let maximum_expiry = anchor_height
            .checked_add(
                u64::try_from(KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1)
                    .expect("proof-count bound fits u64"),
            )
            .and_then(|height| height.checked_add(1))
            .ok_or(KagemushaPromotionReceiptValidationError::ActivationExpiry)?;
        if artifact.height >= expiry || expiry > maximum_expiry {
            return Err(KagemushaPromotionReceiptValidationError::ActivationExpiry);
        }
        let mut finality_verifier = BridgeFinalityVerifier::with_context(
            expectations.binding.network_id,
            expectations
                .trusted_finality_anchor
                .finality_artifact
                .context_id(),
        );
        finality_verifier
            .verify(&expectations.trusted_finality_anchor)
            .map_err(|_| KagemushaPromotionReceiptValidationError::Finality)?;
        for proof in &self.body.finality_proof_chain {
            validate_finality_corridor_context(
                proof,
                &expectations.binding,
                &expectations.validator_bodies,
            )?;
            finality_verifier
                .verify(proof)
                .map_err(|_| KagemushaPromotionReceiptValidationError::Finality)?;
        }
        let anchor = TrustedBlockProofAnchor::from_untrusted_finality_artifact(
            block,
            artifact,
            &self.body.committed_transaction.entrypoint_hash,
        )
        .map_err(|_| KagemushaPromotionReceiptValidationError::FinalityBlockBinding)?;
        let entry_index = usize::try_from(anchor.entry_index())
            .map_err(|_| KagemushaPromotionReceiptValidationError::FinalityBlockBinding)?;
        let block_entrypoint = block
            .entrypoints_cloned()
            .nth(entry_index)
            .ok_or(KagemushaPromotionReceiptValidationError::FinalityBlockBinding)?;
        let TransactionEntrypoint::External(block_transaction) = block_entrypoint else {
            return Err(KagemushaPromotionReceiptValidationError::ActivationAuthorizationWire);
        };
        let block_transaction_wire = block_transaction
            .encode_wire_v1()
            .map_err(|_| KagemushaPromotionReceiptValidationError::ActivationAuthorizationWire)?;
        if block_transaction_wire != transaction_wire {
            return Err(KagemushaPromotionReceiptValidationError::ActivationAuthorizationWire);
        }
        Ok(())
    }

    /// Sign a structurally valid body with its declared receipt-issuer key.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] when structure,
    /// issuer identity, or signing fails. This constructor does not replace
    /// [`Self::verify`] and does not claim that activation occurred.
    pub fn try_sign(
        body: KagemushaV4ActivationFinalityReceiptBodyV1,
        key_pair: &KeyPair,
    ) -> Result<Self, KagemushaPromotionReceiptValidationError> {
        body.validate_structure()?;
        enforce_activation_receipt_frame_size(&body)?;
        if key_pair.public_key() != &body.issuer {
            return Err(KagemushaPromotionReceiptValidationError::SignerMismatch(
                "activation_receipt",
            ));
        }
        let signature = SignatureOf::try_from_hash(key_pair.private_key(), body.signing_hash())
            .map_err(|_| {
                KagemushaPromotionReceiptValidationError::InvalidSignature("activation_receipt")
            })?;
        let receipt = Self {
            schema: KAGEMUSHA_V4_ACTIVATION_FINALITY_RECEIPT_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            body,
            signature,
        };
        enforce_activation_receipt_frame_size(&receipt)?;
        Ok(receipt)
    }

    /// Decode one exact canonical receipt under explicit cumulative limits.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] before decode when
    /// the outer frame exceeds 64 MiB, or when canonical decoding/resource
    /// limits and embedded block-wire bounds are violated.
    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, KagemushaPromotionReceiptValidationError> {
        if bytes.is_empty() || bytes.len() > KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_BYTES {
            return Err(KagemushaPromotionReceiptValidationError::ReceiptSize {
                actual: bytes.len(),
                maximum: KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_BYTES,
            });
        }
        let limits = receipt_decode_limits(bytes.len());
        let receipt: Self = norito::decode_canonical_with_limits(bytes, limits)
            .map_err(|_| KagemushaPromotionReceiptValidationError::ReceiptDecode)?;
        receipt.body.validate_structure()?;
        Ok(receipt)
    }

    /// Decode a raw JSON receipt only after enforcing the ingress byte cap.
    ///
    /// The embedded block-wire parser also bounds decoded base64 length before
    /// allocation. Callers receiving HTTP bodies must invoke this method on the
    /// already size-limited raw body rather than parse an unbounded JSON value.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] for an oversized
    /// body, non-canonical JSON, oversized block token, or invalid structure.
    #[cfg(feature = "json")]
    pub fn decode_json_bounded(
        json: &str,
    ) -> Result<Self, KagemushaPromotionReceiptValidationError> {
        if json.is_empty() || json.len() > KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_JSON_BYTES {
            return Err(KagemushaPromotionReceiptValidationError::JsonSize {
                actual: json.len(),
                maximum: KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_JSON_BYTES,
            });
        }
        let receipt: Self = norito::json::from_str(json)
            .map_err(|_| KagemushaPromotionReceiptValidationError::ReceiptJsonDecode)?;
        enforce_activation_receipt_frame_size(&receipt)?;
        receipt.body.validate_structure()?;
        Ok(receipt)
    }

    /// Verify every external, signature, transaction, inclusion, and finality binding.
    ///
    /// A fresh local [`BridgeFinalityVerifier`] first verifies the exact
    /// pre-submission anchor and then every immediate successor retained by the
    /// receipt. Failed input can therefore never advance shared verifier state.
    /// The method performs no live write and does not submit or activate anything.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaPromotionReceiptValidationError`] on the first
    /// schema, expectation, seal, transaction, block, or finality mismatch.
    #[allow(
        clippy::too_many_lines,
        reason = "the activation verifier keeps every external identity and finality binding in one auditable path"
    )]
    pub fn verify(
        &self,
        expectations: &KagemushaV4ActivationReceiptExpectationsV1,
    ) -> Result<KagemushaV4VerifiedActivationReceiptV1, KagemushaPromotionReceiptValidationError>
    {
        self.verify_envelope_and_expectations(expectations)?;
        let (block, transaction, transaction_wire) =
            self.verify_committed_activation(expectations)?;
        self.verify_finality_and_authorization(
            expectations,
            &block,
            transaction,
            &transaction_wire,
        )?;

        Ok(KagemushaV4VerifiedActivationReceiptV1 {
            finalized_height: block.header().height().get(),
            finalized_block_hash: block.hash(),
            activation_transaction_intent: transaction.hash(),
        })
    }
}

fn validate_governance_multisig_policy(
    authority: &AccountId,
) -> Result<&MultisigPolicy, KagemushaPromotionReceiptValidationError> {
    let Some(policy) = authority.controller().multisig_policy() else {
        return Err(KagemushaPromotionReceiptValidationError::GovernanceAuthority);
    };
    if !kagemusha_v4_governance_policy_requires_distinct_signers(policy) {
        return Err(KagemushaPromotionReceiptValidationError::GovernanceAuthority);
    }
    Ok(policy)
}

/// Return whether a governance policy necessarily requires the documented distinct-signer floor.
pub(super) fn kagemusha_v4_governance_policy_requires_distinct_signers(
    policy: &MultisigPolicy,
) -> bool {
    usize::from(policy.threshold()) >= KAGEMUSHA_V4_ACTIVATION_GOVERNANCE_MIN_SIGNERS
        && policy.members().len() >= KAGEMUSHA_V4_ACTIVATION_GOVERNANCE_MIN_SIGNERS
        && policy
            .members()
            .iter()
            .all(|member| member.weight() < policy.threshold())
}

fn validate_receipt_issuer_role_separation(
    issuer: &PublicKey,
    governance_policy: &MultisigPolicy,
    validator_keys: impl IntoIterator<Item = PublicKey>,
) -> Result<(), KagemushaPromotionReceiptValidationError> {
    if governance_policy
        .members()
        .iter()
        .any(|member| member.public_key() == issuer)
        || validator_keys
            .into_iter()
            .any(|validator_key| &validator_key == issuer)
    {
        return Err(KagemushaPromotionReceiptValidationError::IssuerRoleOverlap);
    }
    Ok(())
}

fn validate_controller_role_separation(
    controller: &PublicKey,
    receipt_issuer: &PublicKey,
    governance_policy: &MultisigPolicy,
    validator_keys: impl IntoIterator<Item = PublicKey>,
) -> Result<(), KagemushaPromotionReceiptValidationError> {
    if controller == receipt_issuer
        || governance_policy
            .members()
            .iter()
            .any(|member| member.public_key() == controller)
        || validator_keys
            .into_iter()
            .any(|validator_key| &validator_key == controller)
    {
        return Err(KagemushaPromotionReceiptValidationError::ControllerRoleOverlap);
    }
    Ok(())
}

pub(super) fn validate_finality_corridor_context(
    proof: &BridgeFinalityProof,
    binding: &KagemushaV4PromotionBindingV1,
    validator_bodies: &[KagemushaV4ValidatorQualificationSealBodyV1;
         KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
) -> Result<(), KagemushaPromotionReceiptValidationError> {
    let context = &proof.finality_artifact.height_context;
    let runtime = &validator_bodies[0].runtime_effective_config;
    if context.network_id != binding.network_id
        || context.mode != crate::block::consensus_v2::ConsensusMode::Permissioned
        || context.nexus_amx_context_hash
            != Hash::prehashed(runtime.genesis_context.nexus_amx_context_hash)
        || context.execution_policy_hash != binding.execution_policy_hash
        || context.da_layout != runtime.genesis_context.da_layout
        || context.snapshot_bootstrap.is_some()
        || context.roster.len() != KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT
        || proof.finality_artifact.validator_set_pops.len() != runtime.validators.len()
        || context
            .roster
            .iter()
            .zip(validator_bodies)
            .any(|(member, body)| member.power != 1 || member.validator != body.validator_id)
        || proof
            .finality_artifact
            .validator_set_pops
            .iter()
            .zip(&runtime.validators)
            .any(|(actual, expected)| actual != &expected.bls_pop)
    {
        return Err(KagemushaPromotionReceiptValidationError::FinalityRoster);
    }
    Ok(())
}

fn enforce_activation_receipt_frame_size<T: norito::NoritoSerialize>(
    value: &T,
) -> Result<usize, KagemushaPromotionReceiptValidationError> {
    let _canonical_flags =
        norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let actual = norito::core::encoded_frame_len(value)
        .map_err(|_| KagemushaPromotionReceiptValidationError::ReceiptEncode)?;
    if actual > KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_BYTES {
        return Err(KagemushaPromotionReceiptValidationError::ReceiptSize {
            actual,
            maximum: KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_BYTES,
        });
    }
    Ok(actual)
}

/// Verify four exact validator seals against external bodies and one shared binding.
///
/// # Errors
///
/// Returns [`KagemushaPromotionReceiptValidationError`] unless both actual and
/// expected bodies are strictly sorted, distinct, byte-for-byte equal, carry
/// the shared binding, and have valid validator signatures.
pub fn verify_kagemusha_v4_validator_qualification_seals(
    seals: &[KagemushaV4ValidatorQualificationSealV1; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
    expected_bodies: &[KagemushaV4ValidatorQualificationSealBodyV1;
         KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
    binding: &KagemushaV4PromotionBindingV1,
) -> Result<(), KagemushaPromotionReceiptValidationError> {
    binding.validate()?;
    let shared_effective_config = &expected_bodies[0].runtime_effective_config;
    let mut previous = None;
    for (seal, expected) in seals.iter().zip(expected_bodies) {
        expected.validate_structure()?;
        if expected.binding != *binding
            || expected.runtime_effective_config != *shared_effective_config
            || seal.body != *expected
            || previous.is_some_and(|validator: &PeerId| validator >= &seal.body.validator_id)
        {
            return Err(KagemushaPromotionReceiptValidationError::ValidatorSet);
        }
        seal.verify()?;
        previous = Some(&seal.body.validator_id);
    }
    Ok(())
}

fn direct_activation_instruction(
    transaction: &SignedTransaction,
) -> Result<&ActivateKagemushaRecursiveReleaseV4, KagemushaPromotionReceiptValidationError> {
    let Executable::Instructions(instructions) = transaction.instructions() else {
        return Err(KagemushaPromotionReceiptValidationError::ActivationTransaction);
    };
    if instructions.len() != 1 {
        return Err(KagemushaPromotionReceiptValidationError::ActivationTransaction);
    }
    instructions[0]
        .as_any()
        .downcast_ref::<ActivateKagemushaRecursiveReleaseV4>()
        .ok_or(KagemushaPromotionReceiptValidationError::ActivationTransaction)
}

fn validate_activation_binding(
    instruction: &ActivateKagemushaRecursiveReleaseV4,
    binding: &KagemushaV4PromotionBindingV1,
) -> Result<(), KagemushaPromotionReceiptValidationError> {
    instruction
        .validate_promotion_id()
        .map_err(|_| KagemushaPromotionReceiptValidationError::ActivationPayload)?;
    if instruction.promotion_binding() != binding {
        return Err(KagemushaPromotionReceiptValidationError::ActivationPayload);
    }
    let activation = instruction.activation();
    activation
        .validate_structure()
        .map_err(|_| KagemushaPromotionReceiptValidationError::ActivationPayload)?;
    let manifest = &activation.release_record.manifest;
    let manifest_sha256 = manifest
        .canonical_sha256()
        .map_err(|_| KagemushaPromotionReceiptValidationError::ActivationPayload)?;
    let release_record_sha256 = canonical_sha256(&activation.release_record)?;
    let device_policy = norito::encode_canonical(instruction.device_attestation_policy())
        .map_err(|_| KagemushaPromotionReceiptValidationError::ActivationPayload)?;
    if manifest.network_id != binding.network_id
        || manifest.reviewed_source_closure_descriptor_sha256
            != binding.reviewed_source_closure_descriptor_sha256
        || manifest_sha256 != binding.manifest_sha256
        || release_record_sha256 != binding.release_record_sha256
        || activation.configured_policy_sha256 != binding.release_policy_source.sha256
        || !binding
            .device_attestation_policy_norito
            .matches_bytes(&device_policy)
    {
        return Err(KagemushaPromotionReceiptValidationError::ActivationPayload);
    }
    Ok(())
}

fn canonical_sha256<T: norito::NoritoSerialize>(
    value: &T,
) -> Result<[u8; 32], KagemushaPromotionReceiptValidationError> {
    norito::encode_canonical(value)
        .map(|bytes| Sha256::digest(bytes).into())
        .map_err(|_| KagemushaPromotionReceiptValidationError::ReceiptEncode)
}

fn verify_typed_signature_hash<T>(
    signature: &SignatureOf<T>,
    signer: &PublicKey,
    signing_hash: HashOf<T>,
    field: &'static str,
) -> Result<(), KagemushaPromotionReceiptValidationError> {
    match signer.try_algorithm() {
        Ok(Algorithm::Ed25519) => {
            iroha_crypto::ed25519_parse_signature(signature.payload())
                .map_err(|_| KagemushaPromotionReceiptValidationError::InvalidSignature(field))?;
        }
        Ok(Algorithm::MlDsa) => {
            iroha_crypto::mldsa65_parse_signature(signature.payload())
                .map_err(|_| KagemushaPromotionReceiptValidationError::InvalidSignature(field))?;
        }
        Ok(Algorithm::BlsNormal) => {}
        _ => {
            return Err(KagemushaPromotionReceiptValidationError::InvalidSignature(
                field,
            ));
        }
    }
    signature
        .verify_hash(signer, signing_hash)
        .map_err(|_| KagemushaPromotionReceiptValidationError::InvalidSignature(field))
}

fn supports_receipt_signature_algorithm(signer: &PublicKey) -> bool {
    matches!(
        signer.try_algorithm(),
        Ok(Algorithm::Ed25519 | Algorithm::MlDsa | Algorithm::BlsNormal)
    )
}

#[derive(Debug, Clone, Copy)]
enum ArtifactKind {
    PromotionReservation,
    ActivationExpectations,
}

fn check_artifact_input_size(
    bytes: &[u8],
    maximum: usize,
    kind: ArtifactKind,
) -> Result<(), KagemushaPromotionReceiptValidationError> {
    if !bytes.is_empty() && bytes.len() <= maximum {
        return Ok(());
    }
    Err(match kind {
        ArtifactKind::PromotionReservation => {
            KagemushaPromotionReceiptValidationError::ReservationSize {
                actual: bytes.len(),
                maximum,
            }
        }
        ArtifactKind::ActivationExpectations => {
            KagemushaPromotionReceiptValidationError::ExpectationsSize {
                actual: bytes.len(),
                maximum,
            }
        }
    })
}

fn enforce_canonical_artifact_size<T: norito::NoritoSerialize>(
    value: &T,
    maximum: usize,
    kind: ArtifactKind,
) -> Result<usize, KagemushaPromotionReceiptValidationError> {
    let _canonical_flags =
        norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let actual = norito::core::encoded_frame_len(value).map_err(|_| match kind {
        ArtifactKind::PromotionReservation => {
            KagemushaPromotionReceiptValidationError::ReservationEncode
        }
        ArtifactKind::ActivationExpectations => {
            KagemushaPromotionReceiptValidationError::ExpectationsEncode
        }
    })?;
    if actual == 0 || actual > maximum {
        return Err(match kind {
            ArtifactKind::PromotionReservation => {
                KagemushaPromotionReceiptValidationError::ReservationSize { actual, maximum }
            }
            ArtifactKind::ActivationExpectations => {
                KagemushaPromotionReceiptValidationError::ExpectationsSize { actual, maximum }
            }
        });
    }
    Ok(actual)
}

fn artifact_decode_limits(maximum: usize, encoded_len: usize) -> norito::DecodeLimits {
    norito::DecodeLimits::new(
        maximum,
        maximum,
        encoded_len.saturating_mul(8),
        maximum.saturating_mul(8),
        norito::core::MAX_OWNED_VALUE_DECODE_DEPTH,
    )
}

fn receipt_decode_limits(encoded_len: usize) -> norito::DecodeLimits {
    let maximum = KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_BYTES;
    let allocation = maximum.saturating_mul(8);
    norito::DecodeLimits::new(
        AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1,
        maximum,
        encoded_len.saturating_mul(8),
        allocation,
        norito::core::MAX_OWNED_VALUE_DECODE_DEPTH,
    )
}

fn block_decode_limits() -> norito::DecodeLimits {
    let maximum = AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1;
    norito::DecodeLimits::new(
        maximum.saturating_mul(8),
        maximum,
        maximum.saturating_mul(8),
        maximum.saturating_mul(8),
        norito::core::MAX_OWNED_VALUE_DECODE_DEPTH,
    )
}

pub(super) fn decode_exact_finalized_block(
    bytes: &[u8],
) -> Result<SignedBlock, KagemushaPromotionReceiptValidationError> {
    if bytes.is_empty() || bytes.len() > AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1 {
        return Err(KagemushaPromotionReceiptValidationError::BlockWireSize {
            actual: bytes.len(),
            maximum: AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1,
        });
    }
    let block = norito::with_decode_limits(block_decode_limits(), || {
        decode_framed_signed_block(bytes).map_err(|_| norito::core::Error::LengthMismatch)
    })
    .map_err(|_| KagemushaPromotionReceiptValidationError::BlockDecode)?;
    let canonical = block
        .encode_wire()
        .map_err(|_| KagemushaPromotionReceiptValidationError::BlockEncode)?;
    if canonical != bytes {
        return Err(KagemushaPromotionReceiptValidationError::NonCanonicalBlock);
    }
    Ok(block)
}

#[cfg(feature = "json")]
fn receipt_json_error(message: &str) -> norito::json::Error {
    norito::json::Error::InvalidField {
        field: "finalized_block_wire".to_owned(),
        message: message.to_owned(),
    }
}

#[cfg(feature = "json")]
fn base64_encoded_len(maximum_decoded_bytes: usize) -> Option<usize> {
    maximum_decoded_bytes
        .checked_add(2)?
        .checked_div(3)?
        .checked_mul(4)
}

#[cfg(feature = "json")]
fn base64_sextet(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

#[cfg(feature = "json")]
fn canonical_base64_decoded_len(
    encoded: &str,
    maximum_decoded_bytes: usize,
) -> Result<usize, norito::json::Error> {
    if encoded.is_empty() || !encoded.len().is_multiple_of(4) {
        return Err(receipt_json_error(
            "base64 token has a non-canonical length",
        ));
    }
    let padding = match encoded.as_bytes() {
        [.., b'=', b'='] => 2,
        [.., b'='] => 1,
        _ => 0,
    };
    let payload_len = encoded.len() - padding;
    let bytes = encoded.as_bytes();
    if bytes[..payload_len]
        .iter()
        .any(|byte| base64_sextet(*byte).is_none())
        || bytes[payload_len..].iter().any(|byte| *byte != b'=')
    {
        return Err(receipt_json_error(
            "expected canonical padded standard base64",
        ));
    }
    let tail_is_canonical = match padding {
        0 => true,
        1 => {
            payload_len % 4 == 3
                && base64_sextet(bytes[payload_len - 1])
                    .is_some_and(|sextet| sextet.is_multiple_of(4))
        }
        2 => {
            payload_len % 4 == 2
                && base64_sextet(bytes[payload_len - 1])
                    .is_some_and(|sextet| sextet.is_multiple_of(16))
        }
        _ => false,
    };
    if !tail_is_canonical {
        return Err(receipt_json_error(
            "base64 token has non-canonical tail bits",
        ));
    }
    let decoded_len = encoded
        .len()
        .checked_div(4)
        .and_then(|length| length.checked_mul(3))
        .and_then(|length| length.checked_sub(padding))
        .ok_or_else(|| receipt_json_error("invalid base64 decoded length"))?;
    if decoded_len > maximum_decoded_bytes {
        return Err(receipt_json_error(
            "decoded block wire exceeds the 32 MiB limit",
        ));
    }
    Ok(decoded_len)
}

#[cfg(feature = "json")]
fn bounded_base64_token<'a>(
    parser: &mut norito::json::Parser<'a>,
    maximum_decoded_bytes: usize,
) -> Result<(&'a str, usize), norito::json::Error> {
    let maximum_encoded_bytes = base64_encoded_len(maximum_decoded_bytes)
        .ok_or_else(|| receipt_json_error("base64 length arithmetic overflow"))?;
    parser.skip_ws();
    let start = parser.position();
    let decoded_token_len = parser.skip_string_bounded(maximum_encoded_bytes)?;
    let end = parser.position();
    let encoded = parser
        .input()
        .get(start.saturating_add(1)..end.saturating_sub(1))
        .ok_or_else(|| receipt_json_error("invalid JSON string bounds"))?;
    if encoded.len() != decoded_token_len || encoded.as_bytes().contains(&b'\\') {
        return Err(receipt_json_error(
            "base64 token must use its unescaped canonical spelling",
        ));
    }
    let decoded_len = canonical_base64_decoded_len(encoded, maximum_decoded_bytes)?;
    Ok((encoded, decoded_len))
}

/// Failure while decoding or validating a Kagemusha promotion receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum KagemushaPromotionReceiptValidationError {
    /// One named structural field is invalid.
    #[error("invalid Kagemusha promotion receipt field: {0}")]
    InvalidField(&'static str),
    /// A signer does not match the signed identity.
    #[error("Kagemusha promotion receipt signer mismatch: {0}")]
    SignerMismatch(&'static str),
    /// A signature is malformed or invalid.
    #[error("invalid Kagemusha promotion receipt signature: {0}")]
    InvalidSignature(&'static str),
    /// The root promotion controller differs from the independently pinned key.
    #[error("Kagemusha promotion controller is not the independently pinned Ed25519 key")]
    PromotionController,
    /// GitHub provenance or a reservation-to-activation binding differs.
    #[error("Kagemusha promotion reservation provenance or release binding differs")]
    PromotionProvenance,
    /// The controller reuses a receipt-issuer, governance, or validator key.
    #[error(
        "promotion controller must be independent of receipt issuer, governance, and validators"
    )]
    ControllerRoleOverlap,
    /// Canonical promotion-reservation encoding failed.
    #[error("failed to encode the canonical Kagemusha promotion reservation")]
    ReservationEncode,
    /// Canonical bounded promotion-reservation decoding failed.
    #[error("failed to decode the canonical bounded Kagemusha promotion reservation")]
    ReservationDecode,
    /// Canonical reservation input violates the encoded-size ceiling.
    #[error("Kagemusha promotion reservation is {actual} bytes; maximum is {maximum}")]
    ReservationSize {
        /// Actual byte length.
        actual: usize,
        /// Maximum permitted byte length.
        maximum: usize,
    },
    /// The expectations artifact does not bind the exact signed reservation bytes.
    #[error("Kagemusha promotion reservation digest differs from its exact bytes")]
    ReservationDigest,
    /// Canonical activation-expectations encoding failed.
    #[error("failed to encode the canonical Kagemusha activation expectations")]
    ExpectationsEncode,
    /// Canonical bounded activation-expectations decoding failed.
    #[error("failed to decode the canonical bounded Kagemusha activation expectations")]
    ExpectationsDecode,
    /// Canonical expectations input violates the encoded-size ceiling.
    #[error("Kagemusha activation expectations are {actual} bytes; maximum is {maximum}")]
    ExpectationsSize {
        /// Actual byte length.
        actual: usize,
        /// Maximum permitted byte length.
        maximum: usize,
    },
    /// Supplied expectations bytes differ from the exact canonical artifact.
    #[error("Kagemusha activation expectations digest differs from exact artifact bytes")]
    ExpectationsDigest,
    /// Four validator identities or their order differ.
    #[error("Kagemusha validator set is not the exact ordered four-validator set")]
    ValidatorSet,
    /// Receipt-controlled values differ from external expectations.
    #[error("Kagemusha activation receipt differs from external expectations")]
    ExpectationMismatch,
    /// Canonical receipt encoding failed.
    #[error("failed to encode the canonical Kagemusha activation receipt")]
    ReceiptEncode,
    /// Canonical bounded receipt decoding failed.
    #[error("failed to decode the canonical bounded Kagemusha activation receipt")]
    ReceiptDecode,
    /// Bounded JSON receipt decoding failed.
    #[error("failed to decode the bounded Kagemusha activation receipt JSON")]
    ReceiptJsonDecode,
    /// Canonical receipt input violates the encoded-size ceiling.
    #[error("Kagemusha activation receipt is {actual} bytes; maximum is {maximum}")]
    ReceiptSize {
        /// Actual byte length.
        actual: usize,
        /// Maximum permitted byte length.
        maximum: usize,
    },
    /// Raw JSON input violates the ingress ceiling.
    #[error("Kagemusha activation receipt JSON is {actual} bytes; maximum is {maximum}")]
    JsonSize {
        /// Actual byte length.
        actual: usize,
        /// Maximum permitted byte length.
        maximum: usize,
    },
    /// Embedded finalized block wire violates its public ceiling.
    #[error("Kagemusha finalized block wire is {actual} bytes; maximum is {maximum}")]
    BlockWireSize {
        /// Actual byte length.
        actual: usize,
        /// Maximum permitted byte length.
        maximum: usize,
    },
    /// The separately retained exact block identity differs from its bytes.
    #[error("finalized SignedBlockWire digest does not match its exact bytes")]
    BlockWireDigest,
    /// Framed signed-block decoding failed.
    #[error("failed to decode the exact finalized SignedBlockWire")]
    BlockDecode,
    /// Canonical block re-encoding failed.
    #[error("failed to re-encode the finalized SignedBlockWire")]
    BlockEncode,
    /// Decoded block bytes are not their exact canonical re-encoding.
    #[error("finalized SignedBlockWire is not canonical")]
    NonCanonicalBlock,
    /// Committed entrypoint, result, or inclusion proof is invalid.
    #[error("invalid committed activation transaction or execution result")]
    CommittedTransaction,
    /// The committed entrypoint is not the exact direct governed transaction.
    #[error("invalid governed activation transaction")]
    ActivationTransaction,
    /// Governance is not the exact strong multisignature authority and bundle.
    #[error("activation governance is not the exact approved strong multisignature policy")]
    GovernanceAuthority,
    /// The receipt issuer reuses a governance-member or validator key.
    #[error("activation receipt issuer must be independent of governance and validators")]
    IssuerRoleOverlap,
    /// The finalized block carries different authorization bytes for the same intent.
    #[error("finalized block activation authorization wire differs from the approved wire")]
    ActivationAuthorizationWire,
    /// The direct activation payload differs from the sealed release and policy.
    #[error("invalid Kagemusha V4 activation payload binding")]
    ActivationPayload,
    /// The activation transaction omits or exceeds the anchor-bounded height expiry.
    #[error("invalid Kagemusha V4 activation transaction height expiry")]
    ActivationExpiry,
    /// The retained finality material is not one bounded contiguous successor chain.
    #[error("invalid Kagemusha V4 activation finality proof chain")]
    FinalityChain,
    /// Finality roster and sealed validators differ.
    #[error("finality roster differs from the four sealed validators")]
    FinalityRoster,
    /// Sumeragi-v2 finality verification failed.
    #[error("invalid activation finality proof")]
    Finality,
    /// Finality does not authenticate the supplied result-bearing block bytes.
    #[error("activation finality does not bind the supplied block and entrypoint")]
    FinalityBlockBinding,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Level,
        account::MultisigMember,
        block::{
            BlockHeader,
            consensus_v2::{
                BlockSubject, ConsensusMode, ConsensusRound, DualQuorum, ExecutionCommitment,
                GlobalPhase, HeightContext, PROTOCOL_VERSION, QuorumCertificate, ValidatorPower,
                finality::V2FinalityArtifact,
            },
        },
        prelude::Log,
        transaction::{
            DataTriggerSequence, FeePaymentIntent, TransactionBuilder, TransactionResult,
            TransactionResultInner,
        },
    };
    use iroha_crypto::MerkleTree;
    use std::num::NonZeroU64;

    fn exact(label: &[u8]) -> KagemushaExactBytesDigestV1 {
        KagemushaExactBytesDigestV1::from_bytes(label).expect("non-empty exact bytes")
    }

    fn network(label: &[u8]) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(label)))
    }

    fn effective_config(
        keys: &[KeyPair],
        execution_policy_hash: Hash,
    ) -> KagemushaV4RuntimeEffectiveConfigProjectionV1 {
        let validators = keys
            .iter()
            .enumerate()
            .map(
                |(index, key)| crate::offline::KagemushaV4RuntimeValidatorProjectionV1 {
                    validator_id: PeerId::new(key.public_key().clone()),
                    public_address: format!("127.0.0.1:{}", 14_000 + index)
                        .parse()
                        .expect("fixture validator address"),
                    bls_pop: iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("fixture validator PoP"),
                },
            )
            .collect::<Vec<_>>()
            .try_into()
            .expect("exactly four runtime validators");
        let projection = KagemushaV4RuntimeEffectiveConfigProjectionV1 {
            chain: crate::ChainId::from("kagemusha-qualification-test"),
            chain_discriminant: 42,
            is_validator: true,
            genesis_public_key: KeyPair::from_seed(vec![0x29; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
            genesis_expected_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"genesis header",
            )),
            validators,
            sumeragi_config_fingerprint: Hash::new(b"effective Sumeragi V2 config"),
            genesis_context: crate::block::consensus_v2::SumeragiV2GenesisContextParameters {
                execution_policy_hash: *execution_policy_hash.as_ref(),
                ..crate::block::consensus_v2::SumeragiV2GenesisContextParameters::recommended()
            },
            kagemusha_max_decoded_bytes: 64 * 1024 * 1024,
        };
        projection
            .validate()
            .expect("valid effective runtime config");
        projection
    }

    fn binding() -> KagemushaV4PromotionBindingV1 {
        KagemushaV4PromotionBindingV1 {
            promotion_controller: KeyPair::from_seed(vec![0x30; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
            promotion_reservation: exact(b"canonical signed promotion reservation"),
            promotion_id: Sha256::digest(b"promotion run 1").into(),
            network_id: network(b"qualification network"),
            reviewed_source_closure_descriptor_sha256: Sha256::digest(b"source closure").into(),
            manifest_sha256: Sha256::digest(b"release manifest").into(),
            release_record_sha256: Sha256::digest(b"release record").into(),
            release_policy_source: exact(b"canonical release policy source"),
            device_attestation_policy_norito: exact(b"canonical device policy Norito"),
            signed_genesis: exact(b"exact signed genesis source"),
            catalog_consensus_policy_digest: Sha256::digest(b"ordered catalog policy").into(),
            execution_policy_hash: Hash::new(b"aggregate execution policy"),
        }
    }

    fn qualification_fixture() -> (
        KagemushaV4PromotionBindingV1,
        [KagemushaV4ValidatorQualificationSealBodyV1; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
        [KagemushaV4ValidatorQualificationSealV1; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
        Vec<KeyPair>,
    ) {
        let binding = binding();
        let mut keys = [0x31_u8, 0x32, 0x33, 0x34]
            .into_iter()
            .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let runtime_effective_config = effective_config(&keys, binding.execution_policy_hash);
        let bodies: [KagemushaV4ValidatorQualificationSealBodyV1;
            KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT] = keys
            .iter()
            .enumerate()
            .map(|(index, key)| KagemushaV4ValidatorQualificationSealBodyV1 {
                schema: KAGEMUSHA_V4_VALIDATOR_QUALIFICATION_SEAL_BODY_SCHEMA.to_owned(),
                version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
                binding: binding.clone(),
                validator_id: PeerId::new(key.public_key().clone()),
                iroha3d_executable: exact(format!("host-{index}-iroha3d").as_bytes()),
                flattened_toml_config_source: exact(
                    format!("host-{index}-flattened-config-source").as_bytes(),
                ),
                runtime_effective_config: runtime_effective_config.clone(),
                catalog_qualification_seal: exact(
                    format!("host-{index}-catalog-qualification-seal").as_bytes(),
                ),
            })
            .collect::<Vec<_>>()
            .try_into()
            .expect("exactly four qualification bodies");
        let seals = bodies
            .iter()
            .zip(&keys)
            .map(|(body, key)| {
                KagemushaV4ValidatorQualificationSealV1::try_sign(body.clone(), key)
                    .expect("sign valid validator qualification body")
            })
            .collect::<Vec<_>>()
            .try_into()
            .expect("exactly four qualification seals");
        (binding, bodies, seals, keys)
    }

    #[test]
    fn runtime_effective_config_consensus_digest_is_domain_separated_and_complete() {
        let (_, bodies, _, _) = qualification_fixture();
        let projection = &bodies[0].runtime_effective_config;
        let digest = projection
            .consensus_sha256()
            .expect("valid runtime projection digest");
        let undomained_digest: [u8; 32] =
            Sha256::digest(norito::encode_canonical(projection).expect("canonical projection"))
                .into();
        assert_ne!(digest, [0; 32]);
        assert_ne!(
            digest, undomained_digest,
            "the consensus identity must include its explicit domain",
        );
        let mut changed = projection.clone();
        changed.kagemusha_max_decoded_bytes += 1;
        assert_ne!(
            changed
                .consensus_sha256()
                .expect("changed projection remains valid"),
            digest,
        );
    }

    #[test]
    fn governance_policy_requires_two_distinct_signers_not_only_threshold_weight() {
        let keys =
            [0x2a_u8, 0x2b].map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519));
        let weighted_policy = MultisigPolicy::new(
            2,
            vec![
                MultisigMember::new(keys[0].public_key().clone(), 2)
                    .expect("valid weight-two governance member"),
                MultisigMember::new(keys[1].public_key().clone(), 1)
                    .expect("valid weight-one governance member"),
            ],
        )
        .expect("structurally valid weighted governance policy");
        assert!(
            !kagemusha_v4_governance_policy_requires_distinct_signers(&weighted_policy),
            "one threshold-weight member cannot satisfy the distinct-governor contract"
        );
        let weighted_authority = AccountId::new_multisig(weighted_policy);
        assert_eq!(
            validate_governance_multisig_policy(&weighted_authority),
            Err(KagemushaPromotionReceiptValidationError::GovernanceAuthority),
        );

        let distinct_policy = MultisigPolicy::new(
            2,
            keys.iter()
                .map(|key| {
                    MultisigMember::new(key.public_key().clone(), 1)
                        .expect("valid unit-weight governance member")
                })
                .collect(),
        )
        .expect("valid two-distinct-signer governance policy");
        assert!(kagemusha_v4_governance_policy_requires_distinct_signers(
            &distinct_policy
        ));
    }

    #[test]
    fn four_host_seals_accept_per_host_bytes_and_roundtrip_canonically() {
        let (binding, bodies, seals, _) = qualification_fixture();
        verify_kagemusha_v4_validator_qualification_seals(&seals, &bodies, &binding)
            .expect("four exact validator seals verify");
        assert_ne!(
            bodies[0].iroha3d_executable, bodies[1].iroha3d_executable,
            "heterogeneous validator executables are per-host identities",
        );
        assert_ne!(
            bodies[0].catalog_qualification_seal, bodies[1].catalog_qualification_seal,
            "host-local catalog-seal bytes are never forced equal",
        );
        let canonical = norito::encode_canonical(&seals[0]).expect("encode qualification seal");
        let decoded =
            norito::decode_canonical::<KagemushaV4ValidatorQualificationSealV1>(&canonical)
                .expect("decode qualification seal");
        assert_eq!(decoded, seals[0]);
        decoded.verify().expect("roundtripped signature verifies");

        #[cfg(feature = "json")]
        {
            let json = norito::json::to_json(&seals[0]).expect("encode qualification JSON");
            let decoded: KagemushaV4ValidatorQualificationSealV1 =
                norito::json::from_str(&json).expect("decode qualification JSON");
            assert_eq!(decoded, seals[0]);
            decoded.verify().expect("JSON signature verifies");
        }
    }

    #[test]
    fn validator_seals_reject_reorder_replay_and_validly_expected_body_tamper() {
        let (binding, bodies, seals, keys) = qualification_fixture();

        let mut reordered = seals.clone();
        reordered.swap(0, 1);
        assert_eq!(
            verify_kagemusha_v4_validator_qualification_seals(&reordered, &bodies, &binding,),
            Err(KagemushaPromotionReceiptValidationError::ValidatorSet),
        );

        let mut replay_binding = binding.clone();
        replay_binding.promotion_id[0] ^= 1;
        let mut replay_body = bodies[0].clone();
        replay_body.binding = replay_binding;
        let replay_seal = KagemushaV4ValidatorQualificationSealV1::try_sign(replay_body, &keys[0])
            .expect("validly sign hostile replay body");
        let mut replayed = seals.clone();
        replayed[0] = replay_seal;
        assert_eq!(
            verify_kagemusha_v4_validator_qualification_seals(&replayed, &bodies, &binding),
            Err(KagemushaPromotionReceiptValidationError::ValidatorSet),
        );

        let mut tampered_body = bodies[0].clone();
        tampered_body.catalog_qualification_seal = exact(b"substituted catalog seal bytes");
        let mut tampered_expected = bodies.clone();
        tampered_expected[0] = tampered_body.clone();
        let mut tampered_seals = seals;
        tampered_seals[0].body = tampered_body;
        assert_eq!(
            verify_kagemusha_v4_validator_qualification_seals(
                &tampered_seals,
                &tampered_expected,
                &binding,
            ),
            Err(KagemushaPromotionReceiptValidationError::InvalidSignature(
                "validator_qualification",
            )),
        );
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the receipt fixture assembles the complete signed governance, validator, transaction, and finality chain"
    )]
    fn roundtrip_receipt() -> KagemushaV4ActivationFinalityReceiptV1 {
        let (binding, bodies, seals, _) = qualification_fixture();
        let runtime = &bodies[0].runtime_effective_config;
        let issuer = KeyPair::from_seed(vec![0x71; 32], Algorithm::Ed25519);
        let governance_keys = [0x72_u8, 0x73, 0x74]
            .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519));
        let governance_policy = MultisigPolicy::new(
            2,
            governance_keys
                .iter()
                .map(|key| {
                    MultisigMember::new(key.public_key().clone(), 1)
                        .expect("valid governance member")
                })
                .collect(),
        )
        .expect("strong governance multisig policy");
        let authority = AccountId::new_multisig(governance_policy);
        let transaction = TransactionBuilder::new(
            binding.network_id,
            authority.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            Level::INFO,
            "roundtrip-only receipt fixture".into(),
        )])
        .sign_multisig([
            governance_keys[0].private_key(),
            governance_keys[1].private_key(),
        ]);
        let entrypoint = TransactionEntrypoint::External(transaction.clone());
        let entrypoint_hash = transaction.hash_as_entrypoint();
        let entrypoint_tree = MerkleTree::from_iter([entrypoint_hash]);
        let result = TransactionResult(
            TransactionResultInner::Ok(DataTriggerSequence::default()),
            Vec::new(),
            None,
        );
        let result_hash = HashOf::new(&result);
        let result_tree = MerkleTree::from_iter([result_hash]);
        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero fixture height"),
            None,
            entrypoint_tree.root(),
            result_tree.root(),
            1,
            0,
        );
        let committed_transaction = CommittedTransaction {
            block_hash: header.hash(),
            entrypoint_hash,
            entrypoint_proof: entrypoint_tree.get_proof(0).expect("entrypoint proof"),
            entrypoint,
            result_hash,
            result_proof: result_tree.get_proof(0).expect("result proof"),
            result,
            merge_inclusion: None,
        };
        let roster = bodies
            .iter()
            .map(|body| ValidatorPower {
                validator: body.validator_id.clone(),
                power: 1,
            })
            .collect::<Vec<_>>();
        let context = HeightContext {
            network_id: binding.network_id,
            protocol_version: PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: u64::MAX,
            next_epoch_snapshot: None,
            mode: ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: DualQuorum::from_roster(&roster).expect("four-validator quorum"),
            roster,
            nexus_amx_context_hash: Hash::prehashed(runtime.genesis_context.nexus_amx_context_hash),
            execution_policy_hash: binding.execution_policy_hash,
            da_layout: runtime.genesis_context.da_layout,
            leader_seed: [0xA5; 32],
        };
        let subject = BlockSubject {
            parent_block_hash: None,
            block_hash: header.hash(),
            payload_hash: Hash::new(b"roundtrip-only proposal payload"),
        };
        let round = ConsensusRound {
            context_id: context.id(),
            height: 1,
            view: 0,
        };
        let execution_commitment = ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"roundtrip parent state"),
            Hash::new(b"roundtrip post state"),
            Hash::new(b"roundtrip writes"),
            1,
            Hash::new(b"roundtrip block wire"),
        );
        let finality_artifact = V2FinalityArtifact::new(
            context,
            subject,
            QuorumCertificate {
                round,
                proposal_round: round,
                phase: GlobalPhase::Commit,
                subject,
                execution_commitment,
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0xA5],
            },
            runtime
                .validators
                .iter()
                .map(|validator| validator.bls_pop.clone())
                .collect(),
        );
        let transaction_wire = transaction
            .encode_wire_v1()
            .expect("encode authorization-bearing transaction");
        let finalized_block_wire = vec![0xA5];
        let promotion_reservation = exact(b"roundtrip signed promotion reservation");
        let activation_expectations_artifact = exact(b"roundtrip signed activation expectations");
        KagemushaV4ActivationFinalityReceiptV1::try_sign(
            KagemushaV4ActivationFinalityReceiptBodyV1 {
                schema: KAGEMUSHA_V4_ACTIVATION_FINALITY_RECEIPT_BODY_SCHEMA.to_owned(),
                version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
                promotion_reservation,
                activation_expectations_artifact,
                binding,
                issuer: issuer.public_key().clone(),
                governance_authority: authority,
                validator_seals: seals,
                activation_transaction_intent: transaction.hash(),
                activation_transaction_wire: KagemushaExactBytesDigestV1::from_bytes(
                    &transaction_wire,
                )
                .expect("transaction wire identity"),
                committed_transaction,
                finalized_block_wire: KagemushaFinalizedBlockWireV1::try_from_bytes(
                    finalized_block_wire.clone(),
                )
                .expect("bounded roundtrip block carrier"),
                finalized_block_wire_digest: KagemushaExactBytesDigestV1::from_bytes(
                    &finalized_block_wire,
                )
                .expect("roundtrip block identity"),
                finality_proof_chain: vec![BridgeFinalityProof {
                    version: crate::bridge::BRIDGE_FINALITY_PROOF_VERSION_V2,
                    block_header: header,
                    finality_artifact,
                }]
                .try_into()
                .expect("one bounded roundtrip successor proof"),
            },
            &issuer,
        )
        .expect("sign roundtrip receipt")
    }

    #[test]
    fn complete_receipt_norito_and_bounded_json_roundtrip_without_claiming_activation() {
        let receipt = roundtrip_receipt();
        let canonical = norito::encode_canonical(&receipt).expect("encode complete receipt");
        let decoded = KagemushaV4ActivationFinalityReceiptV1::decode_canonical(&canonical)
            .expect("bounded canonical receipt decode");
        assert_eq!(decoded, receipt);
        assert_eq!(
            decoded.verify(&KagemushaV4ActivationReceiptExpectationsV1 {
                promotion_controller: KeyPair::from_seed(vec![0x70; 32], Algorithm::Ed25519)
                    .public_key()
                    .clone(),
                promotion_reservation: decoded.body.promotion_reservation,
                activation_expectations_artifact: decoded.body.activation_expectations_artifact,
                binding: decoded.body.binding.clone(),
                receipt_issuer: decoded.body.issuer.clone(),
                governance_authority: decoded.body.governance_authority.clone(),
                governance_multisig_policy: decoded
                    .body
                    .governance_authority
                    .controller()
                    .multisig_policy()
                    .expect("roundtrip fixture has multisig governance")
                    .clone(),
                validator_seals: decoded.body.validator_seals.clone(),
                validator_bodies: decoded.body.validator_seals.clone().map(|seal| seal.body),
                activation_transaction_intent: decoded.body.activation_transaction_intent,
                activation_transaction_wire: decoded.body.activation_transaction_wire,
                trusted_finality_anchor: decoded
                    .body
                    .finality_proof_chain
                    .first()
                    .expect("roundtrip fixture proof")
                    .clone(),
            }),
            Err(KagemushaPromotionReceiptValidationError::BlockDecode),
            "a serializable placeholder is not evidence that activation occurred",
        );
        assert_eq!(
            enforce_activation_receipt_frame_size(&decoded)
                .expect("count canonical receipt without allocating output"),
            canonical.len(),
        );

        #[cfg(feature = "json")]
        {
            let json = norito::json::to_json(&receipt).expect("encode complete receipt JSON");
            let decoded = KagemushaV4ActivationFinalityReceiptV1::decode_json_bounded(&json)
                .expect("bounded JSON receipt decode");
            assert_eq!(decoded, receipt);
            let hostile = json.replacen('{', "{\"unexpected\":true,", 1);
            assert_eq!(
                KagemushaV4ActivationFinalityReceiptV1::decode_json_bounded(&hostile),
                Err(KagemushaPromotionReceiptValidationError::ReceiptJsonDecode),
            );
        }
    }

    #[test]
    fn finality_chain_rejects_oversize_declared_count_before_vector_decode() {
        let receipt = roundtrip_receipt();
        let mut encoded = receipt.body.finality_proof_chain.encode();
        let declared = u64::try_from(KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1)
            .expect("proof-count bound fits u64")
            .saturating_add(1);
        encoded[..core::mem::size_of::<u64>()].copy_from_slice(&declared.to_le_bytes());
        let error = <KagemushaV4ActivationFinalityProofChainV1 as ncore::DecodeFromSlice>::decode_from_slice(
            &encoded,
        )
        .expect_err("oversize declared proof count must fail before vector decoding");
        assert!(
            error.to_string().contains("outside 1..=4096"),
            "expected intrinsic proof-count preflight, got {error}"
        );
    }

    #[test]
    fn finality_chain_iter_borrows_proofs_in_order() {
        let receipt = roundtrip_receipt();
        let chain = &receipt.body.finality_proof_chain;
        let mut proofs = chain.iter();

        assert_eq!(proofs.len(), chain.len());
        assert!(core::ptr::eq(
            proofs.next().expect("non-empty chain"),
            chain.first().expect("non-empty chain"),
        ));
        assert_eq!(proofs.next(), None);
    }

    #[cfg(feature = "json")]
    #[test]
    fn finality_chain_json_rejects_oversize_array_before_element_decode() {
        let mut hostile = String::from("[");
        for index in 0..=KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1 {
            if index != 0 {
                hostile.push(',');
            }
            hostile.push_str("null");
        }
        hostile.push(']');
        let error = norito::json::from_str::<KagemushaV4ActivationFinalityProofChainV1>(&hostile)
            .expect_err("oversize proof array must fail before decoding its null elements");
        assert!(
            error.to_string().contains("outside 1..=4096"),
            "expected intrinsic JSON proof-count preflight, got {error}"
        );
    }

    #[cfg(feature = "json")]
    #[test]
    fn block_json_preflight_rejects_oversize_before_base64_decode() {
        assert!(canonical_base64_decoded_len("YWI=", 1).is_err());
        assert!(canonical_base64_decoded_len("YWJ=", 2).is_err());
        assert_eq!(
            canonical_base64_decoded_len("YWI=", 2).expect("canonical base64 within limit"),
            2
        );
        assert_eq!(
            KagemushaFinalizedBlockWireV1::try_from_bytes(Vec::new()),
            Err(KagemushaPromotionReceiptValidationError::BlockWireSize {
                actual: 0,
                maximum: AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1,
            }),
        );
    }
}
