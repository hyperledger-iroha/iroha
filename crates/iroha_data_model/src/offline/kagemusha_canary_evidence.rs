//! Authenticated, promotion-bound evidence for the post-activation Taira canary.

use std::{num::NonZeroU64, str::FromStr as _};

use crate::{
    NetworkId,
    account::AccountId,
    block::{BlockHeader, SignedBlock, proofs::TrustedBlockProofAnchor},
    bridge::BridgeFinalityVerifier,
    isi::offline::RecordKagemushaTairaCanaryV4,
    metadata::Metadata,
    prelude::Name,
    query::CommittedTransaction,
    transaction::{
        Executable, SignedTransaction, TransactionAdmissionIntent, TransactionEntrypoint,
    },
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, PublicKey, SignatureOf};
use iroha_primitives::json::Json;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

use super::kagemusha_promotion_receipt::{
    KAGEMUSHA_V4_ACTIVATION_EXPECTATIONS_MAX_BYTES,
    KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1, KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_BYTES,
    KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION, KAGEMUSHA_V4_PROMOTION_RESERVATION_MAX_BYTES,
    KagemushaExactBytesDigestV1, KagemushaFinalizedBlockWireV1,
    KagemushaV4ActivationFinalityProofChainV1, KagemushaV4ActivationFinalityReceiptV1,
    KagemushaV4ActivationReceiptExpectationsV1, KagemushaV4PromotionBindingV1,
    KagemushaV4VerifiedActivationReceiptV1, decode_exact_finalized_block,
    validate_finality_corridor_context,
};

/// Maximum canonical bytes accepted for one controller-signed canary permit.
pub const KAGEMUSHA_V4_TAIRA_CANARY_PERMIT_MAX_BYTES: usize = 64 * 1024;
/// Maximum canonical bytes accepted for a signed canary authorization.
pub const KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_MAX_BYTES: usize = 64 * 1024 * 1024;
/// Maximum canonical bytes accepted for an on-chain exact-hash reservation.
pub const KAGEMUSHA_V4_TAIRA_CANARY_RESERVATION_MAX_BYTES: usize = 128 * 1024;
/// Maximum canonical bytes accepted for a signed canary-evidence artifact.
pub const KAGEMUSHA_V4_TAIRA_CANARY_EVIDENCE_MAX_BYTES: usize = 64 * 1024 * 1024;
/// Maximum controller-authorized wall-clock lifetime.
pub const KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_MAX_INTERVAL_MS: u64 = 5 * 60 * 1_000;
/// Maximum exact canonical Torii-origin length.
pub const KAGEMUSHA_V4_TAIRA_CANARY_TORII_ORIGIN_MAX_BYTES: usize = 512;
/// Maximum wall-clock interval occupied by the diagnostic query pass.
pub const KAGEMUSHA_V4_TAIRA_CANARY_QUERY_MAX_WINDOW_MS: u64 = 30 * 60 * 1_000;
/// Maximum clock difference tolerated between the protected host and queried peers.
pub const KAGEMUSHA_V4_TAIRA_CANARY_QUERY_CLOCK_SKEW_MS: u64 = 5 * 60 * 1_000;
/// Schema id of a promotion-bound Taira canary-authorization body.
pub const KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_BODY_SCHEMA: &str =
    "iroha.kagemusha.v4.taira_canary_authorization_body.v1";
/// Schema id of a signed promotion-bound Taira canary permit.
pub const KAGEMUSHA_V4_TAIRA_CANARY_PERMIT_SCHEMA: &str =
    "iroha.kagemusha.v4.taira_canary_permit.v1";
/// Schema id of a promotion-bound Taira canary authorization package.
pub const KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_SCHEMA: &str =
    "iroha.kagemusha.v4.taira_canary_authorization.v1";
/// Schema id of an exact-hash canary-reservation body.
pub const KAGEMUSHA_V4_TAIRA_CANARY_RESERVATION_BODY_SCHEMA: &str =
    "iroha.kagemusha.v4.taira_canary_reservation_body.v1";
/// Schema id of a controller-signed exact-hash canary reservation.
pub const KAGEMUSHA_V4_TAIRA_CANARY_RESERVATION_SCHEMA: &str =
    "iroha.kagemusha.v4.taira_canary_reservation.v1";
/// Schema id of the exact transaction package signed by the promotion controller.
pub const KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_PACKAGE_SCHEMA: &str =
    "iroha.kagemusha.v4.taira_canary_authorization_package.v1";
/// Schema id committed by the exact canary transaction metadata.
pub const KAGEMUSHA_V4_TAIRA_CANARY_TRANSACTION_SCHEMA: &str =
    "iroha.kagemusha.v4.taira_canary_transaction.v1";
/// Schema id of a promotion-bound Taira canary-evidence body.
pub const KAGEMUSHA_V4_TAIRA_CANARY_EVIDENCE_BODY_SCHEMA: &str =
    "iroha.kagemusha.v4.taira_canary_evidence_body.v1";
/// Schema id of a signed promotion-bound Taira canary-evidence artifact.
pub const KAGEMUSHA_V4_TAIRA_CANARY_EVIDENCE_SCHEMA: &str =
    "iroha.kagemusha.v4.taira_canary_evidence.v1";
/// Domain separator for controller canary-permit signatures.
pub const KAGEMUSHA_V4_TAIRA_CANARY_PERMIT_SIGNATURE_DOMAIN: &[u8] =
    b"iroha:kagemusha:v4:taira-canary-permit:v1\0";
/// Domain separator for exact-hash canary-reservation signatures.
pub const KAGEMUSHA_V4_TAIRA_CANARY_RESERVATION_SIGNATURE_DOMAIN: &[u8] =
    b"iroha:kagemusha:v4:taira-canary-reservation:v1\0";
/// Domain separator for exact canary-transaction authorization signatures.
pub const KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_SIGNATURE_DOMAIN: &[u8] =
    b"iroha:kagemusha:v4:taira-canary-authorization:v1\0";
/// Domain separator for independent receipt-issuer canary-evidence signatures.
pub const KAGEMUSHA_V4_TAIRA_CANARY_EVIDENCE_SIGNATURE_DOMAIN: &[u8] =
    b"iroha:kagemusha:v4:taira-canary-evidence:v1\0";

const CANARY_METADATA_SCHEMA: &str = "kagemusha_canary_schema";
const CANARY_METADATA_PROMOTION_ID: &str = "kagemusha_promotion_id";
const CANARY_METADATA_RECEIPT_LENGTH: &str = "kagemusha_activation_receipt_byte_len";
const CANARY_METADATA_RECEIPT_SHA256: &str = "kagemusha_activation_receipt_sha256";
const CANARY_METADATA_TORII_ORIGIN: &str = "kagemusha_torii_origin";
const CANARY_METADATA_EXPIRES_AT_HEIGHT: &str = "expires_at_height";
const QUERY_STATUS_RESPONSE_MAX_BYTES: u64 = 64 * 1024;
const QUERY_NODE_STATUS_MAX_BYTES: u64 = 8 * 1024 * 1024;

/// Build the only metadata map accepted for a Kagemusha V4 Taira canary transaction.
#[must_use]
pub fn kagemusha_v4_taira_canary_transaction_metadata(
    promotion_id: [u8; 32],
    activation_receipt: KagemushaExactBytesDigestV1,
    canonical_torii_origin: &str,
    expires_at_height: NonZeroU64,
) -> Metadata {
    let mut metadata = Metadata::default();
    for (name, value) in [
        (
            CANARY_METADATA_SCHEMA,
            Json::new(KAGEMUSHA_V4_TAIRA_CANARY_TRANSACTION_SCHEMA.to_owned()),
        ),
        (
            CANARY_METADATA_PROMOTION_ID,
            Json::new(hex::encode(promotion_id)),
        ),
        (
            CANARY_METADATA_RECEIPT_LENGTH,
            Json::new(activation_receipt.byte_len),
        ),
        (
            CANARY_METADATA_RECEIPT_SHA256,
            Json::new(hex::encode(activation_receipt.sha256)),
        ),
        (
            CANARY_METADATA_TORII_ORIGIN,
            Json::new(canonical_torii_origin.to_owned()),
        ),
        (
            CANARY_METADATA_EXPIRES_AT_HEIGHT,
            Json::new(expires_at_height.get()),
        ),
    ] {
        metadata.insert(
            Name::from_str(name).expect("static canary metadata names are valid"),
            value,
        );
    }
    metadata
}

/// Validate the exact lower-case HTTPS DNS origin accepted by canary artifacts.
///
/// # Errors
///
/// Returns [`KagemushaV4TairaCanaryEvidenceValidationError`] for a URL with credentials,
/// a path/query/fragment, a non-DNS host, a default explicit port, or non-canonical case.
pub fn validate_kagemusha_v4_taira_canary_torii_origin(
    origin: &str,
) -> Result<(), KagemushaV4TairaCanaryEvidenceValidationError> {
    if origin.is_empty()
        || origin.len() > KAGEMUSHA_V4_TAIRA_CANARY_TORII_ORIGIN_MAX_BYTES
        || !origin.is_ascii()
        || origin != origin.to_ascii_lowercase()
    {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::InvalidField(
            "taira_canary.canonical_torii_origin",
        ));
    }
    let Some(authority) = origin.strip_prefix("https://") else {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::InvalidField(
            "taira_canary.canonical_torii_origin",
        ));
    };
    if authority.is_empty()
        || authority
            .chars()
            .any(|character| matches!(character, '/' | '?' | '#' | '@' | '[' | ']'))
    {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::InvalidField(
            "taira_canary.canonical_torii_origin",
        ));
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
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::InvalidField(
            "taira_canary.canonical_torii_origin",
        ));
    }
    Ok(())
}

