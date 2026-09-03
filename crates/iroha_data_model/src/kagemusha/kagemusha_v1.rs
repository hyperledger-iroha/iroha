//! Canonical first-release wire contract for hardware-guarded KAGEMUSHA.
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

/// Version carried by every clean-slate KAGEMUSHA wire value.
pub const KAGEMUSHA_WIRE_VERSION_V1: u16 = 1;
/// Version of the secure-device lane and journal lifecycle contract.
pub const KAGEMUSHA_DEVICE_LIFECYCLE_VERSION_V1: u16 = 1;
/// Device capability that commits sender state before exposing a payment.
pub const KAGEMUSHA_HANDOFF_CAPABILITY_V1: &str = "kagemusha_handoff_v1";
/// Text transport discriminator for canonical unpadded base64url messages.
pub const KAGEMUSHA_TEXT_PREFIX_V1: &str = "kgm1:";
/// Maximum authoritative asset scale represented by KAGEMUSHA V1.
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
pub const KAGEMUSHA_PAYMENT_REQUEST_MAX_BYTES_V1: usize = 928;
/// Maximum canonical sender-response bytes.
pub const KAGEMUSHA_PAYMENT_MAX_BYTES_V1: usize = 7_552;
/// Maximum canonical receiver-acknowledgement bytes.
pub const KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1: usize = 256;
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
/// Exact canonical envelope bytes for the fixed V1 plaintext and authentication tag.
///
/// This is an encoded-length identity, not a replacement for the existing
/// encrypted-credit transport cap or authenticated envelope validation.
pub const KAGEMUSHA_ENCRYPTED_CREDIT_CANONICAL_BYTES_V1: usize = 327;
/// Maximum canonical bytes in the fixed private credit-opening plaintext.
pub const KAGEMUSHA_CREDIT_OPENING_MAX_BYTES_V1: usize = 256;
/// Exact X25519 public-key width used by encrypted credit envelopes.
pub const KAGEMUSHA_X25519_PUBLIC_KEY_BYTES_V1: usize = 32;
/// Exact XChaCha20-Poly1305 nonce width used by encrypted credit envelopes.
pub const KAGEMUSHA_XCHACHA20POLY1305_NONCE_BYTES_V1: usize = 24;
/// Exact authentication-tag width appended to an encrypted credit plaintext.
pub const KAGEMUSHA_XCHACHA20POLY1305_TAG_BYTES_V1: usize = 16;
/// HKDF-SHA256 salt label for the KAGEMUSHA V1 encrypted-credit KEM.
pub const KAGEMUSHA_ENCRYPTED_CREDIT_KDF_SALT_LABEL_V1: &[u8] =
    b"iroha:kagemusha:v1:credit-envelope-salt\0";
/// HKDF-SHA256 info label for the KAGEMUSHA V1 encrypted-credit KEM.
pub const KAGEMUSHA_ENCRYPTED_CREDIT_KDF_INFO_LABEL_V1: &[u8] =
    b"iroha:kagemusha:v1:credit-envelope-key\0";
/// Maximum canonical hardware-profile registry entry bytes.
pub const KAGEMUSHA_HARDWARE_PROFILE_MAX_BYTES_V1: usize = 512;
/// Maximum canonical compact hardware credential bytes.
pub const KAGEMUSHA_HARDWARE_CREDENTIAL_MAX_BYTES_V1: usize = 768;
/// Exact canonical Norito bytes hashed by a V1 hardware credential identity.
pub const KAGEMUSHA_HARDWARE_CREDENTIAL_ID_PREIMAGE_BYTES_V1: usize = 376;
/// Byte offset of the 32-byte lane in the canonical credential-ID preimage.
pub const KAGEMUSHA_HARDWARE_CREDENTIAL_ID_LANE_OFFSET_V1: usize = 185;
/// Semantic value ranges in the canonical credential-ID preimage, in field order.
///
/// The fields are version, network, profile, suite, firmware policy, policy epoch,
/// lane, hardware epoch, hardware generation, device key, key reference, issuance,
/// and expiry. Header, CRC, and field-length prefixes are outside these ranges.
pub const KAGEMUSHA_HARDWARE_CREDENTIAL_ID_PREIMAGE_FIELD_RANGES_V1: [core::ops::Range<usize>; 13] = [
    41..43,
    44..76,
    77..109,
    110..142,
    143..175,
    176..184,
    185..217,
    218..250,
    251..259,
    260..325,
    326..358,
    359..367,
    368..376,
];
/// Exact canonical Norito bytes hashed by a V1 hardware-profile identity.
pub const KAGEMUSHA_HARDWARE_PROFILE_ID_PREIMAGE_BYTES_V1: usize = 378;
/// Semantic value ranges in the canonical profile-ID preimage, in field order.
///
/// The fields are version, protocol version, provider, platform class (u32-LE),
/// product, firmware policy, enrollment verifier, trust roots, allowed suite,
/// policy epoch, governance key, capabilities, qualification report, activation,
/// and expiry. Header, CRC, and field-length prefixes are outside these ranges.
pub const KAGEMUSHA_HARDWARE_PROFILE_ID_PREIMAGE_FIELD_RANGES_V1: [core::ops::Range<usize>; 15] = [
    41..43,
    44..46,
    47..79,
    80..84,
    85..117,
    118..150,
    151..183,
    184..216,
    217..249,
    250..258,
    259..324,
    325..327,
    328..360,
    361..369,
    370..378,
];
/// Exact canonical Norito bytes hashed by a randomized recipient commitment.
pub const KAGEMUSHA_RECIPIENT_CREDENTIAL_COMMITMENT_PREIMAGE_BYTES_V1: usize = 139;
/// Operation, credential-ID, and private opening ranges in that order.
pub const KAGEMUSHA_RECIPIENT_CREDENTIAL_COMMITMENT_PREIMAGE_FIELD_RANGES_V1: [core::ops::Range<
    usize,
>; 3] = [41..73, 74..106, 107..139];
/// Exact canonical Norito bytes hashed by a mint-credit opening commitment.
pub const KAGEMUSHA_MINT_CREDIT_OPENING_COMMITMENT_PREIMAGE_BYTES_V1: usize = 305;
/// Semantic mint-opening preimage ranges, in the authoritative field order.
///
/// The fields are version, network, asset digest, incarnation, scale, pool,
/// amount, recipient-account digest, one-time key, and private credit opening.
/// The incarnation range selects only its 32 hash bytes; both its outer length
/// at 117 and nested length at 118 remain framing bytes. Root padding occupies
/// 40..48 because the preimage includes a u128 amount.
pub const KAGEMUSHA_MINT_CREDIT_OPENING_COMMITMENT_PREIMAGE_FIELD_RANGES_V1: [core::ops::Range<
    usize,
>; 10] = [
    49..51,
    52..84,
    85..117,
    119..151,
    152..156,
    157..189,
    190..206,
    207..239,
    240..272,
    273..305,
];
/// Exact canonical Norito plaintext bytes in a V1 credit opening.
pub const KAGEMUSHA_CREDIT_OPENING_CANONICAL_BYTES_V1: usize = 200;
/// Version, credit ID, amount, credit opening, recipient opening, and recovery nonce ranges.
///
/// Header, CRC, root padding at 40..48, and field-length prefixes are excluded.
pub const KAGEMUSHA_CREDIT_OPENING_CANONICAL_FIELD_RANGES_V1: [core::ops::Range<usize>; 6] =
    [49..51, 52..84, 85..101, 102..134, 135..167, 168..200];
