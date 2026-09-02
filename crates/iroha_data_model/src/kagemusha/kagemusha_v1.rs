//! Canonical first-release wire contract for hardware-guarded Kagemusha.
//!
//! Decode and `validate_shape*` routines in this module enforce canonical
//! framing, bounded allocation, signatures under embedded keys, and exact
//! public-field bindings. They never authorize money by themselves. Monetary
//! admission additionally requires an authenticated release/profile lookup,
//! authoritative asset-incarnation lookup, exact replay/reserve state, and the
//! release-pinned native cryptographic verifier.

#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    NetworkId, account::AccountId, asset::AssetDefinitionId, nexus::AxtAssetIncarnationV1,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use iroha_crypto::kex::{KeyExchangeScheme as _, X25519Sha256};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use p256::ecdsa::{
    Signature as P256Signature, VerifyingKey as P256VerifyingKey, signature::Verifier as _,
};
use sha2::{Digest as _, Sha256};

/// Version carried by every clean-slate Kagemusha wire value.
pub const KAGEMUSHA_WIRE_VERSION_V1: u16 = 1;
/// Version of the secure-device lane and journal lifecycle contract.
pub const KAGEMUSHA_DEVICE_LIFECYCLE_VERSION_V1: u16 = 1;
/// Device capability that commits sender state before exposing a payment.
pub const KAGEMUSHA_HANDOFF_CAPABILITY_V1: &str = "kagemusha_handoff_v1";
/// Text transport discriminator for canonical unpadded base64url messages.
pub const KAGEMUSHA_TEXT_PREFIX_V1: &str = "kgm1:";
/// Maximum authoritative asset scale represented by Kagemusha V1.
pub const KAGEMUSHA_ASSET_SCALE_MAX_V1: u32 = 28;
/// Maximum lifetime of a signed payment request in Unix milliseconds.
pub const KAGEMUSHA_REQUEST_MAX_TTL_MS_V1: u64 = 5 * 60 * 1_000;
/// Exact canonical uncompressed SEC1 P-256 public-key bytes.
pub const KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1: usize = 65;
/// Exact canonical fixed-width P-256 ECDSA signature bytes.
pub const KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V1: usize = 64;
/// Maximum canonical aggregate-state metadata bytes.
pub const KAGEMUSHA_AGGREGATE_STATE_MAX_BYTES_V1: usize = 768;
/// Maximum canonical receiver-request bytes.
pub const KAGEMUSHA_PAYMENT_REQUEST_MAX_BYTES_V1: usize = 1_024;
/// Maximum canonical sender-response bytes.
pub const KAGEMUSHA_PAYMENT_MAX_BYTES_V1: usize = 7_936;
/// Maximum canonical receiver-acknowledgement bytes.
pub const KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1: usize = 512;
/// Maximum canonical top-up mint-credit bytes.
pub const KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1: usize = 7_936;
/// Maximum canonical pre-debit recipient mint-authorization bytes.
pub const KAGEMUSHA_MINT_AUTHORIZATION_MAX_BYTES_V1: usize = 7_936;
/// Maximum canonical redemption-voucher bytes.
pub const KAGEMUSHA_REDEMPTION_VOUCHER_MAX_BYTES_V1: usize = 7_936;
/// Maximum complete `kgm1:` text request bytes.
pub const KAGEMUSHA_PAYMENT_REQUEST_TEXT_MAX_BYTES_V1: usize =
    KAGEMUSHA_TEXT_PREFIX_V1.len() + unpadded_base64url_len(KAGEMUSHA_PAYMENT_REQUEST_MAX_BYTES_V1);
/// Maximum complete `kgm1:` text payment bytes.
pub const KAGEMUSHA_PAYMENT_TEXT_MAX_BYTES_V1: usize =
    KAGEMUSHA_TEXT_PREFIX_V1.len() + unpadded_base64url_len(KAGEMUSHA_PAYMENT_MAX_BYTES_V1);
/// Maximum complete `kgm1:` text acknowledgement bytes.
pub const KAGEMUSHA_ACKNOWLEDGEMENT_TEXT_MAX_BYTES_V1: usize =
    KAGEMUSHA_TEXT_PREFIX_V1.len() + unpadded_base64url_len(KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1);
/// Maximum complete `kgm1:` text mint-credit bytes.
pub const KAGEMUSHA_MINT_CREDIT_TEXT_MAX_BYTES_V1: usize =
    KAGEMUSHA_TEXT_PREFIX_V1.len() + unpadded_base64url_len(KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1);
/// Maximum complete `kgm1:` text recipient mint-authorization bytes.
pub const KAGEMUSHA_MINT_AUTHORIZATION_TEXT_MAX_BYTES_V1: usize = KAGEMUSHA_TEXT_PREFIX_V1.len()
    + unpadded_base64url_len(KAGEMUSHA_MINT_AUTHORIZATION_MAX_BYTES_V1);
/// Maximum complete `kgm1:` text redemption-voucher bytes.
pub const KAGEMUSHA_REDEMPTION_VOUCHER_TEXT_MAX_BYTES_V1: usize = KAGEMUSHA_TEXT_PREFIX_V1.len()
    + unpadded_base64url_len(KAGEMUSHA_REDEMPTION_VOUCHER_MAX_BYTES_V1);
/// Qualification target for the terminal request/payment/ack delivery trio.
pub const KAGEMUSHA_SESSION_TARGET_BYTES_V1: usize = 8_960;
/// Absolute raw limit for the terminal request/payment/ack delivery trio.
pub const KAGEMUSHA_SESSION_MAX_BYTES_V1: usize = 9_211;
/// Absolute text limit for the terminal request/payment/ack delivery trio.
pub const KAGEMUSHA_TEXT_SESSION_MAX_BYTES_V1: usize = 12_288;
/// Qualification target for the two current recursive proofs.
pub const KAGEMUSHA_PAIRED_PROOF_TARGET_BYTES_V1: usize = 6_144;
/// Absolute canonical byte limit for the complete paired-proof value.
pub const KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1: usize = 6_528;
/// Maximum combined bytes in the two current recursive-proof components.
///
/// The complete encoded-value cap remains authoritative and also accounts for
/// both fixed accumulators, all digests, lengths, and canonical framing.
pub const KAGEMUSHA_CURRENT_PROOFS_MAX_BYTES_V1: usize = 4_990;
/// Maximum bytes in either parity's current proof.
pub const KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1: usize = 2_495;
/// Exact compact delayed-history accumulator bytes for one `k=16` parity.
pub const KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1: usize = 544;
/// Maximum encrypted credit-opening bytes carried by a credit envelope.
pub const KAGEMUSHA_ENCRYPTED_CREDIT_MAX_BYTES_V1: usize = 384;
/// Maximum canonical bytes in the fixed private credit-opening plaintext.
pub const KAGEMUSHA_CREDIT_OPENING_MAX_BYTES_V1: usize = 256;
/// Exact X25519 public-key width used by encrypted credit envelopes.
pub const KAGEMUSHA_X25519_PUBLIC_KEY_BYTES_V1: usize = 32;
/// Exact XChaCha20-Poly1305 nonce width used by encrypted credit envelopes.
pub const KAGEMUSHA_XCHACHA20POLY1305_NONCE_BYTES_V1: usize = 24;
/// Exact authentication-tag width appended to an encrypted credit plaintext.
pub const KAGEMUSHA_XCHACHA20POLY1305_TAG_BYTES_V1: usize = 16;
/// HKDF-SHA256 salt label for the Kagemusha V1 encrypted-credit KEM.
pub const KAGEMUSHA_ENCRYPTED_CREDIT_KDF_SALT_LABEL_V1: &[u8] =
    b"iroha:kagemusha:v1:credit-envelope-salt\0";
/// HKDF-SHA256 info label for the Kagemusha V1 encrypted-credit KEM.
pub const KAGEMUSHA_ENCRYPTED_CREDIT_KDF_INFO_LABEL_V1: &[u8] =
    b"iroha:kagemusha:v1:credit-envelope-key\0";
/// Maximum canonical hardware-profile registry entry bytes.
pub const KAGEMUSHA_HARDWARE_PROFILE_MAX_BYTES_V1: usize = 512;
/// Maximum canonical compact hardware credential bytes.
pub const KAGEMUSHA_HARDWARE_CREDENTIAL_MAX_BYTES_V1: usize = 768;
/// Maximum hardware-owned staging metadata persisted beside one payment.
pub const KAGEMUSHA_INBOX_STAGING_METADATA_MAX_BYTES_V1: u32 = 512;
/// Minimum durable receiver staging bytes for one maximum payment and acknowledgement.
pub const KAGEMUSHA_INBOX_STAGE_MIN_BYTES_V1: u32 = KAGEMUSHA_PAYMENT_MAX_BYTES_V1 as u32
    + KAGEMUSHA_INBOX_STAGING_METADATA_MAX_BYTES_V1
    + KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1 as u32;
/// Maximum sealed transition inputs persisted for deterministic recovery.
pub const KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1: u32 = 2_048;
/// Maximum deterministic recovery-seed material persisted before proving.
pub const KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1: u32 = 512;
/// Maximum paired proof bytes retained inside the verified precommit candidate.
pub const KAGEMUSHA_PRECOMMIT_PAIRED_PROOF_MAX_BYTES_V1: u32 =
    outbox_budget_component_from_usize(KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1);
/// Maximum canonical precommit candidate metadata excluding its paired proof.
pub const KAGEMUSHA_PRECOMMIT_CANDIDATE_METADATA_MAX_BYTES_V1: u32 = 1_024;
/// Maximum authenticated durable-retry metadata beside one terminal envelope.
pub const KAGEMUSHA_OUTBOX_RETRY_METADATA_MAX_BYTES_V1: u32 = 512;

const fn outbox_budget_component_from_usize(value: usize) -> u32 {
    assert!(value <= u32::MAX as usize);
    value as u32
}

const fn checked_outbox_budget_v1(canonical_envelope_max_bytes: usize) -> u32 {
    let parts = [
        KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1,
        KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1,
        KAGEMUSHA_PRECOMMIT_PAIRED_PROOF_MAX_BYTES_V1,
        KAGEMUSHA_PRECOMMIT_CANDIDATE_METADATA_MAX_BYTES_V1,
        outbox_budget_component_from_usize(canonical_envelope_max_bytes),
        KAGEMUSHA_OUTBOX_RETRY_METADATA_MAX_BYTES_V1,
    ];
    let mut total = 0_u32;
    let mut index = 0;
    while index < parts.len() {
        total = match total.checked_add(parts[index]) {
            Some(next) => next,
            None => panic!("Kagemusha V1 outbox budget overflow"),
        };
        index += 1;
    }
    total
}

/// Minimum sender outbox reservation for all recoverable payment artifacts.
pub const KAGEMUSHA_PAYMENT_OUTBOX_MIN_BYTES_V1: u32 =
    checked_outbox_budget_v1(KAGEMUSHA_PAYMENT_MAX_BYTES_V1);
/// Minimum sender outbox reservation for all recoverable redemption artifacts.
pub const KAGEMUSHA_REDEMPTION_OUTBOX_MIN_BYTES_V1: u32 =
    checked_outbox_budget_v1(KAGEMUSHA_REDEMPTION_VOUCHER_MAX_BYTES_V1);
/// Hardware consumes only the exact aggregate-state predecessor.
pub const KAGEMUSHA_HARDWARE_CAPABILITY_EXACT_NEXT_PREDECESSOR_CONSUMPTION_V1: u16 = 1 << 0;
/// Hardware issues each successor authorization at most once.
pub const KAGEMUSHA_HARDWARE_CAPABILITY_ONE_USE_SUCCESSOR_AUTHORIZATION_V1: u16 = 1 << 1;
/// Hardware counter and journal state survive rollback and restore attempts.
pub const KAGEMUSHA_HARDWARE_CAPABILITY_ROLLBACK_RESISTANT_COUNTER_AND_JOURNAL_V1: u16 = 1 << 2;
/// Hardware seals transition inputs and deterministic recovery seeds.
pub const KAGEMUSHA_HARDWARE_CAPABILITY_SEALED_TRANSITION_RECOVERY_V1: u16 = 1 << 3;
/// Hardware makes a committed send an irrevocable receiver-bound credit.
pub const KAGEMUSHA_HARDWARE_CAPABILITY_RECEIVER_BOUND_CREDIT_COMMIT_V1: u16 = 1 << 4;
/// Hardware atomically records accepted credits and rollback-resistant inbox receipts.
pub const KAGEMUSHA_HARDWARE_CAPABILITY_ROLLBACK_RESISTANT_ACCEPTED_CREDIT_INBOX_V1: u16 = 1 << 5;
/// Hardware authenticates inbound staging, exact deduplication, and inbox paging.
pub const KAGEMUSHA_HARDWARE_CAPABILITY_AUTHENTICATED_INBOUND_STAGING_V1: u16 = 1 << 6;
/// Hardware recovers the authoritative replay root and authenticated sparse-tree state.
pub const KAGEMUSHA_HARDWARE_CAPABILITY_AUTHORITATIVE_REPLAY_ROOT_RECOVERY_V1: u16 = 1 << 7;
/// Hardware reserves sender terminal and envelope bytes before locking a predecessor.
pub const KAGEMUSHA_HARDWARE_CAPABILITY_SENDER_OUTBOX_RESERVATION_V1: u16 = 1 << 8;
/// Hardware owns an authenticated durable byte-identical retry outbox.
pub const KAGEMUSHA_HARDWARE_CAPABILITY_AUTHENTICATED_DURABLE_RETRY_OUTBOX_V1: u16 = 1 << 9;
/// Hardware atomically installs one recoverable successor and transition certificate.
pub const KAGEMUSHA_HARDWARE_CAPABILITY_ATOMIC_RECOVERABLE_TRANSITION_V1: u16 = 1 << 10;
/// Hardware authenticates the complete durable monetary state across restart and recovery.
pub const KAGEMUSHA_HARDWARE_CAPABILITY_AUTHENTICATED_DURABLE_STATE_V1: u16 = 1 << 11;
/// Hardware supplies the literal trusted commit time bound by every monetary transition.
pub const KAGEMUSHA_HARDWARE_CAPABILITY_TRUSTED_COMMIT_TIME_V1: u16 = 1 << 12;
/// Hardware rotates the complete balance and replay root offline.
pub const KAGEMUSHA_HARDWARE_CAPABILITY_OFFLINE_HARDWARE_EPOCH_ROTATION_V1: u16 = 1 << 13;
/// Hardware rolls exhausted counters without cloning spend authority.
pub const KAGEMUSHA_HARDWARE_CAPABILITY_ROLLBACK_SAFE_COUNTER_ROLLOVER_V1: u16 = 1 << 14;
/// Hardware fails closed instead of falling back to software authority.
pub const KAGEMUSHA_HARDWARE_CAPABILITY_NO_SOFTWARE_FALLBACK_V1: u16 = 1 << 15;
/// Exact capability set required from every Kagemusha V1 hardware profile.
pub const KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1: u16 =
    KAGEMUSHA_HARDWARE_CAPABILITY_EXACT_NEXT_PREDECESSOR_CONSUMPTION_V1
        | KAGEMUSHA_HARDWARE_CAPABILITY_ONE_USE_SUCCESSOR_AUTHORIZATION_V1
        | KAGEMUSHA_HARDWARE_CAPABILITY_ROLLBACK_RESISTANT_COUNTER_AND_JOURNAL_V1
        | KAGEMUSHA_HARDWARE_CAPABILITY_SEALED_TRANSITION_RECOVERY_V1
        | KAGEMUSHA_HARDWARE_CAPABILITY_RECEIVER_BOUND_CREDIT_COMMIT_V1
        | KAGEMUSHA_HARDWARE_CAPABILITY_ROLLBACK_RESISTANT_ACCEPTED_CREDIT_INBOX_V1
        | KAGEMUSHA_HARDWARE_CAPABILITY_AUTHENTICATED_INBOUND_STAGING_V1
        | KAGEMUSHA_HARDWARE_CAPABILITY_AUTHORITATIVE_REPLAY_ROOT_RECOVERY_V1
        | KAGEMUSHA_HARDWARE_CAPABILITY_SENDER_OUTBOX_RESERVATION_V1
        | KAGEMUSHA_HARDWARE_CAPABILITY_AUTHENTICATED_DURABLE_RETRY_OUTBOX_V1
        | KAGEMUSHA_HARDWARE_CAPABILITY_ATOMIC_RECOVERABLE_TRANSITION_V1
        | KAGEMUSHA_HARDWARE_CAPABILITY_AUTHENTICATED_DURABLE_STATE_V1
        | KAGEMUSHA_HARDWARE_CAPABILITY_TRUSTED_COMMIT_TIME_V1
        | KAGEMUSHA_HARDWARE_CAPABILITY_OFFLINE_HARDWARE_EPOCH_ROTATION_V1
        | KAGEMUSHA_HARDWARE_CAPABILITY_ROLLBACK_SAFE_COUNTER_ROLLOVER_V1
        | KAGEMUSHA_HARDWARE_CAPABILITY_NO_SOFTWARE_FALLBACK_V1;

const DEVICE_KEY_REFERENCE_DOMAIN: &[u8] = b"iroha:kagemusha:v1:device-key-reference";
const ASSET_IDENTITY_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:asset-identity";
const LIABILITY_POOL_DOMAIN: &[u8] = b"iroha:kagemusha:v1:liability-pool";
const AGGREGATE_STATE_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:aggregate-state";
/// Domain shared by the canonical compact outer state commitment and its recursive circuit.
pub const KAGEMUSHA_PASTA_STATE_COMMITMENT_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:pasta-state-commitment";
const REQUEST_SIGNING_DOMAIN: &[u8] = b"iroha:kagemusha:v1:payment-request-signing";
const REQUEST_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:payment-request";
const HARDWARE_PROFILE_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:hardware-profile";
const SUITE_COMMITMENT_DOMAIN: &[u8] = b"iroha:kagemusha:v1:suite-commitment";
const HARDWARE_CREDENTIAL_ID_DOMAIN: &[u8] = b"iroha:kagemusha:v1:hardware-credential-id";
const HARDWARE_CREDENTIAL_SIGNING_DOMAIN: &[u8] = b"iroha:kagemusha:v1:hardware-credential-signing";
const CREDIT_ID_DOMAIN: &[u8] = b"iroha:kagemusha:v1:credit-id";
const STATEMENT_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:send-split-statement";
const LIFECYCLE_BINDING_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:lifecycle-binding";
const CIPHERTEXT_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:ciphertext";
const PEER_CREDIT_CONTEXT_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:peer-credit-context";
const PEER_CREDIT_LIFECYCLE_CONTEXT_DIGEST_DOMAIN: &[u8] =
    b"iroha:kagemusha:v1:peer-credit-lifecycle-context";
const ACCOUNT_IDENTITY_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:account-identity";
const MINT_CREDIT_OPENING_COMMITMENT_DOMAIN: &[u8] =
    b"iroha:kagemusha:v1:mint-credit-opening-commitment";
const RECIPIENT_CREDENTIAL_COMMITMENT_DOMAIN: &[u8] =
    b"iroha:kagemusha:v1:recipient-credential-commitment";