/// Controller-signed authorization body for one bounded post-receipt canary.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4TairaCanaryAuthorizationBodyV1 {
    /// Exact authorization-body schema.
    pub schema: String,
    /// Authorization version.
    pub version: u16,
    /// Complete activation-qualified promotion and consensus identity.
    pub binding: KagemushaV4PromotionBindingV1,
    /// Exact controller-signed activation-expectations identity.
    pub activation_expectations_artifact: KagemushaExactBytesDigestV1,
    /// Exact issuer-signed activation-receipt identity.
    pub activation_finality_receipt: KagemushaExactBytesDigestV1,
    /// Exact transaction authority permitted to relay the canary.
    pub canary_authority: AccountId,
    /// Exact lower-case HTTPS DNS origin authorized for submission and observation.
    pub canonical_torii_origin: String,
    /// Controller-declared beginning of the short authorization interval.
    pub authorized_at_unix_ms: u64,
    /// Controller-declared exclusive end of the short authorization interval.
    pub expires_at_unix_ms: u64,
    /// Exclusive consensus-height expiry enforced by the canary instruction.
    pub expires_at_height: NonZeroU64,
}

impl KagemushaV4TairaCanaryAuthorizationBodyV1 {
    /// Return the domain-separated typed hash signed by the promotion controller.
    #[must_use]
    pub fn signing_hash(&self) -> HashOf<Self> {
        HashOf::from_untyped_unchecked(Hash::new_from_chunks(&[
            KAGEMUSHA_V4_TAIRA_CANARY_PERMIT_SIGNATURE_DOMAIN,
            &self.encode(),
        ]))
    }

    fn validate_structure(&self) -> Result<(), KagemushaV4TairaCanaryEvidenceValidationError> {
        if self.schema != KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_BODY_SCHEMA
            || self.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
            || self.authorized_at_unix_ms == 0
            || self.expires_at_unix_ms <= self.authorized_at_unix_ms
            || self
                .expires_at_unix_ms
                .saturating_sub(self.authorized_at_unix_ms)
                > KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_MAX_INTERVAL_MS
        {
            return Err(KagemushaV4TairaCanaryEvidenceValidationError::InvalidField(
                "taira_canary.authorization_body",
            ));
        }
        self.binding
            .validate()
            .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::ActivationBinding)?;
        validate_kagemusha_v4_taira_canary_torii_origin(&self.canonical_torii_origin)?;
        validate_identity_digest(
            self.activation_expectations_artifact,
            KAGEMUSHA_V4_ACTIVATION_EXPECTATIONS_MAX_BYTES,
            "taira_canary.activation_expectations",
        )?;
        validate_identity_digest(
            self.activation_finality_receipt,
            KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_BYTES,
            "taira_canary.activation_receipt",
        )?;
        Ok(())
    }
}

/// Controller-signed permit embedded into the exact canary instruction.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4TairaCanaryPermitV1 {
    /// Exact permit schema.
    pub schema: String,
    /// Permit version.
    pub version: u16,
    /// Signed permit body.
    pub body: KagemushaV4TairaCanaryAuthorizationBodyV1,
    /// Promotion-controller signature.
    pub signature: SignatureOf<KagemushaV4TairaCanaryAuthorizationBodyV1>,
}

impl KagemushaV4TairaCanaryPermitV1 {
    /// Authenticate all activation inputs and sign one bounded pre-commit permit.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4TairaCanaryEvidenceValidationError`] on any structural,
    /// exact-byte, receipt, signer, or size failure.
    pub fn try_sign(
        body: KagemushaV4TairaCanaryAuthorizationBodyV1,
        controller: &KeyPair,
        expectations: &KagemushaV4ActivationReceiptExpectationsV1,
        receipt: &KagemushaV4ActivationFinalityReceiptV1,
        exact_receipt_bytes: &[u8],
    ) -> Result<Self, KagemushaV4TairaCanaryEvidenceValidationError> {
        body.validate_structure()?;
        enforce_artifact_size(&body, KAGEMUSHA_V4_TAIRA_CANARY_PERMIT_MAX_BYTES)?;
        validate_permit_binding(&body, expectations, receipt, exact_receipt_bytes)?;
        if controller.public_key() != &body.binding.promotion_controller {
            return Err(KagemushaV4TairaCanaryEvidenceValidationError::SignerMismatch);
        }
        let signature = SignatureOf::try_from_hash(controller.private_key(), body.signing_hash())
            .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Signature)?;
        let artifact = Self {
            schema: KAGEMUSHA_V4_TAIRA_CANARY_PERMIT_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            body,
            signature,
        };
        enforce_artifact_size(&artifact, KAGEMUSHA_V4_TAIRA_CANARY_PERMIT_MAX_BYTES)?;
        Ok(artifact)
    }

    /// Decode one exact canonical bounded permit.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4TairaCanaryEvidenceValidationError`] for empty, oversized,
    /// non-canonical, or structurally invalid bytes.
    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, KagemushaV4TairaCanaryEvidenceValidationError> {
        check_input_size(bytes, KAGEMUSHA_V4_TAIRA_CANARY_PERMIT_MAX_BYTES)?;
        let artifact: Self = norito::decode_canonical_with_limits(
            bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Decode)?;
        artifact.body.validate_structure()?;
        if artifact.schema != KAGEMUSHA_V4_TAIRA_CANARY_PERMIT_SCHEMA
            || artifact.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
            || norito::encode_canonical(&artifact)
                .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Encode)?
                != bytes
        {
            return Err(KagemushaV4TairaCanaryEvidenceValidationError::Decode);
        }
        Ok(artifact)
    }

    fn verify_structure_and_signature(
        &self,
    ) -> Result<(), KagemushaV4TairaCanaryEvidenceValidationError> {
        enforce_artifact_size(self, KAGEMUSHA_V4_TAIRA_CANARY_PERMIT_MAX_BYTES)?;
        self.body.validate_structure()?;
        if self.schema != KAGEMUSHA_V4_TAIRA_CANARY_PERMIT_SCHEMA
            || self.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
        {
            return Err(KagemushaV4TairaCanaryEvidenceValidationError::Authorization);
        }
        verify_authorization_signature(
            &self.signature,
            &self.body.binding.promotion_controller,
            self.body.signing_hash(),
        )
    }

    /// Verify the permit at the deterministic consensus execution point.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4TairaCanaryEvidenceValidationError`] when the
    /// permit signature, network, relayer authority, wall-clock interval, or
    /// exclusive height expiry is invalid.
    pub fn verify_for_execution(
        &self,
        network_id: &NetworkId,
        canary_authority: &AccountId,
        block_time_unix_ms: u64,
        block_height: u64,
    ) -> Result<(), KagemushaV4TairaCanaryEvidenceValidationError> {
        self.verify_structure_and_signature()?;
        if &self.body.binding.network_id != network_id
            || &self.body.canary_authority != canary_authority
        {
            return Err(KagemushaV4TairaCanaryEvidenceValidationError::ActivationBinding);
        }
        if block_time_unix_ms < self.body.authorized_at_unix_ms
            || block_time_unix_ms >= self.body.expires_at_unix_ms
            || block_height >= self.body.expires_at_height.get()
        {
            return Err(KagemushaV4TairaCanaryEvidenceValidationError::AuthorizationExpired);
        }
        Ok(())
    }

    fn verify_bound(
        &self,
        expectations: &KagemushaV4ActivationReceiptExpectationsV1,
        receipt: &KagemushaV4ActivationFinalityReceiptV1,
        exact_receipt_bytes: &[u8],
        verification_time_unix_ms: u64,
    ) -> Result<u64, KagemushaV4TairaCanaryEvidenceValidationError> {
        self.verify_structure_and_signature()?;
        let activation_finalized_height =
            validate_permit_binding(&self.body, expectations, receipt, exact_receipt_bytes)?;
        if verification_time_unix_ms < self.body.authorized_at_unix_ms
            || verification_time_unix_ms >= self.body.expires_at_unix_ms
        {
            return Err(KagemushaV4TairaCanaryEvidenceValidationError::AuthorizationExpired);
        }
        Ok(activation_finalized_height)
    }
}

/// Exact-hash projection signed before the full canary transaction is disclosed.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4TairaCanaryReservationBodyV1 {
    /// Exact reservation-body schema.
    pub schema: String,
    /// Reservation-body version.
    pub version: u16,
    /// Controller permit embedded byte-identically in the canary instruction.
    pub permit: KagemushaV4TairaCanaryPermitV1,
    /// Exact signature-bound canary transaction intent.
    pub canary_transaction_intent: HashOf<SignedTransaction>,
    /// Digest of the complete canonical signed transaction wire.
    pub canary_transaction_wire: KagemushaExactBytesDigestV1,
    /// Exact external entrypoint hash later exposed as Core's transaction call hash.
    pub canary_entrypoint_hash: Hash,
}

impl KagemushaV4TairaCanaryReservationBodyV1 {
    /// Return the domain-separated typed hash signed for on-chain reservation.
    #[must_use]
    pub fn signing_hash(&self) -> HashOf<Self> {
        HashOf::from_untyped_unchecked(Hash::new_from_chunks(&[
            KAGEMUSHA_V4_TAIRA_CANARY_RESERVATION_SIGNATURE_DOMAIN,
            &self.encode(),
        ]))
    }