/// Qualification target for the three separately framed protocol messages.
pub const KAGEMUSHA_COMPLETE_EXCHANGE_TARGET_BYTES_V1: usize = 8_960;
/// Absolute raw cap for the complete request/payment/acknowledgement exchange.
pub const KAGEMUSHA_COMPLETE_EXCHANGE_MAX_BYTES_V1: usize = 9_211;
/// Absolute text cap for the three separately framed `kgm1:` messages.
pub const KAGEMUSHA_COMPLETE_TEXT_EXCHANGE_MAX_BYTES_V1: usize = 12_288;
/// Maximum canonical recoverable hardware commit-certificate bytes.
pub const KAGEMUSHA_COMMIT_CERTIFICATE_MAX_BYTES_V1: usize = 1_024;
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
/// Maximum canonical redemption-proof bytes.
pub const KAGEMUSHA_REDEMPTION_PROOF_MAX_BYTES_V1: u32 =
    outbox_budget_component_from_usize(KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1);
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
        outbox_budget_component_from_usize(KAGEMUSHA_COMMIT_CERTIFICATE_MAX_BYTES_V1),
        KAGEMUSHA_REDEMPTION_PROOF_MAX_BYTES_V1,
        outbox_budget_component_from_usize(canonical_envelope_max_bytes),
        KAGEMUSHA_OUTBOX_RETRY_METADATA_MAX_BYTES_V1,
    ];
    let mut total = 0_u32;
    let mut index = 0;
    while index < parts.len() {
        total = match total.checked_add(parts[index]) {
            Some(next) => next,
            None => panic!("KAGEMUSHA V1 outbox budget overflow"),
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
/// Hardware makes a committed payment an irrevocable receiver-bound credit.
pub const KAGEMUSHA_HARDWARE_CAPABILITY_RECEIVER_BOUND_CREDIT_COMMIT_V1: u16 = 1 << 4;
/// Hardware owns a rollback-resistant accepted-credit inbox and durable receipts.
pub const KAGEMUSHA_HARDWARE_CAPABILITY_ROLLBACK_RESISTANT_ACCEPTED_CREDIT_INBOX_V1: u16 = 1 << 5;
/// Hardware authenticates inbound staging, exact deduplication, and inbox paging.
pub const KAGEMUSHA_HARDWARE_CAPABILITY_AUTHENTICATED_INBOUND_STAGING_V1: u16 = 1 << 6;
/// Hardware recovers the authoritative replay root and authenticated sparse-tree state.
pub const KAGEMUSHA_HARDWARE_CAPABILITY_AUTHORITATIVE_REPLAY_ROOT_RECOVERY_V1: u16 = 1 << 7;
/// Hardware reserves sender terminal and envelope bytes before locking a predecessor.
pub const KAGEMUSHA_HARDWARE_CAPABILITY_SENDER_OUTBOX_RESERVATION_V1: u16 = 1 << 8;
/// Hardware owns an authenticated durable byte-identical retry outbox.
pub const KAGEMUSHA_HARDWARE_CAPABILITY_AUTHENTICATED_DURABLE_RETRY_OUTBOX_V1: u16 = 1 << 9;
/// Hardware atomically commits only the exact Core-verified candidate digest.
pub const KAGEMUSHA_HARDWARE_CAPABILITY_ATOMIC_VERIFIED_CANDIDATE_COMMIT_V1: u16 = 1 << 10;
/// Hardware recovers the terminal commit certificate after every terminal outcome.
pub const KAGEMUSHA_HARDWARE_CAPABILITY_RECOVERABLE_TERMINAL_COMMIT_CERTIFICATE_V1: u16 = 1 << 11;
/// Hardware supplies trusted time or consumes a secure monotonic authorization lease.
pub const KAGEMUSHA_HARDWARE_CAPABILITY_TRUSTED_TIME_OR_LEASE_V1: u16 = 1 << 12;
/// Hardware rotates the complete balance and replay root offline.
pub const KAGEMUSHA_HARDWARE_CAPABILITY_OFFLINE_HARDWARE_EPOCH_ROTATION_V1: u16 = 1 << 13;
/// Hardware rolls exhausted counters without cloning spend authority.
pub const KAGEMUSHA_HARDWARE_CAPABILITY_ROLLBACK_SAFE_COUNTER_ROLLOVER_V1: u16 = 1 << 14;
/// Hardware fails closed instead of falling back to software authority.
pub const KAGEMUSHA_HARDWARE_CAPABILITY_NO_SOFTWARE_FALLBACK_V1: u16 = 1 << 15;
/// Exact capability set required from every KAGEMUSHA V1 hardware profile.
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
        | KAGEMUSHA_HARDWARE_CAPABILITY_ATOMIC_VERIFIED_CANDIDATE_COMMIT_V1
        | KAGEMUSHA_HARDWARE_CAPABILITY_RECOVERABLE_TERMINAL_COMMIT_CERTIFICATE_V1
        | KAGEMUSHA_HARDWARE_CAPABILITY_TRUSTED_TIME_OR_LEASE_V1
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
const PREPARED_TRANSFER_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:prepared-transfer";
/// Domain for a request-bound peer credit identity.
pub const KAGEMUSHA_CREDIT_ID_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:credit-id";
/// Domain for the pre-ID commitment to one peer credit opening.
pub const KAGEMUSHA_PEER_CREDIT_OPENING_COMMITMENT_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:peer-credit-opening-commitment";
const STATEMENT_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:send-split-statement";
const LIFECYCLE_BINDING_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:lifecycle-binding";
const CIPHERTEXT_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:ciphertext";
const PEER_CREDIT_CONTEXT_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:peer-credit-context";
const ACCOUNT_IDENTITY_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:account-identity";
const MINT_CREDIT_OPENING_COMMITMENT_DOMAIN: &[u8] =
    b"iroha:kagemusha:v1:mint-credit-opening-commitment";
const RECIPIENT_CREDENTIAL_COMMITMENT_DOMAIN: &[u8] =
    b"iroha:kagemusha:v1:recipient-credential-commitment";
const COMMIT_CERTIFICATE_ID_DOMAIN: &[u8] = b"iroha:kagemusha:v1:commit-certificate-id";
const COMMIT_CERTIFICATE_DIGEST_DOMAIN: &[u8] = b"iroha:kagemusha:v1:commit-certificate";
const HARDWARE_TERMINAL_BODY_COMMITMENT_DOMAIN: &[u8] =
    b"iroha:kagemusha:v1:hardware-terminal-body";
const OUTBOX_RESERVATION_COMMITMENT_DOMAIN: &[u8] = b"iroha:kagemusha:v1:outbox-reservation";
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

/// Error returned when canonical KAGEMUSHA V1 data fails validation.
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
            Self::Codec(error) => write!(f, "canonical KAGEMUSHA V1 codec failed: {error}"),
            Self::EncodedSizeExceeded { actual, max } => {
                write!(f, "KAGEMUSHA V1 wire size {actual} exceeds limit {max}")
            }
            Self::InvalidField { field } => {
                write!(f, "invalid KAGEMUSHA V1 field `{field}`")
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

/// Sole KAGEMUSHA V1 device authority key.
///
/// The wire value is exactly one canonical uncompressed SEC1 NIST P-256 point
/// (`0x04 || x || y`). There is no algorithm tag or selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, IntoSchema)]
#[repr(transparent)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct KagemushaDevicePublicKeyV1([u8; KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1]);

/// Sole KAGEMUSHA V1 device signature.
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
            .expect("KAGEMUSHA device public key must be canonical SEC1 bytes")
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
            .expect("KAGEMUSHA device signature must be canonical raw P-256 bytes")
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

    /// Verify ECDSA-P256-SHA256 under the fixed KAGEMUSHA V1 profile.
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
    /// Terminal transported Eq/Fp history: the private carrier's current opening claim folded
    /// into its complete prior history and bound inside the compact outer proof.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub eq_history: Vec<u8>,
    /// Terminal transported Ep/Fq history: the private carrier's current opening claim folded
    /// into its complete prior history and bound inside the compact outer proof.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub ep_history: Vec<u8>,
}

/// Qualified platform class represented by one governed hardware profile.
///
/// A class label never grants KAGEMUSHA authority by itself. The complete
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

/// Governed KAGEMUSHA V1 non-forking hardware-service profile.
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
    /// Exact network on which the device may authorize KAGEMUSHA.
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

/// Exact one-byte IPM1 payload-kind discriminator.
///
/// These are the only three transported lifecycle message kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, IntoSchema)]
#[repr(u8)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "kind", content = "value", rename_all = "snake_case")]
#[norito(deny_unknown_fields)]
pub enum KagemushaIpm1PayloadKindV1 {
    /// Receiver payment request. Stable tag: `1`.
    Request = 1,
    /// Committed sender payment. Stable tag: `2`.
    Payment = 2,
    /// Durable receiver acknowledgement. Stable tag: `3`.
    Acknowledgement = 3,
}

impl KagemushaIpm1PayloadKindV1 {
    /// Return the frozen one-byte IPM1 wire tag.
    #[must_use]
    pub const fn wire_tag(self) -> u8 {
        self as u8
    }

    /// Parse one frozen IPM1 wire tag.
    ///
    /// # Errors
    ///
    /// Returns an error for every value outside `1..=3`.
    pub fn from_wire_tag(tag: u8) -> Result<Self, KagemushaValidationErrorV1> {
        match tag {
            1 => Ok(Self::Request),
            2 => Ok(Self::Payment),
            3 => Ok(Self::Acknowledgement),
            _ => Err(invalid("kagemusha.ipm1.payload_kind")),
        }
    }
}

impl norito::NoritoSerialize for KagemushaIpm1PayloadKindV1 {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        writer.write_all(&[self.wire_tag()])?;
        Ok(())
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        Some(1)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        Some(1)
    }
}

impl<'de> norito::NoritoDeserialize<'de> for KagemushaIpm1PayloadKindV1 {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("IPM1 payload kind must use a frozen tag")
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

impl<'a> norito::core::DecodeFromSlice<'a> for KagemushaIpm1PayloadKindV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::Error> {
        let tag = *bytes.first().ok_or(norito::Error::LengthMismatch)?;
        let value =
            Self::from_wire_tag(tag).map_err(|error| norito::Error::Message(error.to_string()))?;
        Ok((value, 1))
    }
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
/// the signed payment request or mint-authorization context and is not repeated here.
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
    /// Establish a zero-balance hardware lane. Stable tag: `0`.
    Bootstrap,
    /// Fold one finalized reserve-backed mint credit. Stable tag: `1`.
    MintFold,
    /// Produce one receiver-bound payment credit. Stable tag: `2`.
    SendSplit,
    /// Fold one receiver-bound credit into the aggregate balance. Stable tag: `3`.
    ReceiveFold,
    /// Produce one online redemption voucher. Stable tag: `4`.
    RedeemSplit,
    /// Rotate the hardware epoch without changing monetary value. Stable tag: `5`.
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
    /// Receiver lane commitment for `SendSplit`, otherwise zero.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub receiver_lane_commitment: [u8; 32],
    /// Receiver credit identity for `SendSplit`, otherwise zero.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub credit_id: [u8; 32],
    /// Encrypted-credit digest for `SendSplit`, otherwise zero.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ciphertext_digest: [u8; 32],
}

/// Trusted-time payload proving commitment before the request deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaTrustedCommitTimeV1 {
    /// Hiding commitment to the qualified clock evidence, commit instant, and authority.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub time_evidence_commitment: [u8; 32],
}

/// Secure monotonic-lease evidence proving commitment before a deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaMonotonicLeaseV1 {
    /// Hiding commitment to the unique lease, window, consumed counter, and authority.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub lease_evidence_commitment: [u8; 32],
}

/// Public evidence that qualified hardware committed before the applicable deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(
    tag = "source",
    content = "evidence",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum KagemushaCommitEvidenceV1 {
    /// Qualified hardware supplied trusted time.
    TrustedTime(KagemushaTrustedCommitTimeV1),
    /// Qualified hardware consumed a secure monotonic authorization lease.
    MonotonicLease(KagemushaMonotonicLeaseV1),
}

/// Sender outbox capacity reserved before hardware may consume its predecessor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaOutboxReservationV1 {
    /// Unique one-use reservation identity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub reservation_id: [u8; 32],
    /// Exact operation whose canonical terminal envelope owns the reservation.
    pub operation_kind: KagemushaOperationKindV1,
    /// Physical outbox bytes reserved for all recoverable artifacts.
    pub reserved_outbox_bytes: u32,
    /// Inclusive reservation issuance time in Unix milliseconds.
    pub issued_at_ms: u64,
    /// Exclusive deadline for consuming this reservation.
    pub expires_at_ms: u64,
}

/// Self-free hardware terminal body committed before a certificate ID exists.
///
/// The construction order is terminal body, terminal commitment, certificate
/// ID, then payment or redemption proof. This record contains no derived
/// certificate or final-proof field, avoiding a hash fixed point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaHardwareTerminalBodyV1 {
    /// Wire/lifecycle contract version.
    pub version: u16,
    /// Digest of the exact prepared candidate persisted before commit.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub candidate_envelope_digest: [u8; 32],
    /// Digest of the released lifecycle binding.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub lifecycle_binding_digest: [u8; 32],
    /// Unique transition nullifier installed by this terminal commit.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub transition_nullifier: [u8; 32],
    /// Commitment to the consumed private outbox reservation.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub outbox_reservation_commitment: [u8; 32],
    /// Trusted-time or secure-lease evidence consumed at commit.
    pub commit_evidence: KagemushaCommitEvidenceV1,
    /// Qualified hardware profile governing the terminal body.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub hardware_profile_id: [u8; 32],
    /// Exact hardware-policy epoch.
    pub policy_epoch: u64,
    /// Hiding commitment to the private committed successor state.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub private_successor_commitment: [u8; 32],
    /// Hiding commitment to the rollback-resistant journal terminal record.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub private_journal_commitment: [u8; 32],
    /// Hiding commitment to deterministic recovery material.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub private_recovery_commitment: [u8; 32],
}

/// Recoverable hardware terminal certificate emitted by atomic commit.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaCommitCertificateV1 {
    /// Wire version.
    pub version: u16,
    /// Digest-derived terminal certificate identity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub certificate_id: [u8; 32],
    /// Digest of the durably persisted, locally verified candidate envelope.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub candidate_envelope_digest: [u8; 32],
    /// Digest of the exact released lifecycle binding.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub lifecycle_binding_digest: [u8; 32],
    /// Unique transition nullifier installed by hardware commit.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub transition_nullifier: [u8; 32],
    /// Hiding commitment to the consumed private outbox reservation.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub outbox_reservation_commitment: [u8; 32],
    /// Trusted-time or monotonic-lease evidence used at commit.
    pub commit_evidence: KagemushaCommitEvidenceV1,
    /// Qualified sender hardware profile proven by the terminal proof.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub hardware_profile_id: [u8; 32],
    /// Sender policy epoch proven by the terminal proof.
    pub policy_epoch: u64,
    /// Commitment to the self-free private terminal body and hardware state.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub hardware_terminal_commitment: [u8; 32],
}

/// Final paired proof authorizing one committed offline payment.
///
/// This exposes neither aggregate-state links nor stable credential pseudonyms.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaPaymentProofV1 {
    /// Wire version.
    pub version: u16,
    /// Canonical little-endian Fp protocol digest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub eq_protocol_digest: [u8; 32],
    /// Canonical little-endian Fq protocol digest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ep_protocol_digest: [u8; 32],
    /// Digest of the unlinkable public transition statement.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub semantic_digest: [u8; 32],
    /// Digest of the prepared transition proof/envelope verified by this proof.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub candidate_envelope_digest: [u8; 32],
    /// Digest of the terminal commit certificate verified by this proof.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub commit_certificate_digest: [u8; 32],
    /// Eq/Fp deferred reciprocal-verification audit.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub eq_deferred_audit: [u8; 32],
    /// Ep/Fq deferred reciprocal-verification audit.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ep_deferred_audit: [u8; 32],
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

/// Final paired proof authorizing one online redemption.
///
/// This exposes neither aggregate-state links nor stable credential pseudonyms.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaRedemptionProofV1 {
    /// Wire version.
    pub version: u16,
    /// Canonical little-endian Fp protocol digest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub eq_protocol_digest: [u8; 32],
    /// Canonical little-endian Fq protocol digest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ep_protocol_digest: [u8; 32],
    /// Digest of the unlinkable public transition statement.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub semantic_digest: [u8; 32],
    /// Digest of the prepared transition proof/envelope verified by this proof.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub candidate_envelope_digest: [u8; 32],
    /// Digest of the terminal commit certificate verified by this proof.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub commit_certificate_digest: [u8; 32],
    /// Eq/Fp deferred reciprocal-verification audit.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub eq_deferred_audit: [u8; 32],
    /// Ep/Fq deferred reciprocal-verification audit.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ep_deferred_audit: [u8; 32],
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

/// Receiver-created authorization for any number of distinct exact-amount payments.
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
    /// Exact positive amount requested for each payment made against this request.
    pub amount: u128,
    /// Recipient X25519 public key used to encrypt every independently identified credit.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub recipient_encryption_key: [u8; KAGEMUSHA_X25519_PUBLIC_KEY_BYTES_V1],
    /// Compact qualified-hardware credential binding the receiver device, lane, and policy.
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

/// Compact terminal output bound by the post-commit proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaPaymentOutputV1 {
    /// Wire version.
    pub version: u16,
    /// Digest of the exact signed receiver request.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub request_digest: [u8; 32],
    /// Exact positive amount transferred by this payment.
    pub amount: u128,
    /// Hiding commitment to the consumed sender aggregate state.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub sender_before_commitment: [u8; 32],
    /// Hiding commitment to the immediately usable sender successor state.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub sender_after_commitment: [u8; 32],
    /// Unique, proof-derived transition nullifier with no public state preimage.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub transition_nullifier: [u8; 32],
    /// Request-bound receiver credit identity derived from the transition nullifier.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub credit_id: [u8; 32],
    /// Commitment to amount-bound ciphertext semantics.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ciphertext_commitment: [u8; 32],
    /// Trusted-time or secure monotonic-lease evidence used by hardware commit.
    pub commit_evidence: KagemushaCommitEvidenceV1,
    /// Trusted sender commit time; request expiry gates only this instant.
    pub committed_at_ms: u64,
}