const PAYMENT_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:payment";
const INBOX_RECEIPT_DOMAIN: &[u8] = b"iroha:kagemusha:v1:durable-inbox-receipt";
const ACKNOWLEDGEMENT_SIGNING_DOMAIN: &[u8] = b"iroha:kagemusha:v1:acknowledgement-signing";
const MINT_CREDIT_ID_DOMAIN: &[u8] = b"iroha:kagemusha:v1:mint-credit-id";
const MINT_LIFECYCLE_CONTEXT_DOMAIN: &[u8] = b"iroha:kagemusha:v1:mint-lifecycle-context";
const MINT_STATEMENT_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:mint-statement";
const MINT_AUTHORIZATION_CONTEXT_DIGEST_DOMAIN: &[u8] =
    b"iroha:kagemusha:v1:mint-authorization-context";
const MINT_AUTHORIZATION_STATEMENT_DIGEST_DOMAIN: &[u8] =
    b"iroha:kagemusha:v1:mint-authorization-statement";
const MINT_AUTHORIZATION_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:mint-authorization";
const REDEMPTION_ID_DOMAIN: &[u8] = b"iroha:kagemusha:v1:redemption-id";
const REDEMPTION_STATEMENT_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:redemption-statement";

/// Error returned when canonical Kagemusha V1 data fails validation.
#[derive(Debug)]
pub enum KagemushaValidationErrorV1 {
    /// Canonical Norito encoding or decoding failed.
    Codec(norito::Error),
    /// A bounded wire value exceeded its protocol limit.
    EncodedSizeExceeded {
        /// Encoded byte length.
        actual: usize,
        /// Maximum accepted byte length.
        max: usize,
    },
    /// A public field or binding was malformed.
    InvalidField {
        /// Stable field label.
        field: &'static str,
    },
}

impl core::fmt::Display for KagemushaValidationErrorV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Codec(error) => write!(f, "canonical Kagemusha V1 codec failed: {error}"),
            Self::EncodedSizeExceeded { actual, max } => {
                write!(f, "Kagemusha V1 wire size {actual} exceeds limit {max}")
            }
            Self::InvalidField { field } => {
                write!(f, "invalid Kagemusha V1 field `{field}`")
            }
        }
    }
}

impl std::error::Error for KagemushaValidationErrorV1 {}

impl From<norito::Error> for KagemushaValidationErrorV1 {
    fn from(error: norito::Error) -> Self {
        Self::Codec(error)
    }
}

/// Sole Kagemusha V1 device authority key.
///
/// The wire value is exactly one canonical uncompressed SEC1 NIST P-256 point
/// (`0x04 || x || y`). There is no algorithm tag or selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, IntoSchema)]
#[repr(transparent)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct KagemushaDevicePublicKeyV1([u8; KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1]);

/// Sole Kagemusha V1 device signature.
///
/// The wire value is the fixed-width big-endian ECDSA scalar pair `r || s`.
/// Both scalars must be in `1..n`, and `s` must be low.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, IntoSchema)]
#[repr(transparent)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct KagemushaDeviceSignatureV1([u8; KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V1]);

impl norito::NoritoSerialize for KagemushaDevicePublicKeyV1 {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        self.validate()
            .map_err(|error| norito::Error::Message(error.to_string()))?;
        writer.write_all(&self.0)?;
        Ok(())
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        Some(KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        self.encoded_len_hint()
    }
}

impl<'de> norito::NoritoDeserialize<'de> for KagemushaDevicePublicKeyV1 {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("Kagemusha device public key must be canonical SEC1 bytes")
    }

    fn try_deserialize(archived: &'de norito::core::Archived<Self>) -> Result<Self, norito::Error> {
        let bytes =
            norito::core::payload_slice_from_ptr(core::ptr::from_ref(archived).cast::<u8>())?;
        let (value, used) = <Self as norito::core::DecodeFromSlice>::decode_from_slice(bytes)?;
        if used != bytes.len() {
            return Err(norito::Error::LengthMismatch);
        }
        Ok(value)
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for KagemushaDevicePublicKeyV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::Error> {
        let raw = bytes
            .get(..KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1)
            .ok_or(norito::Error::LengthMismatch)?;
        let value = Self::from_sec1_bytes(raw)
            .map_err(|error| norito::Error::Message(error.to_string()))?;
        Ok((value, KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1))
    }
}

impl norito::NoritoSerialize for KagemushaDeviceSignatureV1 {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        self.validate()
            .map_err(|error| norito::Error::Message(error.to_string()))?;
        writer.write_all(&self.0)?;
        Ok(())
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        Some(KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V1)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        self.encoded_len_hint()
    }
}

impl<'de> norito::NoritoDeserialize<'de> for KagemushaDeviceSignatureV1 {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("Kagemusha device signature must be canonical raw P-256 bytes")
    }

    fn try_deserialize(archived: &'de norito::core::Archived<Self>) -> Result<Self, norito::Error> {
        let bytes =
            norito::core::payload_slice_from_ptr(core::ptr::from_ref(archived).cast::<u8>())?;
        let (value, used) = <Self as norito::core::DecodeFromSlice>::decode_from_slice(bytes)?;
        if used != bytes.len() {
            return Err(norito::Error::LengthMismatch);
        }
        Ok(value)
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for KagemushaDeviceSignatureV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::Error> {
        let raw = bytes
            .get(..KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V1)
            .ok_or(norito::Error::LengthMismatch)?;
        let value =
            Self::from_raw_bytes(raw).map_err(|error| norito::Error::Message(error.to_string()))?;
        Ok((value, KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V1))
    }
}

impl KagemushaDevicePublicKeyV1 {
    /// Parse the canonical uncompressed SEC1 P-256 encoding.
    ///
    /// # Errors
    ///
    /// Returns an error for the wrong width, a compressed or invalid point, or
    /// a non-canonical encoding.
    pub fn from_sec1_bytes(bytes: &[u8]) -> Result<Self, KagemushaValidationErrorV1> {
        let raw: [u8; KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1] =
            bytes.try_into().map_err(|_| invalid("device_public_key"))?;
        if raw[0] != 0x04 {
            return Err(invalid("device_public_key"));
        }
        let verifying_key =
            P256VerifyingKey::from_sec1_bytes(&raw).map_err(|_| invalid("device_public_key"))?;
        if verifying_key.to_encoded_point(false).as_bytes() != raw {
            return Err(invalid("device_public_key"));
        }
        Ok(Self(raw))
    }

    /// Validate a value obtained through a raw Norito or JSON decoder.
    ///
    /// # Errors
    ///
    /// Returns an error unless the key is one canonical uncompressed P-256 point.
    pub fn validate(&self) -> Result<(), KagemushaValidationErrorV1> {
        Self::from_sec1_bytes(&self.0).map(|_| ())
    }

    /// Return the canonical uncompressed SEC1 bytes.
    #[must_use]
    pub const fn as_sec1_bytes(&self) -> &[u8; KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1] {
        &self.0
    }

    fn verifying_key(&self) -> Result<P256VerifyingKey, KagemushaValidationErrorV1> {
        self.validate()?;
        P256VerifyingKey::from_sec1_bytes(&self.0).map_err(|_| invalid("device_public_key"))
    }
}

impl TryFrom<&[u8]> for KagemushaDevicePublicKeyV1 {
    type Error = KagemushaValidationErrorV1;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        Self::from_sec1_bytes(value)
    }
}

impl TryFrom<[u8; KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1]> for KagemushaDevicePublicKeyV1 {
    type Error = KagemushaValidationErrorV1;

    fn try_from(
        value: [u8; KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1],
    ) -> Result<Self, Self::Error> {
        Self::from_sec1_bytes(&value)
    }
}

impl AsRef<[u8]> for KagemushaDevicePublicKeyV1 {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl KagemushaDeviceSignatureV1 {
    /// Parse a canonical fixed-width low-S P-256 ECDSA signature.
    ///
    /// # Errors
    ///
    /// Returns an error for the wrong width, invalid scalars, or a high-S signature.
    pub fn from_raw_bytes(bytes: &[u8]) -> Result<Self, KagemushaValidationErrorV1> {
        let raw: [u8; KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V1] =
            bytes.try_into().map_err(|_| invalid("device_signature"))?;
        let signature = P256Signature::from_slice(&raw).map_err(|_| invalid("device_signature"))?;
        if signature.normalize_s().is_some() {
            return Err(invalid("device_signature"));
        }
        Ok(Self(raw))
    }

    /// Validate a value obtained through a raw Norito or JSON decoder.
    ///
    /// # Errors
    ///
    /// Returns an error unless the signature is fixed-width and low-S.
    pub fn validate(&self) -> Result<(), KagemushaValidationErrorV1> {
        Self::from_raw_bytes(&self.0).map(|_| ())
    }

    /// Return the canonical fixed-width `r || s` bytes.
    #[must_use]
    pub const fn as_raw_bytes(&self) -> &[u8; KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V1] {
        &self.0
    }

    /// Verify ECDSA-P256-SHA256 under the fixed Kagemusha V1 profile.
    ///
    /// # Errors
    ///
    /// Returns an error when the key, signature, or authentication is invalid.
    pub fn verify(
        &self,
        public_key: &KagemushaDevicePublicKeyV1,
        message: &[u8],
    ) -> Result<(), KagemushaValidationErrorV1> {
        self.validate()?;
        let signature =
            P256Signature::from_slice(&self.0).map_err(|_| invalid("device_signature"))?;
        public_key
            .verifying_key()?
            .verify(message, &signature)
            .map_err(|_| invalid("device_signature"))
    }
}

impl TryFrom<&[u8]> for KagemushaDeviceSignatureV1 {
    type Error = KagemushaValidationErrorV1;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        Self::from_raw_bytes(value)
    }
}

impl TryFrom<[u8; KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V1]> for KagemushaDeviceSignatureV1 {
    type Error = KagemushaValidationErrorV1;

    fn try_from(value: [u8; KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V1]) -> Result<Self, Self::Error> {
        Self::from_raw_bytes(&value)
    }
}

impl AsRef<[u8]> for KagemushaDeviceSignatureV1 {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// Constant-size public metadata for one privately valued aggregate balance.
///
/// Folding any number of credits changes the single recursive
/// `state_commitment` without retaining a public input, hop, or provenance
/// list.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaAggregateStateCommitmentV1 {
    /// Wire version.
    pub version: u16,
    /// Authenticated proof-release identifier.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub release_id: [u8; 32],
    /// Exact network identity.
    pub network_id: NetworkId,
    /// Asset represented by this aggregate state.
    pub asset: AssetDefinitionId,
    /// Exact asset incarnation represented by this aggregate state.
    pub asset_incarnation: AxtAssetIncarnationV1,
    /// Authoritative asset scale.
    pub scale: u32,
    /// Canonical network, asset, and incarnation reserve-liability pool.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub liability_pool_id: [u8; 32],
    /// Hardware-controlled state lane.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub lane_id: [u8; 32],
    /// Hardware epoch that scopes sequence reuse after secure re-provisioning.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub hardware_epoch_id: [u8; 32],
    /// Reference to the hardware-authorized state key.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub key_reference: [u8; 32],
    /// Authenticated hardware-policy registry root.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub hardware_policy_id: [u8; 32],
    /// Monotonic logical state sequence within `(lane_id, hardware_epoch_id)`.
    pub sequence: u128,
    /// Commitment to the complete private aggregate balance state and recursive history.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub state_commitment: [u8; 32],
}

/// Canonical field components jointly authenticated by the paired Pasta state proofs.
///
/// `eq` is the little-endian canonical `Fp` output of the Eq state circuit and `ep` is the
/// little-endian canonical `Fq` output of the Ep state circuit. Neither component is monetary
/// authority by itself. The wire state head is
/// [`kagemusha_pasta_state_commitment_v1`], while the paired verifier requires both proofs to
/// expose this exact pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaPastaStateCommitmentV1 {
    /// Eq/Fp native state-commitment component.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub eq: [u8; 32],
    /// Ep/Fq native state-commitment component.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ep: [u8; 32],
}

impl KagemushaPastaStateCommitmentV1 {
    /// Canonical absent component pair used only where a proof role has no aggregate state.
    pub const ZERO: Self = Self {
        eq: [0; 32],
        ep: [0; 32],
    };

    /// Return true only for the canonical all-zero pair.
    #[must_use]
    pub fn is_zero(self) -> bool {
        self == Self::ZERO
    }
}

/// Derive the sole 32-byte public state head from both parity-native commitment components.
///
/// SHA-256 is only the compact collision-resistant name of the pair. Admission still requires
/// an Eq proof for `components.eq` and an Ep proof for `components.ep`; hashing a pair does not
/// grant monetary authority.
#[must_use]
pub fn kagemusha_pasta_state_commitment_v1(
    components: KagemushaPastaStateCommitmentV1,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(KAGEMUSHA_PASTA_STATE_COMMITMENT_DOMAIN_V1);
    hasher.update([0]);
    hasher.update(components.eq);
    hasher.update(components.ep);
    hasher.finalize().into()
}

/// Closed paired-Pasta proof and delayed-history accumulators.
///
/// Every proof instance, including sender split and mint authorization,
/// uses fresh circuit randomness. Credential audits and history accumulators
/// are statement-scoped projections that bind `semantic_digest`; they must
/// never be a stable credential, lane, device, or predecessor pseudonym that
/// links otherwise unrelated transcripts.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaPairedProofV1 {
    /// Wire version.
    pub version: u16,
    /// Canonical little-endian Fp Poseidon digest of the exact Eq circuit protocol.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub eq_protocol_digest: [u8; 32],
    /// Canonical little-endian Fq Poseidon digest of the exact Ep circuit protocol.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ep_protocol_digest: [u8; 32],
    /// Digest of the common semantic statement constrained by both proofs.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub semantic_digest: [u8; 32],
    /// Fresh statement-scoped Fp Poseidon audit of credential proofs recursively accepted by Eq GuardBundle.
    ///
    /// Both state parities expose these same two `u128` limbs so Eq and Ep cannot select
    /// independently valid but mutually unrelated platform credentials. The
    /// circuit binds the semantic digest and fresh proof blinding, so this is
    /// not a reusable credential identifier.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub guard_eq_credential_audit: [u8; 32],
    /// Fresh statement-scoped Fq Poseidon audit of credential proofs recursively accepted by Ep GuardBundle.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub guard_ep_credential_audit: [u8; 32],
    /// Canonical Fp Poseidon commitment to the Eq scalar-verifier equations.
    ///
    /// The Ep proof reconstructs and enforces those equations over Eq's native base field and
    /// exposes the same two `u128` limbs. This cross-parity binding is part of monetary authority,
    /// not descriptive metadata.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub eq_deferred_audit: [u8; 32],
    /// Canonical Fq Poseidon commitment to the Ep scalar-verifier equations.
    ///
    /// The Eq proof reconstructs and enforces those equations over Ep's native base field and
    /// exposes the same two `u128` limbs.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ep_deferred_audit: [u8; 32],
    /// Current Eq/Fp augmented IPA proof.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub eq_proof: Vec<u8>,
    /// Current Ep/Fq augmented IPA proof.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub ep_proof: Vec<u8>,
    /// Freshly rerandomized, statement-bound compact Eq/Fp delayed-history accumulator.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub eq_history: Vec<u8>,
    /// Freshly rerandomized, statement-bound compact Ep/Fq delayed-history accumulator.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub ep_history: Vec<u8>,
}

/// Qualified platform class represented by one governed hardware profile.
///
/// A class label never grants Kagemusha authority by itself. The complete
/// profile, physical qualification evidence, credential, and recursive proof
/// must all validate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "class", content = "value", rename_all = "snake_case")]
#[norito(deny_unknown_fields)]
pub enum KagemushaHardwarePlatformClassV1 {
    /// Qualified Android OEM or secure-element service.
    AndroidOemService,
    /// Qualified Apple OEM or secure-element service.
    AppleOemService,
    /// Qualified dedicated secure element outside a stock mobile API.
    DedicatedSecureElement,
    /// Other governed implementation with equivalent physical evidence.
    OtherQualified,
}

/// Governed Kagemusha V1 non-forking hardware-service profile.
///
/// The capability mask must equal
/// [`KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1`]. A stock hardware-backed
/// signing key is therefore insufficient unless its surrounding service
/// implements and qualifies the complete counter, journal, capacity,
/// atomic-commit, recovery, time/lease, rotation, and no-fallback contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaHardwareProfileV1 {
    /// Wire version.
    pub version: u16,
    /// Protocol version admitted by this profile; V1 accepts only `1`.
    pub protocol_version: u16,
    /// Domain-separated digest of the unsigned canonical profile body.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub hardware_profile_id: [u8; 32],
    /// Stable provider identity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub provider_id: [u8; 32],
    /// Qualified platform/service class.
    pub platform_class: KagemushaHardwarePlatformClassV1,
    /// Digest of the exact governed hardware product class.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub product_class_digest: [u8; 32],
    /// Digest of the exact accepted firmware and secure-service policy.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub firmware_policy_digest: [u8; 32],
    /// Digest of the online enrollment-attestation verifier implementation.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub enrollment_attestation_verifier_digest: [u8; 32],
    /// Digest of the exact attestation trust-root set.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub attestation_trust_roots_digest: [u8; 32],
    /// Commitment to the sole proof suite admitted for issued credentials.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub allowed_suite_commitment: [u8; 32],
    /// Exact governance policy epoch admitted by this profile.
    pub policy_epoch: u64,
    /// P-256 key authorized to issue compact device credentials for this profile.
    pub governance_credential_public_key: KagemushaDevicePublicKeyV1,
    /// Exact required capability bit set; missing and unknown bits fail closed.
    pub capability_mask: u16,
    /// Digest of the physical-device qualification report admitted by governance.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub qualification_report_digest: [u8; 32],
    /// Inclusive profile activation time in Unix milliseconds.
    pub valid_from_ms: u64,
    /// Exclusive profile credential-issuance deadline in Unix milliseconds.
    pub expires_at_ms: u64,
}

/// Compact governance credential consumed by the recursive hardware guard.
///
/// Raw OEM/platform attestation remains an online enrollment input. This
/// credential is the sole compact V1 projection and binds the enrolled device
/// key to the exact network, hardware profile, firmware policy, policy epoch,
/// lane commitment, hardware epoch, and expiry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaHardwareCredentialV1 {
    /// Wire version.
    pub version: u16,
    /// Digest-derived credential identity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub credential_id: [u8; 32],
    /// Exact network on which the device may authorize Kagemusha.
    pub network_id: NetworkId,
    /// Governed hardware profile identity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub hardware_profile_id: [u8; 32],
    /// Exact proof suite selected under the profile's allowed-suite commitment.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub suite_id: [u8; 32],
    /// Exact firmware policy admitted for the device.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub firmware_policy_digest: [u8; 32],
    /// Strictly positive governance policy epoch.
    pub policy_epoch: u64,
    /// Hiding commitment to the device's authoritative aggregate-state lane.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub lane_commitment: [u8; 32],
    /// Rollback-resistant hardware epoch identity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub hardware_epoch_id: [u8; 32],
    /// Monotonic hardware epoch generation used for rollover and rotation.
    pub hardware_epoch_generation: u64,
    /// Device transition, request, staging, and acknowledgement authority key.
    pub device_public_key: KagemushaDevicePublicKeyV1,
    /// Domain-separated reference to `device_public_key`.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub device_key_reference: [u8; 32],
    /// Inclusive credential issuance time in Unix milliseconds.
    pub issued_at_ms: u64,
    /// Exclusive credential expiry in Unix milliseconds.
    pub expires_at_ms: u64,
    /// Governance/profile-issuer signature over the exact compact credential.
    pub governance_signature: KagemushaDeviceSignatureV1,
}

