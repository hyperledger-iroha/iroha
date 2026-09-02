//! Canonical first-release wire contract for hardware-guarded offline cash.
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

/// Version carried by every clean-slate Offline Cash wire value.
pub const OFFLINE_CASH_WIRE_VERSION_V1: u16 = 1;
/// Version of the secure-device lane and journal lifecycle contract.
pub const OFFLINE_CASH_DEVICE_LIFECYCLE_VERSION_V1: u16 = 1;
/// Device capability that commits sender state before exposing a payment.
pub const OFFLINE_CASH_HANDOFF_CAPABILITY_V1: &str = "cash_handoff_v1";
/// Text transport discriminator for canonical unpadded base64url messages.
pub const OFFLINE_CASH_TEXT_PREFIX_V1: &str = "oc1:";
/// Maximum authoritative asset scale represented by Offline Cash V1.
pub const OFFLINE_CASH_ASSET_SCALE_MAX_V1: u32 = 28;
/// Maximum lifetime of a signed payment request in Unix milliseconds.
pub const OFFLINE_CASH_REQUEST_MAX_TTL_MS_V1: u64 = 5 * 60 * 1_000;
/// Exact canonical uncompressed SEC1 P-256 public-key bytes.
pub const OFFLINE_CASH_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1: usize = 65;
/// Exact canonical fixed-width P-256 ECDSA signature bytes.
pub const OFFLINE_CASH_DEVICE_SIGNATURE_BYTES_V1: usize = 64;
/// Maximum canonical aggregate-state metadata bytes.
pub const OFFLINE_CASH_AGGREGATE_STATE_MAX_BYTES_V1: usize = 768;
/// Maximum canonical receiver-request bytes.
pub const OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1: usize = 1_024;
/// Maximum canonical sender-response bytes.
pub const OFFLINE_CASH_PAYMENT_MAX_BYTES_V1: usize = 7_936;
/// Maximum canonical receiver-acknowledgement bytes.
pub const OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1: usize = 512;
/// Maximum canonical top-up mint-credit bytes.
pub const OFFLINE_CASH_MINT_CREDIT_MAX_BYTES_V1: usize = 7_936;
/// Maximum canonical pre-debit recipient mint-authorization bytes.
pub const OFFLINE_CASH_MINT_AUTHORIZATION_MAX_BYTES_V1: usize = 7_936;
/// Maximum canonical redemption-voucher bytes.
pub const OFFLINE_CASH_REDEMPTION_VOUCHER_MAX_BYTES_V1: usize = 7_936;
/// Maximum complete `oc1:` text request bytes.
pub const OFFLINE_CASH_PAYMENT_REQUEST_TEXT_MAX_BYTES_V1: usize = OFFLINE_CASH_TEXT_PREFIX_V1.len()
    + unpadded_base64url_len(OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1);
/// Maximum complete `oc1:` text payment bytes.
pub const OFFLINE_CASH_PAYMENT_TEXT_MAX_BYTES_V1: usize =
    OFFLINE_CASH_TEXT_PREFIX_V1.len() + unpadded_base64url_len(OFFLINE_CASH_PAYMENT_MAX_BYTES_V1);
/// Maximum complete `oc1:` text acknowledgement bytes.
pub const OFFLINE_CASH_ACKNOWLEDGEMENT_TEXT_MAX_BYTES_V1: usize = OFFLINE_CASH_TEXT_PREFIX_V1.len()
    + unpadded_base64url_len(OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1);
/// Maximum complete `oc1:` text mint-credit bytes.
pub const OFFLINE_CASH_MINT_CREDIT_TEXT_MAX_BYTES_V1: usize = OFFLINE_CASH_TEXT_PREFIX_V1.len()
    + unpadded_base64url_len(OFFLINE_CASH_MINT_CREDIT_MAX_BYTES_V1);
/// Maximum complete `oc1:` text recipient mint-authorization bytes.
pub const OFFLINE_CASH_MINT_AUTHORIZATION_TEXT_MAX_BYTES_V1: usize = OFFLINE_CASH_TEXT_PREFIX_V1
    .len()
    + unpadded_base64url_len(OFFLINE_CASH_MINT_AUTHORIZATION_MAX_BYTES_V1);
/// Maximum complete `oc1:` text redemption-voucher bytes.
pub const OFFLINE_CASH_REDEMPTION_VOUCHER_TEXT_MAX_BYTES_V1: usize = OFFLINE_CASH_TEXT_PREFIX_V1
    .len()
    + unpadded_base64url_len(OFFLINE_CASH_REDEMPTION_VOUCHER_MAX_BYTES_V1);
/// Qualification target for the terminal request/payment/ack delivery trio.
pub const OFFLINE_CASH_SESSION_TARGET_BYTES_V1: usize = 8_960;
/// Absolute raw limit for the terminal request/payment/ack delivery trio.
pub const OFFLINE_CASH_SESSION_MAX_BYTES_V1: usize = 9_211;
/// Absolute text limit for the terminal request/payment/ack delivery trio.
pub const OFFLINE_CASH_TEXT_SESSION_MAX_BYTES_V1: usize = 12_288;
/// Qualification target for the two current recursive proofs.
pub const OFFLINE_CASH_PAIRED_PROOF_TARGET_BYTES_V1: usize = 6_144;
/// Absolute canonical byte limit for the complete paired-proof value.
pub const OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1: usize = 6_528;
/// Maximum combined bytes in the two current recursive-proof components.
///
/// The complete encoded-value cap remains authoritative and also accounts for
/// both fixed accumulators, all digests, lengths, and canonical framing.
pub const OFFLINE_CASH_CURRENT_PROOFS_MAX_BYTES_V1: usize = 4_990;
/// Maximum bytes in either parity's current proof.
pub const OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1: usize = 2_495;
/// Exact compact delayed-history accumulator bytes for one `k=16` parity.
pub const OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1: usize = 544;
/// Maximum encrypted credit-opening bytes carried by a credit envelope.
pub const OFFLINE_CASH_ENCRYPTED_CREDIT_MAX_BYTES_V1: usize = 384;
/// Maximum canonical bytes in the fixed private credit-opening plaintext.
pub const OFFLINE_CASH_CREDIT_OPENING_MAX_BYTES_V1: usize = 256;
/// Exact X25519 public-key width used by encrypted credit envelopes.
pub const OFFLINE_CASH_X25519_PUBLIC_KEY_BYTES_V1: usize = 32;
/// Exact XChaCha20-Poly1305 nonce width used by encrypted credit envelopes.
pub const OFFLINE_CASH_XCHACHA20POLY1305_NONCE_BYTES_V1: usize = 24;
/// Exact authentication-tag width appended to an encrypted credit plaintext.
pub const OFFLINE_CASH_XCHACHA20POLY1305_TAG_BYTES_V1: usize = 16;
/// HKDF-SHA256 salt label for the Offline Cash V1 encrypted-credit KEM.
pub const OFFLINE_CASH_ENCRYPTED_CREDIT_KDF_SALT_LABEL_V1: &[u8] =
    b"iroha:offline-cash:v1:credit-envelope-salt\0";
/// HKDF-SHA256 info label for the Offline Cash V1 encrypted-credit KEM.
pub const OFFLINE_CASH_ENCRYPTED_CREDIT_KDF_INFO_LABEL_V1: &[u8] =
    b"iroha:offline-cash:v1:credit-envelope-key\0";
/// Maximum canonical hardware-profile registry entry bytes.
pub const OFFLINE_CASH_HARDWARE_PROFILE_MAX_BYTES_V1: usize = 512;
/// Maximum canonical compact hardware credential bytes.
pub const OFFLINE_CASH_HARDWARE_CREDENTIAL_MAX_BYTES_V1: usize = 768;
/// Maximum canonical sender acceptance-intent bytes.
pub const OFFLINE_CASH_ACCEPTANCE_INTENT_MAX_BYTES_V1: usize = 256;
/// Maximum complete `oc1:` text sender acceptance-intent bytes.
pub const OFFLINE_CASH_ACCEPTANCE_INTENT_TEXT_MAX_BYTES_V1: usize = OFFLINE_CASH_TEXT_PREFIX_V1
    .len()
    + unpadded_base64url_len(OFFLINE_CASH_ACCEPTANCE_INTENT_MAX_BYTES_V1);
/// Maximum canonical proof-bearing sender acceptance authorization bytes.
pub const OFFLINE_CASH_ACCEPTANCE_INTENT_AUTHORIZATION_MAX_BYTES_V1: usize = 7_936;
/// Maximum canonical authenticated sender no-commit closure bytes.
pub const OFFLINE_CASH_NO_COMMIT_CLOSURE_MAX_BYTES_V1: usize = 16_384;
/// Maximum complete `oc1:` authenticated sender no-commit closure bytes.
pub const OFFLINE_CASH_NO_COMMIT_CLOSURE_TEXT_MAX_BYTES_V1: usize = OFFLINE_CASH_TEXT_PREFIX_V1
    .len()
    + unpadded_base64url_len(OFFLINE_CASH_NO_COMMIT_CLOSURE_MAX_BYTES_V1);
/// Maximum complete `oc1:` text sender acceptance authorization bytes.
pub const OFFLINE_CASH_ACCEPTANCE_INTENT_AUTHORIZATION_TEXT_MAX_BYTES_V1: usize =
    OFFLINE_CASH_TEXT_PREFIX_V1.len()
        + unpadded_base64url_len(OFFLINE_CASH_ACCEPTANCE_INTENT_AUTHORIZATION_MAX_BYTES_V1);
/// Maximum canonical one-use acceptance-ticket bytes.
pub const OFFLINE_CASH_ACCEPTANCE_TICKET_MAX_BYTES_V1: usize = 1_024;
/// Maximum complete `oc1:` text acceptance-ticket bytes.
pub const OFFLINE_CASH_ACCEPTANCE_TICKET_TEXT_MAX_BYTES_V1: usize = OFFLINE_CASH_TEXT_PREFIX_V1
    .len()
    + unpadded_base64url_len(OFFLINE_CASH_ACCEPTANCE_TICKET_MAX_BYTES_V1);
/// Qualification target for request, sender authorization, and issued ticket.
pub const OFFLINE_CASH_PRE_TICKET_EXCHANGE_TARGET_BYTES_V1: usize = 8_960;
/// Absolute raw cap for request, sender authorization, and issued ticket.
pub const OFFLINE_CASH_PRE_TICKET_EXCHANGE_MAX_BYTES_V1: usize =
    OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1
        + OFFLINE_CASH_ACCEPTANCE_INTENT_AUTHORIZATION_MAX_BYTES_V1
        + OFFLINE_CASH_ACCEPTANCE_TICKET_MAX_BYTES_V1;
/// Absolute text cap for request, sender authorization, and issued ticket.
pub const OFFLINE_CASH_PRE_TICKET_TEXT_EXCHANGE_MAX_BYTES_V1: usize =
    OFFLINE_CASH_PAYMENT_REQUEST_TEXT_MAX_BYTES_V1
        + OFFLINE_CASH_ACCEPTANCE_INTENT_AUTHORIZATION_TEXT_MAX_BYTES_V1
        + OFFLINE_CASH_ACCEPTANCE_TICKET_TEXT_MAX_BYTES_V1;
/// Qualification target for all five transported protocol messages.
pub const OFFLINE_CASH_COMPLETE_EXCHANGE_TARGET_BYTES_V1: usize = 16_384;
/// Absolute raw cap for request, authorization, ticket, payment, and acknowledgement.
pub const OFFLINE_CASH_COMPLETE_EXCHANGE_MAX_BYTES_V1: usize = OFFLINE_CASH_SESSION_MAX_BYTES_V1
    + OFFLINE_CASH_ACCEPTANCE_INTENT_AUTHORIZATION_MAX_BYTES_V1
    + OFFLINE_CASH_ACCEPTANCE_TICKET_MAX_BYTES_V1;
/// Absolute text cap for all five separately framed protocol messages.
pub const OFFLINE_CASH_COMPLETE_TEXT_EXCHANGE_MAX_BYTES_V1: usize =
    OFFLINE_CASH_TEXT_SESSION_MAX_BYTES_V1
        + OFFLINE_CASH_ACCEPTANCE_INTENT_AUTHORIZATION_TEXT_MAX_BYTES_V1
        + OFFLINE_CASH_ACCEPTANCE_TICKET_TEXT_MAX_BYTES_V1;
/// Maximum canonical recoverable hardware commit-certificate bytes.
pub const OFFLINE_CASH_COMMIT_CERTIFICATE_MAX_BYTES_V1: usize = 1_024;
/// Maximum hardware-owned staging metadata persisted beside one payment.
pub const OFFLINE_CASH_INBOX_STAGING_METADATA_MAX_BYTES_V1: u32 = 512;
/// Minimum capacity that each acceptance ticket must reserve before commitment.
pub const OFFLINE_CASH_ACCEPTANCE_TICKET_MIN_RESERVED_INBOX_BYTES_V1: u32 =
    OFFLINE_CASH_PAYMENT_MAX_BYTES_V1 as u32
        + OFFLINE_CASH_INBOX_STAGING_METADATA_MAX_BYTES_V1
        + OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1 as u32;
/// Maximum sealed transition inputs persisted for deterministic recovery.
pub const OFFLINE_CASH_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1: u32 = 2_048;
/// Maximum deterministic recovery-seed material persisted before proving.
pub const OFFLINE_CASH_RECOVERY_SEEDS_MAX_BYTES_V1: u32 = 512;
/// Maximum paired proof bytes retained inside the verified precommit candidate.
pub const OFFLINE_CASH_PRECOMMIT_PAIRED_PROOF_MAX_BYTES_V1: u32 =
    outbox_budget_component_from_usize(OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1);
/// Maximum canonical precommit candidate metadata excluding its paired proof.
pub const OFFLINE_CASH_PRECOMMIT_CANDIDATE_METADATA_MAX_BYTES_V1: u32 = 1_024;
/// Maximum canonical final commit-wrapper proof bytes.
pub const OFFLINE_CASH_COMMIT_WRAPPER_PROOF_MAX_BYTES_V1: u32 =
    outbox_budget_component_from_usize(OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1);
/// Maximum authenticated durable-retry metadata beside one terminal envelope.
pub const OFFLINE_CASH_OUTBOX_RETRY_METADATA_MAX_BYTES_V1: u32 = 512;

const fn outbox_budget_component_from_usize(value: usize) -> u32 {
    assert!(value <= u32::MAX as usize);
    value as u32
}

const fn checked_outbox_budget_v1(canonical_envelope_max_bytes: usize) -> u32 {
    let parts = [
        OFFLINE_CASH_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1,
        OFFLINE_CASH_RECOVERY_SEEDS_MAX_BYTES_V1,
        OFFLINE_CASH_PRECOMMIT_PAIRED_PROOF_MAX_BYTES_V1,
        OFFLINE_CASH_PRECOMMIT_CANDIDATE_METADATA_MAX_BYTES_V1,
        outbox_budget_component_from_usize(OFFLINE_CASH_COMMIT_CERTIFICATE_MAX_BYTES_V1),
        OFFLINE_CASH_COMMIT_WRAPPER_PROOF_MAX_BYTES_V1,
        outbox_budget_component_from_usize(canonical_envelope_max_bytes),
        OFFLINE_CASH_OUTBOX_RETRY_METADATA_MAX_BYTES_V1,
    ];
    let mut total = 0_u32;
    let mut index = 0;
    while index < parts.len() {
        total = match total.checked_add(parts[index]) {
            Some(next) => next,
            None => panic!("Offline Cash V1 outbox budget overflow"),
        };
        index += 1;
    }
    total
}

/// Minimum sender outbox reservation for all recoverable payment artifacts.
pub const OFFLINE_CASH_PAYMENT_OUTBOX_MIN_BYTES_V1: u32 =
    checked_outbox_budget_v1(OFFLINE_CASH_PAYMENT_MAX_BYTES_V1);
/// Minimum sender outbox reservation for all recoverable redemption artifacts.
pub const OFFLINE_CASH_REDEMPTION_OUTBOX_MIN_BYTES_V1: u32 =
    checked_outbox_budget_v1(OFFLINE_CASH_REDEMPTION_VOUCHER_MAX_BYTES_V1);
/// Hardware consumes only the exact aggregate-state predecessor.
pub const OFFLINE_CASH_HARDWARE_CAPABILITY_EXACT_NEXT_PREDECESSOR_CONSUMPTION_V1: u16 = 1 << 0;
/// Hardware issues each successor authorization at most once.
pub const OFFLINE_CASH_HARDWARE_CAPABILITY_ONE_USE_SUCCESSOR_AUTHORIZATION_V1: u16 = 1 << 1;
/// Hardware counter and journal state survive rollback and restore attempts.
pub const OFFLINE_CASH_HARDWARE_CAPABILITY_ROLLBACK_RESISTANT_COUNTER_AND_JOURNAL_V1: u16 = 1 << 2;
/// Hardware seals transition inputs and deterministic recovery seeds.
pub const OFFLINE_CASH_HARDWARE_CAPABILITY_SEALED_TRANSITION_RECOVERY_V1: u16 = 1 << 3;
/// Hardware issues each acceptance ticket and its reservation at most once.
pub const OFFLINE_CASH_HARDWARE_CAPABILITY_ONE_USE_ACCEPTANCE_TICKETS_V1: u16 = 1 << 4;
/// Hardware durably reserves inbox bytes before issuing an acceptance ticket.
pub const OFFLINE_CASH_HARDWARE_CAPABILITY_DURABLE_INBOX_RESERVATION_V1: u16 = 1 << 5;
/// Hardware authenticates inbound staging, exact deduplication, and inbox paging.
pub const OFFLINE_CASH_HARDWARE_CAPABILITY_AUTHENTICATED_INBOUND_STAGING_V1: u16 = 1 << 6;
/// Hardware recovers the authoritative replay root and authenticated sparse-tree state.
pub const OFFLINE_CASH_HARDWARE_CAPABILITY_AUTHORITATIVE_REPLAY_ROOT_RECOVERY_V1: u16 = 1 << 7;
/// Hardware reserves sender terminal and envelope bytes before locking a predecessor.
pub const OFFLINE_CASH_HARDWARE_CAPABILITY_SENDER_OUTBOX_RESERVATION_V1: u16 = 1 << 8;
/// Hardware owns an authenticated durable byte-identical retry outbox.
pub const OFFLINE_CASH_HARDWARE_CAPABILITY_AUTHENTICATED_DURABLE_RETRY_OUTBOX_V1: u16 = 1 << 9;
/// Hardware atomically commits only the exact Core-verified candidate digest.
pub const OFFLINE_CASH_HARDWARE_CAPABILITY_ATOMIC_VERIFIED_CANDIDATE_COMMIT_V1: u16 = 1 << 10;
/// Hardware recovers the terminal commit certificate after every terminal outcome.
pub const OFFLINE_CASH_HARDWARE_CAPABILITY_RECOVERABLE_TERMINAL_COMMIT_CERTIFICATE_V1: u16 =
    1 << 11;
/// Hardware supplies trusted time or consumes a secure monotonic authorization lease.
pub const OFFLINE_CASH_HARDWARE_CAPABILITY_TRUSTED_TIME_OR_LEASE_V1: u16 = 1 << 12;
/// Hardware rotates the complete balance and replay root offline.
pub const OFFLINE_CASH_HARDWARE_CAPABILITY_OFFLINE_HARDWARE_EPOCH_ROTATION_V1: u16 = 1 << 13;
/// Hardware rolls exhausted counters without cloning spend authority.
pub const OFFLINE_CASH_HARDWARE_CAPABILITY_ROLLBACK_SAFE_COUNTER_ROLLOVER_V1: u16 = 1 << 14;
/// Hardware fails closed instead of falling back to software authority.
pub const OFFLINE_CASH_HARDWARE_CAPABILITY_NO_SOFTWARE_FALLBACK_V1: u16 = 1 << 15;
/// Exact capability set required from every Offline Cash V1 hardware profile.
pub const OFFLINE_CASH_HARDWARE_REQUIRED_CAPABILITIES_V1: u16 =
    OFFLINE_CASH_HARDWARE_CAPABILITY_EXACT_NEXT_PREDECESSOR_CONSUMPTION_V1
        | OFFLINE_CASH_HARDWARE_CAPABILITY_ONE_USE_SUCCESSOR_AUTHORIZATION_V1
        | OFFLINE_CASH_HARDWARE_CAPABILITY_ROLLBACK_RESISTANT_COUNTER_AND_JOURNAL_V1
        | OFFLINE_CASH_HARDWARE_CAPABILITY_SEALED_TRANSITION_RECOVERY_V1
        | OFFLINE_CASH_HARDWARE_CAPABILITY_ONE_USE_ACCEPTANCE_TICKETS_V1
        | OFFLINE_CASH_HARDWARE_CAPABILITY_DURABLE_INBOX_RESERVATION_V1
        | OFFLINE_CASH_HARDWARE_CAPABILITY_AUTHENTICATED_INBOUND_STAGING_V1
        | OFFLINE_CASH_HARDWARE_CAPABILITY_AUTHORITATIVE_REPLAY_ROOT_RECOVERY_V1
        | OFFLINE_CASH_HARDWARE_CAPABILITY_SENDER_OUTBOX_RESERVATION_V1
        | OFFLINE_CASH_HARDWARE_CAPABILITY_AUTHENTICATED_DURABLE_RETRY_OUTBOX_V1
        | OFFLINE_CASH_HARDWARE_CAPABILITY_ATOMIC_VERIFIED_CANDIDATE_COMMIT_V1
        | OFFLINE_CASH_HARDWARE_CAPABILITY_RECOVERABLE_TERMINAL_COMMIT_CERTIFICATE_V1
        | OFFLINE_CASH_HARDWARE_CAPABILITY_TRUSTED_TIME_OR_LEASE_V1
        | OFFLINE_CASH_HARDWARE_CAPABILITY_OFFLINE_HARDWARE_EPOCH_ROTATION_V1
        | OFFLINE_CASH_HARDWARE_CAPABILITY_ROLLBACK_SAFE_COUNTER_ROLLOVER_V1
        | OFFLINE_CASH_HARDWARE_CAPABILITY_NO_SOFTWARE_FALLBACK_V1;

const DEVICE_KEY_REFERENCE_DOMAIN: &[u8] = b"iroha:offline-cash:v1:device-key-reference";
const ASSET_IDENTITY_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:asset-identity";
const LIABILITY_POOL_DOMAIN: &[u8] = b"iroha:offline-cash:v1:liability-pool";
const AGGREGATE_STATE_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:aggregate-state";
/// Domain shared by the canonical compact outer state commitment and its recursive circuit.
pub const OFFLINE_CASH_PASTA_STATE_COMMITMENT_DOMAIN_V1: &[u8] =
    b"iroha:offline-cash:v1:pasta-state-commitment";
const REQUEST_SIGNING_DOMAIN: &[u8] = b"iroha:offline-cash:v1:payment-request-signing";
const REQUEST_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:payment-request";
const HARDWARE_PROFILE_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:hardware-profile";
const SUITE_COMMITMENT_DOMAIN: &[u8] = b"iroha:offline-cash:v1:suite-commitment";
const HARDWARE_CREDENTIAL_ID_DOMAIN: &[u8] = b"iroha:offline-cash:v1:hardware-credential-id";
const HARDWARE_CREDENTIAL_SIGNING_DOMAIN: &[u8] =
    b"iroha:offline-cash:v1:hardware-credential-signing";
const REQUEST_MODE_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:request-mode";
const ACCEPTANCE_INTENT_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:acceptance-intent";
const ACCEPTANCE_INTENT_AUTHORIZATION_STATEMENT_DIGEST_DOMAIN: &[u8] =
    b"iroha:offline-cash:v1:acceptance-intent-authorization-statement";
const ACCEPTANCE_INTENT_AUTHORIZATION_DIGEST_DOMAIN: &[u8] =
    b"iroha:offline-cash:v1:acceptance-intent-authorization";
const NO_COMMIT_CLOSURE_STATEMENT_DIGEST_DOMAIN: &[u8] =
    b"iroha:offline-cash:v1:no-commit-closure-statement";
const NO_COMMIT_CLOSURE_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:no-commit-closure";
const ACCEPTANCE_TICKET_SIGNING_DOMAIN: &[u8] = b"iroha:offline-cash:v1:acceptance-ticket-signing";
const ACCEPTANCE_TICKET_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:acceptance-ticket";
const CREDIT_ID_DOMAIN: &[u8] = b"iroha:offline-cash:v1:credit-id";
const STATEMENT_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:send-split-statement";
const LIFECYCLE_BINDING_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:lifecycle-binding";
const CIPHERTEXT_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:ciphertext";
const PEER_CREDIT_CONTEXT_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:peer-credit-context";
const PEER_CREDIT_LIFECYCLE_CONTEXT_DIGEST_DOMAIN: &[u8] =
    b"iroha:offline-cash:v1:peer-credit-lifecycle-context";
const ACCOUNT_IDENTITY_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:account-identity";
const MINT_CREDIT_OPENING_COMMITMENT_DOMAIN: &[u8] =
    b"iroha:offline-cash:v1:mint-credit-opening-commitment";
const RECIPIENT_CREDENTIAL_COMMITMENT_DOMAIN: &[u8] =
    b"iroha:offline-cash:v1:recipient-credential-commitment";
const COMMIT_CERTIFICATE_ID_DOMAIN: &[u8] = b"iroha:offline-cash:v1:commit-certificate-id";
const COMMIT_CERTIFICATE_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:commit-certificate";
const HARDWARE_TERMINAL_BODY_COMMITMENT_DOMAIN: &[u8] =
    b"iroha:offline-cash:v1:hardware-terminal-body";
const OUTBOX_RESERVATION_COMMITMENT_DOMAIN: &[u8] = b"iroha:offline-cash:v1:outbox-reservation";
const PAYMENT_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:payment";
const INBOX_RECEIPT_DOMAIN: &[u8] = b"iroha:offline-cash:v1:durable-inbox-receipt";
const ACKNOWLEDGEMENT_SIGNING_DOMAIN: &[u8] = b"iroha:offline-cash:v1:acknowledgement-signing";
const MINT_CREDIT_ID_DOMAIN: &[u8] = b"iroha:offline-cash:v1:mint-credit-id";
const MINT_LIFECYCLE_CONTEXT_DOMAIN: &[u8] = b"iroha:offline-cash:v1:mint-lifecycle-context";
const MINT_STATEMENT_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:mint-statement";
const MINT_AUTHORIZATION_CONTEXT_DIGEST_DOMAIN: &[u8] =
    b"iroha:offline-cash:v1:mint-authorization-context";
const MINT_AUTHORIZATION_STATEMENT_DIGEST_DOMAIN: &[u8] =
    b"iroha:offline-cash:v1:mint-authorization-statement";
const MINT_AUTHORIZATION_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:mint-authorization";
const REDEMPTION_ID_DOMAIN: &[u8] = b"iroha:offline-cash:v1:redemption-id";
const REDEMPTION_STATEMENT_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:redemption-statement";

/// Error returned when canonical Offline Cash V1 data fails validation.
#[derive(Debug)]
pub enum OfflineCashValidationErrorV1 {
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

impl core::fmt::Display for OfflineCashValidationErrorV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Codec(error) => write!(f, "canonical Offline Cash V1 codec failed: {error}"),
            Self::EncodedSizeExceeded { actual, max } => {
                write!(f, "Offline Cash V1 wire size {actual} exceeds limit {max}")
            }
            Self::InvalidField { field } => {
                write!(f, "invalid Offline Cash V1 field `{field}`")
            }
        }
    }
}

impl std::error::Error for OfflineCashValidationErrorV1 {}

impl From<norito::Error> for OfflineCashValidationErrorV1 {
    fn from(error: norito::Error) -> Self {
        Self::Codec(error)
    }
}

/// Sole Offline Cash V1 device authority key.
///
/// The wire value is exactly one canonical uncompressed SEC1 NIST P-256 point
/// (`0x04 || x || y`). There is no algorithm tag or selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, IntoSchema)]
#[repr(transparent)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct OfflineCashDevicePublicKeyV1([u8; OFFLINE_CASH_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1]);

/// Sole Offline Cash V1 device signature.
///
/// The wire value is the fixed-width big-endian ECDSA scalar pair `r || s`.
/// Both scalars must be in `1..n`, and `s` must be low.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, IntoSchema)]
#[repr(transparent)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct OfflineCashDeviceSignatureV1([u8; OFFLINE_CASH_DEVICE_SIGNATURE_BYTES_V1]);

impl norito::NoritoSerialize for OfflineCashDevicePublicKeyV1 {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        self.validate()
            .map_err(|error| norito::Error::Message(error.to_string()))?;
        writer.write_all(&self.0)?;
        Ok(())
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        Some(OFFLINE_CASH_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        self.encoded_len_hint()
    }
}

impl<'de> norito::NoritoDeserialize<'de> for OfflineCashDevicePublicKeyV1 {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("Offline Cash device public key must be canonical SEC1 bytes")
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

impl<'a> norito::core::DecodeFromSlice<'a> for OfflineCashDevicePublicKeyV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::Error> {
        let raw = bytes
            .get(..OFFLINE_CASH_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1)
            .ok_or(norito::Error::LengthMismatch)?;
        let value = Self::from_sec1_bytes(raw)
            .map_err(|error| norito::Error::Message(error.to_string()))?;
        Ok((value, OFFLINE_CASH_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1))
    }
}

impl norito::NoritoSerialize for OfflineCashDeviceSignatureV1 {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        self.validate()
            .map_err(|error| norito::Error::Message(error.to_string()))?;
        writer.write_all(&self.0)?;
        Ok(())
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        Some(OFFLINE_CASH_DEVICE_SIGNATURE_BYTES_V1)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        self.encoded_len_hint()
    }
}

impl<'de> norito::NoritoDeserialize<'de> for OfflineCashDeviceSignatureV1 {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("Offline Cash device signature must be canonical raw P-256 bytes")
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

impl<'a> norito::core::DecodeFromSlice<'a> for OfflineCashDeviceSignatureV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::Error> {
        let raw = bytes
            .get(..OFFLINE_CASH_DEVICE_SIGNATURE_BYTES_V1)
            .ok_or(norito::Error::LengthMismatch)?;
        let value =
            Self::from_raw_bytes(raw).map_err(|error| norito::Error::Message(error.to_string()))?;
        Ok((value, OFFLINE_CASH_DEVICE_SIGNATURE_BYTES_V1))
    }
}

impl OfflineCashDevicePublicKeyV1 {
    /// Parse the canonical uncompressed SEC1 P-256 encoding.
    ///
    /// # Errors
    ///
    /// Returns an error for the wrong width, a compressed or invalid point, or
    /// a non-canonical encoding.
    pub fn from_sec1_bytes(bytes: &[u8]) -> Result<Self, OfflineCashValidationErrorV1> {
        let raw: [u8; OFFLINE_CASH_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1] =
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
    pub fn validate(&self) -> Result<(), OfflineCashValidationErrorV1> {
        Self::from_sec1_bytes(&self.0).map(|_| ())
    }

    /// Return the canonical uncompressed SEC1 bytes.
    #[must_use]
    pub const fn as_sec1_bytes(&self) -> &[u8; OFFLINE_CASH_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1] {
        &self.0
    }

    fn verifying_key(&self) -> Result<P256VerifyingKey, OfflineCashValidationErrorV1> {
        self.validate()?;
        P256VerifyingKey::from_sec1_bytes(&self.0).map_err(|_| invalid("device_public_key"))
    }
}

impl TryFrom<&[u8]> for OfflineCashDevicePublicKeyV1 {
    type Error = OfflineCashValidationErrorV1;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        Self::from_sec1_bytes(value)
    }
}

impl TryFrom<[u8; OFFLINE_CASH_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1]> for OfflineCashDevicePublicKeyV1 {
    type Error = OfflineCashValidationErrorV1;

    fn try_from(
        value: [u8; OFFLINE_CASH_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1],
    ) -> Result<Self, Self::Error> {
        Self::from_sec1_bytes(&value)
    }
}

impl AsRef<[u8]> for OfflineCashDevicePublicKeyV1 {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl OfflineCashDeviceSignatureV1 {
    /// Parse a canonical fixed-width low-S P-256 ECDSA signature.
    ///
    /// # Errors
    ///
    /// Returns an error for the wrong width, invalid scalars, or a high-S signature.
    pub fn from_raw_bytes(bytes: &[u8]) -> Result<Self, OfflineCashValidationErrorV1> {
        let raw: [u8; OFFLINE_CASH_DEVICE_SIGNATURE_BYTES_V1] =
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
    pub fn validate(&self) -> Result<(), OfflineCashValidationErrorV1> {
        Self::from_raw_bytes(&self.0).map(|_| ())
    }

    /// Return the canonical fixed-width `r || s` bytes.
    #[must_use]
    pub const fn as_raw_bytes(&self) -> &[u8; OFFLINE_CASH_DEVICE_SIGNATURE_BYTES_V1] {
        &self.0
    }

    /// Verify ECDSA-P256-SHA256 under the fixed Offline Cash V1 profile.
    ///
    /// # Errors
    ///
    /// Returns an error when the key, signature, or authentication is invalid.
    pub fn verify(
        &self,
        public_key: &OfflineCashDevicePublicKeyV1,
        message: &[u8],
    ) -> Result<(), OfflineCashValidationErrorV1> {
        self.validate()?;
        let signature =
            P256Signature::from_slice(&self.0).map_err(|_| invalid("device_signature"))?;
        public_key
            .verifying_key()?
            .verify(message, &signature)
            .map_err(|_| invalid("device_signature"))
    }
}

impl TryFrom<&[u8]> for OfflineCashDeviceSignatureV1 {
    type Error = OfflineCashValidationErrorV1;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        Self::from_raw_bytes(value)
    }
}

impl TryFrom<[u8; OFFLINE_CASH_DEVICE_SIGNATURE_BYTES_V1]> for OfflineCashDeviceSignatureV1 {
    type Error = OfflineCashValidationErrorV1;

