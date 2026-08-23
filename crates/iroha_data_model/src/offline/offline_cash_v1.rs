//! Canonical first-release wire contract for hardware-guarded offline balances.

use super::{
    KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_TTL_MS_V2, KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2,
    KagemushaDevicePublicKeyV2, KagemushaDeviceSignatureV2, KagemushaValidationError,
    is_kagemusha_network_id,
};
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{NetworkId, account::AccountId, asset::AssetDefinitionId};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

/// Version carried by every clean-slate offline-cash wire value.
pub const OFFLINE_CASH_WIRE_VERSION_V1: u16 = 1;
/// Text transport discriminator for canonical unpadded base64url messages.
pub const OFFLINE_CASH_TEXT_PREFIX_V1: &str = "kgm2:";
/// Maximum canonical receiver-request bytes.
pub const OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1: usize = 768;
/// Maximum canonical sender-response bytes.
pub const OFFLINE_CASH_PAYMENT_MAX_BYTES_V1: usize = 7_936;
/// Maximum canonical receiver-acknowledgement bytes.
pub const OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1: usize = 256;
/// Qualification target for the complete three-message raw session.
pub const OFFLINE_CASH_SESSION_TARGET_BYTES_V1: usize = 8_960;
/// Absolute pre-decode raw session limit implied by the text envelope.
pub const OFFLINE_CASH_SESSION_MAX_BYTES_V1: usize = 9_211;
/// Absolute complete text-session limit.
pub const OFFLINE_CASH_TEXT_SESSION_MAX_BYTES_V1: usize = 12_288;
/// Qualification target for the two current recursive proofs.
pub const OFFLINE_CASH_PAIRED_PROOF_TARGET_BYTES_V1: usize = 6_144;
/// Absolute byte limit for the two current recursive proofs.
pub const OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1: usize = 6_400;
/// Maximum bytes in either parity's current proof.
pub const OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1: usize = 3_200;
/// Exact compact delayed-history accumulator bytes for one `k=16` parity.
pub const OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1: usize = 544;
/// Maximum encrypted credit-opening bytes carried by a sender response.
pub const OFFLINE_CASH_ENCRYPTED_CREDIT_MAX_BYTES_V1: usize = 384;

const REQUEST_SIGNING_DOMAIN: &[u8] = b"iroha:offline-cash:v1:payment-request-signing";
const REQUEST_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:payment-request";
const PUBLIC_KEY_REFERENCE_DOMAIN: &[u8] = b"iroha:offline-cash:v1:receiver-key-reference";
const TRANSITION_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:send-split-transition";
const STATEMENT_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:send-split-statement";
const PAYMENT_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:payment";
const ACKNOWLEDGEMENT_SIGNING_DOMAIN: &[u8] = b"iroha:offline-cash:v1:acknowledgement-signing";

/// Public send-split statement decided by both Pasta parities.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashTransferStatementV1 {
    /// Wire version.
    pub version: u16,
    /// Authenticated release identifier.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub release_id: [u8; 32],
    /// Exact network identity.
    pub network_id: NetworkId,
    /// Asset transferred by this relation.
    pub asset: AssetDefinitionId,
    /// Authoritative asset scale.
    pub scale: u32,
    /// Positive transfer amount in atomic units.
    pub amount: u128,
    /// Digest of the exact receiver request.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub request_digest: [u8; 32],
    /// Sender balance commitment consumed by the split.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub sender_before: [u8; 32],
    /// Persisted sender-remainder commitment.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub sender_after: [u8; 32],
    /// Receiver balance commitment named by the request.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub receiver_before: [u8; 32],
    /// Receiver-bound credit commitment produced by the split.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub credit_commitment: [u8; 32],
    /// Digest authorized by the sender hardware guard.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub transition_digest: [u8; 32],
}

/// Closed paired-Pasta proof and delayed-history accumulators.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashPairedProofV1 {
    /// Wire version.
    pub version: u16,
    /// Exact Eq/Fp circuit-and-protocol digest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub eq_protocol_digest: [u8; 32],
    /// Exact Ep/Fq circuit-and-protocol digest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ep_protocol_digest: [u8; 32],
    /// Digest of the common semantic statement constrained by both proofs.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub semantic_digest: [u8; 32],
    /// Current Eq/Fp augmented IPA proof.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub eq_proof: Vec<u8>,
    /// Current Ep/Fq augmented IPA proof.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub ep_proof: Vec<u8>,
    /// Compact Eq/Fp delayed-history accumulator.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub eq_history: Vec<u8>,
    /// Compact Ep/Fq delayed-history accumulator.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub ep_history: Vec<u8>,
}