    fn validate_structure(&self) -> Result<(), KagemushaV4TairaCanaryEvidenceValidationError> {
        if self.schema != KAGEMUSHA_V4_TAIRA_CANARY_RESERVATION_BODY_SCHEMA
            || self.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
            || hash_is_zero(self.canary_transaction_intent.as_ref())
            || hash_is_zero(self.canary_entrypoint_hash.as_ref())
            || Hash::from(self.canary_transaction_intent) != self.canary_entrypoint_hash
        {
            return Err(KagemushaV4TairaCanaryEvidenceValidationError::Authorization);
        }
        validate_identity_digest(
            self.canary_transaction_wire,
            KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_MAX_BYTES,
            "taira_canary.canary_transaction_wire",
        )
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Authorization)
    }
}

/// Controller-signed exact-hash reservation safe to publish before the canary wire.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4TairaCanaryReservationV1 {
    /// Exact reservation schema.
    pub schema: String,
    /// Reservation version.
    pub version: u16,
    /// Exact-hash reservation body.
    pub body: KagemushaV4TairaCanaryReservationBodyV1,
    /// Promotion-controller signature over the complete reservation body.
    pub signature: SignatureOf<KagemushaV4TairaCanaryReservationBodyV1>,
}

impl KagemushaV4TairaCanaryReservationV1 {
    fn try_sign(
        permit: KagemushaV4TairaCanaryPermitV1,
        canary_transaction: &SignedTransaction,
        controller: &KeyPair,
    ) -> Result<Self, KagemushaV4TairaCanaryEvidenceValidationError> {
        permit.verify_structure_and_signature()?;
        if controller.public_key() != &permit.body.binding.promotion_controller {
            return Err(KagemushaV4TairaCanaryEvidenceValidationError::SignerMismatch);
        }
        let transaction_wire = canary_transaction
            .encode_wire_v1()
            .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::CanaryTransaction)?;
        let body = KagemushaV4TairaCanaryReservationBodyV1 {
            schema: KAGEMUSHA_V4_TAIRA_CANARY_RESERVATION_BODY_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            permit,
            canary_transaction_intent: canary_transaction.hash(),
            canary_transaction_wire: KagemushaExactBytesDigestV1::from_bytes(&transaction_wire)
                .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Encode)?,
            canary_entrypoint_hash: Hash::from(canary_transaction.hash_as_entrypoint()),
        };
        body.validate_structure()?;
        let signature = SignatureOf::try_from_hash(controller.private_key(), body.signing_hash())
            .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Signature)?;
        let reservation = Self {
            schema: KAGEMUSHA_V4_TAIRA_CANARY_RESERVATION_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            body,
            signature,
        };
        enforce_artifact_size(
            &reservation,
            KAGEMUSHA_V4_TAIRA_CANARY_RESERVATION_MAX_BYTES,
        )?;
        Ok(reservation)
    }

    /// Verify one bounded reservation at its deterministic consensus execution point.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4TairaCanaryEvidenceValidationError`] when either
    /// controller signature, hash identity, network, actual authorizer, or
    /// exclusive permit bound is invalid.
    pub fn verify_for_execution(
        &self,
        network_id: &NetworkId,
        authorizer: &AccountId,
        block_time_unix_ms: u64,
        block_height: u64,
    ) -> Result<(), KagemushaV4TairaCanaryEvidenceValidationError> {
        self.verify_structure_and_signature()?;
        self.body.permit.verify_for_execution(
            network_id,
            authorizer,
            block_time_unix_ms,
            block_height,
        )
    }

    /// Borrow the controller permit carried by this reservation and exact canary.
    #[must_use]
    pub const fn permit(&self) -> &KagemushaV4TairaCanaryPermitV1 {
        &self.body.permit
    }

    fn verify_structure_and_signature(
        &self,
    ) -> Result<(), KagemushaV4TairaCanaryEvidenceValidationError> {
        enforce_artifact_size(self, KAGEMUSHA_V4_TAIRA_CANARY_RESERVATION_MAX_BYTES)?;
        self.body.validate_structure()?;
        self.body.permit.verify_structure_and_signature()?;
        if self.schema != KAGEMUSHA_V4_TAIRA_CANARY_RESERVATION_SCHEMA
            || self.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
        {
            return Err(KagemushaV4TairaCanaryEvidenceValidationError::Authorization);
        }
        verify_reservation_signature(
            &self.signature,
            &self.body.permit.body.binding.promotion_controller,
            self.body.signing_hash(),
        )
    }
}

/// Exact fee-bearing transaction package independently signed by the controller.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4TairaCanaryAuthorizationPackageV1 {
    /// Exact package schema.
    pub schema: String,
    /// Package version.
    pub version: u16,
    /// Signed pre-disclosure reservation for the exact transaction identities.
    pub reservation: KagemushaV4TairaCanaryReservationV1,
    /// One exact, already signed canary transaction.
    pub canary_transaction: SignedTransaction,
}

impl KagemushaV4TairaCanaryAuthorizationPackageV1 {
    /// Return the domain-separated typed hash authorizing this exact transaction package.
    #[must_use]
    pub fn signing_hash(&self) -> HashOf<Self> {
        HashOf::from_untyped_unchecked(Hash::new_from_chunks(&[
            KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_SIGNATURE_DOMAIN,
            &self.encode(),
        ]))
    }
}

/// Controller-signed exact transaction package carrying its pre-disclosure reservation.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4TairaCanaryAuthorizationV1 {
    /// Exact authorization-package schema.
    pub schema: String,
    /// Authorization-package version.
    pub version: u16,
    /// Controller-signed pre-disclosure exact-hash reservation.
    pub reservation: KagemushaV4TairaCanaryReservationV1,
    /// One exact, already signed canary transaction.
    pub canary_transaction: SignedTransaction,
    /// Promotion-controller signature authorizing the exact package and wire.
    pub signature: SignatureOf<KagemushaV4TairaCanaryAuthorizationPackageV1>,
}

impl KagemushaV4TairaCanaryAuthorizationV1 {
    /// Package one signed transaction around its pre-commit permit.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4TairaCanaryEvidenceValidationError`] when the
    /// permit, transaction, activation binding, or wire identity is invalid.
    pub fn try_sign(
        permit: KagemushaV4TairaCanaryPermitV1,
        canary_transaction: SignedTransaction,
        controller: &KeyPair,
        expectations: &KagemushaV4ActivationReceiptExpectationsV1,
        receipt: &KagemushaV4ActivationFinalityReceiptV1,
        exact_receipt_bytes: &[u8],
    ) -> Result<Self, KagemushaV4TairaCanaryEvidenceValidationError> {
        let transaction_time = duration_millis(canary_transaction.creation_time())?;
        permit.verify_bound(expectations, receipt, exact_receipt_bytes, transaction_time)?;
        let reservation =
            KagemushaV4TairaCanaryReservationV1::try_sign(permit, &canary_transaction, controller)?;
        let package = KagemushaV4TairaCanaryAuthorizationPackageV1 {
            schema: KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_PACKAGE_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            reservation,
            canary_transaction,
        };
        validate_canary_transaction(&package)?;
        let signature =
            SignatureOf::try_from_hash(controller.private_key(), package.signing_hash())
                .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Signature)?;
        let artifact = Self {
            schema: KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            reservation: package.reservation,
            canary_transaction: package.canary_transaction,
            signature,
        };
        enforce_artifact_size(&artifact, KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_MAX_BYTES)?;
        Ok(artifact)
    }

    /// Decode one exact canonical bounded authorization package.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4TairaCanaryEvidenceValidationError`] for empty,
    /// oversized, non-canonical, or internally inconsistent bytes.
    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, KagemushaV4TairaCanaryEvidenceValidationError> {
        check_input_size(bytes, KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_MAX_BYTES)?;
        let artifact: Self = norito::decode_canonical_with_limits(
            bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Decode)?;
        if artifact.schema != KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_SCHEMA
            || artifact.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
            || norito::encode_canonical(&artifact)
                .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Encode)?
                != bytes
        {
            return Err(KagemushaV4TairaCanaryEvidenceValidationError::Decode);
        }
        artifact.verify_structure_and_signatures()?;
        validate_canary_transaction(&artifact.signed_package())?;
        Ok(artifact)
    }

    /// Reverify exact package bytes, embedded permit, activation receipt, and transaction.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4TairaCanaryEvidenceValidationError`] on any mismatch or when
    /// `verification_time_unix_ms` lies outside the signed interval.
    pub fn verify_exact(
        &self,
        exact_authorization_bytes: &[u8],
        expectations: &KagemushaV4ActivationReceiptExpectationsV1,
        receipt: &KagemushaV4ActivationFinalityReceiptV1,
        exact_receipt_bytes: &[u8],
        verification_time_unix_ms: u64,
    ) -> Result<
        KagemushaV4VerifiedTairaCanaryAuthorizationV1,
        KagemushaV4TairaCanaryEvidenceValidationError,
    > {
        check_input_size(
            exact_authorization_bytes,
            KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_MAX_BYTES,
        )?;
        enforce_artifact_size(self, KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_MAX_BYTES)?;
        if self.schema != KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_SCHEMA
            || self.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
            || norito::encode_canonical(self)
                .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Encode)?
                != exact_authorization_bytes
        {
            return Err(KagemushaV4TairaCanaryEvidenceValidationError::Decode);
        }
        self.verify_structure_and_signatures()?;
        let permit = self.reservation.permit();
        let activation_finalized_height = permit.verify_bound(
            expectations,
            receipt,
            exact_receipt_bytes,
            verification_time_unix_ms,
        )?;
        let expires_at_height = validate_canary_transaction(&self.signed_package())?;
        let body = &permit.body;
        Ok(KagemushaV4VerifiedTairaCanaryAuthorizationV1 {
            promotion_id: body.binding.promotion_id,
            network_id: body.binding.network_id,
            canonical_torii_origin: body.canonical_torii_origin.clone(),
            authorized_at_unix_ms: body.authorized_at_unix_ms,
            expires_at_unix_ms: body.expires_at_unix_ms,
            expires_at_height,
            activation_finalized_height,
            activation_finality_receipt: body.activation_finality_receipt,
            authorization_identity: KagemushaExactBytesDigestV1::from_bytes(
                exact_authorization_bytes,
            )
            .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Encode)?,
            canary_transaction: self.canary_transaction.clone(),
            canary_transaction_intent: self.reservation.body.canary_transaction_intent,
            canary_transaction_wire: self.reservation.body.canary_transaction_wire,
        })
    }

    /// Verify the full signed package before publishing its reservation.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4TairaCanaryEvidenceValidationError`] when either
    /// controller signature, exact transaction identity, network, actual
    /// authorizer, or the permit's exclusive bounds fail.
    pub fn verify_for_authorization_execution(
        &self,
        network_id: &NetworkId,
        authorizer: &AccountId,
        block_time_unix_ms: u64,
        block_height: u64,
    ) -> Result<(), KagemushaV4TairaCanaryEvidenceValidationError> {
        enforce_artifact_size(self, KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_MAX_BYTES)?;
        if self.schema != KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_SCHEMA
            || self.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
        {
            return Err(KagemushaV4TairaCanaryEvidenceValidationError::Authorization);
        }
        self.verify_structure_and_signatures()?;
        self.reservation.verify_for_execution(
            network_id,
            authorizer,
            block_time_unix_ms,
            block_height,
        )?;
        validate_canary_transaction(&self.signed_package())?;
        Ok(())
    }

    /// Borrow the pre-disclosure reservation published by the authorization step.
    #[must_use]
    pub const fn reservation(&self) -> &KagemushaV4TairaCanaryReservationV1 {
        &self.reservation
    }

    /// Borrow the controller permit carried by this package and transaction.
    #[must_use]
    pub const fn permit(&self) -> &KagemushaV4TairaCanaryPermitV1 {
        self.reservation.permit()
    }

    fn signed_package(&self) -> KagemushaV4TairaCanaryAuthorizationPackageV1 {
        KagemushaV4TairaCanaryAuthorizationPackageV1 {
            schema: KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_PACKAGE_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            reservation: self.reservation.clone(),
            canary_transaction: self.canary_transaction.clone(),
        }
    }

    fn verify_structure_and_signatures(
        &self,
    ) -> Result<(), KagemushaV4TairaCanaryEvidenceValidationError> {
        self.reservation.verify_structure_and_signature()?;
        let package = self.signed_package();
        verify_authorization_package_signature(
            &self.signature,
            &self
                .reservation
                .body
                .permit
                .body
                .binding
                .promotion_controller,
            package.signing_hash(),
        )
    }
}