/// Exact recipient-only plaintext protected by an encrypted credit envelope.
///
/// The opening is fixed-size and history-independent. Qualified hardware must
/// decode these canonical bytes after AEAD authentication and reject any
/// public `credit_id` or `amount` mismatch before admitting the credit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaCreditOpeningV1 {
    /// Wire version.
    pub version: u16,
    /// Exact public credit identity bound into the AEAD associated data.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub credit_id: [u8; 32],
    /// Exact public credit amount in atomic units.
    pub amount: u128,
    /// Private opening of the pre-ID credit commitment.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub credit_commitment_opening: [u8; 32],
    /// Private opening of the randomized recipient binding.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub recipient_binding_opening: [u8; 32],
    /// Fresh private recovery nonce used by qualified hardware.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub recovery_nonce: [u8; 32],
}

/// Domain selector carried by encrypted-credit associated data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "purpose", content = "value", rename_all = "snake_case")]
#[norito(deny_unknown_fields)]
pub enum KagemushaEncryptedCreditPurposeV1 {
    /// Reserve-backed online top-up credit.
    Mint,
    /// Receiver-bound peer-to-peer payment credit.
    Peer,
}

/// Canonical associated data authenticated by every encrypted credit.
///
/// No ciphertext digest or proof appears here, so the recipient key, opening
/// commitment, credit ID, AEAD bytes, and recursive proof have an acyclic
/// construction order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaEncryptedCreditAadV1 {
    /// Wire version.
    pub version: u16,
    /// Whether this envelope carries a mint or peer credit.
    pub purpose: KagemushaEncryptedCreditPurposeV1,
    /// Exact mint-authorization or peer-transfer pre-ID context digest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub context_digest: [u8; 32],
    /// Mint issuance commitment or peer transition/opening commitment.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub issuance_or_transition_commitment: [u8; 32],
    /// Final credit identity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub credit_id: [u8; 32],
    /// Exact positive amount in atomic units.
    pub amount: u128,
}

/// X25519/HKDF-SHA256/XChaCha20-Poly1305 encrypted credit envelope.
///
/// `encrypted_credit` fields elsewhere on the V1 wire contain the exact
/// canonical Norito encoding of this value. The recipient key is supplied by
/// the signed request or mint-authorization context and is not repeated here.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaEncryptedCreditEnvelopeV1 {
    /// Wire version.
    pub version: u16,
    /// Fresh sender X25519 ephemeral public key.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ephemeral_x25519_public_key: [u8; KAGEMUSHA_X25519_PUBLIC_KEY_BYTES_V1],
    /// Fresh XChaCha20-Poly1305 nonce.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub nonce: [u8; KAGEMUSHA_XCHACHA20POLY1305_NONCE_BYTES_V1],
    /// Combined ciphertext followed by the exact 16-byte Poly1305 tag.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub ciphertext_and_tag: Vec<u8>,
}

/// Monetary operation bound by a released V1 transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "operation", content = "value", rename_all = "snake_case")]
#[norito(deny_unknown_fields)]
pub enum KagemushaOperationKindV1 {
    /// Establish a zero-balance hardware lane.
    Bootstrap,
    /// Fold one finalized reserve-backed mint credit.
    MintFold,
    /// Produce one receiver-bound payment credit.
    SendSplit,
    /// Fold exactly one receiver-bound credit into the aggregate balance.
    ReceiveFold,
    /// Produce one online redemption voucher.
    RedeemSplit,
    /// Rotate the hardware epoch and, when governed, the recursive verifier suite
    /// without changing monetary value.
    Rotate,
}

/// Complete lifecycle context bound by every released V1 transition.
///
/// This fixed record contains no hop, ancestry, origin, receipt-count, fan-in,
/// proof-depth, historical-transition counter, lane, hardware epoch, journal
/// revision, predecessor, or successor. Payment-only identities are nonzero
/// only for `SendSplit` and are canonical zero values for every other operation.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaLifecycleBindingV1 {
    /// Wire version.
    pub version: u16,
    /// Exact network identity.
    pub network_id: NetworkId,
    /// Protocol version; V1 accepts only `1`.
    pub protocol_version: u16,
    /// Governed proof-suite identity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub suite_id: [u8; 32],
    /// Digest of the exact verifying-key set.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub vk_digest: [u8; 32],
    /// Authenticated proof-release identifier.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub release_id: [u8; 32],
    /// Exact asset identity.
    pub asset: AssetDefinitionId,
    /// Exact asset incarnation.
    pub asset_incarnation: AxtAssetIncarnationV1,
    /// Authoritative asset scale.
    pub scale: u32,
    /// Canonical single pooled-reserve identity for the network, asset, and incarnation.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub liability_pool_id: [u8; 32],
    /// Qualified sender hardware profile.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub hardware_profile_id: [u8; 32],
    /// Sender hardware-policy epoch.
    pub policy_epoch: u64,
    /// Exact monetary operation.
    pub operation_kind: KagemushaOperationKindV1,
    /// Receiver request identity for `SendSplit`, otherwise zero.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub request_id: [u8; 32],
    /// Receiver credit identity for `SendSplit`, otherwise zero.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub credit_id: [u8; 32],
    /// Encrypted-credit digest for `SendSplit`, otherwise zero.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ciphertext_digest: [u8; 32],
}

/// Receiver-created authorization for one payment into a stable inbox lane.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaPaymentRequestV1 {
    /// Wire version.
    pub version: u16,
    /// Authenticated proof-release identifier.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub release_id: [u8; 32],
    /// Exact network identity.
    pub network_id: NetworkId,
    /// Requested asset.
    pub asset: AssetDefinitionId,
    /// Exact asset incarnation.
    pub asset_incarnation: AxtAssetIncarnationV1,
    /// Authoritative asset scale.
    pub scale: u32,
    /// Canonical network, asset, and incarnation reserve-liability pool.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub liability_pool_id: [u8; 32],
    /// Recipient account identity.
    pub recipient: AccountId,
    /// Stable receiver hardware lane authorized to accept and fold every matching credit.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub recipient_lane_id: [u8; 32],
    /// Recipient X25519 key protecting every credit opening made against this request.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub recipient_encryption_key: [u8; 32],
    /// Exact positive amount authorized by every payment made against this request.
    pub amount: u128,
    /// Compact qualified-hardware credential authorizing this request and its inbox lane.
    pub hardware_credential: KagemushaHardwareCredentialV1,
    /// Unique recipient nonce.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub request_id: [u8; 32],
    /// Request creation time in Unix milliseconds.
    pub issued_at_ms: u64,
    /// Exclusive sender-commit deadline in Unix milliseconds.
    pub expires_at_ms: u64,
    /// Low-S P-256 signature over the exact unsigned request.
    pub signature: KagemushaDeviceSignatureV1,
}

/// Unlinkable public send statement decided by both Pasta parities.
///
/// The private candidate proof consumes and creates aggregate state, but those
/// predecessor and successor commitments never appear in this public record.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaTransferStatementV1 {
    /// Wire version.
    pub version: u16,
    /// Complete released-credit lifecycle binding.
    pub lifecycle: KagemushaLifecycleBindingV1,
    /// Positive transfer amount in atomic units.
    pub amount: u128,
    /// Unique, proof-derived transition nullifier with no public state preimage.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub transition_nullifier: [u8; 32],
    /// Opaque aggregate-state commitment consumed by sender hardware.
    pub sender_before_commitment: KagemushaPastaStateCommitmentV1,
    /// Opaque exact successor commitment installed by sender hardware.
    pub sender_after_commitment: KagemushaPastaStateCommitmentV1,
    /// Digest of the exact recipient request.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub request_digest: [u8; 32],
    /// Stable receiver lane copied from the signed request.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub recipient_lane_id: [u8; 32],
    /// Request-scoped recipient encryption key copied from the signed request.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub recipient_encryption_key: [u8; 32],
    /// Commitment to amount-bound ciphertext semantics.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ciphertext_commitment: [u8; 32],
    /// Literal trusted hardware commit time in Unix milliseconds.
    pub committed_at_ms: u64,
    /// Commitment to the normalized exact-next hardware transition and recoverable certificate.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub hardware_transition_commitment: [u8; 32],
}

/// Sender response containing one receiver-bound aggregate credit proof.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaPaymentV1 {
    /// Wire version.
    pub version: u16,
    /// Unlinkable public statement decided by both proof parities.
    pub statement: KagemushaTransferStatementV1,
    /// Constant-size recursive proof of the aggregate split and normalized
    /// hardware transition.
    pub proof: KagemushaPairedProofV1,
    /// Recipient-only encrypted credit opening.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub encrypted_credit: Vec<u8>,
}

/// Durable secure-inbox record named by a receiver acknowledgement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaInboxReceiptV1 {
    /// Wire version.
    pub version: u16,
    /// Persisted output credit.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub credit_id: [u8; 32],
    /// Opaque commitment to the private persisted lane, hardware epoch,
    /// sequence, payment, and credit.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub receipt_commitment: [u8; 32],
}

/// Receiver acknowledgement emitted only after durable inbox persistence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaAcknowledgementV1 {
    /// Wire version.
    pub version: u16,
    /// Digest of the accepted recipient request.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub request_digest: [u8; 32],
    /// Digest of the durably persisted sender response.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub payment_digest: [u8; 32],
    /// Durable receiver-inbox receipt; this is not a receiver balance head.
    pub inbox_receipt: KagemushaInboxReceiptV1,
    /// Low-S P-256 signature over the acknowledgement fields.
    pub signature: KagemushaDeviceSignatureV1,
}

/// Pre-ID recipient context authorized before a reserve debit may occur.
///
/// The two randomized commitments and recipient encryption key are sampled
/// before, and independently of, the issuance commitment and credit ID. This
/// exact context digest is included in both derived identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaMintAuthorizationContextV1 {
    /// Wire version.
    pub version: u16,
    /// Unique idempotent top-up operation selected before authorization.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub operation_id: [u8; 32],
    /// Authenticated release selected for the complete mint relation.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub release_id: [u8; 32],
    /// Exact governed proof suite selected from that release.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub suite_id: [u8; 32],
    /// Digest of the exact release-pinned verifying-key set.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub vk_digest: [u8; 32],
    /// Digest of the exact authenticated proof artifact manifest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub artifact_manifest_digest: [u8; 32],
    /// Exact network whose reserve accepts the liability.
    pub network_id: NetworkId,
    /// Asset represented by the mint credit.
    pub asset: AssetDefinitionId,
    /// Exact registered incarnation of `asset`.
    pub asset_incarnation: AxtAssetIncarnationV1,
    /// Authoritative fixed asset scale.
    pub scale: u32,
    /// Sole deterministic pooled reserve for the asset incarnation.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub liability_pool_id: [u8; 32],
    /// Positive amount in atomic units.
    pub amount: u128,
    /// Online account debited by the top-up.
    pub payer: AccountId,
    /// Kagemusha account authorized to receive the credit.
    pub recipient: AccountId,
    /// Exact compact credential privately opened by the authorization proof.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub hardware_credential_id: [u8; 32],
    /// Release-enabled non-forking hardware profile proven by the authorization.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub hardware_profile_id: [u8; 32],
    /// Exact enabled hardware-policy epoch.
    pub policy_epoch: u64,
    /// Fresh commitment to the authenticated credential and its private opening.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub recipient_credential_commitment: [u8; 32],
    /// Fresh pre-ID commitment to the exact foldable credit opening.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub credit_commitment: [u8; 32],
    /// Fresh recipient encryption key whose private half is proven in hardware.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub recipient_one_time_key: [u8; 32],
}

/// Exact pre-debit mint authorization statement.
///
/// Its context is ID-independent. The final fields are populated only after
/// deriving the issuance commitment and credit ID and creating AEAD bytes. The
/// proof therefore authorizes the exact ciphertext without participating in
/// either identifier and cannot create a hash/encryption fixed point.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaMintAuthorizationStatementV1 {
    /// Wire version.
    pub version: u16,
    /// Complete pre-ID recipient authorization context.
    pub context: KagemushaMintAuthorizationContextV1,
    /// Derived pre-encryption issuance commitment.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub issuance_commitment: [u8; 32],
    /// Derived output credit ID subsequently bound into AEAD.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub credit_id: [u8; 32],
    /// Digest of the exact recipient-decryptable AEAD bytes.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ciphertext_digest: [u8; 32],
}

/// Release-pinned recipient authorization verified before reserve mutation.
///
/// Both proof parities establish the exact authenticated credential, fresh
/// commitment openings, recipient encryption-key possession, and ciphertext
/// opening relation described by `statement`. The finalized mint helper later
/// recursively verifies this same authorization digest.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaMintAuthorizationV1 {
    /// Wire version.
    pub version: u16,
    /// Exact recipient and output statement.
    pub statement: KagemushaMintAuthorizationStatementV1,
    /// Paired release-pinned proof of the recipient hardware relation.
    pub proof: KagemushaPairedProofV1,
}

/// Public top-up statement creating one foldable aggregate credit.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaMintCreditStatementV1 {
    /// Wire version.
    pub version: u16,
    /// Complete released `MintFold` lifecycle binding.
    pub lifecycle: KagemushaLifecycleBindingV1,
    /// Per-mint randomized commitment to the recipient credential authenticated
    /// online and proven privately by the mint helper. The stable credential
    /// identity, lane, hardware epoch, device key, and raw credential do not
    /// appear in the public credit. It is sampled independently of the final
    /// credit ID so mint construction remains acyclic.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub recipient_credential_commitment: [u8; 32],
    /// Digest of the exact pre-ID recipient authorization context.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub authorization_context_digest: [u8; 32],
    /// Digest of the complete pre-debit authorization and paired proof.
    ///
    /// This field is intentionally excluded from the credit-ID preimage: the
    /// authorization itself contains that ID. The finalized helper proves the
    /// complete reciprocal binding instead.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub mint_authorization_digest: [u8; 32],
    /// Positive minted amount in atomic units.
    pub amount: u128,
    /// Unique committed online issuance event.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub issuance_commitment: [u8; 32],
    /// Online account that authorized the top-up.
    pub recipient: AccountId,
    /// Receiver-bound pre-ID credit commitment created by the issuance proof.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub credit_commitment: [u8; 32],
    /// Trusted committed-ledger time in Unix milliseconds.
    pub minted_at_ms: u64,
}

/// Constant-size authenticated top-up credit folded into aggregate state.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaMintCreditV1 {
    /// Wire version.
    pub version: u16,
    /// Public issuance statement.
    pub statement: KagemushaMintCreditStatementV1,
    /// Paired proof of committed reserve liability and valid credit creation.
    pub proof: KagemushaPairedProofV1,
    /// Exact cross-parity finality-certificate binding exposed by both helper proofs.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub finality_certificate_binding: [u8; 32],
    /// Recursively authenticated finality-roster identifier used by this mint.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub finality_authority_head: [u8; 32],
    /// Release-pinned genesis finality-roster identifier carried by the authority chain.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub finality_genesis_roster_id: [u8; 32],
    /// Canonical binding of both helper proofs, audits, and complete history accumulators.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub finality_proof_binding_digest: [u8; 32],
    /// Recipient-only encrypted credit opening.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub encrypted_credit: Vec<u8>,
    /// Digest of the authenticated artifact manifest used by the proof.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub artifact_manifest_digest: [u8; 32],
}

/// Unlinkable terminal transition that converts aggregate cash to an online claim.
///
/// Stable sender identity remains hidden while opaque predecessor/successor
/// commitments and the normalized hardware transition are bound publicly.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaRedemptionStatementV1 {
    /// Wire version.
    pub version: u16,
    /// Complete released terminal lifecycle binding.
    pub lifecycle: KagemushaLifecycleBindingV1,
    /// Positive redeemed amount in atomic units.
    pub amount: u128,
    /// Public account credited by successful online redemption.
    pub beneficiary: AccountId,
    /// Unique, proof-derived terminal nullifier with no public state preimage.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub terminal_nullifier: [u8; 32],
    /// Opaque aggregate-state commitment consumed by sender hardware.
    pub sender_before_commitment: KagemushaPastaStateCommitmentV1,
    /// Opaque exact successor commitment installed by sender hardware.
    pub sender_after_commitment: KagemushaPastaStateCommitmentV1,
    /// Commitment to the public redemption claim and private proof output.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub redemption_commitment: [u8; 32],
    /// Unique identity of this exact redemption voucher.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub redemption_id: [u8; 32],
    /// Literal trusted hardware commit time in Unix milliseconds.
    pub committed_at_ms: u64,
    /// Commitment to the normalized exact-next hardware transition and
    /// recoverable private certificate.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub hardware_transition_commitment: [u8; 32],
}

/// Constant-size terminal voucher submitted for online redemption.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaRedemptionVoucherV1 {
    /// Wire version.
    pub version: u16,
    /// Public terminal transition.
    pub statement: KagemushaRedemptionStatementV1,
    /// Constant-size recursive proof of balance conservation and the normalized
    /// hardware transition.
    pub proof: KagemushaPairedProofV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.kagemusha.v1.liability-pool-preimage")]