/// Receiver-created request bound to its one current balance head.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashPaymentRequestV1 {
    /// Wire version.
    pub version: u16,
    /// Authenticated release identifier.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub release_id: [u8; 32],
    /// Exact network identity.
    pub network_id: NetworkId,
    /// Requested asset.
    pub asset: AssetDefinitionId,
    /// Authoritative asset scale.
    pub scale: u32,
    /// Positive requested amount in atomic units.
    pub amount: u128,
    /// Recipient account identity.
    pub recipient: AccountId,
    /// Current receiver balance commitment that the credit must consume.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub receiver_balance_commitment: [u8; 32],
    /// Domain-separated reference to the request-signing key.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub recipient_key_reference: [u8; 32],
    /// Canonical uncompressed P-256 request-signing key.
    pub receiver_public_key: KagemushaDevicePublicKeyV2,
    /// Unique receiver nonce.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub request_id: [u8; 32],
    /// Request creation time in Unix milliseconds.
    pub issued_at_ms: u64,
    /// Exclusive request expiry in Unix milliseconds.
    pub expires_at_ms: u64,
    /// Authenticated hardware-policy registry root.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub hardware_policy_id: [u8; 32],
    /// Low-S P-256 signature over the exact unsigned request.
    pub signature: KagemushaDeviceSignatureV2,
}

/// Sender response containing one receiver-bound credit proof.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashPaymentV1 {
    /// Wire version.
    pub version: u16,
    /// Digest of the receiver request echoed by this response.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub request_digest: [u8; 32],
    /// Common statement decided by both current proofs.
    pub statement: OfflineCashTransferStatementV1,
    /// Closed current proofs and compact delayed histories.
    pub proof: OfflineCashPairedProofV1,
    /// Receiver-only encrypted credit opening.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub encrypted_credit: Vec<u8>,
    /// Digest of the artifact manifest used to produce the proof.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub artifact_manifest_digest: [u8; 32],
}

/// Receiver acknowledgement emitted only after locally persisting `ReceiveFold`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashAcknowledgementV1 {
    /// Wire version.
    pub version: u16,
    /// Authenticated release identifier.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub release_id: [u8; 32],
    /// Digest of the accepted receiver request.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub request_digest: [u8; 32],
    /// Digest of the accepted sender response.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub payment_digest: [u8; 32],
    /// Newly persisted receiver balance commitment.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub receiver_balance_commitment: [u8; 32],
    /// Receiver persistence time in Unix milliseconds.
    pub acknowledged_at_ms: u64,
    /// Low-S P-256 signature over the acknowledgement fields.
    pub signature: KagemushaDeviceSignatureV2,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.offline-cash.v1.payment-request-signing-preimage")]