/// Sender terminal response carrying its post-commit proof and recoverable certificate.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaPaymentV1 {
    /// Wire version.
    pub version: u16,
    /// Exact public output known before hardware commitment.
    pub output: KagemushaPaymentOutputV1,
    /// Recipient-only encrypted credit opening.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub encrypted_credit: Vec<u8>,
    /// Recoverable certificate emitted only after the exact candidate was committed.
    pub commit_certificate: KagemushaCommitCertificateV1,
    /// Post-commit paired proof of the candidate and its hardware terminal certificate.
    pub proof: KagemushaPaymentProofV1,
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
    /// KAGEMUSHA account authorized to receive the credit.
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
/// Both proof parities are required to establish the exact authenticated credential,
/// fresh commitment openings, recipient encryption-key possession, and ciphertext
/// opening relation described by `statement`. Shape validation of this value does
/// not establish any of those monetary relations.
///
/// TODO: Close the finalized MintAuthority helper's recursive authorization and
/// canonical statement-opening relations. Its current signed-root membership of a
/// host-precomputed statement digest is not that closure. MintFold must also join the
/// reopened recipient/lifecycle and exact replay key/value to the active state lane.
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
    /// SHA commitment to the inner paired authority metadata, proved by both compact helpers.
    ///
    /// This preserves the inner audit/history commitment in outer public cells 20..21; it is
    /// not recomputed from the transported outer audit/history fields. Shape validation alone
    /// does not authenticate it: both proof parities and complete histories must be verified.
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
/// Stable sender identity and predecessor/successor commitments remain private
/// proof inputs and never appear in this public record.
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
    /// Commitment to the public redemption claim and private proof output.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub redemption_commitment: [u8; 32],
    /// Unique identity of this exact redemption voucher.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub redemption_id: [u8; 32],
    /// Trusted-time or secure monotonic-lease evidence used by hardware commit.
    pub commit_evidence: KagemushaCommitEvidenceV1,
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
    /// Recoverable hardware terminal certificate for the exact candidate.
    pub commit_certificate: KagemushaCommitCertificateV1,
    /// Final redemption proof of balance conservation and committed terminal state.
    pub proof: KagemushaRedemptionProofV1,
    /// Digest of the authenticated artifact manifest used by the proof.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub artifact_manifest_digest: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.kagemusha.v1.liability-pool-preimage")]
struct LiabilityPoolPreimageV1 {
    network_id: NetworkId,
    asset: AssetDefinitionId,
    asset_incarnation: AxtAssetIncarnationV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode)]
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
    /// Exact positive amount transferred by the request.
    pub amount: u128,
    /// Consumed sender state commitment.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub sender_before_commitment: [u8; 32],
    /// Sender successor state commitment.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub sender_after_commitment: [u8; 32],
    /// Request-bound transfer digest fixed before encryption and proving.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub prepared_transfer_digest: [u8; 32],
    /// Request-owned recipient X25519 public key.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub recipient_encryption_key: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode)]
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

fn fixed_canonical_preimage_bytes_v1<T: Encode, const N: usize>(
    preimage: &T,
    field: &'static str,
) -> Result<[u8; N], KagemushaValidationErrorV1> {
    norito::encode_canonical(preimage)?
        .try_into()
        .map_err(|_| invalid(field))
}

mod canonical_mint_frame_sealed {
    /// Prevent downstream crates from extending the monetary-frame allowlist.
    pub trait Sealed {}
}

/// A KAGEMUSHA monetary type whose canonical Norito frame may enter a proof relation.
///
/// This sealed marker restricts the layout API to the exact V1 mint hierarchy and its typed
/// account/asset identity leaves. It does not validate a value or grant monetary authority; the
/// recursive circuit must still constrain all semantic payload bytes and verify the release-pinned
/// proofs that authenticate them.
pub trait KagemushaCanonicalMintFrameV1: Encode + canonical_mint_frame_sealed::Sealed {
    /// Frozen offset at which this root type's bare payload starts.
    const PAYLOAD_OFFSET_V1: usize;
    /// Frozen canonical V1 layout flags for this monetary type.
    const HEADER_FLAGS_V1: u8;
}

impl canonical_mint_frame_sealed::Sealed for AccountId {}
impl KagemushaCanonicalMintFrameV1 for AccountId {
    const PAYLOAD_OFFSET_V1: usize = 40;
    const HEADER_FLAGS_V1: u8 = norito::core::header_flags::COMPACT_LEN;
}
impl canonical_mint_frame_sealed::Sealed for AssetDefinitionId {}
impl KagemushaCanonicalMintFrameV1 for AssetDefinitionId {
    const PAYLOAD_OFFSET_V1: usize = 40;
    const HEADER_FLAGS_V1: u8 = norito::core::header_flags::COMPACT_LEN;
}
impl canonical_mint_frame_sealed::Sealed for KagemushaMintAuthorizationContextV1 {}
impl KagemushaCanonicalMintFrameV1 for KagemushaMintAuthorizationContextV1 {
    const PAYLOAD_OFFSET_V1: usize = 48;
    const HEADER_FLAGS_V1: u8 = norito::core::header_flags::COMPACT_LEN;
}
impl canonical_mint_frame_sealed::Sealed for KagemushaMintAuthorizationStatementV1 {}
impl KagemushaCanonicalMintFrameV1 for KagemushaMintAuthorizationStatementV1 {
    const PAYLOAD_OFFSET_V1: usize = 48;
    const HEADER_FLAGS_V1: u8 = norito::core::header_flags::COMPACT_LEN;
}
impl canonical_mint_frame_sealed::Sealed for KagemushaPairedProofV1 {}
impl KagemushaCanonicalMintFrameV1 for KagemushaPairedProofV1 {
    const PAYLOAD_OFFSET_V1: usize = 40;
    const HEADER_FLAGS_V1: u8 = norito::core::header_flags::COMPACT_LEN;
}
impl canonical_mint_frame_sealed::Sealed for KagemushaMintAuthorizationV1 {}
impl KagemushaCanonicalMintFrameV1 for KagemushaMintAuthorizationV1 {
    const PAYLOAD_OFFSET_V1: usize = 48;
    const HEADER_FLAGS_V1: u8 = norito::core::header_flags::COMPACT_LEN;
}
impl canonical_mint_frame_sealed::Sealed for KagemushaLifecycleBindingV1 {}
impl KagemushaCanonicalMintFrameV1 for KagemushaLifecycleBindingV1 {
    const PAYLOAD_OFFSET_V1: usize = 40;
    const HEADER_FLAGS_V1: u8 = norito::core::header_flags::COMPACT_LEN;
}
impl canonical_mint_frame_sealed::Sealed for KagemushaMintCreditStatementV1 {}
impl KagemushaCanonicalMintFrameV1 for KagemushaMintCreditStatementV1 {
    const PAYLOAD_OFFSET_V1: usize = 48;
    const HEADER_FLAGS_V1: u8 = norito::core::header_flags::COMPACT_LEN;
}
impl canonical_mint_frame_sealed::Sealed for KagemushaMintCreditV1 {}
impl KagemushaCanonicalMintFrameV1 for KagemushaMintCreditV1 {
    const PAYLOAD_OFFSET_V1: usize = 48;
    const HEADER_FLAGS_V1: u8 = norito::core::header_flags::COMPACT_LEN;
}

/// Model-owned fixed prefix for one variable-length canonical monetary frame.
///
/// The prefix contains the exact Norito header and root-alignment padding. `Some(byte)` pins a
/// codec-owned byte. The sixteen `None` slots are exactly the little-endian payload length at
/// `23..31` and CRC64-XZ at `31..39`; a circuit must derive both from its constrained payload.
/// No payload bytes, proof authority, or protocol-specific capacity are carried here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KagemushaCanonicalFramePrefixV1 {
    bytes: Vec<Option<u8>>,
}

impl KagemushaCanonicalFramePrefixV1 {
    /// Borrow the fixed header/alignment template.
    #[must_use]
    pub fn bytes(&self) -> &[Option<u8>] {
        &self.bytes
    }

    /// Return the exact offset at which the bare canonical payload starts.
    #[must_use]
    pub fn payload_offset(&self) -> usize {
        self.bytes.len()
    }
}

/// Derive authoritative header and root-alignment framing for an allowed mint value.
///
/// The descriptor is obtained from the canonical encoder under its fixed V1 flags, then checked
/// against an independently produced bare payload. This keeps schema bytes, codec version,
/// layout flags, and type alignment in the data model rather than duplicating them in a circuit.
/// The result deliberately omits a capacity: callers must use an existing release/envelope bound,
/// and selecting a smaller buffer must never become a cumulative money-usage restriction.
///
/// # Errors
///
/// Returns an error if canonical framing cannot be encoded or its header/payload split is not the
/// exact V1 layout.
pub fn kagemusha_canonical_mint_frame_prefix_v1<T: KagemushaCanonicalMintFrameV1>(
    value: &T,
) -> Result<KagemushaCanonicalFramePrefixV1, KagemushaValidationErrorV1> {
    const LENGTH_RANGE: core::ops::Range<usize> = 23..31;
    const CHECKSUM_RANGE: core::ops::Range<usize> = 31..39;
    let frame = norito::encode_canonical(value)?;
    let mut payload = Vec::new();
    norito::codec::encode_adaptive_into(value, &mut payload)?;
    let payload_offset = frame
        .len()
        .checked_sub(payload.len())
        .filter(|offset| *offset >= norito::core::Header::SIZE)
        .ok_or_else(|| invalid("kagemusha.canonical_mint_frame.prefix"))?;
    if frame.get(payload_offset..) != Some(payload.as_slice())
        || payload_offset != T::PAYLOAD_OFFSET_V1
        || canonical_frame_overhead_v1::<T>() != T::PAYLOAD_OFFSET_V1
        || frame[39] != T::HEADER_FLAGS_V1
        || frame.get(LENGTH_RANGE.clone())
            != Some(
                &u64::try_from(payload.len())
                    .map_err(|_| invalid("kagemusha.canonical_mint_frame.payload_length"))?
                    .to_le_bytes(),
            )
        || frame[norito::core::Header::SIZE..payload_offset]
            .iter()
            .any(|byte| *byte != 0)
    {
        return Err(invalid("kagemusha.canonical_mint_frame.prefix"));
    }
    let mut bytes = frame[..payload_offset]
        .iter()
        .copied()
        .map(Some)
        .collect::<Vec<_>>();
    bytes[LENGTH_RANGE].fill(None);
    bytes[CHECKSUM_RANGE].fill(None);
    Ok(KagemushaCanonicalFramePrefixV1 { bytes })
}

fn fixed_canonical_preimage_layout_v1<T: Encode, const N: usize>(
    preimage: &T,
    ranges: &[core::ops::Range<usize>],
    values: &[&[u8]],
    field: &'static str,
) -> Result<[Option<u8>; N], KagemushaValidationErrorV1> {
    let bytes = fixed_canonical_preimage_bytes_v1::<_, N>(preimage, field)?;
    if N < 40 || ranges.len() != values.len() {
        return Err(invalid(field));
    }
    let mut layout = bytes.map(Some);
    layout[31..39].fill(None);
    let mut previous_end = 40;
    for (range, value) in ranges.iter().zip(values) {
        if range.start < previous_end || bytes.get(range.clone()) != Some(*value) {
            return Err(invalid(field));
        }
        layout[range.clone()].fill(None);
        previous_end = range.end;
    }
    Ok(layout)
}

/// Return fixed framing for the canonical randomized recipient commitment.
///
/// `Some` bytes pin the authoritative Norito header and field prefixes. Semantic
/// values occupy [`KAGEMUSHA_RECIPIENT_CREDENTIAL_COMMITMENT_PREIMAGE_FIELD_RANGES_V1`].
/// CRC64 bytes 31..39 are `None`, but are part of the hashed preimage: a circuit
/// creating a fresh commitment must compute and constrain them from its payload.
/// This template itself neither computes a witness CRC nor grants authority.
///
/// # Errors
///
/// Returns an error for encoding failure or an unexpected canonical field layout.
pub fn kagemusha_recipient_credential_commitment_preimage_layout_v1() -> Result<
    [Option<u8>; KAGEMUSHA_RECIPIENT_CREDENTIAL_COMMITMENT_PREIMAGE_BYTES_V1],
    KagemushaValidationErrorV1,