struct LiabilityPoolPreimageV1 {
    network_id: NetworkId,
    asset: AssetDefinitionId,
    asset_incarnation: AxtAssetIncarnationV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.kagemusha.v1.hardware-profile-id-preimage")]
struct HardwareProfileIdPreimageV1 {
    version: u16,
    protocol_version: u16,
    provider_id: [u8; 32],
    platform_class: KagemushaHardwarePlatformClassV1,
    product_class_digest: [u8; 32],
    firmware_policy_digest: [u8; 32],
    enrollment_attestation_verifier_digest: [u8; 32],
    attestation_trust_roots_digest: [u8; 32],
    allowed_suite_commitment: [u8; 32],
    policy_epoch: u64,
    governance_credential_public_key: KagemushaDevicePublicKeyV1,
    capability_mask: u16,
    qualification_report_digest: [u8; 32],
    valid_from_ms: u64,
    expires_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.kagemusha.v1.payment-request-signing-preimage")]
struct PaymentRequestSigningPreimageV1 {
    domain: Vec<u8>,
    version: u16,
    release_id: [u8; 32],
    network_id: NetworkId,
    asset: AssetDefinitionId,
    asset_incarnation: AxtAssetIncarnationV1,
    scale: u32,
    liability_pool_id: [u8; 32],
    recipient: AccountId,
    recipient_lane_id: [u8; 32],
    recipient_encryption_key: [u8; 32],
    amount: u128,
    hardware_credential_id: [u8; 32],
    request_id: [u8; 32],
    issued_at_ms: u64,
    expires_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.kagemusha.v1.credit-id-preimage")]
struct CreditIdPreimageV1 {
    transition_nullifier: [u8; 32],
    request_digest: [u8; 32],
    sender_before_commitment: KagemushaPastaStateCommitmentV1,
    sender_after_commitment: KagemushaPastaStateCommitmentV1,
    recipient_lane_id: [u8; 32],
    recipient_encryption_key: [u8; 32],
    amount: u128,
    ciphertext_commitment: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.kagemusha.v1.peer-credit-lifecycle-context-preimage")]
struct PeerCreditLifecycleContextPreimageV1 {
    version: u16,
    network_id: NetworkId,
    protocol_version: u16,
    suite_id: [u8; 32],
    vk_digest: [u8; 32],
    release_id: [u8; 32],
    asset: AssetDefinitionId,
    asset_incarnation: AxtAssetIncarnationV1,
    scale: u32,
    liability_pool_id: [u8; 32],
    hardware_profile_id: [u8; 32],
    policy_epoch: u64,
    operation_kind: KagemushaOperationKindV1,
    request_id: [u8; 32],
}

/// Exact pre-ID peer-transfer context authenticated by encrypted-credit AAD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaPeerCreditContextV1 {
    /// Wire version.
    pub version: u16,
    /// Digest of the exact signed receiver request.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub request_digest: [u8; 32],
    /// Opaque aggregate-state commitment consumed by the sender.
    pub sender_before_commitment: KagemushaPastaStateCommitmentV1,
    /// Opaque exact successor commitment installed by the sender.
    pub sender_after_commitment: KagemushaPastaStateCommitmentV1,
    /// Digest of the released lifecycle fields that exist before encryption.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub lifecycle_context_digest: [u8; 32],
    /// Signed stable receiver lane.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub recipient_lane_id: [u8; 32],
    /// Signed request-scoped recipient X25519 public key.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub recipient_encryption_key: [u8; 32],
    /// Literal hardware-attested sender commit time.
    pub committed_at_ms: u64,
    /// Commitment to the normalized exact-next hardware transition.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub hardware_transition_commitment: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.kagemusha.v1.mint-credit-opening-commitment-preimage")]
struct MintCreditOpeningCommitmentPreimageV1 {
    version: u16,
    network_id: [u8; 32],
    asset_identity_digest: [u8; 32],
    asset_incarnation: AxtAssetIncarnationV1,
    scale: u32,
    liability_pool_id: [u8; 32],
    amount: u128,
    recipient_account_digest: [u8; 32],
    recipient_one_time_key: [u8; 32],
    credit_commitment_opening: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.kagemusha.v1.recipient-credential-commitment-preimage")]
struct RecipientCredentialCommitmentPreimageV1 {
    operation_id: [u8; 32],
    hardware_credential_id: [u8; 32],
    recipient_binding_opening: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.kagemusha.v1.hardware-credential-id-preimage")]
struct HardwareCredentialIdPreimageV1 {
    version: u16,
    network_id: NetworkId,
    hardware_profile_id: [u8; 32],
    suite_id: [u8; 32],
    firmware_policy_digest: [u8; 32],
    policy_epoch: u64,
    lane_commitment: [u8; 32],
    hardware_epoch_id: [u8; 32],
    hardware_epoch_generation: u64,
    device_public_key: KagemushaDevicePublicKeyV1,
    device_key_reference: [u8; 32],
    issued_at_ms: u64,
    expires_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.kagemusha.v1.hardware-credential-signing-preimage")]
struct HardwareCredentialSigningPreimageV1 {
    domain: Vec<u8>,
    credential_id: [u8; 32],
    credential: HardwareCredentialIdPreimageV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.kagemusha.v1.inbox-receipt-preimage")]
struct InboxReceiptPreimageV1 {
    recipient_lane_id: [u8; 32],
    staging_hardware_epoch_id: [u8; 32],
    inbox_sequence: u128,
    credit_id: [u8; 32],
    payment_digest: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.kagemusha.v1.acknowledgement-signing-preimage")]
struct AcknowledgementSigningPreimageV1 {
    domain: Vec<u8>,
    version: u16,
    request_digest: [u8; 32],
    payment_digest: [u8; 32],
    inbox_receipt: KagemushaInboxReceiptV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.kagemusha.v1.mint-credit-id-preimage")]
struct MintCreditIdPreimageV1 {
    lifecycle_context_digest: [u8; 32],
    recipient_credential_commitment: [u8; 32],
    authorization_context_digest: [u8; 32],
    amount: u128,
    issuance_commitment: [u8; 32],
    recipient: AccountId,
    credit_commitment: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.kagemusha.v1.mint-lifecycle-context-preimage")]
struct MintLifecycleContextPreimageV1 {
    version: u16,
    network_id: NetworkId,
    protocol_version: u16,
    suite_id: [u8; 32],
    vk_digest: [u8; 32],
    release_id: [u8; 32],
    asset: AssetDefinitionId,
    asset_incarnation: AxtAssetIncarnationV1,
    scale: u32,
    liability_pool_id: [u8; 32],
    hardware_profile_id: [u8; 32],
    policy_epoch: u64,
    operation_kind: KagemushaOperationKindV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.kagemusha.v1.redemption-id-preimage")]
struct RedemptionIdPreimageV1 {
    lifecycle_binding_digest: [u8; 32],
    terminal_nullifier: [u8; 32],
    sender_before_commitment: KagemushaPastaStateCommitmentV1,
    sender_after_commitment: KagemushaPastaStateCommitmentV1,
    amount: u128,
    beneficiary: AccountId,
    redemption_commitment: [u8; 32],
    committed_at_ms: u64,
    hardware_transition_commitment: [u8; 32],
}

fn invalid(field: &'static str) -> KagemushaValidationErrorV1 {
    KagemushaValidationErrorV1::InvalidField { field }
}

fn digest_encoded<T: Encode>(
    domain: &[u8],
    value: &T,
) -> Result<[u8; 32], KagemushaValidationErrorV1> {
    let bytes = norito::encode_canonical(value)?;
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update([0]);
    hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    hasher.update(bytes);
    Ok(hasher.finalize().into())
}

fn digest_bytes(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update([0]);
    hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    hasher.update(bytes);
    hasher.finalize().into()
}

/// Derive the canonical digest bound to an encrypted Kagemusha credit.
///
/// This hash is a wire/codec operation. It does not encrypt, decrypt, or prove
/// the opening and therefore is never sufficient for monetary admission.
#[must_use]
pub fn kagemusha_ciphertext_digest_v1(bytes: &[u8]) -> [u8; 32] {
    digest_bytes(CIPHERTEXT_DIGEST_DOMAIN, bytes)
}

fn require_valid_x25519_public_key(
    field: &'static str,
    value: [u8; KAGEMUSHA_X25519_PUBLIC_KEY_BYTES_V1],
) -> Result<(), KagemushaValidationErrorV1> {
    X25519Sha256::decode_public_key(&value)
        .map(|_| ())
        .map_err(|_| invalid(field))
}

/// Derive the exact HKDF-SHA256 salt for one encrypted-credit X25519 exchange.
///
/// The construction is `SHA256(label || recipient_public || ephemeral_public)`;
/// `label` already carries its required trailing zero separator.
///
/// # Errors
///
/// Returns an error when either public key is low-order or otherwise invalid.
pub fn kagemusha_encrypted_credit_kdf_salt_v1(
    recipient_x25519_public_key: [u8; KAGEMUSHA_X25519_PUBLIC_KEY_BYTES_V1],
    ephemeral_x25519_public_key: [u8; KAGEMUSHA_X25519_PUBLIC_KEY_BYTES_V1],
) -> Result<[u8; 32], KagemushaValidationErrorV1> {
    require_valid_x25519_public_key(
        "kagemusha.encrypted_credit.recipient_x25519_public_key",
        recipient_x25519_public_key,
    )?;
    require_valid_x25519_public_key(
        "kagemusha.encrypted_credit.ephemeral_x25519_public_key",
        ephemeral_x25519_public_key,
    )?;
    let mut hasher = Sha256::new();
    hasher.update(KAGEMUSHA_ENCRYPTED_CREDIT_KDF_SALT_LABEL_V1);
    hasher.update(recipient_x25519_public_key);
    hasher.update(ephemeral_x25519_public_key);
    Ok(hasher.finalize().into())
}

/// Build the exact HKDF-SHA256 info bytes for encrypted-credit key derivation.
///
/// The result is `label || SHA256(canonical_aad)`; `label` already carries its
/// required trailing zero separator.
///
/// # Errors
///
/// Returns an error when the AAD is invalid or cannot be canonically encoded.
pub fn kagemusha_encrypted_credit_kdf_info_v1(
    aad: &KagemushaEncryptedCreditAadV1,
) -> Result<Vec<u8>, KagemushaValidationErrorV1> {
    let aad_digest = aad.canonical_digest()?;
    let mut info =
        Vec::with_capacity(KAGEMUSHA_ENCRYPTED_CREDIT_KDF_INFO_LABEL_V1.len() + aad_digest.len());
    info.extend_from_slice(KAGEMUSHA_ENCRYPTED_CREDIT_KDF_INFO_LABEL_V1);
    info.extend_from_slice(&aad_digest);
    Ok(info)
}

fn require_nonzero(field: &'static str, value: [u8; 32]) -> Result<(), KagemushaValidationErrorV1> {
    if value == [0; 32] {
        return Err(invalid(field));
    }
    Ok(())
}

fn require_valid_header(
    version: u16,
    network_id: &NetworkId,
    scale: u32,
    amount: Option<u128>,
    field: &'static str,
) -> Result<(), KagemushaValidationErrorV1> {
    if version != KAGEMUSHA_WIRE_VERSION_V1
        || network_id.as_bytes() == &[0; 32]
        || scale > KAGEMUSHA_ASSET_SCALE_MAX_V1
        || amount.is_some_and(|value| value == 0)
    {
        return Err(invalid(field));
    }
    Ok(())
}

fn require_encoded_size<T: Encode>(
    value: &T,
    max: usize,
) -> Result<usize, KagemushaValidationErrorV1> {
    let actual = norito::encode_canonical(value)?.len();
    if actual > max {
        return Err(KagemushaValidationErrorV1::EncodedSizeExceeded { actual, max });
    }
    Ok(actual)
}

/// Decode one already byte-capped canonical frame under resource limits that
/// are installed before derive-generated sequence decoders can reserve space.
fn decode_bounded_canonical<T>(bytes: &[u8], max: usize) -> Result<T, KagemushaValidationErrorV1>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    if bytes.len() > max {
        return Err(KagemushaValidationErrorV1::EncodedSizeExceeded {
            actual: bytes.len(),
            max,
        });
    }
    let limits = norito::canonical_decode_limits(bytes.len());
    Ok(norito::decode_canonical_with_limits(bytes, limits)?)
}

fn encode_kagemusha_text_v1<T: Encode>(
    value: &T,
    raw_max: usize,
    text_max: usize,
) -> Result<String, KagemushaValidationErrorV1> {
    let raw = norito::encode_canonical(value)?;
    if raw.len() > raw_max {
        return Err(KagemushaValidationErrorV1::EncodedSizeExceeded {
            actual: raw.len(),
            max: raw_max,
        });
    }
    let mut text =
        String::with_capacity(KAGEMUSHA_TEXT_PREFIX_V1.len() + unpadded_base64url_len(raw.len()));
    text.push_str(KAGEMUSHA_TEXT_PREFIX_V1);
    URL_SAFE_NO_PAD.encode_string(raw, &mut text);
    if text.len() > text_max {
        return Err(KagemushaValidationErrorV1::EncodedSizeExceeded {
            actual: text.len(),
            max: text_max,
        });
    }
    Ok(text)
}

fn decode_kagemusha_text_payload_v1(
    text: &str,
    raw_max: usize,
    text_max: usize,
) -> Result<Vec<u8>, KagemushaValidationErrorV1> {
    if text.len() > text_max {
        return Err(KagemushaValidationErrorV1::EncodedSizeExceeded {
            actual: text.len(),
            max: text_max,
        });
    }
    if text.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Err(invalid("kagemusha.text.whitespace"));
    }
    let body = text
        .strip_prefix(KAGEMUSHA_TEXT_PREFIX_V1)
        .ok_or_else(|| invalid("kagemusha.text.prefix"))?;
    if body.is_empty() {
        return Err(invalid("kagemusha.text.body"));
    }
    if body.contains('=') {
        return Err(invalid("kagemusha.text.padding"));
    }
    if !body
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(invalid("kagemusha.text.base64url"));
    }
    let raw = URL_SAFE_NO_PAD
        .decode(body.as_bytes())
        .map_err(|_| invalid("kagemusha.text.base64url"))?;
    if raw.len() > raw_max {
        return Err(KagemushaValidationErrorV1::EncodedSizeExceeded {
            actual: raw.len(),
            max: raw_max,
        });
    }
    if URL_SAFE_NO_PAD.encode(&raw) != body {
        return Err(invalid("kagemusha.text.base64url"));
    }
    Ok(raw)
}

fn decode_kagemusha_text_v1<T, F>(
    text: &str,
    raw_max: usize,
    text_max: usize,
    decode: F,
) -> Result<T, KagemushaValidationErrorV1>
where
    T: Encode,
    F: FnOnce(&[u8]) -> Result<T, KagemushaValidationErrorV1>,
{
    let raw = decode_kagemusha_text_payload_v1(text, raw_max, text_max)?;
    let value = decode(&raw)?;
    if encode_kagemusha_text_v1(&value, raw_max, text_max)? != text {
        return Err(invalid("kagemusha.text.canonical"));
    }
    Ok(value)
}

/// Derive the stable reference to an Kagemusha device key.
#[must_use]
pub fn kagemusha_device_key_reference_v1(public_key: &KagemushaDevicePublicKeyV1) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(DEVICE_KEY_REFERENCE_DOMAIN);
    hasher.update([0]);
    hasher.update(public_key.as_sec1_bytes());
    hasher.finalize().into()
}

/// Derive the sole field-neutral digest of an exact typed asset-definition identity.
///
/// This is the normalized GuardBundle/circuit identity. The original typed value remains on the
/// transport and reserve records so adapters never attempt a lossy digest-to-type conversion.
///
/// # Errors
///
/// Returns an error when the canonical typed identity cannot be encoded.
pub fn kagemusha_asset_identity_digest_v1(
    asset: &AssetDefinitionId,
) -> Result<[u8; 32], KagemushaValidationErrorV1> {
    digest_encoded(ASSET_IDENTITY_DIGEST_DOMAIN, asset)
}

/// Derive the sole reserve-liability pool for one network, asset, and incarnation.
///
/// # Errors
///
/// Returns an error when the canonical pool preimage cannot be encoded.
pub fn kagemusha_liability_pool_id_v1(
    network_id: &NetworkId,
    asset: &AssetDefinitionId,
    asset_incarnation: AxtAssetIncarnationV1,
) -> Result<[u8; 32], KagemushaValidationErrorV1> {
    digest_encoded(
        LIABILITY_POOL_DOMAIN,
        &LiabilityPoolPreimageV1 {
            network_id: *network_id,
            asset: asset.clone(),
            asset_incarnation,
        },
    )
}

/// Derive an unlinkable payment credit identity from pre-encryption output bindings.
///
/// The transition nullifier is a private-state-derived circuit output. No
/// predecessor/successor commitment, sequence, or stable sender lane appears
/// in this preimage. `ciphertext_commitment` is sampled independently of the
/// final credit ID and commits the credit-opening semantics. Construction is
/// acyclic: sample the opening and commitment, derive this ID, then bind the ID
/// into AEAD plaintext or associated data. The final lifecycle and recursive
/// proof separately bind the exact ciphertext digest to this commitment and opening.
///
/// # Errors
///
/// Returns an error when the canonical identity preimage cannot be encoded.
#[allow(clippy::too_many_arguments)]
pub fn kagemusha_credit_id_v1(
    transition_nullifier: [u8; 32],
    request_digest: [u8; 32],
    sender_before_commitment: KagemushaPastaStateCommitmentV1,
    sender_after_commitment: KagemushaPastaStateCommitmentV1,
    recipient_lane_id: [u8; 32],
    recipient_encryption_key: [u8; 32],
    amount: u128,
    ciphertext_commitment: [u8; 32],
) -> Result<[u8; 32], KagemushaValidationErrorV1> {
    digest_encoded(
        CREDIT_ID_DOMAIN,
        &CreditIdPreimageV1 {
            transition_nullifier,
            request_digest,
            sender_before_commitment,
            sender_after_commitment,
            recipient_lane_id,
            recipient_encryption_key,
            amount,
            ciphertext_commitment,
        },
    )
}

/// Return the exact canonical plaintext length for every V1 credit opening.
///
/// # Errors
///
/// Returns an error only when canonical Norito encoding fails.
pub fn kagemusha_credit_opening_canonical_len_v1() -> Result<usize, KagemushaValidationErrorV1> {
    Ok(norito::encode_canonical(&KagemushaCreditOpeningV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        credit_id: [1; 32],
        amount: 1,
        credit_commitment_opening: [2; 32],
        recipient_binding_opening: [3; 32],
        recovery_nonce: [4; 32],
    })?
    .len())
}

impl KagemushaCreditOpeningV1 {
    /// Validate the fixed private plaintext independently of public context.
    ///
    /// # Errors
    ///
    /// Returns an error for a reserved version, zero amount, zero opening, or
    /// an unexpected canonical fixed-size encoding.
    pub fn validate_shape(&self) -> Result<(), KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1 || self.amount == 0 {
            return Err(invalid("kagemusha.credit_opening.header"));
        }
        for (field, value) in [
            ("kagemusha.credit_opening.credit_id", self.credit_id),
            (
                "kagemusha.credit_opening.credit_commitment_opening",
                self.credit_commitment_opening,
            ),
            (
                "kagemusha.credit_opening.recipient_binding_opening",
                self.recipient_binding_opening,
            ),
            (
                "kagemusha.credit_opening.recovery_nonce",
                self.recovery_nonce,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        let actual = require_encoded_size(self, KAGEMUSHA_CREDIT_OPENING_MAX_BYTES_V1)?;
        if actual != kagemusha_credit_opening_canonical_len_v1()? {
            return Err(invalid("kagemusha.credit_opening.encoded_length"));
        }
        Ok(())
    }

    /// Validate the private opening against its exact public credit identity and amount.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid shape or a public/private mismatch.
    pub fn validate_shape_against(
        &self,
        credit_id: [u8; 32],
        amount: u128,
    ) -> Result<(), KagemushaValidationErrorV1> {
        self.validate_shape()?;
        if self.credit_id != credit_id || self.amount != amount {
            return Err(invalid("kagemusha.credit_opening.public_binding"));
        }
        Ok(())
    }

    /// Encode this validated fixed plaintext canonically for AEAD sealing.
    ///
    /// # Errors
    ///
    /// Returns an error when validation or canonical encoding fails.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, KagemushaValidationErrorV1> {
        self.validate_shape()?;
        Ok(norito::encode_canonical(self)?)
    }