struct PaymentRequestSigningPreimageV1 {
    domain: Vec<u8>,
    version: u16,
    release_id: [u8; 32],
    network_id: NetworkId,
    asset: AssetDefinitionId,
    scale: u32,
    amount: u128,
    recipient: AccountId,
    receiver_balance_commitment: [u8; 32],
    recipient_key_reference: [u8; 32],
    receiver_public_key: KagemushaDevicePublicKeyV2,
    request_id: [u8; 32],
    issued_at_ms: u64,
    expires_at_ms: u64,
    hardware_policy_id: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
struct TransferTransitionPreimageV1 {
    domain: Vec<u8>,
    version: u16,
    release_id: [u8; 32],
    network_id: NetworkId,
    asset: AssetDefinitionId,
    scale: u32,
    amount: u128,
    request_digest: [u8; 32],
    sender_before: [u8; 32],
    sender_after: [u8; 32],
    receiver_before: [u8; 32],
    credit_commitment: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.offline-cash.v1.acknowledgement-signing-preimage")]
struct AcknowledgementSigningPreimageV1 {
    domain: Vec<u8>,
    version: u16,
    release_id: [u8; 32],
    request_digest: [u8; 32],
    payment_digest: [u8; 32],
    receiver_balance_commitment: [u8; 32],
    acknowledged_at_ms: u64,
}

/// Encode the exact canonical receiver-request bytes authorized by hardware.
///
/// This constructor is the single cross-crate signing contract. Keeping the
/// private Norito preimage here prevents a second Rust type name or field
/// layout from silently changing the canonical header and invalidating an
/// otherwise correct P-256 signature.
///
/// # Errors
///
/// Returns an error when canonical Norito encoding fails.
#[allow(clippy::too_many_arguments)]
pub fn offline_cash_payment_request_signing_bytes_v1(
    version: u16,
    release_id: [u8; 32],
    network_id: &NetworkId,
    asset: &AssetDefinitionId,
    scale: u32,
    amount: u128,
    recipient: &AccountId,
    receiver_balance_commitment: [u8; 32],
    recipient_key_reference: [u8; 32],
    receiver_public_key: KagemushaDevicePublicKeyV2,
    request_id: [u8; 32],
    issued_at_ms: u64,
    expires_at_ms: u64,
    hardware_policy_id: [u8; 32],
) -> Result<Vec<u8>, KagemushaValidationError> {
    Ok(norito::encode_canonical(
        &PaymentRequestSigningPreimageV1 {
            domain: REQUEST_SIGNING_DOMAIN.to_vec(),
            version,
            release_id,
            network_id: *network_id,
            asset: asset.clone(),
            scale,
            amount,
            recipient: recipient.clone(),
            receiver_balance_commitment,
            recipient_key_reference,
            receiver_public_key,
            request_id,
            issued_at_ms,
            expires_at_ms,
            hardware_policy_id,
        },
    )?)
}

/// Encode the exact canonical post-persistence acknowledgement bytes.
///
/// # Errors
///
/// Returns an error when canonical Norito encoding fails.
pub fn offline_cash_acknowledgement_signing_bytes_v1(
    version: u16,
    release_id: [u8; 32],
    request_digest: [u8; 32],
    payment_digest: [u8; 32],
    receiver_balance_commitment: [u8; 32],
    acknowledged_at_ms: u64,
) -> Result<Vec<u8>, KagemushaValidationError> {
    Ok(norito::encode_canonical(
        &AcknowledgementSigningPreimageV1 {
            domain: ACKNOWLEDGEMENT_SIGNING_DOMAIN.to_vec(),
            version,
            release_id,
            request_digest,
            payment_digest,
            receiver_balance_commitment,
            acknowledged_at_ms,
        },
    )?)
}

fn digest_encoded<T: Encode>(
    domain: &[u8],
    value: &T,
) -> Result<[u8; 32], KagemushaValidationError> {
    let bytes = norito::encode_canonical(value)?;
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update([0]);
    hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    hasher.update(bytes);
    Ok(hasher.finalize().into())
}

fn require_nonzero(field: &'static str, value: [u8; 32]) -> Result<(), KagemushaValidationError> {
    if value == [0; 32] {
        return Err(KagemushaValidationError::InvalidRecursiveSpendProof { field });
    }
    Ok(())
}

fn require_encoded_size<T: Encode>(
    value: &T,
    max: usize,
) -> Result<usize, KagemushaValidationError> {
    let actual = norito::encode_canonical(value)?.len();
    if actual > max {
        return Err(KagemushaValidationError::EncodedSizeExceeded { actual, max });
    }
    Ok(actual)
}

/// Decode one already byte-capped canonical frame under resource limits that
/// are installed before derive-generated sequence decoders can reserve space.
///
/// This is intentionally narrower than generic [`Decode`]: callers handling
/// untrusted Offline Cash wire bytes must route through the public typed
/// entrypoints below so the outer cap is checked before the header or any
/// declared collection length is interpreted.
fn decode_bounded_canonical<T>(bytes: &[u8], max: usize) -> Result<T, KagemushaValidationError>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    if bytes.len() > max {
        return Err(KagemushaValidationError::EncodedSizeExceeded {
            actual: bytes.len(),
            max,
        });
    }
    let limits = norito::canonical_decode_limits(bytes.len());
    Ok(norito::decode_canonical_with_limits(bytes, limits)?)
}

/// Derive the stable receiver-key reference carried by a payment request.
#[must_use]
pub fn offline_cash_receiver_key_reference_v1(public_key: &KagemushaDevicePublicKeyV2) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(PUBLIC_KEY_REFERENCE_DOMAIN);
    hasher.update([0]);
    hasher.update(public_key.as_sec1_bytes());
    hasher.finalize().into()
}