> {
    let preimage = RecipientCredentialCommitmentPreimageV1 {
        operation_id: [1; 32],
        hardware_credential_id: [2; 32],
        recipient_binding_opening: [3; 32],
    };
    fixed_canonical_preimage_layout_v1(
        &preimage,
        &KAGEMUSHA_RECIPIENT_CREDENTIAL_COMMITMENT_PREIMAGE_FIELD_RANGES_V1,
        &[
            &preimage.operation_id,
            &preimage.hardware_credential_id,
            &preimage.recipient_binding_opening,
        ],
        "kagemusha.recipient_credential_commitment.preimage_layout",
    )
}

/// Return fixed framing for the canonical mint-credit opening commitment.
///
/// The template pins the complete header, root alignment padding, and all field
/// prefixes, including the incarnation's nested prefix. Semantic value ranges
/// are [`KAGEMUSHA_MINT_CREDIT_OPENING_COMMITMENT_PREIMAGE_FIELD_RANGES_V1`].
/// CRC64 bytes 31..39 are deliberately `None`; fresh commitment circuits must
/// compute them from the canonical payload, excluding header and root padding.
/// No raw field concatenation or alternate digest transcript is introduced.
///
/// # Errors
///
/// Returns an error for encoding failure or an unexpected canonical field layout.
pub fn kagemusha_mint_credit_opening_commitment_preimage_layout_v1() -> Result<
    [Option<u8>; KAGEMUSHA_MINT_CREDIT_OPENING_COMMITMENT_PREIMAGE_BYTES_V1],
    KagemushaValidationErrorV1,
> {
    let field = "kagemusha.mint_credit_opening_commitment.preimage_layout";
    let preimage = MintCreditOpeningCommitmentPreimageV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        network_id: [1; 32],
        asset_identity_digest: [2; 32],
        asset_incarnation: AxtAssetIncarnationV1::try_from_bytes(
            *iroha_crypto::Hash::new(b"kagemusha-mint-opening-preimage-layout").as_ref(),
        )
        .map_err(|_| invalid(field))?,
        scale: 3,
        liability_pool_id: [4; 32],
        amount: 5,
        recipient_account_digest: [6; 32],
        recipient_one_time_key: [7; 32],
        credit_commitment_opening: [8; 32],
    };
    fixed_canonical_preimage_layout_v1(
        &preimage,
        &KAGEMUSHA_MINT_CREDIT_OPENING_COMMITMENT_PREIMAGE_FIELD_RANGES_V1,
        &[
            &preimage.version.to_le_bytes(),
            &preimage.network_id,
            &preimage.asset_identity_digest,
            preimage.asset_incarnation.as_bytes(),
            &preimage.scale.to_le_bytes(),
            &preimage.liability_pool_id,
            &preimage.amount.to_le_bytes(),
            &preimage.recipient_account_digest,
            &preimage.recipient_one_time_key,
            &preimage.credit_commitment_opening,
        ],
        field,
    )
}

/// Return fixed framing for the unchanged canonical hardware-profile identity.
///
/// The platform discriminant is a u32-LE semantic field, not a framing constant.
/// Semantic fields occupy [`KAGEMUSHA_HARDWARE_PROFILE_ID_PREIMAGE_FIELD_RANGES_V1`].
/// CRC64 bytes 31..39 are `None`: compute them when creating a fresh identity, or
/// bind the entire preimage hash to an already authenticated profile identity.
/// This shape-only template supplies no profile qualification or authority.
///
/// # Errors
///
/// Returns an error for encoding failure or an unexpected canonical field layout.
pub fn kagemusha_hardware_profile_id_preimage_layout_v1()
-> Result<[Option<u8>; KAGEMUSHA_HARDWARE_PROFILE_ID_PREIMAGE_BYTES_V1], KagemushaValidationErrorV1>
{
    let field = "kagemusha.hardware_profile.id_preimage_layout";
    let generator = P256VerifyingKey::from_sec1_bytes(&[
        0x03, 0x6b, 0x17, 0xd1, 0xf2, 0xe1, 0x2c, 0x42, 0x47, 0xf8, 0xbc, 0xe6, 0xe5, 0x63, 0xa4,
        0x40, 0xf2, 0x77, 0x03, 0x7d, 0x81, 0x2d, 0xeb, 0x33, 0xa0, 0xf4, 0xa1, 0x39, 0x45, 0xd8,
        0x98, 0xc2, 0x96,
    ])
    .map_err(|_| invalid(field))?;
    let preimage = HardwareProfileIdPreimageV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        protocol_version: KAGEMUSHA_WIRE_VERSION_V1,
        provider_id: [1; 32],
        platform_class: KagemushaHardwarePlatformClassV1::OtherQualified,
        product_class_digest: [2; 32],
        firmware_policy_digest: [3; 32],
        enrollment_attestation_verifier_digest: [4; 32],
        attestation_trust_roots_digest: [5; 32],
        allowed_suite_commitment: [6; 32],
        policy_epoch: 7,
        governance_credential_public_key: KagemushaDevicePublicKeyV1::from_sec1_bytes(
            generator.to_encoded_point(false).as_bytes(),
        )?,
        capability_mask: KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1,
        qualification_report_digest: [8; 32],
        valid_from_ms: 9,
        expires_at_ms: 10,
    };
    fixed_canonical_preimage_layout_v1(
        &preimage,
        &KAGEMUSHA_HARDWARE_PROFILE_ID_PREIMAGE_FIELD_RANGES_V1,
        &[
            &preimage.version.to_le_bytes(),
            &preimage.protocol_version.to_le_bytes(),
            &preimage.provider_id,
            &3_u32.to_le_bytes(),
            &preimage.product_class_digest,
            &preimage.firmware_policy_digest,
            &preimage.enrollment_attestation_verifier_digest,
            &preimage.attestation_trust_roots_digest,
            &preimage.allowed_suite_commitment,
            &preimage.policy_epoch.to_le_bytes(),
            preimage.governance_credential_public_key.as_sec1_bytes(),
            &preimage.capability_mask.to_le_bytes(),
            &preimage.qualification_report_digest,
            &preimage.valid_from_ms.to_le_bytes(),
            &preimage.expires_at_ms.to_le_bytes(),
        ],
        field,
    )
}

/// Return fixed framing for the canonical encrypted-credit plaintext.
///
/// Semantic fields occupy [`KAGEMUSHA_CREDIT_OPENING_CANONICAL_FIELD_RANGES_V1`].
/// Header, root padding, and field prefixes are fixed. The CRC64 at 31..39 is
/// part of the plaintext and must be computed from the payload when a circuit
/// creates these bytes; `None` does not authorize omitting or zeroing the CRC.
///
/// # Errors
///
/// Returns an error for encoding failure or an unexpected canonical field layout.
pub fn kagemusha_credit_opening_canonical_layout_v1()
-> Result<[Option<u8>; KAGEMUSHA_CREDIT_OPENING_CANONICAL_BYTES_V1], KagemushaValidationErrorV1> {
    let opening = KagemushaCreditOpeningV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        credit_id: [1; 32],
        amount: 2,
        credit_commitment_opening: [3; 32],
        recipient_binding_opening: [4; 32],
        recovery_nonce: [5; 32],
    };
    fixed_canonical_preimage_layout_v1(
        &opening,
        &KAGEMUSHA_CREDIT_OPENING_CANONICAL_FIELD_RANGES_V1,
        &[
            &opening.version.to_le_bytes(),
            &opening.credit_id,
            &opening.amount.to_le_bytes(),
            &opening.credit_commitment_opening,
            &opening.recipient_binding_opening,
            &opening.recovery_nonce,
        ],
        "kagemusha.credit_opening.canonical_layout",
    )
}

/// Return fixed framing bytes for the unchanged canonical credential-ID preimage.
///
/// Header bytes 0..31, the layout flag at 39, and all thirteen field-length
/// prefixes are fixed by the authoritative private Norito type. Value bytes and
/// the data-dependent CRC64 at 31..39 are left unconstrained. This template
/// supplies no credential authority and does not verify that CRC; proof users
/// must hash the entire preimage and bind its digest to an authenticated ID.
///
/// # Errors
///
/// Returns an error for a codec failure or unexpected first-release layout.
pub fn kagemusha_hardware_credential_id_preimage_layout_v1() -> Result<
    [Option<u8>; KAGEMUSHA_HARDWARE_CREDENTIAL_ID_PREIMAGE_BYTES_V1],
    KagemushaValidationErrorV1,
> {
    // Encoding this shape-only template never constructs or authorizes a credential.
    let generator = P256VerifyingKey::from_sec1_bytes(&[
        0x03, 0x6b, 0x17, 0xd1, 0xf2, 0xe1, 0x2c, 0x42, 0x47, 0xf8, 0xbc, 0xe6, 0xe5, 0x63, 0xa4,
        0x40, 0xf2, 0x77, 0x03, 0x7d, 0x81, 0x2d, 0xeb, 0x33, 0xa0, 0xf4, 0xa1, 0x39, 0x45, 0xd8,
        0x98, 0xc2, 0x96,
    ])
    .map_err(|_| invalid("kagemusha.hardware_credential.id_preimage_layout"))?;
    let preimage = HardwareCredentialIdPreimageV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        network_id: NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::new(b"kagemusha-credential-preimage-layout"),
        )),
        hardware_profile_id: [0; 32],
        suite_id: [0; 32],
        firmware_policy_digest: [0; 32],
        policy_epoch: 0,
        lane_commitment: [0; 32],
        hardware_epoch_id: [0; 32],
        hardware_epoch_generation: 0,
        device_public_key: KagemushaDevicePublicKeyV1::from_sec1_bytes(
            generator.to_encoded_point(false).as_bytes(),
        )?,
        device_key_reference: [0; 32],
        issued_at_ms: 0,
        expires_at_ms: 0,
    };
    let bytes = norito::encode_canonical(&preimage)?;
    if bytes.len() != KAGEMUSHA_HARDWARE_CREDENTIAL_ID_PREIMAGE_BYTES_V1 {
        return Err(invalid("kagemusha.hardware_credential.id_preimage_layout"));
    }
    let mut layout = [None; KAGEMUSHA_HARDWARE_CREDENTIAL_ID_PREIMAGE_BYTES_V1];
    for (index, byte) in bytes.iter().take(31).enumerate() {
        layout[index] = Some(*byte);
    }
    layout[39] = Some(bytes[39]);
    for (offset, length) in [
        (40, 2),
        (43, 32),
        (76, 32),
        (109, 32),
        (142, 32),
        (175, 8),
        (184, 32),
        (217, 32),
        (250, 8),
        (259, 65),
        (325, 32),
        (358, 8),
        (367, 8),
    ] {
        if bytes[offset] != length {
            return Err(invalid("kagemusha.hardware_credential.id_preimage_layout"));
        }
        layout[offset] = Some(length);
    }
    Ok(layout)
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
    amount: u128,
    beneficiary: AccountId,
    redemption_commitment: [u8; 32],
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

const fn kagemusha_operation_kind_tag_v1(operation: KagemushaOperationKindV1) -> u32 {
    match operation {
        KagemushaOperationKindV1::Bootstrap => 0,
        KagemushaOperationKindV1::MintFold => 1,
        KagemushaOperationKindV1::SendSplit => 2,
        KagemushaOperationKindV1::ReceiveFold => 3,
        KagemushaOperationKindV1::RedeemSplit => 4,
        KagemushaOperationKindV1::Rotate => 5,
    }
}

fn outbox_reservation_circuit_transcript_v1(reservation: KagemushaOutboxReservationV1) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(56);
    bytes.extend_from_slice(&reservation.reservation_id);
    bytes.extend_from_slice(
        &kagemusha_operation_kind_tag_v1(reservation.operation_kind).to_le_bytes(),
    );
    bytes.extend_from_slice(&reservation.reserved_outbox_bytes.to_le_bytes());
    bytes.extend_from_slice(&reservation.issued_at_ms.to_le_bytes());
    bytes.extend_from_slice(&reservation.expires_at_ms.to_le_bytes());
    bytes
}

fn commit_evidence_circuit_transcript_v1(evidence: KagemushaCommitEvidenceV1) -> [u8; 36] {
    let (tag, commitment) = match evidence {
        KagemushaCommitEvidenceV1::TrustedTime(value) => (0_u32, value.time_evidence_commitment),
        KagemushaCommitEvidenceV1::MonotonicLease(value) => {
            (1_u32, value.lease_evidence_commitment)
        }
    };
    let mut bytes = [0_u8; 36];
    bytes[..4].copy_from_slice(&tag.to_le_bytes());
    bytes[4..].copy_from_slice(&commitment);
    bytes
}