    /// Decode one exact canonical plaintext and bind it to public credit fields.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized, malformed, non-canonical, or mismatched bytes.
    pub fn decode_canonical_shape_exact_against(
        bytes: &[u8],
        credit_id: [u8; 32],
        amount: u128,
    ) -> Result<Self, KagemushaValidationErrorV1> {
        if bytes.len() != kagemusha_credit_opening_canonical_len_v1()? {
            return Err(invalid("kagemusha.credit_opening.encoded_length"));
        }
        let opening: Self = decode_bounded_canonical(bytes, KAGEMUSHA_CREDIT_OPENING_MAX_BYTES_V1)?;
        opening.validate_shape_against(credit_id, amount)?;
        Ok(opening)
    }
}

fn peer_credit_lifecycle_context_digest_v1(
    lifecycle: &KagemushaLifecycleBindingV1,
) -> Result<[u8; 32], KagemushaValidationErrorV1> {
    require_valid_header(
        lifecycle.version,
        &lifecycle.network_id,
        lifecycle.scale,
        None,
        "kagemusha.peer_credit_context.lifecycle_header",
    )?;
    lifecycle
        .asset_incarnation
        .validate()
        .map_err(|_| invalid("kagemusha.peer_credit_context.asset_incarnation"))?;
    for (field, value) in [
        ("kagemusha.peer_credit_context.suite_id", lifecycle.suite_id),
        (
            "kagemusha.peer_credit_context.vk_digest",
            lifecycle.vk_digest,
        ),
        (
            "kagemusha.peer_credit_context.release_id",
            lifecycle.release_id,
        ),
        (
            "kagemusha.peer_credit_context.liability_pool_id",
            lifecycle.liability_pool_id,
        ),
        (
            "kagemusha.peer_credit_context.hardware_profile_id",
            lifecycle.hardware_profile_id,
        ),
        (
            "kagemusha.peer_credit_context.request_id",
            lifecycle.request_id,
        ),
    ] {
        require_nonzero(field, value)?;
    }
    if lifecycle.protocol_version != KAGEMUSHA_WIRE_VERSION_V1
        || lifecycle.policy_epoch == 0
        || lifecycle.operation_kind != KagemushaOperationKindV1::SendSplit
        || lifecycle.liability_pool_id
            != kagemusha_liability_pool_id_v1(
                &lifecycle.network_id,
                &lifecycle.asset,
                lifecycle.asset_incarnation,
            )?
    {
        return Err(invalid("kagemusha.peer_credit_context.lifecycle"));
    }
    digest_encoded(
        PEER_CREDIT_LIFECYCLE_CONTEXT_DIGEST_DOMAIN,
        &PeerCreditLifecycleContextPreimageV1 {
            version: lifecycle.version,
            network_id: lifecycle.network_id,
            protocol_version: lifecycle.protocol_version,
            suite_id: lifecycle.suite_id,
            vk_digest: lifecycle.vk_digest,
            release_id: lifecycle.release_id,
            asset: lifecycle.asset.clone(),
            asset_incarnation: lifecycle.asset_incarnation,
            scale: lifecycle.scale,
            liability_pool_id: lifecycle.liability_pool_id,
            hardware_profile_id: lifecycle.hardware_profile_id,
            policy_epoch: lifecycle.policy_epoch,
            operation_kind: lifecycle.operation_kind,
            request_id: lifecycle.request_id,
        },
    )
}

impl KagemushaPeerCreditContextV1 {
    /// Validate the exact pre-encryption request, state, receiver, and lifecycle projection.
    ///
    /// # Errors
    ///
    /// Returns an error for a reserved digest, invalid recipient key, or version.
    pub fn validate_shape(&self) -> Result<(), KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1 {
            return Err(invalid("kagemusha.peer_credit_context.version"));
        }
        for (field, value) in [
            (
                "kagemusha.peer_credit_context.request_digest",
                self.request_digest,
            ),
            (
                "kagemusha.peer_credit_context.lifecycle_context_digest",
                self.lifecycle_context_digest,
            ),
            (
                "kagemusha.peer_credit_context.recipient_lane_id",
                self.recipient_lane_id,
            ),
            (
                "kagemusha.peer_credit_context.hardware_transition_commitment",
                self.hardware_transition_commitment,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        if self.sender_before_commitment.is_zero()
            || self.sender_after_commitment.is_zero()
            || self.sender_before_commitment == self.sender_after_commitment
            || self.committed_at_ms == 0
        {
            return Err(invalid("kagemusha.peer_credit_context.transition"));
        }
        require_valid_x25519_public_key(
            "kagemusha.peer_credit_context.recipient_encryption_key",
            self.recipient_encryption_key,
        )
    }

    /// Return the exact pre-ID peer context digest carried in AEAD associated data.
    ///
    /// # Errors
    ///
    /// Returns an error when validation or canonical encoding fails.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate_shape()?;
        digest_encoded(PEER_CREDIT_CONTEXT_DIGEST_DOMAIN, self)
    }
}

impl KagemushaEncryptedCreditAadV1 {
    /// Validate exact acyclic encrypted-credit associated data.
    ///
    /// # Errors
    ///
    /// Returns an error for a reserved version, digest, commitment, credit ID,
    /// or zero amount.
    pub fn validate_shape(&self) -> Result<(), KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1 || self.amount == 0 {
            return Err(invalid("kagemusha.encrypted_credit_aad.header"));
        }
        for (field, value) in [
            (
                "kagemusha.encrypted_credit_aad.context_digest",
                self.context_digest,
            ),
            (
                "kagemusha.encrypted_credit_aad.issuance_or_transition_commitment",
                self.issuance_or_transition_commitment,
            ),
            ("kagemusha.encrypted_credit_aad.credit_id", self.credit_id),
        ] {
            require_nonzero(field, value)?;
        }
        Ok(())
    }

    /// Return the exact canonical bytes authenticated by XChaCha20-Poly1305.
    ///
    /// # Errors
    ///
    /// Returns an error when validation or canonical encoding fails.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, KagemushaValidationErrorV1> {
        self.validate_shape()?;
        Ok(norito::encode_canonical(self)?)
    }

    /// Return `SHA256(canonical_aad)`, the suffix of the HKDF info string.
    ///
    /// # Errors
    ///
    /// Returns an error when validation or canonical encoding fails.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        let bytes = self.canonical_bytes()?;
        Ok(Sha256::digest(bytes).into())
    }

    /// Construct the exact AAD for a post-encryption mint authorization.
    ///
    /// # Errors
    ///
    /// Returns an error when the authorization statement is invalid.
    pub fn for_mint(
        statement: &KagemushaMintAuthorizationStatementV1,
    ) -> Result<Self, KagemushaValidationErrorV1> {
        statement.validate_shape()?;
        Ok(Self {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            purpose: KagemushaEncryptedCreditPurposeV1::Mint,
            context_digest: statement.context.canonical_digest()?,
            issuance_or_transition_commitment: statement.issuance_commitment,
            credit_id: statement.credit_id,
            amount: statement.context.amount,
        })
    }

    /// Construct the exact AAD for a receiver-bound peer credit.
    ///
    /// The peer transition field is the ID-independent credit-opening
    /// commitment. The surrounding context binds the complete exact request,
    /// sender transition, receiver lane/key, trusted commit time, and all
    /// lifecycle fields available before credit ID and ciphertext derivation.
    ///
    /// # Errors
    ///
    /// Returns an error for any invalid or substituted peer context.
    pub fn for_peer(
        statement: &KagemushaTransferStatementV1,
        request: &KagemushaPaymentRequestV1,
    ) -> Result<Self, KagemushaValidationErrorV1> {
        let context = statement.peer_credit_context_against(request)?;
        Ok(Self {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            purpose: KagemushaEncryptedCreditPurposeV1::Peer,
            context_digest: context.canonical_digest()?,
            issuance_or_transition_commitment: statement.ciphertext_commitment,
            credit_id: statement.lifecycle.credit_id,
            amount: statement.amount,
        })
    }
}

impl KagemushaEncryptedCreditEnvelopeV1 {
    /// Validate the exact canonical envelope shape without performing AEAD open.
    ///
    /// # Errors
    ///
    /// Returns an error for a reserved version, low-order ephemeral key, wrong
    /// fixed ciphertext size, or oversized encoding. Nonce freshness and
    /// randomness are enforced by qualified hardware and cannot be inferred
    /// from one standalone envelope.
    pub fn validate_shape(&self) -> Result<(), KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1 {
            return Err(invalid("kagemusha.encrypted_credit.version"));
        }
        require_valid_x25519_public_key(
            "kagemusha.encrypted_credit.ephemeral_x25519_public_key",
            self.ephemeral_x25519_public_key,
        )?;
        let expected = kagemusha_credit_opening_canonical_len_v1()?
            .checked_add(KAGEMUSHA_XCHACHA20POLY1305_TAG_BYTES_V1)
            .ok_or_else(|| invalid("kagemusha.encrypted_credit.ciphertext_and_tag"))?;
        if self.ciphertext_and_tag.len() != expected {
            return Err(invalid("kagemusha.encrypted_credit.ciphertext_and_tag"));
        }
        require_encoded_size(self, KAGEMUSHA_ENCRYPTED_CREDIT_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Validate envelope shape plus the externally signed recipient X25519 key.
    ///
    /// # Errors
    ///
    /// Returns an error when either envelope shape or recipient key is invalid.
    pub fn validate_shape_against_recipient_key(
        &self,
        recipient_x25519_public_key: [u8; KAGEMUSHA_X25519_PUBLIC_KEY_BYTES_V1],
    ) -> Result<(), KagemushaValidationErrorV1> {
        self.validate_shape()?;
        require_valid_x25519_public_key(
            "kagemusha.encrypted_credit.recipient_x25519_public_key",
            recipient_x25519_public_key,
        )?;
        Ok(())
    }

    /// Encode this exact envelope into an `encrypted_credit` byte field.
    ///
    /// # Errors
    ///
    /// Returns an error when validation or canonical encoding fails.
    pub fn canonical_bytes_against_recipient_key(
        &self,
        recipient_x25519_public_key: [u8; KAGEMUSHA_X25519_PUBLIC_KEY_BYTES_V1],
    ) -> Result<Vec<u8>, KagemushaValidationErrorV1> {
        self.validate_shape_against_recipient_key(recipient_x25519_public_key)?;
        Ok(norito::encode_canonical(self)?)
    }

    /// Decode one exact canonical envelope without opening it.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized, malformed, non-canonical, or invalid bytes.
    pub fn decode_canonical_shape_exact(bytes: &[u8]) -> Result<Self, KagemushaValidationErrorV1> {
        let envelope: Self =
            decode_bounded_canonical(bytes, KAGEMUSHA_ENCRYPTED_CREDIT_MAX_BYTES_V1)?;
        envelope.validate_shape()?;
        Ok(envelope)
    }

    /// Decode one exact envelope and validate its signed recipient key.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid bytes, envelope shape, or recipient key.
    pub fn decode_canonical_shape_exact_against_recipient_key(
        bytes: &[u8],
        recipient_x25519_public_key: [u8; KAGEMUSHA_X25519_PUBLIC_KEY_BYTES_V1],
    ) -> Result<Self, KagemushaValidationErrorV1> {
        let envelope = Self::decode_canonical_shape_exact(bytes)?;
        envelope.validate_shape_against_recipient_key(recipient_x25519_public_key)?;
        Ok(envelope)
    }

    /// Derive the exact HKDF salt after validating this ephemeral key.
    ///
    /// # Errors
    ///
    /// Returns an error when either X25519 public key is invalid.
    pub fn kdf_salt_against_recipient_key(
        &self,
        recipient_x25519_public_key: [u8; KAGEMUSHA_X25519_PUBLIC_KEY_BYTES_V1],
    ) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate_shape_against_recipient_key(recipient_x25519_public_key)?;
        kagemusha_encrypted_credit_kdf_salt_v1(
            recipient_x25519_public_key,
            self.ephemeral_x25519_public_key,
        )
    }
}

/// Derive the exact pre-ID commitment to one mint credit opening.
///
/// This commitment deliberately excludes the authorization-context digest,
/// issuance commitment, credit ID, ciphertext, and proof, so construction is
/// acyclic and the recipient authorization can be proved before reserve debit.
///
/// # Errors
///
/// Returns an error for invalid context, key, opening, pooled reserve, or
/// canonical encoding.
#[allow(clippy::too_many_arguments)]
pub fn kagemusha_mint_credit_opening_commitment_v1(
    network_id: &NetworkId,
    asset: &AssetDefinitionId,
    asset_incarnation: AxtAssetIncarnationV1,
    scale: u32,
    liability_pool_id: [u8; 32],
    amount: u128,
    recipient: &AccountId,
    recipient_one_time_key: [u8; KAGEMUSHA_X25519_PUBLIC_KEY_BYTES_V1],
    credit_commitment_opening: [u8; 32],
) -> Result<[u8; 32], KagemushaValidationErrorV1> {
    require_valid_header(
        KAGEMUSHA_WIRE_VERSION_V1,
        network_id,
        scale,
        Some(amount),
        "kagemusha.mint_credit_opening_commitment.header",
    )?;
    asset_incarnation
        .validate()
        .map_err(|_| invalid("kagemusha.mint_credit_opening_commitment.asset_incarnation"))?;
    require_nonzero(
        "kagemusha.mint_credit_opening_commitment.liability_pool_id",
        liability_pool_id,
    )?;
    require_nonzero(
        "kagemusha.mint_credit_opening_commitment.credit_commitment_opening",
        credit_commitment_opening,
    )?;
    require_valid_x25519_public_key(
        "kagemusha.mint_credit_opening_commitment.recipient_one_time_key",
        recipient_one_time_key,
    )?;
    if liability_pool_id != kagemusha_liability_pool_id_v1(network_id, asset, asset_incarnation)? {
        return Err(invalid(
            "kagemusha.mint_credit_opening_commitment.liability_pool_id",
        ));
    }
    digest_encoded(
        MINT_CREDIT_OPENING_COMMITMENT_DOMAIN,
        &MintCreditOpeningCommitmentPreimageV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            network_id: *network_id.as_bytes(),
            asset_identity_digest: kagemusha_asset_identity_digest_v1(asset)?,
            asset_incarnation,
            scale,
            liability_pool_id,
            amount,
            recipient_account_digest: digest_encoded(ACCOUNT_IDENTITY_DIGEST_DOMAIN, recipient)?,
            recipient_one_time_key,
            credit_commitment_opening,
        },
    )
}

/// Derive the exact randomized recipient-credential commitment for mint authorization.
///
/// # Errors
///
/// Returns an error for a reserved operation, credential, or opening, or when
/// canonical encoding fails.
pub fn kagemusha_recipient_credential_commitment_v1(
    operation_id: [u8; 32],
    hardware_credential_id: [u8; 32],
    recipient_binding_opening: [u8; 32],
) -> Result<[u8; 32], KagemushaValidationErrorV1> {
    for (field, value) in [
        (
            "kagemusha.recipient_credential_commitment.operation_id",
            operation_id,
        ),
        (
            "kagemusha.recipient_credential_commitment.hardware_credential_id",
            hardware_credential_id,
        ),
        (
            "kagemusha.recipient_credential_commitment.recipient_binding_opening",
            recipient_binding_opening,
        ),
    ] {
        require_nonzero(field, value)?;
    }
    digest_encoded(
        RECIPIENT_CREDENTIAL_COMMITMENT_DOMAIN,
        &RecipientCredentialCommitmentPreimageV1 {
            operation_id,
            hardware_credential_id,
            recipient_binding_opening,
        },
    )
}

/// Derive the deterministic durable-inbox receipt commitment.
///
/// The receiving hardware must only authorize the acknowledgement after this
/// exact receipt and payment have committed to durable storage.
///
/// # Errors
///
/// Returns an error when an identity/sequence is reserved or the canonical
/// receipt preimage cannot be encoded.
pub fn kagemusha_inbox_receipt_commitment_v1(
    recipient_lane_id: [u8; 32],
    staging_hardware_epoch_id: [u8; 32],
    inbox_sequence: u128,
    credit_id: [u8; 32],
    payment_digest: [u8; 32],
) -> Result<[u8; 32], KagemushaValidationErrorV1> {
    for (field, value) in [
        (
            "kagemusha.inbox_receipt.recipient_lane_id",
            recipient_lane_id,
        ),
        (
            "kagemusha.inbox_receipt.staging_hardware_epoch_id",
            staging_hardware_epoch_id,
        ),
        ("kagemusha.inbox_receipt.credit_id", credit_id),
        ("kagemusha.inbox_receipt.payment_digest", payment_digest),
    ] {
        require_nonzero(field, value)?;
    }
    if inbox_sequence == 0 {
        return Err(invalid("kagemusha.inbox_receipt.inbox_sequence"));
    }
    digest_encoded(
        INBOX_RECEIPT_DOMAIN,
        &InboxReceiptPreimageV1 {
            recipient_lane_id,
            staging_hardware_epoch_id,
            inbox_sequence,
            credit_id,
            payment_digest,
        },
    )
}

impl KagemushaHardwareProfileV1 {
    fn id_preimage(&self) -> HardwareProfileIdPreimageV1 {
        HardwareProfileIdPreimageV1 {
            version: self.version,
            protocol_version: self.protocol_version,
            provider_id: self.provider_id,
            platform_class: self.platform_class,
            product_class_digest: self.product_class_digest,
            firmware_policy_digest: self.firmware_policy_digest,
            enrollment_attestation_verifier_digest: self.enrollment_attestation_verifier_digest,
            attestation_trust_roots_digest: self.attestation_trust_roots_digest,
            allowed_suite_commitment: self.allowed_suite_commitment,
            policy_epoch: self.policy_epoch,
            governance_credential_public_key: self.governance_credential_public_key,
            capability_mask: self.capability_mask,
            qualification_report_digest: self.qualification_report_digest,
            valid_from_ms: self.valid_from_ms,
            expires_at_ms: self.expires_at_ms,
        }
    }

    /// Compute the domain-separated profile identity from its unsigned body.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn expected_hardware_profile_id(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        digest_encoded(HARDWARE_PROFILE_DIGEST_DOMAIN, &self.id_preimage())
    }

    /// Populate the canonical profile identity.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical profile-body encoding fails.
    pub fn seal_hardware_profile_id(mut self) -> Result<Self, KagemushaValidationErrorV1> {
        self.hardware_profile_id = self.expected_hardware_profile_id()?;
        Ok(self)
    }