impl OfflineCashPaymentRequestV1 {
    /// Return the exact bytes signed by the receiver device.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical Norito encoding fails.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, KagemushaValidationError> {
        offline_cash_payment_request_signing_bytes_v1(
            self.version,
            self.release_id,
            &self.network_id,
            &self.asset,
            self.scale,
            self.amount,
            &self.recipient,
            self.receiver_balance_commitment,
            self.recipient_key_reference,
            self.receiver_public_key,
            self.request_id,
            self.issued_at_ms,
            self.expires_at_ms,
            self.hardware_policy_id,
        )
    }

    /// Decode, canonicalize, and validate one exact bounded receiver request.
    ///
    /// The outer byte cap is enforced before Norito reads a header or declared
    /// sequence length. Decoding then runs under payload-derived sequence and
    /// cumulative allocation limits and rejects any non-canonical byte form.
    ///
    /// # Errors
    ///
    /// Returns an error for an oversized, malformed, non-canonical, or invalid request.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, KagemushaValidationError> {
        let request: Self =
            decode_bounded_canonical(bytes, OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1)?;
        request.validate()?;
        Ok(request)
    }

    /// Validate context, bounds, key binding, signature, and canonical size.
    ///
    /// # Errors
    ///
    /// Returns an error when any first-release request invariant fails.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1 {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.request.version",
            });
        }
        require_nonzero("offline_cash.request.release_id", self.release_id)?;
        require_nonzero(
            "offline_cash.request.receiver_balance_commitment",
            self.receiver_balance_commitment,
        )?;
        require_nonzero("offline_cash.request.request_id", self.request_id)?;
        require_nonzero(
            "offline_cash.request.hardware_policy_id",
            self.hardware_policy_id,
        )?;
        if !is_kagemusha_network_id(&self.network_id) {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.request.network_id",
            });
        }
        if self.amount == 0 || self.scale > KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2 {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.request.amount",
            });
        }
        let ttl = self.expires_at_ms.checked_sub(self.issued_at_ms).ok_or(
            KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.request.expires_at_ms",
            },
        )?;
        if ttl == 0 || ttl > KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_TTL_MS_V2 {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.request.expires_at_ms",
            });
        }
        self.receiver_public_key.validate()?;
        if self.recipient_key_reference
            != offline_cash_receiver_key_reference_v1(&self.receiver_public_key)
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.request.recipient_key_reference",
            });
        }
        self.signature
            .verify(&self.receiver_public_key, &self.canonical_signing_bytes()?)?;
        require_encoded_size(self, OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Return the canonical request identity consumed by `SendSplit`.
    ///
    /// # Errors
    ///
    /// Returns an error when the request is invalid or cannot be encoded.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate()?;
        digest_encoded(REQUEST_DIGEST_DOMAIN, self)
    }
}

impl OfflineCashTransferStatementV1 {
    fn transition_preimage(&self) -> TransferTransitionPreimageV1 {
        TransferTransitionPreimageV1 {
            domain: TRANSITION_DIGEST_DOMAIN.to_vec(),
            version: self.version,
            release_id: self.release_id,
            network_id: self.network_id,
            asset: self.asset.clone(),
            scale: self.scale,
            amount: self.amount,
            request_digest: self.request_digest,
            sender_before: self.sender_before,
            sender_after: self.sender_after,
            receiver_before: self.receiver_before,
            credit_commitment: self.credit_commitment,
        }
    }

    /// Compute the sender-hardware transition digest from all other fields.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn expected_transition_digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        digest_encoded(TRANSITION_DIGEST_DOMAIN, &self.transition_preimage())
    }

    /// Populate the canonical transition digest.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn seal_transition(mut self) -> Result<Self, KagemushaValidationError> {
        self.transition_digest = self.expected_transition_digest()?;
        Ok(self)
    }

    /// Validate the exact public send-split binding.
    ///
    /// # Errors
    ///
    /// Returns an error when context, amount, commitment, or transition binding is invalid.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1
            || !is_kagemusha_network_id(&self.network_id)
            || self.amount == 0
            || self.scale > KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.statement.header",
            });
        }
        for (field, value) in [
            ("offline_cash.statement.release_id", self.release_id),
            ("offline_cash.statement.request_digest", self.request_digest),
            ("offline_cash.statement.sender_before", self.sender_before),
            ("offline_cash.statement.sender_after", self.sender_after),
            (
                "offline_cash.statement.receiver_before",
                self.receiver_before,
            ),
            (
                "offline_cash.statement.credit_commitment",
                self.credit_commitment,
            ),
            (
                "offline_cash.statement.transition_digest",
                self.transition_digest,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        let commitments = [
            self.sender_before,
            self.sender_after,
            self.receiver_before,
            self.credit_commitment,
        ];
        for left in 0..commitments.len() {
            for right in left + 1..commitments.len() {
                if commitments[left] == commitments[right] {
                    return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                        field: "offline_cash.statement.commitments",
                    });
                }
            }
        }
        if self.transition_digest != self.expected_transition_digest()? {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.statement.transition_digest",
            });
        }
        Ok(())
    }

    /// Return the common semantic digest constrained by both Pasta parities.
    ///
    /// # Errors
    ///
    /// Returns an error when the statement is invalid or cannot be encoded.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate()?;
        digest_encoded(STATEMENT_DIGEST_DOMAIN, self)
    }
}