fn commit_certificate_id_circuit_transcript_v1(
    certificate: &KagemushaCommitCertificateV1,
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(238);
    bytes.extend_from_slice(&certificate.version.to_le_bytes());
    bytes.extend_from_slice(&certificate.candidate_envelope_digest);
    bytes.extend_from_slice(&certificate.lifecycle_binding_digest);
    bytes.extend_from_slice(&certificate.transition_nullifier);
    bytes.extend_from_slice(&certificate.outbox_reservation_commitment);
    bytes.extend_from_slice(&commit_evidence_circuit_transcript_v1(
        certificate.commit_evidence,
    ));
    bytes.extend_from_slice(&certificate.hardware_profile_id);
    bytes.extend_from_slice(&certificate.policy_epoch.to_le_bytes());
    bytes.extend_from_slice(&certificate.hardware_terminal_commitment);
    bytes
}

fn commit_certificate_circuit_transcript_v1(certificate: &KagemushaCommitCertificateV1) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(270);
    bytes.extend_from_slice(&certificate.version.to_le_bytes());
    bytes.extend_from_slice(&certificate.certificate_id);
    bytes.extend_from_slice(&certificate.candidate_envelope_digest);
    bytes.extend_from_slice(&certificate.lifecycle_binding_digest);
    bytes.extend_from_slice(&certificate.transition_nullifier);
    bytes.extend_from_slice(&certificate.outbox_reservation_commitment);
    bytes.extend_from_slice(&commit_evidence_circuit_transcript_v1(
        certificate.commit_evidence,
    ));
    bytes.extend_from_slice(&certificate.hardware_profile_id);
    bytes.extend_from_slice(&certificate.policy_epoch.to_le_bytes());
    bytes.extend_from_slice(&certificate.hardware_terminal_commitment);
    bytes
}

/// Derive the canonical digest bound to an encrypted KAGEMUSHA credit.
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

/// Derive the stable reference to an KAGEMUSHA device key.
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

/// Commit the private opening material before deriving its public peer credit ID.
///
/// This commitment deliberately excludes `credit_id`, so construction remains
/// acyclic. The receiver circuit separately constrains the opened credit ID and
/// amount to the proof-derived public values.
///
/// # Errors
///
/// Returns an error for a reserved digest, key, opening, nonce, or zero amount.
pub fn kagemusha_peer_credit_opening_commitment_v1(
    request_digest: [u8; 32],
    recipient_one_time_key: [u8; 32],
    amount: u128,
    credit_commitment_opening: [u8; 32],
    recipient_binding_opening: [u8; 32],
    recovery_nonce: [u8; 32],
) -> Result<[u8; 32], KagemushaValidationErrorV1> {
    for (field, value) in [
        (
            "kagemusha.peer_credit_opening.request_digest",
            request_digest,
        ),
        (
            "kagemusha.peer_credit_opening.recipient_one_time_key",
            recipient_one_time_key,
        ),
        (
            "kagemusha.peer_credit_opening.credit_commitment_opening",
            credit_commitment_opening,
        ),
        (
            "kagemusha.peer_credit_opening.recipient_binding_opening",
            recipient_binding_opening,
        ),
        (
            "kagemusha.peer_credit_opening.recovery_nonce",
            recovery_nonce,
        ),
    ] {
        require_nonzero(field, value)?;
    }
    if amount == 0 {
        return Err(invalid("kagemusha.peer_credit_opening.amount"));
    }
    let mut hasher = Sha256::new();
    hasher.update(KAGEMUSHA_PEER_CREDIT_OPENING_COMMITMENT_DOMAIN_V1);
    hasher.update([0]);
    hasher.update(KAGEMUSHA_WIRE_VERSION_V1.to_le_bytes());
    hasher.update(request_digest);
    hasher.update(recipient_one_time_key);
    hasher.update(amount.to_le_bytes());
    hasher.update(credit_commitment_opening);
    hasher.update(recipient_binding_opening);
    hasher.update(recovery_nonce);
    Ok(hasher.finalize().into())
}

/// Derive a request-bound peer credit identity.
///
/// Hashes the credit-ID domain, a zero separator, the transition nullifier, and
/// exact signed request digest. Each distinct transition nullifier therefore
/// identifies one credit even when many senders satisfy the same request.
///
/// # Errors
///
/// Returns an error for a reserved transition nullifier or request digest.
pub fn kagemusha_credit_id_v1(
    transition_nullifier: [u8; 32],
    request_digest: [u8; 32],
) -> Result<[u8; 32], KagemushaValidationErrorV1> {
    require_nonzero(
        "kagemusha.credit_id.transition_nullifier",
        transition_nullifier,
    )?;
    require_nonzero("kagemusha.credit_id.request_digest", request_digest)?;
    let mut hasher = Sha256::new();
    hasher.update(KAGEMUSHA_CREDIT_ID_DOMAIN_V1);
    hasher.update([0]);
    hasher.update(transition_nullifier);
    hasher.update(request_digest);
    Ok(hasher.finalize().into())
}