    fn try_from(value: [u8; OFFLINE_CASH_DEVICE_SIGNATURE_BYTES_V1]) -> Result<Self, Self::Error> {
        Self::from_raw_bytes(&value)
    }
}

impl AsRef<[u8]> for OfflineCashDeviceSignatureV1 {
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
pub struct OfflineCashAggregateStateCommitmentV1 {
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
/// [`offline_cash_pasta_state_commitment_v1`], while the paired verifier requires both proofs to
/// expose this exact pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashPastaStateCommitmentV1 {
    /// Eq/Fp native state-commitment component.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub eq: [u8; 32],
    /// Ep/Fq native state-commitment component.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ep: [u8; 32],
}

impl OfflineCashPastaStateCommitmentV1 {
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
/// SHA-256 is only the compact collision-resistant name of the pair. Acceptance still requires
/// an Eq proof for `components.eq` and an Ep proof for `components.ep`; hashing a pair does not
/// grant monetary authority.
#[must_use]
pub fn offline_cash_pasta_state_commitment_v1(
    components: OfflineCashPastaStateCommitmentV1,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(OFFLINE_CASH_PASTA_STATE_COMMITMENT_DOMAIN_V1);
    hasher.update([0]);
    hasher.update(components.eq);
    hasher.update(components.ep);
    hasher.finalize().into()
}

/// Closed paired-Pasta proof and delayed-history accumulators.
///
/// Every proof instance, including sender pre-ticket and mint authorization,
/// uses fresh circuit randomness. Credential audits and history accumulators
/// are statement-scoped projections that bind `semantic_digest`; they must
/// never be a stable credential, lane, device, or predecessor pseudonym that
/// links otherwise unrelated transcripts.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashPairedProofV1 {
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
/// A class label never grants offline authority by itself. The complete
/// profile, physical qualification evidence, credential, and recursive proof
/// must all validate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "class", content = "value", rename_all = "snake_case")]
#[norito(deny_unknown_fields)]
pub enum OfflineCashHardwarePlatformClassV1 {
    /// Qualified Android OEM or secure-element service.
    AndroidOemService,
    /// Qualified Apple OEM or secure-element service.
    AppleOemService,
    /// Qualified dedicated secure element outside a stock mobile API.
    DedicatedSecureElement,
    /// Other governed implementation with equivalent physical evidence.
    OtherQualified,
}

/// Governed Offline Cash V1 non-forking hardware-service profile.
///
/// The capability mask must equal
/// [`OFFLINE_CASH_HARDWARE_REQUIRED_CAPABILITIES_V1`]. A stock hardware-backed
/// signing key is therefore insufficient unless its surrounding service
/// implements and qualifies the complete counter, journal, capacity,
/// atomic-commit, recovery, time/lease, rotation, and no-fallback contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashHardwareProfileV1 {
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
    pub platform_class: OfflineCashHardwarePlatformClassV1,
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
    pub governance_credential_public_key: OfflineCashDevicePublicKeyV1,
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
pub struct OfflineCashHardwareCredentialV1 {
    /// Wire version.
    pub version: u16,
    /// Digest-derived credential identity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub credential_id: [u8; 32],
    /// Exact network on which the device may authorize offline cash.
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
    /// Device transition, request, ticket, and commit-certificate authority key.
    pub device_public_key: OfflineCashDevicePublicKeyV1,
    /// Domain-separated reference to `device_public_key`.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub device_key_reference: [u8; 32],
    /// Inclusive credential issuance time in Unix milliseconds.
    pub issued_at_ms: u64,
    /// Exclusive credential expiry in Unix milliseconds.
    pub expires_at_ms: u64,
    /// Governance/profile-issuer signature over the exact compact credential.
    pub governance_signature: OfflineCashDeviceSignatureV1,
}

/// Inclusive per-payment amount interval authorized by a reusable request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashAmountPolicyV1 {
    /// Smallest positive payment amount accepted by this policy.
    pub minimum_amount: u128,
    /// Largest payment amount accepted by this policy.
    pub maximum_amount: u128,
}

/// Exact amount payload for [`OfflineCashPaymentRequestModeV1::SingleExact`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashSingleExactRequestV1 {
    /// Required payment amount.
    pub amount: u128,
}

/// Total payload for [`OfflineCashPaymentRequestModeV1::PartialUntilTotal`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashPartialUntilTotalRequestV1 {
    /// Aggregate invoice total; it is not a protocol history limit.
    pub total_amount: u128,
}

/// Count and amount payload for [`OfflineCashPaymentRequestModeV1::BoundedMultiPayment`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashBoundedMultiPaymentRequestV1 {
    /// Positive request-local payment count bound.
    pub max_payments: u32,
    /// Amount interval for each independently ticketed payment.
    pub per_payment: OfflineCashAmountPolicyV1,
}

/// Per-payment payload for [`OfflineCashPaymentRequestModeV1::OpenReceive`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashOpenReceiveRequestV1 {
    /// Amount interval for each independently ticketed payment.
    pub per_payment: OfflineCashAmountPolicyV1,
}

/// Closed Offline Cash V1 receiver-request mode.
///
/// Every actual sender commitment consumes a distinct one-use
/// [`OfflineCashAcceptanceTicketV1`]. Consequently `OpenReceive` permits an
/// unbounded cumulative number of payments without making a ticket reusable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(
    tag = "mode",
    content = "policy",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum OfflineCashPaymentRequestModeV1 {
    /// Exactly one payment of the stated amount.
    SingleExact(OfflineCashSingleExactRequestV1),
    /// Distinct tickets may authorize partial payments until this total is reached.
    PartialUntilTotal(OfflineCashPartialUntilTotalRequestV1),
    /// At most the stated number of tickets/payments under one request.
    BoundedMultiPayment(OfflineCashBoundedMultiPaymentRequestV1),
    /// Arbitrarily many independently ticketed payments under one request.
    OpenReceive(OfflineCashOpenReceiveRequestV1),
}

/// Sender-selected one-use intent presented before receiver capacity is reserved.
///
/// The random commitment is opened only inside the final wrapper, which proves
/// that it names the exact private predecessor consumed by sender hardware. No
/// sender key, credential, lane, epoch, counter, predecessor, or successor is
/// exposed. Receiver hardware atomically records the intent while applying the
/// request-mode ledger and issuing the corresponding exact-amount ticket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashAcceptanceIntentV1 {
    /// Wire version.
    pub version: u16,
    /// Digest of the exact signed receiver request.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub request_digest: [u8; 32],
    /// Random one-use intent identity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub intent_id: [u8; 32],
    /// Exact positive payment amount requested from receiver capacity.
    pub exact_amount: u128,
    /// Hiding one-use sender authorization bound to the private predecessor.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub sender_one_time_commitment: [u8; 32],
}

/// Release-bound public statement for a pre-ticket sender authorization.
///
/// The sender profile and credential stay private. Both proof parities prove
/// membership in the enabled hardware-profile set of this exact release, so a
/// receiver never substitutes its own profile as sender authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashAcceptanceIntentAuthorizationStatementV1 {
    /// Wire version.
    pub version: u16,
    /// Compact unlinkable sender intent.
    pub intent: OfflineCashAcceptanceIntentV1,
    /// Authenticated release whose enabled-profile set contains the hidden sender profile.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub release_id: [u8; 32],
    /// Exact release-wide proof suite selected for the authorization.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub suite_id: [u8; 32],
    /// Digest of the exact release-pinned verifying-key set.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub vk_digest: [u8; 32],
    /// Digest of the authenticated proof artifact manifest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub artifact_manifest_digest: [u8; 32],
}

/// Proof-bearing sender capability checked before receiver reservation.
///
/// Receiver hardware must not consume request budget or guaranteed inbox
/// capacity from the compact intent alone. The authenticated native verifier
/// first proves that qualified non-forking sender hardware reserved a one-use
/// predecessor authorization for the exact request and amount. This envelope
/// is exchanged before ticket issuance; the later payment embeds only the
/// compact intent whose digest the receiver ticket signed.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashAcceptanceIntentAuthorizationV1 {
    /// Wire version.
    pub version: u16,
    /// Release-bound compact authorization statement.
    pub statement: OfflineCashAcceptanceIntentAuthorizationStatementV1,
    /// Paired proof of hidden enabled-profile membership, qualified hardware,
    /// sufficient private balance, and a one-use authorization tied to the
    /// exact private predecessor.
    pub proof: OfflineCashPairedProofV1,
}

/// Public statement for release-pinned proof that one sender authorization was cancelled.
///
/// The proof establishes that qualified non-forking hardware consumed the exact private
/// authorization predecessor through its one-use cancellation successor. The public statement
/// exposes only an unlinkable cancellation nullifier, never either aggregate state commitment or
/// a hardware counter. Consequently the same private predecessor cannot later produce a terminal
/// payment. Ticket expiry alone never satisfies this statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashNoCommitClosureStatementV1 {
    /// Wire version.
    pub version: u16,
    /// Authenticated proof release.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub release_id: [u8; 32],
    /// Exact proof suite selected by the release.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub suite_id: [u8; 32],
    /// Exact release-pinned verifying-key set.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub vk_digest: [u8; 32],
    /// Authenticated proof artifact manifest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub artifact_manifest_digest: [u8; 32],
    /// Hiding commitment to the sender hardware profile and epoch proven by GuardBundle.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub sender_hardware_binding_commitment: [u8; 32],
    /// Receiver request identity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub request_id: [u8; 32],
    /// Digest of the exact signed request.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub request_digest: [u8; 32],
    /// One-use acceptance-ticket identity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub acceptance_ticket_id: [u8; 32],
    /// Digest of the exact receiver ticket.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ticket_digest: [u8; 32],
    /// Digest of the original proof-bearing sender authorization envelope.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub intent_authorization_digest: [u8; 32],
    /// Digest of the exact compact sender intent recorded by the ticket.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub intent_digest: [u8; 32],
    /// Exact amount reserved by the intent and ticket.
    pub exact_amount: u128,
    /// Original hiding commitment to the authorized sender predecessor.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub sender_one_time_commitment: [u8; 32],
    /// Unique no-commit recovery identity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub recovery_id: [u8; 32],
    /// Unlinkable conflict nullifier shared by every terminal or cancellation successor of the
    /// exact private prepared predecessor.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub cancellation_nullifier: [u8; 32],
    /// Capacity-preserving receiver delivery slot used while recovery is pending.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub equivalent_delivery_slot_commitment: [u8; 32],
}

/// Paired release-pinned proof of one irreversible sender no-commit closure.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashNoCommitClosureV1 {
    /// Wire version.
    pub version: u16,
    /// Exact public closure statement constrained by both proof parities.
    pub statement: OfflineCashNoCommitClosureStatementV1,
    /// Exact signed receiver request whose authorization is being closed.
    pub request: OfflineCashPaymentRequestV1,
    /// Original proof-bearing sender authorization, reverified atomically with the closure.
    pub intent_authorization: OfflineCashAcceptanceIntentAuthorizationV1,
    /// Exact receiver-hardware ticket retained through recovery.
    pub acceptance_ticket: OfflineCashAcceptanceTicketV1,
    /// Paired CommitWrapper proof under the authenticated release roles.
    pub proof: OfflineCashPairedProofV1,
}

/// One-use receiver-hardware reservation issued before sender commitment.
///
/// Expiry is an exclusive sender-commit deadline, not permission to reclaim
/// capacity automatically. It is consumed by staging the bound payment, or an
/// unused reservation moves through governed online recovery while an
/// equivalent durable delivery slot remains preserved until recovery closes.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashAcceptanceTicketV1 {
    /// Wire version.
    pub version: u16,
    /// Exact network inherited from the request.
    pub network_id: NetworkId,
    /// Identity of the request that owns this ticket.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub request_id: [u8; 32],
    /// Digest of the exact signed request.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub request_digest: [u8; 32],
    /// Unique one-use ticket identity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub acceptance_ticket_id: [u8; 32],
    /// Exact asset authorized by the ticket.
    pub asset: AssetDefinitionId,
    /// Exact asset incarnation authorized by the ticket.
    pub asset_incarnation: AxtAssetIncarnationV1,
    /// Authoritative asset scale.
    pub scale: u32,
    /// Request mode repeated verbatim to prevent mode substitution.
    pub request_mode: OfflineCashPaymentRequestModeV1,
    /// Digest of the exact sender intent atomically recorded at issuance.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub intent_digest: [u8; 32],
    /// Exact amount reserved and authorized by this one ticket.
    pub exact_amount: u128,
    /// Physical inbox bytes reserved before the sender may commit.
    pub reserved_inbox_bytes: u32,
    /// Suite-defined recipient one-time public key.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub recipient_one_time_key: [u8; 32],
    /// Hardware profile that issued the reservation.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub hardware_profile_id: [u8; 32],
    /// Hardware policy epoch that issued the reservation.
    pub policy_epoch: u64,
    /// Inclusive ticket issuance time in Unix milliseconds.
    pub issued_at_ms: u64,
    /// Exclusive sender-commit deadline in Unix milliseconds.
    pub expires_at_ms: u64,
    /// Receiver-hardware signature over the exact ticket.
    pub signature: OfflineCashDeviceSignatureV1,
}

/// Exact recipient-only plaintext protected by an encrypted credit envelope.
///
/// The opening is fixed-size and history-independent. Qualified hardware must
/// decode these canonical bytes after AEAD authentication and reject any
/// public `credit_id` or `amount` mismatch before admitting the credit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashCreditOpeningV1 {
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
pub enum OfflineCashEncryptedCreditPurposeV1 {
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
pub struct OfflineCashEncryptedCreditAadV1 {
    /// Wire version.
    pub version: u16,
    /// Whether this envelope carries a mint or peer credit.
    pub purpose: OfflineCashEncryptedCreditPurposeV1,
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
/// the signed ticket or mint-authorization context and is not repeated here.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashEncryptedCreditEnvelopeV1 {
    /// Wire version.
    pub version: u16,
    /// Fresh sender X25519 ephemeral public key.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ephemeral_x25519_public_key: [u8; OFFLINE_CASH_X25519_PUBLIC_KEY_BYTES_V1],
    /// Fresh XChaCha20-Poly1305 nonce.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub nonce: [u8; OFFLINE_CASH_XCHACHA20POLY1305_NONCE_BYTES_V1],
    /// Combined ciphertext followed by the exact 16-byte Poly1305 tag.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub ciphertext_and_tag: Vec<u8>,
}

/// Monetary operation bound by a released V1 transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "operation", content = "value", rename_all = "snake_case")]
#[norito(deny_unknown_fields)]
pub enum OfflineCashOperationKindV1 {
    /// Establish a zero-balance hardware lane.
    Bootstrap,
    /// Fold one finalized reserve-backed mint credit.
    MintFold,
    /// Produce one receiver-bound payment credit.
    SendSplit,
    /// Fold a padded fixed-shape batch of one through sixteen credits.
    ReceiveFoldBatch,
    /// Produce one online redemption voucher.
    RedeemSplit,
    /// Rotate verifier suites through a recursively verified bridge.
    SuiteUpgrade,
    /// Rotate hardware epoch without changing monetary value.
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
pub struct OfflineCashLifecycleBindingV1 {
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
    pub operation_kind: OfflineCashOperationKindV1,
    /// Receiver request identity for `SendSplit`, otherwise zero.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub request_id: [u8; 32],
    /// One-use acceptance-ticket identity for `SendSplit`, otherwise zero.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub acceptance_ticket_id: [u8; 32],
    /// Receiver credit identity for `SendSplit`, otherwise zero.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub credit_id: [u8; 32],
    /// Encrypted-credit digest for `SendSplit`, otherwise zero.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ciphertext_digest: [u8; 32],
}

/// Trusted-time payload for [`OfflineCashCommitEvidenceV1::TrustedTime`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashTrustedCommitTimeV1 {
    /// Hiding commitment to the qualified clock evidence, commit instant, and authority.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub time_evidence_commitment: [u8; 32],
}

/// Secure monotonic lease payload for [`OfflineCashCommitEvidenceV1::MonotonicLease`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashMonotonicCommitLeaseV1 {
    /// Hiding commitment to the unique lease, its window, consumed counter,
    /// and qualified authorization service.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub lease_evidence_commitment: [u8; 32],
}

/// Public evidence that hardware committed before an applicable deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(
    tag = "source",
    content = "evidence",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum OfflineCashCommitEvidenceV1 {
    /// Qualified hardware supplied a trusted Unix commit time.
    TrustedTime(OfflineCashTrustedCommitTimeV1),
    /// Qualified hardware consumed a secure monotonic authorization lease.
    MonotonicLease(OfflineCashMonotonicCommitLeaseV1),
}

/// Sender outbox capacity reserved before hardware may consume its predecessor.
///
/// This private precommit witness is hidden behind
/// [`OfflineCashCommitCertificateV1::outbox_reservation_commitment`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashOutboxReservationV1 {
    /// Unique one-use reservation identity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub reservation_id: [u8; 32],
    /// Exact operation whose canonical terminal envelope owns the reservation.
    pub operation_kind: OfflineCashOperationKindV1,
    /// Physical outbox bytes reserved for the recoverable canonical envelope.
    pub reserved_outbox_bytes: u32,
    /// Inclusive reservation issuance time in Unix milliseconds.
    pub issued_at_ms: u64,
    /// Exclusive deadline for consuming this reservation.
    pub expires_at_ms: u64,
}

/// Self-free hardware terminal body committed before a certificate ID exists.
///
/// This value remains inside qualified hardware. Only its canonical commitment
/// appears in [`OfflineCashCommitCertificateV1`]. It deliberately contains no
/// certificate ID, certificate digest, wrapper digest, or terminal envelope
/// field derived from those values, fixing the construction order as terminal
/// body → terminal commitment → certificate ID → wrapper proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashHardwareTerminalBodyV1 {
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
    /// Opaque trusted-time or secure-lease evidence consumed at commit.
    pub commit_evidence: OfflineCashCommitEvidenceV1,
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
///
/// Stable lane, epoch, credential, journal, successor-authorization, and raw
/// reservation fields remain private beneath `hardware_terminal_commitment`.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashCommitCertificateV1 {
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
    pub commit_evidence: OfflineCashCommitEvidenceV1,
    /// Qualified sender hardware profile proven by the wrapper.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub hardware_profile_id: [u8; 32],
    /// Sender policy epoch proven by the wrapper.
    pub policy_epoch: u64,
    /// Hiding commitment to a self-free private terminal body and hardware state.
    ///
    /// Construct this from [`OfflineCashHardwareTerminalBodyV1`] before deriving
    /// `certificate_id`; it never commits the certificate ID back into itself.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub hardware_terminal_commitment: [u8; 32],
}

/// Final paired proof that turns a prepared transition into committed money.
///
/// This wrapper recursively verifies the private prepared transition proof,
/// credential checks, exact outbox-reservation budget, and terminal certificate.
/// It exposes neither state links nor stable credential-audit pseudonyms.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashCommitWrapperProofV1 {
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
    /// Digest of the prepared transition proof/envelope verified by the wrapper.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub candidate_envelope_digest: [u8; 32],
    /// Digest of the terminal commit certificate verified by the wrapper.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub commit_certificate_digest: [u8; 32],
    /// Eq/Fp deferred reciprocal-verification audit exposed by both parities.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub eq_deferred_audit: [u8; 32],
    /// Ep/Fq deferred reciprocal-verification audit exposed by both parities.
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

/// Receiver-created authorization for one payment into a stable inbox lane.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashPaymentRequestV1 {
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
    /// Reusable request policy; every payment still requires a distinct ticket.
    pub request_mode: OfflineCashPaymentRequestModeV1,
    /// Compact qualified-hardware credential authorizing this request and its tickets.
    pub hardware_credential: OfflineCashHardwareCredentialV1,
    /// Unique recipient nonce.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub request_id: [u8; 32],
    /// Request creation time in Unix milliseconds.
    pub issued_at_ms: u64,
    /// Exclusive sender-commit deadline in Unix milliseconds.
    pub expires_at_ms: u64,
    /// Low-S P-256 signature over the exact unsigned request.
    pub signature: OfflineCashDeviceSignatureV1,
}

/// Unlinkable public send statement decided by both Pasta parities.
///
/// The private candidate proof consumes and creates aggregate state, but those
/// predecessor and successor commitments never appear in this public record.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashTransferStatementV1 {
    /// Wire version.
    pub version: u16,
    /// Complete released-credit lifecycle binding.
    pub lifecycle: OfflineCashLifecycleBindingV1,
    /// Positive transfer amount in atomic units.
    pub amount: u128,
    /// Unique, proof-derived transition nullifier with no public state preimage.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub transition_nullifier: [u8; 32],
    /// Digest of the exact recipient request.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub request_digest: [u8; 32],
    /// Digest of the exact one-use acceptance ticket.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub acceptance_ticket_digest: [u8; 32],
    /// Recipient one-time key copied from the acceptance ticket.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub recipient_one_time_key: [u8; 32],
    /// Commitment to amount-bound ciphertext semantics.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ciphertext_commitment: [u8; 32],
    /// Trusted-time or secure monotonic-lease evidence used by hardware commit.
    pub commit_evidence: OfflineCashCommitEvidenceV1,
}

/// Sender response containing one receiver-bound aggregate credit proof.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashPaymentV1 {
    /// Wire version.
    pub version: u16,
    /// Unlinkable public statement decided by both wrapper-proof parities.
    pub statement: OfflineCashTransferStatementV1,
    /// Sender intent whose private commitment is opened by the wrapper.
    pub acceptance_intent: OfflineCashAcceptanceIntentV1,
    /// One-use receiver capacity reservation consumed by this payment.
    pub acceptance_ticket: OfflineCashAcceptanceTicketV1,
    /// Recoverable hardware terminal certificate for the exact candidate.
    pub commit_certificate: OfflineCashCommitCertificateV1,
    /// Final wrapper proof of the candidate transition and terminal certificate.
    pub proof: OfflineCashCommitWrapperProofV1,
    /// Recipient-only encrypted credit opening.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub encrypted_credit: Vec<u8>,
    /// Digest of the authenticated artifact manifest used to produce the proof.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub artifact_manifest_digest: [u8; 32],
}

/// Durable secure-inbox record named by a receiver acknowledgement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashInboxReceiptV1 {
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
pub struct OfflineCashAcknowledgementV1 {
    /// Wire version.
    pub version: u16,
    /// Digest of the accepted recipient request.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub request_digest: [u8; 32],
    /// Digest of the durably persisted sender response.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub payment_digest: [u8; 32],
    /// Durable receiver-inbox receipt; this is not a receiver balance head.
    pub inbox_receipt: OfflineCashInboxReceiptV1,
    /// Low-S P-256 signature over the acknowledgement fields.
    pub signature: OfflineCashDeviceSignatureV1,
}