impl OfflineCashPairedProofV1 {
    /// Validate fixed parity roles, proof caps, and exact history sizes.
    ///
    /// # Errors
    ///
    /// Returns an error when the paired proof is empty, oversized, aliased, or mis-bound.
    pub fn validate_for_semantic_digest(
        &self,
        expected_semantic_digest: [u8; 32],
    ) -> Result<(), KagemushaValidationError> {
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1 {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.proof.version",
            });
        }
        require_nonzero(
            "offline_cash.proof.eq_protocol_digest",
            self.eq_protocol_digest,
        )?;
        require_nonzero(
            "offline_cash.proof.ep_protocol_digest",
            self.ep_protocol_digest,
        )?;
        require_nonzero("offline_cash.proof.semantic_digest", self.semantic_digest)?;
        if self.eq_protocol_digest == self.ep_protocol_digest
            || self.semantic_digest != expected_semantic_digest
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.proof.role_binding",
            });
        }
        if self.eq_proof.is_empty()
            || self.ep_proof.is_empty()
            || self.eq_proof.len() > OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1
            || self.ep_proof.len() > OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1
            || self.eq_proof.len() + self.ep_proof.len() > OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.proof.current",
            });
        }
        if self.eq_history.len() != OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1
            || self.ep_history.len() != OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1
            || self.eq_history.iter().all(|byte| *byte == 0)
            || self.ep_history.iter().all(|byte| *byte == 0)
            || self.eq_history == self.ep_history
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.proof.history",
            });
        }
        Ok(())
    }
}

impl OfflineCashPaymentV1 {
    /// Decode, canonicalize, and validate one exact bounded sender response.
    ///
    /// The outer byte cap is enforced before Norito reads a header or declared
    /// sequence length. Decoding then runs under payload-derived sequence and
    /// cumulative allocation limits and verifies the exact receiver-request binding.
    ///
    /// # Errors
    ///
    /// Returns an error for an oversized, malformed, non-canonical, or invalid response.
    pub fn decode_canonical_exact_against(
        bytes: &[u8],
        request: &OfflineCashPaymentRequestV1,
    ) -> Result<Self, KagemushaValidationError> {
        let payment: Self = decode_bounded_canonical(bytes, OFFLINE_CASH_PAYMENT_MAX_BYTES_V1)?;
        payment.validate_against(request)?;
        Ok(payment)
    }

    /// Validate this response against the exact signed receiver request.
    ///
    /// # Errors
    ///
    /// Returns an error when context, proof, statement, request, or size binding fails.
    pub fn validate_against(
        &self,
        request: &OfflineCashPaymentRequestV1,
    ) -> Result<(), KagemushaValidationError> {
        request.validate()?;
        let request_digest = request.canonical_digest()?;
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1
            || self.request_digest != request_digest
            || self.statement.version != self.version
            || self.statement.release_id != request.release_id
            || self.statement.network_id != request.network_id
            || self.statement.asset != request.asset
            || self.statement.scale != request.scale
            || self.statement.amount != request.amount
            || self.statement.request_digest != request_digest
            || self.statement.receiver_before != request.receiver_balance_commitment
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.payment.request_binding",
            });
        }
        self.statement.validate()?;
        self.proof
            .validate_for_semantic_digest(self.statement.canonical_digest()?)?;
        if self.encrypted_credit.is_empty()
            || self.encrypted_credit.len() > OFFLINE_CASH_ENCRYPTED_CREDIT_MAX_BYTES_V1
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.payment.encrypted_credit",
            });
        }
        require_nonzero(
            "offline_cash.payment.artifact_manifest_digest",
            self.artifact_manifest_digest,
        )?;
        require_encoded_size(self, OFFLINE_CASH_PAYMENT_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Return the canonical response digest after validating its receiver request.
    ///
    /// # Errors
    ///
    /// Returns an error when the response is invalid or cannot be encoded.
    pub fn canonical_digest_against(
        &self,
        request: &OfflineCashPaymentRequestV1,
    ) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate_against(request)?;
        digest_encoded(PAYMENT_DIGEST_DOMAIN, self)
    }
}

impl OfflineCashAcknowledgementV1 {
    /// Decode, canonicalize, and validate one exact bounded acknowledgement.
    ///
    /// The outer byte cap is enforced before Norito reads a header or declared
    /// sequence length. Decoding then runs under payload-derived sequence and
    /// cumulative allocation limits and verifies the request/response binding.
    ///
    /// # Errors
    ///
    /// Returns an error for an oversized, malformed, non-canonical, or invalid acknowledgement.
    pub fn decode_canonical_exact_against(
        bytes: &[u8],
        request: &OfflineCashPaymentRequestV1,
        payment: &OfflineCashPaymentV1,
    ) -> Result<Self, KagemushaValidationError> {
        let acknowledgement: Self =
            decode_bounded_canonical(bytes, OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1)?;
        acknowledgement.validate_against(request, payment)?;
        Ok(acknowledgement)
    }