/// Derive the request-bound transfer commitment before encryption and proving.
///
/// The semantic transcript binds the exact signed request, sender predecessor
/// and successor, transition nullifier, and the
/// ID-independent ciphertext-opening commitment.
/// This never hashes ciphertext bytes, certificate bytes, or randomized proofs.
///
/// # Errors
///
/// Returns an error for an invalid request or reserved output field.
pub fn kagemusha_prepared_transfer_digest_v1(
    request: &KagemushaPaymentRequestV1,
    sender_before_commitment: [u8; 32],
    sender_after_commitment: [u8; 32],
    transition_nullifier: [u8; 32],
    ciphertext_commitment: [u8; 32],
) -> Result<[u8; 32], KagemushaValidationErrorV1> {
    request.validate_shape()?;
    for (field, value) in [
        (
            "kagemusha.prepared_transfer.sender_before",
            sender_before_commitment,
        ),
        (
            "kagemusha.prepared_transfer.sender_after",
            sender_after_commitment,
        ),
        (
            "kagemusha.prepared_transfer.transition_nullifier",
            transition_nullifier,
        ),
        (
            "kagemusha.prepared_transfer.ciphertext_commitment",
            ciphertext_commitment,
        ),
    ] {
        require_nonzero(field, value)?;
    }
    if sender_before_commitment == sender_after_commitment {
        return Err(invalid("kagemusha.prepared_transfer.state_commitments"));
    }
    let mut transcript = Vec::with_capacity(210);
    transcript.extend_from_slice(&KAGEMUSHA_WIRE_VERSION_V1.to_le_bytes());
    transcript.extend_from_slice(&request.canonical_digest()?);
    transcript.extend_from_slice(&request.amount.to_le_bytes());
    transcript.extend_from_slice(&sender_before_commitment);
    transcript.extend_from_slice(&sender_after_commitment);
    transcript.extend_from_slice(&transition_nullifier);
    transcript.extend_from_slice(&request.recipient_encryption_key);
    transcript.extend_from_slice(&ciphertext_commitment);
    Ok(digest_bytes(PREPARED_TRANSFER_DIGEST_DOMAIN, &transcript))
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

impl KagemushaPeerCreditContextV1 {
    /// Validate the exact pre-encryption request and sender transition projection.
    ///
    /// # Errors
    ///
    /// Returns an error for a reserved digest, invalid recipient key, or version.
    pub fn validate_shape(&self) -> Result<(), KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1 {
            return Err(invalid("kagemusha.peer_credit_context.version"));
        }
        if self.amount == 0 || self.sender_before_commitment == self.sender_after_commitment {
            return Err(invalid("kagemusha.peer_credit_context.amount_or_state"));
        }
        for (field, value) in [
            (
                "kagemusha.peer_credit_context.request_digest",
                self.request_digest,
            ),
            (
                "kagemusha.peer_credit_context.sender_before",
                self.sender_before_commitment,
            ),
            (
                "kagemusha.peer_credit_context.sender_after",
                self.sender_after_commitment,
            ),
            (
                "kagemusha.peer_credit_context.prepared_transfer_digest",
                self.prepared_transfer_digest,
            ),
        ] {
            require_nonzero(field, value)?;
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
    /// sender state transition and all lifecycle fields available before credit
    /// ID and ciphertext derivation.
    ///
    /// # Errors
    ///
    /// Returns an error for any invalid or substituted peer context.
    pub fn for_peer(
        output: &KagemushaPaymentOutputV1,
        request: &KagemushaPaymentRequestV1,
    ) -> Result<Self, KagemushaValidationErrorV1> {
        let context = output.peer_credit_context_against(request)?;
        Ok(Self {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            purpose: KagemushaEncryptedCreditPurposeV1::Peer,
            context_digest: context.canonical_digest()?,
            issuance_or_transition_commitment: output.ciphertext_commitment,
            credit_id: output.credit_id,
            amount: request.amount,
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
    digest_encoded(
        MINT_CREDIT_OPENING_COMMITMENT_DOMAIN,
        &mint_credit_opening_commitment_preimage_v1(
            network_id,
            asset,
            asset_incarnation,
            scale,
            liability_pool_id,
            amount,
            recipient,
            recipient_one_time_key,
            credit_commitment_opening,
        )?,
    )
}

/// Return the exact canonical bytes hashed by a mint-opening commitment.
///
/// This uses the same validated typed preimage as
/// [`kagemusha_mint_credit_opening_commitment_v1`], including canonical typed
/// account and asset identity digests. Its header, padding, nested incarnation
/// prefix, and CRC64 are retained unchanged; this is not a new wire codec.
///
/// # Errors
///
/// Returns an error for invalid inputs, encoding failure, or changed fixed layout.
#[allow(clippy::too_many_arguments)]
pub fn kagemusha_mint_credit_opening_commitment_preimage_v1(
    network_id: &NetworkId,
    asset: &AssetDefinitionId,
    asset_incarnation: AxtAssetIncarnationV1,
    scale: u32,
    liability_pool_id: [u8; 32],
    amount: u128,
    recipient: &AccountId,
    recipient_one_time_key: [u8; KAGEMUSHA_X25519_PUBLIC_KEY_BYTES_V1],
    credit_commitment_opening: [u8; 32],
) -> Result<
    [u8; KAGEMUSHA_MINT_CREDIT_OPENING_COMMITMENT_PREIMAGE_BYTES_V1],
    KagemushaValidationErrorV1,
> {
    fixed_canonical_preimage_bytes_v1(
        &mint_credit_opening_commitment_preimage_v1(
            network_id,
            asset,
            asset_incarnation,
            scale,
            liability_pool_id,
            amount,
            recipient,
            recipient_one_time_key,
            credit_commitment_opening,
        )?,
        "kagemusha.mint_credit_opening_commitment.preimage_layout",
    )
}

#[allow(clippy::too_many_arguments)]
fn mint_credit_opening_commitment_preimage_v1(
    network_id: &NetworkId,
    asset: &AssetDefinitionId,
    asset_incarnation: AxtAssetIncarnationV1,
    scale: u32,
    liability_pool_id: [u8; 32],
    amount: u128,
    recipient: &AccountId,
    recipient_one_time_key: [u8; KAGEMUSHA_X25519_PUBLIC_KEY_BYTES_V1],
    credit_commitment_opening: [u8; 32],
) -> Result<MintCreditOpeningCommitmentPreimageV1, KagemushaValidationErrorV1> {
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
    Ok(MintCreditOpeningCommitmentPreimageV1 {
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
    })
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
    digest_encoded(
        RECIPIENT_CREDENTIAL_COMMITMENT_DOMAIN,
        &recipient_credential_commitment_preimage_v1(
            operation_id,
            hardware_credential_id,
            recipient_binding_opening,
        )?,
    )
}

/// Return the exact canonical bytes hashed by the randomized recipient commitment.
///
/// The authoritative private preimage, input validation, and digest transcript
/// are unchanged. These bytes retain all header, field-prefix, and CRC64 bytes.
///
/// # Errors
///
/// Returns an error for reserved inputs, encoding failure, or changed fixed layout.
pub fn kagemusha_recipient_credential_commitment_preimage_v1(
    operation_id: [u8; 32],
    hardware_credential_id: [u8; 32],
    recipient_binding_opening: [u8; 32],
) -> Result<
    [u8; KAGEMUSHA_RECIPIENT_CREDENTIAL_COMMITMENT_PREIMAGE_BYTES_V1],
    KagemushaValidationErrorV1,
> {
    fixed_canonical_preimage_bytes_v1(
        &recipient_credential_commitment_preimage_v1(
            operation_id,
            hardware_credential_id,
            recipient_binding_opening,
        )?,
        "kagemusha.recipient_credential_commitment.preimage_layout",
    )
}

fn recipient_credential_commitment_preimage_v1(
    operation_id: [u8; 32],
    hardware_credential_id: [u8; 32],
    recipient_binding_opening: [u8; 32],
) -> Result<RecipientCredentialCommitmentPreimageV1, KagemushaValidationErrorV1> {
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
    Ok(RecipientCredentialCommitmentPreimageV1 {
        operation_id,
        hardware_credential_id,
        recipient_binding_opening,
    })
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

    /// Return the exact unchanged canonical Norito profile-ID preimage.
    ///
    /// The 40-byte header and fifteen compact-length-prefixed fields are retained,
    /// including the u32-LE platform discriminant, 65-byte governance key, and CRC64.
    /// This encodes the unsigned identity body without validating or authorizing
    /// the profile, so it can also be used before sealing its profile ID.
    ///
    /// # Errors
    ///
    /// Returns an error for encoding failure or an unexpected fixed V1 layout.
    pub fn canonical_id_preimage_bytes(
        &self,
    ) -> Result<[u8; KAGEMUSHA_HARDWARE_PROFILE_ID_PREIMAGE_BYTES_V1], KagemushaValidationErrorV1>
    {
        fixed_canonical_preimage_bytes_v1(
            &self.id_preimage(),
            "kagemusha.hardware_profile.id_preimage_layout",
        )
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
        Ok(digest_bytes(
            HARDWARE_CREDENTIAL_ID_DOMAIN,
            &self.canonical_id_preimage_bytes()?,
        ))
    }

    /// Return the exact unchanged canonical Norito credential-ID preimage.
    ///
    /// This is not a new wire codec. The 40-byte Norito header is followed by
    /// thirteen compact-length-prefixed fixed-width fields, with payload widths
    /// 2,32,32,32,32,8,32,32,8,65,32,8,8. The lane occupies bytes 185..217.
    /// The identity hashes the credential-ID domain, zero separator, u64-LE
    /// byte length, and these complete bytes, including the canonical header.
    /// A proof opening this preimage must bind its digest to the authenticated
    /// credential ID and the lane at this exact offset, not search for a value.
    ///
    /// # Errors
    ///
    /// Returns an error for an encoding failure or unexpected V1 codec layout.
    pub fn canonical_id_preimage_bytes(&self) -> Result<Vec<u8>, KagemushaValidationErrorV1> {
        let bytes = norito::encode_canonical(&self.id_preimage())?;
        let offset = KAGEMUSHA_HARDWARE_CREDENTIAL_ID_LANE_OFFSET_V1;
        if bytes.len() != KAGEMUSHA_HARDWARE_CREDENTIAL_ID_PREIMAGE_BYTES_V1
            || bytes.get(offset..offset + 32) != Some(self.lane_commitment.as_slice())
        {
            return Err(invalid("kagemusha.hardware_credential.id_preimage_layout"));
        }
        Ok(bytes)
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

/// Return the complete durable outbox reservation for a terminal operation.
///
/// The budget includes the candidate, sealed recovery material, certificate,
/// post-commit proof and final envelope; non-terminal operations have no outbox.
#[must_use]
pub const fn kagemusha_outbox_min_reserved_bytes_v1(
    operation_kind: KagemushaOperationKindV1,
) -> Option<u32> {
    match operation_kind {
        KagemushaOperationKindV1::SendSplit => Some(KAGEMUSHA_PAYMENT_OUTBOX_MIN_BYTES_V1),
        KagemushaOperationKindV1::RedeemSplit => Some(KAGEMUSHA_REDEMPTION_OUTBOX_MIN_BYTES_V1),
        _ => None,
    }
}

impl KagemushaCommitEvidenceV1 {
    /// Validate the hiding commitment for the selected qualified deadline source.
    ///
    /// This is a structural check only. A release-pinned proof must establish
    /// trusted time or monotonic-lease consumption before the commit deadline.
    ///
    /// # Errors
    ///
    /// Returns an error for a reserved zero evidence commitment.
    pub fn validate(&self) -> Result<(), KagemushaValidationErrorV1> {
        match self {
            Self::TrustedTime(evidence) => require_nonzero(
                "kagemusha.commit_evidence.time_evidence_commitment",
                evidence.time_evidence_commitment,
            ),
            Self::MonotonicLease(evidence) => require_nonzero(
                "kagemusha.commit_evidence.lease_evidence_commitment",
                evidence.lease_evidence_commitment,
            ),
        }
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
                    && self.receiver_lane_commitment != [0; 32]
                    && credit_fields.iter().all(|value| *value != [0; 32]) => {}
            KagemushaOperationKindV1::SendSplit => {
                return Err(invalid("kagemusha.lifecycle.payment_binding"));
            }
            KagemushaOperationKindV1::MintFold
                if self.request_id == [0; 32]
                    && self.receiver_lane_commitment == [0; 32]
                    && credit_fields.iter().all(|value| *value != [0; 32]) => {}
            KagemushaOperationKindV1::MintFold => {
                return Err(invalid("kagemusha.lifecycle.mint_binding"));
            }
            _ if [self.request_id, self.receiver_lane_commitment]
                .iter()
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

impl KagemushaOutboxReservationV1 {
    /// Validate operation, capacity, identity, and lifetime.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-terminal operation, insufficient capacity,
    /// reserved identity, or empty lifetime.
    pub fn validate(self) -> Result<(), KagemushaValidationErrorV1> {
        let minimum = kagemusha_outbox_min_reserved_bytes_v1(self.operation_kind)
            .ok_or_else(|| invalid("kagemusha.outbox_reservation.operation_kind"))?;
        if self.reservation_id == [0; 32]
            || self.reserved_outbox_bytes < minimum
            || self.issued_at_ms >= self.expires_at_ms
        {
            return Err(invalid("kagemusha.outbox_reservation"));
        }
        Ok(())
    }

    /// Return the hiding commitment proven by the final terminal proof.
    ///
    /// # Errors
    ///
    /// Returns an error when validation fails.
    pub fn canonical_commitment(self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate()?;
        Ok(digest_bytes(
            OUTBOX_RESERVATION_COMMITMENT_DOMAIN,
            &outbox_reservation_circuit_transcript_v1(self),
        ))
    }
}

impl KagemushaHardwareTerminalBodyV1 {
    /// Return the hiding commitment used by the terminal certificate.
    ///
    /// # Errors
    ///
    /// Returns an error for a reserved field, unsupported version, invalid
    /// commit evidence, or canonical encoding failure.
    pub fn canonical_commitment(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1 || self.policy_epoch == 0 {
            return Err(invalid("kagemusha.hardware_terminal_body.context"));
        }
        self.commit_evidence.validate()?;
        for (field, value) in [
            (
                "kagemusha.hardware_terminal_body.candidate_envelope_digest",
                self.candidate_envelope_digest,
            ),
            (
                "kagemusha.hardware_terminal_body.lifecycle_binding_digest",
                self.lifecycle_binding_digest,
            ),
            (
                "kagemusha.hardware_terminal_body.transition_nullifier",
                self.transition_nullifier,
            ),
            (
                "kagemusha.hardware_terminal_body.outbox_reservation_commitment",
                self.outbox_reservation_commitment,
            ),
            (
                "kagemusha.hardware_terminal_body.hardware_profile_id",
                self.hardware_profile_id,
            ),
            (
                "kagemusha.hardware_terminal_body.private_successor_commitment",
                self.private_successor_commitment,
            ),
            (
                "kagemusha.hardware_terminal_body.private_journal_commitment",
                self.private_journal_commitment,
            ),
            (
                "kagemusha.hardware_terminal_body.private_recovery_commitment",
                self.private_recovery_commitment,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        digest_encoded(HARDWARE_TERMINAL_BODY_COMMITMENT_DOMAIN, self)
    }
}

impl KagemushaCommitCertificateV1 {
    /// Validate public shape and self-identity without authorizing hardware state.
    ///
    /// The native proof must bind the complete sender lifecycle and authenticate
    /// the terminal commitment against a qualified profile.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed evidence, identity, reserved fields or size.
    pub fn validate_shape(&self) -> Result<(), KagemushaValidationErrorV1> {
        self.commit_evidence.validate()?;
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || self.policy_epoch == 0
            || [
                self.candidate_envelope_digest,
                self.lifecycle_binding_digest,
                self.transition_nullifier,
                self.outbox_reservation_commitment,
                self.hardware_profile_id,
                self.hardware_terminal_commitment,
            ]
            .contains(&[0; 32])
            || self.certificate_id != self.expected_certificate_id()?
        {
            return Err(invalid("kagemusha.commit_certificate.shape"));
        }
        require_encoded_size(self, KAGEMUSHA_COMMIT_CERTIFICATE_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Digest the exact public certificate transcript after structural validation.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed certificate shape or self-identity.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate_shape()?;
        Ok(digest_bytes(
            COMMIT_CERTIFICATE_DIGEST_DOMAIN,
            &commit_certificate_circuit_transcript_v1(self),
        ))
    }
    /// Compute the terminal certificate identity without self-reference.
    ///
    /// # Errors
    ///
    /// Returns an error only if its fixed transcript cannot be constructed.
    pub fn expected_certificate_id(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        Ok(digest_bytes(
            COMMIT_CERTIFICATE_ID_DOMAIN,
            &commit_certificate_id_circuit_transcript_v1(self),
        ))
    }

    /// Populate the canonical certificate identity.
    ///
    /// # Errors
    ///
    /// Returns an error if identity derivation fails.
    pub fn seal_certificate_id(mut self) -> Result<Self, KagemushaValidationErrorV1> {
        self.certificate_id = self.expected_certificate_id()?;
        Ok(self)
    }

    /// Bind a self-free terminal body and then derive the certificate identity.
    ///
    /// # Errors
    ///
    /// Returns an error when the body is invalid or differs from the public certificate fields.
    pub fn seal_with_terminal_body(
        mut self,
        body: &KagemushaHardwareTerminalBodyV1,
    ) -> Result<Self, KagemushaValidationErrorV1> {
        if body.version != self.version
            || body.candidate_envelope_digest != self.candidate_envelope_digest
            || body.lifecycle_binding_digest != self.lifecycle_binding_digest
            || body.transition_nullifier != self.transition_nullifier
            || body.outbox_reservation_commitment != self.outbox_reservation_commitment
            || body.commit_evidence != self.commit_evidence
            || body.hardware_profile_id != self.hardware_profile_id
            || body.policy_epoch != self.policy_epoch
        {
            return Err(invalid("kagemusha.hardware_terminal_body.binding"));
        }
        self.hardware_terminal_commitment = body.canonical_commitment()?;
        self.seal_certificate_id()
    }

    /// Validate this recoverable certificate against the exact lifecycle.
    ///
    /// # Errors
    ///
    /// Returns an error for a substituted lifecycle, evidence, nullifier,
    /// terminal body, certificate identity, or size.
    pub fn validate_against(
        &self,
        lifecycle: &KagemushaLifecycleBindingV1,
        expected_evidence: KagemushaCommitEvidenceV1,
        expected_nullifier: [u8; 32],
    ) -> Result<(), KagemushaValidationErrorV1> {
        lifecycle.validate()?;
        self.commit_evidence.validate()?;
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || self.candidate_envelope_digest == [0; 32]
            || self.lifecycle_binding_digest != lifecycle.canonical_digest()?
            || self.transition_nullifier != expected_nullifier
            || self.transition_nullifier == [0; 32]
            || self.outbox_reservation_commitment == [0; 32]
            || self.commit_evidence != expected_evidence
            || self.hardware_profile_id != lifecycle.hardware_profile_id
            || self.policy_epoch != lifecycle.policy_epoch
            || self.hardware_terminal_commitment == [0; 32]
            || self.certificate_id != self.expected_certificate_id()?
        {
            return Err(invalid("kagemusha.commit_certificate.binding"));
        }
        require_encoded_size(self, KAGEMUSHA_COMMIT_CERTIFICATE_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Return the fixed-width certificate digest constrained by both final-proof parities.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid lifecycle, evidence, nullifier, or certificate binding.
    pub fn canonical_digest_against(
        &self,
        lifecycle: &KagemushaLifecycleBindingV1,
        expected_evidence: KagemushaCommitEvidenceV1,
        expected_nullifier: [u8; 32],
    ) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate_against(lifecycle, expected_evidence, expected_nullifier)?;
        Ok(digest_bytes(
            COMMIT_CERTIFICATE_DIGEST_DOMAIN,
            &commit_certificate_circuit_transcript_v1(self),
        ))
    }

    /// Decode and validate one exact bounded terminal certificate.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, oversized, non-canonical, or substituted input.
    pub fn decode_canonical_exact_against(
        bytes: &[u8],
        lifecycle: &KagemushaLifecycleBindingV1,
        expected_evidence: KagemushaCommitEvidenceV1,
        expected_nullifier: [u8; 32],
    ) -> Result<Self, KagemushaValidationErrorV1> {
        let certificate: Self =
            decode_bounded_canonical(bytes, KAGEMUSHA_COMMIT_CERTIFICATE_MAX_BYTES_V1)?;
        certificate.validate_against(lifecycle, expected_evidence, expected_nullifier)?;
        Ok(certificate)
    }
}

impl KagemushaPaymentProofV1 {
    /// Validate bounded proof material and exact terminal bindings.
    ///
    /// Cryptographic verification remains the native core's responsibility;
    /// this method enforces the closed wire shape and public digest equality.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty, oversized, aliased, or substituted field.
    pub fn validate_shape_against(
        &self,
        semantic_digest: [u8; 32],
        candidate_envelope_digest: [u8; 32],
        commit_certificate_digest: [u8; 32],
    ) -> Result<(), KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || self.eq_protocol_digest == [0; 32]
            || self.ep_protocol_digest == [0; 32]
            || self.eq_protocol_digest == self.ep_protocol_digest
            || self.semantic_digest != semantic_digest
            || self.candidate_envelope_digest != candidate_envelope_digest
            || self.commit_certificate_digest != commit_certificate_digest
            || self.eq_deferred_audit == [0; 32]
            || self.ep_deferred_audit == [0; 32]
            || self.eq_deferred_audit == self.ep_deferred_audit
            || self.eq_proof.is_empty()
            || self.ep_proof.is_empty()
            || self.eq_proof.len() > KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1
            || self.ep_proof.len() > KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1
            || self.eq_proof.len() + self.ep_proof.len() > KAGEMUSHA_CURRENT_PROOFS_MAX_BYTES_V1
            || self.eq_history.len() != KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1
            || self.ep_history.len() != KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1
            || self.eq_history.iter().all(|byte| *byte == 0)
            || self.ep_history.iter().all(|byte| *byte == 0)
            || self.eq_history == self.ep_history
        {
            return Err(invalid("kagemusha.payment_proof.binding"));
        }
        require_encoded_size(self, KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Decode and validate one exact bounded payment proof.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, oversized, non-canonical, or substituted input.
    pub fn decode_canonical_exact_against(
        bytes: &[u8],
        semantic_digest: [u8; 32],
        candidate_envelope_digest: [u8; 32],
        commit_certificate_digest: [u8; 32],
    ) -> Result<Self, KagemushaValidationErrorV1> {
        let proof: Self = decode_bounded_canonical(bytes, KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1)?;
        proof.validate_shape_against(
            semantic_digest,
            candidate_envelope_digest,
            commit_certificate_digest,
        )?;
        Ok(proof)
    }
}

impl KagemushaRedemptionProofV1 {
    /// Validate bounded proof material and exact terminal bindings.
    ///
    /// Cryptographic verification remains the native core's responsibility;
    /// this method enforces the closed wire shape and public digest equality.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty, oversized, aliased, or substituted field.
    pub fn validate_shape_against(
        &self,
        semantic_digest: [u8; 32],
        candidate_envelope_digest: [u8; 32],
        commit_certificate_digest: [u8; 32],
    ) -> Result<(), KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || self.eq_protocol_digest == [0; 32]
            || self.ep_protocol_digest == [0; 32]
            || self.eq_protocol_digest == self.ep_protocol_digest
            || self.semantic_digest != semantic_digest
            || self.candidate_envelope_digest != candidate_envelope_digest
            || self.commit_certificate_digest != commit_certificate_digest
            || self.eq_deferred_audit == [0; 32]
            || self.ep_deferred_audit == [0; 32]
            || self.eq_deferred_audit == self.ep_deferred_audit
            || self.eq_proof.is_empty()
            || self.ep_proof.is_empty()
            || self.eq_proof.len() > KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1
            || self.ep_proof.len() > KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1
            || self.eq_proof.len() + self.ep_proof.len() > KAGEMUSHA_CURRENT_PROOFS_MAX_BYTES_V1
            || self.eq_history.len() != KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1
            || self.ep_history.len() != KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1
            || self.eq_history.iter().all(|byte| *byte == 0)
            || self.ep_history.iter().all(|byte| *byte == 0)
            || self.eq_history == self.ep_history
        {
            return Err(invalid("kagemusha.redemption_proof.binding"));
        }
        require_encoded_size(self, KAGEMUSHA_REDEMPTION_PROOF_MAX_BYTES_V1 as usize)?;
        Ok(())
    }

    /// Decode and validate one exact bounded redemption proof.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, oversized, non-canonical, or substituted input.
    pub fn decode_canonical_exact_against(
        bytes: &[u8],
        semantic_digest: [u8; 32],
        candidate_envelope_digest: [u8; 32],
        commit_certificate_digest: [u8; 32],
    ) -> Result<Self, KagemushaValidationErrorV1> {
        let proof: Self =
            decode_bounded_canonical(bytes, KAGEMUSHA_REDEMPTION_PROOF_MAX_BYTES_V1 as usize)?;
        proof.validate_shape_against(
            semantic_digest,
            candidate_envelope_digest,
            commit_certificate_digest,
        )?;
        Ok(proof)
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
/// binds one exact amount, receiver encryption key, and compact hardware credential.
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
    amount: u128,
    recipient_encryption_key: [u8; KAGEMUSHA_X25519_PUBLIC_KEY_BYTES_V1],
    hardware_credential_id: [u8; 32],
    request_id: [u8; 32],
    issued_at_ms: u64,
    expires_at_ms: u64,
) -> Result<Vec<u8>, KagemushaValidationErrorV1> {
    let mut bytes = REQUEST_SIGNING_DOMAIN.to_vec();
    bytes.push(0);
    bytes.extend_from_slice(&version.to_le_bytes());
    bytes.extend_from_slice(&release_id);
    bytes.extend_from_slice(network_id.as_bytes());
    bytes.extend_from_slice(&kagemusha_asset_identity_digest_v1(asset)?);
    bytes.extend_from_slice(asset_incarnation.as_bytes());
    bytes.extend_from_slice(&scale.to_le_bytes());
    bytes.extend_from_slice(&liability_pool_id);
    bytes.extend_from_slice(&digest_encoded(ACCOUNT_IDENTITY_DIGEST_DOMAIN, recipient)?);
    bytes.extend_from_slice(&amount.to_le_bytes());
    bytes.extend_from_slice(&recipient_encryption_key);
    bytes.extend_from_slice(&hardware_credential_id);
    bytes.extend_from_slice(&request_id);
    bytes.extend_from_slice(&issued_at_ms.to_le_bytes());
    bytes.extend_from_slice(&expires_at_ms.to_le_bytes());
    Ok(bytes)
}

impl KagemushaPaymentRequestV1 {
    /// Return the fixed signed semantic transcript, distinct from Norito wire encoding.
    ///
    /// Layout: version:u16 LE | release:32 | network:32 | normalized asset:32 |
    /// incarnation:32 | scale:u32 LE | pool:32 | normalized account:32 |
    /// amount:u128 LE | receiver encryption key:32 | credential ID:32 |
    /// request ID:32 | issued:u64 LE |
    /// expires:u64 LE | signature:64. Identity normalization uses the existing
    /// typed Norito asset/account digest domains; no typed identity is discarded on wire.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid amount/key or unencodable normalized identity.
    pub fn circuit_transcript_bytes(&self) -> Result<Vec<u8>, KagemushaValidationErrorV1> {
        let signed = self.canonical_signing_bytes()?;
        let mut bytes = signed[REQUEST_SIGNING_DOMAIN.len() + 1..].to_vec();
        bytes.extend_from_slice(self.signature.as_raw_bytes());
        Ok(bytes)
    }

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
            self.amount,
            self.recipient_encryption_key,
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
            return Err(invalid("kagemusha.request.liability_pool_id"));
        }
        self.asset_incarnation
            .validate()
            .map_err(|_| invalid("kagemusha.request.asset_incarnation"))?;
        if self.amount == 0 {
            return Err(invalid("kagemusha.request.amount"));
        }
        require_valid_x25519_public_key(
            "kagemusha.request.recipient_encryption_key",
            self.recipient_encryption_key,
        )?;
        self.hardware_credential.validate_shape()?;
        if self.hardware_credential.network_id != self.network_id
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
        Ok(digest_bytes(
            REQUEST_DIGEST_DOMAIN,
            &self.circuit_transcript_bytes()?,
        ))
    }
}

impl KagemushaPaymentOutputV1 {
    /// Return the exact fixed semantic transcript, not Norito wire bytes.
    #[must_use]
    pub fn circuit_transcript_bytes(&self) -> [u8; 254] {
        let mut bytes = [0; 254];
        bytes[..2].copy_from_slice(&self.version.to_le_bytes());
        bytes[2..34].copy_from_slice(&self.request_digest);
        bytes[34..50].copy_from_slice(&self.amount.to_le_bytes());
        for (index, digest) in [
            self.sender_before_commitment,
            self.sender_after_commitment,
            self.transition_nullifier,
            self.credit_id,
            self.ciphertext_commitment,
        ]
        .iter()
        .enumerate()
        {
            bytes[50 + index * 32..82 + index * 32].copy_from_slice(digest);
        }
        bytes[210..246]
            .copy_from_slice(&commit_evidence_circuit_transcript_v1(self.commit_evidence));
        bytes[246..].copy_from_slice(&self.committed_at_ms.to_le_bytes());
        bytes
    }

    /// Digest the fixed output transcript independently of proof and certificate bytes.
    ///
    /// # Errors
    ///
    /// Returns an error for reserved fields, an unchanged state, or invalid evidence.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || self.amount == 0
            || self.committed_at_ms == 0
            || self.sender_before_commitment == self.sender_after_commitment
            || [
                self.request_digest,
                self.sender_before_commitment,
                self.sender_after_commitment,
                self.transition_nullifier,
                self.credit_id,
                self.ciphertext_commitment,
            ]
            .contains(&[0; 32])
        {
            return Err(invalid("kagemusha.payment_output.shape"));
        }
        self.commit_evidence.validate()?;
        Ok(digest_bytes(
            STATEMENT_DIGEST_DOMAIN,
            &self.circuit_transcript_bytes(),
        ))
    }

    /// Compute the request-bound credit ID.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid request or zero nullifier.
    pub fn expected_credit_id_against(
        &self,
        request: &KagemushaPaymentRequestV1,
    ) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        kagemusha_credit_id_v1(self.transition_nullifier, request.canonical_digest()?)
    }

    /// Populate the stable credit ID before encryption.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid request context.
    pub fn seal_credit_id_against(
        mut self,
        request: &KagemushaPaymentRequestV1,
    ) -> Result<Self, KagemushaValidationErrorV1> {
        self.credit_id = self.expected_credit_id_against(request)?;
        Ok(self)
    }

    /// Build the pre-encryption context directly from the signed request.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid request, output, or receiver binding.
    pub fn peer_credit_context_against(
        &self,
        request: &KagemushaPaymentRequestV1,
    ) -> Result<KagemushaPeerCreditContextV1, KagemushaValidationErrorV1> {
        request.validate_shape()?;
        self.canonical_digest()?;
        let request_digest = request.canonical_digest()?;
        if self.request_digest != request_digest
            || self.amount != request.amount
            || self.credit_id != self.expected_credit_id_against(request)?
            || self.committed_at_ms < request.issued_at_ms
            || self.committed_at_ms >= request.expires_at_ms
        {
            return Err(invalid("kagemusha.peer_credit_context.binding"));
        }
        let context = KagemushaPeerCreditContextV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            request_digest,
            amount: self.amount,
            sender_before_commitment: self.sender_before_commitment,
            sender_after_commitment: self.sender_after_commitment,
            prepared_transfer_digest: kagemusha_prepared_transfer_digest_v1(
                request,
                self.sender_before_commitment,
                self.sender_after_commitment,
                self.transition_nullifier,
                self.ciphertext_commitment,
            )?,
            recipient_encryption_key: request.recipient_encryption_key,
        };
        context.validate_shape()?;
        Ok(context)
    }

    /// Validate direct request, receiver, amount, deadline, and output bindings.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid or substituted context.
    pub fn validate_shape_against(
        &self,
        request: &KagemushaPaymentRequestV1,
    ) -> Result<(), KagemushaValidationErrorV1> {
        self.peer_credit_context_against(request)?;
        Ok(())
    }

    /// Digest the output after exact request validation.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid or substituted context.
    pub fn canonical_digest_against(
        &self,
        request: &KagemushaPaymentRequestV1,
    ) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate_shape_against(request)?;
        self.canonical_digest()
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

/// Derive the payment body digest from its two exact component digests.
///
/// Hashes body-domain | zero | u64-LE(64) | output digest | ciphertext digest.
/// It excludes the final proof and commit certificate.
#[must_use]
pub fn kagemusha_payment_body_digest_from_digests_v1(
    output_digest: [u8; 32],
    encrypted_credit_digest: [u8; 32],
) -> [u8; 32] {
    let mut transcript = [0; 64];
    transcript[..32].copy_from_slice(&output_digest);
    transcript[32..].copy_from_slice(&encrypted_credit_digest);
    digest_bytes(b"iroha:kagemusha:v1:payment-body", &transcript)
}

/// Derive the body before hardware commit, without placeholder proof bytes.
///
/// Candidate and final proof semantic digests are both this digest. The final
/// proof independently binds the certificate. Exact context and profile
/// validation remain separate mandatory checks.
///
/// # Errors
///
/// Returns an error for a malformed output or encrypted-credit envelope.
pub fn kagemusha_payment_body_digest_v1(
    output: &KagemushaPaymentOutputV1,
    encrypted_credit: &[u8],
) -> Result<[u8; 32], KagemushaValidationErrorV1> {
    KagemushaEncryptedCreditEnvelopeV1::decode_canonical_shape_exact(encrypted_credit)?;
    Ok(kagemusha_payment_body_digest_from_digests_v1(
        output.canonical_digest()?,
        kagemusha_ciphertext_digest_v1(encrypted_credit),
    ))
}

impl KagemushaPaymentV1 {
    /// Encode a shape-validated committed payment as canonical text.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid context, bindings, framing, or size.
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

    /// Decode bounded canonical text against the exact request.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid context, bindings, framing, or size.
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

    /// Decode the complete bounded canonical committed-payment envelope.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid context, bindings, framing, or size.
    pub fn decode_canonical_shape_exact_against(
        bytes: &[u8],
        request: &KagemushaPaymentRequestV1,
    ) -> Result<Self, KagemushaValidationErrorV1> {
        let payment: Self = decode_bounded_canonical(bytes, KAGEMUSHA_PAYMENT_MAX_BYTES_V1)?;
        payment.validate_shape_against(request)?;
        Ok(payment)
    }

    /// Return the proof-independent body digest after exact output validation.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed or substituted request, key, or output.
    pub fn body_digest_against(
        &self,
        request: &KagemushaPaymentRequestV1,
    ) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1 || self.output.version != self.version {
            return Err(invalid("kagemusha.payment.version"));
        }
        self.output.validate_shape_against(request)?;
        KagemushaEncryptedCreditEnvelopeV1::decode_canonical_shape_exact_against_recipient_key(
            &self.encrypted_credit,
            request.recipient_encryption_key,
        )?;
        kagemusha_payment_body_digest_v1(&self.output, &self.encrypted_credit)
    }

    /// Validate exact wire bindings without claiming cryptographic proof validity.
    ///
    /// Native verification must authenticate the release/profile and recursively
    /// prove the hidden sender transition and exact hardware certificate.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid certificate, proof, evidence, output, or size.
    pub fn validate_shape_against(
        &self,
        request: &KagemushaPaymentRequestV1,
    ) -> Result<(), KagemushaValidationErrorV1> {
        let body_digest = self.body_digest_against(request)?;
        self.commit_certificate.validate_shape()?;
        if self.commit_certificate.transition_nullifier != self.output.transition_nullifier
            || self.commit_certificate.commit_evidence != self.output.commit_evidence
        {
            return Err(invalid("kagemusha.payment.commit_certificate"));
        }
        self.proof.validate_shape_against(
            body_digest,
            self.commit_certificate.candidate_envelope_digest,
            self.commit_certificate.canonical_digest()?,
        )?;
        require_encoded_size(self, KAGEMUSHA_PAYMENT_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Return the complete proof-bearing envelope digest after finalization.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid envelope or contextual binding.
    pub fn canonical_digest_against(
        &self,
        request: &KagemushaPaymentRequestV1,
    ) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        self.validate_shape_against(request)?;
        digest_encoded(PAYMENT_DIGEST_DOMAIN, self)
    }

    /// Return the unlinkable transition nullifier used for conflict detection.
    ///
    /// # Errors
    ///
    /// Returns an error for the reserved zero nullifier.
    pub fn sender_conflict_key(&self) -> Result<[u8; 32], KagemushaValidationErrorV1> {
        require_nonzero(
            "kagemusha.payment.transition_nullifier",
            self.output.transition_nullifier,
        )?;
        Ok(self.output.transition_nullifier)
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
    /// Encode this acknowledgement as canonical unpadded `kgm1:` base64url text.
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
    /// Returns an error for invalid context, size, prefix, or canonical bytes.
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

    /// Decode and validate one bounded durable-inbox acknowledgement.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized, malformed, non-canonical, or invalid input.
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

    /// Validate request, payment, credit, durable receipt, and receiver signature bindings.
    ///
    /// # Errors
    ///
    /// Returns an error when any receipt, identity, signature, or size binding fails.
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
            || self.inbox_receipt.credit_id != payment.output.credit_id
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

/// Validate the complete three-message KAGEMUSHA exchange and return its raw size.
///
/// Distinct payments may independently bind the same signed request. This
/// validator imposes no request-local payment count or cumulative amount state.
///
/// # Errors
///
/// Returns an error for any invalid binding or aggregate raw/text size overrun.
pub fn validate_kagemusha_complete_exchange_shape_v1(
    request: &KagemushaPaymentRequestV1,
    payment: &KagemushaPaymentV1,
    acknowledgement: &KagemushaAcknowledgementV1,
) -> Result<usize, KagemushaValidationErrorV1> {
    request.validate_shape()?;
    payment.validate_shape_against(request)?;
    acknowledgement.validate_shape_against(request, payment)?;
    let lengths = [
        require_encoded_size(request, KAGEMUSHA_PAYMENT_REQUEST_MAX_BYTES_V1)?,
        require_encoded_size(payment, KAGEMUSHA_PAYMENT_MAX_BYTES_V1)?,
        require_encoded_size(acknowledgement, KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1)?,
    ];
    let raw = lengths.iter().sum::<usize>();
    if raw > KAGEMUSHA_COMPLETE_EXCHANGE_MAX_BYTES_V1 {
        return Err(KagemushaValidationErrorV1::EncodedSizeExceeded {
            actual: raw,
            max: KAGEMUSHA_COMPLETE_EXCHANGE_MAX_BYTES_V1,
        });
    }
    let text = lengths
        .iter()
        .map(|length| KAGEMUSHA_TEXT_PREFIX_V1.len() + unpadded_base64url_len(*length))
        .sum::<usize>();
    if text > KAGEMUSHA_COMPLETE_TEXT_EXCHANGE_MAX_BYTES_V1 {
        return Err(KagemushaValidationErrorV1::EncodedSizeExceeded {
            actual: text,
            max: KAGEMUSHA_COMPLETE_TEXT_EXCHANGE_MAX_BYTES_V1,
        });
    }
    Ok(raw)
}

fn canonical_compact_length_width_v1(mut length: usize) -> usize {
    let mut width = 1;
    while length >= 128 {
        length >>= 7;
        width += 1;
    }
    width
}

fn canonical_frame_overhead_v1<T>() -> usize {
    let header = norito::core::Header::SIZE;
    let alignment = norito::core::archived_payload_align::<T>();
    header + (alignment - header % alignment) % alignment
}

fn mint_authorization_encoded_length_v1(
    context_payload_bytes: usize,
    paired_proof_payload_bytes: usize,
) -> Option<usize> {
    let statement_payload_bytes = 102_usize
        .checked_add(context_payload_bytes)?
        .checked_add(canonical_compact_length_width_v1(context_payload_bytes))?;
    canonical_frame_overhead_v1::<KagemushaMintAuthorizationV1>()
        .checked_add(3)?
        .checked_add(statement_payload_bytes)?
        .checked_add(canonical_compact_length_width_v1(statement_payload_bytes))?
        .checked_add(paired_proof_payload_bytes)?
        .checked_add(canonical_compact_length_width_v1(
            paired_proof_payload_bytes,
        ))
}

/// Derive a mint authorization's maximum complete canonical context-frame size.
///
/// The two inputs must be the **exact** current-proof byte widths authenticated
/// by the caller's selected release/circuit shape. A maximum proof allowance is
/// not a minimum proof length and must not be substituted here: doing so could
/// incorrectly exclude valid accounts. This arithmetic helper cannot establish
/// that the caller's proof bytes or release actually have the supplied widths.
///
/// The result includes the context's own Norito header and alignment padding.
/// It is derived only from those exact proof widths, both existing 544-byte
/// histories, canonical framing, and the existing 7,936-byte full-authorization
/// cap. It does not impose an account, controller, or multisig-member-count cap.
/// Full authorization shape checks and cryptographic verification remain required.
///
/// # Errors
///
/// Returns an error for empty/oversized proof widths, arithmetic overflow, or
/// proof framing that cannot fit the existing authorization and paired-proof caps.
pub fn kagemusha_mint_authorization_context_capacity_v1(
    eq_current_proof_bytes: usize,
    ep_current_proof_bytes: usize,
) -> Result<usize, KagemushaValidationErrorV1> {
    let field = "kagemusha.mint_authorization.context_capacity";
    if eq_current_proof_bytes == 0
        || ep_current_proof_bytes == 0
        || eq_current_proof_bytes > KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1
        || ep_current_proof_bytes > KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1
    {
        return Err(invalid(field));
    }
    let proof_bytes = eq_current_proof_bytes
        .checked_add(ep_current_proof_bytes)
        .filter(|total| *total <= KAGEMUSHA_CURRENT_PROOFS_MAX_BYTES_V1)
        .ok_or_else(|| invalid(field))?;
    let eq_payload = eq_current_proof_bytes
        .checked_add(8)
        .ok_or_else(|| invalid(field))?;
    let ep_payload = ep_current_proof_bytes
        .checked_add(8)
        .ok_or_else(|| invalid(field))?;
    // Version, seven digests, two exact history vectors, and both proof vector
    // counts occupy 1,358 payload bytes before the proof bytes and their prefixes.
    let paired_payload = 1_358_usize
        .checked_add(proof_bytes)
        .and_then(|length| length.checked_add(canonical_compact_length_width_v1(eq_payload)))
        .and_then(|length| length.checked_add(canonical_compact_length_width_v1(ep_payload)))
        .ok_or_else(|| invalid(field))?;
    let paired_length = canonical_frame_overhead_v1::<KagemushaPairedProofV1>()
        .checked_add(paired_payload)
        .ok_or_else(|| invalid(field))?;
    if paired_length > KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1
        || mint_authorization_encoded_length_v1(0, paired_payload)
            .is_none_or(|length| length > KAGEMUSHA_MINT_AUTHORIZATION_MAX_BYTES_V1)
    {
        return Err(invalid(field));
    }
    // Monotone integer search handles every compact-length-prefix boundary;
    // no assumption about the shape or count of account controllers is needed.
    let mut low = 0;
    let mut high = KAGEMUSHA_MINT_AUTHORIZATION_MAX_BYTES_V1;
    while low < high {
        let candidate = low + (high - low).div_ceil(2);
        if mint_authorization_encoded_length_v1(candidate, paired_payload)
            .is_some_and(|length| length <= KAGEMUSHA_MINT_AUTHORIZATION_MAX_BYTES_V1)
        {
            low = candidate;
        } else {
            high = candidate - 1;
        }
    }
    canonical_frame_overhead_v1::<KagemushaMintAuthorizationContextV1>()
        .checked_add(low)
        .ok_or_else(|| invalid(field))
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
            amount: self.amount,
            beneficiary: self.beneficiary.clone(),
            redemption_commitment: self.redemption_commitment,
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
        ] {
            require_nonzero(field, value)?;
        }
        if self.amount == 0
            || self.lifecycle.operation_kind != KagemushaOperationKindV1::RedeemSplit
            || self.terminal_nullifier == self.redemption_commitment
            || self.terminal_nullifier == self.redemption_id
            || self.redemption_commitment == self.redemption_id
        {
            return Err(invalid("kagemusha.redemption_statement.operation"));
        }
        self.commit_evidence.validate()?;
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

    /// Validate terminal state consumption, certificate, and redemption-proof binding.
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
        self.commit_certificate.validate_against(
            &self.statement.lifecycle,
            self.statement.commit_evidence,
            self.statement.terminal_nullifier,
        )?;
        let certificate_digest = self.commit_certificate.canonical_digest_against(
            &self.statement.lifecycle,
            self.statement.commit_evidence,
            self.statement.terminal_nullifier,
        )?;
        self.proof.validate_shape_against(
            self.statement.canonical_digest()?,
            self.commit_certificate.candidate_envelope_digest,
            certificate_digest,
        )?;
        require_nonzero(
            "kagemusha.redemption_voucher.artifact_manifest_digest",
            self.artifact_manifest_digest,
        )?;
        require_encoded_size(self, KAGEMUSHA_REDEMPTION_VOUCHER_MAX_BYTES_V1)?;
        Ok(())
    }
}