    /// Validate the exact governed capability set and profile lifetime.
    ///
    /// # Errors
    ///
    /// Returns an error for a reserved identity, incomplete/unknown capability
    /// set, invalid issuer key, invalid lifetime, or oversized encoding.
    pub fn validate(&self) -> Result<(), KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || self.protocol_version != KAGEMUSHA_WIRE_VERSION_V1
            || self.policy_epoch == 0
            || self.capability_mask != KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1
            || self.valid_from_ms >= self.expires_at_ms
        {
            return Err(invalid("kagemusha.hardware_profile.header"));
        }
        for (field, value) in [
            (
                "kagemusha.hardware_profile.hardware_profile_id",
                self.hardware_profile_id,
            ),
            ("kagemusha.hardware_profile.provider_id", self.provider_id),
            (
                "kagemusha.hardware_profile.firmware_policy_digest",
                self.firmware_policy_digest,
            ),
            (
                "kagemusha.hardware_profile.product_class_digest",
                self.product_class_digest,
            ),
            (
                "kagemusha.hardware_profile.enrollment_attestation_verifier_digest",
                self.enrollment_attestation_verifier_digest,
            ),
            (
                "kagemusha.hardware_profile.attestation_trust_roots_digest",
                self.attestation_trust_roots_digest,
            ),
            (
                "kagemusha.hardware_profile.allowed_suite_commitment",
                self.allowed_suite_commitment,
            ),
            (
                "kagemusha.hardware_profile.qualification_report_digest",
                self.qualification_report_digest,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        self.governance_credential_public_key.validate()?;
        if self.hardware_profile_id != self.expected_hardware_profile_id()? {
            return Err(invalid("kagemusha.hardware_profile.hardware_profile_id"));
        }
        require_encoded_size(self, KAGEMUSHA_HARDWARE_PROFILE_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Return the canonical governed profile digest.
    ///
    /// # Errors
    ///
    /// Returns an error when the profile is invalid or cannot be encoded.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate()?;
        Ok(self.hardware_profile_id)
    }
}

impl KagemushaHardwareCredentialV1 {
    fn id_preimage(&self) -> HardwareCredentialIdPreimageV1 {
        HardwareCredentialIdPreimageV1 {
            version: self.version,
            network_id: self.network_id,
            hardware_profile_id: self.hardware_profile_id,
            suite_id: self.suite_id,
            firmware_policy_digest: self.firmware_policy_digest,
            policy_epoch: self.policy_epoch,
            lane_commitment: self.lane_commitment,
            hardware_epoch_id: self.hardware_epoch_id,
            hardware_epoch_generation: self.hardware_epoch_generation,
            device_public_key: self.device_public_key,
            device_key_reference: self.device_key_reference,
            issued_at_ms: self.issued_at_ms,
            expires_at_ms: self.expires_at_ms,
        }
    }

    /// Compute the canonical compact credential identity.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn expected_credential_id(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        digest_encoded(HARDWARE_CREDENTIAL_ID_DOMAIN, &self.id_preimage())
    }

    /// Populate the canonical credential identity before governance signs it.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical identity encoding fails.
    pub fn seal_credential_id(mut self) -> Result<Self, KagemushaValidationErrorV1> {
        self.credential_id = self.expected_credential_id()?;
        Ok(self)
    }

    /// Return the exact bytes signed by the governed profile issuer.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, KagemushaValidationErrorV1> {
        Ok(norito::encode_canonical(
            &HardwareCredentialSigningPreimageV1 {
                domain: HARDWARE_CREDENTIAL_SIGNING_DOMAIN.to_vec(),
                credential_id: self.credential_id,
                credential: self.id_preimage(),
            },
        )?)
    }