/// Capability returned only after exact authorization verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KagemushaV4VerifiedTairaCanaryAuthorizationV1 {
    promotion_id: [u8; 32],
    network_id: NetworkId,
    canonical_torii_origin: String,
    authorized_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    expires_at_height: NonZeroU64,
    activation_finalized_height: u64,
    activation_finality_receipt: KagemushaExactBytesDigestV1,
    authorization_identity: KagemushaExactBytesDigestV1,
    canary_transaction: SignedTransaction,
    canary_transaction_intent: HashOf<SignedTransaction>,
    canary_transaction_wire: KagemushaExactBytesDigestV1,
}

impl KagemushaV4VerifiedTairaCanaryAuthorizationV1 {
    /// Return the authenticated promotion id.
    #[must_use]
    pub const fn promotion_id(&self) -> [u8; 32] {
        self.promotion_id
    }

    /// Return the authenticated network id.
    #[must_use]
    pub const fn network_id(&self) -> &NetworkId {
        &self.network_id
    }

    /// Return the authenticated exact Torii origin.
    #[must_use]
    pub fn canonical_torii_origin(&self) -> &str {
        &self.canonical_torii_origin
    }

    /// Return the signed interval start.
    #[must_use]
    pub const fn authorized_at_unix_ms(&self) -> u64 {
        self.authorized_at_unix_ms
    }

    /// Return the signed exclusive interval end.
    #[must_use]
    pub const fn expires_at_unix_ms(&self) -> u64 {
        self.expires_at_unix_ms
    }

    /// Return the exact transaction height deadline.
    #[must_use]
    pub const fn expires_at_height(&self) -> NonZeroU64 {
        self.expires_at_height
    }

    /// Return the authenticated activation receipt height.
    #[must_use]
    pub const fn activation_finalized_height(&self) -> u64 {
        self.activation_finalized_height
    }

    /// Return the exact activation-receipt identity.
    #[must_use]
    pub const fn activation_finality_receipt(&self) -> KagemushaExactBytesDigestV1 {
        self.activation_finality_receipt
    }

    /// Return the exact authorization-artifact identity.
    #[must_use]
    pub const fn authorization_identity(&self) -> KagemushaExactBytesDigestV1 {
        self.authorization_identity
    }

    /// Borrow the exact signed canary transaction.
    #[must_use]
    pub const fn canary_transaction(&self) -> &SignedTransaction {
        &self.canary_transaction
    }

    /// Return the canary transaction intent.
    #[must_use]
    pub const fn canary_transaction_intent(&self) -> HashOf<SignedTransaction> {
        self.canary_transaction_intent
    }

    /// Return the exact complete transaction-wire identity.
    #[must_use]
    pub const fn canary_transaction_wire(&self) -> KagemushaExactBytesDigestV1 {
        self.canary_transaction_wire
    }
}

/// Exact diagnostic observations made after the canary transaction finalized.
///
/// These observations aid incident review; the embedded committed transaction, block wire, and
/// cryptographic finality extension are the authoritative production evidence.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4TairaCanaryQueryObservationV1 {
    /// Protected-host wall clock immediately before the first live query.
    pub query_started_at_unix_ms: u64,
    /// Protected-host wall clock immediately after the last live query.
    pub query_completed_at_unix_ms: u64,
    /// Exact canonical global pipeline-status response identity.
    pub pipeline_status_response_norito: KagemushaExactBytesDigestV1,
    /// Scope returned by the pipeline-status endpoint; must be `global`.
    pub pipeline_status_scope: String,
    /// Resolution source returned by the pipeline-status endpoint; must be `state`.
    pub pipeline_status_resolved_from: String,
    /// Exact canary transaction intent named by the status response.
    pub pipeline_transaction_intent: HashOf<SignedTransaction>,
    /// Terminal status kind returned by the pipeline endpoint; must be `Applied`.
    pub pipeline_status_kind: String,
    /// Exact carrier height returned with the Applied status.
    pub pipeline_status_block_height: u64,
    /// Exact canonical authenticated transaction-details response identity.
    pub transaction_details_response_norito: KagemushaExactBytesDigestV1,
    /// Trigger completions carried by that exact details response.
    pub transaction_details_trigger_completion_count: u32,
    /// Exact canonical node-status response captured before transaction queries.
    pub node_status_before_norito: KagemushaExactBytesDigestV1,
    /// Node-reported observation time for the before snapshot.
    pub node_status_before_observed_at_ms: u64,
    /// Node-reported committed height for the before snapshot.
    pub node_status_before_height: u64,
    /// Exact canonical node-status response captured after finality queries.
    pub node_status_after_norito: KagemushaExactBytesDigestV1,
    /// Node-reported observation time for the after snapshot.
    pub node_status_after_observed_at_ms: u64,
    /// Node-reported committed height for the after snapshot.
    pub node_status_after_height: u64,
    /// Exact canonical embedded committed-transaction identity.
    pub committed_transaction_norito: KagemushaExactBytesDigestV1,
    /// Exact canonical embedded `SignedBlockWire` identity.
    pub finalized_block_wire: KagemushaExactBytesDigestV1,
    /// Exact canonical embedded successor-proof-chain identity.
    pub finality_proof_chain_norito: KagemushaExactBytesDigestV1,
    /// Number of independently fetched post-receipt successor proofs.
    pub finality_proof_count: u32,
}

impl KagemushaV4TairaCanaryQueryObservationV1 {
    fn validate(
        &self,
        canary_transaction_intent: HashOf<SignedTransaction>,
        finalized_height: u64,
        finalized_block_time_unix_ms: u64,
    ) -> Result<(), KagemushaV4TairaCanaryEvidenceValidationError> {
        if self.query_started_at_unix_ms <= finalized_block_time_unix_ms
            || self.query_completed_at_unix_ms < self.query_started_at_unix_ms
            || self
                .query_completed_at_unix_ms
                .saturating_sub(self.query_started_at_unix_ms)
                > KAGEMUSHA_V4_TAIRA_CANARY_QUERY_MAX_WINDOW_MS
            || self.pipeline_status_scope != "global"
            || self.pipeline_status_resolved_from != "state"
            || self.pipeline_transaction_intent != canary_transaction_intent
            || self.pipeline_status_kind != "Applied"
            || self.pipeline_status_block_height != finalized_height
            || self.transaction_details_trigger_completion_count != 0
            || self.node_status_before_height < finalized_height
            || self.node_status_after_height < self.node_status_before_height
            || self.node_status_after_observed_at_ms < self.node_status_before_observed_at_ms
            || self.finality_proof_count == 0
            || usize::try_from(self.finality_proof_count).map_or(true, |count| {
                count > KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1
            })
        {
            return Err(KagemushaV4TairaCanaryEvidenceValidationError::QueryEvidence);
        }
        let earliest = self
            .query_started_at_unix_ms
            .saturating_sub(KAGEMUSHA_V4_TAIRA_CANARY_QUERY_CLOCK_SKEW_MS);
        let latest = self
            .query_completed_at_unix_ms
            .saturating_add(KAGEMUSHA_V4_TAIRA_CANARY_QUERY_CLOCK_SKEW_MS);
        if !(earliest..=latest).contains(&self.node_status_before_observed_at_ms)
            || !(earliest..=latest).contains(&self.node_status_after_observed_at_ms)
        {
            return Err(KagemushaV4TairaCanaryEvidenceValidationError::QueryEvidence);
        }
        let artifact_maximum = u64::try_from(KAGEMUSHA_V4_TAIRA_CANARY_EVIDENCE_MAX_BYTES)
            .expect("evidence bound fits u64");
        for (digest, maximum) in [
            (
                self.pipeline_status_response_norito,
                QUERY_STATUS_RESPONSE_MAX_BYTES,
            ),
            (self.transaction_details_response_norito, artifact_maximum),
            (self.node_status_before_norito, QUERY_NODE_STATUS_MAX_BYTES),
            (self.node_status_after_norito, QUERY_NODE_STATUS_MAX_BYTES),
            (self.committed_transaction_norito, artifact_maximum),
            (self.finalized_block_wire, artifact_maximum),
            (self.finality_proof_chain_norito, artifact_maximum),
        ] {
            digest
                .validate()
                .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::QueryEvidence)?;
            if digest.byte_len > maximum {
                return Err(KagemushaV4TairaCanaryEvidenceValidationError::QueryEvidence);
            }
        }
        Ok(())
    }
}