    /// Return the exact bytes signed after persisting the receiver balance.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, KagemushaValidationError> {
        offline_cash_acknowledgement_signing_bytes_v1(
            self.version,
            self.release_id,
            self.request_digest,
            self.payment_digest,
            self.receiver_balance_commitment,
            self.acknowledged_at_ms,
        )
    }

    /// Validate this acknowledgement against its request and response.
    ///
    /// # Errors
    ///
    /// Returns an error when identity, time, persistence-head, signature, or size binding fails.
    pub fn validate_against(
        &self,
        request: &OfflineCashPaymentRequestV1,
        payment: &OfflineCashPaymentV1,
    ) -> Result<(), KagemushaValidationError> {
        payment.validate_against(request)?;
        let request_digest = request.canonical_digest()?;
        let payment_digest = payment.canonical_digest_against(request)?;
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1
            || self.release_id != request.release_id
            || self.request_digest != request_digest
            || self.payment_digest != payment_digest
            || self.receiver_balance_commitment == [0; 32]
            || self.receiver_balance_commitment == request.receiver_balance_commitment
            || self.receiver_balance_commitment == payment.statement.credit_commitment
            || self.acknowledged_at_ms < request.issued_at_ms
            || self.acknowledged_at_ms >= request.expires_at_ms
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.acknowledgement.binding",
            });
        }
        self.signature.verify(
            &request.receiver_public_key,
            &self.canonical_signing_bytes()?,
        )?;
        require_encoded_size(self, OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1)?;
        Ok(())
    }
}

fn unpadded_base64url_len(raw_len: usize) -> usize {
    raw_len / 3 * 4
        + match raw_len % 3 {
            0 => 0,
            1 => 2,
            _ => 3,
        }
}

fn validate_offline_cash_raw_session_size_v1(raw: usize) -> Result<(), KagemushaValidationError> {
    if raw > OFFLINE_CASH_SESSION_MAX_BYTES_V1 {
        return Err(KagemushaValidationError::EncodedSizeExceeded {
            actual: raw,
            max: OFFLINE_CASH_SESSION_MAX_BYTES_V1,
        });
    }
    Ok(())
}