    /// Validate canonical credential fields, identity, embedded key, and size.
    ///
    /// This verifies only shape and self-consistency. Monetary callers must
    /// authenticate the credential with [`Self::validate_against_profile`]
    /// using a profile resolved from an authenticated release.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, inconsistent, or oversized credentials.
    pub fn validate_shape(&self) -> Result<(), KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || self.network_id.as_bytes() == &[0; 32]
            || self.policy_epoch == 0
            || self.issued_at_ms >= self.expires_at_ms
        {
            return Err(invalid("kagemusha.hardware_credential.header"));
        }
        for (field, value) in [
            (
                "kagemusha.hardware_credential.credential_id",
                self.credential_id,
            ),
            (
                "kagemusha.hardware_credential.hardware_profile_id",
                self.hardware_profile_id,
            ),
            ("kagemusha.hardware_credential.suite_id", self.suite_id),
            (
                "kagemusha.hardware_credential.firmware_policy_digest",
                self.firmware_policy_digest,
            ),
            (
                "kagemusha.hardware_credential.lane_commitment",
                self.lane_commitment,
            ),
            (
                "kagemusha.hardware_credential.hardware_epoch_id",
                self.hardware_epoch_id,
            ),
            (
                "kagemusha.hardware_credential.device_key_reference",
                self.device_key_reference,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        self.device_public_key.validate()?;
        if self.device_key_reference != kagemusha_device_key_reference_v1(&self.device_public_key)
            || self.credential_id != self.expected_credential_id()?
        {
            return Err(invalid("kagemusha.hardware_credential.identity"));
        }
        require_encoded_size(self, KAGEMUSHA_HARDWARE_CREDENTIAL_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Validate this credential against the exact governed profile.
    ///
    /// # Errors
    ///
    /// Returns an error for any profile, firmware, lifetime, identity, or
    /// governance-signature mismatch.
    pub fn validate_against_profile(
        &self,
        profile: &KagemushaHardwareProfileV1,
    ) -> Result<(), KagemushaValidationErrorV1> {
        profile.validate()?;
        self.validate_shape()?;
        if self.hardware_profile_id != profile.hardware_profile_id
            || self.firmware_policy_digest != profile.firmware_policy_digest
            || self.policy_epoch != profile.policy_epoch
            || digest_bytes(SUITE_COMMITMENT_DOMAIN, &self.suite_id)
                != profile.allowed_suite_commitment
            || self.issued_at_ms < profile.valid_from_ms
            || self.expires_at_ms > profile.expires_at_ms
        {
            return Err(invalid("kagemusha.hardware_credential.profile_binding"));
        }
        self.governance_signature.verify(
            &profile.governance_credential_public_key,
            &self.canonical_signing_bytes()?,
        )
    }
}

impl KagemushaLifecycleBindingV1 {
    /// Validate the complete released-transition lifecycle context.
    ///
    /// # Errors
    ///
    /// Returns an error for a reserved identity, unsupported protocol, invalid
    /// pooled reserve, or malformed operation-specific binding.
    pub fn validate(&self) -> Result<(), KagemushaValidationErrorV1> {
        require_valid_header(
            self.version,
            &self.network_id,
            self.scale,
            None,
            "kagemusha.lifecycle.header",
        )?;
        if self.protocol_version != KAGEMUSHA_WIRE_VERSION_V1 || self.policy_epoch == 0 {
            return Err(invalid("kagemusha.lifecycle.context"));
        }
        self.asset_incarnation
            .validate()
            .map_err(|_| invalid("kagemusha.lifecycle.asset_incarnation"))?;
        for (field, value) in [
            ("kagemusha.lifecycle.suite_id", self.suite_id),
            ("kagemusha.lifecycle.vk_digest", self.vk_digest),
            ("kagemusha.lifecycle.release_id", self.release_id),
            (
                "kagemusha.lifecycle.liability_pool_id",
                self.liability_pool_id,
            ),
            (
                "kagemusha.lifecycle.hardware_profile_id",
                self.hardware_profile_id,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        if self.liability_pool_id
            != kagemusha_liability_pool_id_v1(
                &self.network_id,
                &self.asset,
                self.asset_incarnation,
            )?
        {
            return Err(invalid("kagemusha.lifecycle.liability_pool_id"));
        }
        let credit_fields = [self.credit_id, self.ciphertext_digest];
        match self.operation_kind {
            KagemushaOperationKindV1::SendSplit
                if self.request_id != [0; 32]
                    && credit_fields.iter().all(|value| *value != [0; 32]) => {}
            KagemushaOperationKindV1::SendSplit => {
                return Err(invalid("kagemusha.lifecycle.payment_binding"));
            }
            KagemushaOperationKindV1::MintFold
                if self.request_id == [0; 32]
                    && credit_fields.iter().all(|value| *value != [0; 32]) => {}
            KagemushaOperationKindV1::MintFold => {
                return Err(invalid("kagemusha.lifecycle.mint_binding"));
            }
            _ if core::iter::once(&self.request_id)
                .chain(credit_fields.iter())
                .all(|value| *value == [0; 32]) => {}
            _ => return Err(invalid("kagemusha.lifecycle.non_payment_binding")),
        }
        Ok(())
    }

    /// Return the canonical lifecycle digest bound by proof and certificate.
    ///
    /// # Errors
    ///
    /// Returns an error when the lifecycle is invalid or cannot be encoded.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate()?;
        digest_encoded(LIFECYCLE_BINDING_DIGEST_DOMAIN, self)
    }
}

impl KagemushaAggregateStateCommitmentV1 {
    /// Decode and validate exact bounded aggregate-state metadata.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized, malformed, non-canonical, or invalid state metadata.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, KagemushaValidationErrorV1> {
        let state: Self = decode_bounded_canonical(bytes, KAGEMUSHA_AGGREGATE_STATE_MAX_BYTES_V1)?;
        state.validate()?;
        Ok(state)
    }

    /// Validate the fixed aggregate-state context and commitments.
    ///
    /// # Errors
    ///
    /// Returns an error when any context, identity, or commitment binding is invalid.
    pub fn validate(&self) -> Result<(), KagemushaValidationErrorV1> {
        require_valid_header(
            self.version,
            &self.network_id,
            self.scale,
            None,
            "kagemusha.aggregate_state.header",
        )?;
        self.asset_incarnation
            .validate()
            .map_err(|_| invalid("kagemusha.aggregate_state.asset_incarnation"))?;
        for (field, value) in [
            ("kagemusha.aggregate_state.release_id", self.release_id),
            (
                "kagemusha.aggregate_state.liability_pool_id",
                self.liability_pool_id,
            ),
            ("kagemusha.aggregate_state.lane_id", self.lane_id),
            (
                "kagemusha.aggregate_state.hardware_epoch_id",
                self.hardware_epoch_id,
            ),
            (
                "kagemusha.aggregate_state.key_reference",
                self.key_reference,
            ),
            (
                "kagemusha.aggregate_state.hardware_policy_id",
                self.hardware_policy_id,
            ),
            (
                "kagemusha.aggregate_state.state_commitment",
                self.state_commitment,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        if self.liability_pool_id
            != kagemusha_liability_pool_id_v1(
                &self.network_id,
                &self.asset,
                self.asset_incarnation,
            )?
        {
            return Err(invalid("kagemusha.aggregate_state.liability_pool_id"));
        }
        require_encoded_size(self, KAGEMUSHA_AGGREGATE_STATE_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Return the fixed aggregate-state identity committed by recursive proofs.
    ///
    /// # Errors
    ///
    /// Returns an error when the state is invalid or cannot be encoded.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate()?;
        digest_encoded(AGGREGATE_STATE_DIGEST_DOMAIN, self)
    }
}

/// Encode the exact canonical recipient-request bytes authorized by hardware.
///
/// This constructor is the single cross-crate signing contract. The request
/// binds one exact positive amount, receiver lane/key, and compact hardware credential.
///
/// # Errors
///
/// Returns an error when canonical Norito encoding fails.
#[allow(clippy::too_many_arguments)]
pub fn kagemusha_payment_request_signing_bytes_v1(
    version: u16,
    release_id: [u8; 32],
    network_id: &NetworkId,
    asset: &AssetDefinitionId,
    asset_incarnation: AxtAssetIncarnationV1,
    scale: u32,
    liability_pool_id: [u8; 32],
    recipient: &AccountId,
    recipient_lane_id: [u8; 32],
    recipient_encryption_key: [u8; 32],
    amount: u128,
    hardware_credential_id: [u8; 32],
    request_id: [u8; 32],
    issued_at_ms: u64,
    expires_at_ms: u64,
) -> Result<Vec<u8>, KagemushaValidationErrorV1> {
    Ok(norito::encode_canonical(
        &PaymentRequestSigningPreimageV1 {
            domain: REQUEST_SIGNING_DOMAIN.to_vec(),
            version,
            release_id,
            network_id: *network_id,
            asset: asset.clone(),
            asset_incarnation,
            scale,
            liability_pool_id,
            recipient: recipient.clone(),
            recipient_lane_id,
            recipient_encryption_key,
            amount,
            hardware_credential_id,
            request_id,
            issued_at_ms,
            expires_at_ms,
        },
    )?)
}

impl KagemushaPaymentRequestV1 {
    /// Return the exact bytes signed by the recipient device.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical Norito encoding fails.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, KagemushaValidationErrorV1> {
        kagemusha_payment_request_signing_bytes_v1(
            self.version,
            self.release_id,
            &self.network_id,
            &self.asset,
            self.asset_incarnation,
            self.scale,
            self.liability_pool_id,
            &self.recipient,
            self.recipient_lane_id,
            self.recipient_encryption_key,
            self.amount,
            self.hardware_credential.credential_id,
            self.request_id,
            self.issued_at_ms,
            self.expires_at_ms,
        )
    }

    /// Encode this validated request as canonical unpadded `kgm1:` base64url text.
    ///
    /// # Errors
    ///
    /// Returns an error when the request is invalid, cannot be encoded, or exceeds its cap.
    pub fn encode_text(&self) -> Result<String, KagemushaValidationErrorV1> {
        self.validate_shape()?;
        encode_kagemusha_text_v1(
            self,
            KAGEMUSHA_PAYMENT_REQUEST_MAX_BYTES_V1,
            KAGEMUSHA_PAYMENT_REQUEST_TEXT_MAX_BYTES_V1,
        )
    }

    /// Decode one exact canonical unpadded `kgm1:` request.
    ///
    /// Text syntax and size are checked before base64 decoding, and the raw cap
    /// is checked before Norito decoding. The decoded request must re-encode to
    /// the exact original text.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid, oversized, padded, non-canonical, or legacy text.
    pub fn decode_text_exact(text: &str) -> Result<Self, KagemushaValidationErrorV1> {
        decode_kagemusha_text_v1(
            text,
            KAGEMUSHA_PAYMENT_REQUEST_MAX_BYTES_V1,
            KAGEMUSHA_PAYMENT_REQUEST_TEXT_MAX_BYTES_V1,
            Self::decode_canonical_exact,
        )
    }

    /// Decode and validate one exact bounded recipient request.
    ///
    /// The byte cap is enforced before Norito reads a header or declared
    /// sequence length, and decoding rejects non-canonical byte forms.
    ///
    /// # Errors
    ///
    /// Returns an error for an oversized, malformed, non-canonical, or invalid request.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, KagemushaValidationErrorV1> {
        let request: Self =
            decode_bounded_canonical(bytes, KAGEMUSHA_PAYMENT_REQUEST_MAX_BYTES_V1)?;
        request.validate_shape()?;
        Ok(request)
    }

    /// Validate the request's shape and its signature under its embedded key.
    ///
    /// The expiry is an exclusive deadline for the sender's trusted commit. It
    /// is not a validity horizon for an already committed payment.
    /// This deliberately does **not** authenticate the embedded governance
    /// credential. Monetary callers must additionally call
    /// [`Self::validate_against_profile`] with a profile resolved from an
    /// authenticated release catalog.
    ///
    /// # Errors
    ///
    /// Returns an error when any request invariant fails.
    pub fn validate_shape(&self) -> Result<(), KagemushaValidationErrorV1> {
        require_valid_header(
            self.version,
            &self.network_id,
            self.scale,
            None,
            "kagemusha.request.header",
        )?;
        for (field, value) in [
            ("kagemusha.request.release_id", self.release_id),
            (
                "kagemusha.request.liability_pool_id",
                self.liability_pool_id,
            ),
            ("kagemusha.request.request_id", self.request_id),
            (
                "kagemusha.request.recipient_lane_id",
                self.recipient_lane_id,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        require_valid_x25519_public_key(
            "kagemusha.request.recipient_encryption_key",
            self.recipient_encryption_key,
        )?;
        if self.liability_pool_id
            != kagemusha_liability_pool_id_v1(
                &self.network_id,
                &self.asset,
                self.asset_incarnation,
            )?
        {
            return Err(invalid("kagemusha.request.liability_pool_id"));
        }
        self.asset_incarnation
            .validate()
            .map_err(|_| invalid("kagemusha.request.asset_incarnation"))?;
        if self.amount == 0 {
            return Err(invalid("kagemusha.request.amount"));
        }
        self.hardware_credential.validate_shape()?;
        if self.hardware_credential.network_id != self.network_id
            || self.hardware_credential.lane_commitment != self.recipient_lane_id
            || self.issued_at_ms < self.hardware_credential.issued_at_ms
            || self.expires_at_ms > self.hardware_credential.expires_at_ms
        {
            return Err(invalid("kagemusha.request.hardware_credential"));
        }
        let ttl = self
            .expires_at_ms
            .checked_sub(self.issued_at_ms)
            .ok_or_else(|| invalid("kagemusha.request.expires_at_ms"))?;
        if ttl == 0 || ttl > KAGEMUSHA_REQUEST_MAX_TTL_MS_V1 {
            return Err(invalid("kagemusha.request.expires_at_ms"));
        }
        self.signature.verify(
            &self.hardware_credential.device_public_key,
            &self.canonical_signing_bytes()?,
        )?;
        require_encoded_size(self, KAGEMUSHA_PAYMENT_REQUEST_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Authenticate the compact credential against one release-resolved
    /// hardware profile after validating the complete request shape.
    ///
    /// This authenticates the request and receiver hardware identity only. It
    /// does not verify a later payment's recursive proof; Core must still use
    /// the release-pinned native proof verifier before monetary admission.
    ///
    /// # Errors
    ///
    /// Returns an error for any structural, signature, profile, suite, policy,
    /// firmware, capability, or lifetime mismatch.
    pub fn validate_against_profile(
        &self,
        profile: &KagemushaHardwareProfileV1,
    ) -> Result<(), KagemushaValidationErrorV1> {
        self.validate_shape()?;
        self.hardware_credential.validate_against_profile(profile)
    }

    /// Return the canonical request identity consumed by a sender split.
    ///
    /// # Errors
    ///
    /// Returns an error when the request is invalid or cannot be encoded.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate_shape()?;
        digest_encoded(REQUEST_DIGEST_DOMAIN, self)
    }
}

impl KagemushaTransferStatementV1 {
    /// Build the exact pre-ID context authenticated by a peer-credit envelope.
    ///
    /// This projection intentionally excludes `lifecycle.credit_id`,
    /// `lifecycle.ciphertext_digest`, proof bytes, and certificate openings. It
    /// can therefore be computed before AEAD sealing while still binding the
    /// exact request, sender transition, trusted commit time, receiver lane/key,
    /// and normalized hardware-transition commitment.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid or substituted request/session binding.
    pub fn peer_credit_context_against(
        &self,
        request: &KagemushaPaymentRequestV1,
    ) -> Result<KagemushaPeerCreditContextV1, KagemushaValidationErrorV1> {
        request.validate_shape()?;
        let request_digest = request.canonical_digest()?;
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || self.lifecycle.version != self.version
            || self.amount == 0
            || self.transition_nullifier == [0; 32]
            || self.sender_before_commitment.is_zero()
            || self.sender_after_commitment.is_zero()
            || self.sender_before_commitment == self.sender_after_commitment
            || self.ciphertext_commitment == [0; 32]
            || self.hardware_transition_commitment == [0; 32]
            || self.request_digest != request_digest
            || self.recipient_lane_id != request.recipient_lane_id
            || self.recipient_encryption_key != request.recipient_encryption_key
            || self.amount != request.amount
            || self.committed_at_ms < request.issued_at_ms
            || self.committed_at_ms >= request.expires_at_ms
            || self.lifecycle.release_id != request.release_id
            || self.lifecycle.network_id != request.network_id
            || self.lifecycle.asset != request.asset
            || self.lifecycle.asset_incarnation != request.asset_incarnation
            || self.lifecycle.scale != request.scale
            || self.lifecycle.liability_pool_id != request.liability_pool_id
            || self.lifecycle.suite_id != request.hardware_credential.suite_id
            || self.lifecycle.request_id != request.request_id
            || self.lifecycle.credit_id != self.expected_credit_id()?
        {
            return Err(invalid("kagemusha.peer_credit_context.binding"));
        }
        let context = KagemushaPeerCreditContextV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            request_digest,
            sender_before_commitment: self.sender_before_commitment,
            sender_after_commitment: self.sender_after_commitment,
            lifecycle_context_digest: peer_credit_lifecycle_context_digest_v1(&self.lifecycle)?,
            recipient_lane_id: self.recipient_lane_id,
            recipient_encryption_key: self.recipient_encryption_key,
            committed_at_ms: self.committed_at_ms,
            hardware_transition_commitment: self.hardware_transition_commitment,
        };
        context.validate_shape()?;
        Ok(context)
    }

    /// Compute the required output-credit identity.
    ///
    /// # Errors
    ///
    /// Returns an error when the canonical identity preimage cannot be encoded.
    pub fn expected_credit_id(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        kagemusha_credit_id_v1(
            self.transition_nullifier,
            self.request_digest,
            self.sender_before_commitment,
            self.sender_after_commitment,
            self.recipient_lane_id,
            self.recipient_encryption_key,
            self.amount,
            self.ciphertext_commitment,
        )
    }

    /// Populate the canonical unlinkable output-credit identity before AEAD.
    ///
    /// The `ciphertext_commitment` must already bind an ID-independent opening.
    /// Callers may then encrypt with the returned credit ID in plaintext or
    /// associated data and set `lifecycle.ciphertext_digest` to the resulting
    /// exact bytes before validating the complete statement.
    ///
    /// # Errors
    ///
    /// Returns an error when credit identity hashing fails.
    pub fn seal_credit_id(mut self) -> Result<Self, KagemushaValidationErrorV1> {
        self.lifecycle.credit_id = self.expected_credit_id()?;
        Ok(self)
    }

    /// Validate the exact unlinkable public send binding.
    ///
    /// # Errors
    ///
    /// Returns an error when lifecycle, output, nullifier, or hardware binding is invalid.
    pub fn validate(&self) -> Result<(), KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1 || self.lifecycle.version != self.version {
            return Err(invalid("kagemusha.statement.version"));
        }
        self.lifecycle.validate()?;
        for (field, value) in [
            (
                "kagemusha.statement.transition_nullifier",
                self.transition_nullifier,
            ),
            ("kagemusha.statement.request_digest", self.request_digest),
            (
                "kagemusha.statement.recipient_lane_id",
                self.recipient_lane_id,
            ),
            (
                "kagemusha.statement.recipient_encryption_key",
                self.recipient_encryption_key,
            ),
            (
                "kagemusha.statement.ciphertext_commitment",
                self.ciphertext_commitment,
            ),
            (
                "kagemusha.statement.hardware_transition_commitment",
                self.hardware_transition_commitment,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        require_valid_x25519_public_key(
            "kagemusha.statement.recipient_encryption_key",
            self.recipient_encryption_key,
        )?;
        if self.amount == 0
            || self.committed_at_ms == 0
            || self.sender_before_commitment.is_zero()
            || self.sender_after_commitment.is_zero()
            || self.sender_before_commitment == self.sender_after_commitment
            || self.lifecycle.operation_kind != KagemushaOperationKindV1::SendSplit
        {
            return Err(invalid("kagemusha.statement.operation"));
        }
        if self.lifecycle.credit_id != self.expected_credit_id()? {
            return Err(invalid("kagemusha.statement.credit_id"));
        }
        Ok(())
    }

    /// Return the common semantic digest constrained by both Pasta parities.
    ///
    /// # Errors
    ///
    /// Returns an error when the statement is invalid or cannot be encoded.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate()?;
        digest_encoded(STATEMENT_DIGEST_DOMAIN, self)
    }
}

impl KagemushaPairedProofV1 {
    /// Validate fixed parity roles, proof caps, and exact history sizes.
    ///
    /// # Errors
    ///
    /// Returns an error when the proof is empty, oversized, aliased, or mis-bound.
    pub fn validate_shape_for_semantic_digest(
        &self,
        expected_semantic_digest: [u8; 32],
    ) -> Result<(), KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1 {
            return Err(invalid("kagemusha.proof.version"));
        }
        require_nonzero(
            "kagemusha.proof.eq_protocol_digest",
            self.eq_protocol_digest,
        )?;
        require_nonzero(
            "kagemusha.proof.ep_protocol_digest",
            self.ep_protocol_digest,
        )?;
        require_nonzero("kagemusha.proof.semantic_digest", self.semantic_digest)?;
        require_nonzero(
            "kagemusha.proof.guard_eq_credential_audit",
            self.guard_eq_credential_audit,
        )?;
        require_nonzero(
            "kagemusha.proof.guard_ep_credential_audit",
            self.guard_ep_credential_audit,
        )?;
        require_nonzero("kagemusha.proof.eq_deferred_audit", self.eq_deferred_audit)?;
        require_nonzero("kagemusha.proof.ep_deferred_audit", self.ep_deferred_audit)?;
        if self.eq_protocol_digest == self.ep_protocol_digest
            || self.semantic_digest != expected_semantic_digest
            || self.guard_eq_credential_audit == self.guard_ep_credential_audit
            || self.eq_deferred_audit == self.ep_deferred_audit
        {
            return Err(invalid("kagemusha.proof.role_binding"));
        }
        if self.eq_proof.is_empty()
            || self.ep_proof.is_empty()
            || self.eq_proof.len() > KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1
            || self.ep_proof.len() > KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1
            || self.eq_proof.len() + self.ep_proof.len() > KAGEMUSHA_CURRENT_PROOFS_MAX_BYTES_V1
        {
            return Err(invalid("kagemusha.proof.current"));
        }
        if self.eq_history.len() != KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1
            || self.ep_history.len() != KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1
            || self.eq_history.iter().all(|byte| *byte == 0)
            || self.ep_history.iter().all(|byte| *byte == 0)
            || self.eq_history == self.ep_history
        {
            return Err(invalid("kagemusha.proof.history"));
        }
        require_encoded_size(self, KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1)?;
        Ok(())
    }
}

impl KagemushaPaymentV1 {
    /// Encode this validated payment as canonical unpadded `kgm1:` base64url text.
    ///
    /// # Errors
    ///
    /// Returns an error when request validation, encoding, or a size bound fails.
    pub fn encode_text_against(
        &self,
        request: &KagemushaPaymentRequestV1,
    ) -> Result<String, KagemushaValidationErrorV1> {
        self.validate_shape_against(request)?;
        encode_kagemusha_text_v1(
            self,
            KAGEMUSHA_PAYMENT_MAX_BYTES_V1,
            KAGEMUSHA_PAYMENT_TEXT_MAX_BYTES_V1,
        )
    }

    /// Decode one exact canonical unpadded `kgm1:` payment against its request.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid context, size, prefix, padding, base64url, or Norito bytes.
    pub fn decode_text_exact_against(
        text: &str,
        request: &KagemushaPaymentRequestV1,
    ) -> Result<Self, KagemushaValidationErrorV1> {
        decode_kagemusha_text_v1(
            text,
            KAGEMUSHA_PAYMENT_MAX_BYTES_V1,
            KAGEMUSHA_PAYMENT_TEXT_MAX_BYTES_V1,
            |bytes| Self::decode_canonical_shape_exact_against(bytes, request),
        )
    }

    /// Decode and validate one exact bounded sender response.
    ///
    /// The outer cap is enforced before Norito reads declared collection lengths.
    ///
    /// # Errors
    ///
    /// Returns an error for an oversized, malformed, non-canonical, or invalid response.
    pub fn decode_canonical_shape_exact_against(
        bytes: &[u8],
        request: &KagemushaPaymentRequestV1,
    ) -> Result<Self, KagemushaValidationErrorV1> {
        let payment: Self = decode_bounded_canonical(bytes, KAGEMUSHA_PAYMENT_MAX_BYTES_V1)?;
        payment.validate_shape_against(request)?;
        Ok(payment)
    }

    /// Validate this response's complete structural binding against the exact
    /// signed recipient request.
    ///
    /// The literal trusted hardware commit instant must fall inside the signed
    /// request window. This function deliberately accepts no current-wall-clock
    /// argument: money committed in-window remains valid indefinitely.
    ///
    /// # Errors
    ///
    /// This checks proof framing and digest bindings, not the recursive proof's
    /// cryptographic validity or the release catalog. Monetary admission must
    /// first authenticate the request profile and then use the release-pinned
    /// native proof verifier.
    ///
    /// Returns an error when a public context, proof shape, statement, request,
    /// trusted commit time, or size binding fails.
    pub fn validate_shape_against(
        &self,
        request: &KagemushaPaymentRequestV1,
    ) -> Result<(), KagemushaValidationErrorV1> {
        request.validate_shape()?;
        let request_digest = request.canonical_digest()?;
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || self.statement.version != self.version
            || self.statement.lifecycle.release_id != request.release_id
            || self.statement.lifecycle.network_id != request.network_id
            || self.statement.lifecycle.asset != request.asset
            || self.statement.lifecycle.asset_incarnation != request.asset_incarnation
            || self.statement.lifecycle.scale != request.scale
            || self.statement.lifecycle.liability_pool_id != request.liability_pool_id
            || self.statement.lifecycle.suite_id != request.hardware_credential.suite_id
            || self.statement.request_digest != request_digest
            || self.statement.lifecycle.request_id != request.request_id
            || self.statement.recipient_lane_id != request.recipient_lane_id
            || self.statement.recipient_encryption_key != request.recipient_encryption_key
            || self.statement.amount != request.amount
            || self.statement.committed_at_ms < request.issued_at_ms
            || self.statement.committed_at_ms >= request.expires_at_ms
        {
            return Err(invalid("kagemusha.payment.request_binding"));
        }
        self.statement.validate()?;
        KagemushaEncryptedCreditEnvelopeV1::decode_canonical_shape_exact_against_recipient_key(
            &self.encrypted_credit,
            self.statement.recipient_encryption_key,
        )?;
        KagemushaEncryptedCreditAadV1::for_peer(&self.statement, request)?;
        if self.statement.lifecycle.ciphertext_digest
            != kagemusha_ciphertext_digest_v1(&self.encrypted_credit)
        {
            return Err(invalid("kagemusha.payment.encrypted_credit"));
        }
        self.proof
            .validate_shape_for_semantic_digest(self.statement.canonical_digest()?)?;
        require_encoded_size(self, KAGEMUSHA_PAYMENT_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Return the canonical response digest after validating its request.
    ///
    /// # Errors
    ///
    /// Returns an error when the response is invalid or cannot be encoded.
    pub fn canonical_digest_against(
        &self,
        request: &KagemushaPaymentRequestV1,
    ) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate_shape_against(request)?;
        digest_encoded(PAYMENT_DIGEST_DOMAIN, self)
    }

    /// Return the unlinkable circuit nullifier used for conflict detection.
    ///
    /// # Errors
    ///
    /// Returns an error only when the reserved all-zero value is present.
    pub fn sender_conflict_key(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        require_nonzero(
            "kagemusha.payment.transition_nullifier",
            self.statement.transition_nullifier,
        )?;
        Ok(self.statement.transition_nullifier)
    }
}

/// Encode the exact canonical post-persistence acknowledgement bytes.
///
/// # Errors
///
/// Returns an error when canonical Norito encoding fails.
pub fn kagemusha_acknowledgement_signing_bytes_v1(
    version: u16,
    request_digest: [u8; 32],
    payment_digest: [u8; 32],
    inbox_receipt: KagemushaInboxReceiptV1,
) -> Result<Vec<u8>, KagemushaValidationErrorV1> {
    Ok(norito::encode_canonical(
        &AcknowledgementSigningPreimageV1 {
            domain: ACKNOWLEDGEMENT_SIGNING_DOMAIN.to_vec(),
            version,
            request_digest,
            payment_digest,
            inbox_receipt,
        },
    )?)
}

impl KagemushaAcknowledgementV1 {
    /// Encode this validated acknowledgement as canonical unpadded `kgm1:` base64url text.
    ///
    /// # Errors
    ///
    /// Returns an error when context validation, encoding, or a size bound fails.
    pub fn encode_text_against(
        &self,
        request: &KagemushaPaymentRequestV1,
        payment: &KagemushaPaymentV1,
    ) -> Result<String, KagemushaValidationErrorV1> {
        self.validate_shape_against(request, payment)?;
        encode_kagemusha_text_v1(
            self,
            KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1,
            KAGEMUSHA_ACKNOWLEDGEMENT_TEXT_MAX_BYTES_V1,
        )
    }

    /// Decode one exact canonical unpadded `kgm1:` acknowledgement.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid context, size, prefix, padding, base64url, or Norito bytes.
    pub fn decode_text_exact_against(
        text: &str,
        request: &KagemushaPaymentRequestV1,
        payment: &KagemushaPaymentV1,
    ) -> Result<Self, KagemushaValidationErrorV1> {
        decode_kagemusha_text_v1(
            text,
            KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1,
            KAGEMUSHA_ACKNOWLEDGEMENT_TEXT_MAX_BYTES_V1,
            |bytes| Self::decode_canonical_shape_exact_against(bytes, request, payment),
        )
    }

    /// Decode and validate one exact bounded durable-inbox acknowledgement.
    ///
    /// # Errors
    ///
    /// Returns an error for an oversized, malformed, non-canonical, or invalid acknowledgement.
    pub fn decode_canonical_shape_exact_against(
        bytes: &[u8],
        request: &KagemushaPaymentRequestV1,
        payment: &KagemushaPaymentV1,
    ) -> Result<Self, KagemushaValidationErrorV1> {
        let acknowledgement: Self =
            decode_bounded_canonical(bytes, KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1)?;
        acknowledgement.validate_shape_against(request, payment)?;
        Ok(acknowledgement)
    }

    /// Return the exact bytes signed after persisting the inbox receipt.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, KagemushaValidationErrorV1> {
        kagemusha_acknowledgement_signing_bytes_v1(
            self.version,
            self.request_digest,
            self.payment_digest,
            self.inbox_receipt,
        )
    }

    /// Validate this acknowledgement's structural binding against its request
    /// and response.
    ///
    /// Hardware epoch, monotonic inbox sequence, and acknowledgement time stay
    /// private under the signed receipt commitment.
    ///
    /// # Errors
    ///
    /// Returns an error when receipt, identity, signature, or size binding fails.
    pub fn validate_shape_against(
        &self,
        request: &KagemushaPaymentRequestV1,
        payment: &KagemushaPaymentV1,
    ) -> Result<(), KagemushaValidationErrorV1> {
        payment.validate_shape_against(request)?;
        let request_digest = request.canonical_digest()?;
        let payment_digest = payment.canonical_digest_against(request)?;
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || self.request_digest != request_digest
            || self.payment_digest != payment_digest
            || self.inbox_receipt.version != self.version
            || self.inbox_receipt.credit_id != payment.statement.lifecycle.credit_id
            || self.inbox_receipt.receipt_commitment == [0; 32]
        {
            return Err(invalid("kagemusha.acknowledgement.binding"));
        }
        self.signature.verify(
            &request.hardware_credential.device_public_key,
            &self.canonical_signing_bytes()?,
        )?;
        require_encoded_size(self, KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1)?;
        Ok(())
    }
}

const fn unpadded_base64url_len(raw_len: usize) -> usize {
    raw_len / 3 * 4
        + match raw_len % 3 {
            0 => 0,
            1 => 2,
            _ => 3,
        }
}

fn validate_kagemusha_raw_session_size_v1(raw: usize) -> Result<(), KagemushaValidationErrorV1> {
    if raw > KAGEMUSHA_SESSION_MAX_BYTES_V1 {
        return Err(KagemushaValidationErrorV1::EncodedSizeExceeded {
            actual: raw,
            max: KAGEMUSHA_SESSION_MAX_BYTES_V1,
        });
    }
    Ok(())
}

/// Validate a complete session's structural bindings and return its raw size.
///
/// This does not cryptographically verify the recursive payment proof or
/// authenticate the embedded hardware profile against a release catalog.
///
/// # Errors
///
/// Returns an error when a message is invalid or the aggregate raw/text envelope is oversized.
pub fn validate_kagemusha_session_shape_v1(
    request: &KagemushaPaymentRequestV1,
    payment: &KagemushaPaymentV1,
    acknowledgement: &KagemushaAcknowledgementV1,
) -> Result<usize, KagemushaValidationErrorV1> {
    acknowledgement.validate_shape_against(request, payment)?;
    let lengths = [
        require_encoded_size(request, KAGEMUSHA_PAYMENT_REQUEST_MAX_BYTES_V1)?,
        require_encoded_size(payment, KAGEMUSHA_PAYMENT_MAX_BYTES_V1)?,
        require_encoded_size(acknowledgement, KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1)?,
    ];
    let raw = lengths.iter().sum::<usize>();
    validate_kagemusha_raw_session_size_v1(raw)?;
    let text = lengths
        .iter()
        .map(|length| KAGEMUSHA_TEXT_PREFIX_V1.len() + unpadded_base64url_len(*length))
        .sum::<usize>();
    if text > KAGEMUSHA_TEXT_SESSION_MAX_BYTES_V1 {
        return Err(KagemushaValidationErrorV1::EncodedSizeExceeded {
            actual: text,
            max: KAGEMUSHA_TEXT_SESSION_MAX_BYTES_V1,
        });
    }
    Ok(raw)
}

impl KagemushaMintAuthorizationContextV1 {
    /// Validate the exact pre-ID recipient, asset, release, and commitment context.
    ///
    /// # Errors
    ///
    /// Returns an error for a reserved value, invalid asset incarnation, or
    /// non-canonical pooled-reserve identity.
    pub fn validate_shape(&self) -> Result<(), KagemushaValidationErrorV1> {
        require_valid_header(
            self.version,
            &self.network_id,
            self.scale,
            Some(self.amount),
            "kagemusha.mint_authorization_context.header",
        )?;
        self.asset_incarnation
            .validate()
            .map_err(|_| invalid("kagemusha.mint_authorization_context.asset_incarnation"))?;
        for (field, value) in [
            (
                "kagemusha.mint_authorization_context.operation_id",
                self.operation_id,
            ),
            (
                "kagemusha.mint_authorization_context.release_id",
                self.release_id,
            ),
            (
                "kagemusha.mint_authorization_context.suite_id",
                self.suite_id,
            ),
            (
                "kagemusha.mint_authorization_context.vk_digest",
                self.vk_digest,
            ),
            (
                "kagemusha.mint_authorization_context.artifact_manifest_digest",
                self.artifact_manifest_digest,
            ),
            (
                "kagemusha.mint_authorization_context.liability_pool_id",
                self.liability_pool_id,
            ),
            (
                "kagemusha.mint_authorization_context.hardware_credential_id",
                self.hardware_credential_id,
            ),
            (
                "kagemusha.mint_authorization_context.hardware_profile_id",
                self.hardware_profile_id,
            ),
            (
                "kagemusha.mint_authorization_context.recipient_credential_commitment",
                self.recipient_credential_commitment,
            ),
            (
                "kagemusha.mint_authorization_context.credit_commitment",
                self.credit_commitment,
            ),
            (
                "kagemusha.mint_authorization_context.recipient_one_time_key",
                self.recipient_one_time_key,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        require_valid_x25519_public_key(
            "kagemusha.mint_authorization_context.recipient_one_time_key",
            self.recipient_one_time_key,
        )?;
        if self.policy_epoch == 0
            || self.liability_pool_id
                != kagemusha_liability_pool_id_v1(
                    &self.network_id,
                    &self.asset,
                    self.asset_incarnation,
                )?
        {
            return Err(invalid("kagemusha.mint_authorization_context.binding"));
        }
        Ok(())
    }

    /// Return the pre-ID digest included in issuance and credit identities.
    ///
    /// # Errors
    ///
    /// Returns an error when shape validation or canonical encoding fails.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate_shape()?;
        digest_encoded(MINT_AUTHORIZATION_CONTEXT_DIGEST_DOMAIN, self)
    }
}

impl KagemushaMintAuthorizationStatementV1 {
    /// Validate the complete post-encryption statement without granting authority.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid context, version, identifier, or ciphertext binding.
    pub fn validate_shape(&self) -> Result<(), KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1 || self.context.version != self.version {
            return Err(invalid("kagemusha.mint_authorization_statement.version"));
        }
        self.context.validate_shape()?;
        for (field, value) in [
            (
                "kagemusha.mint_authorization_statement.issuance_commitment",
                self.issuance_commitment,
            ),
            (
                "kagemusha.mint_authorization_statement.credit_id",
                self.credit_id,
            ),
            (
                "kagemusha.mint_authorization_statement.ciphertext_digest",
                self.ciphertext_digest,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        Ok(())
    }

    /// Return the semantic digest constrained by both authorization proof parities.
    ///
    /// # Errors
    ///
    /// Returns an error when shape validation or canonical encoding fails.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate_shape()?;
        digest_encoded(MINT_AUTHORIZATION_STATEMENT_DIGEST_DOMAIN, self)
    }

    /// Return the exact mint associated data authenticated by the credit envelope.
    ///
    /// # Errors
    ///
    /// Returns an error when this authorization statement is invalid.
    pub fn encrypted_credit_aad(
        &self,
    ) -> Result<KagemushaEncryptedCreditAadV1, KagemushaValidationErrorV1> {
        KagemushaEncryptedCreditAadV1::for_mint(self)
    }
}

impl KagemushaMintAuthorizationV1 {
    /// Validate proof framing and exact statement binding without granting authority.
    ///
    /// Core must resolve the named release, profile, suite, verifying keys, and
    /// artifact manifest from authenticated state and cryptographically verify
    /// both proof parities before mutating payer balance or pooled reserve.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid statement/proof binding or encoded size.
    pub fn validate_shape(&self) -> Result<(), KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1 || self.statement.version != self.version {
            return Err(invalid("kagemusha.mint_authorization.version"));
        }
        let semantic_digest = self.statement.canonical_digest()?;
        self.proof
            .validate_shape_for_semantic_digest(semantic_digest)?;
        require_encoded_size(self, KAGEMUSHA_MINT_AUTHORIZATION_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Return the digest recursively bound by the finalized mint helper.
    ///
    /// # Errors
    ///
    /// Returns an error when shape validation or canonical encoding fails.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate_shape()?;
        digest_encoded(MINT_AUTHORIZATION_DIGEST_DOMAIN, self)
    }

    /// Encode one shape-validated mint authorization as `kgm1:` text.
    ///
    /// # Errors
    ///
    /// Returns an error when validation, encoding, or a size bound fails.
    pub fn encode_text_shape(&self) -> Result<String, KagemushaValidationErrorV1> {
        self.validate_shape()?;
        encode_kagemusha_text_v1(
            self,
            KAGEMUSHA_MINT_AUTHORIZATION_MAX_BYTES_V1,
            KAGEMUSHA_MINT_AUTHORIZATION_TEXT_MAX_BYTES_V1,
        )
    }

    /// Decode one exact canonical mint authorization without granting authority.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid size, text framing, canonical bytes, or shape.
    pub fn decode_text_shape_exact(text: &str) -> Result<Self, KagemushaValidationErrorV1> {
        decode_kagemusha_text_v1(
            text,
            KAGEMUSHA_MINT_AUTHORIZATION_MAX_BYTES_V1,
            KAGEMUSHA_MINT_AUTHORIZATION_TEXT_MAX_BYTES_V1,
            Self::decode_canonical_shape_exact,
        )
    }

    /// Decode one exact bounded mint authorization without granting authority.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized, malformed, non-canonical, or invalid bytes.
    pub fn decode_canonical_shape_exact(bytes: &[u8]) -> Result<Self, KagemushaValidationErrorV1> {
        let authorization: Self =
            decode_bounded_canonical(bytes, KAGEMUSHA_MINT_AUTHORIZATION_MAX_BYTES_V1)?;
        authorization.validate_shape()?;
        Ok(authorization)
    }
}

impl KagemushaMintCreditStatementV1 {
    fn lifecycle_context_digest(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        digest_encoded(
            MINT_LIFECYCLE_CONTEXT_DOMAIN,
            &MintLifecycleContextPreimageV1 {
                version: self.lifecycle.version,
                network_id: self.lifecycle.network_id,
                protocol_version: self.lifecycle.protocol_version,
                suite_id: self.lifecycle.suite_id,
                vk_digest: self.lifecycle.vk_digest,
                release_id: self.lifecycle.release_id,
                asset: self.lifecycle.asset.clone(),
                asset_incarnation: self.lifecycle.asset_incarnation,
                scale: self.lifecycle.scale,
                liability_pool_id: self.lifecycle.liability_pool_id,
                hardware_profile_id: self.lifecycle.hardware_profile_id,
                policy_epoch: self.lifecycle.policy_epoch,
                operation_kind: self.lifecycle.operation_kind,
            },
        )
    }

    fn credit_id_preimage(&self) -> Result<MintCreditIdPreimageV1, KagemushaValidationErrorV1> {
        Ok(MintCreditIdPreimageV1 {
            lifecycle_context_digest: self.lifecycle_context_digest()?,
            recipient_credential_commitment: self.recipient_credential_commitment,
            authorization_context_digest: self.authorization_context_digest,
            amount: self.amount,
            issuance_commitment: self.issuance_commitment,
            recipient: self.recipient.clone(),
            credit_commitment: self.credit_commitment,
        })
    }

    /// Compute the unique mint-credit identity from committed issuance and output bindings.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn expected_credit_id(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        digest_encoded(MINT_CREDIT_ID_DOMAIN, &self.credit_id_preimage()?)
    }

    /// Populate the canonical mint-credit identity.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn seal_credit_id(mut self) -> Result<Self, KagemushaValidationErrorV1> {
        self.lifecycle.credit_id = self.expected_credit_id()?;
        Ok(self)
    }

    /// Validate a public committed-liability mint statement's complete shape.
    ///
    /// This does not authenticate the committed credential, release, or proof.
    /// Core must match the statement to the finalized authenticated top-up and
    /// verify the release-pinned helper proof before monetary admission.
    ///
    /// # Errors
    ///
    /// Returns an error when any issuance, recipient, or output binding is invalid.
    pub fn validate_shape(&self) -> Result<(), KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || self.lifecycle.version != self.version
            || self.amount == 0
            || self.minted_at_ms == 0
        {
            return Err(invalid("kagemusha.mint_statement.header"));
        }
        self.lifecycle.validate()?;
        for (field, value) in [
            (
                "kagemusha.mint_statement.recipient_credential_commitment",
                self.recipient_credential_commitment,
            ),
            (
                "kagemusha.mint_statement.authorization_context_digest",
                self.authorization_context_digest,
            ),
            (
                "kagemusha.mint_statement.mint_authorization_digest",
                self.mint_authorization_digest,
            ),
            (
                "kagemusha.mint_statement.issuance_commitment",
                self.issuance_commitment,
            ),
            (
                "kagemusha.mint_statement.credit_commitment",
                self.credit_commitment,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        if self.lifecycle.operation_kind != KagemushaOperationKindV1::MintFold
            || self.lifecycle.credit_id != self.expected_credit_id()?
        {
            return Err(invalid("kagemusha.mint_statement.credit_id"));
        }
        Ok(())
    }

    /// Return the mint statement digest constrained by both proof parities.
    ///
    /// # Errors
    ///
    /// Returns an error when the statement is invalid or cannot be encoded.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate_shape()?;
        digest_encoded(MINT_STATEMENT_DIGEST_DOMAIN, self)
    }
}

impl KagemushaMintCreditV1 {
    /// Encode this shape-validated mint credit as canonical `kgm1:` text.
    ///
    /// # Errors
    ///
    /// Returns an error when validation, encoding, or a size bound fails.
    pub fn encode_text_shape(&self) -> Result<String, KagemushaValidationErrorV1> {
        self.validate_shape()?;
        encode_kagemusha_text_v1(
            self,
            KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1,
            KAGEMUSHA_MINT_CREDIT_TEXT_MAX_BYTES_V1,
        )
    }

    /// Decode one exact canonical unpadded `kgm1:` mint credit.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid size, prefix, padding, base64url, Norito, or credit data.
    pub fn decode_text_shape_exact(text: &str) -> Result<Self, KagemushaValidationErrorV1> {
        decode_kagemusha_text_v1(
            text,
            KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1,
            KAGEMUSHA_MINT_CREDIT_TEXT_MAX_BYTES_V1,
            Self::decode_canonical_shape_exact,
        )
    }

    /// Decode and validate one exact bounded top-up mint credit.
    ///
    /// # Errors
    ///
    /// Returns an error for an oversized, malformed, non-canonical, or invalid credit.
    pub fn decode_canonical_shape_exact(bytes: &[u8]) -> Result<Self, KagemushaValidationErrorV1> {
        let credit: Self = decode_bounded_canonical(bytes, KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1)?;
        credit.validate_shape()?;
        Ok(credit)
    }

    /// Validate committed-liability, proof framing, recipient opening, and
    /// release-binding shape.
    ///
    /// This does not cryptographically verify either proof parity or
    /// authenticate the release/profile. Core must do both before folding.
    ///
    /// # Errors
    ///
    /// Returns an error when any mint-credit invariant fails.
    pub fn validate_shape(&self) -> Result<(), KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1 || self.statement.version != self.version {
            return Err(invalid("kagemusha.mint_credit.version"));
        }
        self.statement.validate_shape()?;
        self.proof
            .validate_shape_for_semantic_digest(self.statement.canonical_digest()?)?;
        for (field, value) in [
            (
                "kagemusha.mint_credit.finality_certificate_binding",
                self.finality_certificate_binding,
            ),
            (
                "kagemusha.mint_credit.finality_authority_head",
                self.finality_authority_head,
            ),
            (
                "kagemusha.mint_credit.finality_genesis_roster_id",
                self.finality_genesis_roster_id,
            ),
            (
                "kagemusha.mint_credit.finality_proof_binding_digest",
                self.finality_proof_binding_digest,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        KagemushaEncryptedCreditEnvelopeV1::decode_canonical_shape_exact(&self.encrypted_credit)?;
        if self.statement.lifecycle.ciphertext_digest
            != kagemusha_ciphertext_digest_v1(&self.encrypted_credit)
        {
            return Err(invalid("kagemusha.mint_credit.encrypted_credit"));
        }
        require_nonzero(
            "kagemusha.mint_credit.artifact_manifest_digest",
            self.artifact_manifest_digest,
        )?;
        require_encoded_size(self, KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Validate a mint credit against the exact pre-debit recipient authorization.
    ///
    /// This remains a shape and digest-binding check. Core must authenticate the
    /// release and cryptographically verify the authorization and mint helper
    /// proofs before debit, reserve mutation, or folding.
    ///
    /// # Errors
    ///
    /// Returns an error for any substituted authorization, recipient key,
    /// ciphertext, release, asset, credential, or amount binding.
    pub fn validate_shape_against_authorization(
        &self,
        authorization: &KagemushaMintAuthorizationV1,
    ) -> Result<(), KagemushaValidationErrorV1> {
        self.validate_shape()?;
        authorization.validate_shape()?;
        let context = &authorization.statement.context;
        if self.statement.authorization_context_digest != context.canonical_digest()?
            || self.statement.mint_authorization_digest != authorization.canonical_digest()?
            || self.statement.issuance_commitment != authorization.statement.issuance_commitment
            || self.statement.lifecycle.credit_id != authorization.statement.credit_id
            || self.statement.lifecycle.ciphertext_digest
                != authorization.statement.ciphertext_digest
            || self.statement.amount != context.amount
            || self.statement.recipient != context.recipient
            || self.statement.recipient_credential_commitment
                != context.recipient_credential_commitment
            || self.statement.credit_commitment != context.credit_commitment
            || self.statement.lifecycle.release_id != context.release_id
            || self.statement.lifecycle.suite_id != context.suite_id
            || self.statement.lifecycle.vk_digest != context.vk_digest
            || self.statement.lifecycle.network_id != context.network_id
            || self.statement.lifecycle.asset != context.asset
            || self.statement.lifecycle.asset_incarnation != context.asset_incarnation
            || self.statement.lifecycle.scale != context.scale
            || self.statement.lifecycle.liability_pool_id != context.liability_pool_id
            || self.statement.lifecycle.hardware_profile_id != context.hardware_profile_id
            || self.statement.lifecycle.policy_epoch != context.policy_epoch
            || self.artifact_manifest_digest != context.artifact_manifest_digest
            || authorization.statement.ciphertext_digest
                != kagemusha_ciphertext_digest_v1(&self.encrypted_credit)
        {
            return Err(invalid("kagemusha.mint_credit.authorization_binding"));
        }
        KagemushaEncryptedCreditEnvelopeV1::decode_canonical_shape_exact_against_recipient_key(
            &self.encrypted_credit,
            context.recipient_one_time_key,
        )?;
        authorization.statement.encrypted_credit_aad()?;
        Ok(())
    }
}

impl KagemushaRedemptionStatementV1 {
    fn redemption_id_preimage(&self) -> Result<RedemptionIdPreimageV1, KagemushaValidationErrorV1> {
        Ok(RedemptionIdPreimageV1 {
            lifecycle_binding_digest: self.lifecycle.canonical_digest()?,
            terminal_nullifier: self.terminal_nullifier,
            sender_before_commitment: self.sender_before_commitment,
            sender_after_commitment: self.sender_after_commitment,
            amount: self.amount,
            beneficiary: self.beneficiary.clone(),
            redemption_commitment: self.redemption_commitment,
            committed_at_ms: self.committed_at_ms,
            hardware_transition_commitment: self.hardware_transition_commitment,
        })
    }

    /// Compute the identity of this exact redemption output.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn expected_redemption_id(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        digest_encoded(REDEMPTION_ID_DOMAIN, &self.redemption_id_preimage()?)
    }

    /// Populate the canonical redemption identity.
    ///
    /// # Errors
    ///
    /// Returns an error when identity hashing fails.
    pub fn seal_redemption_id(mut self) -> Result<Self, KagemushaValidationErrorV1> {
        self.redemption_id = self.expected_redemption_id()?;
        Ok(self)
    }

    /// Return the unlinkable circuit nullifier used for conflict detection.
    ///
    /// # Errors
    ///
    /// Returns an error only when the reserved all-zero value is present.
    pub fn sender_conflict_key(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        require_nonzero(
            "kagemusha.redemption_statement.terminal_nullifier",
            self.terminal_nullifier,
        )?;
        Ok(self.terminal_nullifier)
    }

    /// Validate an unlinkable terminal aggregate-state transition.
    ///
    /// # Errors
    ///
    /// Returns an error when any lifecycle, nullifier, output, or commit binding is invalid.
    pub fn validate_shape(&self) -> Result<(), KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1 || self.lifecycle.version != self.version {
            return Err(invalid("kagemusha.redemption_statement.version"));
        }
        self.lifecycle.validate()?;
        for (field, value) in [
            (
                "kagemusha.redemption_statement.terminal_nullifier",
                self.terminal_nullifier,
            ),
            (
                "kagemusha.redemption_statement.redemption_commitment",
                self.redemption_commitment,
            ),
            (
                "kagemusha.redemption_statement.redemption_id",
                self.redemption_id,
            ),
            (
                "kagemusha.redemption_statement.hardware_transition_commitment",
                self.hardware_transition_commitment,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        if self.amount == 0
            || self.committed_at_ms == 0
            || self.sender_before_commitment.is_zero()
            || self.sender_after_commitment.is_zero()
            || self.lifecycle.operation_kind != KagemushaOperationKindV1::RedeemSplit
            || self.sender_before_commitment == self.sender_after_commitment
            || self.terminal_nullifier == self.redemption_commitment
            || self.terminal_nullifier == self.redemption_id
            || self.redemption_commitment == self.redemption_id
        {
            return Err(invalid("kagemusha.redemption_statement.operation"));
        }
        if self.redemption_id != self.expected_redemption_id()? {
            return Err(invalid("kagemusha.redemption_statement.redemption_id"));
        }
        Ok(())
    }

    /// Return the redemption semantic digest constrained by both proof parities.
    ///
    /// # Errors
    ///
    /// Returns an error when the statement is invalid or cannot be encoded.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate_shape()?;
        digest_encoded(REDEMPTION_STATEMENT_DIGEST_DOMAIN, self)
    }
}

impl KagemushaRedemptionVoucherV1 {
    /// Encode this shape-validated voucher as canonical `kgm1:` text.
    ///
    /// # Errors
    ///
    /// Returns an error when validation, encoding, or a size bound fails.
    pub fn encode_text_shape(&self) -> Result<String, KagemushaValidationErrorV1> {
        self.validate_shape()?;
        encode_kagemusha_text_v1(
            self,
            KAGEMUSHA_REDEMPTION_VOUCHER_MAX_BYTES_V1,
            KAGEMUSHA_REDEMPTION_VOUCHER_TEXT_MAX_BYTES_V1,
        )
    }

    /// Decode one exact canonical unpadded `kgm1:` redemption voucher.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid size, prefix, padding, base64url, Norito, or voucher data.
    pub fn decode_text_shape_exact(text: &str) -> Result<Self, KagemushaValidationErrorV1> {
        decode_kagemusha_text_v1(
            text,
            KAGEMUSHA_REDEMPTION_VOUCHER_MAX_BYTES_V1,
            KAGEMUSHA_REDEMPTION_VOUCHER_TEXT_MAX_BYTES_V1,
            Self::decode_canonical_shape_exact,
        )
    }

    /// Decode and validate one exact bounded redemption voucher.
    ///
    /// # Errors
    ///
    /// Returns an error for an oversized, malformed, non-canonical, or invalid voucher.
    pub fn decode_canonical_shape_exact(bytes: &[u8]) -> Result<Self, KagemushaValidationErrorV1> {
        let voucher: Self =
            decode_bounded_canonical(bytes, KAGEMUSHA_REDEMPTION_VOUCHER_MAX_BYTES_V1)?;
        voucher.validate_shape()?;
        Ok(voucher)
    }

    /// Validate terminal state consumption and recursive hardware binding.
    ///
    /// Global redemption admission must additionally reject a previously seen
    /// `terminal_nullifier`.
    ///
    /// # Errors
    ///
    /// Returns an error when any voucher invariant fails.
    pub fn validate_shape(&self) -> Result<(), KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1 || self.statement.version != self.version {
            return Err(invalid("kagemusha.redemption_voucher.version"));
        }
        self.statement.validate_shape()?;
        self.proof
            .validate_shape_for_semantic_digest(self.statement.canonical_digest()?)?;
        require_encoded_size(self, KAGEMUSHA_REDEMPTION_VOUCHER_MAX_BYTES_V1)?;
        Ok(())
    }
}