/// Issuer-signed body containing complete production canary finality evidence.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4TairaCanaryEvidenceBodyV1 {
    /// Exact evidence-body schema.
    pub schema: String,
    /// Evidence-body version.
    pub version: u16,
    /// Independently pinned promotion controller.
    pub promotion_controller: PublicKey,
    /// Non-zero promotion id derived from the reservation.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub promotion_id: [u8; 32],
    /// Exact genesis-derived network identity.
    pub network_id: NetworkId,
    /// Exact controller-signed promotion-reservation identity.
    pub promotion_reservation: KagemushaExactBytesDigestV1,
    /// Exact controller-signed activation-expectations identity.
    pub activation_expectations_artifact: KagemushaExactBytesDigestV1,
    /// Exact issuer-signed activation-finality receipt identity.
    pub activation_finality_receipt: KagemushaExactBytesDigestV1,
    /// Exact controller-signed canary-authorization identity.
    pub canary_authorization: KagemushaExactBytesDigestV1,
    /// Independent receipt issuer that also signs this evidence.
    pub issuer: PublicKey,
    /// Activation transaction authenticated by the receipt.
    pub activation_transaction_intent: HashOf<SignedTransaction>,
    /// Activation carrier height authenticated by the receipt.
    pub activation_finalized_height: u64,
    /// Activation carrier hash authenticated by the receipt.
    pub activation_finalized_block_hash: HashOf<BlockHeader>,
    /// Exact payload-only canary transaction intent.
    pub canary_transaction_intent: HashOf<SignedTransaction>,
    /// Exact authorization-bearing canary transaction wire identity.
    pub canary_transaction_wire: KagemushaExactBytesDigestV1,
    /// Full successful canary transaction and its Merkle inclusion proofs.
    pub committed_transaction: CommittedTransaction,
    /// Exact canonical canary carrier `SignedBlockWire` bytes.
    pub finalized_block_wire: KagemushaFinalizedBlockWireV1,
    /// Digest of those exact canonical block bytes.
    pub finalized_block_wire_digest: KagemushaExactBytesDigestV1,
    /// Non-empty contiguous finality extension after the receipt terminal proof.
    pub finality_proof_chain: KagemushaV4ActivationFinalityProofChainV1,
    /// Finalized canary carrier height.
    pub finalized_height: u64,
    /// Finalized canary carrier hash.
    pub finalized_block_hash: HashOf<BlockHeader>,
    /// Diagnostic live-query observations collected after finalization.
    pub query: KagemushaV4TairaCanaryQueryObservationV1,
}

impl KagemushaV4TairaCanaryEvidenceBodyV1 {
    /// Return the domain-separated typed hash signed by the independent issuer.
    #[must_use]
    pub fn signing_hash(&self) -> HashOf<Self> {
        HashOf::from_untyped_unchecked(Hash::new_from_chunks(&[
            KAGEMUSHA_V4_TAIRA_CANARY_EVIDENCE_SIGNATURE_DOMAIN,
            &self.encode(),
        ]))
    }

    fn validate_structure(&self) -> Result<(), KagemushaV4TairaCanaryEvidenceValidationError> {
        if self.schema != KAGEMUSHA_V4_TAIRA_CANARY_EVIDENCE_BODY_SCHEMA
            || self.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
            || !matches!(
                self.promotion_controller.try_algorithm(),
                Ok(Algorithm::Ed25519)
            )
            || !supports_evidence_signature_algorithm(&self.issuer)
            || self.promotion_controller == self.issuer
            || self.promotion_id == [0; 32]
            || self.network_id.as_bytes().iter().all(|byte| *byte == 0)
            || self.activation_finalized_height == 0
            || self.finalized_height <= self.activation_finalized_height
            || hash_is_zero(self.activation_transaction_intent.as_ref())
            || hash_is_zero(self.canary_transaction_intent.as_ref())
            || hash_is_zero(self.activation_finalized_block_hash.as_ref().as_ref())
            || hash_is_zero(self.finalized_block_hash.as_ref().as_ref())
            || self.finality_proof_chain.is_empty()
        {
            return Err(KagemushaV4TairaCanaryEvidenceValidationError::InvalidField(
                "taira_canary.evidence_body",
            ));
        }
        validate_identity_digest(
            self.promotion_reservation,
            KAGEMUSHA_V4_PROMOTION_RESERVATION_MAX_BYTES,
            "taira_canary.promotion_reservation",
        )?;
        validate_identity_digest(
            self.activation_expectations_artifact,
            KAGEMUSHA_V4_ACTIVATION_EXPECTATIONS_MAX_BYTES,
            "taira_canary.activation_expectations",
        )?;
        validate_identity_digest(
            self.activation_finality_receipt,
            KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_BYTES,
            "taira_canary.activation_receipt",
        )?;
        validate_identity_digest(
            self.canary_authorization,
            KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_MAX_BYTES,
            "taira_canary.authorization",
        )?;
        self.canary_transaction_wire.validate().map_err(|_| {
            KagemushaV4TairaCanaryEvidenceValidationError::InvalidField(
                "taira_canary.canary_transaction_wire",
            )
        })?;
        self.finalized_block_wire_digest.validate().map_err(|_| {
            KagemushaV4TairaCanaryEvidenceValidationError::InvalidField(
                "taira_canary.finalized_block_wire",
            )
        })?;
        if !self
            .finalized_block_wire_digest
            .matches_bytes(self.finalized_block_wire.as_bytes())
        {
            return Err(KagemushaV4TairaCanaryEvidenceValidationError::BlockBinding);
        }
        Ok(())
    }
}

/// Issuer-signed, promotion-bound evidence of an actual post-activation canary commit.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaV4TairaCanaryEvidenceV1 {
    /// Exact evidence schema.
    pub schema: String,
    /// Evidence version.
    pub version: u16,
    /// Signed evidence statement.
    pub body: KagemushaV4TairaCanaryEvidenceBodyV1,
    /// Independent issuer signature.
    pub signature: SignatureOf<KagemushaV4TairaCanaryEvidenceBodyV1>,
}

