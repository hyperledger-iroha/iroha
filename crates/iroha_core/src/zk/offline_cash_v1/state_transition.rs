//! Private, linear state owners for the offline-cash V1 balance machine.
//!
//! This module deliberately has no wire codec for balance, pending, credit, or
//! guard-authorization owners. They are in-process capabilities, not receipts a
//! host may deserialize. Durable storage is expected to encrypt and authenticate
//! their private openings behind the platform hardware boundary.

use core::fmt;

use iroha_data_model::{
    NetworkId,
    account::AccountId,
    asset::AssetDefinitionId,
    offline::{
        KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_TTL_MS_V2, KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2,
        KagemushaDevicePublicKeyV2, KagemushaDeviceSignatureV2, OFFLINE_CASH_WIRE_VERSION_V1,
        OfflineCashAcknowledgementV1, OfflineCashPaymentRequestV1, OfflineCashPaymentV1,
        OfflineCashTransferStatementV1, offline_cash_acknowledgement_signing_bytes_v1,
        offline_cash_payment_request_signing_bytes_v1,
    },
};
use sha2::{Digest as _, Sha256};
use zeroize::Zeroizing;

use super::{
    OfflineCashPairedProofVerifierV1, OfflineCashTerminalVerifierV1, VerifiedOfflineCashCreditV1,
    state_relation::{
        offline_cash_balance_head_v1, offline_cash_credit_head_v1, offline_cash_receive_opening_v1,
        offline_cash_receive_transition_digest_v1, offline_cash_send_split_openings_v1,
        offline_cash_send_split_seed_v1, offline_cash_state_lineage_digest_v1,
    },
};

const CONTEXT_DOMAIN: &[u8] = b"iroha:offline-cash:v1:private-context";
const OPEN_PENDING_DOMAIN: &[u8] = b"iroha:offline-cash:v1:open-pending";
const CANCEL_PENDING_DOMAIN: &[u8] = b"iroha:offline-cash:v1:cancel-expired-pending";
const GUARD_CHALLENGE_DOMAIN: &[u8] = b"iroha:offline-cash:v1:guard-challenge";
const GUARD_CAPABILITY_DOMAIN: &[u8] = b"iroha:offline-cash:v1:guard-capability";
const INTENT_CHALLENGE_DOMAIN: &[u8] = b"iroha:offline-cash:v1:hardware-intent";
const INTENT_CAPABILITY_DOMAIN: &[u8] = b"iroha:offline-cash:v1:intent-capability";
const INTENT_CANCELLATION_DOMAIN: &[u8] = b"iroha:offline-cash:v1:intent-cancellation";
const ACKNOWLEDGEMENT_OWNER_DOMAIN: &[u8] = b"iroha:offline-cash:v1:verified-acknowledgement";
const RECEIVE_ACK_AUTHORIZATION_DOMAIN: &[u8] = b"iroha:offline-cash:v1:receive-ack-authorization";
const RECEIVE_REQUEST_SIGNING_BINDING_DOMAIN: &[u8] =
    b"iroha:offline-cash:v1:receive-request-signing-binding";
const RECEIVE_COMPLETION_DOMAIN: &[u8] = b"iroha:offline-cash:v1:receive-completion";

/// Exact SHA-256-sized private-state binding.
pub(crate) type Digest = [u8; 32];