/// Pre-ID recipient context authorized before a reserve debit may occur.
///
/// The two randomized commitments and recipient encryption key are sampled
/// before, and independently of, the issuance commitment and credit ID. This
/// exact context digest is included in both derived identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashMintAuthorizationContextV1 {
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
    /// Offline account authorized to receive the credit.
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
pub struct OfflineCashMintAuthorizationStatementV1 {
    /// Wire version.
    pub version: u16,
    /// Complete pre-ID recipient authorization context.
    pub context: OfflineCashMintAuthorizationContextV1,
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
pub struct OfflineCashMintAuthorizationV1 {
    /// Wire version.
    pub version: u16,
    /// Exact recipient and output statement.
    pub statement: OfflineCashMintAuthorizationStatementV1,
    /// Paired release-pinned proof of the recipient hardware relation.
    pub proof: OfflineCashPairedProofV1,
}

/// Public top-up statement creating one foldable aggregate credit.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashMintCreditStatementV1 {
    /// Wire version.
    pub version: u16,
    /// Complete released `MintFold` lifecycle binding.
    pub lifecycle: OfflineCashLifecycleBindingV1,
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
pub struct OfflineCashMintCreditV1 {
    /// Wire version.
    pub version: u16,
    /// Public issuance statement.
    pub statement: OfflineCashMintCreditStatementV1,
    /// Paired proof of committed reserve liability and valid credit creation.
    pub proof: OfflineCashPairedProofV1,
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
/// Private sender lane, epoch, credential, journal, predecessor, successor, and
/// sequence values are constrained by the wrapper but absent from this record.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashRedemptionStatementV1 {
    /// Wire version.
    pub version: u16,
    /// Complete released terminal lifecycle binding.
    pub lifecycle: OfflineCashLifecycleBindingV1,
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
    pub commit_evidence: OfflineCashCommitEvidenceV1,
}

/// Constant-size terminal voucher submitted for online redemption.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashRedemptionVoucherV1 {
    /// Wire version.
    pub version: u16,
    /// Public terminal transition.
    pub statement: OfflineCashRedemptionStatementV1,
    /// Recoverable hardware terminal certificate for the exact candidate.
    pub commit_certificate: OfflineCashCommitCertificateV1,
    /// Final wrapper proof of balance conservation and committed terminal state.
    pub proof: OfflineCashCommitWrapperProofV1,
    /// Digest of the authenticated artifact manifest used by the proof.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub artifact_manifest_digest: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.offline-cash.v1.liability-pool-preimage")]
struct LiabilityPoolPreimageV1 {
    network_id: NetworkId,
    asset: AssetDefinitionId,
    asset_incarnation: AxtAssetIncarnationV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.offline-cash.v1.hardware-profile-id-preimage")]
struct HardwareProfileIdPreimageV1 {
    version: u16,
    protocol_version: u16,
    provider_id: [u8; 32],
    platform_class: OfflineCashHardwarePlatformClassV1,
    product_class_digest: [u8; 32],
    firmware_policy_digest: [u8; 32],
    enrollment_attestation_verifier_digest: [u8; 32],
    attestation_trust_roots_digest: [u8; 32],
    allowed_suite_commitment: [u8; 32],
    policy_epoch: u64,
    governance_credential_public_key: OfflineCashDevicePublicKeyV1,
    capability_mask: u16,
    qualification_report_digest: [u8; 32],
    valid_from_ms: u64,
    expires_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.offline-cash.v1.payment-request-signing-preimage")]
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
    request_mode: OfflineCashPaymentRequestModeV1,
    hardware_credential_id: [u8; 32],
    request_id: [u8; 32],
    issued_at_ms: u64,
    expires_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.offline-cash.v1.credit-id-preimage")]
struct CreditIdPreimageV1 {
    transition_nullifier: [u8; 32],
    request_digest: [u8; 32],
    acceptance_ticket_digest: [u8; 32],
    recipient_one_time_key: [u8; 32],
    amount: u128,
    ciphertext_commitment: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.offline-cash.v1.peer-credit-lifecycle-context-preimage")]
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
    operation_kind: OfflineCashOperationKindV1,
    request_id: [u8; 32],
    acceptance_ticket_id: [u8; 32],
}

/// Exact pre-ID peer-transfer context authenticated by encrypted-credit AAD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashPeerCreditContextV1 {
    /// Wire version.
    pub version: u16,
    /// Digest of the exact signed receiver request.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub request_digest: [u8; 32],
    /// Digest of the exact sender one-use intent.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub acceptance_intent_digest: [u8; 32],
    /// Digest of the exact receiver one-use ticket.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub acceptance_ticket_digest: [u8; 32],
    /// Digest of the released lifecycle fields that exist before encryption.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub lifecycle_context_digest: [u8; 32],
    /// Signed recipient X25519 one-time public key.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub recipient_one_time_key: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.offline-cash.v1.mint-credit-opening-commitment-preimage")]
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
#[norito(schema_name = "iroha.offline-cash.v1.recipient-credential-commitment-preimage")]
struct RecipientCredentialCommitmentPreimageV1 {
    operation_id: [u8; 32],
    hardware_credential_id: [u8; 32],
    recipient_binding_opening: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.offline-cash.v1.hardware-credential-id-preimage")]
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
    device_public_key: OfflineCashDevicePublicKeyV1,
    device_key_reference: [u8; 32],
    issued_at_ms: u64,
    expires_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.offline-cash.v1.hardware-credential-signing-preimage")]
struct HardwareCredentialSigningPreimageV1 {
    domain: Vec<u8>,
    credential_id: [u8; 32],
    credential: HardwareCredentialIdPreimageV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.offline-cash.v1.acceptance-ticket-signing-preimage")]
struct AcceptanceTicketSigningPreimageV1 {
    domain: Vec<u8>,
    version: u16,
    network_id: NetworkId,
    request_id: [u8; 32],
    request_digest: [u8; 32],
    acceptance_ticket_id: [u8; 32],
    asset: AssetDefinitionId,
    asset_incarnation: AxtAssetIncarnationV1,
    scale: u32,
    request_mode: OfflineCashPaymentRequestModeV1,
    intent_digest: [u8; 32],
    exact_amount: u128,
    reserved_inbox_bytes: u32,
    recipient_one_time_key: [u8; 32],
    hardware_profile_id: [u8; 32],
    policy_epoch: u64,
    issued_at_ms: u64,
    expires_at_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.offline-cash.v1.inbox-receipt-preimage")]
struct InboxReceiptPreimageV1 {
    recipient_lane_id: [u8; 32],
    staging_hardware_epoch_id: [u8; 32],
    inbox_sequence: u128,
    credit_id: [u8; 32],
    payment_digest: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.offline-cash.v1.acknowledgement-signing-preimage")]
struct AcknowledgementSigningPreimageV1 {
    domain: Vec<u8>,
    version: u16,
    request_digest: [u8; 32],
    payment_digest: [u8; 32],
    inbox_receipt: OfflineCashInboxReceiptV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.offline-cash.v1.mint-credit-id-preimage")]
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
#[norito(schema_name = "iroha.offline-cash.v1.mint-lifecycle-context-preimage")]
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
    operation_kind: OfflineCashOperationKindV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.offline-cash.v1.redemption-id-preimage")]
struct RedemptionIdPreimageV1 {
    lifecycle_binding_digest: [u8; 32],
    terminal_nullifier: [u8; 32],
    amount: u128,
    beneficiary: AccountId,
    redemption_commitment: [u8; 32],
}

fn invalid(field: &'static str) -> OfflineCashValidationErrorV1 {
    OfflineCashValidationErrorV1::InvalidField { field }
}

fn digest_encoded<T: Encode>(
    domain: &[u8],
    value: &T,
) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
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

fn acceptance_intent_circuit_transcript_v1(intent: &OfflineCashAcceptanceIntentV1) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(114);
    bytes.extend_from_slice(&intent.version.to_le_bytes());
    bytes.extend_from_slice(&intent.request_digest);
    bytes.extend_from_slice(&intent.intent_id);
    bytes.extend_from_slice(&intent.exact_amount.to_le_bytes());
    bytes.extend_from_slice(&intent.sender_one_time_commitment);
    bytes
}

fn acceptance_intent_authorization_statement_circuit_transcript_v1(
    statement: &OfflineCashAcceptanceIntentAuthorizationStatementV1,
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(244);
    bytes.extend_from_slice(&statement.version.to_le_bytes());
    bytes.extend_from_slice(&acceptance_intent_circuit_transcript_v1(&statement.intent));
    bytes.extend_from_slice(&statement.release_id);
    bytes.extend_from_slice(&statement.suite_id);
    bytes.extend_from_slice(&statement.vk_digest);
    bytes.extend_from_slice(&statement.artifact_manifest_digest);
    bytes
}

fn no_commit_closure_statement_circuit_transcript_v1(
    statement: &OfflineCashNoCommitClosureStatementV1,
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(498);
    bytes.extend_from_slice(&statement.version.to_le_bytes());
    bytes.extend_from_slice(&statement.release_id);
    bytes.extend_from_slice(&statement.suite_id);
    bytes.extend_from_slice(&statement.vk_digest);
    bytes.extend_from_slice(&statement.artifact_manifest_digest);
    bytes.extend_from_slice(&statement.sender_hardware_binding_commitment);
    bytes.extend_from_slice(&statement.request_id);
    bytes.extend_from_slice(&statement.request_digest);
    bytes.extend_from_slice(&statement.acceptance_ticket_id);
    bytes.extend_from_slice(&statement.ticket_digest);
    bytes.extend_from_slice(&statement.intent_authorization_digest);
    bytes.extend_from_slice(&statement.intent_digest);
    bytes.extend_from_slice(&statement.exact_amount.to_le_bytes());
    bytes.extend_from_slice(&statement.sender_one_time_commitment);
    bytes.extend_from_slice(&statement.recovery_id);
    bytes.extend_from_slice(&statement.cancellation_nullifier);
    bytes.extend_from_slice(&statement.equivalent_delivery_slot_commitment);
    bytes
}

const fn offline_cash_operation_kind_tag_v1(operation: OfflineCashOperationKindV1) -> u32 {
    match operation {
        OfflineCashOperationKindV1::Bootstrap => 0,
        OfflineCashOperationKindV1::MintFold => 1,
        OfflineCashOperationKindV1::SendSplit => 2,
        OfflineCashOperationKindV1::ReceiveFoldBatch => 3,
        OfflineCashOperationKindV1::RedeemSplit => 4,
        OfflineCashOperationKindV1::SuiteUpgrade => 5,
        OfflineCashOperationKindV1::Rotate => 6,
    }
}

fn outbox_reservation_circuit_transcript_v1(
    reservation: OfflineCashOutboxReservationV1,
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(56);
    bytes.extend_from_slice(&reservation.reservation_id);
    bytes.extend_from_slice(
        &offline_cash_operation_kind_tag_v1(reservation.operation_kind).to_le_bytes(),
    );
    bytes.extend_from_slice(&reservation.reserved_outbox_bytes.to_le_bytes());
    bytes.extend_from_slice(&reservation.issued_at_ms.to_le_bytes());
    bytes.extend_from_slice(&reservation.expires_at_ms.to_le_bytes());
    bytes
}

fn commit_evidence_circuit_transcript_v1(evidence: OfflineCashCommitEvidenceV1) -> [u8; 36] {
    let (tag, commitment) = match evidence {
        OfflineCashCommitEvidenceV1::TrustedTime(value) => (0_u32, value.time_evidence_commitment),
        OfflineCashCommitEvidenceV1::MonotonicLease(value) => {
            (1_u32, value.lease_evidence_commitment)
        }
    };
    let mut bytes = [0_u8; 36];
    bytes[..4].copy_from_slice(&tag.to_le_bytes());
    bytes[4..].copy_from_slice(&commitment);
    bytes
}

fn commit_certificate_id_circuit_transcript_v1(
    certificate: &OfflineCashCommitCertificateV1,
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

fn commit_certificate_circuit_transcript_v1(
    certificate: &OfflineCashCommitCertificateV1,
) -> Vec<u8> {
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

/// Derive the canonical digest bound to an encrypted Offline Cash credit.
///
/// This hash is a wire/codec operation. It does not encrypt, decrypt, or prove
/// the opening and therefore is never sufficient for monetary admission.
#[must_use]
pub fn offline_cash_ciphertext_digest_v1(bytes: &[u8]) -> [u8; 32] {
    digest_bytes(CIPHERTEXT_DIGEST_DOMAIN, bytes)
}

fn require_valid_x25519_public_key(
    field: &'static str,
    value: [u8; OFFLINE_CASH_X25519_PUBLIC_KEY_BYTES_V1],
) -> Result<(), OfflineCashValidationErrorV1> {
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
pub fn offline_cash_encrypted_credit_kdf_salt_v1(
    recipient_x25519_public_key: [u8; OFFLINE_CASH_X25519_PUBLIC_KEY_BYTES_V1],
    ephemeral_x25519_public_key: [u8; OFFLINE_CASH_X25519_PUBLIC_KEY_BYTES_V1],
) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
    require_valid_x25519_public_key(
        "offline_cash.encrypted_credit.recipient_x25519_public_key",
        recipient_x25519_public_key,
    )?;
    require_valid_x25519_public_key(
        "offline_cash.encrypted_credit.ephemeral_x25519_public_key",
        ephemeral_x25519_public_key,
    )?;
    let mut hasher = Sha256::new();
    hasher.update(OFFLINE_CASH_ENCRYPTED_CREDIT_KDF_SALT_LABEL_V1);
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
pub fn offline_cash_encrypted_credit_kdf_info_v1(
    aad: &OfflineCashEncryptedCreditAadV1,
) -> Result<Vec<u8>, OfflineCashValidationErrorV1> {
    let aad_digest = aad.canonical_digest()?;
    let mut info = Vec::with_capacity(
        OFFLINE_CASH_ENCRYPTED_CREDIT_KDF_INFO_LABEL_V1.len() + aad_digest.len(),
    );
    info.extend_from_slice(OFFLINE_CASH_ENCRYPTED_CREDIT_KDF_INFO_LABEL_V1);
    info.extend_from_slice(&aad_digest);
    Ok(info)
}

fn require_nonzero(
    field: &'static str,
    value: [u8; 32],
) -> Result<(), OfflineCashValidationErrorV1> {
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
) -> Result<(), OfflineCashValidationErrorV1> {
    if version != OFFLINE_CASH_WIRE_VERSION_V1
        || network_id.as_bytes() == &[0; 32]
        || scale > OFFLINE_CASH_ASSET_SCALE_MAX_V1
        || amount.is_some_and(|value| value == 0)
    {
        return Err(invalid(field));
    }
    Ok(())
}

fn require_encoded_size<T: Encode>(
    value: &T,
    max: usize,
) -> Result<usize, OfflineCashValidationErrorV1> {
    let actual = norito::encode_canonical(value)?.len();
    if actual > max {
        return Err(OfflineCashValidationErrorV1::EncodedSizeExceeded { actual, max });
    }
    Ok(actual)
}

/// Decode one already byte-capped canonical frame under resource limits that
/// are installed before derive-generated sequence decoders can reserve space.
fn decode_bounded_canonical<T>(bytes: &[u8], max: usize) -> Result<T, OfflineCashValidationErrorV1>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    if bytes.len() > max {
        return Err(OfflineCashValidationErrorV1::EncodedSizeExceeded {
            actual: bytes.len(),
            max,
        });
    }
    let limits = norito::canonical_decode_limits(bytes.len());
    Ok(norito::decode_canonical_with_limits(bytes, limits)?)
}

fn encode_offline_cash_text_v1<T: Encode>(
    value: &T,
    raw_max: usize,
    text_max: usize,
) -> Result<String, OfflineCashValidationErrorV1> {
    let raw = norito::encode_canonical(value)?;
    if raw.len() > raw_max {
        return Err(OfflineCashValidationErrorV1::EncodedSizeExceeded {
            actual: raw.len(),
            max: raw_max,
        });
    }
    let mut text = String::with_capacity(
        OFFLINE_CASH_TEXT_PREFIX_V1.len() + unpadded_base64url_len(raw.len()),
    );
    text.push_str(OFFLINE_CASH_TEXT_PREFIX_V1);
    URL_SAFE_NO_PAD.encode_string(raw, &mut text);
    if text.len() > text_max {
        return Err(OfflineCashValidationErrorV1::EncodedSizeExceeded {
            actual: text.len(),
            max: text_max,
        });
    }
    Ok(text)
}

fn decode_offline_cash_text_payload_v1(
    text: &str,
    raw_max: usize,
    text_max: usize,
) -> Result<Vec<u8>, OfflineCashValidationErrorV1> {
    if text.len() > text_max {
        return Err(OfflineCashValidationErrorV1::EncodedSizeExceeded {
            actual: text.len(),
            max: text_max,
        });
    }
    if text.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Err(invalid("offline_cash.text.whitespace"));
    }
    let body = text
        .strip_prefix(OFFLINE_CASH_TEXT_PREFIX_V1)
        .ok_or_else(|| invalid("offline_cash.text.prefix"))?;
    if body.is_empty() {
        return Err(invalid("offline_cash.text.body"));
    }
    if body.contains('=') {
        return Err(invalid("offline_cash.text.padding"));
    }
    if !body
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(invalid("offline_cash.text.base64url"));
    }
    let raw = URL_SAFE_NO_PAD
        .decode(body.as_bytes())
        .map_err(|_| invalid("offline_cash.text.base64url"))?;
    if raw.len() > raw_max {
        return Err(OfflineCashValidationErrorV1::EncodedSizeExceeded {
            actual: raw.len(),
            max: raw_max,
        });
    }
    if URL_SAFE_NO_PAD.encode(&raw) != body {
        return Err(invalid("offline_cash.text.base64url"));
    }
    Ok(raw)
}

fn decode_offline_cash_text_v1<T, F>(
    text: &str,
    raw_max: usize,
    text_max: usize,
    decode: F,
) -> Result<T, OfflineCashValidationErrorV1>
where
    T: Encode,
    F: FnOnce(&[u8]) -> Result<T, OfflineCashValidationErrorV1>,
{
    let raw = decode_offline_cash_text_payload_v1(text, raw_max, text_max)?;
    let value = decode(&raw)?;
    if encode_offline_cash_text_v1(&value, raw_max, text_max)? != text {
        return Err(invalid("offline_cash.text.canonical"));
    }
    Ok(value)
}

/// Derive the stable reference to an Offline Cash device key.
#[must_use]
pub fn offline_cash_device_key_reference_v1(public_key: &OfflineCashDevicePublicKeyV1) -> [u8; 32] {
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
pub fn offline_cash_asset_identity_digest_v1(
    asset: &AssetDefinitionId,
) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
    digest_encoded(ASSET_IDENTITY_DIGEST_DOMAIN, asset)
}

/// Derive the sole reserve-liability pool for one network, asset, and incarnation.
///
/// # Errors
///
/// Returns an error when the canonical pool preimage cannot be encoded.
pub fn offline_cash_liability_pool_id_v1(
    network_id: &NetworkId,
    asset: &AssetDefinitionId,
    asset_incarnation: AxtAssetIncarnationV1,
) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
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
/// into AEAD plaintext or associated data. The final lifecycle and wrapper
/// separately bind the exact ciphertext digest to this commitment and opening.
///
/// # Errors
///
/// Returns an error when the canonical identity preimage cannot be encoded.
#[allow(clippy::too_many_arguments)]
pub fn offline_cash_credit_id_v1(
    transition_nullifier: [u8; 32],
    request_digest: [u8; 32],
    acceptance_ticket_digest: [u8; 32],
    recipient_one_time_key: [u8; 32],
    amount: u128,
    ciphertext_commitment: [u8; 32],
) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
    digest_encoded(
        CREDIT_ID_DOMAIN,
        &CreditIdPreimageV1 {
            transition_nullifier,
            request_digest,
            acceptance_ticket_digest,
            recipient_one_time_key,
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
pub fn offline_cash_credit_opening_canonical_len_v1() -> Result<usize, OfflineCashValidationErrorV1>
{
    Ok(norito::encode_canonical(&OfflineCashCreditOpeningV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        credit_id: [1; 32],
        amount: 1,
        credit_commitment_opening: [2; 32],
        recipient_binding_opening: [3; 32],
        recovery_nonce: [4; 32],
    })?
    .len())
}

impl OfflineCashCreditOpeningV1 {
    /// Validate the fixed private plaintext independently of public context.
    ///
    /// # Errors
    ///
    /// Returns an error for a reserved version, zero amount, zero opening, or
    /// an unexpected canonical fixed-size encoding.
    pub fn validate_shape(&self) -> Result<(), OfflineCashValidationErrorV1> {
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1 || self.amount == 0 {
            return Err(invalid("offline_cash.credit_opening.header"));
        }
        for (field, value) in [
            ("offline_cash.credit_opening.credit_id", self.credit_id),
            (
                "offline_cash.credit_opening.credit_commitment_opening",
                self.credit_commitment_opening,
            ),
            (
                "offline_cash.credit_opening.recipient_binding_opening",
                self.recipient_binding_opening,
            ),
            (
                "offline_cash.credit_opening.recovery_nonce",
                self.recovery_nonce,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        let actual = require_encoded_size(self, OFFLINE_CASH_CREDIT_OPENING_MAX_BYTES_V1)?;
        if actual != offline_cash_credit_opening_canonical_len_v1()? {
            return Err(invalid("offline_cash.credit_opening.encoded_length"));
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
    ) -> Result<(), OfflineCashValidationErrorV1> {
        self.validate_shape()?;
        if self.credit_id != credit_id || self.amount != amount {
            return Err(invalid("offline_cash.credit_opening.public_binding"));
        }
        Ok(())
    }

    /// Encode this validated fixed plaintext canonically for AEAD sealing.
    ///
    /// # Errors
    ///
    /// Returns an error when validation or canonical encoding fails.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, OfflineCashValidationErrorV1> {
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
    ) -> Result<Self, OfflineCashValidationErrorV1> {
        if bytes.len() != offline_cash_credit_opening_canonical_len_v1()? {
            return Err(invalid("offline_cash.credit_opening.encoded_length"));
        }
        let opening: Self =
            decode_bounded_canonical(bytes, OFFLINE_CASH_CREDIT_OPENING_MAX_BYTES_V1)?;
        opening.validate_shape_against(credit_id, amount)?;
        Ok(opening)
    }
}

fn peer_credit_lifecycle_context_digest_v1(
    lifecycle: &OfflineCashLifecycleBindingV1,
) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
    require_valid_header(
        lifecycle.version,
        &lifecycle.network_id,
        lifecycle.scale,
        None,
        "offline_cash.peer_credit_context.lifecycle_header",
    )?;
    lifecycle
        .asset_incarnation
        .validate()
        .map_err(|_| invalid("offline_cash.peer_credit_context.asset_incarnation"))?;
    for (field, value) in [
        (
            "offline_cash.peer_credit_context.suite_id",
            lifecycle.suite_id,
        ),
        (
            "offline_cash.peer_credit_context.vk_digest",
            lifecycle.vk_digest,
        ),
        (
            "offline_cash.peer_credit_context.release_id",
            lifecycle.release_id,
        ),
        (
            "offline_cash.peer_credit_context.liability_pool_id",
            lifecycle.liability_pool_id,
        ),
        (
            "offline_cash.peer_credit_context.hardware_profile_id",
            lifecycle.hardware_profile_id,
        ),
        (
            "offline_cash.peer_credit_context.request_id",
            lifecycle.request_id,
        ),
        (
            "offline_cash.peer_credit_context.acceptance_ticket_id",
            lifecycle.acceptance_ticket_id,
        ),
    ] {
        require_nonzero(field, value)?;
    }
    if lifecycle.protocol_version != OFFLINE_CASH_WIRE_VERSION_V1
        || lifecycle.policy_epoch == 0
        || lifecycle.operation_kind != OfflineCashOperationKindV1::SendSplit
        || lifecycle.liability_pool_id
            != offline_cash_liability_pool_id_v1(
                &lifecycle.network_id,
                &lifecycle.asset,
                lifecycle.asset_incarnation,
            )?
    {
        return Err(invalid("offline_cash.peer_credit_context.lifecycle"));
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
            acceptance_ticket_id: lifecycle.acceptance_ticket_id,
        },
    )
}

impl OfflineCashPeerCreditContextV1 {
    /// Validate the exact pre-encryption request, intent, ticket, and lifecycle projection.
    ///
    /// # Errors
    ///
    /// Returns an error for a reserved digest, invalid recipient key, or version.
    pub fn validate_shape(&self) -> Result<(), OfflineCashValidationErrorV1> {
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1 {
            return Err(invalid("offline_cash.peer_credit_context.version"));
        }
        for (field, value) in [
            (
                "offline_cash.peer_credit_context.request_digest",
                self.request_digest,
            ),
            (
                "offline_cash.peer_credit_context.acceptance_intent_digest",
                self.acceptance_intent_digest,
            ),
            (
                "offline_cash.peer_credit_context.acceptance_ticket_digest",
                self.acceptance_ticket_digest,
            ),
            (
                "offline_cash.peer_credit_context.lifecycle_context_digest",
                self.lifecycle_context_digest,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        require_valid_x25519_public_key(
            "offline_cash.peer_credit_context.recipient_one_time_key",
            self.recipient_one_time_key,
        )
    }

    /// Return the exact pre-ID peer context digest carried in AEAD associated data.
    ///
    /// # Errors
    ///
    /// Returns an error when validation or canonical encoding fails.
    pub fn canonical_digest(&self) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        self.validate_shape()?;
        digest_encoded(PEER_CREDIT_CONTEXT_DIGEST_DOMAIN, self)
    }
}

impl OfflineCashEncryptedCreditAadV1 {
    /// Validate exact acyclic encrypted-credit associated data.
    ///
    /// # Errors
    ///
    /// Returns an error for a reserved version, digest, commitment, credit ID,
    /// or zero amount.
    pub fn validate_shape(&self) -> Result<(), OfflineCashValidationErrorV1> {
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1 || self.amount == 0 {
            return Err(invalid("offline_cash.encrypted_credit_aad.header"));
        }
        for (field, value) in [
            (
                "offline_cash.encrypted_credit_aad.context_digest",
                self.context_digest,
            ),
            (
                "offline_cash.encrypted_credit_aad.issuance_or_transition_commitment",
                self.issuance_or_transition_commitment,
            ),
            (
                "offline_cash.encrypted_credit_aad.credit_id",
                self.credit_id,
            ),
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
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, OfflineCashValidationErrorV1> {
        self.validate_shape()?;
        Ok(norito::encode_canonical(self)?)
    }

    /// Return `SHA256(canonical_aad)`, the suffix of the HKDF info string.
    ///
    /// # Errors
    ///
    /// Returns an error when validation or canonical encoding fails.
    pub fn canonical_digest(&self) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        let bytes = self.canonical_bytes()?;
        Ok(Sha256::digest(bytes).into())
    }

    /// Construct the exact AAD for a post-encryption mint authorization.
    ///
    /// # Errors
    ///
    /// Returns an error when the authorization statement is invalid.
    pub fn for_mint(
        statement: &OfflineCashMintAuthorizationStatementV1,
    ) -> Result<Self, OfflineCashValidationErrorV1> {
        statement.validate_shape()?;
        Ok(Self {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            purpose: OfflineCashEncryptedCreditPurposeV1::Mint,
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
    /// sender intent, receiver ticket, and all lifecycle fields available
    /// before credit ID and ciphertext derivation.
    ///
    /// # Errors
    ///
    /// Returns an error for any invalid or substituted peer context.
    pub fn for_peer(
        statement: &OfflineCashTransferStatementV1,
        request: &OfflineCashPaymentRequestV1,
        intent: &OfflineCashAcceptanceIntentV1,
        ticket: &OfflineCashAcceptanceTicketV1,
    ) -> Result<Self, OfflineCashValidationErrorV1> {
        let context = statement.peer_credit_context_against(request, intent, ticket)?;
        Ok(Self {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            purpose: OfflineCashEncryptedCreditPurposeV1::Peer,
            context_digest: context.canonical_digest()?,
            issuance_or_transition_commitment: statement.ciphertext_commitment,
            credit_id: statement.lifecycle.credit_id,
            amount: statement.amount,
        })
    }
}

impl OfflineCashEncryptedCreditEnvelopeV1 {
    /// Validate the exact canonical envelope shape without performing AEAD open.
    ///
    /// # Errors
    ///
    /// Returns an error for a reserved version, low-order ephemeral key, wrong
    /// fixed ciphertext size, or oversized encoding. Nonce freshness and
    /// randomness are enforced by qualified hardware and cannot be inferred
    /// from one standalone envelope.
    pub fn validate_shape(&self) -> Result<(), OfflineCashValidationErrorV1> {
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1 {
            return Err(invalid("offline_cash.encrypted_credit.version"));
        }
        require_valid_x25519_public_key(
            "offline_cash.encrypted_credit.ephemeral_x25519_public_key",
            self.ephemeral_x25519_public_key,
        )?;
        let expected = offline_cash_credit_opening_canonical_len_v1()?
            .checked_add(OFFLINE_CASH_XCHACHA20POLY1305_TAG_BYTES_V1)
            .ok_or_else(|| invalid("offline_cash.encrypted_credit.ciphertext_and_tag"))?;
        if self.ciphertext_and_tag.len() != expected {
            return Err(invalid("offline_cash.encrypted_credit.ciphertext_and_tag"));
        }
        require_encoded_size(self, OFFLINE_CASH_ENCRYPTED_CREDIT_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Validate envelope shape plus the externally signed recipient X25519 key.
    ///
    /// # Errors
    ///
    /// Returns an error when either envelope shape or recipient key is invalid.
    pub fn validate_shape_against_recipient_key(
        &self,
        recipient_x25519_public_key: [u8; OFFLINE_CASH_X25519_PUBLIC_KEY_BYTES_V1],
    ) -> Result<(), OfflineCashValidationErrorV1> {
        self.validate_shape()?;
        require_valid_x25519_public_key(
            "offline_cash.encrypted_credit.recipient_x25519_public_key",
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
        recipient_x25519_public_key: [u8; OFFLINE_CASH_X25519_PUBLIC_KEY_BYTES_V1],
    ) -> Result<Vec<u8>, OfflineCashValidationErrorV1> {
        self.validate_shape_against_recipient_key(recipient_x25519_public_key)?;
        Ok(norito::encode_canonical(self)?)
    }

    /// Decode one exact canonical envelope without opening it.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized, malformed, non-canonical, or invalid bytes.
    pub fn decode_canonical_shape_exact(
        bytes: &[u8],
    ) -> Result<Self, OfflineCashValidationErrorV1> {
        let envelope: Self =
            decode_bounded_canonical(bytes, OFFLINE_CASH_ENCRYPTED_CREDIT_MAX_BYTES_V1)?;
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
        recipient_x25519_public_key: [u8; OFFLINE_CASH_X25519_PUBLIC_KEY_BYTES_V1],
    ) -> Result<Self, OfflineCashValidationErrorV1> {
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
        recipient_x25519_public_key: [u8; OFFLINE_CASH_X25519_PUBLIC_KEY_BYTES_V1],
    ) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        self.validate_shape_against_recipient_key(recipient_x25519_public_key)?;
        offline_cash_encrypted_credit_kdf_salt_v1(
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
pub fn offline_cash_mint_credit_opening_commitment_v1(
    network_id: &NetworkId,
    asset: &AssetDefinitionId,
    asset_incarnation: AxtAssetIncarnationV1,
    scale: u32,
    liability_pool_id: [u8; 32],
    amount: u128,
    recipient: &AccountId,
    recipient_one_time_key: [u8; OFFLINE_CASH_X25519_PUBLIC_KEY_BYTES_V1],
    credit_commitment_opening: [u8; 32],
) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
    require_valid_header(
        OFFLINE_CASH_WIRE_VERSION_V1,
        network_id,
        scale,
        Some(amount),
        "offline_cash.mint_credit_opening_commitment.header",
    )?;
    asset_incarnation
        .validate()
        .map_err(|_| invalid("offline_cash.mint_credit_opening_commitment.asset_incarnation"))?;
    require_nonzero(
        "offline_cash.mint_credit_opening_commitment.liability_pool_id",
        liability_pool_id,
    )?;
    require_nonzero(
        "offline_cash.mint_credit_opening_commitment.credit_commitment_opening",
        credit_commitment_opening,
    )?;
    require_valid_x25519_public_key(
        "offline_cash.mint_credit_opening_commitment.recipient_one_time_key",
        recipient_one_time_key,
    )?;
    if liability_pool_id != offline_cash_liability_pool_id_v1(network_id, asset, asset_incarnation)?
    {
        return Err(invalid(
            "offline_cash.mint_credit_opening_commitment.liability_pool_id",
        ));
    }
    digest_encoded(
        MINT_CREDIT_OPENING_COMMITMENT_DOMAIN,
        &MintCreditOpeningCommitmentPreimageV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            network_id: *network_id.as_bytes(),
            asset_identity_digest: offline_cash_asset_identity_digest_v1(asset)?,
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
pub fn offline_cash_recipient_credential_commitment_v1(
    operation_id: [u8; 32],
    hardware_credential_id: [u8; 32],
    recipient_binding_opening: [u8; 32],
) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
    for (field, value) in [
        (
            "offline_cash.recipient_credential_commitment.operation_id",
            operation_id,
        ),
        (
            "offline_cash.recipient_credential_commitment.hardware_credential_id",
            hardware_credential_id,
        ),
        (
            "offline_cash.recipient_credential_commitment.recipient_binding_opening",
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
pub fn offline_cash_inbox_receipt_commitment_v1(
    recipient_lane_id: [u8; 32],
    staging_hardware_epoch_id: [u8; 32],
    inbox_sequence: u128,
    credit_id: [u8; 32],
    payment_digest: [u8; 32],
) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
    for (field, value) in [
        (
            "offline_cash.inbox_receipt.recipient_lane_id",
            recipient_lane_id,
        ),
        (
            "offline_cash.inbox_receipt.staging_hardware_epoch_id",
            staging_hardware_epoch_id,
        ),
        ("offline_cash.inbox_receipt.credit_id", credit_id),
        ("offline_cash.inbox_receipt.payment_digest", payment_digest),
    ] {
        require_nonzero(field, value)?;
    }
    if inbox_sequence == 0 {
        return Err(invalid("offline_cash.inbox_receipt.inbox_sequence"));
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

impl OfflineCashHardwareProfileV1 {
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
    pub fn expected_hardware_profile_id(&self) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        digest_encoded(HARDWARE_PROFILE_DIGEST_DOMAIN, &self.id_preimage())
    }

    /// Populate the canonical profile identity.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical profile-body encoding fails.
    pub fn seal_hardware_profile_id(mut self) -> Result<Self, OfflineCashValidationErrorV1> {
        self.hardware_profile_id = self.expected_hardware_profile_id()?;
        Ok(self)
    }

    /// Validate the exact governed capability set and profile lifetime.
    ///
    /// # Errors
    ///
    /// Returns an error for a reserved identity, incomplete/unknown capability
    /// set, invalid issuer key, invalid lifetime, or oversized encoding.
    pub fn validate(&self) -> Result<(), OfflineCashValidationErrorV1> {
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1
            || self.protocol_version != OFFLINE_CASH_WIRE_VERSION_V1
            || self.policy_epoch == 0
            || self.capability_mask != OFFLINE_CASH_HARDWARE_REQUIRED_CAPABILITIES_V1
            || self.valid_from_ms >= self.expires_at_ms
        {
            return Err(invalid("offline_cash.hardware_profile.header"));
        }
        for (field, value) in [
            (
                "offline_cash.hardware_profile.hardware_profile_id",
                self.hardware_profile_id,
            ),
            (
                "offline_cash.hardware_profile.provider_id",
                self.provider_id,
            ),
            (
                "offline_cash.hardware_profile.firmware_policy_digest",
                self.firmware_policy_digest,
            ),
            (
                "offline_cash.hardware_profile.product_class_digest",
                self.product_class_digest,
            ),
            (
                "offline_cash.hardware_profile.enrollment_attestation_verifier_digest",
                self.enrollment_attestation_verifier_digest,
            ),
            (
                "offline_cash.hardware_profile.attestation_trust_roots_digest",
                self.attestation_trust_roots_digest,
            ),
            (
                "offline_cash.hardware_profile.allowed_suite_commitment",
                self.allowed_suite_commitment,
            ),
            (
                "offline_cash.hardware_profile.qualification_report_digest",
                self.qualification_report_digest,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        self.governance_credential_public_key.validate()?;
        if self.hardware_profile_id != self.expected_hardware_profile_id()? {
            return Err(invalid("offline_cash.hardware_profile.hardware_profile_id"));
        }
        require_encoded_size(self, OFFLINE_CASH_HARDWARE_PROFILE_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Return the canonical governed profile digest.
    ///
    /// # Errors
    ///
    /// Returns an error when the profile is invalid or cannot be encoded.
    pub fn canonical_digest(&self) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        self.validate()?;
        Ok(self.hardware_profile_id)
    }
}

impl OfflineCashHardwareCredentialV1 {
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
    pub fn expected_credential_id(&self) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        digest_encoded(HARDWARE_CREDENTIAL_ID_DOMAIN, &self.id_preimage())
    }

    /// Populate the canonical credential identity before governance signs it.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical identity encoding fails.
    pub fn seal_credential_id(mut self) -> Result<Self, OfflineCashValidationErrorV1> {
        self.credential_id = self.expected_credential_id()?;
        Ok(self)
    }

    /// Return the exact bytes signed by the governed profile issuer.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, OfflineCashValidationErrorV1> {
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
    pub fn validate_shape(&self) -> Result<(), OfflineCashValidationErrorV1> {
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1
            || self.network_id.as_bytes() == &[0; 32]
            || self.policy_epoch == 0
            || self.issued_at_ms >= self.expires_at_ms
        {
            return Err(invalid("offline_cash.hardware_credential.header"));
        }
        for (field, value) in [
            (
                "offline_cash.hardware_credential.credential_id",
                self.credential_id,
            ),
            (
                "offline_cash.hardware_credential.hardware_profile_id",
                self.hardware_profile_id,
            ),
            ("offline_cash.hardware_credential.suite_id", self.suite_id),
            (
                "offline_cash.hardware_credential.firmware_policy_digest",
                self.firmware_policy_digest,
            ),
            (
                "offline_cash.hardware_credential.lane_commitment",
                self.lane_commitment,
            ),
            (
                "offline_cash.hardware_credential.hardware_epoch_id",
                self.hardware_epoch_id,
            ),
            (
                "offline_cash.hardware_credential.device_key_reference",
                self.device_key_reference,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        self.device_public_key.validate()?;
        if self.device_key_reference
            != offline_cash_device_key_reference_v1(&self.device_public_key)
            || self.credential_id != self.expected_credential_id()?
        {
            return Err(invalid("offline_cash.hardware_credential.identity"));
        }
        require_encoded_size(self, OFFLINE_CASH_HARDWARE_CREDENTIAL_MAX_BYTES_V1)?;
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
        profile: &OfflineCashHardwareProfileV1,
    ) -> Result<(), OfflineCashValidationErrorV1> {
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
            return Err(invalid("offline_cash.hardware_credential.profile_binding"));
        }
        self.governance_signature.verify(
            &profile.governance_credential_public_key,
            &self.canonical_signing_bytes()?,
        )
    }
}

impl OfflineCashAmountPolicyV1 {
    /// Validate a non-empty inclusive amount interval.
    ///
    /// # Errors
    ///
    /// Returns an error when zero or an inverted interval is present.
    pub fn validate(self) -> Result<(), OfflineCashValidationErrorV1> {
        if self.minimum_amount == 0 || self.minimum_amount > self.maximum_amount {
            return Err(invalid("offline_cash.amount_policy"));
        }
        Ok(())
    }

    /// Return whether this interval authorizes `amount`.
    #[must_use]
    pub const fn contains(self, amount: u128) -> bool {
        amount >= self.minimum_amount && amount <= self.maximum_amount
    }
}

impl OfflineCashPaymentRequestModeV1 {
    /// Validate the mode's positive amount/count policy.
    ///
    /// # Errors
    ///
    /// Returns an error for zero amounts/counts or malformed per-payment ranges.
    pub fn validate(self) -> Result<(), OfflineCashValidationErrorV1> {
        match self {
            Self::SingleExact(policy) if policy.amount > 0 => Ok(()),
            Self::PartialUntilTotal(policy) if policy.total_amount > 0 => Ok(()),
            Self::BoundedMultiPayment(policy) if policy.max_payments > 0 => {
                policy.per_payment.validate()
            }
            Self::OpenReceive(policy) => policy.per_payment.validate(),
            _ => Err(invalid("offline_cash.request_mode")),
        }
    }

    /// Return whether this request mode permits one exact ticket amount.
    ///
    /// This is the stateless per-ticket predicate only. Receiver hardware must
    /// additionally apply its atomic private request ledger so a collection of
    /// unresolved and consumed tickets cannot exceed a total or count bound.
    #[must_use]
    pub const fn accepts_exact_amount(self, amount: u128) -> bool {
        if amount == 0 {
            return false;
        }
        match self {
            Self::SingleExact(request) => amount == request.amount,
            Self::PartialUntilTotal(request) => amount <= request.total_amount,
            Self::BoundedMultiPayment(request) => request.per_payment.contains(amount),
            Self::OpenReceive(request) => request.per_payment.contains(amount),
        }
    }

    /// Return the canonical digest repeated by tickets and proofs.
    ///
    /// # Errors
    ///
    /// Returns an error when the mode is invalid or cannot be encoded.
    pub fn canonical_digest(self) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        self.validate()?;
        digest_encoded(REQUEST_MODE_DIGEST_DOMAIN, &self)
    }
}

impl OfflineCashAcceptanceIntentV1 {
    /// Validate this one-use sender intent against the exact signed request.
    ///
    /// This is a shape check. Receiver hardware must atomically reserve the
    /// amount/count in its private request-mode ledger when it issues a ticket.
    ///
    /// # Errors
    ///
    /// Returns an error for a wrong request, invalid amount, reserved identity,
    /// malformed commitment, or oversized encoding.
    pub fn validate_shape_against(
        &self,
        request: &OfflineCashPaymentRequestV1,
    ) -> Result<(), OfflineCashValidationErrorV1> {
        request.validate_shape()?;
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1
            || self.request_digest != request.canonical_digest()?
            || self.intent_id == [0; 32]
            || self.sender_one_time_commitment == [0; 32]
            || !request.request_mode.accepts_exact_amount(self.exact_amount)
        {
            return Err(invalid("offline_cash.acceptance_intent.binding"));
        }
        require_encoded_size(self, OFFLINE_CASH_ACCEPTANCE_INTENT_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Return the canonical intent digest signed into the acceptance ticket.
    ///
    /// # Errors
    ///
    /// Returns an error when the intent is invalid or cannot be encoded.
    pub fn canonical_digest_against(
        &self,
        request: &OfflineCashPaymentRequestV1,
    ) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        self.validate_shape_against(request)?;
        Ok(digest_bytes(
            ACCEPTANCE_INTENT_DIGEST_DOMAIN,
            &acceptance_intent_circuit_transcript_v1(self),
        ))
    }

    /// Encode this validated intent as canonical unpadded `oc1:` base64url text.
    ///
    /// # Errors
    ///
    /// Returns an error when validation, encoding, or a size bound fails.
    pub fn encode_text_against(
        &self,
        request: &OfflineCashPaymentRequestV1,
    ) -> Result<String, OfflineCashValidationErrorV1> {
        self.validate_shape_against(request)?;
        encode_offline_cash_text_v1(
            self,
            OFFLINE_CASH_ACCEPTANCE_INTENT_MAX_BYTES_V1,
            OFFLINE_CASH_ACCEPTANCE_INTENT_TEXT_MAX_BYTES_V1,
        )
    }

    /// Decode one exact canonical intent against its signed request.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid context, size, framing, or canonical bytes.
    pub fn decode_text_exact_against(
        text: &str,
        request: &OfflineCashPaymentRequestV1,
    ) -> Result<Self, OfflineCashValidationErrorV1> {
        decode_offline_cash_text_v1(
            text,
            OFFLINE_CASH_ACCEPTANCE_INTENT_MAX_BYTES_V1,
            OFFLINE_CASH_ACCEPTANCE_INTENT_TEXT_MAX_BYTES_V1,
            |bytes| Self::decode_canonical_shape_exact_against(bytes, request),
        )
    }

    /// Decode one exact bounded canonical intent against its request.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized, malformed, non-canonical, or invalid bytes.
    pub fn decode_canonical_shape_exact_against(
        bytes: &[u8],
        request: &OfflineCashPaymentRequestV1,
    ) -> Result<Self, OfflineCashValidationErrorV1> {
        let intent: Self =
            decode_bounded_canonical(bytes, OFFLINE_CASH_ACCEPTANCE_INTENT_MAX_BYTES_V1)?;
        intent.validate_shape_against(request)?;
        Ok(intent)
    }
}

impl OfflineCashAcceptanceIntentAuthorizationStatementV1 {
    /// Validate the release-wide authorization statement against one request.
    ///
    /// This exposes no sender profile or credential identifier. The paired
    /// circuit must prove that the private sender credential belongs to an
    /// enabled profile under `release_id` and is admitted by the exact suite,
    /// verifying-key set, and artifact manifest named here.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid intent or release binding.
    pub fn validate_shape_against(
        &self,
        request: &OfflineCashPaymentRequestV1,
    ) -> Result<(), OfflineCashValidationErrorV1> {
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1
            || self.release_id != request.release_id
            || self.suite_id != request.hardware_credential.suite_id
        {
            return Err(invalid(
                "offline_cash.acceptance_intent_authorization_statement.context",
            ));
        }
        self.intent.validate_shape_against(request)?;
        for (field, value) in [
            (
                "offline_cash.acceptance_intent_authorization_statement.release_id",
                self.release_id,
            ),
            (
                "offline_cash.acceptance_intent_authorization_statement.suite_id",
                self.suite_id,
            ),
            (
                "offline_cash.acceptance_intent_authorization_statement.vk_digest",
                self.vk_digest,
            ),
            (
                "offline_cash.acceptance_intent_authorization_statement.artifact_manifest_digest",
                self.artifact_manifest_digest,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        Ok(())
    }

    /// Return the exact semantic digest constrained by both proof parities.
    ///
    /// # Errors
    ///
    /// Returns an error when shape validation or canonical encoding fails.
    pub fn canonical_digest_against(
        &self,
        request: &OfflineCashPaymentRequestV1,
    ) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        self.validate_shape_against(request)?;
        Ok(digest_bytes(
            ACCEPTANCE_INTENT_AUTHORIZATION_STATEMENT_DIGEST_DOMAIN,
            &acceptance_intent_authorization_statement_circuit_transcript_v1(self),
        ))
    }
}

impl OfflineCashAcceptanceIntentAuthorizationV1 {
    /// Return the compact intent embedded later in the payment.
    #[must_use]
    pub const fn intent(&self) -> OfflineCashAcceptanceIntentV1 {
        self.statement.intent
    }

    /// Validate the bounded proof envelope against the exact signed request.
    ///
    /// This is deliberately a shape and cross-field check only. Receiver
    /// hardware must authenticate the release and artifact manifest, then
    /// cryptographically verify both proof parities before it records the
    /// intent or reserves request budget and inbox capacity.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid statement, proof binding, version, or
    /// encoded-size bound.
    pub fn validate_shape_against(
        &self,
        request: &OfflineCashPaymentRequestV1,
    ) -> Result<(), OfflineCashValidationErrorV1> {
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1 {
            return Err(invalid(
                "offline_cash.acceptance_intent_authorization.version",
            ));
        }
        if self.statement.version != self.version {
            return Err(invalid(
                "offline_cash.acceptance_intent_authorization.statement_version",
            ));
        }
        let authorization_statement_digest = self.statement.canonical_digest_against(request)?;
        self.proof
            .validate_shape_for_semantic_digest(authorization_statement_digest)?;
        require_encoded_size(
            self,
            OFFLINE_CASH_ACCEPTANCE_INTENT_AUTHORIZATION_MAX_BYTES_V1,
        )?;
        Ok(())
    }

    /// Return the digest of the complete pre-ticket authorization envelope.
    ///
    /// The acceptance ticket still signs the compact intent digest, so the
    /// large proof does not become part of the payment. Receiver hardware may
    /// use this envelope digest in its private atomic reservation journal.
    ///
    /// # Errors
    ///
    /// Returns an error when shape validation or canonical encoding fails.
    pub fn canonical_digest_against(
        &self,
        request: &OfflineCashPaymentRequestV1,
    ) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        self.validate_shape_against(request)?;
        digest_encoded(ACCEPTANCE_INTENT_AUTHORIZATION_DIGEST_DOMAIN, self)
    }

    /// Encode one shape-validated authorization as canonical `oc1:` text.
    ///
    /// # Errors
    ///
    /// Returns an error when validation, encoding, or a size bound fails.
    pub fn encode_text_shape_against(
        &self,
        request: &OfflineCashPaymentRequestV1,
    ) -> Result<String, OfflineCashValidationErrorV1> {
        self.validate_shape_against(request)?;
        encode_offline_cash_text_v1(
            self,
            OFFLINE_CASH_ACCEPTANCE_INTENT_AUTHORIZATION_MAX_BYTES_V1,
            OFFLINE_CASH_ACCEPTANCE_INTENT_AUTHORIZATION_TEXT_MAX_BYTES_V1,
        )
    }

    /// Decode one exact canonical authorization without granting authority.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid context, size, framing, or canonical bytes.
    pub fn decode_text_shape_exact_against(
        text: &str,
        request: &OfflineCashPaymentRequestV1,
    ) -> Result<Self, OfflineCashValidationErrorV1> {
        decode_offline_cash_text_v1(
            text,
            OFFLINE_CASH_ACCEPTANCE_INTENT_AUTHORIZATION_MAX_BYTES_V1,
            OFFLINE_CASH_ACCEPTANCE_INTENT_AUTHORIZATION_TEXT_MAX_BYTES_V1,
            |bytes| Self::decode_canonical_shape_exact_against(bytes, request),
        )
    }

    /// Decode one exact bounded authorization without granting authority.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized, malformed, non-canonical, or invalid bytes.
    pub fn decode_canonical_shape_exact_against(
        bytes: &[u8],
        request: &OfflineCashPaymentRequestV1,
    ) -> Result<Self, OfflineCashValidationErrorV1> {
        let authorization: Self = decode_bounded_canonical(
            bytes,
            OFFLINE_CASH_ACCEPTANCE_INTENT_AUTHORIZATION_MAX_BYTES_V1,
        )?;
        authorization.validate_shape_against(request)?;
        Ok(authorization)
    }
}

impl OfflineCashNoCommitClosureStatementV1 {
    /// Validate the fixed unlinkable public shape.
    ///
    /// # Errors
    ///
    /// Returns an error for zero or substituted authority bindings.
    pub fn validate_shape(&self) -> Result<(), OfflineCashValidationErrorV1> {
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1 || self.exact_amount == 0 {
            return Err(invalid("offline_cash.no_commit_closure.shape"));
        }
        if [
            self.release_id,
            self.suite_id,
            self.vk_digest,
            self.artifact_manifest_digest,
            self.sender_hardware_binding_commitment,
            self.request_id,
            self.request_digest,
            self.acceptance_ticket_id,
            self.ticket_digest,
            self.intent_authorization_digest,
            self.intent_digest,
            self.sender_one_time_commitment,
            self.recovery_id,
            self.cancellation_nullifier,
            self.equivalent_delivery_slot_commitment,
        ]
        .contains(&[0; 32])
        {
            return Err(invalid("offline_cash.no_commit_closure.zero_binding"));
        }
        Ok(())
    }

    /// Return the canonical statement identity constrained by both proof parities.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed fields or canonical encoding failure.
    pub fn canonical_digest(&self) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        self.validate_shape()?;
        Ok(digest_bytes(
            NO_COMMIT_CLOSURE_STATEMENT_DIGEST_DOMAIN,
            &no_commit_closure_statement_circuit_transcript_v1(self),
        ))
    }
}

impl OfflineCashNoCommitClosureV1 {
    /// Validate the bounded paired proof against its exact statement.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, substituted, or oversized proof material.
    pub fn validate_shape(&self) -> Result<(), OfflineCashValidationErrorV1> {
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1 || self.statement.version != self.version {
            return Err(invalid("offline_cash.no_commit_closure.version"));
        }
        self.request.validate_shape()?;
        self.intent_authorization
            .validate_shape_against(&self.request)?;
        let intent = self.intent_authorization.intent();
        self.acceptance_ticket
            .validate_shape_against(&self.request, &intent)?;
        let request_digest = self.request.canonical_digest()?;
        let intent_digest = intent.canonical_digest_against(&self.request)?;
        let ticket_digest = self
            .acceptance_ticket
            .canonical_digest_against(&self.request, &intent)?;
        let authorization_digest = self
            .intent_authorization
            .canonical_digest_against(&self.request)?;
        if self.statement.request_id != self.request.request_id
            || self.statement.request_digest != request_digest
            || self.statement.acceptance_ticket_id != self.acceptance_ticket.acceptance_ticket_id
            || self.statement.ticket_digest != ticket_digest
            || self.statement.intent_authorization_digest != authorization_digest
            || self.statement.intent_digest != intent_digest
            || self.statement.exact_amount != intent.exact_amount
            || self.statement.exact_amount != self.acceptance_ticket.exact_amount
            || self.statement.sender_one_time_commitment != intent.sender_one_time_commitment
            || self.statement.release_id != self.intent_authorization.statement.release_id
            || self.statement.suite_id != self.intent_authorization.statement.suite_id
            || self.statement.vk_digest != self.intent_authorization.statement.vk_digest
            || self.statement.artifact_manifest_digest
                != self.intent_authorization.statement.artifact_manifest_digest
        {
            return Err(invalid("offline_cash.no_commit_closure.context"));
        }
        self.proof
            .validate_shape_for_semantic_digest(self.statement.canonical_digest()?)?;
        require_encoded_size(self, OFFLINE_CASH_NO_COMMIT_CLOSURE_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Return the canonical identity of this complete proof envelope.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed fields or canonical encoding failure.
    pub fn canonical_digest(&self) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        self.validate_shape()?;
        digest_encoded(NO_COMMIT_CLOSURE_DIGEST_DOMAIN, self)
    }

    /// Encode one shape-validated no-commit closure as canonical `oc1:` text.
    ///
    /// This is a codec boundary only. It does not verify either recursive proof or grant
    /// cancellation authority.
    ///
    /// # Errors
    ///
    /// Returns an error when validation, encoding, or a size bound fails.
    pub fn encode_text_shape(&self) -> Result<String, OfflineCashValidationErrorV1> {
        self.validate_shape()?;
        encode_offline_cash_text_v1(
            self,
            OFFLINE_CASH_NO_COMMIT_CLOSURE_MAX_BYTES_V1,
            OFFLINE_CASH_NO_COMMIT_CLOSURE_TEXT_MAX_BYTES_V1,
        )
    }

    /// Decode one exact canonical `oc1:` no-commit closure without granting authority.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid shape, size, framing, or canonical bytes.
    pub fn decode_text_shape_exact(text: &str) -> Result<Self, OfflineCashValidationErrorV1> {
        decode_offline_cash_text_v1(
            text,
            OFFLINE_CASH_NO_COMMIT_CLOSURE_MAX_BYTES_V1,
            OFFLINE_CASH_NO_COMMIT_CLOSURE_TEXT_MAX_BYTES_V1,
            Self::decode_canonical_shape_exact,
        )
    }

    /// Decode one exact bounded canonical no-commit closure without granting authority.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized, malformed, non-canonical, or invalid bytes.
    pub fn decode_canonical_shape_exact(
        bytes: &[u8],
    ) -> Result<Self, OfflineCashValidationErrorV1> {
        let closure: Self =
            decode_bounded_canonical(bytes, OFFLINE_CASH_NO_COMMIT_CLOSURE_MAX_BYTES_V1)?;
        closure.validate_shape()?;
        Ok(closure)
    }
}

impl OfflineCashAcceptanceTicketV1 {
    /// Return the exact bytes signed by receiver hardware.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, OfflineCashValidationErrorV1> {
        Ok(norito::encode_canonical(
            &AcceptanceTicketSigningPreimageV1 {
                domain: ACCEPTANCE_TICKET_SIGNING_DOMAIN.to_vec(),
                version: self.version,
                network_id: self.network_id,
                request_id: self.request_id,
                request_digest: self.request_digest,
                acceptance_ticket_id: self.acceptance_ticket_id,
                asset: self.asset.clone(),
                asset_incarnation: self.asset_incarnation,
                scale: self.scale,
                request_mode: self.request_mode,
                intent_digest: self.intent_digest,
                exact_amount: self.exact_amount,
                reserved_inbox_bytes: self.reserved_inbox_bytes,
                recipient_one_time_key: self.recipient_one_time_key,
                hardware_profile_id: self.hardware_profile_id,
                policy_epoch: self.policy_epoch,
                issued_at_ms: self.issued_at_ms,
                expires_at_ms: self.expires_at_ms,
            },
        )?)
    }

    /// Validate this one-use capacity reservation against its exact request and intent.
    ///
    /// Expiry never releases capacity automatically. An unused reservation can
    /// only enter governed online recovery while an equivalent durable delivery
    /// slot remains preserved.
    ///
    /// # Errors
    ///
    /// Returns an error for any request, mode, amount, capacity, expiry,
    /// credential, or hardware-signature mismatch.
    pub fn validate_shape_against(
        &self,
        request: &OfflineCashPaymentRequestV1,
        intent: &OfflineCashAcceptanceIntentV1,
    ) -> Result<(), OfflineCashValidationErrorV1> {
        request.validate_shape()?;
        intent.validate_shape_against(request)?;
        let request_digest = request.canonical_digest()?;
        let intent_digest = intent.canonical_digest_against(request)?;
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1
            || self.network_id != request.network_id
            || self.request_id != request.request_id
            || self.request_digest != request_digest
            || self.intent_digest != intent_digest
            || self.exact_amount != intent.exact_amount
            || self.asset != request.asset
            || self.asset_incarnation != request.asset_incarnation
            || self.scale != request.scale
            || self.request_mode != request.request_mode
            || self.hardware_profile_id != request.hardware_credential.hardware_profile_id
            || self.policy_epoch != request.hardware_credential.policy_epoch
            || self.issued_at_ms < request.issued_at_ms
            || self.expires_at_ms > request.expires_at_ms
            || self.issued_at_ms >= self.expires_at_ms
            || self.reserved_inbox_bytes
                < OFFLINE_CASH_ACCEPTANCE_TICKET_MIN_RESERVED_INBOX_BYTES_V1
            || !request.request_mode.accepts_exact_amount(self.exact_amount)
        {
            return Err(invalid("offline_cash.acceptance_ticket.request_binding"));
        }
        for (field, value) in [
            (
                "offline_cash.acceptance_ticket.acceptance_ticket_id",
                self.acceptance_ticket_id,
            ),
            (
                "offline_cash.acceptance_ticket.intent_digest",
                self.intent_digest,
            ),
            (
                "offline_cash.acceptance_ticket.recipient_one_time_key",
                self.recipient_one_time_key,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        require_valid_x25519_public_key(
            "offline_cash.acceptance_ticket.recipient_one_time_key",
            self.recipient_one_time_key,
        )?;
        self.signature.verify(
            &request.hardware_credential.device_public_key,
            &self.canonical_signing_bytes()?,
        )?;
        require_encoded_size(self, OFFLINE_CASH_ACCEPTANCE_TICKET_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Return the canonical one-use ticket digest.
    ///
    /// # Errors
    ///
    /// Returns an error when request/intent/ticket validation or encoding fails.
    pub fn canonical_digest_against(
        &self,
        request: &OfflineCashPaymentRequestV1,
        intent: &OfflineCashAcceptanceIntentV1,
    ) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        self.validate_shape_against(request, intent)?;
        digest_encoded(ACCEPTANCE_TICKET_DIGEST_DOMAIN, self)
    }

    /// Encode one shape-validated acceptance ticket as canonical `oc1:` text.
    ///
    /// # Errors
    ///
    /// Returns an error when validation, encoding, or a size bound fails.
    pub fn encode_text_shape_against(
        &self,
        request: &OfflineCashPaymentRequestV1,
        intent: &OfflineCashAcceptanceIntentV1,
    ) -> Result<String, OfflineCashValidationErrorV1> {
        self.validate_shape_against(request, intent)?;
        encode_offline_cash_text_v1(
            self,
            OFFLINE_CASH_ACCEPTANCE_TICKET_MAX_BYTES_V1,
            OFFLINE_CASH_ACCEPTANCE_TICKET_TEXT_MAX_BYTES_V1,
        )
    }

    /// Decode one exact canonical acceptance ticket without granting capacity.
    ///
    /// Receiver hardware must still atomically reserve the ticket in its private
    /// request and inbox ledgers after decoding succeeds.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid context, size, framing, or canonical bytes.
    pub fn decode_text_shape_exact_against(
        text: &str,
        request: &OfflineCashPaymentRequestV1,
        intent: &OfflineCashAcceptanceIntentV1,
    ) -> Result<Self, OfflineCashValidationErrorV1> {
        decode_offline_cash_text_v1(
            text,
            OFFLINE_CASH_ACCEPTANCE_TICKET_MAX_BYTES_V1,
            OFFLINE_CASH_ACCEPTANCE_TICKET_TEXT_MAX_BYTES_V1,
            |bytes| Self::decode_canonical_shape_exact_against(bytes, request, intent),
        )
    }

    /// Decode one exact bounded canonical acceptance ticket without reserving it.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized, malformed, non-canonical, or invalid bytes.
    pub fn decode_canonical_shape_exact_against(
        bytes: &[u8],
        request: &OfflineCashPaymentRequestV1,
        intent: &OfflineCashAcceptanceIntentV1,
    ) -> Result<Self, OfflineCashValidationErrorV1> {
        let ticket: Self =
            decode_bounded_canonical(bytes, OFFLINE_CASH_ACCEPTANCE_TICKET_MAX_BYTES_V1)?;
        ticket.validate_shape_against(request, intent)?;
        Ok(ticket)
    }
}

impl OfflineCashLifecycleBindingV1 {
    /// Validate the complete released-transition lifecycle context.
    ///
    /// # Errors
    ///
    /// Returns an error for a reserved identity, unsupported protocol, invalid
    /// pooled reserve, or malformed operation-specific binding.
    pub fn validate(&self) -> Result<(), OfflineCashValidationErrorV1> {
        require_valid_header(
            self.version,
            &self.network_id,
            self.scale,
            None,
            "offline_cash.lifecycle.header",
        )?;
        if self.protocol_version != OFFLINE_CASH_WIRE_VERSION_V1 || self.policy_epoch == 0 {
            return Err(invalid("offline_cash.lifecycle.context"));
        }
        self.asset_incarnation
            .validate()
            .map_err(|_| invalid("offline_cash.lifecycle.asset_incarnation"))?;
        for (field, value) in [
            ("offline_cash.lifecycle.suite_id", self.suite_id),
            ("offline_cash.lifecycle.vk_digest", self.vk_digest),
            ("offline_cash.lifecycle.release_id", self.release_id),
            (
                "offline_cash.lifecycle.liability_pool_id",
                self.liability_pool_id,
            ),
            (
                "offline_cash.lifecycle.hardware_profile_id",
                self.hardware_profile_id,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        if self.liability_pool_id
            != offline_cash_liability_pool_id_v1(
                &self.network_id,
                &self.asset,
                self.asset_incarnation,
            )?
        {
            return Err(invalid("offline_cash.lifecycle.liability_pool_id"));
        }
        let request_fields = [self.request_id, self.acceptance_ticket_id];
        let credit_fields = [self.credit_id, self.ciphertext_digest];
        match self.operation_kind {
            OfflineCashOperationKindV1::SendSplit
                if request_fields.iter().all(|value| *value != [0; 32])
                    && credit_fields.iter().all(|value| *value != [0; 32]) => {}
            OfflineCashOperationKindV1::SendSplit => {
                return Err(invalid("offline_cash.lifecycle.payment_binding"));
            }
            OfflineCashOperationKindV1::MintFold
                if request_fields.iter().all(|value| *value == [0; 32])
                    && credit_fields.iter().all(|value| *value != [0; 32]) => {}
            OfflineCashOperationKindV1::MintFold => {
                return Err(invalid("offline_cash.lifecycle.mint_binding"));
            }
            _ if request_fields
                .iter()
                .chain(credit_fields.iter())
                .all(|value| *value == [0; 32]) => {}
            _ => return Err(invalid("offline_cash.lifecycle.non_payment_binding")),
        }
        Ok(())
    }

    /// Return the canonical lifecycle digest bound by proof and certificate.
    ///
    /// # Errors
    ///
    /// Returns an error when the lifecycle is invalid or cannot be encoded.
    pub fn canonical_digest(&self) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        self.validate()?;
        digest_encoded(LIFECYCLE_BINDING_DIGEST_DOMAIN, self)
    }
}

impl OfflineCashCommitEvidenceV1 {
    /// Validate trusted-time or secure monotonic-lease evidence.
    ///
    /// # Errors
    ///
    /// Returns an error for a reserved hiding commitment. Commit instants,
    /// lease windows, rollback-resistant counters, clock epochs, and raw lease
    /// identities remain private wrapper witnesses so consecutive public
    /// payments disclose neither a timestamp nor a linkable sequence. The
    /// wrapper proves the applicable request/ticket deadline predicate.
    pub fn validate(self) -> Result<(), OfflineCashValidationErrorV1> {
        match self {
            Self::TrustedTime(evidence) if evidence.time_evidence_commitment != [0; 32] => Ok(()),
            Self::MonotonicLease(lease) if lease.lease_evidence_commitment != [0; 32] => Ok(()),
            _ => Err(invalid("offline_cash.commit_evidence")),
        }
    }
}

/// Return the operation-specific minimum durable sender-outbox reservation.
///
/// Operations that do not emit a recoverable external terminal envelope return
/// `None` and cannot construct an [`OfflineCashOutboxReservationV1`].
#[must_use]
pub const fn offline_cash_outbox_min_reserved_bytes_v1(
    operation_kind: OfflineCashOperationKindV1,
) -> Option<u32> {
    match operation_kind {
        OfflineCashOperationKindV1::SendSplit => Some(OFFLINE_CASH_PAYMENT_OUTBOX_MIN_BYTES_V1),
        OfflineCashOperationKindV1::RedeemSplit => {
            Some(OFFLINE_CASH_REDEMPTION_OUTBOX_MIN_BYTES_V1)
        }
        _ => None,
    }
}

impl OfflineCashOutboxReservationV1 {
    fn validate(self) -> Result<(), OfflineCashValidationErrorV1> {
        let minimum = offline_cash_outbox_min_reserved_bytes_v1(self.operation_kind)
            .ok_or_else(|| invalid("offline_cash.outbox_reservation.operation_kind"))?;
        if self.reservation_id == [0; 32]
            || self.reserved_outbox_bytes < minimum
            || self.issued_at_ms >= self.expires_at_ms
        {
            return Err(invalid("offline_cash.outbox_reservation"));
        }
        Ok(())
    }

    /// Return the hiding public commitment proven by the final wrapper.
    ///
    /// # Errors
    ///
    /// Returns an error when the operation-specific reservation is too small,
    /// malformed, or cannot be encoded.
    pub fn canonical_commitment(self) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        self.validate()?;
        Ok(digest_bytes(
            OUTBOX_RESERVATION_COMMITMENT_DOMAIN,
            &outbox_reservation_circuit_transcript_v1(self),
        ))
    }
}

impl OfflineCashHardwareTerminalBodyV1 {
    /// Return the hiding commitment used by the terminal certificate.
    ///
    /// # Errors
    ///
    /// Returns an error for a reserved field, unsupported version, invalid
    /// commit evidence, or canonical encoding failure.
    pub fn canonical_commitment(&self) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1 || self.policy_epoch == 0 {
            return Err(invalid("offline_cash.hardware_terminal_body.context"));
        }
        self.commit_evidence.validate()?;
        for (field, value) in [
            (
                "offline_cash.hardware_terminal_body.candidate_envelope_digest",
                self.candidate_envelope_digest,
            ),
            (
                "offline_cash.hardware_terminal_body.lifecycle_binding_digest",
                self.lifecycle_binding_digest,
            ),
            (
                "offline_cash.hardware_terminal_body.transition_nullifier",
                self.transition_nullifier,
            ),
            (
                "offline_cash.hardware_terminal_body.outbox_reservation_commitment",
                self.outbox_reservation_commitment,
            ),
            (
                "offline_cash.hardware_terminal_body.hardware_profile_id",
                self.hardware_profile_id,
            ),
            (
                "offline_cash.hardware_terminal_body.private_successor_commitment",
                self.private_successor_commitment,
            ),
            (
                "offline_cash.hardware_terminal_body.private_journal_commitment",
                self.private_journal_commitment,
            ),
            (
                "offline_cash.hardware_terminal_body.private_recovery_commitment",
                self.private_recovery_commitment,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        digest_encoded(HARDWARE_TERMINAL_BODY_COMMITMENT_DOMAIN, self)
    }
}

impl OfflineCashCommitCertificateV1 {
    /// Compute the canonical terminal-certificate identity.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn expected_certificate_id(&self) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        Ok(digest_bytes(
            COMMIT_CERTIFICATE_ID_DOMAIN,
            &commit_certificate_id_circuit_transcript_v1(self),
        ))
    }

    /// Populate the canonical certificate identity.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn seal_certificate_id(mut self) -> Result<Self, OfflineCashValidationErrorV1> {
        self.certificate_id = self.expected_certificate_id()?;
        Ok(self)
    }

    /// Bind a self-free terminal body, then derive the certificate identity.
    ///
    /// This is the canonical construction path. It exact-matches every public
    /// terminal field before hashing the private body and only then derives
    /// `certificate_id`, so no implementation can accidentally construct a
    /// commitment/ID fixed point.
    ///
    /// # Errors
    ///
    /// Returns an error when the body is invalid or differs from this
    /// certificate's public fields.
    pub fn seal_with_terminal_body(
        mut self,
        body: &OfflineCashHardwareTerminalBodyV1,
    ) -> Result<Self, OfflineCashValidationErrorV1> {
        if body.version != self.version
            || body.candidate_envelope_digest != self.candidate_envelope_digest
            || body.lifecycle_binding_digest != self.lifecycle_binding_digest
            || body.transition_nullifier != self.transition_nullifier
            || body.outbox_reservation_commitment != self.outbox_reservation_commitment
            || body.commit_evidence != self.commit_evidence
            || body.hardware_profile_id != self.hardware_profile_id
            || body.policy_epoch != self.policy_epoch
        {
            return Err(invalid("offline_cash.hardware_terminal_body.binding"));
        }
        self.hardware_terminal_commitment = body.canonical_commitment()?;
        self.seal_certificate_id()
    }

    fn validate_against(
        &self,
        lifecycle: &OfflineCashLifecycleBindingV1,
        expected_evidence: OfflineCashCommitEvidenceV1,
        expected_nullifier: [u8; 32],
    ) -> Result<(), OfflineCashValidationErrorV1> {
        lifecycle.validate()?;
        self.commit_evidence.validate()?;
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1
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
            return Err(invalid("offline_cash.commit_certificate.binding"));
        }
        require_encoded_size(self, OFFLINE_CASH_COMMIT_CERTIFICATE_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Return the fixed-width terminal-certificate digest constrained by both wrapper parities.
    ///
    /// # Errors
    ///
    /// Returns an error when any lifecycle, evidence, nullifier, or certificate binding is
    /// invalid.
    pub fn canonical_digest_against(
        &self,
        lifecycle: &OfflineCashLifecycleBindingV1,
        expected_evidence: OfflineCashCommitEvidenceV1,
        expected_nullifier: [u8; 32],
    ) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        self.validate_against(lifecycle, expected_evidence, expected_nullifier)?;
        Ok(digest_bytes(
            COMMIT_CERTIFICATE_DIGEST_DOMAIN,
            &commit_certificate_circuit_transcript_v1(self),
        ))
    }
}

impl OfflineCashCommitWrapperProofV1 {
    // Core cryptographically verifies both recursive parities, private credential/outbox
    // witnesses, and terminal hardware authority. This data-model layer deliberately performs
    // only bounded-envelope and public-binding validation.
    fn validate_bindings(
        &self,
        semantic_digest: [u8; 32],
        candidate_envelope_digest: [u8; 32],
        commit_certificate_digest: [u8; 32],
    ) -> Result<(), OfflineCashValidationErrorV1> {
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1
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
            || self.eq_proof.len() > OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1
            || self.ep_proof.len() > OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1
            || self.eq_proof.len() + self.ep_proof.len() > OFFLINE_CASH_CURRENT_PROOFS_MAX_BYTES_V1
            || self.eq_history.len() != OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1
            || self.ep_history.len() != OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1
            || self.eq_history.iter().all(|byte| *byte == 0)
            || self.ep_history.iter().all(|byte| *byte == 0)
            || self.eq_history == self.ep_history
        {
            return Err(invalid("offline_cash.commit_wrapper_proof.binding"));
        }
        require_encoded_size(self, OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1)?;
        Ok(())
    }
}

impl OfflineCashAggregateStateCommitmentV1 {
    /// Decode and validate exact bounded aggregate-state metadata.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized, malformed, non-canonical, or invalid state metadata.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, OfflineCashValidationErrorV1> {
        let state: Self =
            decode_bounded_canonical(bytes, OFFLINE_CASH_AGGREGATE_STATE_MAX_BYTES_V1)?;
        state.validate()?;
        Ok(state)
    }

    /// Validate the fixed aggregate-state context and commitments.
    ///
    /// # Errors
    ///
    /// Returns an error when any context, identity, or commitment binding is invalid.
    pub fn validate(&self) -> Result<(), OfflineCashValidationErrorV1> {
        require_valid_header(
            self.version,
            &self.network_id,
            self.scale,
            None,
            "offline_cash.aggregate_state.header",
        )?;
        self.asset_incarnation
            .validate()
            .map_err(|_| invalid("offline_cash.aggregate_state.asset_incarnation"))?;
        for (field, value) in [
            ("offline_cash.aggregate_state.release_id", self.release_id),
            (
                "offline_cash.aggregate_state.liability_pool_id",
                self.liability_pool_id,
            ),
            ("offline_cash.aggregate_state.lane_id", self.lane_id),
            (
                "offline_cash.aggregate_state.hardware_epoch_id",
                self.hardware_epoch_id,
            ),
            (
                "offline_cash.aggregate_state.key_reference",
                self.key_reference,
            ),
            (
                "offline_cash.aggregate_state.hardware_policy_id",
                self.hardware_policy_id,
            ),
            (
                "offline_cash.aggregate_state.state_commitment",
                self.state_commitment,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        if self.liability_pool_id
            != offline_cash_liability_pool_id_v1(
                &self.network_id,
                &self.asset,
                self.asset_incarnation,
            )?
        {
            return Err(invalid("offline_cash.aggregate_state.liability_pool_id"));
        }
        require_encoded_size(self, OFFLINE_CASH_AGGREGATE_STATE_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Return the fixed aggregate-state identity committed by recursive proofs.
    ///
    /// # Errors
    ///
    /// Returns an error when the state is invalid or cannot be encoded.
    pub fn canonical_digest(&self) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        self.validate()?;
        digest_encoded(AGGREGATE_STATE_DIGEST_DOMAIN, self)
    }
}

/// Encode the exact canonical recipient-request bytes authorized by hardware.
///
/// This constructor is the single cross-crate signing contract. The request
/// binds the reusable mode and compact hardware credential; the one-use
/// capacity reservation is signed separately as an acceptance ticket.
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
    asset_incarnation: AxtAssetIncarnationV1,
    scale: u32,
    liability_pool_id: [u8; 32],
    recipient: &AccountId,
    request_mode: OfflineCashPaymentRequestModeV1,
    hardware_credential_id: [u8; 32],
    request_id: [u8; 32],
    issued_at_ms: u64,
    expires_at_ms: u64,
) -> Result<Vec<u8>, OfflineCashValidationErrorV1> {
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
            request_mode,
            hardware_credential_id,
            request_id,
            issued_at_ms,
            expires_at_ms,
        },
    )?)
}

impl OfflineCashPaymentRequestV1 {
    /// Return the exact bytes signed by the recipient device.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical Norito encoding fails.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, OfflineCashValidationErrorV1> {
        offline_cash_payment_request_signing_bytes_v1(
            self.version,
            self.release_id,
            &self.network_id,
            &self.asset,
            self.asset_incarnation,
            self.scale,
            self.liability_pool_id,
            &self.recipient,
            self.request_mode,
            self.hardware_credential.credential_id,
            self.request_id,
            self.issued_at_ms,
            self.expires_at_ms,
        )
    }

    /// Encode this validated request as canonical unpadded `oc1:` base64url text.
    ///
    /// # Errors
    ///
    /// Returns an error when the request is invalid, cannot be encoded, or exceeds its cap.
    pub fn encode_text(&self) -> Result<String, OfflineCashValidationErrorV1> {
        self.validate_shape()?;
        encode_offline_cash_text_v1(
            self,
            OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1,
            OFFLINE_CASH_PAYMENT_REQUEST_TEXT_MAX_BYTES_V1,
        )
    }

    /// Decode one exact canonical unpadded `oc1:` request.
    ///
    /// Text syntax and size are checked before base64 decoding, and the raw cap
    /// is checked before Norito decoding. The decoded request must re-encode to
    /// the exact original text.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid, oversized, padded, non-canonical, or legacy text.
    pub fn decode_text_exact(text: &str) -> Result<Self, OfflineCashValidationErrorV1> {
        decode_offline_cash_text_v1(
            text,
            OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1,
            OFFLINE_CASH_PAYMENT_REQUEST_TEXT_MAX_BYTES_V1,
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
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, OfflineCashValidationErrorV1> {
        let request: Self =
            decode_bounded_canonical(bytes, OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1)?;
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
    pub fn validate_shape(&self) -> Result<(), OfflineCashValidationErrorV1> {
        require_valid_header(
            self.version,
            &self.network_id,
            self.scale,
            None,
            "offline_cash.request.header",
        )?;
        for (field, value) in [
            ("offline_cash.request.release_id", self.release_id),
            (
                "offline_cash.request.liability_pool_id",
                self.liability_pool_id,
            ),
            ("offline_cash.request.request_id", self.request_id),
        ] {
            require_nonzero(field, value)?;
        }
        if self.liability_pool_id
            != offline_cash_liability_pool_id_v1(
                &self.network_id,
                &self.asset,
                self.asset_incarnation,
            )?
        {
            return Err(invalid("offline_cash.request.liability_pool_id"));
        }
        self.asset_incarnation
            .validate()
            .map_err(|_| invalid("offline_cash.request.asset_incarnation"))?;
        self.request_mode.validate()?;
        self.hardware_credential.validate_shape()?;
        if self.hardware_credential.network_id != self.network_id
            || self.issued_at_ms < self.hardware_credential.issued_at_ms
            || self.expires_at_ms > self.hardware_credential.expires_at_ms
        {
            return Err(invalid("offline_cash.request.hardware_credential"));
        }
        let ttl = self
            .expires_at_ms
            .checked_sub(self.issued_at_ms)
            .ok_or_else(|| invalid("offline_cash.request.expires_at_ms"))?;
        if ttl == 0 || ttl > OFFLINE_CASH_REQUEST_MAX_TTL_MS_V1 {
            return Err(invalid("offline_cash.request.expires_at_ms"));
        }
        self.signature.verify(
            &self.hardware_credential.device_public_key,
            &self.canonical_signing_bytes()?,
        )?;
        require_encoded_size(self, OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1)?;
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
        profile: &OfflineCashHardwareProfileV1,
    ) -> Result<(), OfflineCashValidationErrorV1> {
        self.validate_shape()?;
        self.hardware_credential.validate_against_profile(profile)
    }

    /// Return the canonical request identity consumed by a sender split.
    ///
    /// # Errors
    ///
    /// Returns an error when the request is invalid or cannot be encoded.
    pub fn canonical_digest(&self) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        self.validate_shape()?;
        digest_encoded(REQUEST_DIGEST_DOMAIN, self)
    }
}

impl OfflineCashTransferStatementV1 {
    /// Build the exact pre-ID context authenticated by a peer-credit envelope.
    ///
    /// This projection intentionally excludes `lifecycle.credit_id`,
    /// `lifecycle.ciphertext_digest`, proofs, and terminal certificate fields.
    /// It can therefore be computed before AEAD sealing while still binding the
    /// complete exact signed request, sender intent, receiver ticket, and
    /// released lifecycle context.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid or substituted request/session binding.
    pub fn peer_credit_context_against(
        &self,
        request: &OfflineCashPaymentRequestV1,
        intent: &OfflineCashAcceptanceIntentV1,
        ticket: &OfflineCashAcceptanceTicketV1,
    ) -> Result<OfflineCashPeerCreditContextV1, OfflineCashValidationErrorV1> {
        request.validate_shape()?;
        intent.validate_shape_against(request)?;
        ticket.validate_shape_against(request, intent)?;
        let request_digest = request.canonical_digest()?;
        let intent_digest = intent.canonical_digest_against(request)?;
        let ticket_digest = ticket.canonical_digest_against(request, intent)?;
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1
            || self.lifecycle.version != self.version
            || self.amount == 0
            || self.transition_nullifier == [0; 32]
            || self.ciphertext_commitment == [0; 32]
            || self.request_digest != request_digest
            || self.acceptance_ticket_digest != ticket_digest
            || self.recipient_one_time_key != ticket.recipient_one_time_key
            || self.amount != ticket.exact_amount
            || self.lifecycle.release_id != request.release_id
            || self.lifecycle.network_id != request.network_id
            || self.lifecycle.asset != request.asset
            || self.lifecycle.asset_incarnation != request.asset_incarnation
            || self.lifecycle.scale != request.scale
            || self.lifecycle.liability_pool_id != request.liability_pool_id
            || self.lifecycle.suite_id != request.hardware_credential.suite_id
            || self.lifecycle.request_id != request.request_id
            || self.lifecycle.acceptance_ticket_id != ticket.acceptance_ticket_id
            || self.lifecycle.credit_id != self.expected_credit_id()?
        {
            return Err(invalid("offline_cash.peer_credit_context.binding"));
        }
        let context = OfflineCashPeerCreditContextV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            request_digest,
            acceptance_intent_digest: intent_digest,
            acceptance_ticket_digest: ticket_digest,
            lifecycle_context_digest: peer_credit_lifecycle_context_digest_v1(&self.lifecycle)?,
            recipient_one_time_key: self.recipient_one_time_key,
        };
        context.validate_shape()?;
        Ok(context)
    }

    /// Compute the required output-credit identity.
    ///
    /// # Errors
    ///
    /// Returns an error when the canonical identity preimage cannot be encoded.
    pub fn expected_credit_id(&self) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        offline_cash_credit_id_v1(
            self.transition_nullifier,
            self.request_digest,
            self.acceptance_ticket_digest,
            self.recipient_one_time_key,
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
    pub fn seal_credit_id(mut self) -> Result<Self, OfflineCashValidationErrorV1> {
        self.lifecycle.credit_id = self.expected_credit_id()?;
        Ok(self)
    }

    /// Validate the exact unlinkable public send binding.
    ///
    /// # Errors
    ///
    /// Returns an error when lifecycle, output, nullifier, or commit evidence is invalid.
    pub fn validate(&self) -> Result<(), OfflineCashValidationErrorV1> {
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1 || self.lifecycle.version != self.version {
            return Err(invalid("offline_cash.statement.version"));
        }
        self.lifecycle.validate()?;
        for (field, value) in [
            (
                "offline_cash.statement.transition_nullifier",
                self.transition_nullifier,
            ),
            ("offline_cash.statement.request_digest", self.request_digest),
            (
                "offline_cash.statement.acceptance_ticket_digest",
                self.acceptance_ticket_digest,
            ),
            (
                "offline_cash.statement.recipient_one_time_key",
                self.recipient_one_time_key,
            ),
            (
                "offline_cash.statement.ciphertext_commitment",
                self.ciphertext_commitment,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        require_valid_x25519_public_key(
            "offline_cash.statement.recipient_one_time_key",
            self.recipient_one_time_key,
        )?;
        if self.amount == 0
            || self.lifecycle.operation_kind != OfflineCashOperationKindV1::SendSplit
        {
            return Err(invalid("offline_cash.statement.operation"));
        }
        self.commit_evidence.validate()?;
        if self.lifecycle.credit_id != self.expected_credit_id()? {
            return Err(invalid("offline_cash.statement.credit_id"));
        }
        Ok(())
    }

    /// Return the common semantic digest constrained by both Pasta parities.
    ///
    /// # Errors
    ///
    /// Returns an error when the statement is invalid or cannot be encoded.
    pub fn canonical_digest(&self) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        self.validate()?;
        digest_encoded(STATEMENT_DIGEST_DOMAIN, self)
    }
}

impl OfflineCashPairedProofV1 {
    /// Validate fixed parity roles, proof caps, and exact history sizes.
    ///
    /// # Errors
    ///
    /// Returns an error when the proof is empty, oversized, aliased, or mis-bound.
    pub fn validate_shape_for_semantic_digest(
        &self,
        expected_semantic_digest: [u8; 32],
    ) -> Result<(), OfflineCashValidationErrorV1> {
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1 {
            return Err(invalid("offline_cash.proof.version"));
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
        require_nonzero(
            "offline_cash.proof.guard_eq_credential_audit",
            self.guard_eq_credential_audit,
        )?;
        require_nonzero(
            "offline_cash.proof.guard_ep_credential_audit",
            self.guard_ep_credential_audit,
        )?;
        require_nonzero(
            "offline_cash.proof.eq_deferred_audit",
            self.eq_deferred_audit,
        )?;
        require_nonzero(
            "offline_cash.proof.ep_deferred_audit",
            self.ep_deferred_audit,
        )?;
        if self.eq_protocol_digest == self.ep_protocol_digest
            || self.semantic_digest != expected_semantic_digest
            || self.guard_eq_credential_audit == self.guard_ep_credential_audit
            || self.eq_deferred_audit == self.ep_deferred_audit
        {
            return Err(invalid("offline_cash.proof.role_binding"));
        }
        if self.eq_proof.is_empty()
            || self.ep_proof.is_empty()
            || self.eq_proof.len() > OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1
            || self.ep_proof.len() > OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1
            || self.eq_proof.len() + self.ep_proof.len() > OFFLINE_CASH_CURRENT_PROOFS_MAX_BYTES_V1
        {
            return Err(invalid("offline_cash.proof.current"));
        }
        if self.eq_history.len() != OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1
            || self.ep_history.len() != OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1
            || self.eq_history.iter().all(|byte| *byte == 0)
            || self.ep_history.iter().all(|byte| *byte == 0)
            || self.eq_history == self.ep_history
        {
            return Err(invalid("offline_cash.proof.history"));
        }
        require_encoded_size(self, OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1)?;
        Ok(())
    }
}

impl OfflineCashPaymentV1 {
    /// Encode this validated payment as canonical unpadded `oc1:` base64url text.
    ///
    /// # Errors
    ///
    /// Returns an error when request validation, encoding, or a size bound fails.
    pub fn encode_text_against(
        &self,
        request: &OfflineCashPaymentRequestV1,
    ) -> Result<String, OfflineCashValidationErrorV1> {
        self.validate_shape_against(request)?;
        encode_offline_cash_text_v1(
            self,
            OFFLINE_CASH_PAYMENT_MAX_BYTES_V1,
            OFFLINE_CASH_PAYMENT_TEXT_MAX_BYTES_V1,
        )
    }

    /// Decode one exact canonical unpadded `oc1:` payment against its request.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid context, size, prefix, padding, base64url, or Norito bytes.
    pub fn decode_text_exact_against(
        text: &str,
        request: &OfflineCashPaymentRequestV1,
    ) -> Result<Self, OfflineCashValidationErrorV1> {
        decode_offline_cash_text_v1(
            text,
            OFFLINE_CASH_PAYMENT_MAX_BYTES_V1,
            OFFLINE_CASH_PAYMENT_TEXT_MAX_BYTES_V1,
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
        request: &OfflineCashPaymentRequestV1,
    ) -> Result<Self, OfflineCashValidationErrorV1> {
        let payment: Self = decode_bounded_canonical(bytes, OFFLINE_CASH_PAYMENT_MAX_BYTES_V1)?;
        payment.validate_shape_against(request)?;
        Ok(payment)
    }

    /// Validate this response's complete structural binding against the exact
    /// signed recipient request.
    ///
    /// The trusted commit instant or consumed monotonic-lease window stays
    /// private. The release-pinned wrapper verifier must prove it fell inside
    /// both request and ticket windows. This function deliberately accepts no
    /// current-wall-clock argument: committed money remains valid indefinitely.
    ///
    /// # Errors
    ///
    /// This checks proof framing and digest bindings, not the recursive proof's
    /// cryptographic validity or the release catalog. Monetary admission must
    /// first authenticate the request profile and then use the release-pinned
    /// native proof verifier.
    ///
    /// Returns an error when a public context, proof shape, statement, request,
    /// opaque evidence, or size binding fails.
    pub fn validate_shape_against(
        &self,
        request: &OfflineCashPaymentRequestV1,
    ) -> Result<(), OfflineCashValidationErrorV1> {
        request.validate_shape()?;
        let request_digest = request.canonical_digest()?;
        self.acceptance_intent.validate_shape_against(request)?;
        self.acceptance_ticket
            .validate_shape_against(request, &self.acceptance_intent)?;
        let ticket_digest = self
            .acceptance_ticket
            .canonical_digest_against(request, &self.acceptance_intent)?;
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1
            || self.statement.version != self.version
            || self.statement.lifecycle.release_id != request.release_id
            || self.statement.lifecycle.network_id != request.network_id
            || self.statement.lifecycle.asset != request.asset
            || self.statement.lifecycle.asset_incarnation != request.asset_incarnation
            || self.statement.lifecycle.scale != request.scale
            || self.statement.lifecycle.liability_pool_id != request.liability_pool_id
            || self.statement.lifecycle.suite_id != request.hardware_credential.suite_id
            || self.statement.request_digest != request_digest
            || self.statement.acceptance_ticket_digest != ticket_digest
            || self.statement.lifecycle.request_id != request.request_id
            || self.statement.lifecycle.acceptance_ticket_id
                != self.acceptance_ticket.acceptance_ticket_id
            || self.statement.recipient_one_time_key
                != self.acceptance_ticket.recipient_one_time_key
            || self.acceptance_ticket.exact_amount != self.statement.amount
        {
            return Err(invalid("offline_cash.payment.request_binding"));
        }
        self.statement.validate()?;
        OfflineCashEncryptedCreditEnvelopeV1::decode_canonical_shape_exact_against_recipient_key(
            &self.encrypted_credit,
            self.statement.recipient_one_time_key,
        )?;
        OfflineCashEncryptedCreditAadV1::for_peer(
            &self.statement,
            request,
            &self.acceptance_intent,
            &self.acceptance_ticket,
        )?;
        if self.statement.lifecycle.ciphertext_digest
            != offline_cash_ciphertext_digest_v1(&self.encrypted_credit)
        {
            return Err(invalid("offline_cash.payment.encrypted_credit"));
        }
        self.commit_certificate.validate_against(
            &self.statement.lifecycle,
            self.statement.commit_evidence,
            self.statement.transition_nullifier,
        )?;
        let certificate_digest = self.commit_certificate.canonical_digest_against(
            &self.statement.lifecycle,
            self.statement.commit_evidence,
            self.statement.transition_nullifier,
        )?;
        self.proof.validate_bindings(
            self.statement.canonical_digest()?,
            self.commit_certificate.candidate_envelope_digest,
            certificate_digest,
        )?;
        require_nonzero(
            "offline_cash.payment.artifact_manifest_digest",
            self.artifact_manifest_digest,
        )?;
        let encoded_size = require_encoded_size(self, OFFLINE_CASH_PAYMENT_MAX_BYTES_V1)?;
        if encoded_size
            > usize::try_from(self.acceptance_ticket.reserved_inbox_bytes).unwrap_or(usize::MAX)
        {
            return Err(invalid("offline_cash.payment.reserved_inbox_bytes"));
        }
        Ok(())
    }

    /// Return the canonical response digest after validating its request.
    ///
    /// # Errors
    ///
    /// Returns an error when the response is invalid or cannot be encoded.
    pub fn canonical_digest_against(
        &self,
        request: &OfflineCashPaymentRequestV1,
    ) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        self.validate_shape_against(request)?;
        digest_encoded(PAYMENT_DIGEST_DOMAIN, self)
    }

    /// Return the unlinkable circuit nullifier used for conflict detection.
    ///
    /// # Errors
    ///
    /// Returns an error only when the reserved all-zero value is present.
    pub fn sender_conflict_key(&self) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        require_nonzero(
            "offline_cash.payment.transition_nullifier",
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
pub fn offline_cash_acknowledgement_signing_bytes_v1(
    version: u16,
    request_digest: [u8; 32],
    payment_digest: [u8; 32],
    inbox_receipt: OfflineCashInboxReceiptV1,
) -> Result<Vec<u8>, OfflineCashValidationErrorV1> {
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

impl OfflineCashAcknowledgementV1 {
    /// Encode this validated acknowledgement as canonical unpadded `oc1:` base64url text.
    ///
    /// # Errors
    ///
    /// Returns an error when context validation, encoding, or a size bound fails.
    pub fn encode_text_against(
        &self,
        request: &OfflineCashPaymentRequestV1,
        payment: &OfflineCashPaymentV1,
    ) -> Result<String, OfflineCashValidationErrorV1> {
        self.validate_shape_against(request, payment)?;
        encode_offline_cash_text_v1(
            self,
            OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1,
            OFFLINE_CASH_ACKNOWLEDGEMENT_TEXT_MAX_BYTES_V1,
        )
    }

    /// Decode one exact canonical unpadded `oc1:` acknowledgement.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid context, size, prefix, padding, base64url, or Norito bytes.
    pub fn decode_text_exact_against(
        text: &str,
        request: &OfflineCashPaymentRequestV1,
        payment: &OfflineCashPaymentV1,
    ) -> Result<Self, OfflineCashValidationErrorV1> {
        decode_offline_cash_text_v1(
            text,
            OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1,
            OFFLINE_CASH_ACKNOWLEDGEMENT_TEXT_MAX_BYTES_V1,
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
        request: &OfflineCashPaymentRequestV1,
        payment: &OfflineCashPaymentV1,
    ) -> Result<Self, OfflineCashValidationErrorV1> {
        let acknowledgement: Self =
            decode_bounded_canonical(bytes, OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1)?;
        acknowledgement.validate_shape_against(request, payment)?;
        Ok(acknowledgement)
    }

    /// Return the exact bytes signed after persisting the inbox receipt.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, OfflineCashValidationErrorV1> {
        offline_cash_acknowledgement_signing_bytes_v1(
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
        request: &OfflineCashPaymentRequestV1,
        payment: &OfflineCashPaymentV1,
    ) -> Result<(), OfflineCashValidationErrorV1> {
        payment.validate_shape_against(request)?;
        let request_digest = request.canonical_digest()?;
        let payment_digest = payment.canonical_digest_against(request)?;
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1
            || self.request_digest != request_digest
            || self.payment_digest != payment_digest
            || self.inbox_receipt.version != self.version
            || self.inbox_receipt.credit_id != payment.statement.lifecycle.credit_id
            || self.inbox_receipt.receipt_commitment == [0; 32]
        {
            return Err(invalid("offline_cash.acknowledgement.binding"));
        }
        self.signature.verify(
            &request.hardware_credential.device_public_key,
            &self.canonical_signing_bytes()?,
        )?;
        require_encoded_size(self, OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1)?;
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

fn validate_offline_cash_raw_session_size_v1(
    raw: usize,
) -> Result<(), OfflineCashValidationErrorV1> {
    if raw > OFFLINE_CASH_SESSION_MAX_BYTES_V1 {
        return Err(OfflineCashValidationErrorV1::EncodedSizeExceeded {
            actual: raw,
            max: OFFLINE_CASH_SESSION_MAX_BYTES_V1,
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
pub fn validate_offline_cash_session_shape_v1(
    request: &OfflineCashPaymentRequestV1,
    payment: &OfflineCashPaymentV1,
    acknowledgement: &OfflineCashAcknowledgementV1,
) -> Result<usize, OfflineCashValidationErrorV1> {
    acknowledgement.validate_shape_against(request, payment)?;
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
        return Err(OfflineCashValidationErrorV1::EncodedSizeExceeded {
            actual: text,
            max: OFFLINE_CASH_TEXT_SESSION_MAX_BYTES_V1,
        });
    }
    Ok(raw)
}

/// Validate the complete pre-ticket exchange and return its raw transported size.
///
/// This performs bounded structural checks only. Receiver hardware must
/// authenticate the release and verify the authorization proof before it
/// persists the envelope, records an intent decision, or reserves capacity.
///
/// # Errors
///
/// Returns an error when a binding is invalid or the raw/text exchange exceeds
/// its aggregate cap.
pub fn validate_offline_cash_pre_ticket_exchange_shape_v1(
    request: &OfflineCashPaymentRequestV1,
    authorization: &OfflineCashAcceptanceIntentAuthorizationV1,
    ticket: &OfflineCashAcceptanceTicketV1,
) -> Result<usize, OfflineCashValidationErrorV1> {
    authorization.validate_shape_against(request)?;
    ticket.validate_shape_against(request, &authorization.intent())?;
    let lengths = [
        require_encoded_size(request, OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1)?,
        require_encoded_size(
            authorization,
            OFFLINE_CASH_ACCEPTANCE_INTENT_AUTHORIZATION_MAX_BYTES_V1,
        )?,
        require_encoded_size(ticket, OFFLINE_CASH_ACCEPTANCE_TICKET_MAX_BYTES_V1)?,
    ];
    let raw = lengths.iter().sum::<usize>();
    if raw > OFFLINE_CASH_PRE_TICKET_EXCHANGE_MAX_BYTES_V1 {
        return Err(OfflineCashValidationErrorV1::EncodedSizeExceeded {
            actual: raw,
            max: OFFLINE_CASH_PRE_TICKET_EXCHANGE_MAX_BYTES_V1,
        });
    }
    let text = lengths
        .iter()
        .map(|length| OFFLINE_CASH_TEXT_PREFIX_V1.len() + unpadded_base64url_len(*length))
        .sum::<usize>();
    if text > OFFLINE_CASH_PRE_TICKET_TEXT_EXCHANGE_MAX_BYTES_V1 {
        return Err(OfflineCashValidationErrorV1::EncodedSizeExceeded {
            actual: text,
            max: OFFLINE_CASH_PRE_TICKET_TEXT_EXCHANGE_MAX_BYTES_V1,
        });
    }
    Ok(raw)
}

/// Validate all five transported Offline Cash messages and return their raw size.
///
/// The ticket is transported once from receiver to sender and then embedded in
/// the payment transported back. Its standalone bytes therefore count in
/// addition to the payment envelope. The historical 9,211/12,288-byte limits
/// continue to govern the terminal request/payment/ack trio; these separate
/// bounds cover the proof-bearing pre-ticket leg and the full exchange.
///
/// # Errors
///
/// Returns an error for a substituted intent/ticket, any invalid message, or
/// an aggregate raw/text size overrun.
pub fn validate_offline_cash_complete_exchange_shape_v1(
    request: &OfflineCashPaymentRequestV1,
    authorization: &OfflineCashAcceptanceIntentAuthorizationV1,
    ticket: &OfflineCashAcceptanceTicketV1,
    payment: &OfflineCashPaymentV1,
    acknowledgement: &OfflineCashAcknowledgementV1,
) -> Result<usize, OfflineCashValidationErrorV1> {
    validate_offline_cash_pre_ticket_exchange_shape_v1(request, authorization, ticket)?;
    validate_offline_cash_session_shape_v1(request, payment, acknowledgement)?;
    if payment.acceptance_intent != authorization.intent() || &payment.acceptance_ticket != ticket {
        return Err(invalid("offline_cash.complete_exchange.binding"));
    }
    let lengths = [
        require_encoded_size(request, OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1)?,
        require_encoded_size(
            authorization,
            OFFLINE_CASH_ACCEPTANCE_INTENT_AUTHORIZATION_MAX_BYTES_V1,
        )?,
        require_encoded_size(ticket, OFFLINE_CASH_ACCEPTANCE_TICKET_MAX_BYTES_V1)?,
        require_encoded_size(payment, OFFLINE_CASH_PAYMENT_MAX_BYTES_V1)?,
        require_encoded_size(acknowledgement, OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1)?,
    ];
    let raw = lengths.iter().sum::<usize>();
    if raw > OFFLINE_CASH_COMPLETE_EXCHANGE_MAX_BYTES_V1 {
        return Err(OfflineCashValidationErrorV1::EncodedSizeExceeded {
            actual: raw,
            max: OFFLINE_CASH_COMPLETE_EXCHANGE_MAX_BYTES_V1,
        });
    }
    let text = lengths
        .iter()
        .map(|length| OFFLINE_CASH_TEXT_PREFIX_V1.len() + unpadded_base64url_len(*length))
        .sum::<usize>();
    if text > OFFLINE_CASH_COMPLETE_TEXT_EXCHANGE_MAX_BYTES_V1 {
        return Err(OfflineCashValidationErrorV1::EncodedSizeExceeded {
            actual: text,
            max: OFFLINE_CASH_COMPLETE_TEXT_EXCHANGE_MAX_BYTES_V1,
        });
    }
    Ok(raw)
}

impl OfflineCashMintAuthorizationContextV1 {
    /// Validate the exact pre-ID recipient, asset, release, and commitment context.
    ///
    /// # Errors
    ///
    /// Returns an error for a reserved value, invalid asset incarnation, or
    /// non-canonical pooled-reserve identity.
    pub fn validate_shape(&self) -> Result<(), OfflineCashValidationErrorV1> {
        require_valid_header(
            self.version,
            &self.network_id,
            self.scale,
            Some(self.amount),
            "offline_cash.mint_authorization_context.header",
        )?;
        self.asset_incarnation
            .validate()
            .map_err(|_| invalid("offline_cash.mint_authorization_context.asset_incarnation"))?;
        for (field, value) in [
            (
                "offline_cash.mint_authorization_context.operation_id",
                self.operation_id,
            ),
            (
                "offline_cash.mint_authorization_context.release_id",
                self.release_id,
            ),
            (
                "offline_cash.mint_authorization_context.suite_id",
                self.suite_id,
            ),
            (
                "offline_cash.mint_authorization_context.vk_digest",
                self.vk_digest,
            ),
            (
                "offline_cash.mint_authorization_context.artifact_manifest_digest",
                self.artifact_manifest_digest,
            ),
            (
                "offline_cash.mint_authorization_context.liability_pool_id",
                self.liability_pool_id,
            ),
            (
                "offline_cash.mint_authorization_context.hardware_credential_id",
                self.hardware_credential_id,
            ),
            (
                "offline_cash.mint_authorization_context.hardware_profile_id",
                self.hardware_profile_id,
            ),
            (
                "offline_cash.mint_authorization_context.recipient_credential_commitment",
                self.recipient_credential_commitment,
            ),
            (
                "offline_cash.mint_authorization_context.credit_commitment",
                self.credit_commitment,
            ),
            (
                "offline_cash.mint_authorization_context.recipient_one_time_key",
                self.recipient_one_time_key,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        require_valid_x25519_public_key(
            "offline_cash.mint_authorization_context.recipient_one_time_key",
            self.recipient_one_time_key,
        )?;
        if self.policy_epoch == 0
            || self.liability_pool_id
                != offline_cash_liability_pool_id_v1(
                    &self.network_id,
                    &self.asset,
                    self.asset_incarnation,
                )?
        {
            return Err(invalid("offline_cash.mint_authorization_context.binding"));
        }
        Ok(())
    }

    /// Return the pre-ID digest included in issuance and credit identities.
    ///
    /// # Errors
    ///
    /// Returns an error when shape validation or canonical encoding fails.
    pub fn canonical_digest(&self) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        self.validate_shape()?;
        digest_encoded(MINT_AUTHORIZATION_CONTEXT_DIGEST_DOMAIN, self)
    }
}

impl OfflineCashMintAuthorizationStatementV1 {
    /// Validate the complete post-encryption statement without granting authority.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid context, version, identifier, or ciphertext binding.
    pub fn validate_shape(&self) -> Result<(), OfflineCashValidationErrorV1> {
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1 || self.context.version != self.version {
            return Err(invalid("offline_cash.mint_authorization_statement.version"));
        }
        self.context.validate_shape()?;
        for (field, value) in [
            (
                "offline_cash.mint_authorization_statement.issuance_commitment",
                self.issuance_commitment,
            ),
            (
                "offline_cash.mint_authorization_statement.credit_id",
                self.credit_id,
            ),
            (
                "offline_cash.mint_authorization_statement.ciphertext_digest",
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
    pub fn canonical_digest(&self) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
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
    ) -> Result<OfflineCashEncryptedCreditAadV1, OfflineCashValidationErrorV1> {
        OfflineCashEncryptedCreditAadV1::for_mint(self)
    }
}

impl OfflineCashMintAuthorizationV1 {
    /// Validate proof framing and exact statement binding without granting authority.
    ///
    /// Core must resolve the named release, profile, suite, verifying keys, and
    /// artifact manifest from authenticated state and cryptographically verify
    /// both proof parities before mutating payer balance or pooled reserve.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid statement/proof binding or encoded size.
    pub fn validate_shape(&self) -> Result<(), OfflineCashValidationErrorV1> {
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1 || self.statement.version != self.version {
            return Err(invalid("offline_cash.mint_authorization.version"));
        }
        let semantic_digest = self.statement.canonical_digest()?;
        self.proof
            .validate_shape_for_semantic_digest(semantic_digest)?;
        require_encoded_size(self, OFFLINE_CASH_MINT_AUTHORIZATION_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Return the digest recursively bound by the finalized mint helper.
    ///
    /// # Errors
    ///
    /// Returns an error when shape validation or canonical encoding fails.
    pub fn canonical_digest(&self) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        self.validate_shape()?;
        digest_encoded(MINT_AUTHORIZATION_DIGEST_DOMAIN, self)
    }

    /// Encode one shape-validated mint authorization as `oc1:` text.
    ///
    /// # Errors
    ///
    /// Returns an error when validation, encoding, or a size bound fails.
    pub fn encode_text_shape(&self) -> Result<String, OfflineCashValidationErrorV1> {
        self.validate_shape()?;
        encode_offline_cash_text_v1(
            self,
            OFFLINE_CASH_MINT_AUTHORIZATION_MAX_BYTES_V1,
            OFFLINE_CASH_MINT_AUTHORIZATION_TEXT_MAX_BYTES_V1,
        )
    }

    /// Decode one exact canonical mint authorization without granting authority.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid size, text framing, canonical bytes, or shape.
    pub fn decode_text_shape_exact(text: &str) -> Result<Self, OfflineCashValidationErrorV1> {
        decode_offline_cash_text_v1(
            text,
            OFFLINE_CASH_MINT_AUTHORIZATION_MAX_BYTES_V1,
            OFFLINE_CASH_MINT_AUTHORIZATION_TEXT_MAX_BYTES_V1,
            Self::decode_canonical_shape_exact,
        )
    }

    /// Decode one exact bounded mint authorization without granting authority.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized, malformed, non-canonical, or invalid bytes.
    pub fn decode_canonical_shape_exact(
        bytes: &[u8],
    ) -> Result<Self, OfflineCashValidationErrorV1> {
        let authorization: Self =
            decode_bounded_canonical(bytes, OFFLINE_CASH_MINT_AUTHORIZATION_MAX_BYTES_V1)?;
        authorization.validate_shape()?;
        Ok(authorization)
    }
}

impl OfflineCashMintCreditStatementV1 {
    fn lifecycle_context_digest(&self) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
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

    fn credit_id_preimage(&self) -> Result<MintCreditIdPreimageV1, OfflineCashValidationErrorV1> {
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
    pub fn expected_credit_id(&self) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        digest_encoded(MINT_CREDIT_ID_DOMAIN, &self.credit_id_preimage()?)
    }

    /// Populate the canonical mint-credit identity.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn seal_credit_id(mut self) -> Result<Self, OfflineCashValidationErrorV1> {
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
    pub fn validate_shape(&self) -> Result<(), OfflineCashValidationErrorV1> {
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1
            || self.lifecycle.version != self.version
            || self.amount == 0
            || self.minted_at_ms == 0
        {
            return Err(invalid("offline_cash.mint_statement.header"));
        }
        self.lifecycle.validate()?;
        for (field, value) in [
            (
                "offline_cash.mint_statement.recipient_credential_commitment",
                self.recipient_credential_commitment,
            ),
            (
                "offline_cash.mint_statement.authorization_context_digest",
                self.authorization_context_digest,
            ),
            (
                "offline_cash.mint_statement.mint_authorization_digest",
                self.mint_authorization_digest,
            ),
            (
                "offline_cash.mint_statement.issuance_commitment",
                self.issuance_commitment,
            ),
            (
                "offline_cash.mint_statement.credit_commitment",
                self.credit_commitment,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        if self.lifecycle.operation_kind != OfflineCashOperationKindV1::MintFold
            || self.lifecycle.credit_id != self.expected_credit_id()?
        {
            return Err(invalid("offline_cash.mint_statement.credit_id"));
        }
        Ok(())
    }

    /// Return the mint statement digest constrained by both proof parities.
    ///
    /// # Errors
    ///
    /// Returns an error when the statement is invalid or cannot be encoded.
    pub fn canonical_digest(&self) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        self.validate_shape()?;
        digest_encoded(MINT_STATEMENT_DIGEST_DOMAIN, self)
    }
}

impl OfflineCashMintCreditV1 {
    /// Encode this shape-validated mint credit as canonical `oc1:` text.
    ///
    /// # Errors
    ///
    /// Returns an error when validation, encoding, or a size bound fails.
    pub fn encode_text_shape(&self) -> Result<String, OfflineCashValidationErrorV1> {
        self.validate_shape()?;
        encode_offline_cash_text_v1(
            self,
            OFFLINE_CASH_MINT_CREDIT_MAX_BYTES_V1,
            OFFLINE_CASH_MINT_CREDIT_TEXT_MAX_BYTES_V1,
        )
    }

    /// Decode one exact canonical unpadded `oc1:` mint credit.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid size, prefix, padding, base64url, Norito, or credit data.
    pub fn decode_text_shape_exact(text: &str) -> Result<Self, OfflineCashValidationErrorV1> {
        decode_offline_cash_text_v1(
            text,
            OFFLINE_CASH_MINT_CREDIT_MAX_BYTES_V1,
            OFFLINE_CASH_MINT_CREDIT_TEXT_MAX_BYTES_V1,
            Self::decode_canonical_shape_exact,
        )
    }

    /// Decode and validate one exact bounded top-up mint credit.
    ///
    /// # Errors
    ///
    /// Returns an error for an oversized, malformed, non-canonical, or invalid credit.
    pub fn decode_canonical_shape_exact(
        bytes: &[u8],
    ) -> Result<Self, OfflineCashValidationErrorV1> {
        let credit: Self = decode_bounded_canonical(bytes, OFFLINE_CASH_MINT_CREDIT_MAX_BYTES_V1)?;
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
    pub fn validate_shape(&self) -> Result<(), OfflineCashValidationErrorV1> {
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1 || self.statement.version != self.version {
            return Err(invalid("offline_cash.mint_credit.version"));
        }
        self.statement.validate_shape()?;
        self.proof
            .validate_shape_for_semantic_digest(self.statement.canonical_digest()?)?;
        for (field, value) in [
            (
                "offline_cash.mint_credit.finality_certificate_binding",
                self.finality_certificate_binding,
            ),
            (
                "offline_cash.mint_credit.finality_authority_head",
                self.finality_authority_head,
            ),
            (
                "offline_cash.mint_credit.finality_genesis_roster_id",
                self.finality_genesis_roster_id,
            ),
            (
                "offline_cash.mint_credit.finality_proof_binding_digest",
                self.finality_proof_binding_digest,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        OfflineCashEncryptedCreditEnvelopeV1::decode_canonical_shape_exact(&self.encrypted_credit)?;
        if self.statement.lifecycle.ciphertext_digest
            != offline_cash_ciphertext_digest_v1(&self.encrypted_credit)
        {
            return Err(invalid("offline_cash.mint_credit.encrypted_credit"));
        }
        require_nonzero(
            "offline_cash.mint_credit.artifact_manifest_digest",
            self.artifact_manifest_digest,
        )?;
        require_encoded_size(self, OFFLINE_CASH_MINT_CREDIT_MAX_BYTES_V1)?;
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
        authorization: &OfflineCashMintAuthorizationV1,
    ) -> Result<(), OfflineCashValidationErrorV1> {
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
                != offline_cash_ciphertext_digest_v1(&self.encrypted_credit)
        {
            return Err(invalid("offline_cash.mint_credit.authorization_binding"));
        }
        OfflineCashEncryptedCreditEnvelopeV1::decode_canonical_shape_exact_against_recipient_key(
            &self.encrypted_credit,
            context.recipient_one_time_key,
        )?;
        authorization.statement.encrypted_credit_aad()?;
        Ok(())
    }
}

impl OfflineCashRedemptionStatementV1 {
    fn redemption_id_preimage(
        &self,
    ) -> Result<RedemptionIdPreimageV1, OfflineCashValidationErrorV1> {
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
    pub fn expected_redemption_id(&self) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        digest_encoded(REDEMPTION_ID_DOMAIN, &self.redemption_id_preimage()?)
    }

    /// Populate the canonical redemption identity.
    ///
    /// # Errors
    ///
    /// Returns an error when identity hashing fails.
    pub fn seal_redemption_id(mut self) -> Result<Self, OfflineCashValidationErrorV1> {
        self.redemption_id = self.expected_redemption_id()?;
        Ok(self)
    }

    /// Return the unlinkable circuit nullifier used for conflict detection.
    ///
    /// # Errors
    ///
    /// Returns an error only when the reserved all-zero value is present.
    pub fn sender_conflict_key(&self) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        require_nonzero(
            "offline_cash.redemption_statement.terminal_nullifier",
            self.terminal_nullifier,
        )?;
        Ok(self.terminal_nullifier)
    }

    /// Validate an unlinkable terminal aggregate-state transition.
    ///
    /// # Errors
    ///
    /// Returns an error when any lifecycle, nullifier, output, or commit binding is invalid.
    pub fn validate_shape(&self) -> Result<(), OfflineCashValidationErrorV1> {
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1 || self.lifecycle.version != self.version {
            return Err(invalid("offline_cash.redemption_statement.version"));
        }
        self.lifecycle.validate()?;
        for (field, value) in [
            (
                "offline_cash.redemption_statement.terminal_nullifier",
                self.terminal_nullifier,
            ),
            (
                "offline_cash.redemption_statement.redemption_commitment",
                self.redemption_commitment,
            ),
            (
                "offline_cash.redemption_statement.redemption_id",
                self.redemption_id,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        if self.amount == 0
            || self.lifecycle.operation_kind != OfflineCashOperationKindV1::RedeemSplit
            || self.terminal_nullifier == self.redemption_commitment
            || self.terminal_nullifier == self.redemption_id
            || self.redemption_commitment == self.redemption_id
        {
            return Err(invalid("offline_cash.redemption_statement.operation"));
        }
        self.commit_evidence.validate()?;
        if self.redemption_id != self.expected_redemption_id()? {
            return Err(invalid("offline_cash.redemption_statement.redemption_id"));
        }
        Ok(())
    }

    /// Return the redemption semantic digest constrained by both proof parities.
    ///
    /// # Errors
    ///
    /// Returns an error when the statement is invalid or cannot be encoded.
    pub fn canonical_digest(&self) -> Result<[u8; 32], OfflineCashValidationErrorV1> {
        self.validate_shape()?;
        digest_encoded(REDEMPTION_STATEMENT_DIGEST_DOMAIN, self)
    }
}

impl OfflineCashRedemptionVoucherV1 {
    /// Encode this shape-validated voucher as canonical `oc1:` text.
    ///
    /// # Errors
    ///
    /// Returns an error when validation, encoding, or a size bound fails.
    pub fn encode_text_shape(&self) -> Result<String, OfflineCashValidationErrorV1> {
        self.validate_shape()?;
        encode_offline_cash_text_v1(
            self,
            OFFLINE_CASH_REDEMPTION_VOUCHER_MAX_BYTES_V1,
            OFFLINE_CASH_REDEMPTION_VOUCHER_TEXT_MAX_BYTES_V1,
        )
    }

    /// Decode one exact canonical unpadded `oc1:` redemption voucher.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid size, prefix, padding, base64url, Norito, or voucher data.
    pub fn decode_text_shape_exact(text: &str) -> Result<Self, OfflineCashValidationErrorV1> {
        decode_offline_cash_text_v1(
            text,
            OFFLINE_CASH_REDEMPTION_VOUCHER_MAX_BYTES_V1,
            OFFLINE_CASH_REDEMPTION_VOUCHER_TEXT_MAX_BYTES_V1,
            Self::decode_canonical_shape_exact,
        )
    }

    /// Decode and validate one exact bounded redemption voucher.
    ///
    /// # Errors
    ///
    /// Returns an error for an oversized, malformed, non-canonical, or invalid voucher.
    pub fn decode_canonical_shape_exact(
        bytes: &[u8],
    ) -> Result<Self, OfflineCashValidationErrorV1> {
        let voucher: Self =
            decode_bounded_canonical(bytes, OFFLINE_CASH_REDEMPTION_VOUCHER_MAX_BYTES_V1)?;
        voucher.validate_shape()?;
        Ok(voucher)
    }

    /// Validate terminal state consumption, certificate, and wrapper binding.
    ///
    /// Global redemption admission must additionally reject a previously seen
    /// `terminal_nullifier`.
    ///
    /// # Errors
    ///
    /// Returns an error when any voucher invariant fails.
    pub fn validate_shape(&self) -> Result<(), OfflineCashValidationErrorV1> {
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1 || self.statement.version != self.version {
            return Err(invalid("offline_cash.redemption_voucher.version"));
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
        self.proof.validate_bindings(
            self.statement.canonical_digest()?,
            self.commit_certificate.candidate_envelope_digest,
            certificate_digest,
        )?;
        require_nonzero(
            "offline_cash.redemption_voucher.artifact_manifest_digest",
            self.artifact_manifest_digest,
        )?;
        require_encoded_size(self, OFFLINE_CASH_REDEMPTION_VOUCHER_MAX_BYTES_V1)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{block::BlockHeader, domain::DomainId};
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};

    const FIXTURE_RELEASE_ID_V1: [u8; 32] = [0x01; 32];
    const FIXTURE_VK_SET_DIGEST_V1: [u8; 32] = [0x37; 32];
    const FIXTURE_ARTIFACT_MANIFEST_DIGEST_V1: [u8; 32] = [0x36; 32];

    fn network() -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"offline-cash-v1",
        )))
    }

    fn asset() -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "xor".parse().expect("asset name"),
        )
    }

    fn asset_incarnation(
        network_id: &NetworkId,
        asset: &AssetDefinitionId,
        ordinal: u64,
    ) -> AxtAssetIncarnationV1 {
        let registration_header = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            [
                b"offline-cash-v1-asset-registration:".as_slice(),
                &ordinal.to_le_bytes(),
            ]
            .concat(),
        ));
        AxtAssetIncarnationV1::derive(
            network_id,
            asset,
            &registration_header,
            &Hash::new(
                [
                    b"offline-cash-v1-asset-execution:".as_slice(),
                    &ordinal.to_le_bytes(),
                ]
                .concat(),
            ),
            ordinal,
        )
    }

    fn account(seed: u8) -> AccountId {
        AccountId::new(
            KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        )
    }

    fn signing_key(seed: u8) -> SigningKey {
        SigningKey::from_bytes((&[seed; 32]).into()).expect("P-256 signing key")
    }

    fn public_key(key: &SigningKey) -> OfflineCashDevicePublicKeyV1 {
        let encoded = key.verifying_key().to_encoded_point(false);
        OfflineCashDevicePublicKeyV1::from_sec1_bytes(encoded.as_bytes()).expect("public key")
    }

    fn sign(key: &SigningKey, bytes: &[u8]) -> OfflineCashDeviceSignatureV1 {
        let signature: Signature = key.sign(bytes);
        let signature = signature.normalize_s().unwrap_or(signature);
        OfflineCashDeviceSignatureV1::from_raw_bytes(signature.to_bytes().as_ref())
            .expect("canonical signature")
    }

    const fn suite_id() -> [u8; 32] {
        [0x23; 32]
    }

    fn hardware_profile() -> OfflineCashHardwareProfileV1 {
        OfflineCashHardwareProfileV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            protocol_version: OFFLINE_CASH_WIRE_VERSION_V1,
            hardware_profile_id: [0; 32],
            provider_id: [1; 32],
            platform_class: OfflineCashHardwarePlatformClassV1::DedicatedSecureElement,
            product_class_digest: [2; 32],
            firmware_policy_digest: [3; 32],
            enrollment_attestation_verifier_digest: [4; 32],
            attestation_trust_roots_digest: [5; 32],
            allowed_suite_commitment: digest_bytes(SUITE_COMMITMENT_DOMAIN, &suite_id()),
            policy_epoch: 1,
            governance_credential_public_key: public_key(&signing_key(0x31)),
            capability_mask: OFFLINE_CASH_HARDWARE_REQUIRED_CAPABILITIES_V1,
            qualification_report_digest: [7; 32],
            valid_from_ms: 1,
            expires_at_ms: 100_000,
        }
        .seal_hardware_profile_id()
        .expect("hardware profile id")
    }

    fn hardware_credential() -> OfflineCashHardwareCredentialV1 {
        let profile = hardware_profile();
        let device_signing_key = signing_key(7);
        let device_public_key = public_key(&device_signing_key);
        let governance_signing_key = signing_key(0x31);
        let mut credential = OfflineCashHardwareCredentialV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            credential_id: [0; 32],
            network_id: network(),
            hardware_profile_id: profile.hardware_profile_id,
            suite_id: suite_id(),
            firmware_policy_digest: profile.firmware_policy_digest,
            policy_epoch: profile.policy_epoch,
            lane_commitment: [0x32; 32],
            hardware_epoch_id: [0x33; 32],
            hardware_epoch_generation: 1,
            device_public_key,
            device_key_reference: offline_cash_device_key_reference_v1(&device_public_key),
            issued_at_ms: 500,
            expires_at_ms: 90_000,
            governance_signature: sign(&governance_signing_key, b"placeholder"),
        }
        .seal_credential_id()
        .expect("credential id");
        credential.governance_signature = sign(
            &governance_signing_key,
            &credential
                .canonical_signing_bytes()
                .expect("credential signing bytes"),
        );
        credential
            .validate_against_profile(&profile)
            .expect("credential validates against profile");
        credential
    }

    #[test]
    fn hardware_profile_requires_every_granular_capability_bit() {
        let capabilities = [
            OFFLINE_CASH_HARDWARE_CAPABILITY_EXACT_NEXT_PREDECESSOR_CONSUMPTION_V1,
            OFFLINE_CASH_HARDWARE_CAPABILITY_ONE_USE_SUCCESSOR_AUTHORIZATION_V1,
            OFFLINE_CASH_HARDWARE_CAPABILITY_ROLLBACK_RESISTANT_COUNTER_AND_JOURNAL_V1,
            OFFLINE_CASH_HARDWARE_CAPABILITY_SEALED_TRANSITION_RECOVERY_V1,
            OFFLINE_CASH_HARDWARE_CAPABILITY_ONE_USE_ACCEPTANCE_TICKETS_V1,
            OFFLINE_CASH_HARDWARE_CAPABILITY_DURABLE_INBOX_RESERVATION_V1,
            OFFLINE_CASH_HARDWARE_CAPABILITY_AUTHENTICATED_INBOUND_STAGING_V1,
            OFFLINE_CASH_HARDWARE_CAPABILITY_AUTHORITATIVE_REPLAY_ROOT_RECOVERY_V1,
            OFFLINE_CASH_HARDWARE_CAPABILITY_SENDER_OUTBOX_RESERVATION_V1,
            OFFLINE_CASH_HARDWARE_CAPABILITY_AUTHENTICATED_DURABLE_RETRY_OUTBOX_V1,
            OFFLINE_CASH_HARDWARE_CAPABILITY_ATOMIC_VERIFIED_CANDIDATE_COMMIT_V1,
            OFFLINE_CASH_HARDWARE_CAPABILITY_RECOVERABLE_TERMINAL_COMMIT_CERTIFICATE_V1,
            OFFLINE_CASH_HARDWARE_CAPABILITY_TRUSTED_TIME_OR_LEASE_V1,
            OFFLINE_CASH_HARDWARE_CAPABILITY_OFFLINE_HARDWARE_EPOCH_ROTATION_V1,
            OFFLINE_CASH_HARDWARE_CAPABILITY_ROLLBACK_SAFE_COUNTER_ROLLOVER_V1,
            OFFLINE_CASH_HARDWARE_CAPABILITY_NO_SOFTWARE_FALLBACK_V1,
        ];
        let expected: [u16; 16] = core::array::from_fn(|index| 1_u16 << index);
        assert_eq!(capabilities, expected);
        assert_eq!(OFFLINE_CASH_HARDWARE_REQUIRED_CAPABILITIES_V1, u16::MAX);

        let profile = hardware_profile();
        profile.validate().expect("complete profile validates");
        for capability in capabilities {
            let mut incomplete = profile;
            incomplete.capability_mask &= !capability;
            let incomplete = incomplete
                .seal_hardware_profile_id()
                .expect("incomplete profile id");
            assert!(
                incomplete.validate().is_err(),
                "profile omitted capability {capability:#06x}"
            );
        }
    }

    #[test]
    fn hardware_profile_identity_and_credential_suite_are_exactly_bound() {
        let profile = hardware_profile();
        assert_eq!(
            profile.hardware_profile_id,
            profile.expected_hardware_profile_id().expect("profile id")
        );
        let mut tampered = profile;
        tampered.product_class_digest = [0xEF; 32];
        assert!(tampered.validate().is_err());

        let credential = hardware_credential();
        credential
            .validate_against_profile(&profile)
            .expect("credential profile binding");
        let governance_signing_key = signing_key(0x31);
        let mut wrong_suite = credential;
        wrong_suite.suite_id = [0xEE; 32];
        wrong_suite = wrong_suite
            .seal_credential_id()
            .expect("wrong-suite credential id");
        wrong_suite.governance_signature = sign(
            &governance_signing_key,
            &wrong_suite
                .canonical_signing_bytes()
                .expect("wrong-suite signing bytes"),
        );
        assert!(wrong_suite.validate_against_profile(&profile).is_err());
    }

    #[test]
    fn self_consistent_request_credential_is_not_release_authority() {
        let legitimate_profile = hardware_profile();
        let bogus_governance_key = signing_key(0x41);
        let mut bogus_profile = legitimate_profile;
        bogus_profile.provider_id = [0x42; 32];
        bogus_profile.governance_credential_public_key = public_key(&bogus_governance_key);
        bogus_profile = bogus_profile
            .seal_hardware_profile_id()
            .expect("bogus profile identity");

        let mut bogus_credential = hardware_credential();
        bogus_credential.hardware_profile_id = bogus_profile.hardware_profile_id;
        bogus_credential = bogus_credential
            .seal_credential_id()
            .expect("bogus credential identity");
        bogus_credential.governance_signature = sign(
            &bogus_governance_key,
            &bogus_credential
                .canonical_signing_bytes()
                .expect("bogus credential bytes"),
        );

        let mut request = request();
        request.hardware_credential = bogus_credential;
        request.signature = sign(
            &signing_key(7),
            &request
                .canonical_signing_bytes()
                .expect("bogus request signing bytes"),
        );
        request
            .validate_shape()
            .expect("self-consistent untrusted credential remains parseable");
        request
            .validate_against_profile(&bogus_profile)
            .expect("bogus profile is internally consistent");
        assert!(
            request
                .validate_against_profile(&legitimate_profile)
                .is_err()
        );
    }

    #[test]
    fn all_request_modes_are_canonical_positive_policies() {
        let interval = OfflineCashAmountPolicyV1 {
            minimum_amount: 1,
            maximum_amount: 100,
        };
        for mode in [
            OfflineCashPaymentRequestModeV1::SingleExact(OfflineCashSingleExactRequestV1 {
                amount: 10,
            }),
            OfflineCashPaymentRequestModeV1::PartialUntilTotal(
                OfflineCashPartialUntilTotalRequestV1 { total_amount: 100 },
            ),
            OfflineCashPaymentRequestModeV1::BoundedMultiPayment(
                OfflineCashBoundedMultiPaymentRequestV1 {
                    max_payments: 3,
                    per_payment: interval,
                },
            ),
            OfflineCashPaymentRequestModeV1::OpenReceive(OfflineCashOpenReceiveRequestV1 {
                per_payment: interval,
            }),
        ] {
            mode.validate().expect("valid mode");
            assert_ne!(mode.canonical_digest().expect("mode digest"), [0; 32]);
        }
        assert!(
            OfflineCashPaymentRequestModeV1::SingleExact(OfflineCashSingleExactRequestV1 {
                amount: 10
            })
            .accepts_exact_amount(10)
        );
        assert!(
            !OfflineCashPaymentRequestModeV1::SingleExact(OfflineCashSingleExactRequestV1 {
                amount: 10
            })
            .accepts_exact_amount(9)
        );
        assert!(
            OfflineCashPaymentRequestModeV1::PartialUntilTotal(
                OfflineCashPartialUntilTotalRequestV1 { total_amount: 100 }
            )
            .accepts_exact_amount(100)
        );
        assert!(
            !OfflineCashPaymentRequestModeV1::PartialUntilTotal(
                OfflineCashPartialUntilTotalRequestV1 { total_amount: 100 }
            )
            .accepts_exact_amount(101)
        );
    }

    #[test]
    fn capacity_reservations_reject_operation_specific_under_reservation() {
        let request = request();
        let intent = acceptance_intent(&request);
        let mut ticket = acceptance_ticket(&request, &intent);
        ticket.reserved_inbox_bytes =
            OFFLINE_CASH_ACCEPTANCE_TICKET_MIN_RESERVED_INBOX_BYTES_V1 - 1;
        ticket.signature = sign(
            &signing_key(7),
            &ticket
                .canonical_signing_bytes()
                .expect("under-reserved ticket bytes"),
        );
        assert!(ticket.validate_shape_against(&request, &intent).is_err());

        for (operation_kind, minimum) in [
            (
                OfflineCashOperationKindV1::SendSplit,
                OFFLINE_CASH_PAYMENT_OUTBOX_MIN_BYTES_V1,
            ),
            (
                OfflineCashOperationKindV1::RedeemSplit,
                OFFLINE_CASH_REDEMPTION_OUTBOX_MIN_BYTES_V1,
            ),
        ] {
            let reservation = OfflineCashOutboxReservationV1 {
                reservation_id: [0xFA; 32],
                operation_kind,
                reserved_outbox_bytes: minimum,
                issued_at_ms: 1,
                expires_at_ms: 2,
            };
            reservation
                .canonical_commitment()
                .expect("exact outbox minimum");
            assert!(
                OfflineCashOutboxReservationV1 {
                    reserved_outbox_bytes: minimum - 1,
                    ..reservation
                }
                .canonical_commitment()
                .is_err()
            );
        }
    }

    fn state_pair(tag: u8) -> OfflineCashPastaStateCommitmentV1 {
        OfflineCashPastaStateCommitmentV1 {
            eq: [tag; 32],
            ep: [tag.wrapping_add(1); 32],
        }
    }

    fn paired_proof(
        semantic_digest: [u8; 32],
        _predecessor_state: OfflineCashPastaStateCommitmentV1,
        _successor_state: OfflineCashPastaStateCommitmentV1,
    ) -> OfflineCashPairedProofV1 {
        OfflineCashPairedProofV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            eq_protocol_digest: [0x15; 32],
            ep_protocol_digest: [0x16; 32],
            semantic_digest,
            guard_eq_credential_audit: [0x19; 32],
            guard_ep_credential_audit: [0x1A; 32],
            eq_deferred_audit: [0x13; 32],
            ep_deferred_audit: [0x14; 32],
            eq_proof: vec![0xA1; 128],
            ep_proof: vec![0xB2; 128],
            eq_history: vec![0xC3; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
            ep_history: vec![0xD4; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
        }
    }

    fn encrypted_credit_fixture(recipient_one_time_key: [u8; 32], tag: u8) -> Vec<u8> {
        let mut ephemeral_x25519_public_key = [0; 32];
        ephemeral_x25519_public_key[0] = 9;
        OfflineCashEncryptedCreditEnvelopeV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            ephemeral_x25519_public_key,
            nonce: [tag; OFFLINE_CASH_XCHACHA20POLY1305_NONCE_BYTES_V1],
            ciphertext_and_tag: vec![
                tag;
                offline_cash_credit_opening_canonical_len_v1()
                    .expect("opening length")
                    + OFFLINE_CASH_XCHACHA20POLY1305_TAG_BYTES_V1
            ],
        }
        .canonical_bytes_against_recipient_key(recipient_one_time_key)
        .expect("canonical encrypted credit fixture")
    }

    fn seal_commit_certificate(
        certificate: OfflineCashCommitCertificateV1,
    ) -> OfflineCashCommitCertificateV1 {
        let body = OfflineCashHardwareTerminalBodyV1 {
            version: certificate.version,
            candidate_envelope_digest: certificate.candidate_envelope_digest,
            lifecycle_binding_digest: certificate.lifecycle_binding_digest,
            transition_nullifier: certificate.transition_nullifier,
            outbox_reservation_commitment: certificate.outbox_reservation_commitment,
            commit_evidence: certificate.commit_evidence,
            hardware_profile_id: certificate.hardware_profile_id,
            policy_epoch: certificate.policy_epoch,
            private_successor_commitment: [0xE1; 32],
            private_journal_commitment: [0xE2; 32],
            private_recovery_commitment: [0xE3; 32],
        };
        certificate
            .seal_with_terminal_body(&body)
            .expect("self-free terminal certificate")
    }

    fn request() -> OfflineCashPaymentRequestV1 {
        let signing_key = signing_key(7);
        let network_id = network();
        let asset = asset();
        let asset_incarnation = asset_incarnation(&network_id, &asset, 1);
        let placeholder = sign(&signing_key, b"placeholder");
        let mut request = OfflineCashPaymentRequestV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            release_id: FIXTURE_RELEASE_ID_V1,
            network_id,
            liability_pool_id: offline_cash_liability_pool_id_v1(
                &network_id,
                &asset,
                asset_incarnation,
            )
            .expect("liability pool"),
            asset,
            asset_incarnation,
            scale: 4,
            recipient: account(0xA5),
            request_mode: OfflineCashPaymentRequestModeV1::SingleExact(
                OfflineCashSingleExactRequestV1 { amount: 12_345 },
            ),
            hardware_credential: hardware_credential(),
            request_id: [5; 32],
            issued_at_ms: 1_000,
            expires_at_ms: 61_000,
            signature: placeholder,
        };
        request.signature = sign(
            &signing_key,
            &request.canonical_signing_bytes().expect("request bytes"),
        );
        request
    }

    fn acceptance_intent(request: &OfflineCashPaymentRequestV1) -> OfflineCashAcceptanceIntentV1 {
        OfflineCashAcceptanceIntentV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            request_digest: request.canonical_digest().expect("request digest"),
            intent_id: [0x32; 32],
            exact_amount: 12_345,
            sender_one_time_commitment: [0x33; 32],
        }
    }

    fn acceptance_intent_authorization(
        request: &OfflineCashPaymentRequestV1,
    ) -> OfflineCashAcceptanceIntentAuthorizationV1 {
        let intent = acceptance_intent(request);
        let statement = OfflineCashAcceptanceIntentAuthorizationStatementV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            intent,
            release_id: request.release_id,
            suite_id: request.hardware_credential.suite_id,
            vk_digest: FIXTURE_VK_SET_DIGEST_V1,
            artifact_manifest_digest: FIXTURE_ARTIFACT_MANIFEST_DIGEST_V1,
        };
        let semantic_digest = statement
            .canonical_digest_against(request)
            .expect("authorization statement digest");
        OfflineCashAcceptanceIntentAuthorizationV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            statement,
            proof: paired_proof(
                semantic_digest,
                OfflineCashPastaStateCommitmentV1::ZERO,
                OfflineCashPastaStateCommitmentV1::ZERO,
            ),
        }
    }

    fn acceptance_ticket(
        request: &OfflineCashPaymentRequestV1,
        intent: &OfflineCashAcceptanceIntentV1,
    ) -> OfflineCashAcceptanceTicketV1 {
        let signing_key = signing_key(7);
        let request_digest = request.canonical_digest().expect("request digest");
        let intent_digest = intent
            .canonical_digest_against(request)
            .expect("intent digest");
        let mut ticket = OfflineCashAcceptanceTicketV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            network_id: request.network_id,
            request_id: request.request_id,
            request_digest,
            acceptance_ticket_id: [0x34; 32],
            asset: request.asset.clone(),
            asset_incarnation: request.asset_incarnation,
            scale: request.scale,
            request_mode: request.request_mode,
            intent_digest,
            exact_amount: intent.exact_amount,
            reserved_inbox_bytes: OFFLINE_CASH_ACCEPTANCE_TICKET_MIN_RESERVED_INBOX_BYTES_V1,
            recipient_one_time_key: [0x35; 32],
            hardware_profile_id: request.hardware_credential.hardware_profile_id,
            policy_epoch: request.hardware_credential.policy_epoch,
            issued_at_ms: request.issued_at_ms,
            expires_at_ms: request.expires_at_ms,
            signature: sign(&signing_key, b"placeholder"),
        };
        ticket.signature = sign(
            &signing_key,
            &ticket
                .canonical_signing_bytes()
                .expect("ticket signing bytes"),
        );
        ticket
            .validate_shape_against(request, intent)
            .expect("acceptance ticket");
        ticket
    }

    fn no_commit_closure(
        request: &OfflineCashPaymentRequestV1,
        authorization: &OfflineCashAcceptanceIntentAuthorizationV1,
        ticket: &OfflineCashAcceptanceTicketV1,
    ) -> OfflineCashNoCommitClosureV1 {
        let intent = authorization.intent();
        let statement = OfflineCashNoCommitClosureStatementV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            release_id: authorization.statement.release_id,
            suite_id: authorization.statement.suite_id,
            vk_digest: authorization.statement.vk_digest,
            artifact_manifest_digest: authorization.statement.artifact_manifest_digest,
            sender_hardware_binding_commitment: [0x46; 32],
            request_id: request.request_id,
            request_digest: request.canonical_digest().expect("request digest"),
            acceptance_ticket_id: ticket.acceptance_ticket_id,
            ticket_digest: ticket
                .canonical_digest_against(request, &intent)
                .expect("ticket digest"),
            intent_authorization_digest: authorization
                .canonical_digest_against(request)
                .expect("authorization digest"),
            intent_digest: intent
                .canonical_digest_against(request)
                .expect("intent digest"),
            exact_amount: intent.exact_amount,
            sender_one_time_commitment: intent.sender_one_time_commitment,
            recovery_id: [0x47; 32],
            cancellation_nullifier: [0x48; 32],
            equivalent_delivery_slot_commitment: [0x49; 32],
        };
        let closure = OfflineCashNoCommitClosureV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            proof: paired_proof(
                statement
                    .canonical_digest()
                    .expect("no-commit statement digest"),
                OfflineCashPastaStateCommitmentV1::ZERO,
                OfflineCashPastaStateCommitmentV1::ZERO,
            ),
            statement,
            request: request.clone(),
            intent_authorization: authorization.clone(),
            acceptance_ticket: ticket.clone(),
        };
        closure.validate_shape().expect("no-commit closure");
        closure
    }

    fn payment(request: &OfflineCashPaymentRequestV1) -> OfflineCashPaymentV1 {
        let request_digest = request.canonical_digest().expect("request digest");
        let acceptance_intent = acceptance_intent(request);
        let acceptance_ticket = acceptance_ticket(request, &acceptance_intent);
        let acceptance_ticket_digest = acceptance_ticket
            .canonical_digest_against(request, &acceptance_intent)
            .expect("acceptance ticket digest");
        let encrypted_credit =
            encrypted_credit_fixture(acceptance_ticket.recipient_one_time_key, 0xE5);
        let commit_evidence =
            OfflineCashCommitEvidenceV1::TrustedTime(OfflineCashTrustedCommitTimeV1 {
                time_evidence_commitment: [0x36; 32],
            });
        let statement = OfflineCashTransferStatementV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            lifecycle: OfflineCashLifecycleBindingV1 {
                version: OFFLINE_CASH_WIRE_VERSION_V1,
                network_id: request.network_id,
                protocol_version: OFFLINE_CASH_WIRE_VERSION_V1,
                suite_id: request.hardware_credential.suite_id,
                vk_digest: FIXTURE_VK_SET_DIGEST_V1,
                release_id: request.release_id,
                asset: request.asset.clone(),
                asset_incarnation: request.asset_incarnation,
                scale: request.scale,
                liability_pool_id: request.liability_pool_id,
                hardware_profile_id: request.hardware_credential.hardware_profile_id,
                policy_epoch: request.hardware_credential.policy_epoch,
                operation_kind: OfflineCashOperationKindV1::SendSplit,
                request_id: request.request_id,
                acceptance_ticket_id: acceptance_ticket.acceptance_ticket_id,
                credit_id: [0; 32],
                ciphertext_digest: offline_cash_ciphertext_digest_v1(&encrypted_credit),
            },
            amount: 12_345,
            transition_nullifier: [0x38; 32],
            request_digest,
            acceptance_ticket_digest,
            recipient_one_time_key: acceptance_ticket.recipient_one_time_key,
            ciphertext_commitment: [0x39; 32],
            commit_evidence,
        }
        .seal_credit_id()
        .expect("seal credit id");
        let semantic_digest = statement.canonical_digest().expect("statement digest");
        let commit_certificate = seal_commit_certificate(OfflineCashCommitCertificateV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            certificate_id: [0; 32],
            candidate_envelope_digest: [0x3A; 32],
            lifecycle_binding_digest: statement
                .lifecycle
                .canonical_digest()
                .expect("lifecycle digest"),
            transition_nullifier: statement.transition_nullifier,
            outbox_reservation_commitment: OfflineCashOutboxReservationV1 {
                reservation_id: [0x3B; 32],
                operation_kind: OfflineCashOperationKindV1::SendSplit,
                reserved_outbox_bytes: OFFLINE_CASH_PAYMENT_OUTBOX_MIN_BYTES_V1,
                issued_at_ms: request.issued_at_ms,
                expires_at_ms: request.expires_at_ms,
            }
            .canonical_commitment()
            .expect("outbox commitment"),
            commit_evidence,
            hardware_profile_id: statement.lifecycle.hardware_profile_id,
            policy_epoch: statement.lifecycle.policy_epoch,
            hardware_terminal_commitment: [0; 32],
        });
        let commit_certificate_digest = commit_certificate
            .canonical_digest_against(
                &statement.lifecycle,
                commit_evidence,
                statement.transition_nullifier,
            )
            .expect("certificate digest");
        OfflineCashPaymentV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            statement,
            acceptance_intent,
            acceptance_ticket,
            commit_certificate: commit_certificate.clone(),
            proof: OfflineCashCommitWrapperProofV1 {
                version: OFFLINE_CASH_WIRE_VERSION_V1,
                eq_protocol_digest: [0x15; 32],
                ep_protocol_digest: [0x16; 32],
                semantic_digest,
                candidate_envelope_digest: commit_certificate.candidate_envelope_digest,
                commit_certificate_digest,
                eq_deferred_audit: [0x13; 32],
                ep_deferred_audit: [0x14; 32],
                eq_proof: vec![0xA1; 128],
                ep_proof: vec![0xB2; 128],
                eq_history: vec![0xC3; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
                ep_history: vec![0xD4; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
            },
            encrypted_credit,
            artifact_manifest_digest: FIXTURE_ARTIFACT_MANIFEST_DIGEST_V1,
        }
    }

    fn reseal_payment(payment: &mut OfflineCashPaymentV1) {
        payment.statement.lifecycle.credit_id = [0; 32];
        payment.statement = payment
            .statement
            .clone()
            .seal_credit_id()
            .expect("reseal credit id");
        payment.commit_certificate.lifecycle_binding_digest = payment
            .statement
            .lifecycle
            .canonical_digest()
            .expect("lifecycle digest");
        payment.commit_certificate.transition_nullifier = payment.statement.transition_nullifier;
        payment.commit_certificate.commit_evidence = payment.statement.commit_evidence;
        payment.commit_certificate.certificate_id = [0; 32];
        payment.commit_certificate = payment
            .commit_certificate
            .clone()
            .seal_certificate_id()
            .expect("reseal certificate");
        payment.proof.semantic_digest = payment
            .statement
            .canonical_digest()
            .expect("statement digest");
        payment.proof.candidate_envelope_digest =
            payment.commit_certificate.candidate_envelope_digest;
        payment.proof.commit_certificate_digest = payment
            .commit_certificate
            .canonical_digest_against(
                &payment.statement.lifecycle,
                payment.statement.commit_evidence,
                payment.statement.transition_nullifier,
            )
            .expect("certificate digest");
    }

    fn acknowledgement(
        request: &OfflineCashPaymentRequestV1,
        payment: &OfflineCashPaymentV1,
    ) -> OfflineCashAcknowledgementV1 {
        let signing_key = signing_key(7);
        let request_digest = request.canonical_digest().expect("request digest");
        let payment_digest = payment
            .canonical_digest_against(request)
            .expect("payment digest");
        let staging_hardware_epoch_id = [0xA3; 32];
        let inbox_receipt = OfflineCashInboxReceiptV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            credit_id: payment.statement.lifecycle.credit_id,
            receipt_commitment: offline_cash_inbox_receipt_commitment_v1(
                request.hardware_credential.lane_commitment,
                staging_hardware_epoch_id,
                73,
                payment.statement.lifecycle.credit_id,
                payment_digest,
            )
            .expect("receipt commitment"),
        };
        let mut acknowledgement = OfflineCashAcknowledgementV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            request_digest,
            payment_digest,
            inbox_receipt,
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

    fn aggregate_state() -> OfflineCashAggregateStateCommitmentV1 {
        let network_id = network();
        let asset = asset();
        let asset_incarnation = asset_incarnation(&network_id, &asset, 1);
        OfflineCashAggregateStateCommitmentV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            release_id: FIXTURE_RELEASE_ID_V1,
            network_id,
            liability_pool_id: offline_cash_liability_pool_id_v1(
                &network_id,
                &asset,
                asset_incarnation,
            )
            .expect("liability pool"),
            asset,
            asset_incarnation,
            scale: 4,
            lane_id: [2; 32],
            hardware_epoch_id: [3; 32],
            key_reference: [4; 32],
            hardware_policy_id: [5; 32],
            sequence: u128::from(u64::MAX) + 9,
            state_commitment: [6; 32],
        }
    }

    #[test]
    fn typed_asset_identity_has_one_canonical_guard_digest() {
        let network_id = network();
        let asset = asset();
        let other_asset = AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "rose".parse().expect("asset name"),
        );
        let digest = offline_cash_asset_identity_digest_v1(&asset).expect("asset digest");
        assert_ne!(digest, [0; 32]);
        assert_eq!(
            digest,
            offline_cash_asset_identity_digest_v1(&asset).expect("same asset digest")
        );
        assert_ne!(
            digest,
            offline_cash_asset_identity_digest_v1(&other_asset).expect("other asset digest")
        );
        assert_ne!(*network_id.as_bytes(), [0; 32]);
    }

    #[test]
    fn encrypted_credit_contract_has_exact_acyclic_shape() {
        let request = request();
        let payment = payment(&request);
        let opening = OfflineCashCreditOpeningV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            credit_id: payment.statement.lifecycle.credit_id,
            amount: payment.statement.amount,
            credit_commitment_opening: [0x81; 32],
            recipient_binding_opening: [0x82; 32],
            recovery_nonce: [0x83; 32],
        };
        let opening_bytes = opening.canonical_bytes().expect("credit opening bytes");
        assert_eq!(
            opening_bytes.len(),
            offline_cash_credit_opening_canonical_len_v1().expect("opening length")
        );
        assert_eq!(
            OfflineCashCreditOpeningV1::decode_canonical_shape_exact_against(
                &opening_bytes,
                opening.credit_id,
                opening.amount,
            )
            .expect("credit opening roundtrip"),
            opening
        );
        assert!(
            OfflineCashCreditOpeningV1::decode_canonical_shape_exact_against(
                &opening_bytes,
                [0xFF; 32],
                opening.amount,
            )
            .is_err()
        );

        let aad = OfflineCashEncryptedCreditAadV1::for_peer(
            &payment.statement,
            &request,
            &payment.acceptance_intent,
            &payment.acceptance_ticket,
        )
        .expect("peer AAD");
        assert_eq!(aad.purpose, OfflineCashEncryptedCreditPurposeV1::Peer);
        assert_eq!(aad.credit_id, opening.credit_id);
        assert_eq!(aad.amount, opening.amount);
        let aad_bytes = aad.canonical_bytes().expect("AAD bytes");
        let expected_aad_digest: [u8; 32] = Sha256::digest(aad_bytes).into();
        assert_eq!(
            aad.canonical_digest().expect("AAD digest"),
            expected_aad_digest
        );
        let info = offline_cash_encrypted_credit_kdf_info_v1(&aad).expect("KDF info");
        assert!(info.starts_with(OFFLINE_CASH_ENCRYPTED_CREDIT_KDF_INFO_LABEL_V1));
        assert_eq!(
            info.len(),
            OFFLINE_CASH_ENCRYPTED_CREDIT_KDF_INFO_LABEL_V1.len() + 32
        );

        let envelope = OfflineCashEncryptedCreditEnvelopeV1::decode_canonical_shape_exact_against_recipient_key(
            &payment.encrypted_credit,
            payment.statement.recipient_one_time_key,
        )
        .expect("encrypted-credit envelope");
        let salt = envelope
            .kdf_salt_against_recipient_key(payment.statement.recipient_one_time_key)
            .expect("KDF salt");
        assert_ne!(salt, [0; 32]);
        assert_eq!(
            envelope
                .canonical_bytes_against_recipient_key(payment.statement.recipient_one_time_key,)
                .expect("canonical envelope"),
            payment.encrypted_credit
        );

        let mut low_order_ephemeral = envelope.clone();
        low_order_ephemeral.ephemeral_x25519_public_key = [0; 32];
        assert!(low_order_ephemeral.validate_shape().is_err());
        assert!(
            envelope
                .validate_shape_against_recipient_key([0; 32])
                .is_err()
        );
        let mut wrong_ciphertext_size = envelope;
        wrong_ciphertext_size.ciphertext_and_tag.push(0);
        assert!(wrong_ciphertext_size.validate_shape().is_err());
    }

    #[test]
    fn mint_pre_id_commitments_bind_openings_without_final_identifiers() {
        let credential = hardware_credential();
        let network_id = network();
        let asset = asset();
        let asset_incarnation = asset_incarnation(&network_id, &asset, 1);
        let liability_pool_id =
            offline_cash_liability_pool_id_v1(&network_id, &asset, asset_incarnation)
                .expect("liability pool");
        let recipient = account(0xA5);
        let recipient_one_time_key = [0x73; 32];
        let first = offline_cash_mint_credit_opening_commitment_v1(
            &network_id,
            &asset,
            asset_incarnation,
            4,
            liability_pool_id,
            88_000,
            &recipient,
            recipient_one_time_key,
            [0x91; 32],
        )
        .expect("mint opening commitment");
        let second = offline_cash_mint_credit_opening_commitment_v1(
            &network_id,
            &asset,
            asset_incarnation,
            4,
            liability_pool_id,
            88_000,
            &recipient,
            recipient_one_time_key,
            [0x92; 32],
        )
        .expect("second mint opening commitment");
        assert_ne!(first, second);
        let binding = offline_cash_recipient_credential_commitment_v1(
            [0x71; 32],
            credential.credential_id,
            [0x93; 32],
        )
        .expect("recipient credential commitment");
        assert_ne!(binding, [0; 32]);
        assert!(
            offline_cash_recipient_credential_commitment_v1(
                [0; 32],
                credential.credential_id,
                [0x93; 32],
            )
            .is_err()
        );
    }

    fn mint_exchange() -> (OfflineCashMintAuthorizationV1, OfflineCashMintCreditV1) {
        let credential = hardware_credential();
        let network_id = network();
        let asset = asset();
        let asset_incarnation = asset_incarnation(&network_id, &asset, 1);
        let liability_pool_id =
            offline_cash_liability_pool_id_v1(&network_id, &asset, asset_incarnation)
                .expect("liability pool");
        let recipient_one_time_key = [0x73; 32];
        let encrypted_credit = encrypted_credit_fixture(recipient_one_time_key, 0x91);
        let context = OfflineCashMintAuthorizationContextV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            operation_id: [0x71; 32],
            release_id: FIXTURE_RELEASE_ID_V1,
            suite_id: credential.suite_id,
            vk_digest: FIXTURE_VK_SET_DIGEST_V1,
            artifact_manifest_digest: FIXTURE_ARTIFACT_MANIFEST_DIGEST_V1,
            network_id,
            asset: asset.clone(),
            asset_incarnation,
            scale: 4,
            liability_pool_id,
            amount: 88_000,
            payer: account(0x94),
            recipient: account(0xA5),
            hardware_credential_id: credential.credential_id,
            hardware_profile_id: credential.hardware_profile_id,
            policy_epoch: credential.policy_epoch,
            recipient_credential_commitment: [0x38; 32],
            credit_commitment: [6; 32],
            recipient_one_time_key,
        };
        let authorization_context_digest = context
            .canonical_digest()
            .expect("mint authorization context digest");
        let mut credit_statement = OfflineCashMintCreditStatementV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            lifecycle: OfflineCashLifecycleBindingV1 {
                version: OFFLINE_CASH_WIRE_VERSION_V1,
                network_id,
                protocol_version: OFFLINE_CASH_WIRE_VERSION_V1,
                suite_id: credential.suite_id,
                vk_digest: FIXTURE_VK_SET_DIGEST_V1,
                release_id: FIXTURE_RELEASE_ID_V1,
                asset: asset.clone(),
                asset_incarnation,
                scale: 4,
                liability_pool_id,
                hardware_profile_id: credential.hardware_profile_id,
                policy_epoch: credential.policy_epoch,
                operation_kind: OfflineCashOperationKindV1::MintFold,
                request_id: [0; 32],
                acceptance_ticket_id: [0; 32],
                credit_id: [0; 32],
                ciphertext_digest: offline_cash_ciphertext_digest_v1(&encrypted_credit),
            },
            recipient_credential_commitment: context.recipient_credential_commitment,
            authorization_context_digest,
            // The final authorization digest is deliberately excluded from
            // `expected_credit_id`, breaking the otherwise circular binding.
            mint_authorization_digest: [0x3A; 32],
            amount: context.amount,
            issuance_commitment: [2; 32],
            recipient: context.recipient.clone(),
            credit_commitment: context.credit_commitment,
            minted_at_ms: 5_000,
        }
        .seal_credit_id()
        .expect("seal mint credit");

        let authorization_statement = OfflineCashMintAuthorizationStatementV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            context: context.clone(),
            issuance_commitment: credit_statement.issuance_commitment,
            credit_id: credit_statement.lifecycle.credit_id,
            ciphertext_digest: credit_statement.lifecycle.ciphertext_digest,
        };
        let authorization_semantic_digest = authorization_statement
            .canonical_digest()
            .expect("mint authorization semantic digest");
        let authorization = OfflineCashMintAuthorizationV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            statement: authorization_statement,
            proof: paired_proof(
                authorization_semantic_digest,
                OfflineCashPastaStateCommitmentV1::ZERO,
                OfflineCashPastaStateCommitmentV1::ZERO,
            ),
        };
        credit_statement.mint_authorization_digest = authorization
            .canonical_digest()
            .expect("mint authorization envelope digest");
        credit_statement
            .validate_shape()
            .expect("final bound mint statement");
        let credit = OfflineCashMintCreditV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            proof: paired_proof(
                credit_statement
                    .canonical_digest()
                    .expect("mint statement digest"),
                OfflineCashPastaStateCommitmentV1::ZERO,
                OfflineCashPastaStateCommitmentV1::ZERO,
            ),
            statement: credit_statement,
            finality_certificate_binding: [8; 32],
            finality_authority_head: [9; 32],
            finality_genesis_roster_id: [10; 32],
            finality_proof_binding_digest: [11; 32],
            encrypted_credit,
            artifact_manifest_digest: context.artifact_manifest_digest,
        };
        credit
            .validate_shape_against_authorization(&authorization)
            .expect("mint credit and pre-debit authorization are coherent");
        (authorization, credit)
    }

    fn mint_credit() -> OfflineCashMintCreditV1 {
        mint_exchange().1
    }

    fn mint_authorization() -> OfflineCashMintAuthorizationV1 {
        mint_exchange().0
    }

    fn redemption_voucher() -> OfflineCashRedemptionVoucherV1 {
        let network_id = network();
        let asset = asset();
        let asset_incarnation = asset_incarnation(&network_id, &asset, 1);
        let credential = hardware_credential();
        let commit_evidence =
            OfflineCashCommitEvidenceV1::TrustedTime(OfflineCashTrustedCommitTimeV1 {
                time_evidence_commitment: [0x40; 32],
            });
        let statement = OfflineCashRedemptionStatementV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            lifecycle: OfflineCashLifecycleBindingV1 {
                version: OFFLINE_CASH_WIRE_VERSION_V1,
                network_id,
                protocol_version: OFFLINE_CASH_WIRE_VERSION_V1,
                suite_id: credential.suite_id,
                vk_digest: FIXTURE_VK_SET_DIGEST_V1,
                release_id: FIXTURE_RELEASE_ID_V1,
                asset: asset.clone(),
                asset_incarnation,
                scale: 4,
                liability_pool_id: offline_cash_liability_pool_id_v1(
                    &network_id,
                    &asset,
                    asset_incarnation,
                )
                .expect("liability pool"),
                hardware_profile_id: credential.hardware_profile_id,
                policy_epoch: credential.policy_epoch,
                operation_kind: OfflineCashOperationKindV1::RedeemSplit,
                request_id: [0; 32],
                acceptance_ticket_id: [0; 32],
                credit_id: [0; 32],
                ciphertext_digest: [0; 32],
            },
            amount: 51_000,
            beneficiary: account(0xB6),
            terminal_nullifier: [0x42; 32],
            redemption_commitment: [7; 32],
            redemption_id: [0; 32],
            commit_evidence,
        }
        .seal_redemption_id()
        .expect("seal redemption");
        let semantic_digest = statement
            .canonical_digest()
            .expect("redemption semantic digest");
        let commit_certificate = seal_commit_certificate(OfflineCashCommitCertificateV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            certificate_id: [0; 32],
            candidate_envelope_digest: [0x43; 32],
            lifecycle_binding_digest: statement
                .lifecycle
                .canonical_digest()
                .expect("lifecycle digest"),
            transition_nullifier: statement.terminal_nullifier,
            outbox_reservation_commitment: OfflineCashOutboxReservationV1 {
                reservation_id: [0x44; 32],
                operation_kind: OfflineCashOperationKindV1::RedeemSplit,
                reserved_outbox_bytes: OFFLINE_CASH_REDEMPTION_OUTBOX_MIN_BYTES_V1,
                issued_at_ms: 8_000,
                expires_at_ms: 10_000,
            }
            .canonical_commitment()
            .expect("outbox commitment"),
            commit_evidence,
            hardware_profile_id: statement.lifecycle.hardware_profile_id,
            policy_epoch: statement.lifecycle.policy_epoch,
            hardware_terminal_commitment: [0; 32],
        });
        let certificate_digest = commit_certificate
            .canonical_digest_against(
                &statement.lifecycle,
                statement.commit_evidence,
                statement.terminal_nullifier,
            )
            .expect("certificate digest");
        OfflineCashRedemptionVoucherV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            statement,
            commit_certificate: commit_certificate.clone(),
            proof: OfflineCashCommitWrapperProofV1 {
                version: OFFLINE_CASH_WIRE_VERSION_V1,
                eq_protocol_digest: [0x15; 32],
                ep_protocol_digest: [0x16; 32],
                semantic_digest,
                candidate_envelope_digest: commit_certificate.candidate_envelope_digest,
                commit_certificate_digest: certificate_digest,
                eq_deferred_audit: [0x13; 32],
                ep_deferred_audit: [0x14; 32],
                eq_proof: vec![0xA1; 128],
                ep_proof: vec![0xB2; 128],
                eq_history: vec![0xC3; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
                ep_history: vec![0xD4; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
            },
            artifact_manifest_digest: FIXTURE_ARTIFACT_MANIFEST_DIGEST_V1,
        }
    }

    #[test]
    fn no_commit_closure_roundtrips_and_rejects_context_substitution() {
        let request = request();
        let authorization = acceptance_intent_authorization(&request);
        let ticket = acceptance_ticket(&request, &authorization.intent());
        let closure = no_commit_closure(&request, &authorization, &ticket);
        let bytes = norito::encode_canonical(&closure).expect("encode no-commit closure");
        assert_eq!(
            OfflineCashNoCommitClosureV1::decode_canonical_shape_exact(&bytes)
                .expect("decode no-commit closure"),
            closure,
        );
        let text = closure
            .encode_text_shape()
            .expect("encode no-commit closure text");
        assert_eq!(
            OfflineCashNoCommitClosureV1::decode_text_shape_exact(&text)
                .expect("decode no-commit closure text"),
            closure,
        );

        let mut substituted = closure.clone();
        substituted.statement.intent_authorization_digest[0] ^= 1;
        substituted.proof.semantic_digest = substituted
            .statement
            .canonical_digest()
            .expect("substituted statement digest");
        assert!(substituted.validate_shape().is_err());
        assert!(
            OfflineCashNoCommitClosureV1::decode_canonical_shape_exact(&vec![
                0;
                OFFLINE_CASH_NO_COMMIT_CLOSURE_MAX_BYTES_V1
                    + 1
            ])
            .is_err()
        );
    }

    #[test]
    fn canonical_session_roundtrips_and_fits_transport_caps() {
        assert_eq!(OFFLINE_CASH_TEXT_PREFIX_V1, "oc1:");
        let request = request();
        let authorization = acceptance_intent_authorization(&request);
        let ticket = acceptance_ticket(&request, &authorization.intent());
        let payment = payment(&request);
        let acknowledgement = acknowledgement(&request, &payment);
        let raw = validate_offline_cash_session_shape_v1(&request, &payment, &acknowledgement)
            .expect("valid session");
        assert!(raw < OFFLINE_CASH_SESSION_TARGET_BYTES_V1);
        let pre_ticket_raw =
            validate_offline_cash_pre_ticket_exchange_shape_v1(&request, &authorization, &ticket)
                .expect("valid pre-ticket exchange");
        assert!(pre_ticket_raw < OFFLINE_CASH_PRE_TICKET_EXCHANGE_TARGET_BYTES_V1);
        let complete_raw = validate_offline_cash_complete_exchange_shape_v1(
            &request,
            &authorization,
            &ticket,
            &payment,
            &acknowledgement,
        )
        .expect("valid complete exchange");
        assert!(complete_raw < OFFLINE_CASH_COMPLETE_EXCHANGE_TARGET_BYTES_V1);

        let request_bytes = norito::encode_canonical(&request).expect("encode request");
        let decoded_request = OfflineCashPaymentRequestV1::decode_canonical_exact(&request_bytes)
            .expect("decode request");
        assert_eq!(decoded_request, request);

        let intent = payment.acceptance_intent;
        let intent_text = intent
            .encode_text_against(&decoded_request)
            .expect("encode intent text");
        assert_eq!(
            OfflineCashAcceptanceIntentV1::decode_text_exact_against(
                &intent_text,
                &decoded_request,
            )
            .expect("decode intent text"),
            intent
        );

        let ticket_bytes = norito::encode_canonical(&ticket).expect("encode acceptance ticket");
        assert_eq!(
            OfflineCashAcceptanceTicketV1::decode_canonical_shape_exact_against(
                &ticket_bytes,
                &decoded_request,
                &intent,
            )
            .expect("decode acceptance ticket"),
            ticket
        );
        let ticket_text = ticket
            .encode_text_shape_against(&decoded_request, &intent)
            .expect("encode acceptance ticket text");
        assert_eq!(
            OfflineCashAcceptanceTicketV1::decode_text_shape_exact_against(
                &ticket_text,
                &decoded_request,
                &intent,
            )
            .expect("decode acceptance ticket text"),
            ticket
        );

        let payment_bytes = norito::encode_canonical(&payment).expect("encode payment");
        let decoded_payment = OfflineCashPaymentV1::decode_canonical_shape_exact_against(
            &payment_bytes,
            &decoded_request,
        )
        .expect("decode payment");
        assert_eq!(decoded_payment, payment);

        let acknowledgement_bytes =
            norito::encode_canonical(&acknowledgement).expect("encode acknowledgement");
        let decoded_acknowledgement =
            OfflineCashAcknowledgementV1::decode_canonical_shape_exact_against(
                &acknowledgement_bytes,
                &decoded_request,
                &decoded_payment,
            )
            .expect("decode acknowledgement");
        assert_eq!(decoded_acknowledgement, acknowledgement);
    }

    #[test]
    fn sender_authorization_is_verified_before_compact_intent_reservation() {
        let request = request();
        let authorization = acceptance_intent_authorization(&request);
        authorization
            .validate_shape_against(&request)
            .expect("authorization shape");
        assert_ne!(
            authorization
                .canonical_digest_against(&request)
                .expect("authorization digest"),
            authorization
                .intent()
                .canonical_digest_against(&request)
                .expect("compact intent digest")
        );

        let text = authorization
            .encode_text_shape_against(&request)
            .expect("authorization text");
        assert_eq!(
            OfflineCashAcceptanceIntentAuthorizationV1::decode_text_shape_exact_against(
                &text, &request,
            )
            .expect("decode authorization text"),
            authorization
        );

        let mut wrong_semantics = authorization.clone();
        wrong_semantics.proof.semantic_digest = [0xEF; 32];
        assert!(wrong_semantics.validate_shape_against(&request).is_err());

        let mut missing_manifest = authorization;
        missing_manifest.statement.artifact_manifest_digest = [0; 32];
        assert!(missing_manifest.validate_shape_against(&request).is_err());
    }

    #[test]
    fn circuit_bound_semantic_digests_use_fixed_transcripts_not_transport_frames() {
        let request = request();
        let intent = acceptance_intent(&request);
        let authorization = acceptance_intent_authorization(&request);
        let ticket = acceptance_ticket(&request, &intent);
        let closure = no_commit_closure(&request, &authorization, &ticket);
        let reservation = OfflineCashOutboxReservationV1 {
            reservation_id: [0x3B; 32],
            operation_kind: OfflineCashOperationKindV1::SendSplit,
            reserved_outbox_bytes: OFFLINE_CASH_PAYMENT_OUTBOX_MIN_BYTES_V1,
            issued_at_ms: request.issued_at_ms,
            expires_at_ms: request.expires_at_ms,
        };
        let payment = payment(&request);

        let intent_transcript = acceptance_intent_circuit_transcript_v1(&intent);
        assert_eq!(intent_transcript.len(), 114);
        assert_ne!(
            intent_transcript,
            norito::encode_canonical(&intent).expect("canonical intent transport")
        );
        assert_eq!(
            intent
                .canonical_digest_against(&request)
                .expect("intent digest"),
            digest_bytes(ACCEPTANCE_INTENT_DIGEST_DOMAIN, &intent_transcript)
        );

        let authorization_transcript =
            acceptance_intent_authorization_statement_circuit_transcript_v1(
                &authorization.statement,
            );
        assert_eq!(authorization_transcript.len(), 244);
        assert_eq!(
            authorization
                .statement
                .canonical_digest_against(&request)
                .expect("authorization statement digest"),
            digest_bytes(
                ACCEPTANCE_INTENT_AUTHORIZATION_STATEMENT_DIGEST_DOMAIN,
                &authorization_transcript,
            )
        );

        let closure_transcript =
            no_commit_closure_statement_circuit_transcript_v1(&closure.statement);
        assert_eq!(closure_transcript.len(), 498);
        assert_eq!(
            closure
                .statement
                .canonical_digest()
                .expect("closure digest"),
            digest_bytes(
                NO_COMMIT_CLOSURE_STATEMENT_DIGEST_DOMAIN,
                &closure_transcript,
            )
        );

        let reservation_transcript = outbox_reservation_circuit_transcript_v1(reservation);
        assert_eq!(reservation_transcript.len(), 56);
        assert_eq!(
            reservation
                .canonical_commitment()
                .expect("reservation digest"),
            digest_bytes(
                OUTBOX_RESERVATION_COMMITMENT_DOMAIN,
                &reservation_transcript,
            )
        );

        let certificate = &payment.commit_certificate;
        let certificate_id_transcript = commit_certificate_id_circuit_transcript_v1(certificate);
        assert_eq!(certificate_id_transcript.len(), 238);
        assert_eq!(
            certificate
                .expected_certificate_id()
                .expect("certificate ID"),
            digest_bytes(COMMIT_CERTIFICATE_ID_DOMAIN, &certificate_id_transcript)
        );
        let certificate_transcript = commit_certificate_circuit_transcript_v1(certificate);
        assert_eq!(certificate_transcript.len(), 270);
        assert_eq!(
            certificate
                .canonical_digest_against(
                    &payment.statement.lifecycle,
                    payment.statement.commit_evidence,
                    payment.statement.transition_nullifier,
                )
                .expect("certificate digest"),
            digest_bytes(COMMIT_CERTIFICATE_DIGEST_DOMAIN, &certificate_transcript)
        );
    }

    #[test]
    fn request_credential_and_mode_are_signed_and_payment_has_no_state_links() {
        let mut lane_request = request();
        lane_request.hardware_credential.lane_commitment = [0x55; 32];
        assert!(lane_request.validate_shape().is_err());

        let mut policy_request = request();
        policy_request.request_mode =
            OfflineCashPaymentRequestModeV1::OpenReceive(OfflineCashOpenReceiveRequestV1 {
                per_payment: OfflineCashAmountPolicyV1 {
                    minimum_amount: 1,
                    maximum_amount: 20_000,
                },
            });
        assert!(policy_request.validate_shape().is_err());

        let json = norito::json::to_string(&payment(&request())).expect("payment JSON");
        for private_name in [
            "sender_lane_id",
            "sender_hardware_epoch_id",
            "sender_before",
            "sender_after",
            "predecessor_state",
            "successor_state",
            "hardware_credential_id",
            "credential_audit",
        ] {
            assert!(!json.contains(private_name), "leaked {private_name}");
        }

        let redemption_json =
            norito::json::to_string(&redemption_voucher()).expect("redemption JSON");
        for private_name in [
            "sender_lane_id",
            "sender_hardware_epoch_id",
            "sender_key_reference",
            "sender_before",
            "sender_after",
            "predecessor_state",
            "successor_state",
            "hardware_credential_id",
            "credential_audit",
        ] {
            assert!(
                !redemption_json.contains(private_name),
                "redemption leaked {private_name}"
            );
        }
    }

    #[test]
    fn wrapper_binds_precommit_candidate_and_terminal_certificate() {
        let request = request();
        let mut wrong_candidate = payment(&request);
        wrong_candidate.proof.candidate_envelope_digest = [0xA9; 32];
        assert!(wrong_candidate.validate_shape_against(&request).is_err());

        let mut wrong_terminal = payment(&request);
        wrong_terminal
            .commit_certificate
            .hardware_terminal_commitment = [0xAA; 32];
        assert!(wrong_terminal.validate_shape_against(&request).is_err());
    }

    #[test]
    fn terminal_body_is_committed_before_and_without_certificate_identity() {
        let request = request();
        let certificate = payment(&request).commit_certificate;
        let mut body = OfflineCashHardwareTerminalBodyV1 {
            version: certificate.version,
            candidate_envelope_digest: certificate.candidate_envelope_digest,
            lifecycle_binding_digest: certificate.lifecycle_binding_digest,
            transition_nullifier: certificate.transition_nullifier,
            outbox_reservation_commitment: certificate.outbox_reservation_commitment,
            commit_evidence: certificate.commit_evidence,
            hardware_profile_id: certificate.hardware_profile_id,
            policy_epoch: certificate.policy_epoch,
            private_successor_commitment: [0xE1; 32],
            private_journal_commitment: [0xE2; 32],
            private_recovery_commitment: [0xE3; 32],
        };
        assert_eq!(
            body.canonical_commitment().expect("terminal commitment"),
            certificate.hardware_terminal_commitment
        );
        let body_json = norito::json::to_string(&body).expect("terminal body JSON");
        assert!(!body_json.contains("certificate_id"));

        let unsealed = OfflineCashCommitCertificateV1 {
            certificate_id: [0; 32],
            hardware_terminal_commitment: [0; 32],
            ..certificate.clone()
        };
        body.private_recovery_commitment = [0xE4; 32];
        let changed = unsealed
            .seal_with_terminal_body(&body)
            .expect("changed terminal body");
        assert_ne!(
            changed.hardware_terminal_commitment,
            certificate.hardware_terminal_commitment
        );
        assert_ne!(changed.certificate_id, certificate.certificate_id);
    }

    #[test]
    fn ticket_cannot_be_reused_with_another_sender_intent() {
        let request = request();
        let mut payment = payment(&request);
        payment.acceptance_intent.intent_id = [0xD1; 32];
        payment.acceptance_intent.sender_one_time_commitment = [0xD2; 32];
        assert!(payment.validate_shape_against(&request).is_err());
    }

    #[test]
    fn commit_deadline_and_inbox_sequence_witnesses_are_not_public() {
        let request = request();
        let mut payment = payment(&request);
        let OfflineCashCommitEvidenceV1::TrustedTime(evidence) =
            &mut payment.statement.commit_evidence
        else {
            unreachable!("test fixture uses trusted time")
        };
        evidence.time_evidence_commitment = [0xA7; 32];
        reseal_payment(&mut payment);
        payment
            .validate_shape_against(&request)
            .expect("shape accepts an opaque nonzero deadline witness");

        let acknowledgement = acknowledgement(&request, &payment);
        acknowledgement
            .validate_shape_against(&request, &payment)
            .expect("durable acknowledgement remains valid without public time");

        let payment_json = norito::json::to_string(&payment).expect("payment JSON");
        let acknowledgement_json =
            norito::json::to_string(&acknowledgement).expect("acknowledgement JSON");
        for private_name in [
            "committed_at_ms",
            "lease_id",
            "authorization_counter",
            "clock_epoch",
        ] {
            assert!(
                !payment_json.contains(&format!("\"{private_name}\"")),
                "payment leaked {private_name}"
            );
        }
        for private_name in [
            "staging_hardware_epoch_id",
            "inbox_sequence",
            "acknowledged_at_ms",
        ] {
            assert!(
                !acknowledgement_json.contains(&format!("\"{private_name}\"")),
                "acknowledgement leaked {private_name}"
            );
        }
    }

    #[test]
    fn output_identity_and_conflict_key_have_distinct_jobs() {
        let request = request();
        let first = payment(&request);
        let mut competing = payment(&request);
        competing.statement.ciphertext_commitment = [0x44; 32];
        reseal_payment(&mut competing);

        assert_ne!(
            first.statement.lifecycle.credit_id,
            competing.statement.lifecycle.credit_id
        );
        assert_eq!(
            first.sender_conflict_key().expect("first conflict key"),
            competing
                .sender_conflict_key()
                .expect("competing conflict key")
        );
    }

    #[test]
    fn peer_credit_identity_is_fixed_before_id_bound_aead() {
        let request = request();
        let payment = payment(&request);
        let credit_id = payment.statement.lifecycle.credit_id;
        let semantic_digest = payment
            .statement
            .canonical_digest()
            .expect("payment statement digest");

        let mut later_ciphertext = payment.statement.clone();
        later_ciphertext.lifecycle.ciphertext_digest = [0x76; 32];
        assert_eq!(
            later_ciphertext
                .expected_credit_id()
                .expect("pre-encryption credit ID"),
            credit_id,
            "AEAD bytes cannot participate in the ID they are allowed to bind"
        );
        later_ciphertext
            .validate()
            .expect("statement remains structurally bound");
        assert_ne!(
            later_ciphertext
                .canonical_digest()
                .expect("later ciphertext statement digest"),
            semantic_digest,
            "the final wrapper still binds the exact ciphertext digest"
        );
    }

    #[test]
    fn acknowledgement_binds_exact_durable_inbox_receipt() {
        let request = request();
        let payment = payment(&request);
        let mut acknowledgement = acknowledgement(&request, &payment);
        acknowledgement.inbox_receipt.receipt_commitment[0] ^= 1;
        assert!(
            acknowledgement
                .validate_shape_against(&request, &payment)
                .is_err()
        );
    }

    #[test]
    fn parity_substitution_and_oversized_proofs_are_rejected() {
        let request = request();
        let mut substituted = payment(&request);
        substituted.proof.ep_protocol_digest = substituted.proof.eq_protocol_digest;
        assert!(substituted.validate_shape_against(&request).is_err());

        let mut substituted_audit = payment(&request);
        substituted_audit.proof.ep_deferred_audit = substituted_audit.proof.eq_deferred_audit;
        assert!(substituted_audit.validate_shape_against(&request).is_err());

        let mut oversized = payment(&request);
        oversized.proof.eq_proof = vec![0xAA; OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1 + 1];
        assert!(oversized.validate_shape_against(&request).is_err());
    }

    #[test]
    fn aggregate_mint_and_redemption_wires_are_bounded_and_exact() {
        let aggregate = aggregate_state();
        let aggregate_bytes = norito::encode_canonical(&aggregate).expect("encode aggregate");
        assert_eq!(
            OfflineCashAggregateStateCommitmentV1::decode_canonical_exact(&aggregate_bytes)
                .expect("decode aggregate"),
            aggregate
        );

        let (authorization, mint) = mint_exchange();
        let mint_bytes = norito::encode_canonical(&mint).expect("encode mint");
        assert_eq!(
            OfflineCashMintCreditV1::decode_canonical_shape_exact(&mint_bytes)
                .expect("decode mint"),
            mint
        );
        mint.validate_shape_against_authorization(&authorization)
            .expect("mint credit is bound to pre-debit authorization");

        let redemption = redemption_voucher();
        let redemption_bytes = norito::encode_canonical(&redemption).expect("encode redemption");
        assert_eq!(
            OfflineCashRedemptionVoucherV1::decode_canonical_shape_exact(&redemption_bytes)
                .expect("decode redemption"),
            redemption
        );

        let authorization_text = authorization
            .encode_text_shape()
            .expect("mint authorization text");
        assert_eq!(
            OfflineCashMintAuthorizationV1::decode_text_shape_exact(&authorization_text)
                .expect("decode mint authorization"),
            authorization
        );
        let mut substituted_ciphertext = authorization;
        substituted_ciphertext.statement.ciphertext_digest = [0x75; 32];
        assert!(substituted_ciphertext.validate_shape().is_err());
    }

    #[test]
    fn mint_credit_identity_precedes_commit_time_while_statement_digest_binds_it() {
        let statement = mint_credit().statement;
        let credit_id = statement.lifecycle.credit_id;
        let statement_digest = statement.canonical_digest().expect("statement digest");

        let mut later_commit = statement;
        later_commit.minted_at_ms += 1;
        assert_eq!(
            later_commit
                .expected_credit_id()
                .expect("pre-commit credit identity"),
            credit_id
        );
        later_commit
            .validate_shape()
            .expect("later committed statement");
        assert_ne!(
            later_commit
                .canonical_digest()
                .expect("later statement digest"),
            statement_digest
        );

        let mut substituted_credential = mint_credit();
        substituted_credential
            .statement
            .recipient_credential_commitment = [0xE1; 32];
        assert!(substituted_credential.validate_shape().is_err());

        let mut substituted_ciphertext = mint_credit();
        substituted_ciphertext.encrypted_credit[0] ^= 1;
        assert!(substituted_ciphertext.validate_shape().is_err());
    }

    #[test]
    fn redemption_nullifier_is_independent_from_the_public_claim_identity() {
        let first = redemption_voucher();
        let mut competing_statement = first.statement.clone();
        competing_statement.amount += 1;
        competing_statement.redemption_commitment = [0x62; 32];
        competing_statement.redemption_id = [0; 32];
        competing_statement = competing_statement
            .seal_redemption_id()
            .expect("seal competing voucher");
        assert_eq!(
            first.statement.terminal_nullifier,
            competing_statement.terminal_nullifier
        );
        assert_ne!(
            first.statement.redemption_id,
            competing_statement.redemption_id
        );

        let mut later_statement = competing_statement.clone();
        later_statement.terminal_nullifier = [0x63; 32];
        later_statement.redemption_id = [0; 32];
        later_statement = later_statement
            .seal_redemption_id()
            .expect("seal later voucher");
        assert_ne!(
            first.statement.terminal_nullifier,
            later_statement.terminal_nullifier
        );
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
                OfflineCashAcceptanceIntentV1::decode_canonical_shape_exact_against(
                    &vec![0; OFFLINE_CASH_ACCEPTANCE_INTENT_MAX_BYTES_V1 + 1],
                    &request,
                )
                .map(|_| ()),
                OFFLINE_CASH_ACCEPTANCE_INTENT_MAX_BYTES_V1 + 1,
                OFFLINE_CASH_ACCEPTANCE_INTENT_MAX_BYTES_V1,
            ),
            (
                OfflineCashAcceptanceIntentAuthorizationV1::decode_canonical_shape_exact_against(
                    &vec![0; OFFLINE_CASH_ACCEPTANCE_INTENT_AUTHORIZATION_MAX_BYTES_V1 + 1],
                    &request,
                )
                .map(|_| ()),
                OFFLINE_CASH_ACCEPTANCE_INTENT_AUTHORIZATION_MAX_BYTES_V1 + 1,
                OFFLINE_CASH_ACCEPTANCE_INTENT_AUTHORIZATION_MAX_BYTES_V1,
            ),
            (
                OfflineCashPaymentV1::decode_canonical_shape_exact_against(
                    &vec![0; OFFLINE_CASH_PAYMENT_MAX_BYTES_V1 + 1],
                    &request,
                )
                .map(|_| ()),
                OFFLINE_CASH_PAYMENT_MAX_BYTES_V1 + 1,
                OFFLINE_CASH_PAYMENT_MAX_BYTES_V1,
            ),
            (
                OfflineCashAcknowledgementV1::decode_canonical_shape_exact_against(
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
                Err(OfflineCashValidationErrorV1::EncodedSizeExceeded { actual, max })
                    if actual == expected_actual && max == expected_max
            ));
        }
    }

    #[test]
    fn exact_decoders_reject_noncanonical_or_forged_lengths() {
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
            OfflineCashPaymentV1::decode_canonical_shape_exact_against(&payment_bytes, &request)
                .is_err()
        );
        assert!(
            OfflineCashAcknowledgementV1::decode_canonical_shape_exact_against(
                &acknowledgement_bytes,
                &request,
                &payment,
            )
            .is_err()
        );
    }

    #[test]
    fn text_transport_roundtrips_every_wire_exactly() {
        let request = request();
        let acceptance_authorization = acceptance_intent_authorization(&request);
        let acceptance_ticket = acceptance_ticket(&request, &acceptance_authorization.intent());
        let no_commit_closure =
            no_commit_closure(&request, &acceptance_authorization, &acceptance_ticket);
        let payment = payment(&request);
        let acknowledgement = acknowledgement(&request, &payment);
        let (mint_authorization, mint) = mint_exchange();
        let redemption = redemption_voucher();

        let request_text = request.encode_text().expect("encode request text");
        assert!(request_text.starts_with(OFFLINE_CASH_TEXT_PREFIX_V1));
        assert!(!request_text.contains('='));
        let decoded_request =
            OfflineCashPaymentRequestV1::decode_text_exact(&request_text).expect("request text");
        assert_eq!(decoded_request, request);
        assert_eq!(
            decoded_request.encode_text().expect("re-encode"),
            request_text
        );

        let acceptance_authorization_text = acceptance_authorization
            .encode_text_shape_against(&decoded_request)
            .expect("encode acceptance authorization text");
        let decoded_acceptance_authorization =
            OfflineCashAcceptanceIntentAuthorizationV1::decode_text_shape_exact_against(
                &acceptance_authorization_text,
                &decoded_request,
            )
            .expect("acceptance authorization text");
        assert_eq!(decoded_acceptance_authorization, acceptance_authorization);

        let acceptance_ticket_text = acceptance_ticket
            .encode_text_shape_against(&decoded_request, &decoded_acceptance_authorization.intent())
            .expect("encode acceptance ticket text");
        let decoded_acceptance_ticket =
            OfflineCashAcceptanceTicketV1::decode_text_shape_exact_against(
                &acceptance_ticket_text,
                &decoded_request,
                &decoded_acceptance_authorization.intent(),
            )
            .expect("acceptance ticket text");
        assert_eq!(decoded_acceptance_ticket, acceptance_ticket);

        let no_commit_closure_text = no_commit_closure
            .encode_text_shape()
            .expect("encode no-commit closure text");
        let decoded_no_commit_closure =
            OfflineCashNoCommitClosureV1::decode_text_shape_exact(&no_commit_closure_text)
                .expect("no-commit closure text");
        assert_eq!(decoded_no_commit_closure, no_commit_closure);
        assert_eq!(
            decoded_no_commit_closure
                .encode_text_shape()
                .expect("re-encode"),
            no_commit_closure_text,
        );

        let payment_text = payment
            .encode_text_against(&request)
            .expect("encode payment text");
        let decoded_payment =
            OfflineCashPaymentV1::decode_text_exact_against(&payment_text, &decoded_request)
                .expect("payment text");
        assert_eq!(decoded_payment, payment);
        assert_eq!(
            decoded_payment
                .encode_text_against(&decoded_request)
                .expect("re-encode"),
            payment_text
        );

        let acknowledgement_text = acknowledgement
            .encode_text_against(&request, &payment)
            .expect("encode acknowledgement text");
        let decoded_acknowledgement = OfflineCashAcknowledgementV1::decode_text_exact_against(
            &acknowledgement_text,
            &decoded_request,
            &decoded_payment,
        )
        .expect("acknowledgement text");
        assert_eq!(decoded_acknowledgement, acknowledgement);
        assert_eq!(
            decoded_acknowledgement
                .encode_text_against(&decoded_request, &decoded_payment)
                .expect("re-encode"),
            acknowledgement_text
        );

        let mint_authorization_text = mint_authorization
            .encode_text_shape()
            .expect("encode mint authorization text");
        let decoded_mint_authorization =
            OfflineCashMintAuthorizationV1::decode_text_shape_exact(&mint_authorization_text)
                .expect("mint authorization text");
        assert_eq!(decoded_mint_authorization, mint_authorization);

        let mint_text = mint.encode_text_shape().expect("encode mint text");
        let decoded_mint =
            OfflineCashMintCreditV1::decode_text_shape_exact(&mint_text).expect("mint text");
        assert_eq!(decoded_mint, mint);
        decoded_mint
            .validate_shape_against_authorization(&decoded_mint_authorization)
            .expect("decoded mint binding");
        assert_eq!(
            decoded_mint.encode_text_shape().expect("re-encode"),
            mint_text
        );

        let redemption_text = redemption
            .encode_text_shape()
            .expect("encode redemption text");
        let decoded_redemption =
            OfflineCashRedemptionVoucherV1::decode_text_shape_exact(&redemption_text)
                .expect("redemption text");
        assert_eq!(decoded_redemption, redemption);
        assert_eq!(
            decoded_redemption.encode_text_shape().expect("re-encode"),
            redemption_text
        );
    }

    #[test]
    fn text_transport_rejects_prefix_padding_whitespace_and_noncanonical_base64url() {
        let text = request().encode_text().expect("request text");
        let body = text
            .strip_prefix(OFFLINE_CASH_TEXT_PREFIX_V1)
            .expect("canonical prefix");
        for invalid_text in [
            body.to_owned(),
            format!("kgm2:{body}"),
            format!("oc2:{body}"),
            format!("{text}="),
            format!("{text}\n"),
            format!("oc1:{} {}", &body[..4], &body[4..]),
            format!("oc1:${}", &body[1..]),
        ] {
            assert!(
                OfflineCashPaymentRequestV1::decode_text_exact(&invalid_text).is_err(),
                "accepted invalid text: {invalid_text:?}"
            );
        }

        // `AB` has non-zero unused trailing bits for the one decoded zero byte;
        // even a permissive base64 decoder must not make it canonical text.
        assert!(decode_offline_cash_text_payload_v1("oc1:AB", 1, 8).is_err());
    }

    #[test]
    fn text_transport_enforces_text_and_decoded_raw_caps_before_norito() {
        let oversized_text = format!(
            "oc1:{}",
            "A".repeat(OFFLINE_CASH_PAYMENT_REQUEST_TEXT_MAX_BYTES_V1)
        );
        assert!(matches!(
            OfflineCashPaymentRequestV1::decode_text_exact(&oversized_text),
            Err(OfflineCashValidationErrorV1::EncodedSizeExceeded { actual, max })
                if actual == oversized_text.len()
                    && max == OFFLINE_CASH_PAYMENT_REQUEST_TEXT_MAX_BYTES_V1
        ));

        let exact_raw = vec![0_u8; OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1];
        let exact_text = format!(
            "{OFFLINE_CASH_TEXT_PREFIX_V1}{}",
            URL_SAFE_NO_PAD.encode(&exact_raw)
        );
        assert_eq!(
            exact_text.len(),
            OFFLINE_CASH_PAYMENT_REQUEST_TEXT_MAX_BYTES_V1
        );
        assert_eq!(
            decode_offline_cash_text_payload_v1(
                &exact_text,
                OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1,
                OFFLINE_CASH_PAYMENT_REQUEST_TEXT_MAX_BYTES_V1,
            )
            .expect("exact raw boundary")
            .len(),
            OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1
        );

        let oversized_raw = vec![0_u8; 17];
        let oversized_raw_text = format!(
            "{OFFLINE_CASH_TEXT_PREFIX_V1}{}",
            URL_SAFE_NO_PAD.encode(&oversized_raw)
        );
        assert!(matches!(
            decode_offline_cash_text_payload_v1(&oversized_raw_text, 16, usize::MAX),
            Err(OfflineCashValidationErrorV1::EncodedSizeExceeded {
                actual: 17,
                max: 16
            })
        ));
    }

    #[test]
    fn contextual_text_decoders_reject_request_and_payment_substitution() {
        let request = request();
        let payment = payment(&request);
        let acknowledgement = acknowledgement(&request, &payment);
        let payment_text = payment.encode_text_against(&request).expect("payment text");
        let acknowledgement_text = acknowledgement
            .encode_text_against(&request, &payment)
            .expect("acknowledgement text");

        let mut substituted_request = request.clone();
        substituted_request.request_id = [0xEE; 32];
        substituted_request.signature = sign(
            &signing_key(7),
            &substituted_request
                .canonical_signing_bytes()
                .expect("substituted signing bytes"),
        );
        substituted_request
            .validate_shape()
            .expect("valid other request");
        assert!(
            OfflineCashPaymentV1::decode_text_exact_against(&payment_text, &substituted_request,)
                .is_err()
        );
        assert!(
            OfflineCashAcknowledgementV1::decode_text_exact_against(
                &acknowledgement_text,
                &substituted_request,
                &payment,
            )
            .is_err()
        );

        let mut substituted_payment = payment.clone();
        substituted_payment.statement.ciphertext_commitment = [0x72; 32];
        reseal_payment(&mut substituted_payment);
        substituted_payment
            .validate_shape_against(&request)
            .expect("valid other payment");
        assert!(
            OfflineCashAcknowledgementV1::decode_text_exact_against(
                &acknowledgement_text,
                &request,
                &substituted_payment,
            )
            .is_err()
        );
    }

    #[test]
    fn paired_proof_complete_encoding_respects_ceiling() {
        let semantic_digest = [0xA7; 32];
        let mut proof = paired_proof(
            semantic_digest,
            OfflineCashPastaStateCommitmentV1::ZERO,
            OfflineCashPastaStateCommitmentV1::ZERO,
        );
        proof.eq_proof = vec![0x11];
        proof.ep_proof = vec![0x22];
        let fixed_bytes = norito::encode_canonical(&proof)
            .expect("encode minimum proof")
            .len()
            - 2;
        let eq_len = OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1;
        let ep_len = OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1;
        assert_eq!(eq_len + ep_len, OFFLINE_CASH_CURRENT_PROOFS_MAX_BYTES_V1);
        assert!(
            fixed_bytes + OFFLINE_CASH_CURRENT_PROOFS_MAX_BYTES_V1
                <= OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1
        );
        proof.eq_proof = vec![0x11; eq_len];
        proof.ep_proof = vec![0x22; ep_len];
        assert!(
            norito::encode_canonical(&proof)
                .expect("encode boundary proof")
                .len()
                <= OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1
        );
        proof
            .validate_shape_for_semantic_digest(semantic_digest)
            .expect("exact boundary proof");

        proof.ep_proof.push(0x22);
        assert!(matches!(
            proof.validate_shape_for_semantic_digest(semantic_digest),
            Err(OfflineCashValidationErrorV1::InvalidField {
                field: "offline_cash.proof.current"
            })
        ));
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
            Err(OfflineCashValidationErrorV1::EncodedSizeExceeded { actual, max })
                if actual == OFFLINE_CASH_SESSION_MAX_BYTES_V1 + 1
                    && max == OFFLINE_CASH_SESSION_MAX_BYTES_V1
        ));
    }

    fn fixture_archive(fixture: &norito::json::Value, name: &str) -> Vec<u8> {
        let entry = fixture.get(name).expect("fixture entry");
        hex::decode(
            entry
                .get("norito_hex")
                .and_then(norito::json::Value::as_str)
                .expect("fixture Norito hex"),
        )
        .expect("fixture hex is canonical")
    }

    fn fixture_transport_entry(bytes: &[u8], text: &str) -> norito::json::Value {
        norito::json!({
            "norito_hex": (hex::encode(bytes)),
            "oc1": text,
            "raw_bytes": (u64::try_from(bytes.len()).expect("fixture length fits u64")),
        })
    }

    fn fixture_binary_entry(bytes: &[u8]) -> norito::json::Value {
        norito::json!({
            "norito_hex": (hex::encode(bytes)),
            "raw_bytes": (u64::try_from(bytes.len()).expect("fixture length fits u64")),
        })
    }

    fn fixture_exchange_summary(
        raw_bytes: usize,
        text_bytes: usize,
        raw_target_bytes: usize,
        raw_hard_cap_bytes: usize,
        text_hard_cap_bytes: usize,
    ) -> norito::json::Value {
        norito::json!({
            "raw_bytes": (u64::try_from(raw_bytes).expect("raw length fits u64")),
            "text_bytes": (u64::try_from(text_bytes).expect("text length fits u64")),
            "raw_target_bytes": (u64::try_from(raw_target_bytes).expect("target fits u64")),
            "raw_hard_cap_bytes": (u64::try_from(raw_hard_cap_bytes).expect("cap fits u64")),
            "text_hard_cap_bytes": (u64::try_from(text_hard_cap_bytes).expect("cap fits u64")),
            "within_raw_target": (raw_bytes <= raw_target_bytes),
            "within_raw_hard_cap": (raw_bytes <= raw_hard_cap_bytes),
            "within_text_hard_cap": (text_bytes <= text_hard_cap_bytes),
        })
    }

    fn canonical_fixture_v2() -> norito::json::Value {
        const TYPED_ENVELOPE_KAT_HEX: &str = concat!(
            "4e525430000073550b5069c0fdb105ebe7e810b71b3f001f01000000000000c7",
            "59e8c2f2209cf402020100208520f0098930a754748b7ddcb43ef75a0dbf3a0d",
            "26381af4eba4a98eaa9b4e6a18a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5",
            "a5a5a5a5a5a5a5e001d80000000000000020a5da85d238bff0f1abcda157",
            "21f478de45b547b51fcc35f4b72046b940421acc8805d46c005aee52d3333e3",
            "aa29a9bebf1016b51379248bdef107a99cd821318c57714dc32a650bd9de9884",
            "da13dfd6c14a1afc1b76d0f0f770e5b564d58ad9f30aca3a000b2640686632",
            "3a13f9400e4fa372552dc2245fc03d73621f8373e211ec3c18d4bcb571c784a",
            "6c02d838fdbbd21799db09a42a469a714ee0781022529ffcf9e407896080e960",
            "a2d9e246984fc7a09f814592008674939b2a0493ee9a88beb1d5ea71dc53c00",
            "55b1250cb633381d32010106c2e",
        );
        const KAT_RECIPIENT_PUBLIC: [u8; 32] = [
            0xde, 0x9e, 0xdb, 0x7d, 0x7b, 0x7d, 0xc1, 0xb4, 0xd3, 0x5b, 0x61, 0xc2, 0xec, 0xe4,
            0x35, 0x37, 0x3f, 0x83, 0x43, 0xc8, 0x5b, 0x78, 0x67, 0x4d, 0xad, 0xfc, 0x7e, 0x14,
            0x6f, 0x88, 0x2b, 0x4f,
        ];

        let request = request();
        let acceptance_authorization = acceptance_intent_authorization(&request);
        let acceptance_ticket = acceptance_ticket(&request, &acceptance_authorization.intent());
        let no_commit_closure =
            no_commit_closure(&request, &acceptance_authorization, &acceptance_ticket);
        let payment = payment(&request);
        let acknowledgement = acknowledgement(&request, &payment);
        let (mint_authorization, mint_credit) = mint_exchange();
        let redemption_voucher = redemption_voucher();

        assert_eq!(
            (
                acceptance_authorization.statement.release_id,
                acceptance_authorization.statement.suite_id,
                acceptance_authorization.statement.vk_digest,
                acceptance_authorization.statement.artifact_manifest_digest,
            ),
            (
                payment.statement.lifecycle.release_id,
                payment.statement.lifecycle.suite_id,
                payment.statement.lifecycle.vk_digest,
                payment.artifact_manifest_digest,
            ),
            "five-message fixture must authenticate under one release binding",
        );
        assert_eq!(
            (
                acceptance_authorization.statement.release_id,
                acceptance_authorization.statement.suite_id,
                acceptance_authorization.statement.vk_digest,
                acceptance_authorization.statement.artifact_manifest_digest,
            ),
            (
                mint_authorization.statement.context.release_id,
                mint_authorization.statement.context.suite_id,
                mint_authorization.statement.context.vk_digest,
                mint_authorization
                    .statement
                    .context
                    .artifact_manifest_digest,
            ),
            "mint fixture must use the shared authenticated release",
        );
        assert_eq!(
            (
                acceptance_authorization.statement.release_id,
                acceptance_authorization.statement.suite_id,
                acceptance_authorization.statement.vk_digest,
                acceptance_authorization.statement.artifact_manifest_digest,
            ),
            (
                redemption_voucher.statement.lifecycle.release_id,
                redemption_voucher.statement.lifecycle.suite_id,
                redemption_voucher.statement.lifecycle.vk_digest,
                redemption_voucher.artifact_manifest_digest,
            ),
            "redemption fixture must use the shared authenticated release",
        );

        let request_bytes = norito::encode_canonical(&request).expect("fixture request bytes");
        let acceptance_authorization_bytes = norito::encode_canonical(&acceptance_authorization)
            .expect("fixture acceptance authorization bytes");
        let acceptance_ticket_bytes =
            norito::encode_canonical(&acceptance_ticket).expect("fixture acceptance ticket bytes");
        let no_commit_closure_bytes =
            norito::encode_canonical(&no_commit_closure).expect("fixture no-commit closure bytes");
        let payment_bytes = norito::encode_canonical(&payment).expect("fixture payment bytes");
        let acknowledgement_bytes =
            norito::encode_canonical(&acknowledgement).expect("fixture acknowledgement bytes");
        let mint_authorization_bytes = norito::encode_canonical(&mint_authorization)
            .expect("fixture mint authorization bytes");
        let mint_credit_bytes =
            norito::encode_canonical(&mint_credit).expect("fixture mint credit bytes");
        let redemption_voucher_bytes = norito::encode_canonical(&redemption_voucher)
            .expect("fixture redemption voucher bytes");

        let request_text = request.encode_text().expect("fixture request text");
        let acceptance_authorization_text = acceptance_authorization
            .encode_text_shape_against(&request)
            .expect("fixture acceptance authorization text");
        let acceptance_ticket_text = acceptance_ticket
            .encode_text_shape_against(&request, &acceptance_authorization.intent())
            .expect("fixture acceptance ticket text");
        let no_commit_closure_text = no_commit_closure
            .encode_text_shape()
            .expect("fixture no-commit closure text");
        let payment_text = payment
            .encode_text_against(&request)
            .expect("fixture payment text");
        let acknowledgement_text = acknowledgement
            .encode_text_against(&request, &payment)
            .expect("fixture acknowledgement text");
        let mint_authorization_text = mint_authorization
            .encode_text_shape()
            .expect("fixture mint authorization text");
        let mint_credit_text = mint_credit
            .encode_text_shape()
            .expect("fixture mint credit text");
        let redemption_voucher_text = redemption_voucher
            .encode_text_shape()
            .expect("fixture redemption voucher text");

        let pre_ticket_raw = request_bytes.len()
            + acceptance_authorization_bytes.len()
            + acceptance_ticket_bytes.len();
        let pre_ticket_text =
            request_text.len() + acceptance_authorization_text.len() + acceptance_ticket_text.len();
        let terminal_raw = request_bytes.len() + payment_bytes.len() + acknowledgement_bytes.len();
        let terminal_text = request_text.len() + payment_text.len() + acknowledgement_text.len();
        let complete_raw = pre_ticket_raw + payment_bytes.len() + acknowledgement_bytes.len();
        let complete_text = pre_ticket_text + payment_text.len() + acknowledgement_text.len();
        assert_eq!(
            validate_offline_cash_complete_exchange_shape_v1(
                &request,
                &acceptance_authorization,
                &acceptance_ticket,
                &payment,
                &acknowledgement,
            )
            .expect("fixture complete exchange"),
            complete_raw
        );
        mint_credit
            .validate_shape_against_authorization(&mint_authorization)
            .expect("fixture mint exchange");

        let credit_opening = OfflineCashCreditOpeningV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            credit_id: [0x11; 32],
            amount: 37,
            credit_commitment_opening: [0x22; 32],
            recipient_binding_opening: [0x33; 32],
            recovery_nonce: [0x44; 32],
        };
        let encrypted_credit_aad = OfflineCashEncryptedCreditAadV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            purpose: OfflineCashEncryptedCreditPurposeV1::Peer,
            context_digest: [0x55; 32],
            issuance_or_transition_commitment: [0x66; 32],
            credit_id: credit_opening.credit_id,
            amount: credit_opening.amount,
        };
        let credit_opening_bytes = credit_opening
            .canonical_bytes()
            .expect("fixture credit opening bytes");
        let encrypted_credit_aad_bytes = encrypted_credit_aad
            .canonical_bytes()
            .expect("fixture encrypted-credit AAD bytes");
        let encrypted_credit_envelope_bytes =
            hex::decode(TYPED_ENVELOPE_KAT_HEX).expect("typed envelope KAT hex");
        let encrypted_credit_envelope =
            OfflineCashEncryptedCreditEnvelopeV1::decode_canonical_shape_exact_against_recipient_key(
                &encrypted_credit_envelope_bytes,
                KAT_RECIPIENT_PUBLIC,
            )
            .expect("typed envelope KAT shape");
        assert_eq!(
            encrypted_credit_envelope
                .canonical_bytes_against_recipient_key(KAT_RECIPIENT_PUBLIC)
                .expect("typed envelope KAT canonical bytes"),
            encrypted_credit_envelope_bytes
        );

        let mut encrypted_credit_envelope_entry =
            fixture_binary_entry(&encrypted_credit_envelope_bytes);
        encrypted_credit_envelope_entry
            .as_object_mut()
            .expect("fixture entry object")
            .insert(
                "recipient_x25519_public_key_hex".to_owned(),
                norito::json::Value::String(hex::encode(KAT_RECIPIENT_PUBLIC)),
            );

        norito::json!({
            "fixture_version": 2,
            "protocol": "OfflineCashV1",
            "text_prefix": OFFLINE_CASH_TEXT_PREFIX_V1,
            "canonical_source": "Rust iroha_data_model OfflineCashV1 Norito derivation plus the native encrypted-credit KAT; every SDK consumes identical bytes",
            "proof_fixture_scope": "canonical shape fixture only; structural recursive-proof bytes do not qualify a release",
            "payment_request": (fixture_transport_entry(&request_bytes, &request_text)),
            "acceptance_intent_authorization": (fixture_transport_entry(
                &acceptance_authorization_bytes,
                &acceptance_authorization_text,
            )),
            "acceptance_ticket": (fixture_transport_entry(
                &acceptance_ticket_bytes,
                &acceptance_ticket_text,
            )),
            "no_commit_closure": (fixture_transport_entry(
                &no_commit_closure_bytes,
                &no_commit_closure_text,
            )),
            "payment": (fixture_transport_entry(&payment_bytes, &payment_text)),
            "acknowledgement": (fixture_transport_entry(
                &acknowledgement_bytes,
                &acknowledgement_text,
            )),
            "mint_authorization": (fixture_transport_entry(
                &mint_authorization_bytes,
                &mint_authorization_text,
            )),
            "mint_credit": (fixture_transport_entry(&mint_credit_bytes, &mint_credit_text)),
            "redemption_voucher": (fixture_transport_entry(
                &redemption_voucher_bytes,
                &redemption_voucher_text,
            )),
            "encrypted_credit_envelope": encrypted_credit_envelope_entry,
            "encrypted_credit_aad": (fixture_binary_entry(&encrypted_credit_aad_bytes)),
            "credit_opening": (fixture_binary_entry(&credit_opening_bytes)),
            "pre_ticket_exchange": (fixture_exchange_summary(
                pre_ticket_raw,
                pre_ticket_text,
                OFFLINE_CASH_PRE_TICKET_EXCHANGE_TARGET_BYTES_V1,
                OFFLINE_CASH_PRE_TICKET_EXCHANGE_MAX_BYTES_V1,
                OFFLINE_CASH_PRE_TICKET_TEXT_EXCHANGE_MAX_BYTES_V1,
            )),
            "terminal_trio": (fixture_exchange_summary(
                terminal_raw,
                terminal_text,
                OFFLINE_CASH_SESSION_TARGET_BYTES_V1,
                OFFLINE_CASH_SESSION_MAX_BYTES_V1,
                OFFLINE_CASH_TEXT_SESSION_MAX_BYTES_V1,
            )),
            "complete_five_message": (fixture_exchange_summary(
                complete_raw,
                complete_text,
                OFFLINE_CASH_COMPLETE_EXCHANGE_TARGET_BYTES_V1,
                OFFLINE_CASH_COMPLETE_EXCHANGE_MAX_BYTES_V1,
                OFFLINE_CASH_COMPLETE_TEXT_EXCHANGE_MAX_BYTES_V1,
            )),
        })
    }

    #[test]
    fn canonical_fixture_v2_is_generated_by_and_decodes_with_the_native_model() {
        let expected = canonical_fixture_v2();
        let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/offline/offline_cash_v1.json");
        if std::env::var_os("UPDATE_OFFLINE_CASH_V1_FIXTURE").is_some() {
            let rendered = format!(
                "{}\n",
                norito::json::to_string_pretty(&expected).expect("render canonical fixture")
            );
            std::fs::write(&fixture_path, rendered).expect("write canonical fixture");
            return;
        }
        let fixture: norito::json::Value = norito::json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/offline/offline_cash_v1.json"
        )))
        .expect("shared Offline Cash V1 fixture JSON");
        assert_eq!(
            fixture, expected,
            "regenerate with UPDATE_OFFLINE_CASH_V1_FIXTURE=1"
        );

        let request_archive = fixture_archive(&fixture, "payment_request");
        let request = OfflineCashPaymentRequestV1::decode_canonical_exact(&request_archive)
            .expect("fixture request");
        let authorization =
            OfflineCashAcceptanceIntentAuthorizationV1::decode_canonical_shape_exact_against(
                &fixture_archive(&fixture, "acceptance_intent_authorization"),
                &request,
            )
            .expect("fixture acceptance authorization");
        let ticket = OfflineCashAcceptanceTicketV1::decode_canonical_shape_exact_against(
            &fixture_archive(&fixture, "acceptance_ticket"),
            &request,
            &authorization.intent(),
        )
        .expect("fixture acceptance ticket");
        OfflineCashNoCommitClosureV1::decode_canonical_shape_exact(&fixture_archive(
            &fixture,
            "no_commit_closure",
        ))
        .expect("fixture no-commit closure");
        let payment = OfflineCashPaymentV1::decode_canonical_shape_exact_against(
            &fixture_archive(&fixture, "payment"),
            &request,
        )
        .expect("fixture payment");
        let acknowledgement = OfflineCashAcknowledgementV1::decode_canonical_shape_exact_against(
            &fixture_archive(&fixture, "acknowledgement"),
            &request,
            &payment,
        )
        .expect("fixture acknowledgement");
        validate_offline_cash_complete_exchange_shape_v1(
            &request,
            &authorization,
            &ticket,
            &payment,
            &acknowledgement,
        )
        .expect("fixture complete exchange");

        let mint_authorization = OfflineCashMintAuthorizationV1::decode_canonical_shape_exact(
            &fixture_archive(&fixture, "mint_authorization"),
        )
        .expect("fixture mint authorization");
        let mint_credit = OfflineCashMintCreditV1::decode_canonical_shape_exact(&fixture_archive(
            &fixture,
            "mint_credit",
        ))
        .expect("fixture mint credit");
        mint_credit
            .validate_shape_against_authorization(&mint_authorization)
            .expect("fixture mint binding");
        let redemption_archive = fixture_archive(&fixture, "redemption_voucher");
        OfflineCashRedemptionVoucherV1::decode_canonical_shape_exact(&redemption_archive)
            .expect("fixture redemption voucher");
    }
}