impl KagemushaV4TairaCanaryEvidenceV1 {
    /// Verify all embedded production evidence and sign it with the receipt issuer.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4TairaCanaryEvidenceValidationError`] for any malformed,
    /// unauthenticated, spliced, unsuccessful, non-final, signer-mismatched, or oversized input.
    #[allow(clippy::too_many_arguments)]
    pub fn try_sign(
        body: KagemushaV4TairaCanaryEvidenceBodyV1,
        issuer: &KeyPair,
        authorization: &KagemushaV4TairaCanaryAuthorizationV1,
        exact_authorization_bytes: &[u8],
        expectations: &KagemushaV4ActivationReceiptExpectationsV1,
        receipt: &KagemushaV4ActivationFinalityReceiptV1,
        exact_receipt_bytes: &[u8],
    ) -> Result<Self, KagemushaV4TairaCanaryEvidenceValidationError> {
        body.validate_structure()?;
        enforce_artifact_size(&body, KAGEMUSHA_V4_TAIRA_CANARY_EVIDENCE_MAX_BYTES)?;
        verify_evidence_body(
            &body,
            EvidenceVerificationInputs {
                authorization,
                exact_authorization_bytes,
                expectations,
                receipt,
                exact_receipt_bytes,
            },
        )?;
        if issuer.public_key() != &body.issuer {
            return Err(KagemushaV4TairaCanaryEvidenceValidationError::SignerMismatch);
        }
        let signature = SignatureOf::try_from_hash(issuer.private_key(), body.signing_hash())
            .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Signature)?;
        let artifact = Self {
            schema: KAGEMUSHA_V4_TAIRA_CANARY_EVIDENCE_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            body,
            signature,
        };
        enforce_artifact_size(&artifact, KAGEMUSHA_V4_TAIRA_CANARY_EVIDENCE_MAX_BYTES)?;
        Ok(artifact)
    }

    /// Decode one exact canonical bounded artifact.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4TairaCanaryEvidenceValidationError`] for empty, oversized,
    /// non-canonical, or structurally invalid bytes.
    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, KagemushaV4TairaCanaryEvidenceValidationError> {
        check_input_size(bytes, KAGEMUSHA_V4_TAIRA_CANARY_EVIDENCE_MAX_BYTES)?;
        let artifact: Self = norito::decode_canonical_with_limits(
            bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Decode)?;
        artifact.body.validate_structure()?;
        if artifact.schema != KAGEMUSHA_V4_TAIRA_CANARY_EVIDENCE_SCHEMA
            || artifact.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
            || norito::encode_canonical(&artifact)
                .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Encode)?
                != bytes
        {
            return Err(KagemushaV4TairaCanaryEvidenceValidationError::Decode);
        }
        Ok(artifact)
    }

    /// Verify exact encoding, issuer signature, authorization, commit, block, and finality chain.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4TairaCanaryEvidenceValidationError`] on any exact-byte,
    /// activation, authorization, execution, block, finality, or query mismatch.
    #[allow(clippy::too_many_arguments)]
    pub fn verify_exact(
        &self,
        exact_evidence_bytes: &[u8],
        authorization: &KagemushaV4TairaCanaryAuthorizationV1,
        exact_authorization_bytes: &[u8],
        expectations: &KagemushaV4ActivationReceiptExpectationsV1,
        receipt: &KagemushaV4ActivationFinalityReceiptV1,
        exact_receipt_bytes: &[u8],
    ) -> Result<
        KagemushaV4VerifiedTairaCanaryEvidenceV1,
        KagemushaV4TairaCanaryEvidenceValidationError,
    > {
        check_input_size(
            exact_evidence_bytes,
            KAGEMUSHA_V4_TAIRA_CANARY_EVIDENCE_MAX_BYTES,
        )?;
        enforce_artifact_size(self, KAGEMUSHA_V4_TAIRA_CANARY_EVIDENCE_MAX_BYTES)?;
        self.body.validate_structure()?;
        if self.schema != KAGEMUSHA_V4_TAIRA_CANARY_EVIDENCE_SCHEMA
            || self.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
            || self.body.issuer != *expectations.receipt_issuer()
            || norito::encode_canonical(self)
                .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Encode)?
                != exact_evidence_bytes
        {
            return Err(KagemushaV4TairaCanaryEvidenceValidationError::Decode);
        }
        verify_evidence_signature(&self.signature, &self.body.issuer, self.body.signing_hash())?;
        let verified = verify_evidence_body(
            &self.body,
            EvidenceVerificationInputs {
                authorization,
                exact_authorization_bytes,
                expectations,
                receipt,
                exact_receipt_bytes,
            },
        )?;
        Ok(verified)
    }
}

/// Capability returned only after complete production canary verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KagemushaV4VerifiedTairaCanaryEvidenceV1 {
    promotion_id: [u8; 32],
    activation_expectations_artifact: KagemushaExactBytesDigestV1,
    activation_finality_receipt: KagemushaExactBytesDigestV1,
    authorization_identity: KagemushaExactBytesDigestV1,
    activation_finalized_height: u64,
    activation_finalized_block_hash: HashOf<BlockHeader>,
    activation_transaction_intent: HashOf<SignedTransaction>,
    finalized_height: u64,
    finalized_block_hash: HashOf<BlockHeader>,
    canary_transaction_intent: HashOf<SignedTransaction>,
    canary_transaction_wire: KagemushaExactBytesDigestV1,
}

impl KagemushaV4VerifiedTairaCanaryEvidenceV1 {
    /// Return the authenticated promotion id.
    #[must_use]
    pub const fn promotion_id(&self) -> [u8; 32] {
        self.promotion_id
    }

    /// Return the exact authenticated activation-expectations artifact identity.
    #[must_use]
    pub const fn activation_expectations_artifact(&self) -> KagemushaExactBytesDigestV1 {
        self.activation_expectations_artifact
    }

    /// Return the exact activation-finality receipt identity.
    #[must_use]
    pub const fn activation_finality_receipt(&self) -> KagemushaExactBytesDigestV1 {
        self.activation_finality_receipt
    }

    /// Return the exact controller authorization identity.
    #[must_use]
    pub const fn authorization_identity(&self) -> KagemushaExactBytesDigestV1 {
        self.authorization_identity
    }

    /// Return the activation receipt carrier height.
    #[must_use]
    pub const fn activation_finalized_height(&self) -> u64 {
        self.activation_finalized_height
    }

    /// Return the activation receipt carrier hash.
    #[must_use]
    pub const fn activation_finalized_block_hash(&self) -> HashOf<BlockHeader> {
        self.activation_finalized_block_hash
    }

    /// Return the activation transaction intent.
    #[must_use]
    pub const fn activation_transaction_intent(&self) -> HashOf<SignedTransaction> {
        self.activation_transaction_intent
    }

    /// Return the exact finalized canary height.
    #[must_use]
    pub const fn finalized_height(&self) -> u64 {
        self.finalized_height
    }

    /// Return the exact finalized canary block hash.
    #[must_use]
    pub const fn finalized_block_hash(&self) -> HashOf<BlockHeader> {
        self.finalized_block_hash
    }

    /// Return the exact canary transaction intent.
    #[must_use]
    pub const fn canary_transaction_intent(&self) -> HashOf<SignedTransaction> {
        self.canary_transaction_intent
    }

    /// Return the exact authorization-bearing canary transaction wire identity.
    #[must_use]
    pub const fn canary_transaction_wire(&self) -> KagemushaExactBytesDigestV1 {
        self.canary_transaction_wire
    }
}

fn validate_canary_transaction(
    authorization: &KagemushaV4TairaCanaryAuthorizationPackageV1,
) -> Result<NonZeroU64, KagemushaV4TairaCanaryEvidenceValidationError> {
    if authorization.schema != KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_PACKAGE_SCHEMA
        || authorization.version != KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION
    {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::Authorization);
    }
    authorization.reservation.verify_structure_and_signature()?;
    let reservation = &authorization.reservation.body;
    let permit = &reservation.permit;
    let body = &permit.body;
    let transaction = &authorization.canary_transaction;
    if transaction.network_id() != Some(&body.binding.network_id)
        || transaction.authority() != &body.canary_authority
        || transaction.nonce().is_none()
        || transaction.attachments().is_some()
        || transaction.admission_intent() != TransactionAdmissionIntent::Ordinary
    {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::CanaryTransaction);
    }
    transaction
        .verify_signature()
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::CanaryTransaction)?;
    if transaction.hash() != reservation.canary_transaction_intent
        || Hash::from(transaction.hash_as_entrypoint()) != reservation.canary_entrypoint_hash
    {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::CanaryTransaction);
    }
    let creation_time_unix_ms = duration_millis(transaction.creation_time())?;
    let time_to_live_ms = transaction
        .time_to_live()
        .ok_or(KagemushaV4TairaCanaryEvidenceValidationError::CanaryTransaction)
        .and_then(duration_millis)?;
    let wall_expiry = creation_time_unix_ms
        .checked_add(time_to_live_ms)
        .ok_or(KagemushaV4TairaCanaryEvidenceValidationError::CanaryTransaction)?;
    if time_to_live_ms == 0
        || creation_time_unix_ms < body.authorized_at_unix_ms
        || creation_time_unix_ms >= body.expires_at_unix_ms
        || wall_expiry > body.expires_at_unix_ms
    {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::CanaryTransaction);
    }
    let expires_at_height = transaction
        .expires_at_height()
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::CanaryTransaction)?
        .and_then(NonZeroU64::new)
        .ok_or(KagemushaV4TairaCanaryEvidenceValidationError::CanaryTransaction)?;
    if expires_at_height != body.expires_at_height {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::CanaryTransaction);
    }
    let expected_metadata = kagemusha_v4_taira_canary_transaction_metadata(
        body.binding.promotion_id,
        body.activation_finality_receipt,
        &body.canonical_torii_origin,
        body.expires_at_height,
    );
    if transaction.metadata() != &expected_metadata {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::CanaryTransaction);
    }
    let Executable::Instructions(instructions) = transaction.instructions() else {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::CanaryTransaction);
    };
    let [instruction] = instructions.as_ref() else {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::CanaryTransaction);
    };
    let Some(record) = instruction
        .as_any()
        .downcast_ref::<RecordKagemushaTairaCanaryV4>()
    else {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::CanaryTransaction);
    };
    let embedded_permit = norito::encode_canonical(record.permit())
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::CanaryTransaction)?;
    let packaged_permit = norito::encode_canonical(permit)
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::CanaryTransaction)?;
    if record.permit() != permit || embedded_permit != packaged_permit {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::CanaryTransaction);
    }
    let transaction_wire = transaction
        .encode_wire_v1()
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::CanaryTransaction)?;
    reservation
        .canary_transaction_wire
        .validate()
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::CanaryTransaction)?;
    if !reservation
        .canary_transaction_wire
        .matches_bytes(&transaction_wire)
    {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::CanaryTransaction);
    }
    Ok(expires_at_height)
}