fn digest_framed(domain: &[u8], parts: &[&[u8]]) -> Digest {
    let mut hasher = Sha256::new();
    hasher.update(
        u64::try_from(domain.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(domain);
    for part in parts {
        hasher.update(u64::try_from(part.len()).unwrap_or(u64::MAX).to_le_bytes());
        hasher.update(part);
    }
    hasher.finalize().into()
}

fn secret_digest(domain: &[u8], parts: &[&[u8]]) -> Zeroizing<Digest> {
    Zeroizing::new(digest_framed(domain, parts))
}

/// Move-only singleton balance owner.
mod balance;
/// Canonical private-state context.
mod context;
/// Terminal-bound receiver credit ownership.
mod credit;
/// Sealed exact-next hardware guard capabilities.
mod guard;
/// Authenticated durable staging for sender payment publication.
mod outbox;
/// Pending request state and expired cancellation.
mod pending;
/// Deterministic receiver fold relation.
mod receive;
/// Deterministic sender split relation.
mod send;

pub(crate) use balance::BalanceOwnerV1;
use balance::{BalanceSnapshotV1, balance_head};
pub(crate) use context::OfflineCashStateContextV1;
pub(crate) use credit::{CreditOwnerV1, OutgoingCreditOwnerV1};
use credit::{
    DecryptedCreditOpeningOwnerV1, bind_verified_credit_v1, credit_commitment,
    terminal_credit_matches,
};
pub(crate) use guard::{
    ExactNextHardwareGuardBackendV1, HardwareGuardSessionV1, HardwareReceiveTerminalQueryV1,
    HardwareTerminalOperationV1, HardwareTerminalOutcomeV1,
};
pub(crate) use guard::{
    GuardChallengeV1, GuardOperationV1, HardwareGuardErrorV1, HardwareIntentAuthorizationOwnerV1,
    HardwareIntentCancellationOwnerV1, HardwareIntentChallengeV1, HardwareIntentKindV1,
};
#[cfg(test)]
pub(crate) use guard::{
    HardwareActiveIntentOutcomeV1, HardwareIntentCommitRequestV1, HardwareIntentRequestV1,
    HardwareReceiveSigningResultV1, sealed,
};
use guard::{
    cancellation_authorization_matches, guard_authorization_matches, intent_authorization_matches,
};
use outbox::authorize_publication;
pub(crate) use outbox::{
    AuthenticatedPaymentOutboxBackendV1, AuthenticatedPaymentOutboxErrorV1,
    AuthenticatedPaymentOutboxRecordV1, PaymentOutboxKeyV1, PaymentOutboxPublicationV1,
};
pub(crate) use pending::PendingOwnerV1;
#[cfg(test)]
pub(crate) use pending::UnsignedReceiveRequestV1;
#[cfg(test)]
pub(crate) use pending::{
    apply_open_pending_v1, prepare_cancel_expired_pending_v1, prepare_open_pending_v1,
    recover_cancelled_pending_v1, recover_pending_v1,
};
use pending::{receive_intent_challenge, request_live};
pub(super) use receive::ReceiveFoldOutputV1;
#[cfg(test)]
pub(crate) use receive::{
    ReceiveFoldPlanV1, apply_receive_fold_v1, issue_receive_acknowledgement_v1,
    prepare_receive_fold_v1, recover_committed_receive_fold_v1,
    recover_receive_acknowledgement_owner_v1,
};
#[cfg(test)]
use send::verified_acknowledgement_for_test_v1;
#[cfg(test)]
pub(crate) use send::{
    SendSplitPlanV1, UnpublishedPaymentOwnerV1, VerifiedAcknowledgementOwnerV1,
    finalize_send_split_v1, prepare_send_split_v1, publish_send_split_v1,
    recover_committed_send_split_v1, recover_published_send_v1,
    recover_unpublished_send_payment_v1, stage_send_split_payment_v1,
};

/// Exact private-state transition failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StateTransitionErrorV1 {
    /// Context could not be represented canonically or violates V1 bounds.
    InvalidContext,
    /// Signed request failed its canonical wire validation.
    InvalidRequest,
    /// Request names another release, network, asset, or scale.
    ContextMismatch,
    /// A transfer or credit amount is not positive.
    ZeroAmount,
    /// Sender does not own enough atomic units.
    InsufficientFunds,
    /// Checked `u128` conservation/addition overflowed.
    ArithmeticOverflow,
    /// Request is not live at the supplied authoritative local time.
    RequestNotLive,
    /// Pending request has not reached its exclusive expiry.
    RequestNotExpired,
    /// Request names a different receiver current head.
    RequestHeadMismatch,
    /// Wallet already has one active receive request.
    PendingRequestActive,
    /// Wallet has no active request to consume.
    NoPendingRequest,
    /// Pending request, verified credit, or balance binding differs.
    RequestMismatch,
    /// Private credit opening or its send transition is inconsistent.
    CreditMismatch,
    /// Terminal proof decision is not the exact pending request/statement.
    TerminalVerificationMismatch,
    /// Authenticated ciphertext/opening/key-reference binding differs.
    EncryptedOpeningMismatch,
    /// A prepared transition no longer names the current owner state.
    StaleState,
    /// Prepared private values or their public statement were corrupted.
    CorruptPlan,
    /// Hardware session/capability is scoped to another state or operation.
    GuardBindingMismatch,
    /// Rollback-resistant hardware already owns another wallet intent.
    HardwareIntentActive,
    /// The durable hardware intent is absent or differs from the local owner.
    HardwareIntentMismatch,
    /// A published sender intent has not been bound to one exact payment.
    PaymentNotPublished,
    /// Sender finalization did not carry the exact verified receiver acknowledgement.
    AcknowledgementMismatch,
    /// Monotonic guard sequence has no exact `u64` successor.
    GuardSequenceExhausted,
    /// Sealed hardware backend rejected the exact-next request.
    HardwareGuard(HardwareGuardErrorV1),
    /// Authenticated durable payment outbox rejected the operation.
    PaymentOutbox(AuthenticatedPaymentOutboxErrorV1),
}

impl fmt::Display for StateTransitionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidContext => formatter.write_str("invalid offline-cash private context"),
            Self::InvalidRequest => formatter.write_str("invalid offline-cash receive request"),
            Self::ContextMismatch => formatter.write_str("offline-cash context mismatch"),
            Self::ZeroAmount => formatter.write_str("offline-cash amount must be positive"),
            Self::InsufficientFunds => formatter.write_str("insufficient offline-cash balance"),
            Self::ArithmeticOverflow => formatter.write_str("offline-cash amount overflow"),
            Self::RequestNotLive => formatter.write_str("offline-cash request is not live"),
            Self::RequestNotExpired => {
                formatter.write_str("offline-cash pending request has not expired")
            }
            Self::RequestHeadMismatch => {
                formatter.write_str("offline-cash request names another balance head")
            }
            Self::PendingRequestActive => {
                formatter.write_str("offline-cash wallet already has a pending request")
            }
            Self::NoPendingRequest => {
                formatter.write_str("offline-cash wallet has no pending request")
            }
            Self::RequestMismatch => formatter.write_str("offline-cash request binding mismatch"),
            Self::CreditMismatch => formatter.write_str("offline-cash credit binding mismatch"),
            Self::TerminalVerificationMismatch => {
                formatter.write_str("offline-cash terminal verification binding mismatch")
            }
            Self::EncryptedOpeningMismatch => {
                formatter.write_str("offline-cash encrypted opening binding mismatch")
            }
            Self::StaleState => formatter.write_str("offline-cash state changed after preparation"),
            Self::CorruptPlan => formatter.write_str("offline-cash prepared transition corrupted"),
            Self::GuardBindingMismatch => {
                formatter.write_str("offline-cash hardware authorization binding mismatch")
            }
            Self::HardwareIntentActive => {
                formatter.write_str("offline-cash hardware already owns another wallet intent")
            }
            Self::HardwareIntentMismatch => {
                formatter.write_str("offline-cash hardware intent binding mismatch")
            }
            Self::PaymentNotPublished => {
                formatter.write_str("offline-cash sender intent has no published payment")
            }
            Self::AcknowledgementMismatch => {
                formatter.write_str("offline-cash acknowledgement binding mismatch")
            }
            Self::GuardSequenceExhausted => {
                formatter.write_str("offline-cash hardware sequence exhausted")
            }
            Self::HardwareGuard(error) => {
                write!(formatter, "offline-cash hardware guard: {error:?}")
            }
            Self::PaymentOutbox(error) => {
                write!(formatter, "offline-cash payment outbox: {error:?}")
            }
        }
    }
}

impl std::error::Error for StateTransitionErrorV1 {}

#[cfg(test)]
mod tests;