/// Validate the complete request/response/acknowledgement session and return its raw size.
///
/// # Errors
///
/// Returns an error when a message is invalid or the aggregate raw/text envelope is oversized.
pub fn validate_offline_cash_session_v1(
    request: &OfflineCashPaymentRequestV1,
    payment: &OfflineCashPaymentV1,
    acknowledgement: &OfflineCashAcknowledgementV1,
) -> Result<usize, KagemushaValidationError> {
    acknowledgement.validate_against(request, payment)?;
    let lengths = [
        require_encoded_size(request, OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1)?,
        require_encoded_size(payment, OFFLINE_CASH_PAYMENT_MAX_BYTES_V1)?,
        require_encoded_size(acknowledgement, OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1)?,
    ];
    let raw = lengths.iter().sum::<usize>();
    validate_offline_cash_raw_session_size_v1(raw)?;
    let text = lengths
        .iter()
        .map(|length| OFFLINE_CASH_TEXT_PREFIX_V1.len() + unpadded_base64url_len(*length))
        .sum::<usize>();
    if text > OFFLINE_CASH_TEXT_SESSION_MAX_BYTES_V1 {
        return Err(KagemushaValidationError::EncodedSizeExceeded {
            actual: text,
            max: OFFLINE_CASH_TEXT_SESSION_MAX_BYTES_V1,
        });
    }
    Ok(raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{domain::DomainId, offline::kagemusha_test_network_id};
    use iroha_crypto::{Algorithm, KeyPair};
    use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};

    fn asset() -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "xor".parse().expect("asset name"),
        )
    }

    fn account() -> AccountId {
        AccountId::new(
            KeyPair::from_seed(vec![0xA5; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        )
    }

    fn signing_key() -> SigningKey {
        SigningKey::from_bytes((&[7_u8; 32]).into()).expect("P-256 signing key")
    }

    fn sign(key: &SigningKey, bytes: &[u8]) -> KagemushaDeviceSignatureV2 {
        let signature: Signature = key.sign(bytes);
        let signature = signature.normalize_s().unwrap_or(signature);
        KagemushaDeviceSignatureV2::from_raw_bytes(signature.to_bytes().as_ref())
            .expect("canonical signature")
    }

    fn request() -> OfflineCashPaymentRequestV1 {
        let signing_key = signing_key();
        let encoded = signing_key.verifying_key().to_encoded_point(false);
        let public_key =
            KagemushaDevicePublicKeyV2::from_sec1_bytes(encoded.as_bytes()).expect("public key");
        let placeholder = sign(&signing_key, b"placeholder");
        let mut request = OfflineCashPaymentRequestV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            release_id: [1; 32],
            network_id: kagemusha_test_network_id(b"offline-cash-v1"),
            asset: asset(),
            scale: 4,
            amount: 12_345,
            recipient: account(),
            receiver_balance_commitment: [2; 32],
            recipient_key_reference: offline_cash_receiver_key_reference_v1(&public_key),
            receiver_public_key: public_key,
            request_id: [3; 32],
            issued_at_ms: 1_000,
            expires_at_ms: 61_000,
            hardware_policy_id: [4; 32],
            signature: placeholder,
        };
        request.signature = sign(
            &signing_key,
            &request.canonical_signing_bytes().expect("request bytes"),
        );
        request
    }

    fn payment(request: &OfflineCashPaymentRequestV1) -> OfflineCashPaymentV1 {
        let request_digest = request.canonical_digest().expect("request digest");
        let statement = OfflineCashTransferStatementV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            release_id: request.release_id,
            network_id: request.network_id,
            asset: request.asset.clone(),
            scale: request.scale,
            amount: request.amount,
            request_digest,
            sender_before: [5; 32],
            sender_after: [6; 32],
            receiver_before: request.receiver_balance_commitment,
            credit_commitment: [7; 32],
            transition_digest: [0; 32],
        }
        .seal_transition()
        .expect("seal transition");
        let semantic_digest = statement.canonical_digest().expect("statement digest");
        OfflineCashPaymentV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            request_digest,
            statement,
            proof: OfflineCashPairedProofV1 {
                version: OFFLINE_CASH_WIRE_VERSION_V1,
                eq_protocol_digest: [8; 32],
                ep_protocol_digest: [9; 32],
                semantic_digest,
                eq_proof: vec![0xA1; 128],
                ep_proof: vec![0xB2; 128],
                eq_history: vec![0xC3; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
                ep_history: vec![0xD4; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
            },
            encrypted_credit: vec![0xE5; 128],
            artifact_manifest_digest: [10; 32],
        }
    }

    fn acknowledgement(
        request: &OfflineCashPaymentRequestV1,
        payment: &OfflineCashPaymentV1,
    ) -> OfflineCashAcknowledgementV1 {
        let signing_key = signing_key();
        let mut acknowledgement = OfflineCashAcknowledgementV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            release_id: request.release_id,
            request_digest: request.canonical_digest().expect("request digest"),
            payment_digest: payment
                .canonical_digest_against(request)
                .expect("payment digest"),
            receiver_balance_commitment: [11; 32],
            acknowledged_at_ms: request.issued_at_ms + 1,
            signature: sign(&signing_key, b"placeholder"),
        };
        acknowledgement.signature = sign(
            &signing_key,
            &acknowledgement
                .canonical_signing_bytes()
                .expect("acknowledgement bytes"),
        );
        acknowledgement
    }

    #[test]
    fn canonical_session_roundtrips_and_fits_transport_caps() {
        let request = request();
        let payment = payment(&request);
        let acknowledgement = acknowledgement(&request, &payment);
        let raw = validate_offline_cash_session_v1(&request, &payment, &acknowledgement)
            .expect("valid session");
        assert!(raw < OFFLINE_CASH_SESSION_TARGET_BYTES_V1);
        let request_bytes = norito::encode_canonical(&request).expect("encode request");
        let decoded_request = OfflineCashPaymentRequestV1::decode_canonical_exact(&request_bytes)
            .expect("decode request");
        assert_eq!(decoded_request, request);

        let payment_bytes = norito::encode_canonical(&payment).expect("encode payment");
        let decoded_payment =
            OfflineCashPaymentV1::decode_canonical_exact_against(&payment_bytes, &decoded_request)
                .expect("decode payment");
        assert_eq!(decoded_payment, payment);

        let acknowledgement_bytes =
            norito::encode_canonical(&acknowledgement).expect("encode acknowledgement");
        let decoded_acknowledgement = OfflineCashAcknowledgementV1::decode_canonical_exact_against(
            &acknowledgement_bytes,
            &decoded_request,
            &decoded_payment,
        )
        .expect("decode acknowledgement");
        assert_eq!(decoded_acknowledgement, acknowledgement);
    }

    #[test]
    fn exact_decoders_reject_outer_cap_before_parsing() {
        let request = request();
        let payment = payment(&request);
        for (result, expected_actual, expected_max) in [
            (
                OfflineCashPaymentRequestV1::decode_canonical_exact(&vec![
                    0;
                    OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1
                        + 1
                ])
                .map(|_| ()),
                OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1 + 1,
                OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1,
            ),
            (
                OfflineCashPaymentV1::decode_canonical_exact_against(
                    &vec![0; OFFLINE_CASH_PAYMENT_MAX_BYTES_V1 + 1],
                    &request,
                )
                .map(|_| ()),
                OFFLINE_CASH_PAYMENT_MAX_BYTES_V1 + 1,
                OFFLINE_CASH_PAYMENT_MAX_BYTES_V1,
            ),
            (
                OfflineCashAcknowledgementV1::decode_canonical_exact_against(
                    &vec![0; OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1 + 1],
                    &request,
                    &payment,
                )
                .map(|_| ()),
                OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1 + 1,
                OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1,
            ),
        ] {
            assert!(matches!(
                result,
                Err(KagemushaValidationError::EncodedSizeExceeded { actual, max })
                    if actual == expected_actual && max == expected_max
            ));
        }
    }

    #[test]
    fn exact_decoders_reject_forged_declared_lengths() {
        const NORITO_PAYLOAD_LENGTH_OFFSET: usize = 4 + 1 + 1 + 16 + 1;
        const NORITO_PAYLOAD_LENGTH_END: usize = NORITO_PAYLOAD_LENGTH_OFFSET + 8;

        let request = request();
        let payment = payment(&request);
        let acknowledgement = acknowledgement(&request, &payment);
        let mut noncanonical_request =
            norito::encode_canonical(&request).expect("encode noncanonical request fixture");
        noncanonical_request.push(0);
        assert!(
            OfflineCashPaymentRequestV1::decode_canonical_exact(&noncanonical_request).is_err()
        );

        let mut request_bytes = norito::encode_canonical(&request).expect("encode request");
        let mut payment_bytes = norito::encode_canonical(&payment).expect("encode payment");
        let mut acknowledgement_bytes =
            norito::encode_canonical(&acknowledgement).expect("encode acknowledgement");
        for bytes in [
            &mut request_bytes,
            &mut payment_bytes,
            &mut acknowledgement_bytes,
        ] {
            bytes[NORITO_PAYLOAD_LENGTH_OFFSET..NORITO_PAYLOAD_LENGTH_END]
                .copy_from_slice(&u64::MAX.to_le_bytes());
        }

        assert!(OfflineCashPaymentRequestV1::decode_canonical_exact(&request_bytes).is_err());
        assert!(
            OfflineCashPaymentV1::decode_canonical_exact_against(&payment_bytes, &request).is_err()
        );
        assert!(
            OfflineCashAcknowledgementV1::decode_canonical_exact_against(
                &acknowledgement_bytes,
                &request,
                &payment,
            )
            .is_err()
        );
    }

    #[test]
    fn raw_session_hard_limit_is_distinct_from_qualification_target() {
        assert!(
            validate_offline_cash_raw_session_size_v1(OFFLINE_CASH_SESSION_TARGET_BYTES_V1 + 1)
                .is_ok()
        );
        assert!(
            validate_offline_cash_raw_session_size_v1(OFFLINE_CASH_SESSION_MAX_BYTES_V1).is_ok()
        );
        assert!(matches!(
            validate_offline_cash_raw_session_size_v1(OFFLINE_CASH_SESSION_MAX_BYTES_V1 + 1),
            Err(KagemushaValidationError::EncodedSizeExceeded { actual, max })
                if actual == OFFLINE_CASH_SESSION_MAX_BYTES_V1 + 1
                    && max == OFFLINE_CASH_SESSION_MAX_BYTES_V1
        ));
    }

    #[test]
    fn request_signature_binds_the_current_balance_head() {
        let mut request = request();
        request.receiver_balance_commitment = [0x55; 32];
        assert!(request.validate().is_err());
    }

    #[test]
    fn parity_substitution_and_oversized_proofs_are_rejected() {
        let request = request();
        let mut substituted = payment(&request);
        substituted.proof.ep_protocol_digest = substituted.proof.eq_protocol_digest;
        assert!(substituted.validate_against(&request).is_err());
        let mut oversized = payment(&request);
        oversized.proof.eq_proof = vec![0xAA; OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1 + 1];
        assert!(oversized.validate_against(&request).is_err());
    }

    #[test]
    fn acknowledgement_binds_the_persisted_receiver_head() {
        let request = request();
        let payment = payment(&request);
        let mut acknowledgement = acknowledgement(&request, &payment);
        acknowledgement.receiver_balance_commitment = request.receiver_balance_commitment;
        assert!(
            acknowledgement
                .validate_against(&request, &payment)
                .is_err()
        );
    }
}