fn validate_permit_binding(
    body: &KagemushaV4TairaCanaryAuthorizationBodyV1,
    expectations: &KagemushaV4ActivationReceiptExpectationsV1,
    receipt: &KagemushaV4ActivationFinalityReceiptV1,
    exact_receipt_bytes: &[u8],
) -> Result<u64, KagemushaV4TairaCanaryEvidenceValidationError> {
    check_input_size(
        exact_receipt_bytes,
        KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_BYTES,
    )?;
    if norito::encode_canonical(receipt)
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::ActivationReceipt)?
        != exact_receipt_bytes
        || !body
            .activation_finality_receipt
            .matches_bytes(exact_receipt_bytes)
    {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::ActivationReceipt);
    }
    let verified_receipt = receipt
        .verify(expectations)
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::ActivationReceipt)?;
    if body.binding != *expectations.binding()
        || body.activation_expectations_artifact != expectations.activation_expectations_artifact()
    {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::ActivationBinding);
    }
    let activation_block =
        decode_exact_finalized_block(receipt.body.finalized_block_wire.as_bytes())
            .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::ActivationReceipt)?;
    let activation_block_time = duration_millis(activation_block.header().creation_time())?;
    if body.authorized_at_unix_ms <= activation_block_time {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::ActivationBinding);
    }
    let expires_at_height = body.expires_at_height.get();
    let maximum_expiry = verified_receipt
        .finalized_height()
        .checked_add(
            u64::try_from(KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1)
                .expect("proof bound fits u64"),
        )
        .and_then(|height| height.checked_add(1))
        .ok_or(KagemushaV4TairaCanaryEvidenceValidationError::CanaryTransaction)?;
    if expires_at_height <= verified_receipt.finalized_height().saturating_add(1)
        || expires_at_height > maximum_expiry
    {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::CanaryTransaction);
    }
    Ok(verified_receipt.finalized_height())
}

#[derive(Clone, Copy)]
struct EvidenceVerificationInputs<'a> {
    authorization: &'a KagemushaV4TairaCanaryAuthorizationV1,
    exact_authorization_bytes: &'a [u8],
    expectations: &'a KagemushaV4ActivationReceiptExpectationsV1,
    receipt: &'a KagemushaV4ActivationFinalityReceiptV1,
    exact_receipt_bytes: &'a [u8],
}

struct VerifiedEvidencePrerequisites {
    block: SignedBlock,
    block_time_unix_ms: u64,
    authorization: KagemushaV4VerifiedTairaCanaryAuthorizationV1,
    receipt: KagemushaV4VerifiedActivationReceiptV1,
    authorization_identity: KagemushaExactBytesDigestV1,
}

fn verify_evidence_body(
    body: &KagemushaV4TairaCanaryEvidenceBodyV1,
    inputs: EvidenceVerificationInputs<'_>,
) -> Result<KagemushaV4VerifiedTairaCanaryEvidenceV1, KagemushaV4TairaCanaryEvidenceValidationError>
{
    let verified = verify_evidence_prerequisites(body, &inputs)?;
    let authorization_identity = verified.authorization_identity;
    verify_evidence_block_binding(body, &verified)?;
    let authorized_wire = verify_committed_canary(body, &verified)?;
    verify_evidence_finality(
        body,
        inputs.receipt,
        inputs.expectations,
        &verified,
        &authorized_wire,
    )?;
    verify_query_evidence(body, verified.block_time_unix_ms)?;

    Ok(KagemushaV4VerifiedTairaCanaryEvidenceV1 {
        promotion_id: body.promotion_id,
        activation_expectations_artifact: body.activation_expectations_artifact,
        activation_finality_receipt: body.activation_finality_receipt,
        authorization_identity,
        activation_finalized_height: body.activation_finalized_height,
        activation_finalized_block_hash: body.activation_finalized_block_hash,
        activation_transaction_intent: body.activation_transaction_intent,
        finalized_height: body.finalized_height,
        finalized_block_hash: body.finalized_block_hash,
        canary_transaction_intent: body.canary_transaction_intent,
        canary_transaction_wire: body.canary_transaction_wire,
    })
}

fn verify_evidence_prerequisites(
    body: &KagemushaV4TairaCanaryEvidenceBodyV1,
    inputs: &EvidenceVerificationInputs<'_>,
) -> Result<VerifiedEvidencePrerequisites, KagemushaV4TairaCanaryEvidenceValidationError> {
    let EvidenceVerificationInputs {
        authorization,
        exact_authorization_bytes,
        expectations,
        receipt,
        exact_receipt_bytes,
    } = *inputs;
    body.validate_structure()?;
    let block = decode_exact_finalized_block(body.finalized_block_wire.as_bytes())
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::BlockBinding)?;
    let block_time_unix_ms = duration_millis(block.header().creation_time())?;
    let authorization = authorization.verify_exact(
        exact_authorization_bytes,
        expectations,
        receipt,
        exact_receipt_bytes,
        block_time_unix_ms,
    )?;
    let receipt = receipt
        .verify(expectations)
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::ActivationReceipt)?;
    let authorization_identity = KagemushaExactBytesDigestV1::from_bytes(exact_authorization_bytes)
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Authorization)?;
    let binding = expectations.binding();
    if body.promotion_controller != *expectations.promotion_controller()
        || body.promotion_id != binding.promotion_id
        || body.network_id != binding.network_id
        || body.promotion_reservation != expectations.promotion_reservation()
        || body.activation_expectations_artifact != expectations.activation_expectations_artifact()
        || !body
            .activation_finality_receipt
            .matches_bytes(exact_receipt_bytes)
        || body.canary_authorization != authorization_identity
        || body.issuer != *expectations.receipt_issuer()
        || body.activation_transaction_intent != receipt.activation_transaction_intent()
        || body.activation_finalized_height != receipt.finalized_height()
        || body.activation_finalized_block_hash != receipt.finalized_block_hash()
        || body.canary_transaction_intent != authorization.canary_transaction_intent()
        || body.canary_transaction_wire != authorization.canary_transaction_wire()
    {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::ActivationBinding);
    }
    Ok(VerifiedEvidencePrerequisites {
        block,
        block_time_unix_ms,
        authorization,
        receipt,
        authorization_identity,
    })
}

fn verify_evidence_block_binding(
    body: &KagemushaV4TairaCanaryEvidenceBodyV1,
    verified: &VerifiedEvidencePrerequisites,
) -> Result<(), KagemushaV4TairaCanaryEvidenceValidationError> {
    if body.finalized_height != verified.block.header().height().get()
        || body.finalized_block_hash != verified.block.hash()
        || body.finalized_height <= verified.receipt.finalized_height()
        || body.finalized_height >= verified.authorization.expires_at_height().get()
    {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::BlockBinding);
    }
    Ok(())
}

fn verify_committed_canary(
    body: &KagemushaV4TairaCanaryEvidenceBodyV1,
    verified: &VerifiedEvidencePrerequisites,
) -> Result<Vec<u8>, KagemushaV4TairaCanaryEvidenceValidationError> {
    let transaction = verified.authorization.canary_transaction();
    let transaction_creation = duration_millis(transaction.creation_time())?;
    let transaction_ttl = transaction
        .time_to_live()
        .ok_or(KagemushaV4TairaCanaryEvidenceValidationError::CanaryTransaction)
        .and_then(duration_millis)?;
    let transaction_wall_expiry = transaction_creation
        .checked_add(transaction_ttl)
        .ok_or(KagemushaV4TairaCanaryEvidenceValidationError::CanaryTransaction)?;
    if verified.block_time_unix_ms < transaction_creation
        || verified.block_time_unix_ms >= transaction_wall_expiry
        || verified.block_time_unix_ms < verified.authorization.authorized_at_unix_ms()
        || verified.block_time_unix_ms >= verified.authorization.expires_at_unix_ms()
    {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::AuthorizationExpired);
    }
    let committed = &body.committed_transaction;
    let TransactionEntrypoint::External(committed_transaction) = &committed.entrypoint else {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::CommittedTransaction);
    };
    if committed.merge_inclusion.is_some()
        || committed.result.0.is_err()
        || !committed.result.1.is_empty()
        || !committed.verify_inclusion_in_block(&verified.block)
        || committed_transaction.hash() != body.canary_transaction_intent
        || verified
            .block
            .entrypoints_cloned()
            .filter(|entrypoint| entrypoint.hash() == committed.entrypoint_hash)
            .count()
            != 1
    {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::CommittedTransaction);
    }
    committed_transaction
        .verify_signature()
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::CommittedTransaction)?;
    let committed_wire = committed_transaction
        .encode_wire_v1()
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::CommittedTransaction)?;
    let authorized_wire = transaction
        .encode_wire_v1()
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::CanaryTransaction)?;
    if committed_wire != authorized_wire
        || !body.canary_transaction_wire.matches_bytes(&committed_wire)
    {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::CommittedTransaction);
    }
    Ok(authorized_wire)
}

fn verify_evidence_finality(
    body: &KagemushaV4TairaCanaryEvidenceBodyV1,
    receipt: &KagemushaV4ActivationFinalityReceiptV1,
    expectations: &KagemushaV4ActivationReceiptExpectationsV1,
    verified: &VerifiedEvidencePrerequisites,
    authorized_wire: &[u8],
) -> Result<(), KagemushaV4TairaCanaryEvidenceValidationError> {
    let receipt_terminal = receipt
        .body
        .finality_proof_chain
        .last()
        .ok_or(KagemushaV4TairaCanaryEvidenceValidationError::Finality)?;
    let first_extension = body
        .finality_proof_chain
        .first()
        .ok_or(KagemushaV4TairaCanaryEvidenceValidationError::Finality)?;
    let final_extension = body
        .finality_proof_chain
        .last()
        .ok_or(KagemushaV4TairaCanaryEvidenceValidationError::Finality)?;
    let proof_count = u64::try_from(body.finality_proof_chain.len())
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Finality)?;
    if receipt_terminal.finality_artifact.height.checked_add(1)
        != Some(first_extension.finality_artifact.height)
        || receipt_terminal
            .finality_artifact
            .height
            .checked_add(proof_count)
            != Some(body.finalized_height)
        || final_extension.finality_artifact.height != body.finalized_height
        || final_extension.finality_artifact.block_hash != body.finalized_block_hash
    {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::Finality);
    }
    let binding = expectations.binding();
    let mut finality_verifier = BridgeFinalityVerifier::with_context(
        binding.network_id,
        expectations
            .trusted_finality_anchor()
            .finality_artifact
            .context_id(),
    );
    finality_verifier
        .verify(expectations.trusted_finality_anchor())
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Finality)?;
    for proof in &receipt.body.finality_proof_chain {
        validate_finality_corridor_context(proof, binding, expectations.validator_bodies())
            .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Finality)?;
        finality_verifier
            .verify(proof)
            .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Finality)?;
    }
    for proof in &body.finality_proof_chain {
        validate_finality_corridor_context(proof, binding, expectations.validator_bodies())
            .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Finality)?;
        finality_verifier
            .verify(proof)
            .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Finality)?;
    }
    let anchor = TrustedBlockProofAnchor::from_untrusted_finality_artifact(
        &verified.block,
        &final_extension.finality_artifact,
        &body.committed_transaction.entrypoint_hash,
    )
    .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::BlockBinding)?;
    let entry_index = usize::try_from(anchor.entry_index())
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::BlockBinding)?;
    let Some(TransactionEntrypoint::External(block_transaction)) =
        verified.block.entrypoints_cloned().nth(entry_index)
    else {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::BlockBinding);
    };
    if block_transaction
        .encode_wire_v1()
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::BlockBinding)?
        != authorized_wire
    {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::BlockBinding);
    }
    Ok(())
}

fn verify_query_evidence(
    body: &KagemushaV4TairaCanaryEvidenceBodyV1,
    block_time_unix_ms: u64,
) -> Result<(), KagemushaV4TairaCanaryEvidenceValidationError> {
    let committed_digest = canonical_digest(&body.committed_transaction)?;
    let proof_chain_digest = canonical_digest(&body.finality_proof_chain)?;
    if body.query.committed_transaction_norito != committed_digest
        || body.query.finalized_block_wire != body.finalized_block_wire_digest
        || body.query.finality_proof_chain_norito != proof_chain_digest
        || usize::try_from(body.query.finality_proof_count) != Ok(body.finality_proof_chain.len())
    {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::QueryEvidence);
    }
    body.query.validate(
        body.canary_transaction_intent,
        body.finalized_height,
        block_time_unix_ms,
    )
}

fn duration_millis(
    duration: std::time::Duration,
) -> Result<u64, KagemushaV4TairaCanaryEvidenceValidationError> {
    u64::try_from(duration.as_millis())
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::InvalidField("time"))
}

fn canonical_digest<T: norito::NoritoSerialize>(
    value: &T,
) -> Result<KagemushaExactBytesDigestV1, KagemushaV4TairaCanaryEvidenceValidationError> {
    let bytes = norito::encode_canonical(value)
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Encode)?;
    KagemushaExactBytesDigestV1::from_bytes(&bytes)
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Encode)
}

fn validate_identity_digest(
    digest: KagemushaExactBytesDigestV1,
    maximum: usize,
    field: &'static str,
) -> Result<(), KagemushaV4TairaCanaryEvidenceValidationError> {
    digest
        .validate()
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::InvalidField(field))?;
    if digest.byte_len
        > u64::try_from(maximum).expect("artifact byte limits fit the exact digest length field")
    {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::InvalidField(
            field,
        ));
    }
    Ok(())
}

fn hash_is_zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

fn enforce_artifact_size<T: norito::NoritoSerialize>(
    value: &T,
    maximum: usize,
) -> Result<(), KagemushaV4TairaCanaryEvidenceValidationError> {
    let _canonical_flags =
        norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let preflight = norito::core::encoded_frame_len(value)
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Encode)?;
    if preflight > maximum {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::Size {
            actual: preflight,
            maximum,
        });
    }
    let bytes = norito::encode_canonical(value)
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Encode)?;
    check_input_size(&bytes, maximum)
}

fn check_input_size(
    bytes: &[u8],
    maximum: usize,
) -> Result<(), KagemushaV4TairaCanaryEvidenceValidationError> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::Size {
            actual: bytes.len(),
            maximum,
        });
    }
    Ok(())
}

fn supports_evidence_signature_algorithm(key: &PublicKey) -> bool {
    matches!(
        key.try_algorithm(),
        Ok(Algorithm::Ed25519 | Algorithm::MlDsa | Algorithm::BlsNormal)
    )
}

fn verify_authorization_signature(
    signature: &SignatureOf<KagemushaV4TairaCanaryAuthorizationBodyV1>,
    signer: &PublicKey,
    hash: HashOf<KagemushaV4TairaCanaryAuthorizationBodyV1>,
) -> Result<(), KagemushaV4TairaCanaryEvidenceValidationError> {
    if !matches!(signer.try_algorithm(), Ok(Algorithm::Ed25519)) {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::Signature);
    }
    iroha_crypto::ed25519_parse_signature(signature.payload())
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Signature)?;
    signature
        .verify_hash(signer, hash)
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Signature)
}

fn verify_authorization_package_signature(
    signature: &SignatureOf<KagemushaV4TairaCanaryAuthorizationPackageV1>,
    signer: &PublicKey,
    hash: HashOf<KagemushaV4TairaCanaryAuthorizationPackageV1>,
) -> Result<(), KagemushaV4TairaCanaryEvidenceValidationError> {
    if !matches!(signer.try_algorithm(), Ok(Algorithm::Ed25519)) {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::Signature);
    }
    iroha_crypto::ed25519_parse_signature(signature.payload())
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Signature)?;
    signature
        .verify_hash(signer, hash)
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Signature)
}

fn verify_reservation_signature(
    signature: &SignatureOf<KagemushaV4TairaCanaryReservationBodyV1>,
    signer: &PublicKey,
    hash: HashOf<KagemushaV4TairaCanaryReservationBodyV1>,
) -> Result<(), KagemushaV4TairaCanaryEvidenceValidationError> {
    if !matches!(signer.try_algorithm(), Ok(Algorithm::Ed25519)) {
        return Err(KagemushaV4TairaCanaryEvidenceValidationError::Signature);
    }
    iroha_crypto::ed25519_parse_signature(signature.payload())
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Signature)?;
    signature
        .verify_hash(signer, hash)
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Signature)
}

fn verify_evidence_signature(
    signature: &SignatureOf<KagemushaV4TairaCanaryEvidenceBodyV1>,
    signer: &PublicKey,
    hash: HashOf<KagemushaV4TairaCanaryEvidenceBodyV1>,
) -> Result<(), KagemushaV4TairaCanaryEvidenceValidationError> {
    match signer.try_algorithm() {
        Ok(Algorithm::Ed25519) => {
            iroha_crypto::ed25519_parse_signature(signature.payload())
                .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Signature)?;
        }
        Ok(Algorithm::MlDsa) => {
            iroha_crypto::mldsa65_parse_signature(signature.payload())
                .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Signature)?;
        }
        Ok(Algorithm::BlsNormal) => {}
        _ => return Err(KagemushaV4TairaCanaryEvidenceValidationError::Signature),
    }
    signature
        .verify_hash(signer, hash)
        .map_err(|_| KagemushaV4TairaCanaryEvidenceValidationError::Signature)
}

/// Failure while decoding or validating promotion-bound Taira canary artifacts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum KagemushaV4TairaCanaryEvidenceValidationError {
    /// One named structural field is invalid.
    #[error("invalid Kagemusha Taira canary field: {0}")]
    InvalidField(&'static str),
    /// The declared signer differs from the supplied private key.
    #[error("Kagemusha Taira canary signer does not match the declared key")]
    SignerMismatch,
    /// Canonical encoding failed.
    #[error("failed to encode canonical Kagemusha Taira canary artifact")]
    Encode,
    /// Canonical bounded decoding failed.
    #[error("failed to decode canonical Kagemusha Taira canary artifact")]
    Decode,
    /// An artifact violates its canonical encoded-size ceiling.
    #[error("Kagemusha Taira canary artifact is {actual} bytes; maximum is {maximum}")]
    Size {
        /// Actual exact byte length.
        actual: usize,
        /// Maximum permitted byte length.
        maximum: usize,
    },
    /// An authorization or evidence signature is malformed or invalid.
    #[error("invalid Kagemusha Taira canary signature")]
    Signature,
    /// The supplied activation receipt is invalid or differs from exact bytes.
    #[error("Kagemusha Taira canary activation receipt is invalid or non-exact")]
    ActivationReceipt,
    /// A promotion or activation identity differs from authenticated inputs.
    #[error("Kagemusha Taira canary differs from authenticated activation inputs")]
    ActivationBinding,
    /// The exact controller authorization is absent, invalid, or mismatched.
    #[error("invalid or mismatched Kagemusha Taira canary authorization")]
    Authorization,
    /// The canary was not committed within both signed expiry domains.
    #[error("Kagemusha Taira canary authorization was not live at commit")]
    AuthorizationExpired,
    /// The pre-signed canary transaction violates its exact deterministic profile.
    #[error("invalid Kagemusha Taira canary transaction")]
    CanaryTransaction,
    /// The committed transaction is unsuccessful, absent, or not the authorized wire.
    #[error("invalid Kagemusha Taira committed canary transaction")]
    CommittedTransaction,
    /// The exact canary block wire, header, or transaction location is invalid.
    #[error("invalid Kagemusha Taira canary block binding")]
    BlockBinding,
    /// The post-receipt finality extension is absent, discontinuous, or invalid.
    #[error("invalid Kagemusha Taira canary finality extension")]
    Finality,
    /// Diagnostic live-query observations disagree with the production evidence.
    #[error("invalid Kagemusha Taira canary query evidence")]
    QueryEvidence,
}
