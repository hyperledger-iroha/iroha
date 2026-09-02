//! Recoverable sender/redeemer candidate lifecycle and capacity reservations.
//!
//! Nothing in this module exposes an aggregate predecessor or successor. Those values remain in
//! the sender's authenticated journal and are passed only to the local transition/wrapper prover.

use std::collections::BTreeMap;

use iroha_data_model::offline::{
    OFFLINE_CASH_ACCEPTANCE_TICKET_MIN_RESERVED_INBOX_BYTES_V1,
    OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1, OFFLINE_CASH_COMMIT_CERTIFICATE_MAX_BYTES_V1,
    OFFLINE_CASH_ENCRYPTED_CREDIT_MAX_BYTES_V1, OFFLINE_CASH_NO_COMMIT_CLOSURE_MAX_BYTES_V1,
    OFFLINE_CASH_OUTBOX_RETRY_METADATA_MAX_BYTES_V1, OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1,
    OFFLINE_CASH_PAYMENT_MAX_BYTES_V1, OFFLINE_CASH_PAYMENT_OUTBOX_MIN_BYTES_V1,
    OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1, OFFLINE_CASH_PRE_TICKET_EXCHANGE_MAX_BYTES_V1,
    OFFLINE_CASH_RECOVERY_SEEDS_MAX_BYTES_V1, OFFLINE_CASH_REDEMPTION_OUTBOX_MIN_BYTES_V1,
    OFFLINE_CASH_REDEMPTION_VOUCHER_MAX_BYTES_V1,
    OFFLINE_CASH_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1, OFFLINE_CASH_WIRE_VERSION_V1,
    OfflineCashAcceptanceIntentAuthorizationV1, OfflineCashAcceptanceIntentV1,
    OfflineCashAcceptanceTicketV1, OfflineCashAuthenticatedReleaseV1,
    OfflineCashCommitCertificateV1, OfflineCashCommitEvidenceV1, OfflineCashCommitWrapperProofV1,
    OfflineCashLifecycleBindingV1, OfflineCashNoCommitClosureStatementV1,
    OfflineCashOperationKindV1, OfflineCashOutboxReservationV1, OfflineCashPairedProofV1,
    OfflineCashPaymentRequestV1, OfflineCashPaymentV1, OfflineCashRedemptionStatementV1,
    OfflineCashRedemptionVoucherV1, OfflineCashTransferStatementV1,
};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

use super::{
    DigestV1, OfflineCashStateErrorV1, OfflineCashStateV1, TransitionProofStatementV1,
    canonical_sha256_digest,
};
use crate::zk::offline_cash_v1_recursion::{
    OfflineCashPastaParityV1, OfflineCashRecursionArtifactsV1, OfflineCashRecursivePublicOutputV1,
    OfflineCashRecursiveVerifierV1, OfflineCashStateRelationPublicInputsV1,
    VerifiedOfflineCashNoCommitClosureV1, canonical_commit_certificate_digest_v1,
    verify_offline_cash_recursive_proof_v1, verify_offline_cash_state_proof_v1,
};

const CIPHERTEXT_DIGEST_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:ciphertext";
const PREPARATION_ID_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:outgoing-preparation";
const CANDIDATE_ENVELOPE_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:precommit-candidate";
const OUTGOING_ENVELOPE_DIGEST_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:terminal-envelope";
const TERMINAL_DECISION_ACCUMULATOR_DOMAIN_V1: &[u8] =
    b"iroha:offline-cash:v1:terminal-decision-accumulator";
const TERMINAL_TICKET_RETRY_HORIZON_V1: usize = 64;
// A pending snapshot contains two logical views of the payment and receiver stage certificate
// (the pending fold record and the byte-identical ACK replay record). Keep the complete
// 65,536-byte GuardBundle, both size-bounded request/payment encodings, two acknowledgement encodings,
// and generous fixed-structure/framing headroom inside the allocation made before ticket issue.
// Exact materialized collection bytes are checked by the state-machine projection below; this is
// only the pre-commit worst-case reservation, never a history/count admission bound.
pub(super) const RECEIVER_SNAPSHOT_ENTRY_MAX_BYTES_V1: u64 = 256 * 1024;
const RECEIVER_SNAPSHOT_ENTRY_EXTRA_BYTES_V1: u64 = RECEIVER_SNAPSHOT_ENTRY_MAX_BYTES_V1
    - OFFLINE_CASH_ACCEPTANCE_TICKET_MIN_RESERVED_INBOX_BYTES_V1 as u64;
const _: () = assert!(
    RECEIVER_SNAPSHOT_ENTRY_MAX_BYTES_V1
        > 2 * (OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1 as u64
            + OFFLINE_CASH_PAYMENT_MAX_BYTES_V1 as u64
            + super::OFFLINE_CASH_GUARD_BUNDLE_MAX_BYTES_V1 as u64)
            + 2 * OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1 as u64
            + 16 * 1024
);
// This V1 book has fewer than sixteen independently length-prefixed fields. A canonical u64
// varint can grow by at most nine bytes (one to ten bytes), so this covers every enclosing-field
// threshold a single terminal transition can cross even when several fields change together.
const TERMINAL_METADATA_LENGTH_PREFIX_HEADROOM_BYTES_V1: u64 = 16 * 9;
// A usable lane must be able to retain one complete receiver workflow, not merely the raw bytes
// named by the ticket. Reserve the maximum snapshot plus both copies of the pre-ticket exchange
// needed by recovery, the authenticated no-commit closure, and every enclosing length-prefix
// transition. These are byte bounds for one live operation, never cumulative history bounds.
const COMPLETE_RECEIVE_OPERATION_HEADROOM_BYTES_V1: u64 = 2
    * OFFLINE_CASH_PRE_TICKET_EXCHANGE_MAX_BYTES_V1 as u64
    + OFFLINE_CASH_NO_COMMIT_CLOSURE_MAX_BYTES_V1 as u64
    + TERMINAL_METADATA_LENGTH_PREFIX_HEADROOM_BYTES_V1;
const MINIMUM_DURABLE_INBOX_BYTES_V1: u64 =
    RECEIVER_SNAPSHOT_ENTRY_MAX_BYTES_V1 + COMPLETE_RECEIVE_OPERATION_HEADROOM_BYTES_V1;
// The protocol reservation is hardware-bound and covers the transported proof/envelope artifacts.
// Core may need a larger local slot because its recoverable journal deliberately retains typed
// values beside their byte-identical retry encoding. That implementation-only allocation is
// recomputed from the operation kind and never changes the public reservation commitment.
const MAXIMUM_OPERATION_OUTBOX_RESERVATION_BYTES_V1: u64 =
    if OFFLINE_CASH_PAYMENT_OUTBOX_MIN_BYTES_V1 >= OFFLINE_CASH_REDEMPTION_OUTBOX_MIN_BYTES_V1 {
        OFFLINE_CASH_PAYMENT_OUTBOX_MIN_BYTES_V1 as u64
    } else {
        OFFLINE_CASH_REDEMPTION_OUTBOX_MIN_BYTES_V1 as u64
    };
// The prepared record is checked against this implementation-storage cap before the hardware can
// lock its predecessor. The remaining terms below are independently bounded by their canonical
// wire validators, so one live-slot reservation covers the largest durable finalized record,
// including both its typed envelope and byte-identical retry bytes.
const PREPARED_OUTGOING_CANDIDATE_MAX_BYTES_V1: u64 = 64 * 1024;
const DURABLE_OUTGOING_RECORD_STORED_DIGEST_BYTES_V1: u64 = 3 * 32;
const DURABLE_OUTGOING_RECORD_FRAMING_HEADROOM_BYTES_V1: u64 =
    TERMINAL_METADATA_LENGTH_PREFIX_HEADROOM_BYTES_V1;
const FINALIZED_OUTBOX_ENTRY_KEY_BYTES_V1: u64 = 32;
const FINALIZED_OUTBOX_ENTRY_FRAMING_HEADROOM_BYTES_V1: u64 = 2 * 9;
const OUTBOX_TERMINAL_LEDGER_METADATA_HEADROOM_BYTES_V1: u64 = 512;

const fn maximum_durable_outgoing_record_bytes_for_envelope_v1(
    terminal_envelope_bytes: u64,
) -> u64 {
    PREPARED_OUTGOING_CANDIDATE_MAX_BYTES_V1
        + OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1 as u64
        + OFFLINE_CASH_COMMIT_CERTIFICATE_MAX_BYTES_V1 as u64
        + 2 * terminal_envelope_bytes
        + OFFLINE_CASH_OUTBOX_RETRY_METADATA_MAX_BYTES_V1 as u64
        + DURABLE_OUTGOING_RECORD_STORED_DIGEST_BYTES_V1
        + DURABLE_OUTGOING_RECORD_FRAMING_HEADROOM_BYTES_V1
}

const PAYMENT_DURABLE_OUTGOING_RECORD_MAX_BYTES_V1: u64 =
    maximum_durable_outgoing_record_bytes_for_envelope_v1(OFFLINE_CASH_PAYMENT_MAX_BYTES_V1 as u64);
const REDEMPTION_DURABLE_OUTGOING_RECORD_MAX_BYTES_V1: u64 =
    maximum_durable_outgoing_record_bytes_for_envelope_v1(
        OFFLINE_CASH_REDEMPTION_VOUCHER_MAX_BYTES_V1 as u64,
    );
const PAYMENT_LIVE_OUTBOX_SLOT_MAX_BYTES_V1: u64 = PAYMENT_DURABLE_OUTGOING_RECORD_MAX_BYTES_V1
    + FINALIZED_OUTBOX_ENTRY_KEY_BYTES_V1
    + FINALIZED_OUTBOX_ENTRY_FRAMING_HEADROOM_BYTES_V1;
const REDEMPTION_LIVE_OUTBOX_SLOT_MAX_BYTES_V1: u64 =
    REDEMPTION_DURABLE_OUTGOING_RECORD_MAX_BYTES_V1
        + FINALIZED_OUTBOX_ENTRY_KEY_BYTES_V1
        + FINALIZED_OUTBOX_ENTRY_FRAMING_HEADROOM_BYTES_V1;
const MAXIMUM_LIVE_OUTBOX_SLOT_BYTES_V1: u64 = if MAXIMUM_OPERATION_OUTBOX_RESERVATION_BYTES_V1
    >= PAYMENT_LIVE_OUTBOX_SLOT_MAX_BYTES_V1
    && MAXIMUM_OPERATION_OUTBOX_RESERVATION_BYTES_V1 >= REDEMPTION_LIVE_OUTBOX_SLOT_MAX_BYTES_V1
{
    MAXIMUM_OPERATION_OUTBOX_RESERVATION_BYTES_V1
} else if PAYMENT_LIVE_OUTBOX_SLOT_MAX_BYTES_V1 >= REDEMPTION_LIVE_OUTBOX_SLOT_MAX_BYTES_V1 {
    PAYMENT_LIVE_OUTBOX_SLOT_MAX_BYTES_V1
} else {
    REDEMPTION_LIVE_OUTBOX_SLOT_MAX_BYTES_V1
};
const MINIMUM_DURABLE_OUTBOX_BYTES_V1: u64 =
    MAXIMUM_LIVE_OUTBOX_SLOT_BYTES_V1 + OUTBOX_TERMINAL_LEDGER_METADATA_HEADROOM_BYTES_V1;
const _: () = assert!(MINIMUM_DURABLE_INBOX_BYTES_V1 == 298_640);
const _: () = assert!(MINIMUM_DURABLE_OUTBOX_BYTES_V1 == 90_274);

const fn maximum_durable_outgoing_record_bytes_v1(
    operation_kind: OfflineCashOperationKindV1,
) -> Option<u64> {
    match operation_kind {
        OfflineCashOperationKindV1::SendSplit => Some(PAYMENT_DURABLE_OUTGOING_RECORD_MAX_BYTES_V1),
        OfflineCashOperationKindV1::RedeemSplit => {
            Some(REDEMPTION_DURABLE_OUTGOING_RECORD_MAX_BYTES_V1)
        }
        _ => None,
    }
}

/// Return the local physical slot reserved before issuing hardware commit authority.
pub(super) const fn implementation_live_outbox_slot_bytes_v1(
    operation_kind: OfflineCashOperationKindV1,
) -> Option<u64> {
    match operation_kind {
        OfflineCashOperationKindV1::SendSplit => Some(PAYMENT_LIVE_OUTBOX_SLOT_MAX_BYTES_V1),
        OfflineCashOperationKindV1::RedeemSplit => Some(REDEMPTION_LIVE_OUTBOX_SLOT_MAX_BYTES_V1),
        _ => None,
    }
}

/// Physical durable-storage budget assigned to one offline-cash lane.
///
/// These are resource bounds, never protocol history/count bounds. A device may increase them or
/// reclaim completed slots without changing public proofs or payment semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct OfflineCashDurableCapacityV1 {
    /// Bytes durably available for acceptance-ticket reservations and staged credits.
    pub inbox_bytes: u64,
    /// Bytes durably available for prepared candidates and byte-identical terminal retries.
    pub outbox_bytes: u64,
}

impl OfflineCashDurableCapacityV1 {
    /// Conservative inbox floor for one complete recoverable receive operation.
    pub const MINIMUM_INBOX_BYTES: u64 = MINIMUM_DURABLE_INBOX_BYTES_V1;
    /// Conservative outbox floor for one complete recoverable terminal operation.
    pub const MINIMUM_OUTBOX_BYTES: u64 = MINIMUM_DURABLE_OUTBOX_BYTES_V1;

    /// Validate capacity for one complete recoverable receive and terminal operation.
    ///
    /// The floors include the raw protocol artifacts and conservative surrounding durable-metadata
    /// headroom. Permanent decisions and later concurrent operations are still charged exactly by
    /// the receiver book and sender journal, so larger deployments must provision additional bytes.
    pub fn validate(self) -> Result<(), OfflineCashStateErrorV1> {
        if self.inbox_bytes < Self::MINIMUM_INBOX_BYTES
            || self.outbox_bytes < Self::MINIMUM_OUTBOX_BYTES
        {
            return Err(OfflineCashStateErrorV1::InvalidDurableCapacity);
        }
        Ok(())
    }
}

/// Byte-exact receiver-hardware ticket decision persisted before sender commitment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurableAcceptanceTicketDecisionV1 {
    /// Exact signed ticket returned for every identical authorization replay.
    pub ticket: OfflineCashAcceptanceTicketV1,
    /// Canonical digest of `ticket` under its bound request and intent.
    pub ticket_digest: DigestV1,
}

/// Result of durably reserving one receiver acceptance ticket.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AcceptanceTicketReservationOutcomeV1 {
    /// A new hardware-backed inbox reservation was installed.
    Reserved(DurableAcceptanceTicketDecisionV1),
    /// The byte-identical reservation had already been installed during recovery.
    AlreadyReserved(DurableAcceptanceTicketDecisionV1),
}

/// Result of consuming a one-use acceptance ticket while staging its payment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcceptanceTicketUseOutcomeV1 {
    /// The ticket was consumed for the first time.
    Consumed,
    /// The exact payment had already consumed the ticket.
    Duplicate,
}

/// Result of durably opening authenticated sender no-commit recovery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcceptanceTicketNoCommitRecoveryOutcomeV1 {
    /// The exact authenticated recovery entered its capacity-preserving pending state.
    Begun,
    /// The same authenticated recovery was already durably pending.
    AlreadyPending,
    /// The same authenticated recovery had already reached its permanent closure tombstone.
    AlreadyClosed,
}

/// Result of closing one authenticated sender no-commit recovery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcceptanceTicketNoCommitClosureOutcomeV1 {
    /// The unresolved request charge and equivalent delivery slot were released exactly once.
    Closed,
    /// The same closure was already durably applied.
    AlreadyClosed,
}

/// Opaque native-verifier decision for one acceptance-intent authorization.
///
/// This decision is not decodable and its constructor is crate-private. A successful host callback
/// therefore cannot be confused with proof authority: receiver capacity is consumed only after
/// the native verifier has decided both recursive parities and their delayed histories under the
/// exact authenticated CommitWrapper key roles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OfflineCashAcceptanceIntentAuthorizationDecisionV1 {
    release_id: DigestV1,
    artifact_manifest_digest: DigestV1,
    vk_set_digest: DigestV1,
    request_digest: DigestV1,
    authorization_digest: DigestV1,
    eq_protocol_digest: DigestV1,
    ep_protocol_digest: DigestV1,
}

impl OfflineCashAcceptanceIntentAuthorizationDecisionV1 {
    /// Mint a decision after native verification has accepted both proof parities and histories.
    pub(crate) fn authenticated(
        release_id: DigestV1,
        artifact_manifest_digest: DigestV1,
        vk_set_digest: DigestV1,
        request_digest: DigestV1,
        authorization_digest: DigestV1,
        eq_protocol_digest: DigestV1,
        ep_protocol_digest: DigestV1,
    ) -> Result<Self, String> {
        if [
            release_id,
            artifact_manifest_digest,
            vk_set_digest,
            request_digest,
            authorization_digest,
            eq_protocol_digest,
            ep_protocol_digest,
        ]
        .contains(&[0; 32])
            || eq_protocol_digest == ep_protocol_digest
        {
            return Err("invalid Offline Cash acceptance-intent verifier decision".to_owned());
        }
        Ok(Self {
            release_id,
            artifact_manifest_digest,
            vk_set_digest,
            request_digest,
            authorization_digest,
            eq_protocol_digest,
            ep_protocol_digest,
        })
    }

    fn authorizes(
        &self,
        release: &OfflineCashAuthenticatedReleaseV1,
        request_digest: DigestV1,
        authorization_digest: DigestV1,
        authorization: &OfflineCashAcceptanceIntentAuthorizationV1,
    ) -> bool {
        self.release_id == release.release_id()
            && self.artifact_manifest_digest == release.manifest_digest()
            && self.vk_set_digest == release.vk_set_digest()
            && self.request_digest == request_digest
            && self.authorization_digest == authorization_digest
            && self.eq_protocol_digest == release.commit_wrapper_eq_protocol_digest()
            && self.ep_protocol_digest == release.commit_wrapper_ep_protocol_digest()
            && self.eq_protocol_digest == authorization.proof.eq_protocol_digest
            && self.ep_protocol_digest == authorization.proof.ep_protocol_digest
    }
}

/// Fail-closed verifier for the sender capability proof presented before ticket issuance.
///
/// Implementations must use only artifacts from `release`, verify both Pasta parities, and prove
/// that qualified non-forking sender hardware reserved the intent's one-use predecessor
/// authorization for the exact request and amount.
pub trait OfflineCashAcceptanceIntentAuthorizationVerifierV1 {
    /// Verify the proof-bearing intent authorization against its authenticated release.
    fn verify_acceptance_intent_authorization(
        &self,
        release: &OfflineCashAuthenticatedReleaseV1,
        request: &OfflineCashPaymentRequestV1,
        authorization: &OfflineCashAcceptanceIntentAuthorizationV1,
    ) -> Result<OfflineCashAcceptanceIntentAuthorizationDecisionV1, String>;
}

/// Opaque proof-verified authorization consumed by receiver ticket issuance.
///
/// The fields are deliberately private and this type is neither cloneable nor decodable. The only
/// production constructor authenticates the complete release/profile and invokes the native
/// paired-proof verifier. This prevents raw envelopes or host booleans from consuming permanent
/// intent-decision state or inbox capacity.
#[derive(Debug, PartialEq, Eq)]
pub struct VerifiedOfflineCashAcceptanceIntentAuthorizationV1 {
    request: OfflineCashPaymentRequestV1,
    intent: OfflineCashAcceptanceIntentV1,
    proof_envelope_digest: DigestV1,
}

impl VerifiedOfflineCashAcceptanceIntentAuthorizationV1 {
    /// Authenticate and verify one sender intent authorization.
    pub fn verify<V: OfflineCashAcceptanceIntentAuthorizationVerifierV1>(
        request: OfflineCashPaymentRequestV1,
        authorization: OfflineCashAcceptanceIntentAuthorizationV1,
        release: &OfflineCashAuthenticatedReleaseV1,
        verifier: &V,
    ) -> Result<Self, OfflineCashStateErrorV1> {
        if authorization.version != OFFLINE_CASH_WIRE_VERSION_V1
            || request.release_id != release.release_id()
            || authorization.statement.release_id != release.release_id()
            || authorization.statement.vk_digest != release.vk_set_digest()
            || authorization.statement.artifact_manifest_digest != release.manifest_digest()
        {
            return Err(OfflineCashStateErrorV1::InvalidAcceptanceIntentAuthorization);
        }
        authorization
            .validate_shape_against(&request)
            .map_err(|_| OfflineCashStateErrorV1::InvalidAcceptanceIntentAuthorization)?;
        let receiver_profile = release
            .enabled_profile(request.hardware_credential.hardware_profile_id)
            .ok_or(OfflineCashStateErrorV1::InvalidHardwareProfile)?;
        request
            .validate_against_profile(&receiver_profile.hardware_profile)
            .map_err(|_| OfflineCashStateErrorV1::InvalidHardwareProfile)?;
        let request_digest = request
            .canonical_digest()
            .map_err(|_| OfflineCashStateErrorV1::InvalidAcceptanceIntentAuthorization)?;
        let proof_envelope_digest = authorization
            .canonical_digest_against(&request)
            .map_err(|_| OfflineCashStateErrorV1::InvalidAcceptanceIntentAuthorization)?;
        let decision = verifier
            .verify_acceptance_intent_authorization(release, &request, &authorization)
            .map_err(OfflineCashStateErrorV1::ProofRejected)?;
        if proof_envelope_digest == [0; 32]
            || !decision.authorizes(
                release,
                request_digest,
                proof_envelope_digest,
                &authorization,
            )
        {
            return Err(OfflineCashStateErrorV1::InvalidAcceptanceIntentAuthorization);
        }
        Ok(Self {
            request,
            intent: authorization.intent(),
            proof_envelope_digest,
        })
    }

    /// Construct a verified authorization capability for state-machine fixtures.
    #[cfg(test)]
    pub(super) fn from_test_parts(
        request: OfflineCashPaymentRequestV1,
        intent: OfflineCashAcceptanceIntentV1,
        proof_envelope_digest: DigestV1,
    ) -> Self {
        assert_ne!(
            proof_envelope_digest, [0; 32],
            "test authorization digest must be non-zero"
        );
        Self {
            request,
            intent,
            proof_envelope_digest,
        }
    }

    /// Return the digest of the exact proof-bearing authorization envelope.
    #[must_use]
    pub const fn proof_envelope_digest(&self) -> DigestV1 {
        self.proof_envelope_digest
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct ReservedAcceptanceTicketV1 {
    request: OfflineCashPaymentRequestV1,
    intent: OfflineCashAcceptanceIntentV1,
    intent_authorization_digest: DigestV1,
    ticket: OfflineCashAcceptanceTicketV1,
    ticket_digest: DigestV1,
    consumed_payment_digest: Option<DigestV1>,
    consumed_amount: Option<u128>,
    slot_released: bool,
    no_commit_recovery: Option<PendingAcceptanceTicketNoCommitRecoveryV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct DurableAcceptanceIntentTicketDecisionRecordV1 {
    request: OfflineCashPaymentRequestV1,
    request_digest: DigestV1,
    intent: OfflineCashAcceptanceIntentV1,
    intent_digest: DigestV1,
    intent_authorization_digest: DigestV1,
    ticket: OfflineCashAcceptanceTicketV1,
    ticket_digest: DigestV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
struct PendingAcceptanceTicketNoCommitRecoveryV1 {
    statement: OfflineCashNoCommitClosureStatementV1,
    statement_digest: DigestV1,
    closure_digest: DigestV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
struct ClosedAcceptanceTicketNoCommitTombstoneV1 {
    acceptance_ticket_id: DigestV1,
    recovery_id: DigestV1,
    cancellation_nullifier: DigestV1,
    statement_digest: DigestV1,
    closure_digest: DigestV1,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct AcceptanceTicketCapacityMetersV1 {
    committed_inbox_bytes: u64,
    retained_metadata_bytes: u64,
    reserved_terminal_metadata_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode)]
struct TerminalDecisionAccumulatorStepV1 {
    previous_accumulator: DigestV1,
    previous_terminal_count: u128,
    acceptance_ticket_id: DigestV1,
    ticket_digest: DigestV1,
    intent_authorization_digest: DigestV1,
    payment_digest: DigestV1,
    exact_amount: u128,
}

/// Receiver-owned capacity and one-use ticket ledger.
///
/// Expiry is never interpreted as permission to free capacity. A reservation remains live until
/// it is consumed by the bound payment or an authenticated online-recovery protocol closes it.
/// Issuance also reserves the larger of the folded-payment record and no-commit tombstone/index
/// footprints. Terminal processing converts that headroom into exact retained canonical bytes;
/// it never performs a new capacity admission and there is no entry-count admission limit.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct OfflineCashAcceptanceTicketBookV1 {
    total_inbox_bytes: u64,
    committed_inbox_bytes: u64,
    retained_metadata_bytes: u64,
    reserved_terminal_metadata_bytes: u64,
    receiver_snapshot_live_bytes: u64,
    receiver_snapshot_retained_bytes: u64,
    tickets: BTreeMap<DigestV1, ReservedAcceptanceTicketV1>,
    intent_ticket_decisions: BTreeMap<DigestV1, DurableAcceptanceIntentTicketDecisionRecordV1>,
    ticket_intent_ids: BTreeMap<DigestV1, DigestV1>,
    closed_no_commit_tombstones: BTreeMap<DigestV1, ClosedAcceptanceTicketNoCommitTombstoneV1>,
    no_commit_recovery_ticket_ids: BTreeMap<DigestV1, DigestV1>,
    no_commit_cancellation_ticket_ids: BTreeMap<DigestV1, DigestV1>,
    terminal_decision_accumulator: DigestV1,
    compacted_terminal_count: u128,
    terminal_retry_order: Vec<DigestV1>,
}

fn canonical_map_entry_metadata_bytes_v1<K, V>(
    key: K,
    value: V,
) -> Result<u64, OfflineCashStateErrorV1>
where
    K: Ord + Encode,
    V: Encode,
{
    let empty = norito::encode_canonical(&BTreeMap::<K, V>::new())
        .map_err(|_| OfflineCashStateErrorV1::CanonicalEncoding)?;
    let mut singleton = BTreeMap::new();
    singleton.insert(key, value);
    let singleton = norito::encode_canonical(&singleton)
        .map_err(|_| OfflineCashStateErrorV1::CanonicalEncoding)?;
    let bytes = singleton
        .len()
        .checked_sub(empty.len())
        .ok_or(OfflineCashStateErrorV1::StateInvariant)?;
    u64::try_from(bytes).map_err(|_| OfflineCashStateErrorV1::ArithmeticOverflow)
}

fn canonical_retry_order_metadata_bytes_v1(
    retry_order: &[DigestV1],
) -> Result<u64, OfflineCashStateErrorV1> {
    let empty = norito::encode_canonical(&Vec::<DigestV1>::new())
        .map_err(|_| OfflineCashStateErrorV1::CanonicalEncoding)?;
    let encoded = norito::encode_canonical(&retry_order.to_vec())
        .map_err(|_| OfflineCashStateErrorV1::CanonicalEncoding)?;
    let bytes = encoded
        .len()
        .checked_sub(empty.len())
        .ok_or(OfflineCashStateErrorV1::StateInvariant)?;
    u64::try_from(bytes).map_err(|_| OfflineCashStateErrorV1::ArithmeticOverflow)
}

fn checked_capacity_add_v1(total: &mut u64, bytes: u64) -> Result<(), OfflineCashStateErrorV1> {
    *total = total
        .checked_add(bytes)
        .ok_or(OfflineCashStateErrorV1::ArithmeticOverflow)?;
    Ok(())
}

fn no_commit_identity_index_metadata_bytes_v1(
    recovery_id: DigestV1,
    cancellation_nullifier: DigestV1,
    ticket_id: DigestV1,
) -> Result<u64, OfflineCashStateErrorV1> {
    let recovery = canonical_map_entry_metadata_bytes_v1(recovery_id, ticket_id)?;
    let cancellation = canonical_map_entry_metadata_bytes_v1(cancellation_nullifier, ticket_id)?;
    recovery
        .checked_add(cancellation)
        .ok_or(OfflineCashStateErrorV1::ArithmeticOverflow)
}

fn closed_no_commit_metadata_bytes_v1(
    tombstone: ClosedAcceptanceTicketNoCommitTombstoneV1,
) -> Result<u64, OfflineCashStateErrorV1> {
    let mut bytes =
        canonical_map_entry_metadata_bytes_v1(tombstone.acceptance_ticket_id, tombstone)?;
    checked_capacity_add_v1(
        &mut bytes,
        no_commit_identity_index_metadata_bytes_v1(
            tombstone.recovery_id,
            tombstone.cancellation_nullifier,
            tombstone.acceptance_ticket_id,
        )?,
    )?;
    Ok(bytes)
}

fn retained_payment_metadata_bytes_v1(
    ticket_id: DigestV1,
    entry: &ReservedAcceptanceTicketV1,
) -> Result<u64, OfflineCashStateErrorV1> {
    let mut terminal = entry.clone();
    terminal.consumed_payment_digest = Some([u8::MAX; 32]);
    terminal.consumed_amount = Some(terminal.ticket.exact_amount);
    terminal.slot_released = true;
    terminal.no_commit_recovery = None;
    let mut bytes = canonical_map_entry_metadata_bytes_v1(ticket_id, terminal)?;
    checked_capacity_add_v1(
        &mut bytes,
        canonical_retry_order_metadata_bytes_v1(&[[u8::MAX; 32]])?,
    )?;
    Ok(bytes)
}

fn terminal_metadata_reservation_bytes_v1(
    ticket_id: DigestV1,
    entry: &ReservedAcceptanceTicketV1,
) -> Result<u64, OfflineCashStateErrorV1> {
    let payment = retained_payment_metadata_bytes_v1(ticket_id, entry)?;
    let closure = closed_no_commit_metadata_bytes_v1(ClosedAcceptanceTicketNoCommitTombstoneV1 {
        acceptance_ticket_id: ticket_id,
        recovery_id: [u8::MAX; 32],
        cancellation_nullifier: [u8::MAX - 1; 32],
        statement_digest: [u8::MAX - 2; 32],
        closure_digest: [u8::MAX - 3; 32],
    })?;
    let mut pending_entry = entry.clone();
    pending_entry.no_commit_recovery = Some(PendingAcceptanceTicketNoCommitRecoveryV1 {
        statement: OfflineCashNoCommitClosureStatementV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            release_id: [u8::MAX; 32],
            suite_id: [u8::MAX; 32],
            vk_digest: [u8::MAX; 32],
            artifact_manifest_digest: [u8::MAX; 32],
            sender_hardware_binding_commitment: [u8::MAX; 32],
            request_id: entry.request.request_id,
            request_digest: [u8::MAX; 32],
            acceptance_ticket_id: ticket_id,
            ticket_digest: entry.ticket_digest,
            intent_authorization_digest: entry.intent_authorization_digest,
            intent_digest: [u8::MAX; 32],
            exact_amount: entry.ticket.exact_amount,
            sender_one_time_commitment: entry.intent.sender_one_time_commitment,
            recovery_id: [u8::MAX; 32],
            cancellation_nullifier: [u8::MAX - 1; 32],
            equivalent_delivery_slot_commitment: [u8::MAX; 32],
        },
        statement_digest: [u8::MAX; 32],
        closure_digest: [u8::MAX; 32],
    });
    let base_entry = canonical_map_entry_metadata_bytes_v1(ticket_id, entry.clone())?;
    let pending_entry = canonical_map_entry_metadata_bytes_v1(ticket_id, pending_entry)?;
    let pending_indexes =
        no_commit_identity_index_metadata_bytes_v1([u8::MAX; 32], [u8::MAX - 1; 32], ticket_id)?;
    let pending = pending_entry
        .checked_sub(base_entry)
        .and_then(|bytes| bytes.checked_add(pending_indexes))
        .ok_or(OfflineCashStateErrorV1::ArithmeticOverflow)?;
    payment
        .max(closure)
        .max(pending)
        .checked_add(TERMINAL_METADATA_LENGTH_PREFIX_HEADROOM_BYTES_V1)
        .ok_or(OfflineCashStateErrorV1::ArithmeticOverflow)
}

fn canonical_dynamic_ticket_book_metadata_bytes_v1(
    book: &OfflineCashAcceptanceTicketBookV1,
) -> Result<u64, OfflineCashStateErrorV1> {
    let mut normalized = book.clone();
    normalized.total_inbox_bytes = 0;
    normalized.committed_inbox_bytes = 0;
    normalized.retained_metadata_bytes = 0;
    normalized.reserved_terminal_metadata_bytes = 0;
    normalized.receiver_snapshot_live_bytes = 0;
    normalized.receiver_snapshot_retained_bytes = 0;
    let encoded = norito::encode_canonical(&normalized)
        .map_err(|_| OfflineCashStateErrorV1::CanonicalEncoding)?;
    let empty = norito::encode_canonical(&OfflineCashAcceptanceTicketBookV1::new(0))
        .map_err(|_| OfflineCashStateErrorV1::CanonicalEncoding)?;
    let bytes = encoded
        .len()
        .checked_sub(empty.len())
        .ok_or(OfflineCashStateErrorV1::StateInvariant)?;
    u64::try_from(bytes).map_err(|_| OfflineCashStateErrorV1::ArithmeticOverflow)
}

fn acceptance_ticket_capacity_meters_v1(
    book: &OfflineCashAcceptanceTicketBookV1,
) -> Result<AcceptanceTicketCapacityMetersV1, OfflineCashStateErrorV1> {
    let book_retained_metadata_bytes = canonical_dynamic_ticket_book_metadata_bytes_v1(book)?;
    let retained_metadata_bytes = book_retained_metadata_bytes
        .checked_add(book.receiver_snapshot_live_bytes)
        .and_then(|bytes| bytes.checked_add(book.receiver_snapshot_retained_bytes))
        .ok_or(OfflineCashStateErrorV1::ArithmeticOverflow)?;
    let mut live_inbox_bytes = 0_u64;
    let mut initial_terminal_metadata_reservation = 0_u64;
    for (ticket_id, entry) in &book.tickets {
        if !entry.slot_released {
            checked_capacity_add_v1(
                &mut live_inbox_bytes,
                u64::from(entry.ticket.reserved_inbox_bytes),
            )?;
            checked_capacity_add_v1(
                &mut initial_terminal_metadata_reservation,
                terminal_metadata_reservation_bytes_v1(*ticket_id, entry)?,
            )?;
            checked_capacity_add_v1(
                &mut initial_terminal_metadata_reservation,
                RECEIVER_SNAPSHOT_ENTRY_EXTRA_BYTES_V1,
            )?;
        }
    }

    let mut without_materialized_terminal = book.clone();
    let pending_identities: Vec<_> = without_materialized_terminal
        .tickets
        .values_mut()
        .filter_map(|entry| {
            if !entry.slot_released {
                entry.consumed_payment_digest = None;
                entry.consumed_amount = None;
            }
            entry.no_commit_recovery.take().map(|pending| {
                (
                    pending.statement.recovery_id,
                    pending.statement.cancellation_nullifier,
                )
            })
        })
        .collect();
    for (recovery_id, cancellation_nullifier) in pending_identities {
        without_materialized_terminal
            .no_commit_recovery_ticket_ids
            .remove(&recovery_id);
        without_materialized_terminal
            .no_commit_cancellation_ticket_ids
            .remove(&cancellation_nullifier);
    }
    let metadata_before_terminal_materialization =
        canonical_dynamic_ticket_book_metadata_bytes_v1(&without_materialized_terminal)?;
    let materialized_terminal_metadata = book_retained_metadata_bytes
        .checked_sub(metadata_before_terminal_materialization)
        .ok_or(OfflineCashStateErrorV1::StateInvariant)?;
    let raw_reserved_terminal_metadata_bytes = initial_terminal_metadata_reservation
        .checked_sub(materialized_terminal_metadata)
        .ok_or(OfflineCashStateErrorV1::StateInvariant)?;
    let reserved_terminal_metadata_bytes = live_inbox_bytes
        .checked_add(raw_reserved_terminal_metadata_bytes)
        .and_then(|bytes| bytes.checked_sub(book.receiver_snapshot_live_bytes))
        .ok_or(OfflineCashStateErrorV1::ArithmeticOverflow)?;
    let committed_inbox_bytes = retained_metadata_bytes
        .checked_add(reserved_terminal_metadata_bytes)
        .ok_or(OfflineCashStateErrorV1::ArithmeticOverflow)?;
    Ok(AcceptanceTicketCapacityMetersV1 {
        committed_inbox_bytes,
        retained_metadata_bytes,
        reserved_terminal_metadata_bytes,
    })
}

impl OfflineCashAcceptanceTicketBookV1 {
    /// Create an empty receiver-capacity ledger.
    #[must_use]
    pub const fn new(total_inbox_bytes: u64) -> Self {
        Self {
            total_inbox_bytes,
            committed_inbox_bytes: 0,
            retained_metadata_bytes: 0,
            reserved_terminal_metadata_bytes: 0,
            receiver_snapshot_live_bytes: 0,
            receiver_snapshot_retained_bytes: 0,
            tickets: BTreeMap::new(),
            intent_ticket_decisions: BTreeMap::new(),
            ticket_intent_ids: BTreeMap::new(),
            closed_no_commit_tombstones: BTreeMap::new(),
            no_commit_recovery_ticket_ids: BTreeMap::new(),
            no_commit_cancellation_ticket_ids: BTreeMap::new(),
            terminal_decision_accumulator: [0; 32],
            compacted_terminal_count: 0,
            terminal_retry_order: Vec::new(),
        }
    }

    /// Preseed one structurally valid consumed ticket without repeatedly recomputing aggregate
    /// capacity meters in large state-machine stress tests.
    #[cfg(test)]
    pub(super) fn preseed_consumed_payment_for_test(
        &mut self,
        request: OfflineCashPaymentRequestV1,
        payment: &OfflineCashPaymentV1,
        intent_authorization_digest: DigestV1,
    ) -> Result<(), OfflineCashStateErrorV1> {
        payment
            .validate_shape_against(&request)
            .map_err(|_| OfflineCashStateErrorV1::InvalidPeerCredit)?;
        if intent_authorization_digest == [0; 32] {
            return Err(OfflineCashStateErrorV1::InvalidAcceptanceIntentAuthorization);
        }

        let request_digest = request
            .canonical_digest()
            .map_err(|_| OfflineCashStateErrorV1::InvalidPaymentRequest)?;
        let intent = payment.acceptance_intent;
        let intent_digest = intent
            .canonical_digest_against(&request)
            .map_err(|_| OfflineCashStateErrorV1::InvalidAcceptanceTicket)?;
        let ticket = payment.acceptance_ticket.clone();
        let ticket_digest = ticket
            .canonical_digest_against(&request, &intent)
            .map_err(|_| OfflineCashStateErrorV1::InvalidAcceptanceTicket)?;
        let payment_digest = payment
            .canonical_digest_against(&request)
            .map_err(|_| OfflineCashStateErrorV1::InvalidPeerCredit)?;
        let ticket_id = ticket.acceptance_ticket_id;
        let intent_id = intent.intent_id;
        if self.intent_ticket_decisions.contains_key(&intent_id)
            || self.ticket_intent_ids.contains_key(&ticket_id)
            || self.tickets.contains_key(&ticket_id)
            || self.closed_no_commit_tombstones.contains_key(&ticket_id)
        {
            return Err(OfflineCashStateErrorV1::StateInvariant);
        }

        self.intent_ticket_decisions.insert(
            intent_id,
            DurableAcceptanceIntentTicketDecisionRecordV1 {
                request: request.clone(),
                request_digest,
                intent,
                intent_digest,
                intent_authorization_digest,
                ticket: ticket.clone(),
                ticket_digest,
            },
        );
        self.ticket_intent_ids.insert(ticket_id, intent_id);
        self.tickets.insert(
            ticket_id,
            ReservedAcceptanceTicketV1 {
                request,
                intent,
                intent_authorization_digest,
                ticket,
                ticket_digest,
                consumed_payment_digest: Some(payment_digest),
                consumed_amount: Some(payment.statement.amount),
                slot_released: false,
                no_commit_recovery: None,
            },
        );
        Ok(())
    }

    /// Recompute exact meters once after [`Self::preseed_consumed_payment_for_test`] calls.
    #[cfg(test)]
    pub(super) fn finish_consumed_payment_preseed_for_test(
        &mut self,
    ) -> Result<(), OfflineCashStateErrorV1> {
        let meters = acceptance_ticket_capacity_meters_v1(self)?;
        if meters.committed_inbox_bytes > self.total_inbox_bytes {
            return Err(OfflineCashStateErrorV1::ReceiverCapacityExhausted);
        }
        self.committed_inbox_bytes = meters.committed_inbox_bytes;
        self.retained_metadata_bytes = meters.retained_metadata_bytes;
        self.reserved_terminal_metadata_bytes = meters.reserved_terminal_metadata_bytes;
        Ok(())
    }

    /// Return physical bytes still available for new tickets.
    #[must_use]
    pub const fn available_inbox_bytes(&self) -> u64 {
        self.total_inbox_bytes
            .saturating_sub(self.committed_inbox_bytes)
    }

    /// Return the physical inbox capacity governed by this durable ledger.
    #[must_use]
    pub const fn total_inbox_bytes(&self) -> u64 {
        self.total_inbox_bytes
    }

    /// Return all bytes committed to live slots, retained metadata, or terminal headroom.
    #[must_use]
    pub const fn committed_inbox_bytes(&self) -> u64 {
        self.committed_inbox_bytes
    }

    /// Return exact canonical bytes retained by durable decision and replay metadata.
    #[must_use]
    pub const fn retained_metadata_bytes(&self) -> u64 {
        self.retained_metadata_bytes
    }

    /// Return pre-reserved bytes that guarantee every live ticket can reach either terminal form.
    #[must_use]
    pub const fn reserved_terminal_metadata_bytes(&self) -> u64 {
        self.reserved_terminal_metadata_bytes
    }

    fn receiver_snapshot_live_capacity_bytes(&self) -> Result<u64, OfflineCashStateErrorV1> {
        self.tickets
            .values()
            .filter(|entry| !entry.slot_released)
            .try_fold(0_u64, |total, entry| {
                total
                    .checked_add(u64::from(entry.ticket.reserved_inbox_bytes))
                    .and_then(|bytes| bytes.checked_add(RECEIVER_SNAPSHOT_ENTRY_EXTRA_BYTES_V1))
                    .ok_or(OfflineCashStateErrorV1::ArithmeticOverflow)
            })
    }

    /// Convert pre-issued snapshot headroom into exact retained peer-credit metadata.
    ///
    /// `live_snapshot_bytes` is the exact canonical contribution of pending credits and their
    /// ACK-replay records. `retained_snapshot_bytes` is the exact contribution of folded peer
    /// receipts and consumed-credit index entries. This transition never performs new capacity
    /// admission: all materialized bytes must fit the ticket allocations committed before sender
    /// hardware could expose value.
    pub(super) fn reconcile_receiver_snapshot_usage(
        &mut self,
        live_snapshot_bytes: u64,
        retained_snapshot_bytes: u64,
        maximum_committed_bytes: u64,
    ) -> Result<(), OfflineCashStateErrorV1> {
        if live_snapshot_bytes > self.receiver_snapshot_live_capacity_bytes()? {
            return Err(OfflineCashStateErrorV1::StateInvariant);
        }
        let mut next = self.clone();
        next.receiver_snapshot_live_bytes = live_snapshot_bytes;
        next.receiver_snapshot_retained_bytes = retained_snapshot_bytes;
        let meters = acceptance_ticket_capacity_meters_v1(&next)?;
        if meters.committed_inbox_bytes > maximum_committed_bytes
            || meters.committed_inbox_bytes > next.total_inbox_bytes
        {
            return Err(OfflineCashStateErrorV1::StateInvariant);
        }
        next.committed_inbox_bytes = meters.committed_inbox_bytes;
        next.retained_metadata_bytes = meters.retained_metadata_bytes;
        next.reserved_terminal_metadata_bytes = meters.reserved_terminal_metadata_bytes;
        *self = next;
        Ok(())
    }

    /// Build the exact ticket-book successor for one atomic receiver-snapshot reconciliation.
    ///
    /// Ticket releases are applied to one private clone before exact capacity meters are recomputed
    /// once for the complete release set. No intermediate clone is externally visible, so charging the
    /// final retained snapshot contribution and releasing all of its pre-reserved terminal
    /// headroom in one reconciliation preserves the same fail-closed capacity invariant without
    /// repeatedly encoding the complete durable ticket history for every active slot.
    pub(super) fn receiver_snapshot_folded_successor(
        &self,
        live_snapshot_bytes: u64,
        retained_snapshot_bytes: u64,
        folded_tickets: &[(DigestV1, DigestV1)],
    ) -> Result<Self, OfflineCashStateErrorV1> {
        let maximum_committed_bytes = self.committed_inbox_bytes;
        let mut next = self.clone();
        next.receiver_snapshot_live_bytes = live_snapshot_bytes;
        next.receiver_snapshot_retained_bytes = retained_snapshot_bytes;
        for &(ticket_id, payment_digest) in folded_tickets {
            next.release_folded_unmetered(ticket_id, payment_digest)?;
        }
        if live_snapshot_bytes > next.receiver_snapshot_live_capacity_bytes()? {
            return Err(OfflineCashStateErrorV1::StateInvariant);
        }
        let meters = acceptance_ticket_capacity_meters_v1(&next)?;
        if meters.committed_inbox_bytes > maximum_committed_bytes
            || meters.committed_inbox_bytes > next.total_inbox_bytes
        {
            return Err(OfflineCashStateErrorV1::StateInvariant);
        }
        next.committed_inbox_bytes = meters.committed_inbox_bytes;
        next.retained_metadata_bytes = meters.retained_metadata_bytes;
        next.reserved_terminal_metadata_bytes = meters.reserved_terminal_metadata_bytes;
        Ok(next)
    }

    /// Return the authenticated rolling commitment to terminal decisions beyond retry horizon.
    #[must_use]
    pub const fn terminal_decision_accumulator(&self) -> DigestV1 {
        self.terminal_decision_accumulator
    }

    /// Return the cumulative number of terminal decisions compacted into the accumulator.
    #[must_use]
    pub const fn compacted_terminal_count(&self) -> u128 {
        self.compacted_terminal_count
    }

    /// Durably enter the capacity-preserving first phase of sender no-commit recovery.
    ///
    /// Only an opaque capability returned after release-pinned paired-proof verification can open
    /// this phase. The ticket bytes and equivalent physical delivery slot remain reserved until
    /// [`Self::close_authenticated_no_commit_recovery`] succeeds.
    pub fn begin_authenticated_no_commit_recovery(
        &mut self,
        verified: &VerifiedOfflineCashNoCommitClosureV1,
    ) -> Result<AcceptanceTicketNoCommitRecoveryOutcomeV1, OfflineCashStateErrorV1> {
        let statement = *verified.statement();
        let statement_digest = verified.statement_digest();
        let closure_digest = verified.closure_digest();
        if statement
            .canonical_digest()
            .map_err(|_| OfflineCashStateErrorV1::InvalidRecoveryMaterial)?
            != statement_digest
            || closure_digest == [0; 32]
        {
            return Err(OfflineCashStateErrorV1::InvalidRecoveryMaterial);
        }
        let ticket_id = statement.acceptance_ticket_id;
        if let Some(tombstone) = self.closed_no_commit_tombstones.get(&ticket_id) {
            if tombstone.statement_digest == statement_digest
                && tombstone.closure_digest == closure_digest
                && tombstone.recovery_id == statement.recovery_id
                && tombstone.cancellation_nullifier == statement.cancellation_nullifier
            {
                ensure_no_commit_identity_indexes(
                    &self.no_commit_recovery_ticket_ids,
                    &self.no_commit_cancellation_ticket_ids,
                    &statement,
                )?;
                return Ok(AcceptanceTicketNoCommitRecoveryOutcomeV1::AlreadyClosed);
            }
            return Err(OfflineCashStateErrorV1::InvalidRecoveryMaterial);
        }
        let entry = self
            .tickets
            .get(&ticket_id)
            .ok_or(OfflineCashStateErrorV1::InvalidAcceptanceTicket)?;
        validate_no_commit_statement_against_ticket(&statement, entry)?;
        reject_cross_ticket_no_commit_identity_reuse(
            &self.no_commit_recovery_ticket_ids,
            &self.no_commit_cancellation_ticket_ids,
            &statement,
        )?;
        if let Some(existing) = entry.no_commit_recovery {
            if existing.statement == statement
                && existing.statement_digest == statement_digest
                && existing.closure_digest == closure_digest
            {
                ensure_no_commit_identity_indexes(
                    &self.no_commit_recovery_ticket_ids,
                    &self.no_commit_cancellation_ticket_ids,
                    &statement,
                )?;
                return Ok(AcceptanceTicketNoCommitRecoveryOutcomeV1::AlreadyPending);
            }
            return Err(OfflineCashStateErrorV1::InvalidRecoveryMaterial);
        }
        if entry.consumed_payment_digest.is_some()
            || entry.consumed_amount.is_some()
            || entry.slot_released
        {
            return Err(OfflineCashStateErrorV1::InvalidAcceptanceTicket);
        }
        if self
            .no_commit_recovery_ticket_ids
            .contains_key(&statement.recovery_id)
            || self
                .no_commit_cancellation_ticket_ids
                .contains_key(&statement.cancellation_nullifier)
        {
            return Err(OfflineCashStateErrorV1::StateInvariant);
        }
        let mut next = self.clone();
        next.tickets
            .get_mut(&ticket_id)
            .ok_or(OfflineCashStateErrorV1::StateInvariant)?
            .no_commit_recovery = Some(PendingAcceptanceTicketNoCommitRecoveryV1 {
            statement,
            statement_digest,
            closure_digest,
        });
        next.no_commit_recovery_ticket_ids
            .insert(statement.recovery_id, ticket_id);
        next.no_commit_cancellation_ticket_ids
            .insert(statement.cancellation_nullifier, ticket_id);
        let capacity_meters = acceptance_ticket_capacity_meters_v1(&next)?;
        if capacity_meters.committed_inbox_bytes != self.committed_inbox_bytes {
            return Err(OfflineCashStateErrorV1::StateInvariant);
        }
        next.committed_inbox_bytes = capacity_meters.committed_inbox_bytes;
        next.retained_metadata_bytes = capacity_meters.retained_metadata_bytes;
        next.reserved_terminal_metadata_bytes = capacity_meters.reserved_terminal_metadata_bytes;
        *self = next;
        Ok(AcceptanceTicketNoCommitRecoveryOutcomeV1::Begun)
    }

    /// Close one previously opened authenticated no-commit recovery exactly once.
    ///
    /// Successful closure alone releases the physical slot. Compact statement/proof identities
    /// remain in a permanent conflict-closed tombstone, while the exact intent-to-ticket decision
    /// remains replayable;
    /// neither expiry nor relocation calls this transition.
    pub fn close_authenticated_no_commit_recovery(
        &mut self,
        verified: VerifiedOfflineCashNoCommitClosureV1,
    ) -> Result<AcceptanceTicketNoCommitClosureOutcomeV1, OfflineCashStateErrorV1> {
        let statement_digest = verified.statement_digest();
        let closure_digest = verified.closure_digest();
        let statement = verified.into_statement();
        if statement
            .canonical_digest()
            .map_err(|_| OfflineCashStateErrorV1::InvalidRecoveryMaterial)?
            != statement_digest
            || closure_digest == [0; 32]
        {
            return Err(OfflineCashStateErrorV1::InvalidRecoveryMaterial);
        }
        let ticket_id = statement.acceptance_ticket_id;
        if let Some(tombstone) = self.closed_no_commit_tombstones.get(&ticket_id) {
            if tombstone.statement_digest == statement_digest
                && tombstone.closure_digest == closure_digest
                && tombstone.recovery_id == statement.recovery_id
                && tombstone.cancellation_nullifier == statement.cancellation_nullifier
            {
                ensure_no_commit_identity_indexes(
                    &self.no_commit_recovery_ticket_ids,
                    &self.no_commit_cancellation_ticket_ids,
                    &statement,
                )?;
                return Ok(AcceptanceTicketNoCommitClosureOutcomeV1::AlreadyClosed);
            }
            return Err(OfflineCashStateErrorV1::InvalidRecoveryMaterial);
        }
        let entry = self
            .tickets
            .get(&ticket_id)
            .ok_or(OfflineCashStateErrorV1::InvalidAcceptanceTicket)?;
        validate_no_commit_statement_against_ticket(&statement, entry)?;
        let pending = entry
            .no_commit_recovery
            .ok_or(OfflineCashStateErrorV1::InvalidRecoveryMaterial)?;
        if pending.statement != statement
            || pending.statement_digest != statement_digest
            || pending.closure_digest != closure_digest
        {
            return Err(OfflineCashStateErrorV1::InvalidRecoveryMaterial);
        }
        ensure_no_commit_identity_indexes(
            &self.no_commit_recovery_ticket_ids,
            &self.no_commit_cancellation_ticket_ids,
            &statement,
        )?;
        if entry.consumed_payment_digest.is_some()
            || entry.consumed_amount.is_some()
            || entry.slot_released
        {
            return Err(OfflineCashStateErrorV1::InvalidAcceptanceTicket);
        }

        let tombstone = ClosedAcceptanceTicketNoCommitTombstoneV1 {
            acceptance_ticket_id: ticket_id,
            recovery_id: statement.recovery_id,
            cancellation_nullifier: statement.cancellation_nullifier,
            statement_digest,
            closure_digest,
        };
        if self.closed_no_commit_tombstones.contains_key(&ticket_id)
            || self.terminal_retry_order.contains(&ticket_id)
        {
            return Err(OfflineCashStateErrorV1::StateInvariant);
        }
        let mut next_book = self.clone();
        let Some(_removed) = next_book.tickets.remove(&ticket_id) else {
            return Err(OfflineCashStateErrorV1::StateInvariant);
        };
        next_book
            .closed_no_commit_tombstones
            .insert(ticket_id, tombstone);
        let capacity_meters = acceptance_ticket_capacity_meters_v1(&next_book)?;
        if capacity_meters.committed_inbox_bytes > self.committed_inbox_bytes {
            return Err(OfflineCashStateErrorV1::StateInvariant);
        }
        next_book.committed_inbox_bytes = capacity_meters.committed_inbox_bytes;
        next_book.retained_metadata_bytes = capacity_meters.retained_metadata_bytes;
        next_book.reserved_terminal_metadata_bytes =
            capacity_meters.reserved_terminal_metadata_bytes;
        *self = next_book;
        Ok(AcceptanceTicketNoCommitClosureOutcomeV1::Closed)
    }

    /// Reserve one signed, capacity-backed ticket.
    ///
    /// A byte-identical recovery replay is idempotent. The intent-to-ticket decision and both
    /// identities remain permanent even after terminal compaction or authenticated no-commit
    /// closure; any differing reuse fails closed. Distinct tickets for the same request are
    /// admitted independently, subject only to exact-amount binding and physical inbox capacity.
    pub fn reserve(
        &mut self,
        verified_authorization: VerifiedOfflineCashAcceptanceIntentAuthorizationV1,
        ticket: OfflineCashAcceptanceTicketV1,
    ) -> Result<AcceptanceTicketReservationOutcomeV1, OfflineCashStateErrorV1> {
        let VerifiedOfflineCashAcceptanceIntentAuthorizationV1 {
            request,
            intent,
            proof_envelope_digest: intent_authorization_digest,
        } = verified_authorization;
        ticket
            .validate_shape_against(&request, &intent)
            .map_err(|_| OfflineCashStateErrorV1::InvalidAcceptanceTicket)?;
        let ticket_digest = ticket
            .canonical_digest_against(&request, &intent)
            .map_err(|_| OfflineCashStateErrorV1::InvalidAcceptanceTicket)?;
        let request_digest = request
            .canonical_digest()
            .map_err(|_| OfflineCashStateErrorV1::InvalidPaymentRequest)?;
        let intent_digest = intent
            .canonical_digest_against(&request)
            .map_err(|_| OfflineCashStateErrorV1::InvalidAcceptanceTicket)?;
        let intent_id = intent.intent_id;
        if intent_authorization_digest == [0; 32] {
            return Err(OfflineCashStateErrorV1::InvalidAcceptanceIntentAuthorization);
        }
        if let Some(existing) = self.intent_ticket_decisions.get(&intent_id) {
            return if existing.request == request
                && existing.request_digest == request_digest
                && existing.intent == intent
                && existing.intent_digest == intent_digest
                && existing.intent_authorization_digest == intent_authorization_digest
                && existing.ticket == ticket
                && existing.ticket_digest == ticket_digest
            {
                if self.ticket_intent_ids.get(&ticket.acceptance_ticket_id) != Some(&intent_id) {
                    return Err(OfflineCashStateErrorV1::StateInvariant);
                }
                Ok(AcceptanceTicketReservationOutcomeV1::AlreadyReserved(
                    DurableAcceptanceTicketDecisionV1 {
                        ticket: existing.ticket.clone(),
                        ticket_digest: existing.ticket_digest,
                    },
                ))
            } else {
                Err(OfflineCashStateErrorV1::InvalidAcceptanceTicket)
            };
        }
        if self
            .ticket_intent_ids
            .contains_key(&ticket.acceptance_ticket_id)
            || self.tickets.contains_key(&ticket.acceptance_ticket_id)
            || self
                .closed_no_commit_tombstones
                .contains_key(&ticket.acceptance_ticket_id)
        {
            return Err(OfflineCashStateErrorV1::InvalidAcceptanceTicket);
        }

        let decision_record = DurableAcceptanceIntentTicketDecisionRecordV1 {
            request: request.clone(),
            request_digest,
            intent,
            intent_digest,
            intent_authorization_digest,
            ticket: ticket.clone(),
            ticket_digest,
        };
        let reserved_entry = ReservedAcceptanceTicketV1 {
            request: request.clone(),
            intent,
            intent_authorization_digest,
            ticket: ticket.clone(),
            ticket_digest,
            consumed_payment_digest: None,
            consumed_amount: None,
            slot_released: false,
            no_commit_recovery: None,
        };
        let mut next = self.clone();
        let decision = DurableAcceptanceTicketDecisionV1 {
            ticket: ticket.clone(),
            ticket_digest,
        };
        next.intent_ticket_decisions
            .insert(intent_id, decision_record);
        next.ticket_intent_ids
            .insert(ticket.acceptance_ticket_id, intent_id);
        next.tickets
            .insert(ticket.acceptance_ticket_id, reserved_entry);
        let capacity_meters = acceptance_ticket_capacity_meters_v1(&next)
            .map_err(|_| OfflineCashStateErrorV1::ReceiverCapacityExhausted)?;
        if capacity_meters.committed_inbox_bytes > next.total_inbox_bytes {
            return Err(OfflineCashStateErrorV1::ReceiverCapacityExhausted);
        }
        next.committed_inbox_bytes = capacity_meters.committed_inbox_bytes;
        next.retained_metadata_bytes = capacity_meters.retained_metadata_bytes;
        next.reserved_terminal_metadata_bytes = capacity_meters.reserved_terminal_metadata_bytes;
        *self = next;
        Ok(AcceptanceTicketReservationOutcomeV1::Reserved(decision))
    }

    /// Consume one reservation for a durably staged payment.
    ///
    /// The reserved bytes remain committed while the payment occupies the durable inbox. Folding
    /// later converts the slot through [`Self::receiver_snapshot_folded_successor`].
    pub fn consume(
        &mut self,
        request: &OfflineCashPaymentRequestV1,
        payment: &OfflineCashPaymentV1,
    ) -> Result<AcceptanceTicketUseOutcomeV1, OfflineCashStateErrorV1> {
        payment
            .validate_shape_against(request)
            .map_err(|_| OfflineCashStateErrorV1::InvalidPeerCredit)?;
        let ticket_id = payment.acceptance_ticket.acceptance_ticket_id;
        let payment_digest = payment
            .canonical_digest_against(request)
            .map_err(|_| OfflineCashStateErrorV1::InvalidPeerCredit)?;
        let entry = self
            .tickets
            .get(&ticket_id)
            .ok_or(OfflineCashStateErrorV1::InvalidAcceptanceTicket)?;
        if entry.request != *request
            || entry.intent != payment.acceptance_intent
            || entry.ticket != payment.acceptance_ticket
            || entry.no_commit_recovery.is_some()
        {
            return Err(OfflineCashStateErrorV1::InvalidAcceptanceTicket);
        }
        if let Some(existing) = entry.consumed_payment_digest {
            return if existing == payment_digest {
                Ok(AcceptanceTicketUseOutcomeV1::Duplicate)
            } else {
                Err(OfflineCashStateErrorV1::InvalidAcceptanceTicket)
            };
        }

        let amount = payment.statement.amount;
        let mut next = self.clone();
        let entry = next
            .tickets
            .get_mut(&ticket_id)
            .ok_or(OfflineCashStateErrorV1::StateInvariant)?;
        entry.consumed_payment_digest = Some(payment_digest);
        entry.consumed_amount = Some(amount);
        let capacity_meters = acceptance_ticket_capacity_meters_v1(&next)?;
        if capacity_meters.committed_inbox_bytes != self.committed_inbox_bytes {
            return Err(OfflineCashStateErrorV1::StateInvariant);
        }
        next.committed_inbox_bytes = capacity_meters.committed_inbox_bytes;
        next.retained_metadata_bytes = capacity_meters.retained_metadata_bytes;
        next.reserved_terminal_metadata_bytes = capacity_meters.reserved_terminal_metadata_bytes;
        *self = next;
        Ok(AcceptanceTicketUseOutcomeV1::Consumed)
    }

    /// Release physical inbox bytes after the bound credit has entered the authenticated replay
    /// tree and the byte-identical acknowledgement has been durably retained.
    #[cfg(test)]
    pub(crate) fn release_folded(
        &mut self,
        acceptance_ticket_id: DigestV1,
        expected_payment_digest: DigestV1,
    ) -> Result<(), OfflineCashStateErrorV1> {
        let mut next = self.clone();
        next.release_folded_unmetered(acceptance_ticket_id, expected_payment_digest)?;
        let capacity_meters = acceptance_ticket_capacity_meters_v1(&next)?;
        if capacity_meters.committed_inbox_bytes > self.committed_inbox_bytes {
            return Err(OfflineCashStateErrorV1::StateInvariant);
        }
        next.committed_inbox_bytes = capacity_meters.committed_inbox_bytes;
        next.retained_metadata_bytes = capacity_meters.retained_metadata_bytes;
        next.reserved_terminal_metadata_bytes = capacity_meters.reserved_terminal_metadata_bytes;
        *self = next;
        Ok(())
    }

    fn release_folded_unmetered(
        &mut self,
        acceptance_ticket_id: DigestV1,
        expected_payment_digest: DigestV1,
    ) -> Result<(), OfflineCashStateErrorV1> {
        let entry = self
            .tickets
            .get(&acceptance_ticket_id)
            .ok_or(OfflineCashStateErrorV1::InvalidAcceptanceTicket)?;
        if entry.consumed_payment_digest != Some(expected_payment_digest) {
            return Err(OfflineCashStateErrorV1::InvalidAcceptanceTicket);
        }
        if entry.slot_released {
            return Ok(());
        }
        self.tickets
            .get_mut(&acceptance_ticket_id)
            .ok_or(OfflineCashStateErrorV1::StateInvariant)?
            .slot_released = true;
        self.terminal_retry_order.push(acceptance_ticket_id);
        if self.terminal_retry_order.len() > TERMINAL_TICKET_RETRY_HORIZON_V1 {
            let compacted_ticket_id = self.terminal_retry_order.remove(0);
            let compacted = self
                .tickets
                .remove(&compacted_ticket_id)
                .ok_or(OfflineCashStateErrorV1::StateInvariant)?;
            let payment_digest = compacted
                .consumed_payment_digest
                .ok_or(OfflineCashStateErrorV1::StateInvariant)?;
            if !compacted.slot_released {
                return Err(OfflineCashStateErrorV1::StateInvariant);
            }
            self.terminal_decision_accumulator = canonical_sha256_digest(
                TERMINAL_DECISION_ACCUMULATOR_DOMAIN_V1,
                &TerminalDecisionAccumulatorStepV1 {
                    previous_accumulator: self.terminal_decision_accumulator,
                    previous_terminal_count: self.compacted_terminal_count,
                    acceptance_ticket_id: compacted_ticket_id,
                    ticket_digest: compacted.ticket_digest,
                    intent_authorization_digest: compacted.intent_authorization_digest,
                    payment_digest,
                    exact_amount: compacted.ticket.exact_amount,
                },
            )?;
            self.compacted_terminal_count = self
                .compacted_terminal_count
                .checked_add(1)
                .ok_or(OfflineCashStateErrorV1::ArithmeticOverflow)?;
        }
        Ok(())
    }

    pub(crate) fn validate_recovered_with_snapshot_usage(
        &self,
        live_snapshot_bytes: u64,
        retained_snapshot_bytes: u64,
    ) -> Result<(), OfflineCashStateErrorV1> {
        let mut expected_ticket_intent_ids = BTreeMap::<DigestV1, DigestV1>::new();
        for (intent_id, decision) in &self.intent_ticket_decisions {
            let request_digest = decision
                .request
                .canonical_digest()
                .map_err(|_| OfflineCashStateErrorV1::SnapshotIntegrity)?;
            let intent_digest = decision
                .intent
                .canonical_digest_against(&decision.request)
                .map_err(|_| OfflineCashStateErrorV1::SnapshotIntegrity)?;
            let ticket_digest = decision
                .ticket
                .canonical_digest_against(&decision.request, &decision.intent)
                .map_err(|_| OfflineCashStateErrorV1::SnapshotIntegrity)?;
            if *intent_id != decision.intent.intent_id
                || decision.request_digest != request_digest
                || decision.intent_digest != intent_digest
                || decision.intent_authorization_digest == [0; 32]
                || decision.ticket_digest != ticket_digest
                || expected_ticket_intent_ids
                    .insert(decision.ticket.acceptance_ticket_id, *intent_id)
                    .is_some()
            {
                return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
            }
        }
        if expected_ticket_intent_ids != self.ticket_intent_ids {
            return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
        }

        let mut expected_recovery_ticket_ids = BTreeMap::<DigestV1, DigestV1>::new();
        let mut expected_cancellation_ticket_ids = BTreeMap::<DigestV1, DigestV1>::new();
        for (ticket_id, entry) in &self.tickets {
            entry
                .ticket
                .validate_shape_against(&entry.request, &entry.intent)
                .map_err(|_| OfflineCashStateErrorV1::SnapshotIntegrity)?;
            let ticket_digest = entry
                .ticket
                .canonical_digest_against(&entry.request, &entry.intent)
                .map_err(|_| OfflineCashStateErrorV1::SnapshotIntegrity)?;
            let decision = self
                .intent_ticket_decisions
                .get(&entry.intent.intent_id)
                .ok_or(OfflineCashStateErrorV1::SnapshotIntegrity)?;
            if !acceptance_intent_ticket_decision_matches_entry(decision, entry) {
                return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
            }
            if let Some(recovery) = entry.no_commit_recovery {
                validate_no_commit_statement_against_ticket(&recovery.statement, entry)
                    .map_err(|_| OfflineCashStateErrorV1::SnapshotIntegrity)?;
                if recovery
                    .statement
                    .canonical_digest()
                    .map_err(|_| OfflineCashStateErrorV1::SnapshotIntegrity)?
                    != recovery.statement_digest
                    || recovery.closure_digest == [0; 32]
                    || expected_recovery_ticket_ids
                        .insert(recovery.statement.recovery_id, *ticket_id)
                        .is_some()
                    || expected_cancellation_ticket_ids
                        .insert(recovery.statement.cancellation_nullifier, *ticket_id)
                        .is_some()
                {
                    return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
                }
            }
            if *ticket_id != entry.ticket.acceptance_ticket_id
                || entry.ticket_digest != ticket_digest
                || entry.intent_authorization_digest == [0; 32]
                || entry.consumed_payment_digest.is_some() != entry.consumed_amount.is_some()
                || entry.no_commit_recovery.is_some() && entry.consumed_payment_digest.is_some()
                || entry.no_commit_recovery.is_some() && entry.slot_released
                || entry.no_commit_recovery.is_none()
                    && entry.slot_released
                    && entry.consumed_payment_digest.is_none()
            {
                return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
            }
            if entry
                .consumed_amount
                .is_some_and(|amount| entry.ticket.exact_amount != amount)
            {
                return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
            }
        }
        for (ticket_id, tombstone) in &self.closed_no_commit_tombstones {
            if *ticket_id != tombstone.acceptance_ticket_id
                || tombstone.recovery_id == [0; 32]
                || tombstone.cancellation_nullifier == [0; 32]
                || tombstone.statement_digest == [0; 32]
                || tombstone.closure_digest == [0; 32]
                || self.tickets.contains_key(ticket_id)
                || self.terminal_retry_order.contains(ticket_id)
                || !self.ticket_intent_ids.contains_key(ticket_id)
                || expected_recovery_ticket_ids
                    .insert(tombstone.recovery_id, *ticket_id)
                    .is_some()
                || expected_cancellation_ticket_ids
                    .insert(tombstone.cancellation_nullifier, *ticket_id)
                    .is_some()
            {
                return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
            }
        }
        let capacity_meters = acceptance_ticket_capacity_meters_v1(self)
            .map_err(|_| OfflineCashStateErrorV1::SnapshotIntegrity)?;
        if capacity_meters.committed_inbox_bytes != self.committed_inbox_bytes
            || capacity_meters.retained_metadata_bytes != self.retained_metadata_bytes
            || capacity_meters.reserved_terminal_metadata_bytes
                != self.reserved_terminal_metadata_bytes
            || capacity_meters.committed_inbox_bytes > self.total_inbox_bytes
            || self.receiver_snapshot_live_bytes != live_snapshot_bytes
            || self.receiver_snapshot_retained_bytes != retained_snapshot_bytes
            || live_snapshot_bytes > self.receiver_snapshot_live_capacity_bytes()?
            || expected_recovery_ticket_ids != self.no_commit_recovery_ticket_ids
            || expected_cancellation_ticket_ids != self.no_commit_cancellation_ticket_ids
            || self.terminal_retry_order.len() > TERMINAL_TICKET_RETRY_HORIZON_V1
            || self.compacted_terminal_count == 0 && self.terminal_decision_accumulator != [0; 32]
            || self.compacted_terminal_count != 0 && self.terminal_decision_accumulator == [0; 32]
        {
            return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
        }
        let mut retry_ids = BTreeMap::new();
        for ticket_id in &self.terminal_retry_order {
            let entry = self
                .tickets
                .get(ticket_id)
                .ok_or(OfflineCashStateErrorV1::SnapshotIntegrity)?;
            if !entry.slot_released || retry_ids.insert(*ticket_id, ()).is_some() {
                return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn validate_recovered(&self) -> Result<(), OfflineCashStateErrorV1> {
        self.validate_recovered_with_snapshot_usage(0, self.receiver_snapshot_retained_bytes)
    }
}

fn acceptance_intent_ticket_decision_matches_entry(
    decision: &DurableAcceptanceIntentTicketDecisionRecordV1,
    entry: &ReservedAcceptanceTicketV1,
) -> bool {
    decision.request == entry.request
        && decision.intent == entry.intent
        && decision.intent_authorization_digest == entry.intent_authorization_digest
        && decision.ticket == entry.ticket
        && decision.ticket_digest == entry.ticket_digest
}

fn reject_cross_ticket_no_commit_identity_reuse(
    recovery_ticket_ids: &BTreeMap<DigestV1, DigestV1>,
    cancellation_ticket_ids: &BTreeMap<DigestV1, DigestV1>,
    statement: &OfflineCashNoCommitClosureStatementV1,
) -> Result<(), OfflineCashStateErrorV1> {
    let ticket_id = statement.acceptance_ticket_id;
    if recovery_ticket_ids
        .get(&statement.recovery_id)
        .is_some_and(|existing| *existing != ticket_id)
        || cancellation_ticket_ids
            .get(&statement.cancellation_nullifier)
            .is_some_and(|existing| *existing != ticket_id)
    {
        return Err(OfflineCashStateErrorV1::InvalidRecoveryMaterial);
    }
    Ok(())
}

fn ensure_no_commit_identity_indexes(
    recovery_ticket_ids: &BTreeMap<DigestV1, DigestV1>,
    cancellation_ticket_ids: &BTreeMap<DigestV1, DigestV1>,
    statement: &OfflineCashNoCommitClosureStatementV1,
) -> Result<(), OfflineCashStateErrorV1> {
    if recovery_ticket_ids.get(&statement.recovery_id) != Some(&statement.acceptance_ticket_id)
        || cancellation_ticket_ids.get(&statement.cancellation_nullifier)
            != Some(&statement.acceptance_ticket_id)
    {
        return Err(OfflineCashStateErrorV1::StateInvariant);
    }
    Ok(())
}

fn validate_no_commit_statement_against_ticket(
    statement: &OfflineCashNoCommitClosureStatementV1,
    entry: &ReservedAcceptanceTicketV1,
) -> Result<(), OfflineCashStateErrorV1> {
    statement
        .validate_shape()
        .map_err(|_| OfflineCashStateErrorV1::InvalidRecoveryMaterial)?;
    let request_digest = entry
        .request
        .canonical_digest()
        .map_err(|_| OfflineCashStateErrorV1::InvalidRecoveryMaterial)?;
    let intent_digest = entry
        .intent
        .canonical_digest_against(&entry.request)
        .map_err(|_| OfflineCashStateErrorV1::InvalidRecoveryMaterial)?;
    if statement.release_id != entry.request.release_id
        || statement.suite_id != entry.request.hardware_credential.suite_id
        || statement.request_id != entry.request.request_id
        || statement.request_digest != request_digest
        || statement.acceptance_ticket_id != entry.ticket.acceptance_ticket_id
        || statement.ticket_digest != entry.ticket_digest
        || statement.intent_authorization_digest != entry.intent_authorization_digest
        || statement.intent_digest != intent_digest
        || statement.exact_amount != entry.intent.exact_amount
        || statement.exact_amount != entry.ticket.exact_amount
        || statement.sender_one_time_commitment != entry.intent.sender_one_time_commitment
    {
        return Err(OfflineCashStateErrorV1::InvalidRecoveryMaterial);
    }
    Ok(())
}

/// Result of reserving sender durable-outbox capacity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SenderOutboxReservationOutcomeV1 {
    /// A new one-use reservation was installed.
    Reserved,
    /// The byte-identical reservation already existed during recovery.
    AlreadyReserved,
}

/// Opaque authority to ask qualified hardware to commit one exact prepared predecessor.
///
/// The state machine issues this only after the reservation and prepare record have been installed
/// together. It is intentionally neither cloneable nor serializable; crash recovery reissues it
/// only from a validated canonical journal snapshot.
#[derive(Debug, PartialEq, Eq)]
pub struct OfflineCashOutgoingCommitCapabilityV1 {
    preparation_id: DigestV1,
    reservation_commitment: DigestV1,
    _non_clone_seal: std::cell::Cell<()>,
}

impl OfflineCashOutgoingCommitCapabilityV1 {
    pub(super) fn for_prepared(
        prepared: &PreparedOutgoingCandidateV1,
    ) -> Result<Self, OfflineCashStateErrorV1> {
        Ok(Self {
            preparation_id: prepared.preparation_id,
            reservation_commitment: prepared
                .outbox_reservation
                .canonical_commitment()
                .map_err(|_| OfflineCashStateErrorV1::InvalidCandidateStage)?,
            _non_clone_seal: std::cell::Cell::new(()),
        })
    }

    pub(super) fn authorizes(
        &self,
        prepared: &PreparedOutgoingCandidateV1,
    ) -> Result<(), OfflineCashStateErrorV1> {
        let expected = Self::for_prepared(prepared)?;
        if *self != expected {
            return Err(OfflineCashStateErrorV1::InvalidCandidateStage);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
struct SenderOutboxReservationRecordV1 {
    reservation: OfflineCashOutboxReservationV1,
    reservation_commitment: DigestV1,
    terminal_envelope_digest: Option<DigestV1>,
    released: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct SenderOutboxCapacityMetersV1 {
    committed_outbox_bytes: u64,
    retained_metadata_bytes: u64,
    reserved_terminal_metadata_bytes: u64,
}

/// Sender-owned physical capacity ledger for recoverable terminal operations.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct OfflineCashSenderOutboxCapacityV1 {
    total_outbox_bytes: u64,
    committed_outbox_bytes: u64,
    retained_metadata_bytes: u64,
    reserved_terminal_metadata_bytes: u64,
    reservations: BTreeMap<DigestV1, SenderOutboxReservationRecordV1>,
}

impl OfflineCashSenderOutboxCapacityV1 {
    /// Create an empty sender-capacity ledger.
    #[must_use]
    pub const fn new(total_outbox_bytes: u64) -> Self {
        Self {
            total_outbox_bytes,
            committed_outbox_bytes: 0,
            retained_metadata_bytes: 0,
            reserved_terminal_metadata_bytes: 0,
            reservations: BTreeMap::new(),
        }
    }

    /// Return bytes still available for new terminal operations.
    #[must_use]
    pub const fn available_outbox_bytes(&self) -> u64 {
        self.total_outbox_bytes
            .saturating_sub(self.committed_outbox_bytes)
    }

    /// Return the physical outbox capacity governed by this durable ledger.
    #[must_use]
    pub const fn total_outbox_bytes(&self) -> u64 {
        self.total_outbox_bytes
    }

    /// Return bytes committed to live slots, permanent metadata, and terminal headroom.
    #[must_use]
    pub const fn committed_outbox_bytes(&self) -> u64 {
        self.committed_outbox_bytes
    }

    /// Return exact canonical bytes retained by reservation and release records.
    #[must_use]
    pub const fn retained_metadata_bytes(&self) -> u64 {
        self.retained_metadata_bytes
    }

    /// Return pre-reserved bytes that guarantee every live operation can become terminal.
    #[must_use]
    pub const fn reserved_terminal_metadata_bytes(&self) -> u64 {
        self.reserved_terminal_metadata_bytes
    }

    /// Reserve complete recovery, proof, certificate, final-envelope, and retry capacity before
    /// the hardware predecessor is locked.
    pub fn reserve(
        &mut self,
        reservation: OfflineCashOutboxReservationV1,
        journal: &OfflineCashOutgoingCandidateJournalV1,
    ) -> Result<SenderOutboxReservationOutcomeV1, OfflineCashStateErrorV1> {
        let commitment = reservation
            .canonical_commitment()
            .map_err(|_| OfflineCashStateErrorV1::SenderOutboxCapacityExhausted)?;
        if let Some(existing) = self.reservations.get(&reservation.reservation_id) {
            if existing.reservation_commitment != commitment
                || existing.terminal_envelope_digest.is_some()
                || existing.released
            {
                return Err(OfflineCashStateErrorV1::CandidateConflict);
            }
            self.validate_capacity_meters(journal, false)?;
            return Ok(SenderOutboxReservationOutcomeV1::AlreadyReserved);
        }
        let mut next = self.clone();
        next.reservations.insert(
            reservation.reservation_id,
            SenderOutboxReservationRecordV1 {
                reservation,
                reservation_commitment: commitment,
                terminal_envelope_digest: None,
                released: false,
            },
        );
        next.reconcile_capacity_meters(journal)?;
        *self = next;
        Ok(SenderOutboxReservationOutcomeV1::Reserved)
    }

    pub(super) fn require_reservation(
        &self,
        reservation: OfflineCashOutboxReservationV1,
    ) -> Result<DigestV1, OfflineCashStateErrorV1> {
        let commitment = reservation
            .canonical_commitment()
            .map_err(|_| OfflineCashStateErrorV1::SenderOutboxCapacityExhausted)?;
        let existing = self
            .reservations
            .get(&reservation.reservation_id)
            .ok_or(OfflineCashStateErrorV1::SenderOutboxCapacityExhausted)?;
        if existing.reservation_commitment != commitment
            || existing.terminal_envelope_digest.is_some()
            || existing.released
        {
            return Err(OfflineCashStateErrorV1::CandidateConflict);
        }
        Ok(commitment)
    }

    fn bind_terminal_envelope(
        &mut self,
        reservation: OfflineCashOutboxReservationV1,
        envelope_digest: DigestV1,
    ) -> Result<(), OfflineCashStateErrorV1> {
        let commitment = reservation
            .canonical_commitment()
            .map_err(|_| OfflineCashStateErrorV1::SenderOutboxCapacityExhausted)?;
        let record = self
            .reservations
            .get_mut(&reservation.reservation_id)
            .ok_or(OfflineCashStateErrorV1::SenderOutboxCapacityExhausted)?;
        if record.reservation_commitment != commitment || record.released {
            return Err(OfflineCashStateErrorV1::CandidateConflict);
        }
        match record.terminal_envelope_digest {
            None => record.terminal_envelope_digest = Some(envelope_digest),
            Some(existing) if existing == envelope_digest => {}
            Some(_) => return Err(OfflineCashStateErrorV1::CandidateConflict),
        }
        Ok(())
    }

    fn mark_terminal_released(
        &mut self,
        reservation_id: DigestV1,
        expected_envelope_digest: DigestV1,
    ) -> Result<(), OfflineCashStateErrorV1> {
        let record = self
            .reservations
            .get_mut(&reservation_id)
            .ok_or(OfflineCashStateErrorV1::InvalidCandidateStage)?;
        if record.terminal_envelope_digest != Some(expected_envelope_digest) {
            return Err(OfflineCashStateErrorV1::CandidateConflict);
        }
        if record.released {
            return Ok(());
        }
        record.released = true;
        Ok(())
    }

    fn validate_reservation_records(&self) -> Result<(), OfflineCashStateErrorV1> {
        for (reservation_id, record) in &self.reservations {
            let commitment = record
                .reservation
                .canonical_commitment()
                .map_err(|_| OfflineCashStateErrorV1::SnapshotIntegrity)?;
            if *reservation_id != record.reservation.reservation_id
                || record.reservation_commitment != commitment
                || record.released && record.terminal_envelope_digest.is_none()
            {
                return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
            }
        }
        Ok(())
    }

    fn reconcile_capacity_meters(
        &mut self,
        journal: &OfflineCashOutgoingCandidateJournalV1,
    ) -> Result<(), OfflineCashStateErrorV1> {
        let meters = sender_outbox_capacity_meters_v1(self, journal)?;
        if meters.committed_outbox_bytes > self.total_outbox_bytes {
            return Err(OfflineCashStateErrorV1::SenderOutboxCapacityExhausted);
        }
        self.committed_outbox_bytes = meters.committed_outbox_bytes;
        self.retained_metadata_bytes = meters.retained_metadata_bytes;
        self.reserved_terminal_metadata_bytes = meters.reserved_terminal_metadata_bytes;
        Ok(())
    }

    fn reconcile_terminal_capacity_meters(
        &mut self,
        journal: &OfflineCashOutgoingCandidateJournalV1,
        precommitted_outbox_bytes: u64,
    ) -> Result<(), OfflineCashStateErrorV1> {
        let meters = sender_outbox_capacity_meters_v1(self, journal)?;
        if meters.committed_outbox_bytes > precommitted_outbox_bytes
            || meters.committed_outbox_bytes > self.total_outbox_bytes
        {
            return Err(OfflineCashStateErrorV1::StateInvariant);
        }
        self.committed_outbox_bytes = meters.committed_outbox_bytes;
        self.retained_metadata_bytes = meters.retained_metadata_bytes;
        self.reserved_terminal_metadata_bytes = meters.reserved_terminal_metadata_bytes;
        Ok(())
    }

    fn validate_capacity_meters(
        &self,
        journal: &OfflineCashOutgoingCandidateJournalV1,
        snapshot: bool,
    ) -> Result<(), OfflineCashStateErrorV1> {
        let meters = sender_outbox_capacity_meters_v1(self, journal).map_err(|error| {
            if snapshot {
                OfflineCashStateErrorV1::SnapshotIntegrity
            } else {
                error
            }
        })?;
        if meters.committed_outbox_bytes != self.committed_outbox_bytes
            || meters.retained_metadata_bytes != self.retained_metadata_bytes
            || meters.reserved_terminal_metadata_bytes != self.reserved_terminal_metadata_bytes
            || meters.committed_outbox_bytes > self.total_outbox_bytes
        {
            return Err(if snapshot {
                OfflineCashStateErrorV1::SnapshotIntegrity
            } else {
                OfflineCashStateErrorV1::StateInvariant
            });
        }
        Ok(())
    }

    pub(crate) fn validate_recovered(
        &self,
        journal: &OfflineCashOutgoingCandidateJournalV1,
    ) -> Result<(), OfflineCashStateErrorV1> {
        self.validate_reservation_records()?;
        self.validate_capacity_meters(journal, true)
    }
}

/// Sender inputs already sealed by qualified hardware before recursive proving starts.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct PreparedSendMaterialV1 {
    /// Exact state-transition statement generated by the aggregate state machine.
    pub proof_statement: TransitionProofStatementV1,
    /// Common state-proof transport digest reconstructed before proving.
    pub transport_semantic_digest: DigestV1,
    /// Exact signed reusable receiver request.
    pub request: OfflineCashPaymentRequestV1,
    /// Sender-selected exact amount and unlinkable one-use predecessor commitment.
    pub acceptance_intent: OfflineCashAcceptanceIntentV1,
    /// Exact one-use receiver inbox reservation.
    pub acceptance_ticket: OfflineCashAcceptanceTicketV1,
    /// Proof-derived unlinkable transition nullifier.
    pub transition_nullifier: DigestV1,
    /// Amount-bound encrypted-credit commitment.
    pub ciphertext_commitment: DigestV1,
    /// Recipient-only encrypted credit opening.
    pub encrypted_credit: Vec<u8>,
    /// Trusted-time or secure monotonic-lease evidence selected during prepare.
    pub commit_evidence: OfflineCashCommitEvidenceV1,
    /// One-use reservation for every recoverable sender artifact.
    pub outbox_reservation: OfflineCashOutboxReservationV1,
    /// Hardware-sealed transition inputs. Raw secrets are never accepted here.
    pub sealed_transition_inputs: Vec<u8>,
    /// Hardware-sealed deterministic proof/envelope recovery seeds.
    pub sealed_recovery_seeds: Vec<u8>,
    /// Digest of the normalized GuardBundle statement used by the private candidate proof.
    pub normalized_guard_statement_digest: DigestV1,
}

/// Redeemer inputs already sealed by qualified hardware before recursive proving starts.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct PreparedRedemptionMaterialV1 {
    /// Exact state-transition statement generated by the aggregate state machine.
    pub proof_statement: TransitionProofStatementV1,
    /// Common state-proof transport digest reconstructed before proving.
    pub transport_semantic_digest: DigestV1,
    /// Positive amount converted back to an online claim.
    pub amount: u128,
    /// Account credited by successful settlement.
    pub beneficiary: iroha_data_model::account::AccountId,
    /// Proof-derived unlinkable terminal nullifier.
    pub terminal_nullifier: DigestV1,
    /// Commitment to the public claim and private proof output.
    pub redemption_commitment: DigestV1,
    /// Trusted-time or secure monotonic-lease evidence selected during prepare.
    pub commit_evidence: OfflineCashCommitEvidenceV1,
    /// One-use reservation for every recoverable sender artifact.
    pub outbox_reservation: OfflineCashOutboxReservationV1,
    /// Hardware-sealed transition inputs. Raw secrets are never accepted here.
    pub sealed_transition_inputs: Vec<u8>,
    /// Hardware-sealed deterministic proof/envelope recovery seeds.
    pub sealed_recovery_seeds: Vec<u8>,
    /// Digest of the normalized GuardBundle statement used by the private candidate proof.
    pub normalized_guard_statement_digest: DigestV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct PreparedSendPublicProjectionV1 {
    request: OfflineCashPaymentRequestV1,
    acceptance_intent: OfflineCashAcceptanceIntentV1,
    acceptance_ticket: OfflineCashAcceptanceTicketV1,
    statement: OfflineCashTransferStatementV1,
    encrypted_credit: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
enum PreparedPublicProjectionV1 {
    Send(Box<PreparedSendPublicProjectionV1>),
    Redemption {
        statement: OfflineCashRedemptionStatementV1,
    },
}

impl PreparedPublicProjectionV1 {
    fn lifecycle(&self) -> &OfflineCashLifecycleBindingV1 {
        match self {
            Self::Send(projection) => &projection.statement.lifecycle,
            Self::Redemption { statement } => &statement.lifecycle,
        }
    }

    fn semantic_digest(&self) -> Result<DigestV1, OfflineCashStateErrorV1> {
        match self {
            Self::Send(projection) => projection
                .statement
                .canonical_digest()
                .map_err(|_| OfflineCashStateErrorV1::InvalidPeerCredit),
            Self::Redemption { statement } => statement
                .canonical_digest()
                .map_err(|_| OfflineCashStateErrorV1::InvalidRedemption),
        }
    }

    const fn transition_nullifier(&self) -> DigestV1 {
        match self {
            Self::Send(projection) => projection.statement.transition_nullifier,
            Self::Redemption { statement } => statement.terminal_nullifier,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode)]
struct PreparationIdPreimageV1 {
    predecessor_state: OfflineCashStateV1,
    successor_state: OfflineCashStateV1,
    state_transition_digest: DigestV1,
    proof_statement: TransitionProofStatementV1,
    transport_semantic_digest: DigestV1,
    projection: PreparedPublicProjectionV1,
    outbox_reservation: OfflineCashOutboxReservationV1,
    sealed_transition_inputs: Vec<u8>,
    sealed_recovery_seeds: Vec<u8>,
    normalized_guard_statement_digest: DigestV1,
}

/// Durable sender-local prepare record.
///
/// Both aggregate heads are intentionally private and never copied into payment, redemption, ACK,
/// or receiver-verification inputs.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct PreparedOutgoingCandidateV1 {
    /// Wire/state version.
    pub version: u16,
    /// Canonical identity of the exact sealed preparation.
    pub preparation_id: DigestV1,
    /// Private aggregate predecessor consumed by the candidate proof.
    pub(crate) predecessor_state: OfflineCashStateV1,
    /// Private aggregate successor installed only after wrapper persistence.
    pub(crate) successor_state: OfflineCashStateV1,
    /// Digest of the exact private recursive state-transition statement.
    pub state_transition_digest: DigestV1,
    /// Exact verifier-reconstructed transition statement retained for crash recovery.
    pub proof_statement: TransitionProofStatementV1,
    /// Exact semantic digest constrained by the paired aggregate-state proof.
    pub transport_semantic_digest: DigestV1,
    projection: PreparedPublicProjectionV1,
    /// One-use reservation that covers the entire recoverable terminal workflow.
    pub outbox_reservation: OfflineCashOutboxReservationV1,
    /// Hardware-sealed transition inputs.
    pub sealed_transition_inputs: Vec<u8>,
    /// Hardware-sealed deterministic recovery seeds.
    pub sealed_recovery_seeds: Vec<u8>,
    /// Exact normalized hardware relation consumed by the private proof.
    pub normalized_guard_statement_digest: DigestV1,
}

impl PreparedOutgoingCandidateV1 {
    /// Build and durably reserve one sender payment before any predecessor can be committed.
    pub fn send(
        predecessor_state: OfflineCashStateV1,
        successor_state: OfflineCashStateV1,
        state_transition_digest: DigestV1,
        material: PreparedSendMaterialV1,
    ) -> Result<Self, OfflineCashStateErrorV1> {
        validate_private_state_link(
            &predecessor_state,
            &successor_state,
            material.acceptance_intent.exact_amount,
            OfflineCashOperationKindV1::SendSplit,
        )?;
        validate_recovery_material(
            &material.sealed_transition_inputs,
            &material.sealed_recovery_seeds,
            material.normalized_guard_statement_digest,
        )?;
        material
            .request
            .validate_shape()
            .map_err(|_| OfflineCashStateErrorV1::InvalidPaymentRequest)?;
        material
            .acceptance_ticket
            .validate_shape_against(&material.request, &material.acceptance_intent)
            .map_err(|_| OfflineCashStateErrorV1::InvalidAcceptanceTicket)?;
        material
            .acceptance_intent
            .validate_shape_against(&material.request)
            .map_err(|_| OfflineCashStateErrorV1::InvalidAcceptanceTicket)?;
        material
            .commit_evidence
            .validate()
            .map_err(|_| OfflineCashStateErrorV1::InvalidPaymentRequest)?;
        if state_transition_digest == [0; 32]
            || material.transition_nullifier == [0; 32]
            || material.ciphertext_commitment == [0; 32]
            || material.encrypted_credit.is_empty()
            || material.encrypted_credit.len() > OFFLINE_CASH_ENCRYPTED_CREDIT_MAX_BYTES_V1
            || material.acceptance_ticket.exact_amount != material.acceptance_intent.exact_amount
            || material.outbox_reservation.operation_kind != OfflineCashOperationKindV1::SendSplit
        {
            return Err(OfflineCashStateErrorV1::InvalidPaymentRequest);
        }
        validate_request_against_state(&material.request, &predecessor_state)?;

        let request_digest = material
            .request
            .canonical_digest()
            .map_err(|_| OfflineCashStateErrorV1::InvalidPaymentRequest)?;
        let ticket_digest = material
            .acceptance_ticket
            .canonical_digest_against(&material.request, &material.acceptance_intent)
            .map_err(|_| OfflineCashStateErrorV1::InvalidAcceptanceTicket)?;
        let ciphertext_digest =
            digest_raw_bytes(CIPHERTEXT_DIGEST_DOMAIN_V1, &material.encrypted_credit);
        let lifecycle = OfflineCashLifecycleBindingV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            network_id: predecessor_state.lane.network_id,
            protocol_version: predecessor_state.protocol_version,
            suite_id: predecessor_state.suite_id,
            vk_digest: predecessor_state.vk_digest,
            release_id: predecessor_state.release_id,
            asset: predecessor_state.lane.asset.clone(),
            asset_incarnation: predecessor_state.asset_incarnation,
            scale: predecessor_state.lane.scale,
            liability_pool_id: predecessor_state.liability_pool_id,
            hardware_profile_id: predecessor_state.hardware_profile_id,
            policy_epoch: predecessor_state.policy_epoch,
            operation_kind: OfflineCashOperationKindV1::SendSplit,
            request_id: material.request.request_id,
            acceptance_ticket_id: material.acceptance_ticket.acceptance_ticket_id,
            credit_id: [1; 32],
            ciphertext_digest,
        };
        let statement = OfflineCashTransferStatementV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            lifecycle,
            amount: material.acceptance_intent.exact_amount,
            transition_nullifier: material.transition_nullifier,
            request_digest,
            acceptance_ticket_digest: ticket_digest,
            recipient_one_time_key: material.acceptance_ticket.recipient_one_time_key,
            ciphertext_commitment: material.ciphertext_commitment,
            commit_evidence: material.commit_evidence,
        }
        .seal_credit_id()
        .map_err(|_| OfflineCashStateErrorV1::InvalidPeerCredit)?;
        statement
            .validate()
            .map_err(|_| OfflineCashStateErrorV1::InvalidPeerCredit)?;
        Self::new(
            predecessor_state,
            successor_state,
            state_transition_digest,
            material.proof_statement,
            material.transport_semantic_digest,
            PreparedPublicProjectionV1::Send(Box::new(PreparedSendPublicProjectionV1 {
                request: material.request,
                acceptance_intent: material.acceptance_intent,
                acceptance_ticket: material.acceptance_ticket,
                statement,
                encrypted_credit: material.encrypted_credit,
            })),
            material.outbox_reservation,
            material.sealed_transition_inputs,
            material.sealed_recovery_seeds,
            material.normalized_guard_statement_digest,
        )
    }

    /// Build and durably reserve one partial or full redemption before predecessor commit.
    pub fn redemption(
        predecessor_state: OfflineCashStateV1,
        successor_state: OfflineCashStateV1,
        state_transition_digest: DigestV1,
        material: PreparedRedemptionMaterialV1,
    ) -> Result<Self, OfflineCashStateErrorV1> {
        validate_private_state_link(
            &predecessor_state,
            &successor_state,
            material.amount,
            OfflineCashOperationKindV1::RedeemSplit,
        )?;
        validate_recovery_material(
            &material.sealed_transition_inputs,
            &material.sealed_recovery_seeds,
            material.normalized_guard_statement_digest,
        )?;
        material
            .commit_evidence
            .validate()
            .map_err(|_| OfflineCashStateErrorV1::InvalidRedemption)?;
        if state_transition_digest == [0; 32]
            || material.amount == 0
            || material.terminal_nullifier == [0; 32]
            || material.redemption_commitment == [0; 32]
            || material.terminal_nullifier == material.redemption_commitment
            || material.outbox_reservation.operation_kind != OfflineCashOperationKindV1::RedeemSplit
        {
            return Err(OfflineCashStateErrorV1::InvalidRedemption);
        }
        let lifecycle = OfflineCashLifecycleBindingV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            network_id: predecessor_state.lane.network_id,
            protocol_version: predecessor_state.protocol_version,
            suite_id: predecessor_state.suite_id,
            vk_digest: predecessor_state.vk_digest,
            release_id: predecessor_state.release_id,
            asset: predecessor_state.lane.asset.clone(),
            asset_incarnation: predecessor_state.asset_incarnation,
            scale: predecessor_state.lane.scale,
            liability_pool_id: predecessor_state.liability_pool_id,
            hardware_profile_id: predecessor_state.hardware_profile_id,
            policy_epoch: predecessor_state.policy_epoch,
            operation_kind: OfflineCashOperationKindV1::RedeemSplit,
            request_id: [0; 32],
            acceptance_ticket_id: [0; 32],
            credit_id: [0; 32],
            ciphertext_digest: [0; 32],
        };
        let statement = OfflineCashRedemptionStatementV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            lifecycle,
            amount: material.amount,
            beneficiary: material.beneficiary,
            terminal_nullifier: material.terminal_nullifier,
            redemption_commitment: material.redemption_commitment,
            redemption_id: [1; 32],
            commit_evidence: material.commit_evidence,
        }
        .seal_redemption_id()
        .map_err(|_| OfflineCashStateErrorV1::InvalidRedemption)?;
        statement
            .validate_shape()
            .map_err(|_| OfflineCashStateErrorV1::InvalidRedemption)?;
        Self::new(
            predecessor_state,
            successor_state,
            state_transition_digest,
            material.proof_statement,
            material.transport_semantic_digest,
            PreparedPublicProjectionV1::Redemption { statement },
            material.outbox_reservation,
            material.sealed_transition_inputs,
            material.sealed_recovery_seeds,
            material.normalized_guard_statement_digest,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new(
        predecessor_state: OfflineCashStateV1,
        successor_state: OfflineCashStateV1,
        state_transition_digest: DigestV1,
        proof_statement: TransitionProofStatementV1,
        transport_semantic_digest: DigestV1,
        projection: PreparedPublicProjectionV1,
        outbox_reservation: OfflineCashOutboxReservationV1,
        sealed_transition_inputs: Vec<u8>,
        sealed_recovery_seeds: Vec<u8>,
        normalized_guard_statement_digest: DigestV1,
    ) -> Result<Self, OfflineCashStateErrorV1> {
        validate_prepared_transition_statement(
            &predecessor_state,
            &successor_state,
            state_transition_digest,
            &proof_statement,
            transport_semantic_digest,
            &projection,
            normalized_guard_statement_digest,
        )?;
        let preparation_id = canonical_sha256_digest(
            PREPARATION_ID_DOMAIN_V1,
            &PreparationIdPreimageV1 {
                predecessor_state: predecessor_state.clone(),
                successor_state: successor_state.clone(),
                state_transition_digest,
                proof_statement: proof_statement.clone(),
                transport_semantic_digest,
                projection: projection.clone(),
                outbox_reservation,
                sealed_transition_inputs: sealed_transition_inputs.clone(),
                sealed_recovery_seeds: sealed_recovery_seeds.clone(),
                normalized_guard_statement_digest,
            },
        )?;
        let prepared = Self {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            preparation_id,
            predecessor_state,
            successor_state,
            state_transition_digest,
            proof_statement,
            transport_semantic_digest,
            projection,
            outbox_reservation,
            sealed_transition_inputs,
            sealed_recovery_seeds,
            normalized_guard_statement_digest,
        };
        let encoded_len = norito::encode_canonical(&prepared)
            .map_err(|_| OfflineCashStateErrorV1::CanonicalEncoding)?
            .len();
        if u64::try_from(encoded_len).map_err(|_| OfflineCashStateErrorV1::ArithmeticOverflow)?
            > PREPARED_OUTGOING_CANDIDATE_MAX_BYTES_V1
        {
            return Err(OfflineCashStateErrorV1::InvalidRecoveryMaterial);
        }
        Ok(prepared)
    }

    /// Return the public lifecycle bound by the private candidate.
    #[must_use]
    pub fn lifecycle(&self) -> &OfflineCashLifecycleBindingV1 {
        self.projection.lifecycle()
    }

    /// Return the proof-derived transition/terminal nullifier.
    #[must_use]
    pub const fn transition_nullifier(&self) -> DigestV1 {
        self.projection.transition_nullifier()
    }

    /// Return the semantic digest proven by the private candidate.
    pub fn semantic_digest(&self) -> Result<DigestV1, OfflineCashStateErrorV1> {
        self.projection.semantic_digest()
    }

    /// Borrow sender-local aggregate heads for the local wrapper prover only.
    #[must_use]
    pub(crate) fn private_state_link(&self) -> (&OfflineCashStateV1, &OfflineCashStateV1) {
        (&self.predecessor_state, &self.successor_state)
    }

    /// Borrow the payment-only ticket for sender-local wrapper witness construction.
    #[must_use]
    pub(crate) fn acceptance_ticket(&self) -> Option<&OfflineCashAcceptanceTicketV1> {
        match &self.projection {
            PreparedPublicProjectionV1::Send(projection) => Some(&projection.acceptance_ticket),
            PreparedPublicProjectionV1::Redemption { .. } => None,
        }
    }

    /// Borrow the payment-only sender intent for local terminal-witness construction.
    #[must_use]
    pub(crate) fn acceptance_intent(&self) -> Option<&OfflineCashAcceptanceIntentV1> {
        match &self.projection {
            PreparedPublicProjectionV1::Send(projection) => Some(&projection.acceptance_intent),
            PreparedPublicProjectionV1::Redemption { .. } => None,
        }
    }

    /// Return the payment-only one-use sender commitment, or canonical zero for redemption.
    #[must_use]
    pub(crate) fn sender_one_time_commitment(&self) -> DigestV1 {
        self.acceptance_intent()
            .map_or([0; 32], |intent| intent.sender_one_time_commitment)
    }

    pub(crate) fn candidate_public_inputs(
        &self,
        artifacts: OfflineCashRecursionArtifactsV1,
        proof: &OfflineCashPairedProofV1,
    ) -> Result<OfflineCashStateRelationPublicInputsV1, String> {
        let statement = &self.proof_statement;
        Ok(OfflineCashStateRelationPublicInputsV1 {
            operation: statement.kind.into(),
            predecessor: Some(self.predecessor_state.clone()),
            successor: self.successor_state.clone(),
            amount: statement.amount,
            journal_revision_before: statement.journal_revision_before,
            journal_revision_after: statement.journal_revision_after,
            transition_effect_digest: statement.effect_digest,
            mint_finality_semantic_digest: statement.mint_finality_semantic_digest,
            mint_finality_proof_binding_digest: statement.mint_finality_proof_binding_digest,
            peer_credit_id: statement.peer_credit_id,
            peer_recipient_lane_id: statement.peer_recipient_lane_id,
            lifecycle_binding_digest: statement.lifecycle_binding_digest,
            precommit_binding_digest: statement.precommit_binding_digest,
            suite_upgrade_authorization_digest: statement.suite_upgrade_authorization_digest,
            transport_semantic_digest: self.transport_semantic_digest,
            guard_statement_digest: self.normalized_guard_statement_digest,
            eq_protocol_digest: artifacts.eq_protocol_digest,
            ep_protocol_digest: artifacts.ep_protocol_digest,
            guard_eq_protocol_digest: artifacts
                .guard_bundle_protocol_digest(OfflineCashPastaParityV1::Eq)
                .map_err(|error| error.to_string())?,
            guard_ep_protocol_digest: artifacts
                .guard_bundle_protocol_digest(OfflineCashPastaParityV1::Ep)
                .map_err(|error| error.to_string())?,
            mint_eq_protocol_digest: artifacts
                .mint_finality_protocol_digest(OfflineCashPastaParityV1::Eq)
                .map_err(|error| error.to_string())?,
            mint_ep_protocol_digest: artifacts
                .mint_finality_protocol_digest(OfflineCashPastaParityV1::Ep)
                .map_err(|error| error.to_string())?,
            guard_eq_credential_audit: proof.guard_eq_credential_audit,
            guard_ep_credential_audit: proof.guard_ep_credential_audit,
            eq_deferred_audit: proof.eq_deferred_audit,
            ep_deferred_audit: proof.ep_deferred_audit,
        })
    }

    pub(super) fn validate_recovered(&self) -> Result<(), OfflineCashStateErrorV1> {
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1 {
            return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
        }
        let operation = self.projection.lifecycle().operation_kind;
        validate_private_state_link(
            &self.predecessor_state,
            &self.successor_state,
            self.proof_statement.amount,
            operation,
        )?;
        validate_recovery_material(
            &self.sealed_transition_inputs,
            &self.sealed_recovery_seeds,
            self.normalized_guard_statement_digest,
        )?;
        validate_recovered_projection(
            &self.projection,
            &self.predecessor_state,
            self.outbox_reservation,
        )?;
        let reconstructed = Self::new(
            self.predecessor_state.clone(),
            self.successor_state.clone(),
            self.state_transition_digest,
            self.proof_statement.clone(),
            self.transport_semantic_digest,
            self.projection.clone(),
            self.outbox_reservation,
            self.sealed_transition_inputs.clone(),
            self.sealed_recovery_seeds.clone(),
            self.normalized_guard_statement_digest,
        )?;
        if reconstructed != *self {
            return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }
}

/// Fail-closed verifier seam for the private prepared transition proof.
pub trait OfflineCashCandidateProofVerifierV1 {
    /// Verify that `proof` consumes the sealed private predecessor, creates the exact private
    /// successor, and exposes only the candidate's semantic projection.
    fn verify_candidate_proof(
        &self,
        candidate: &PreparedOutgoingCandidateV1,
        proof: &OfflineCashPairedProofV1,
    ) -> Result<(), String>;
}

/// Prepared transition plus the locally verified proof durably persisted before hardware commit.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct PersistedOutgoingCandidateV1 {
    /// Immutable prepared inputs and private state link.
    pub prepared: PreparedOutgoingCandidateV1,
    /// Paired recursive proof of the prepared aggregate transition.
    pub candidate_proof: OfflineCashPairedProofV1,
    /// Digest hardware must bind into its atomic terminal certificate.
    pub candidate_envelope_digest: DigestV1,
}

impl PersistedOutgoingCandidateV1 {
    /// Verify and persist one prepared proof without consuming the predecessor.
    pub fn verify_and_persist<V: OfflineCashCandidateProofVerifierV1>(
        prepared: PreparedOutgoingCandidateV1,
        candidate_proof: OfflineCashPairedProofV1,
        verifier: &V,
    ) -> Result<Self, OfflineCashStateErrorV1> {
        let semantic_digest = prepared.transport_semantic_digest;
        candidate_proof
            .validate_shape_for_semantic_digest(semantic_digest)
            .map_err(|_| OfflineCashStateErrorV1::InvalidProofBundle)?;
        verifier
            .verify_candidate_proof(&prepared, &candidate_proof)
            .map_err(OfflineCashStateErrorV1::ProofRejected)?;
        let candidate_envelope_digest = canonical_sha256_digest(
            CANDIDATE_ENVELOPE_DOMAIN_V1,
            &(prepared.clone(), candidate_proof.clone()),
        )?;
        if candidate_envelope_digest == [0; 32] {
            return Err(OfflineCashStateErrorV1::InvalidCandidateStage);
        }
        Ok(Self {
            prepared,
            candidate_proof,
            candidate_envelope_digest,
        })
    }

    fn validate_recovered<R: OfflineCashRecursiveVerifierV1>(
        &self,
        artifacts: OfflineCashRecursionArtifactsV1,
        verifier: &R,
    ) -> Result<(), OfflineCashStateErrorV1> {
        self.prepared.validate_recovered()?;
        self.candidate_proof
            .validate_shape_for_semantic_digest(self.prepared.transport_semantic_digest)
            .map_err(|_| OfflineCashStateErrorV1::SnapshotIntegrity)?;
        let public_inputs = self
            .prepared
            .candidate_public_inputs(artifacts, &self.candidate_proof)
            .map_err(|_| OfflineCashStateErrorV1::SnapshotIntegrity)?;
        verify_offline_cash_state_proof_v1(
            verifier,
            artifacts,
            &public_inputs,
            &self.candidate_proof,
        )
        .map_err(|_| OfflineCashStateErrorV1::SnapshotIntegrity)?;
        let candidate_envelope_digest = canonical_sha256_digest(
            CANDIDATE_ENVELOPE_DOMAIN_V1,
            &(self.prepared.clone(), self.candidate_proof.clone()),
        )?;
        if candidate_envelope_digest == [0; 32]
            || candidate_envelope_digest != self.candidate_envelope_digest
        {
            return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }
}

/// Locally verified candidate plus the exact-once terminal certificate recovered from hardware.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct CommittedOutgoingCandidateV1 {
    /// Candidate durably persisted before commit.
    pub candidate: PersistedOutgoingCandidateV1,
    /// Terminal hardware certificate for that exact candidate digest.
    pub commit_certificate: OfflineCashCommitCertificateV1,
    /// Canonical certificate digest constrained by the wrapper.
    pub commit_certificate_digest: DigestV1,
}

impl CommittedOutgoingCandidateV1 {
    /// Bind a recovered terminal certificate to the exact persisted candidate.
    pub fn from_hardware_commit(
        candidate: PersistedOutgoingCandidateV1,
        commit_certificate: OfflineCashCommitCertificateV1,
    ) -> Result<Self, OfflineCashStateErrorV1> {
        let prepared = &candidate.prepared;
        let lifecycle_digest = prepared
            .lifecycle()
            .canonical_digest()
            .map_err(|_| OfflineCashStateErrorV1::HardwareCertificateMismatch)?;
        let reservation_commitment = prepared
            .outbox_reservation
            .canonical_commitment()
            .map_err(|_| OfflineCashStateErrorV1::HardwareCertificateMismatch)?;
        let expected_certificate_id = commit_certificate
            .expected_certificate_id()
            .map_err(|_| OfflineCashStateErrorV1::HardwareCertificateMismatch)?;
        if commit_certificate.version != OFFLINE_CASH_WIRE_VERSION_V1
            || commit_certificate.certificate_id != expected_certificate_id
            || commit_certificate.candidate_envelope_digest != candidate.candidate_envelope_digest
            || commit_certificate.lifecycle_binding_digest != lifecycle_digest
            || commit_certificate.transition_nullifier != prepared.transition_nullifier()
            || commit_certificate.outbox_reservation_commitment != reservation_commitment
            || commit_certificate.commit_evidence
                != projection_commit_evidence(&prepared.projection)
            || commit_certificate.hardware_profile_id != prepared.lifecycle().hardware_profile_id
            || commit_certificate.policy_epoch != prepared.lifecycle().policy_epoch
            || commit_certificate.hardware_terminal_commitment == [0; 32]
        {
            return Err(OfflineCashStateErrorV1::HardwareCertificateMismatch);
        }
        let commit_certificate_digest = canonical_commit_certificate_digest_v1(&commit_certificate)
            .map_err(|_| OfflineCashStateErrorV1::HardwareCertificateMismatch)?;
        Ok(Self {
            candidate,
            commit_certificate,
            commit_certificate_digest,
        })
    }

    /// Build the unlinkable public inputs verified by a receiver or settlement node.
    pub fn public_wrapper_inputs(
        &self,
    ) -> Result<OfflineCashCommitWrapperPublicInputsV1, OfflineCashStateErrorV1> {
        let prepared = &self.candidate.prepared;
        let semantic_digest = prepared.semantic_digest()?;
        match &prepared.projection {
            PreparedPublicProjectionV1::Send(projection) => {
                let PreparedSendPublicProjectionV1 {
                    request, statement, ..
                } = projection.as_ref();
                OfflineCashRecursivePublicOutputV1::new(
                    statement.lifecycle.clone(),
                    semantic_digest,
                    self.candidate.candidate_envelope_digest,
                    self.commit_certificate_digest,
                    statement.transition_nullifier,
                    statement.request_digest,
                    statement.acceptance_ticket_digest,
                    statement.ciphertext_commitment,
                    statement.amount,
                    super::canonical_terminal_send_output_binding_v1(
                        statement.lifecycle.credit_id,
                        request.hardware_credential.lane_commitment,
                        statement.request_digest,
                        statement.acceptance_ticket_digest,
                        statement.ciphertext_commitment,
                        statement.amount,
                    ),
                )
                .map_err(|error| OfflineCashStateErrorV1::ProofRejected(error.to_string()))
            }
            PreparedPublicProjectionV1::Redemption { statement } => {
                OfflineCashRecursivePublicOutputV1::new(
                    statement.lifecycle.clone(),
                    semantic_digest,
                    self.candidate.candidate_envelope_digest,
                    self.commit_certificate_digest,
                    statement.terminal_nullifier,
                    [0; 32],
                    [0; 32],
                    [0; 32],
                    statement.amount,
                    statement.redemption_commitment,
                )
                .map_err(|error| OfflineCashStateErrorV1::ProofRejected(error.to_string()))
            }
        }
    }

    /// Borrow private state heads for the sender-local wrapper prover.
    ///
    /// Receiver and settlement verification must use [`Self::public_wrapper_inputs`] instead.
    #[must_use]
    pub(crate) fn private_state_link(&self) -> (&OfflineCashStateV1, &OfflineCashStateV1) {
        self.candidate.prepared.private_state_link()
    }

    fn validate_recovered<R: OfflineCashRecursiveVerifierV1>(
        &self,
        artifacts: OfflineCashRecursionArtifactsV1,
        verifier: &R,
    ) -> Result<(), OfflineCashStateErrorV1> {
        self.candidate.validate_recovered(artifacts, verifier)?;
        let reconstructed =
            Self::from_hardware_commit(self.candidate.clone(), self.commit_certificate.clone())?;
        if reconstructed != *self {
            return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }
}

/// Sole public commit-wrapper input projection. It intentionally contains no aggregate heads.
pub type OfflineCashCommitWrapperPublicInputsV1 = OfflineCashRecursivePublicOutputV1;

/// Fail-closed terminal wrapper verifier seam used by both payment and redemption finalization.
pub trait OfflineCashCommitWrapperVerifierV1 {
    /// Verify the final proof against only the unlinkable public projection.
    fn verify_commit_wrapper(
        &self,
        public_inputs: &OfflineCashCommitWrapperPublicInputsV1,
        proof: &OfflineCashCommitWrapperProofV1,
    ) -> Result<(), String>;
}

/// Final terminal wire envelope retained by the authenticated retry outbox.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub enum OfflineCashOutgoingEnvelopeV1 {
    /// Receiver-bound offline payment.
    Payment(OfflineCashPaymentV1),
    /// Chain-facing redemption voucher.
    Redemption(OfflineCashRedemptionVoucherV1),
}

impl OfflineCashOutgoingEnvelopeV1 {
    fn canonical_bytes(&self) -> Result<Vec<u8>, OfflineCashStateErrorV1> {
        match self {
            Self::Payment(payment) => norito::encode_canonical(payment),
            Self::Redemption(voucher) => norito::encode_canonical(voucher),
        }
        .map_err(|_| OfflineCashStateErrorV1::CanonicalEncoding)
    }
}

/// Complete durable retry record installed before any terminal bytes are exposed.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct DurableOutgoingEnvelopeV1 {
    /// Hardware-committed private candidate retained for deterministic recovery.
    pub committed: CommittedOutgoingCandidateV1,
    /// Sole public payment or redemption envelope.
    pub envelope: OfflineCashOutgoingEnvelopeV1,
    /// Exact canonical bytes returned by every retry.
    pub canonical_envelope_bytes: Vec<u8>,
    /// Domain-separated digest of those exact bytes.
    pub envelope_digest: DigestV1,
    /// Bounded authenticated transport retry metadata.
    pub retry_metadata: Vec<u8>,
}

impl DurableOutgoingEnvelopeV1 {
    /// Return the exact canonical bytes occupied by this durable retry record.
    pub(super) fn canonical_storage_bytes(&self) -> Result<u64, OfflineCashStateErrorV1> {
        let encoded = norito::encode_canonical(self)
            .map_err(|_| OfflineCashStateErrorV1::CanonicalEncoding)?;
        u64::try_from(encoded.len()).map_err(|_| OfflineCashStateErrorV1::ArithmeticOverflow)
    }

    fn validate_storage_bound(&self) -> Result<(), OfflineCashStateErrorV1> {
        let operation_kind = self
            .committed
            .candidate
            .prepared
            .outbox_reservation
            .operation_kind;
        let variant_matches_operation = matches!(
            (operation_kind, &self.envelope),
            (
                OfflineCashOperationKindV1::SendSplit,
                OfflineCashOutgoingEnvelopeV1::Payment(_)
            ) | (
                OfflineCashOperationKindV1::RedeemSplit,
                OfflineCashOutgoingEnvelopeV1::Redemption(_)
            )
        );
        let maximum = maximum_durable_outgoing_record_bytes_v1(operation_kind)
            .ok_or(OfflineCashStateErrorV1::StateInvariant)?;
        if !variant_matches_operation || self.canonical_storage_bytes()? > maximum {
            return Err(OfflineCashStateErrorV1::StateInvariant);
        }
        Ok(())
    }

    /// Generate, verify, and package the final wrapper without exposing bytes or changing state.
    pub fn finalize<V: OfflineCashCommitWrapperVerifierV1>(
        committed: CommittedOutgoingCandidateV1,
        wrapper_proof: OfflineCashCommitWrapperProofV1,
        artifact_manifest_digest: DigestV1,
        retry_metadata: Vec<u8>,
        verifier: &V,
    ) -> Result<Self, OfflineCashStateErrorV1> {
        if artifact_manifest_digest == [0; 32]
            || retry_metadata.len()
                > usize::try_from(OFFLINE_CASH_OUTBOX_RETRY_METADATA_MAX_BYTES_V1)
                    .unwrap_or(usize::MAX)
        {
            return Err(OfflineCashStateErrorV1::InvalidRecoveryMaterial);
        }
        let public_inputs = committed.public_wrapper_inputs()?;
        if wrapper_proof.version != OFFLINE_CASH_WIRE_VERSION_V1
            || wrapper_proof.semantic_digest != public_inputs.semantic_digest
            || wrapper_proof.candidate_envelope_digest != public_inputs.candidate_envelope_digest
            || wrapper_proof.commit_certificate_digest != public_inputs.commit_certificate_digest
        {
            return Err(OfflineCashStateErrorV1::InvalidProofBundle);
        }
        verifier
            .verify_commit_wrapper(&public_inputs, &wrapper_proof)
            .map_err(OfflineCashStateErrorV1::ProofRejected)?;

        let prepared = &committed.candidate.prepared;
        let envelope = match &prepared.projection {
            PreparedPublicProjectionV1::Send(projection) => {
                let PreparedSendPublicProjectionV1 {
                    request,
                    acceptance_intent,
                    acceptance_ticket,
                    statement,
                    encrypted_credit,
                } = projection.as_ref();
                let payment = OfflineCashPaymentV1 {
                    version: OFFLINE_CASH_WIRE_VERSION_V1,
                    statement: statement.clone(),
                    acceptance_intent: *acceptance_intent,
                    acceptance_ticket: acceptance_ticket.clone(),
                    commit_certificate: committed.commit_certificate.clone(),
                    proof: wrapper_proof,
                    encrypted_credit: encrypted_credit.clone(),
                    artifact_manifest_digest,
                };
                payment
                    .validate_shape_against(request)
                    .map_err(|_| OfflineCashStateErrorV1::InvalidPeerCredit)?;
                OfflineCashOutgoingEnvelopeV1::Payment(payment)
            }
            PreparedPublicProjectionV1::Redemption { statement } => {
                let voucher = OfflineCashRedemptionVoucherV1 {
                    version: OFFLINE_CASH_WIRE_VERSION_V1,
                    statement: statement.clone(),
                    commit_certificate: committed.commit_certificate.clone(),
                    proof: wrapper_proof,
                    artifact_manifest_digest,
                };
                voucher
                    .validate_shape()
                    .map_err(|_| OfflineCashStateErrorV1::InvalidRedemption)?;
                OfflineCashOutgoingEnvelopeV1::Redemption(voucher)
            }
        };
        let canonical_envelope_bytes = envelope.canonical_bytes()?;
        let maximum = match &envelope {
            OfflineCashOutgoingEnvelopeV1::Payment(_) => OFFLINE_CASH_PAYMENT_MAX_BYTES_V1,
            OfflineCashOutgoingEnvelopeV1::Redemption(_) => {
                OFFLINE_CASH_REDEMPTION_VOUCHER_MAX_BYTES_V1
            }
        };
        if canonical_envelope_bytes.len() > maximum {
            return Err(OfflineCashStateErrorV1::InvalidProofBundle);
        }
        let envelope_digest = digest_raw_bytes(
            OUTGOING_ENVELOPE_DIGEST_DOMAIN_V1,
            &canonical_envelope_bytes,
        );
        let finalized = Self {
            committed,
            envelope,
            canonical_envelope_bytes,
            envelope_digest,
            retry_metadata,
        };
        finalized.validate_storage_bound()?;
        Ok(finalized)
    }

    /// Borrow the exact byte-identical terminal retry envelope.
    #[must_use]
    pub fn retry_bytes(&self) -> &[u8] {
        &self.canonical_envelope_bytes
    }

    /// Borrow the private successor installed atomically with durable final-envelope persistence.
    #[must_use]
    pub(crate) fn successor_state(&self) -> &OfflineCashStateV1 {
        &self.committed.candidate.prepared.successor_state
    }

    fn validate_recovered<R: OfflineCashRecursiveVerifierV1>(
        &self,
        artifacts: OfflineCashRecursionArtifactsV1,
        verifier: &R,
    ) -> Result<(), OfflineCashStateErrorV1> {
        self.validate_storage_bound()
            .map_err(|_| OfflineCashStateErrorV1::SnapshotIntegrity)?;
        self.committed.validate_recovered(artifacts, verifier)?;
        let wrapper_proof = match &self.envelope {
            OfflineCashOutgoingEnvelopeV1::Payment(payment) => payment.proof.clone(),
            OfflineCashOutgoingEnvelopeV1::Redemption(voucher) => voucher.proof.clone(),
        };
        let artifact_manifest_digest = match &self.envelope {
            OfflineCashOutgoingEnvelopeV1::Payment(payment) => payment.artifact_manifest_digest,
            OfflineCashOutgoingEnvelopeV1::Redemption(voucher) => voucher.artifact_manifest_digest,
        };
        if artifact_manifest_digest != artifacts.artifact_manifest_digest {
            return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
        }
        let recovered_verifier = RecoveredCommitWrapperVerifierV1 {
            artifacts,
            verifier,
        };
        let reconstructed = Self::finalize(
            self.committed.clone(),
            wrapper_proof,
            artifact_manifest_digest,
            self.retry_metadata.clone(),
            &recovered_verifier,
        )?;
        if reconstructed != *self {
            return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }
}

struct RecoveredCommitWrapperVerifierV1<'a, R> {
    artifacts: OfflineCashRecursionArtifactsV1,
    verifier: &'a R,
}

impl<R: OfflineCashRecursiveVerifierV1> OfflineCashCommitWrapperVerifierV1
    for RecoveredCommitWrapperVerifierV1<'_, R>
{
    fn verify_commit_wrapper(
        &self,
        public_inputs: &OfflineCashCommitWrapperPublicInputsV1,
        proof: &OfflineCashCommitWrapperProofV1,
    ) -> Result<(), String> {
        verify_offline_cash_recursive_proof_v1(
            self.verifier,
            self.artifacts,
            public_inputs.clone(),
            proof,
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
    }
}

/// Durable stage of the sole outgoing transition allowed on one serialized hardware lane.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub enum OfflineCashOutgoingJournalStageV1 {
    /// No predecessor is locked.
    Empty,
    /// Hardware locked and sealed inputs; proof generation may resume.
    Prepared(PreparedOutgoingCandidateV1),
    /// Candidate proof was locally verified and persisted; hardware commit may resume.
    Candidate(PersistedOutgoingCandidateV1),
    /// Hardware committed the exact candidate; wrapper generation may resume.
    Committed(CommittedOutgoingCandidateV1),
}

/// Recoverable exact-once prepare → prove → commit → wrapper → expose journal.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct OfflineCashOutgoingCandidateJournalV1 {
    stage: OfflineCashOutgoingJournalStageV1,
    finalized_outbox: BTreeMap<DigestV1, DurableOutgoingEnvelopeV1>,
    released_envelopes: BTreeMap<DigestV1, DigestV1>,
}

impl Default for OfflineCashOutgoingCandidateJournalV1 {
    fn default() -> Self {
        Self {
            stage: OfflineCashOutgoingJournalStageV1::Empty,
            finalized_outbox: BTreeMap::new(),
            released_envelopes: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode)]
struct SenderCapacityCanonicalProjectionV1 {
    outbox: OfflineCashSenderOutboxCapacityV1,
    journal: OfflineCashOutgoingCandidateJournalV1,
}

fn canonical_sender_retained_metadata_bytes_v1(
    outbox: &OfflineCashSenderOutboxCapacityV1,
    journal: &OfflineCashOutgoingCandidateJournalV1,
) -> Result<u64, OfflineCashStateErrorV1> {
    let mut normalized_outbox = outbox.clone();
    normalized_outbox.total_outbox_bytes = 0;
    normalized_outbox.committed_outbox_bytes = 0;
    normalized_outbox.retained_metadata_bytes = 0;
    normalized_outbox.reserved_terminal_metadata_bytes = 0;
    let mut normalized_journal = journal.clone();
    normalized_journal.stage = OfflineCashOutgoingJournalStageV1::Empty;
    normalized_journal.finalized_outbox.clear();
    let encoded = norito::encode_canonical(&SenderCapacityCanonicalProjectionV1 {
        outbox: normalized_outbox,
        journal: normalized_journal,
    })
    .map_err(|_| OfflineCashStateErrorV1::CanonicalEncoding)?;
    let empty = norito::encode_canonical(&SenderCapacityCanonicalProjectionV1 {
        outbox: OfflineCashSenderOutboxCapacityV1::new(0),
        journal: OfflineCashOutgoingCandidateJournalV1::default(),
    })
    .map_err(|_| OfflineCashStateErrorV1::CanonicalEncoding)?;
    let bytes = encoded
        .len()
        .checked_sub(empty.len())
        .ok_or(OfflineCashStateErrorV1::StateInvariant)?;
    u64::try_from(bytes).map_err(|_| OfflineCashStateErrorV1::ArithmeticOverflow)
}

fn canonical_sender_materialized_live_bytes_v1(
    outbox: &OfflineCashSenderOutboxCapacityV1,
    journal: &OfflineCashOutgoingCandidateJournalV1,
) -> Result<u64, OfflineCashStateErrorV1> {
    let mut normalized_outbox = outbox.clone();
    normalized_outbox.total_outbox_bytes = 0;
    normalized_outbox.committed_outbox_bytes = 0;
    normalized_outbox.retained_metadata_bytes = 0;
    normalized_outbox.reserved_terminal_metadata_bytes = 0;
    let encoded = norito::encode_canonical(&SenderCapacityCanonicalProjectionV1 {
        outbox: normalized_outbox.clone(),
        journal: journal.clone(),
    })
    .map_err(|_| OfflineCashStateErrorV1::CanonicalEncoding)?;
    let mut baseline_journal = journal.clone();
    baseline_journal.stage = OfflineCashOutgoingJournalStageV1::Empty;
    baseline_journal.finalized_outbox.clear();
    let baseline = norito::encode_canonical(&SenderCapacityCanonicalProjectionV1 {
        outbox: normalized_outbox,
        journal: baseline_journal,
    })
    .map_err(|_| OfflineCashStateErrorV1::CanonicalEncoding)?;
    let bytes = encoded
        .len()
        .checked_sub(baseline.len())
        .ok_or(OfflineCashStateErrorV1::StateInvariant)?;
    u64::try_from(bytes).map_err(|_| OfflineCashStateErrorV1::ArithmeticOverflow)
}

fn sender_outbox_capacity_meters_v1(
    outbox: &OfflineCashSenderOutboxCapacityV1,
    journal: &OfflineCashOutgoingCandidateJournalV1,
) -> Result<SenderOutboxCapacityMetersV1, OfflineCashStateErrorV1> {
    let retained_metadata_bytes = canonical_sender_retained_metadata_bytes_v1(outbox, journal)?;
    let mut live_slot_bytes = 0_u64;
    let mut terminal_outbox = outbox.clone();
    let mut terminal_journal = journal.clone();
    terminal_journal.stage = OfflineCashOutgoingJournalStageV1::Empty;
    terminal_journal.finalized_outbox.clear();
    for (reservation_id, record) in &mut terminal_outbox.reservations {
        if record.released {
            continue;
        }
        checked_capacity_add_v1(
            &mut live_slot_bytes,
            u64::from(record.reservation.reserved_outbox_bytes).max(
                implementation_live_outbox_slot_bytes_v1(record.reservation.operation_kind)
                    .ok_or(OfflineCashStateErrorV1::StateInvariant)?,
            ),
        )?;
        if terminal_journal
            .released_envelopes
            .contains_key(reservation_id)
        {
            return Err(OfflineCashStateErrorV1::StateInvariant);
        }
        let terminal_digest = record.terminal_envelope_digest.unwrap_or([u8::MAX; 32]);
        record.terminal_envelope_digest = Some(terminal_digest);
        record.released = true;
        terminal_journal
            .released_envelopes
            .insert(*reservation_id, terminal_digest);
    }
    for (reservation_id, finalized) in &journal.finalized_outbox {
        let reservation = finalized.committed.candidate.prepared.outbox_reservation;
        let record = outbox
            .reservations
            .get(reservation_id)
            .ok_or(OfflineCashStateErrorV1::StateInvariant)?;
        if reservation.reservation_id != *reservation_id
            || record.reservation != reservation
            || record.released
        {
            return Err(OfflineCashStateErrorV1::StateInvariant);
        }
        finalized.validate_storage_bound()?;
    }
    let materialized_live_bytes = canonical_sender_materialized_live_bytes_v1(outbox, journal)?;
    if materialized_live_bytes > live_slot_bytes {
        return Err(OfflineCashStateErrorV1::StateInvariant);
    }
    let terminal_metadata_bytes =
        canonical_sender_retained_metadata_bytes_v1(&terminal_outbox, &terminal_journal)?;
    let reserved_terminal_metadata_bytes = terminal_metadata_bytes
        .checked_sub(retained_metadata_bytes)
        .ok_or(OfflineCashStateErrorV1::StateInvariant)?;
    let committed_outbox_bytes = retained_metadata_bytes
        .checked_add(live_slot_bytes)
        .and_then(|bytes| bytes.checked_add(reserved_terminal_metadata_bytes))
        .ok_or(OfflineCashStateErrorV1::ArithmeticOverflow)?;
    Ok(SenderOutboxCapacityMetersV1 {
        committed_outbox_bytes,
        retained_metadata_bytes,
        reserved_terminal_metadata_bytes,
    })
}

impl OfflineCashOutgoingCandidateJournalV1 {
    /// Borrow the current durable stage.
    #[must_use]
    pub const fn stage(&self) -> &OfflineCashOutgoingJournalStageV1 {
        &self.stage
    }

    /// Install or idempotently recover the exact sealed prepare record.
    pub fn prepare(
        &mut self,
        prepared: PreparedOutgoingCandidateV1,
    ) -> Result<(), OfflineCashStateErrorV1> {
        match &self.stage {
            OfflineCashOutgoingJournalStageV1::Empty => {
                self.stage = OfflineCashOutgoingJournalStageV1::Prepared(prepared);
                Ok(())
            }
            OfflineCashOutgoingJournalStageV1::Prepared(existing) if existing == &prepared => {
                Ok(())
            }
            _ => Err(OfflineCashStateErrorV1::InvalidCandidateStage),
        }
    }

    /// Advance from an exact prepare record to its locally verified persisted candidate.
    pub fn persist_candidate(
        &mut self,
        candidate: PersistedOutgoingCandidateV1,
    ) -> Result<(), OfflineCashStateErrorV1> {
        match &self.stage {
            OfflineCashOutgoingJournalStageV1::Prepared(prepared)
                if prepared == &candidate.prepared =>
            {
                self.stage = OfflineCashOutgoingJournalStageV1::Candidate(candidate);
                Ok(())
            }
            OfflineCashOutgoingJournalStageV1::Candidate(existing) if existing == &candidate => {
                Ok(())
            }
            OfflineCashOutgoingJournalStageV1::Prepared(_) => {
                Err(OfflineCashStateErrorV1::CandidateConflict)
            }
            _ => Err(OfflineCashStateErrorV1::InvalidCandidateStage),
        }
    }

    /// Advance from a persisted candidate to the exact recovered hardware terminal certificate.
    pub fn commit(
        &mut self,
        committed: CommittedOutgoingCandidateV1,
    ) -> Result<(), OfflineCashStateErrorV1> {
        match &self.stage {
            OfflineCashOutgoingJournalStageV1::Candidate(candidate)
                if candidate == &committed.candidate =>
            {
                self.stage = OfflineCashOutgoingJournalStageV1::Committed(committed);
                Ok(())
            }
            OfflineCashOutgoingJournalStageV1::Committed(existing) if existing == &committed => {
                Ok(())
            }
            OfflineCashOutgoingJournalStageV1::Candidate(_) => {
                Err(OfflineCashStateErrorV1::CandidateConflict)
            }
            _ => Err(OfflineCashStateErrorV1::InvalidCandidateStage),
        }
    }

    /// Persist the final canonical envelope and bind its bytes to the reserved outbox.
    pub fn install_finalized(
        &mut self,
        finalized: DurableOutgoingEnvelopeV1,
        outbox: &mut OfflineCashSenderOutboxCapacityV1,
    ) -> Result<(), OfflineCashStateErrorV1> {
        let reservation_id = finalized
            .committed
            .candidate
            .prepared
            .outbox_reservation
            .reservation_id;
        if self.released_envelopes.contains_key(&reservation_id) {
            return Err(OfflineCashStateErrorV1::CandidateConflict);
        }
        if let Some(existing) = self.finalized_outbox.get(&reservation_id) {
            if existing != &finalized {
                return Err(OfflineCashStateErrorV1::CandidateConflict);
            }
            outbox.validate_capacity_meters(self, false)?;
            return Ok(());
        }
        outbox.validate_capacity_meters(self, false)?;
        match &self.stage {
            OfflineCashOutgoingJournalStageV1::Committed(committed)
                if committed == &finalized.committed =>
            {
                let mut next = self.clone();
                let mut next_outbox = outbox.clone();
                next_outbox.bind_terminal_envelope(
                    finalized.committed.candidate.prepared.outbox_reservation,
                    finalized.envelope_digest,
                )?;
                next.finalized_outbox.insert(reservation_id, finalized);
                next.stage = OfflineCashOutgoingJournalStageV1::Empty;
                next_outbox
                    .reconcile_terminal_capacity_meters(&next, outbox.committed_outbox_bytes)?;
                *self = next;
                *outbox = next_outbox;
                Ok(())
            }
            OfflineCashOutgoingJournalStageV1::Committed(_) => {
                Err(OfflineCashStateErrorV1::CandidateConflict)
            }
            _ => Err(OfflineCashStateErrorV1::InvalidCandidateStage),
        }
    }

    /// Return terminal bytes only after final wrapper verification and durable installation.
    pub fn expose(&self, reservation_id: DigestV1) -> Result<&[u8], OfflineCashStateErrorV1> {
        self.finalized_outbox
            .get(&reservation_id)
            .map(DurableOutgoingEnvelopeV1::retry_bytes)
            .ok_or(OfflineCashStateErrorV1::InvalidCandidateStage)
    }

    /// Borrow one durable terminal envelope for exact retry or settlement processing.
    #[must_use]
    pub fn finalized_envelope(
        &self,
        reservation_id: DigestV1,
    ) -> Option<&DurableOutgoingEnvelopeV1> {
        self.finalized_outbox.get(&reservation_id)
    }

    /// Return the number of independently retryable terminal envelopes.
    #[must_use]
    pub fn finalized_outbox_count(&self) -> usize {
        self.finalized_outbox.len()
    }

    /// Retire a byte-identical retry envelope only after an authenticated ACK or idempotent
    /// settlement result proves retry is no longer necessary.
    pub(crate) fn release_finalized(
        &mut self,
        reservation_id: DigestV1,
        expected_envelope_digest: DigestV1,
        outbox: &mut OfflineCashSenderOutboxCapacityV1,
    ) -> Result<(), OfflineCashStateErrorV1> {
        if let Some(existing) = self.released_envelopes.get(&reservation_id) {
            if *existing != expected_envelope_digest {
                return Err(OfflineCashStateErrorV1::CandidateConflict);
            }
            outbox.validate_capacity_meters(self, false)?;
            return Ok(());
        }
        let finalized = self
            .finalized_outbox
            .get(&reservation_id)
            .ok_or(OfflineCashStateErrorV1::InvalidCandidateStage)?;
        if finalized.envelope_digest != expected_envelope_digest {
            return Err(OfflineCashStateErrorV1::CandidateConflict);
        }
        let mut next = self.clone();
        let mut next_outbox = outbox.clone();
        next_outbox.mark_terminal_released(reservation_id, expected_envelope_digest)?;
        next.finalized_outbox.remove(&reservation_id);
        next.released_envelopes
            .insert(reservation_id, expected_envelope_digest);
        next_outbox.reconcile_capacity_meters(&next)?;
        *self = next;
        *outbox = next_outbox;
        Ok(())
    }

    pub(crate) fn validate_recovered<R: OfflineCashRecursiveVerifierV1>(
        &self,
        state: &OfflineCashStateV1,
        journal_revision: u128,
        outbox: &OfflineCashSenderOutboxCapacityV1,
        artifacts: OfflineCashRecursionArtifactsV1,
        verifier: &R,
    ) -> Result<(), OfflineCashStateErrorV1> {
        state.validate()?;
        outbox.validate_recovered(self)?;

        let active_reservation = match &self.stage {
            OfflineCashOutgoingJournalStageV1::Empty => None,
            OfflineCashOutgoingJournalStageV1::Prepared(prepared) => {
                prepared.validate_recovered()?;
                if &prepared.predecessor_state != state
                    || prepared.proof_statement.journal_revision_before != journal_revision
                {
                    return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
                }
                Some((prepared.outbox_reservation, false))
            }
            OfflineCashOutgoingJournalStageV1::Candidate(candidate) => {
                candidate.validate_recovered(artifacts, verifier)?;
                if &candidate.prepared.predecessor_state != state
                    || candidate.prepared.proof_statement.journal_revision_before
                        != journal_revision
                {
                    return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
                }
                Some((candidate.prepared.outbox_reservation, false))
            }
            OfflineCashOutgoingJournalStageV1::Committed(committed) => {
                committed.validate_recovered(artifacts, verifier)?;
                if &committed.candidate.prepared.successor_state != state
                    || committed
                        .candidate
                        .prepared
                        .proof_statement
                        .journal_revision_after
                        != journal_revision
                {
                    return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
                }
                Some((committed.candidate.prepared.outbox_reservation, false))
            }
        };

        for (reservation_id, finalized) in &self.finalized_outbox {
            finalized.validate_recovered(artifacts, verifier)?;
            let reservation = finalized.committed.candidate.prepared.outbox_reservation;
            if *reservation_id != reservation.reservation_id
                || self.released_envelopes.contains_key(reservation_id)
            {
                return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
            }
            let record = outbox
                .reservations
                .get(reservation_id)
                .ok_or(OfflineCashStateErrorV1::SnapshotIntegrity)?;
            if record.reservation != reservation
                || record.released
                || record.terminal_envelope_digest != Some(finalized.envelope_digest)
            {
                return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
            }
            validate_installed_successor_not_ahead(state, finalized.successor_state())?;
        }
        for (reservation_id, envelope_digest) in &self.released_envelopes {
            if *envelope_digest == [0; 32] || self.finalized_outbox.contains_key(reservation_id) {
                return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
            }
            let record = outbox
                .reservations
                .get(reservation_id)
                .ok_or(OfflineCashStateErrorV1::SnapshotIntegrity)?;
            if !record.released || record.terminal_envelope_digest != Some(*envelope_digest) {
                return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
            }
        }

        for (reservation_id, record) in &outbox.reservations {
            let is_active = active_reservation
                .is_some_and(|(reservation, _)| reservation.reservation_id == *reservation_id);
            let is_finalized = self.finalized_outbox.contains_key(reservation_id);
            let is_released = self.released_envelopes.contains_key(reservation_id);
            if usize::from(is_active) + usize::from(is_finalized) + usize::from(is_released) != 1
                || (is_active && (record.released || record.terminal_envelope_digest.is_some()))
            {
                return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
            }
        }
        if let Some((reservation, _)) = active_reservation {
            let record = outbox
                .reservations
                .get(&reservation.reservation_id)
                .ok_or(OfflineCashStateErrorV1::SnapshotIntegrity)?;
            if record.reservation != reservation {
                return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
            }
        }
        Ok(())
    }
}

fn validate_installed_successor_not_ahead(
    current: &OfflineCashStateV1,
    installed: &OfflineCashStateV1,
) -> Result<(), OfflineCashStateErrorV1> {
    if current.release_id != installed.release_id
        || current.asset_incarnation != installed.asset_incarnation
        || current.liability_pool_id != installed.liability_pool_id
        || current.lane != installed.lane
        || current.hardware_epoch.generation < installed.hardware_epoch.generation
        || (current.hardware_epoch.generation == installed.hardware_epoch.generation
            && current.hardware_epoch != installed.hardware_epoch)
        || (current.hardware_epoch == installed.hardware_epoch
            && current.logical_sequence < installed.logical_sequence)
    {
        return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
    }
    Ok(())
}

fn validate_recovered_projection(
    projection: &PreparedPublicProjectionV1,
    predecessor: &OfflineCashStateV1,
    reservation: OfflineCashOutboxReservationV1,
) -> Result<(), OfflineCashStateErrorV1> {
    reservation
        .canonical_commitment()
        .map_err(|_| OfflineCashStateErrorV1::SnapshotIntegrity)?;
    match projection {
        PreparedPublicProjectionV1::Send(projection) => {
            let PreparedSendPublicProjectionV1 {
                request,
                acceptance_intent,
                acceptance_ticket,
                statement,
                encrypted_credit,
            } = projection.as_ref();
            request
                .validate_shape()
                .map_err(|_| OfflineCashStateErrorV1::SnapshotIntegrity)?;
            acceptance_intent
                .validate_shape_against(request)
                .map_err(|_| OfflineCashStateErrorV1::SnapshotIntegrity)?;
            acceptance_ticket
                .validate_shape_against(request, acceptance_intent)
                .map_err(|_| OfflineCashStateErrorV1::SnapshotIntegrity)?;
            statement
                .validate()
                .map_err(|_| OfflineCashStateErrorV1::SnapshotIntegrity)?;
            validate_request_against_state(request, predecessor)?;
            let request_digest = request
                .canonical_digest()
                .map_err(|_| OfflineCashStateErrorV1::SnapshotIntegrity)?;
            let ticket_digest = acceptance_ticket
                .canonical_digest_against(request, acceptance_intent)
                .map_err(|_| OfflineCashStateErrorV1::SnapshotIntegrity)?;
            if reservation.operation_kind != OfflineCashOperationKindV1::SendSplit
                || statement.lifecycle.operation_kind != OfflineCashOperationKindV1::SendSplit
                || statement.lifecycle.request_id != request.request_id
                || statement.lifecycle.acceptance_ticket_id
                    != acceptance_ticket.acceptance_ticket_id
                || statement.lifecycle.credit_id == [0; 32]
                || statement.lifecycle.ciphertext_digest
                    != digest_raw_bytes(CIPHERTEXT_DIGEST_DOMAIN_V1, encrypted_credit)
                || statement.request_digest != request_digest
                || statement.acceptance_ticket_digest != ticket_digest
                || statement.recipient_one_time_key != acceptance_ticket.recipient_one_time_key
                || statement.amount != acceptance_intent.exact_amount
                || encrypted_credit.is_empty()
                || encrypted_credit.len() > OFFLINE_CASH_ENCRYPTED_CREDIT_MAX_BYTES_V1
            {
                return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
            }
        }
        PreparedPublicProjectionV1::Redemption { statement } => {
            statement
                .validate_shape()
                .map_err(|_| OfflineCashStateErrorV1::SnapshotIntegrity)?;
            if reservation.operation_kind != OfflineCashOperationKindV1::RedeemSplit
                || statement.lifecycle.operation_kind != OfflineCashOperationKindV1::RedeemSplit
                || statement.lifecycle.request_id != [0; 32]
                || statement.lifecycle.acceptance_ticket_id != [0; 32]
                || statement.lifecycle.credit_id != [0; 32]
                || statement.lifecycle.ciphertext_digest != [0; 32]
            {
                return Err(OfflineCashStateErrorV1::SnapshotIntegrity);
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_prepared_transition_statement(
    predecessor: &OfflineCashStateV1,
    successor: &OfflineCashStateV1,
    state_transition_digest: DigestV1,
    statement: &TransitionProofStatementV1,
    transport_semantic_digest: DigestV1,
    projection: &PreparedPublicProjectionV1,
    normalized_guard_statement_digest: DigestV1,
) -> Result<(), OfflineCashStateErrorV1> {
    let lifecycle_digest = projection
        .lifecycle()
        .canonical_digest()
        .map_err(|_| OfflineCashStateErrorV1::InvalidCandidateStage)?;
    let projection_semantic_digest = projection
        .semantic_digest()
        .map_err(|_| OfflineCashStateErrorV1::InvalidCandidateStage)?;
    let common_invalid = statement.version != super::OFFLINE_CASH_STATE_VERSION_V1
        || statement.protocol_version != predecessor.protocol_version
        || statement.predecessor_suite_id != predecessor.suite_id
        || statement.predecessor_vk_digest != predecessor.vk_digest
        || statement.successor_suite_id != successor.suite_id
        || statement.successor_vk_digest != successor.vk_digest
        || statement.release_id != predecessor.release_id
        || statement.asset_incarnation != predecessor.asset_incarnation
        || statement.liability_pool_id != predecessor.liability_pool_id
        || statement.hardware_profile_id != predecessor.hardware_profile_id
        || statement.policy_epoch != predecessor.policy_epoch
        || statement.lane != predecessor.lane
        || statement.predecessor_commitment != predecessor.state_commitment
        || statement.successor_commitment != successor.state_commitment
        || statement.predecessor_sequence != predecessor.logical_sequence
        || statement.successor_sequence != successor.logical_sequence
        || statement.predecessor_epoch != predecessor.hardware_epoch
        || statement.successor_epoch != successor.hardware_epoch
        || statement.predecessor_device_policy_binding != predecessor.device_policy_binding
        || statement.successor_device_policy_binding != successor.device_policy_binding
        || statement.predecessor_state_nonce_commitment != predecessor.state_nonce_commitment
        || statement.successor_state_nonce_commitment != successor.state_nonce_commitment
        || statement.journal_revision_after
            != statement
                .journal_revision_before
                .checked_add(1)
                .ok_or(OfflineCashStateErrorV1::JournalRevisionOverflow)?
        || statement.effect_digest == [0; 32]
        || statement.effect_digest != projection_semantic_digest
        || statement.lifecycle_binding_digest != lifecycle_digest
        || statement.precommit_binding_digest == [0; 32]
        || statement.mint_finality_semantic_digest != [0; 32]
        || statement.mint_finality_proof_binding_digest != [0; 32]
        || statement.suite_upgrade_authorization_digest != [0; 32]
        || transport_semantic_digest == [0; 32]
        || normalized_guard_statement_digest == [0; 32]
        || statement.digest()? != state_transition_digest;
    if common_invalid {
        return Err(OfflineCashStateErrorV1::InvalidCandidateStage);
    }
    let operation_valid = match projection {
        PreparedPublicProjectionV1::Send(projection) => {
            let request = &projection.request;
            let transfer = &projection.statement;
            statement.kind == super::OfflineCashTransitionKindV1::SendSplit
                && statement.amount == transfer.amount
                && statement.peer_credit_id == transfer.lifecycle.credit_id
                && statement.peer_recipient_lane_id == request.hardware_credential.lane_commitment
        }
        PreparedPublicProjectionV1::Redemption {
            statement: redemption,
        } => {
            statement.kind == super::OfflineCashTransitionKindV1::RedeemSplit
                && statement.amount == redemption.amount
                && statement.peer_credit_id == [0; 32]
                && statement.peer_recipient_lane_id == [0; 32]
        }
    };
    if !operation_valid {
        return Err(OfflineCashStateErrorV1::InvalidCandidateStage);
    }
    Ok(())
}

fn validate_private_state_link(
    predecessor: &OfflineCashStateV1,
    successor: &OfflineCashStateV1,
    amount: u128,
    operation: OfflineCashOperationKindV1,
) -> Result<(), OfflineCashStateErrorV1> {
    predecessor.validate()?;
    successor.validate()?;
    if amount == 0
        || predecessor.context() != successor.context()
        || predecessor.lane != successor.lane
        || predecessor.hardware_epoch != successor.hardware_epoch
        || predecessor.device_policy_binding != successor.device_policy_binding
        || predecessor.consumed_credit_root != successor.consumed_credit_root
        || predecessor.state_commitment == successor.state_commitment
        || successor.logical_sequence
            != predecessor
                .logical_sequence
                .checked_add(1)
                .ok_or(OfflineCashStateErrorV1::SequenceOverflow)?
        || !matches!(
            operation,
            OfflineCashOperationKindV1::SendSplit | OfflineCashOperationKindV1::RedeemSplit
        )
        || successor.balance
            != predecessor
                .balance
                .checked_sub(amount)
                .ok_or(OfflineCashStateErrorV1::InsufficientBalance)?
    {
        return Err(OfflineCashStateErrorV1::StateInvariant);
    }
    Ok(())
}

fn validate_request_against_state(
    request: &OfflineCashPaymentRequestV1,
    state: &OfflineCashStateV1,
) -> Result<(), OfflineCashStateErrorV1> {
    if request.release_id != state.release_id
        || request.network_id != state.lane.network_id
        || request.asset != state.lane.asset
        || request.asset_incarnation != state.asset_incarnation
        || request.scale != state.lane.scale
        || request.liability_pool_id != state.liability_pool_id
        || request.hardware_credential.suite_id != state.suite_id
    {
        return Err(OfflineCashStateErrorV1::InvalidPaymentRequest);
    }
    Ok(())
}

fn validate_recovery_material(
    sealed_transition_inputs: &[u8],
    sealed_recovery_seeds: &[u8],
    normalized_guard_statement_digest: DigestV1,
) -> Result<(), OfflineCashStateErrorV1> {
    if sealed_transition_inputs.is_empty()
        || sealed_transition_inputs.len()
            > usize::try_from(OFFLINE_CASH_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1)
                .unwrap_or(usize::MAX)
        || sealed_recovery_seeds.is_empty()
        || sealed_recovery_seeds.len()
            > usize::try_from(OFFLINE_CASH_RECOVERY_SEEDS_MAX_BYTES_V1).unwrap_or(usize::MAX)
        || normalized_guard_statement_digest == [0; 32]
    {
        return Err(OfflineCashStateErrorV1::InvalidRecoveryMaterial);
    }
    Ok(())
}

fn projection_commit_evidence(
    projection: &PreparedPublicProjectionV1,
) -> OfflineCashCommitEvidenceV1 {
    match projection {
        PreparedPublicProjectionV1::Send(projection) => projection.statement.commit_evidence,
        PreparedPublicProjectionV1::Redemption { statement } => statement.commit_evidence,
    }
}

fn digest_raw_bytes(domain: &[u8], bytes: &[u8]) -> DigestV1 {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update([0]);
    hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    hasher.update(bytes);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        account::AccountId,
        asset::AssetDefinitionId,
        block::BlockHeader,
        domain::DomainId,
        nexus::AxtAssetIncarnationV1,
        offline::{
            OFFLINE_CASH_ACCEPTANCE_TICKET_MIN_RESERVED_INBOX_BYTES_V1,
            OFFLINE_CASH_HARDWARE_REQUIRED_CAPABILITIES_V1,
            OFFLINE_CASH_PAYMENT_OUTBOX_MIN_BYTES_V1, OFFLINE_CASH_REDEMPTION_OUTBOX_MIN_BYTES_V1,
            OFFLINE_CASH_XCHACHA20POLY1305_NONCE_BYTES_V1,
            OFFLINE_CASH_XCHACHA20POLY1305_TAG_BYTES_V1, OfflineCashAcknowledgementV1,
            OfflineCashDevicePublicKeyV1, OfflineCashDeviceSignatureV1,
            OfflineCashEncryptedCreditEnvelopeV1, OfflineCashHardwareCredentialV1,
            OfflineCashInboxReceiptV1, OfflineCashNoCommitClosureV1,
            OfflineCashPastaStateCommitmentV1, OfflineCashTrustedCommitTimeV1,
            offline_cash_ciphertext_digest_v1, offline_cash_credit_opening_canonical_len_v1,
            offline_cash_device_key_reference_v1, offline_cash_inbox_receipt_commitment_v1,
            offline_cash_liability_pool_id_v1,
        },
    };
    use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};

    use super::super::{
        AcceptedPaymentReceiptV1, CREDIT_ENVELOPE_DOMAIN, ConsumedCreditRecordV1, CreditIdV1,
        CreditStageCertificateV1, CreditStageStatementV1, DevicePolicyBindingV1,
        DurableAcknowledgementV1, ExactConsumedCreditIndex, HardwareEpochV1, OfflineCashLaneIdV1,
        StagedCreditV1, receiver_snapshot_capacity_usage_v1,
    };
    use super::*;
    use crate::zk::offline_cash_v1_recursion::{
        OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1, OfflineCashNoCommitClosureDecisionV1,
        OfflineCashNoCommitClosureVerifierV1,
    };

    struct AcceptingNoCommitClosureVerifier;

    impl OfflineCashNoCommitClosureVerifierV1 for AcceptingNoCommitClosureVerifier {
        fn verify_no_commit_closure(
            &self,
            closure: &OfflineCashNoCommitClosureV1,
        ) -> Result<OfflineCashNoCommitClosureDecisionV1, String> {
            OfflineCashNoCommitClosureDecisionV1::authenticated(closure)
        }
    }

    struct RejectingNoCommitClosureVerifier;

    impl OfflineCashNoCommitClosureVerifierV1 for RejectingNoCommitClosureVerifier {
        fn verify_no_commit_closure(
            &self,
            _closure: &OfflineCashNoCommitClosureV1,
        ) -> Result<OfflineCashNoCommitClosureDecisionV1, String> {
            Err("rejected test closure".to_owned())
        }
    }

    struct AcceptingOutgoingVerifier;

    impl OfflineCashCandidateProofVerifierV1 for AcceptingOutgoingVerifier {
        fn verify_candidate_proof(
            &self,
            _candidate: &PreparedOutgoingCandidateV1,
            _proof: &OfflineCashPairedProofV1,
        ) -> Result<(), String> {
            Ok(())
        }
    }

    impl OfflineCashCommitWrapperVerifierV1 for AcceptingOutgoingVerifier {
        fn verify_commit_wrapper(
            &self,
            _public_inputs: &OfflineCashCommitWrapperPublicInputsV1,
            _proof: &OfflineCashCommitWrapperProofV1,
        ) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn durable_capacity_requires_complete_operation_floors() {
        let exact = OfflineCashDurableCapacityV1 {
            inbox_bytes: OfflineCashDurableCapacityV1::MINIMUM_INBOX_BYTES,
            outbox_bytes: OfflineCashDurableCapacityV1::MINIMUM_OUTBOX_BYTES,
        };
        assert_eq!(exact.validate(), Ok(()));
        assert_eq!(OfflineCashDurableCapacityV1::MINIMUM_INBOX_BYTES, 298_640);
        assert_eq!(OfflineCashDurableCapacityV1::MINIMUM_OUTBOX_BYTES, 90_274);

        assert_eq!(
            OfflineCashDurableCapacityV1 {
                inbox_bytes: OfflineCashDurableCapacityV1::MINIMUM_INBOX_BYTES - 1,
                ..exact
            }
            .validate(),
            Err(OfflineCashStateErrorV1::InvalidDurableCapacity)
        );
        assert_eq!(
            OfflineCashDurableCapacityV1 {
                outbox_bytes: OfflineCashDurableCapacityV1::MINIMUM_OUTBOX_BYTES - 1,
                ..exact
            }
            .validate(),
            Err(OfflineCashStateErrorV1::InvalidDurableCapacity)
        );

        let old_raw_inbox = u64::from(OFFLINE_CASH_ACCEPTANCE_TICKET_MIN_RESERVED_INBOX_BYTES_V1);
        assert_eq!(
            OfflineCashDurableCapacityV1 {
                inbox_bytes: old_raw_inbox,
                ..exact
            }
            .validate(),
            Err(OfflineCashStateErrorV1::InvalidDurableCapacity)
        );
        assert_eq!(
            OfflineCashDurableCapacityV1 {
                outbox_bytes: MAXIMUM_OPERATION_OUTBOX_RESERVATION_BYTES_V1,
                ..exact
            }
            .validate(),
            Err(OfflineCashStateErrorV1::InvalidDurableCapacity)
        );
    }

    #[test]
    fn durable_capacity_floors_cover_worst_bounded_projections() {
        assert_eq!(
            OfflineCashDurableCapacityV1::MINIMUM_INBOX_BYTES,
            RECEIVER_SNAPSHOT_ENTRY_MAX_BYTES_V1
                + 2 * OFFLINE_CASH_PRE_TICKET_EXCHANGE_MAX_BYTES_V1 as u64
                + OFFLINE_CASH_NO_COMMIT_CLOSURE_MAX_BYTES_V1 as u64
                + TERMINAL_METADATA_LENGTH_PREFIX_HEADROOM_BYTES_V1
        );
        assert_eq!(
            OfflineCashDurableCapacityV1::MINIMUM_OUTBOX_BYTES,
            MAXIMUM_LIVE_OUTBOX_SLOT_BYTES_V1 + OUTBOX_TERMINAL_LEDGER_METADATA_HEADROOM_BYTES_V1
        );

        let receiver_tag = u8::MAX - 2;
        let payment_request = request(100, receiver_tag);
        let acceptance_intent = intent(&payment_request, receiver_tag, 100);
        let acceptance_ticket = ticket(&payment_request, &acceptance_intent, receiver_tag);
        let projected_receiver_bytes = exact_capacity_for_ticket(
            &payment_request,
            acceptance_intent,
            &acceptance_ticket,
            receiver_tag,
        );
        assert!(
            projected_receiver_bytes <= OfflineCashDurableCapacityV1::MINIMUM_INBOX_BYTES,
            "projected receiver bytes {projected_receiver_bytes} exceed the durable floor"
        );
        let mut receiver = OfflineCashAcceptanceTicketBookV1::new(
            OfflineCashDurableCapacityV1::MINIMUM_INBOX_BYTES,
        );
        receiver
            .reserve(
                verified_authorization(&payment_request, acceptance_intent, receiver_tag),
                acceptance_ticket,
            )
            .expect("complete receiver floor must admit one ticket");

        let operation_kind = if OFFLINE_CASH_PAYMENT_OUTBOX_MIN_BYTES_V1
            >= OFFLINE_CASH_REDEMPTION_OUTBOX_MIN_BYTES_V1
        {
            OfflineCashOperationKindV1::SendSplit
        } else {
            OfflineCashOperationKindV1::RedeemSplit
        };
        let reservation = OfflineCashOutboxReservationV1 {
            reservation_id: [u8::MAX; 32],
            operation_kind,
            reserved_outbox_bytes: u32::try_from(MAXIMUM_OPERATION_OUTBOX_RESERVATION_BYTES_V1)
                .expect("V1 outbox reservation fits u32"),
            issued_at_ms: u64::MAX - 1,
            expires_at_ms: u64::MAX,
        };
        let journal = OfflineCashOutgoingCandidateJournalV1::default();
        let mut outbox = OfflineCashSenderOutboxCapacityV1::new(
            OfflineCashDurableCapacityV1::MINIMUM_OUTBOX_BYTES,
        );
        outbox
            .reserve(reservation, &journal)
            .expect("complete outbox floor must admit one worst-shaped reservation");
        assert!(
            outbox.committed_outbox_bytes() <= OfflineCashDurableCapacityV1::MINIMUM_OUTBOX_BYTES
        );
    }

    fn network() -> iroha_data_model::NetworkId {
        iroha_data_model::NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"candidate-lifecycle")),
        )
    }

    fn asset() -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "xor".parse().expect("asset name"),
        )
    }

    fn account() -> AccountId {
        AccountId::new(
            KeyPair::from_seed(vec![0x44; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        )
    }

    fn asset_incarnation(tag: u8) -> AxtAssetIncarnationV1 {
        let mut bytes = [tag; 32];
        bytes[31] |= 1;
        AxtAssetIncarnationV1::try_from_bytes(bytes).expect("asset incarnation")
    }

    fn signing_key() -> SigningKey {
        SigningKey::from_bytes((&[0x21; 32]).into()).expect("P-256 key")
    }

    fn public_key(key: &SigningKey) -> OfflineCashDevicePublicKeyV1 {
        OfflineCashDevicePublicKeyV1::from_sec1_bytes(
            key.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .expect("public key")
    }

    fn sign(key: &SigningKey, bytes: &[u8]) -> OfflineCashDeviceSignatureV1 {
        let signature: Signature = key.sign(bytes);
        let signature = signature.normalize_s().unwrap_or(signature);
        OfflineCashDeviceSignatureV1::from_raw_bytes(signature.to_bytes().as_ref())
            .expect("signature")
    }

    fn request(amount: u128, tag: u8) -> OfflineCashPaymentRequestV1 {
        let key = signing_key();
        let device_public_key = public_key(&key);
        let mut credential = OfflineCashHardwareCredentialV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            credential_id: [0; 32],
            network_id: network(),
            hardware_profile_id: [0x31; 32],
            suite_id: [0x32; 32],
            firmware_policy_digest: [0x33; 32],
            policy_epoch: 1,
            lane_commitment: [0x34; 32],
            hardware_epoch_id: [0x35; 32],
            hardware_epoch_generation: 1,
            device_public_key,
            device_key_reference: offline_cash_device_key_reference_v1(&device_public_key),
            issued_at_ms: 1,
            expires_at_ms: 100_000,
            governance_signature: sign(&key, b"shape-only governance signature"),
        }
        .seal_credential_id()
        .expect("credential id");
        credential.governance_signature = sign(&key, b"shape-only governance signature");
        let asset = asset();
        let asset_incarnation = asset_incarnation(0x39);
        let mut request = OfflineCashPaymentRequestV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            release_id: [0x41; 32],
            network_id: network(),
            asset: asset.clone(),
            asset_incarnation,
            scale: 2,
            liability_pool_id: offline_cash_liability_pool_id_v1(
                &network(),
                &asset,
                asset_incarnation,
            )
            .expect("pool"),
            recipient: account(),
            amount,
            hardware_credential: credential,
            request_id: [tag; 32],
            issued_at_ms: 100,
            expires_at_ms: 10_000,
            signature: sign(&key, b"placeholder"),
        };
        request.signature = sign(
            &key,
            &request
                .canonical_signing_bytes()
                .expect("request signing bytes"),
        );
        request.validate_shape().expect("valid request");
        request
    }

    fn intent(
        request: &OfflineCashPaymentRequestV1,
        tag: u8,
        exact_amount: u128,
    ) -> OfflineCashAcceptanceIntentV1 {
        OfflineCashAcceptanceIntentV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            request_digest: request.canonical_digest().expect("request digest"),
            intent_id: [tag.wrapping_add(0x40); 32],
            exact_amount,
            sender_one_time_commitment: [tag.wrapping_add(2); 32],
        }
    }

    fn ticket(
        request: &OfflineCashPaymentRequestV1,
        intent: &OfflineCashAcceptanceIntentV1,
        tag: u8,
    ) -> OfflineCashAcceptanceTicketV1 {
        let key = signing_key();
        let mut ticket = OfflineCashAcceptanceTicketV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            network_id: request.network_id,
            request_id: request.request_id,
            request_digest: request.canonical_digest().expect("request digest"),
            acceptance_ticket_id: [tag; 32],
            asset: request.asset.clone(),
            asset_incarnation: request.asset_incarnation,
            scale: request.scale,
            intent_digest: intent
                .canonical_digest_against(request)
                .expect("intent digest"),
            exact_amount: intent.exact_amount,
            reserved_inbox_bytes: OFFLINE_CASH_ACCEPTANCE_TICKET_MIN_RESERVED_INBOX_BYTES_V1,
            recipient_one_time_key: [tag.wrapping_add(1); 32],
            hardware_profile_id: request.hardware_credential.hardware_profile_id,
            policy_epoch: request.hardware_credential.policy_epoch,
            issued_at_ms: 200,
            expires_at_ms: 9_000,
            signature: sign(&key, b"placeholder"),
        };
        ticket.signature = sign(
            &key,
            &ticket
                .canonical_signing_bytes()
                .expect("ticket signing bytes"),
        );
        ticket
            .validate_shape_against(request, intent)
            .expect("valid ticket");
        ticket
    }

    fn payment_for_consumption(
        request: &OfflineCashPaymentRequestV1,
        acceptance_intent: &OfflineCashAcceptanceIntentV1,
        acceptance_ticket: &OfflineCashAcceptanceTicketV1,
        tag: u8,
    ) -> OfflineCashPaymentV1 {
        let mut ephemeral_x25519_public_key = [0; 32];
        ephemeral_x25519_public_key[0] = 9;
        let encrypted_credit = OfflineCashEncryptedCreditEnvelopeV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            ephemeral_x25519_public_key,
            nonce: [tag.wrapping_add(0x20); OFFLINE_CASH_XCHACHA20POLY1305_NONCE_BYTES_V1],
            ciphertext_and_tag: vec![
                tag.wrapping_add(0x21);
                offline_cash_credit_opening_canonical_len_v1()
                    .expect("credit opening length")
                    + OFFLINE_CASH_XCHACHA20POLY1305_TAG_BYTES_V1
            ],
        }
        .canonical_bytes_against_recipient_key(acceptance_ticket.recipient_one_time_key)
        .expect("canonical encrypted credit");
        let commit_evidence =
            OfflineCashCommitEvidenceV1::TrustedTime(OfflineCashTrustedCommitTimeV1 {
                time_evidence_commitment: [tag.wrapping_add(0x22); 32],
            });
        let request_digest = request.canonical_digest().expect("request digest");
        let acceptance_ticket_digest = acceptance_ticket
            .canonical_digest_against(request, acceptance_intent)
            .expect("ticket digest");
        let statement = OfflineCashTransferStatementV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            lifecycle: OfflineCashLifecycleBindingV1 {
                version: OFFLINE_CASH_WIRE_VERSION_V1,
                network_id: request.network_id,
                protocol_version: super::super::OFFLINE_CASH_STATE_VERSION_V1,
                suite_id: request.hardware_credential.suite_id,
                vk_digest: [tag.wrapping_add(0x23); 32],
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
            amount: acceptance_ticket.exact_amount,
            transition_nullifier: [tag.wrapping_add(0x24); 32],
            request_digest,
            acceptance_ticket_digest,
            recipient_one_time_key: acceptance_ticket.recipient_one_time_key,
            ciphertext_commitment: [tag.wrapping_add(0x25); 32],
            commit_evidence,
        }
        .seal_credit_id()
        .expect("credit id");
        let semantic_digest = statement.canonical_digest().expect("statement digest");
        let candidate_envelope_digest = [tag.wrapping_add(0x26); 32];
        let commit_certificate = OfflineCashCommitCertificateV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            certificate_id: [0; 32],
            candidate_envelope_digest,
            lifecycle_binding_digest: statement
                .lifecycle
                .canonical_digest()
                .expect("lifecycle digest"),
            transition_nullifier: statement.transition_nullifier,
            outbox_reservation_commitment: [tag.wrapping_add(0x27); 32],
            commit_evidence,
            hardware_profile_id: statement.lifecycle.hardware_profile_id,
            policy_epoch: statement.lifecycle.policy_epoch,
            hardware_terminal_commitment: [tag.wrapping_add(0x28); 32],
        }
        .seal_certificate_id()
        .expect("commit certificate");
        let commit_certificate_digest = canonical_commit_certificate_digest_v1(&commit_certificate)
            .expect("commit certificate digest");
        let payment = OfflineCashPaymentV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            statement,
            acceptance_intent: *acceptance_intent,
            acceptance_ticket: acceptance_ticket.clone(),
            commit_certificate,
            proof: OfflineCashCommitWrapperProofV1 {
                version: OFFLINE_CASH_WIRE_VERSION_V1,
                eq_protocol_digest: [tag.wrapping_add(0x29); 32],
                ep_protocol_digest: [tag.wrapping_add(0x2A); 32],
                semantic_digest,
                candidate_envelope_digest,
                commit_certificate_digest,
                eq_deferred_audit: [tag.wrapping_add(0x2B); 32],
                ep_deferred_audit: [tag.wrapping_add(0x2C); 32],
                eq_proof: vec![tag.wrapping_add(0x2D)],
                ep_proof: vec![tag.wrapping_add(0x2E)],
                eq_history: vec![tag.wrapping_add(0x2F); OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
                ep_history: vec![tag.wrapping_add(0x30); OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
            },
            encrypted_credit,
            artifact_manifest_digest: [tag.wrapping_add(0x31); 32],
        };
        payment
            .validate_shape_against(request)
            .expect("valid payment");
        payment
    }

    fn staged_and_terminal_receiver_snapshot_bytes(
        request: &OfflineCashPaymentRequestV1,
        payment: &OfflineCashPaymentV1,
    ) -> (u64, u64) {
        let credit_id = CreditIdV1(payment.statement.lifecycle.credit_id);
        let envelope_digest = canonical_sha256_digest(CREDIT_ENVELOPE_DOMAIN, payment)
            .expect("canonical payment envelope digest");
        let stage_certificate = CreditStageCertificateV1 {
            statement: CreditStageStatementV1 {
                version: super::super::OFFLINE_CASH_STATE_VERSION_V1,
                recipient_lane: OfflineCashLaneIdV1 {
                    network_id: request.network_id,
                    device_lane_id: request.hardware_credential.lane_commitment,
                    asset: request.asset.clone(),
                    scale: request.scale,
                },
                receiver_state_commitment: [0xD1; 32],
                receiver_hardware_epoch: HardwareEpochV1 {
                    generation: u128::from(request.hardware_credential.hardware_epoch_generation),
                    epoch_id: request.hardware_credential.hardware_epoch_id,
                },
                receiver_device_policy_binding: DevicePolicyBindingV1 {
                    device_key_reference: request.hardware_credential.device_key_reference,
                    hardware_policy_id: request.hardware_credential.firmware_policy_digest,
                },
                receiver_state_nonce_commitment: [0xD2; 32],
                credit_id,
                envelope_digest,
                staged_at_ms: 300,
                journal_revision_before: 0,
                journal_revision_after: 1,
            },
            guard_bundle: vec![0xD3],
        };
        let request_digest = request.canonical_digest().expect("request digest");
        let payment_digest = payment
            .canonical_digest_against(request)
            .expect("payment digest");
        let inbox_receipt = OfflineCashInboxReceiptV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            credit_id: credit_id.0,
            receipt_commitment: offline_cash_inbox_receipt_commitment_v1(
                request.hardware_credential.lane_commitment,
                request.hardware_credential.hardware_epoch_id,
                1,
                credit_id.0,
                payment_digest,
            )
            .expect("inbox receipt commitment"),
        };
        let mut acknowledgement = OfflineCashAcknowledgementV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            request_digest,
            payment_digest,
            inbox_receipt,
            signature: sign(&signing_key(), b"placeholder acknowledgement"),
        };
        acknowledgement.signature = sign(
            &signing_key(),
            &acknowledgement
                .canonical_signing_bytes()
                .expect("acknowledgement signing bytes"),
        );
        let durable_acknowledgement =
            DurableAcknowledgementV1::from_acknowledgement(acknowledgement, request, payment)
                .expect("durable acknowledgement");
        let staged = StagedCreditV1 {
            request: request.clone(),
            payment: payment.clone(),
            envelope_digest,
            stage_certificate: stage_certificate.clone(),
        };
        let receipt = AcceptedPaymentReceiptV1 {
            credit_id,
            envelope_digest,
            request: request.clone(),
            payment: payment.clone(),
            stage_certificate,
            durable_acknowledgement,
        };
        let pending = BTreeMap::from([(credit_id, staged)]);
        let receipts = BTreeMap::from([(credit_id, receipt)]);
        let staged_usage = receiver_snapshot_capacity_usage_v1(
            &pending,
            &receipts,
            &ExactConsumedCreditIndex::empty(),
        )
        .expect("staged receiver snapshot usage");
        assert_eq!(staged_usage.retained_bytes, 0);
        assert!(staged_usage.live_bytes > 0);

        let consumed = ExactConsumedCreditIndex::from_records(&[ConsumedCreditRecordV1 {
            credit_id,
            envelope_digest,
        }])
        .expect("terminal consumed-credit index");
        let terminal_usage =
            receiver_snapshot_capacity_usage_v1(&BTreeMap::new(), &receipts, &consumed)
                .expect("terminal receiver snapshot usage");
        assert_eq!(terminal_usage.live_bytes, 0);
        assert!(terminal_usage.retained_bytes > 0);
        (staged_usage.live_bytes, terminal_usage.retained_bytes)
    }

    fn outgoing_state(balance: u128, logical_sequence: u128, nonce_tag: u8) -> OfflineCashStateV1 {
        let network = network();
        let asset = asset();
        let asset_incarnation = asset_incarnation(0x39);
        let lane = super::super::OfflineCashLaneIdV1 {
            network_id: network,
            device_lane_id: [0xA1; 32],
            asset: asset.clone(),
            scale: 2,
        };
        OfflineCashStateV1::build(
            super::super::OfflineCashStateContextV1 {
                protocol_version: super::super::OFFLINE_CASH_STATE_VERSION_V1,
                suite_id: [0xA2; 32],
                vk_digest: [0xA3; 32],
                release_id: [0xA4; 32],
                asset_incarnation,
                hardware_profile_id: [0xA5; 32],
                policy_epoch: 1,
            },
            offline_cash_liability_pool_id_v1(&network, &asset, asset_incarnation)
                .expect("liability pool"),
            lane,
            balance,
            logical_sequence,
            super::super::HardwareEpochV1 {
                generation: 1,
                epoch_id: [0xA6; 32],
            },
            super::super::DevicePolicyBindingV1 {
                device_key_reference: [0xA7; 32],
                hardware_policy_id: [0xA8; 32],
            },
            [nonce_tag; 32],
            OfflineCashPastaStateCommitmentV1::ZERO,
        )
        .expect("aggregate state")
    }

    fn prepared_redemption_for_capacity(reservation_id: DigestV1) -> PreparedOutgoingCandidateV1 {
        let predecessor = outgoing_state(10, 0, 0xA9);
        let successor = outgoing_state(7, 1, 0xAA);
        let amount = 3;
        let terminal_nullifier = [0xAB; 32];
        let redemption_commitment = [0xAC; 32];
        let commit_evidence =
            OfflineCashCommitEvidenceV1::TrustedTime(OfflineCashTrustedCommitTimeV1 {
                time_evidence_commitment: [0xAD; 32],
            });
        let outbox_reservation = OfflineCashOutboxReservationV1 {
            reservation_id,
            operation_kind: OfflineCashOperationKindV1::RedeemSplit,
            reserved_outbox_bytes: OFFLINE_CASH_REDEMPTION_OUTBOX_MIN_BYTES_V1,
            issued_at_ms: 100,
            expires_at_ms: 1_000,
        };
        let lifecycle = OfflineCashLifecycleBindingV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            network_id: predecessor.lane.network_id,
            protocol_version: predecessor.protocol_version,
            suite_id: predecessor.suite_id,
            vk_digest: predecessor.vk_digest,
            release_id: predecessor.release_id,
            asset: predecessor.lane.asset.clone(),
            asset_incarnation: predecessor.asset_incarnation,
            scale: predecessor.lane.scale,
            liability_pool_id: predecessor.liability_pool_id,
            hardware_profile_id: predecessor.hardware_profile_id,
            policy_epoch: predecessor.policy_epoch,
            operation_kind: OfflineCashOperationKindV1::RedeemSplit,
            request_id: [0; 32],
            acceptance_ticket_id: [0; 32],
            credit_id: [0; 32],
            ciphertext_digest: [0; 32],
        };
        let beneficiary = account();
        let redemption_statement = OfflineCashRedemptionStatementV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            lifecycle: lifecycle.clone(),
            amount,
            beneficiary: beneficiary.clone(),
            terminal_nullifier,
            redemption_commitment,
            redemption_id: [1; 32],
            commit_evidence,
        }
        .seal_redemption_id()
        .expect("redemption id");
        let effect_digest = redemption_statement
            .canonical_digest()
            .expect("redemption statement digest");
        let proof_statement = TransitionProofStatementV1 {
            version: super::super::OFFLINE_CASH_STATE_VERSION_V1,
            protocol_version: predecessor.protocol_version,
            predecessor_suite_id: predecessor.suite_id,
            predecessor_vk_digest: predecessor.vk_digest,
            successor_suite_id: successor.suite_id,
            successor_vk_digest: successor.vk_digest,
            kind: super::super::OfflineCashTransitionKindV1::RedeemSplit,
            amount,
            mint_finality_semantic_digest: [0; 32],
            mint_finality_proof_binding_digest: [0; 32],
            peer_credit_id: [0; 32],
            peer_recipient_lane_id: [0; 32],
            lifecycle_binding_digest: lifecycle.canonical_digest().expect("lifecycle digest"),
            precommit_binding_digest: outbox_reservation
                .canonical_commitment()
                .expect("reservation commitment"),
            suite_upgrade_authorization_digest: [0; 32],
            release_id: predecessor.release_id,
            asset_incarnation: predecessor.asset_incarnation,
            liability_pool_id: predecessor.liability_pool_id,
            hardware_profile_id: predecessor.hardware_profile_id,
            policy_epoch: predecessor.policy_epoch,
            lane: predecessor.lane.clone(),
            predecessor_commitment: predecessor.state_commitment,
            successor_commitment: successor.state_commitment,
            predecessor_sequence: predecessor.logical_sequence,
            successor_sequence: successor.logical_sequence,
            predecessor_epoch: predecessor.hardware_epoch,
            successor_epoch: successor.hardware_epoch,
            predecessor_device_policy_binding: predecessor.device_policy_binding,
            successor_device_policy_binding: successor.device_policy_binding,
            predecessor_state_nonce_commitment: predecessor.state_nonce_commitment,
            successor_state_nonce_commitment: successor.state_nonce_commitment,
            journal_revision_before: 0,
            journal_revision_after: 1,
            effect_digest,
        };
        let state_transition_digest = proof_statement.digest().expect("transition digest");
        PreparedOutgoingCandidateV1::redemption(
            predecessor,
            successor,
            state_transition_digest,
            PreparedRedemptionMaterialV1 {
                proof_statement,
                transport_semantic_digest: [0xAE; 32],
                amount,
                beneficiary,
                terminal_nullifier,
                redemption_commitment,
                commit_evidence,
                outbox_reservation,
                sealed_transition_inputs: vec![0xAF],
                sealed_recovery_seeds: vec![0xB0],
                normalized_guard_statement_digest: [0xB1; 32],
            },
        )
        .expect("prepared redemption")
    }

    fn outgoing_candidate_proof(
        prepared: &PreparedOutgoingCandidateV1,
    ) -> OfflineCashPairedProofV1 {
        OfflineCashPairedProofV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            eq_protocol_digest: [0xB2; 32],
            ep_protocol_digest: [0xB3; 32],
            semantic_digest: prepared.transport_semantic_digest,
            guard_eq_credential_audit: [0xB4; 32],
            guard_ep_credential_audit: [0xB5; 32],
            eq_deferred_audit: [0xB6; 32],
            ep_deferred_audit: [0xB7; 32],
            eq_proof: vec![0xB8],
            ep_proof: vec![0xB9],
            eq_history: vec![0xBA; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
            ep_history: vec![0xBB; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
        }
    }

    fn outgoing_commit_certificate(
        candidate: &PersistedOutgoingCandidateV1,
    ) -> OfflineCashCommitCertificateV1 {
        let prepared = &candidate.prepared;
        OfflineCashCommitCertificateV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            certificate_id: [0; 32],
            candidate_envelope_digest: candidate.candidate_envelope_digest,
            lifecycle_binding_digest: prepared
                .lifecycle()
                .canonical_digest()
                .expect("lifecycle digest"),
            transition_nullifier: prepared.transition_nullifier(),
            outbox_reservation_commitment: prepared
                .outbox_reservation
                .canonical_commitment()
                .expect("reservation commitment"),
            commit_evidence: projection_commit_evidence(&prepared.projection),
            hardware_profile_id: prepared.lifecycle().hardware_profile_id,
            policy_epoch: prepared.lifecycle().policy_epoch,
            hardware_terminal_commitment: [0xBC; 32],
        }
        .seal_certificate_id()
        .expect("commit certificate")
    }

    fn exact_capacity_for_ticket(
        request: &OfflineCashPaymentRequestV1,
        intent: OfflineCashAcceptanceIntentV1,
        ticket: &OfflineCashAcceptanceTicketV1,
        authorization_tag: u8,
    ) -> u64 {
        let mut probe = OfflineCashAcceptanceTicketBookV1::new(u64::MAX);
        probe
            .reserve(
                verified_authorization(request, intent, authorization_tag),
                ticket.clone(),
            )
            .expect("capacity probe reservation");
        probe.committed_inbox_bytes()
    }

    fn verified_authorization(
        request: &OfflineCashPaymentRequestV1,
        intent: OfflineCashAcceptanceIntentV1,
        tag: u8,
    ) -> VerifiedOfflineCashAcceptanceIntentAuthorizationV1 {
        let authorization = intent_authorization(request, intent, tag);
        VerifiedOfflineCashAcceptanceIntentAuthorizationV1 {
            request: request.clone(),
            intent,
            proof_envelope_digest: authorization
                .canonical_digest_against(request)
                .expect("authorization digest"),
        }
    }

    fn intent_authorization(
        request: &OfflineCashPaymentRequestV1,
        intent: OfflineCashAcceptanceIntentV1,
        tag: u8,
    ) -> OfflineCashAcceptanceIntentAuthorizationV1 {
        let statement =
            iroha_data_model::offline::OfflineCashAcceptanceIntentAuthorizationStatementV1 {
                version: OFFLINE_CASH_WIRE_VERSION_V1,
                intent,
                release_id: request.release_id,
                suite_id: request.hardware_credential.suite_id,
                vk_digest: [0x81; 32],
                artifact_manifest_digest: [0x82; 32],
            };
        let semantic_digest = statement
            .canonical_digest_against(request)
            .expect("authorization statement digest");
        let authorization = OfflineCashAcceptanceIntentAuthorizationV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            statement,
            proof: OfflineCashPairedProofV1 {
                version: OFFLINE_CASH_WIRE_VERSION_V1,
                eq_protocol_digest: [tag.wrapping_add(0x41); 32],
                ep_protocol_digest: [tag.wrapping_add(0x61); 32],
                semantic_digest,
                guard_eq_credential_audit: [tag.wrapping_add(0x42); 32],
                guard_ep_credential_audit: [tag.wrapping_add(0x62); 32],
                eq_deferred_audit: [tag.wrapping_add(0x43); 32],
                ep_deferred_audit: [tag.wrapping_add(0x63); 32],
                eq_proof: vec![tag.wrapping_add(0x44)],
                ep_proof: vec![tag.wrapping_add(0x64)],
                eq_history: vec![tag.wrapping_add(0x45); OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
                ep_history: vec![tag.wrapping_add(0x65); OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
            },
        };
        authorization
            .validate_shape_against(request)
            .expect("authorization shape");
        authorization
    }

    fn no_commit_closure(
        request: &OfflineCashPaymentRequestV1,
        intent: &OfflineCashAcceptanceIntentV1,
        ticket: &OfflineCashAcceptanceTicketV1,
        intent_authorization_digest: DigestV1,
        tag: u8,
    ) -> OfflineCashNoCommitClosureV1 {
        let intent_authorization =
            intent_authorization(request, *intent, intent_authorization_digest[0]);
        let intent_authorization_digest = intent_authorization
            .canonical_digest_against(request)
            .expect("authorization digest");
        let statement = OfflineCashNoCommitClosureStatementV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            release_id: request.release_id,
            suite_id: request.hardware_credential.suite_id,
            vk_digest: [0x81; 32],
            artifact_manifest_digest: [0x82; 32],
            sender_hardware_binding_commitment: [0x83; 32],
            request_id: request.request_id,
            request_digest: request.canonical_digest().expect("request digest"),
            acceptance_ticket_id: ticket.acceptance_ticket_id,
            ticket_digest: ticket
                .canonical_digest_against(request, intent)
                .expect("ticket digest"),
            intent_authorization_digest,
            intent_digest: intent
                .canonical_digest_against(request)
                .expect("intent digest"),
            exact_amount: intent.exact_amount,
            sender_one_time_commitment: intent.sender_one_time_commitment,
            recovery_id: [tag.wrapping_add(0x10); 32],
            cancellation_nullifier: [tag.wrapping_add(0x20); 32],
            equivalent_delivery_slot_commitment: [tag.wrapping_add(0x30); 32],
        };
        let semantic_digest = statement
            .canonical_digest()
            .expect("closure statement digest");
        let closure = OfflineCashNoCommitClosureV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            statement,
            request: request.clone(),
            intent_authorization,
            acceptance_ticket: ticket.clone(),
            proof: OfflineCashPairedProofV1 {
                version: OFFLINE_CASH_WIRE_VERSION_V1,
                eq_protocol_digest: [0x91; 32],
                ep_protocol_digest: [0x92; 32],
                semantic_digest,
                guard_eq_credential_audit: [0x93; 32],
                guard_ep_credential_audit: [0x94; 32],
                eq_deferred_audit: [0x95; 32],
                ep_deferred_audit: [0x96; 32],
                eq_proof: vec![0x97],
                ep_proof: vec![0x98],
                eq_history: vec![0x99; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
                ep_history: vec![0x9A; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
            },
        };
        closure.validate_shape().expect("closure shape");
        closure
    }

    fn reseal_no_commit_closure_statement(closure: &mut OfflineCashNoCommitClosureV1) {
        closure.proof.semantic_digest = closure
            .statement
            .canonical_digest()
            .expect("closure statement digest");
        closure.validate_shape().expect("closure shape");
    }

    #[test]
    fn acceptance_authorization_decision_requires_distinct_decided_parities() {
        assert!(
            OfflineCashAcceptanceIntentAuthorizationDecisionV1::authenticated(
                [1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32], [7; 32],
            )
            .is_ok()
        );
        assert!(
            OfflineCashAcceptanceIntentAuthorizationDecisionV1::authenticated(
                [1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32], [6; 32],
            )
            .is_err()
        );
        assert!(
            OfflineCashAcceptanceIntentAuthorizationDecisionV1::authenticated(
                [1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [0; 32], [7; 32],
            )
            .is_err()
        );
    }

    #[test]
    fn durable_metadata_is_pre_reserved_and_restore_meters_are_exact() {
        let payment_request = request(9, 21);
        let acceptance_intent = intent(&payment_request, 21, 9);
        let acceptance_ticket = ticket(&payment_request, &acceptance_intent, 21);
        let exact_capacity =
            exact_capacity_for_ticket(&payment_request, acceptance_intent, &acceptance_ticket, 21);
        assert!(exact_capacity >= RECEIVER_SNAPSHOT_ENTRY_MAX_BYTES_V1);

        let mut short = OfflineCashAcceptanceTicketBookV1::new(exact_capacity - 1);
        assert_eq!(
            short.reserve(
                verified_authorization(&payment_request, acceptance_intent, 21),
                acceptance_ticket.clone(),
            ),
            Err(OfflineCashStateErrorV1::ReceiverCapacityExhausted)
        );
        assert_eq!(short.committed_inbox_bytes(), 0);
        assert!(short.tickets.is_empty());
        assert!(short.intent_ticket_decisions.is_empty());

        let mut payment_book = OfflineCashAcceptanceTicketBookV1::new(exact_capacity);
        payment_book
            .reserve(
                verified_authorization(&payment_request, acceptance_intent, 21),
                acceptance_ticket.clone(),
            )
            .expect("exact-capacity reservation");
        assert_eq!(payment_book.available_inbox_bytes(), 0);
        assert!(payment_book.retained_metadata_bytes() > 0);
        assert!(payment_book.reserved_terminal_metadata_bytes() > 0);
        payment_book
            .validate_recovered()
            .expect("issued ticket meters");

        let mut tampered = payment_book.clone();
        tampered.retained_metadata_bytes += 1;
        assert_eq!(
            tampered.validate_recovered(),
            Err(OfflineCashStateErrorV1::SnapshotIntegrity)
        );
        let mut tampered = payment_book.clone();
        tampered.reserved_terminal_metadata_bytes -= 1;
        assert_eq!(
            tampered.validate_recovered(),
            Err(OfflineCashStateErrorV1::SnapshotIntegrity)
        );
        let mut tampered = payment_book.clone();
        tampered.committed_inbox_bytes -= 1;
        assert_eq!(
            tampered.validate_recovered(),
            Err(OfflineCashStateErrorV1::SnapshotIntegrity)
        );
        let mut tampered = payment_book.clone();
        tampered.receiver_snapshot_live_bytes += 1;
        assert_eq!(
            tampered.validate_recovered(),
            Err(OfflineCashStateErrorV1::SnapshotIntegrity)
        );
        let mut tampered = payment_book.clone();
        tampered.receiver_snapshot_retained_bytes += 1;
        assert_eq!(
            tampered.validate_recovered(),
            Err(OfflineCashStateErrorV1::SnapshotIntegrity)
        );

        let payment_digest = [0xD1; 32];
        let entry = payment_book
            .tickets
            .get_mut(&acceptance_ticket.acceptance_ticket_id)
            .expect("live ticket");
        entry.consumed_payment_digest = Some(payment_digest);
        entry.consumed_amount = Some(acceptance_ticket.exact_amount);
        payment_book
            .release_folded(acceptance_ticket.acceptance_ticket_id, payment_digest)
            .expect("pre-reserved payment terminal metadata");
        assert_eq!(payment_book.reserved_terminal_metadata_bytes(), 0);
        assert_eq!(
            payment_book.committed_inbox_bytes(),
            payment_book.retained_metadata_bytes()
        );
        payment_book
            .validate_recovered()
            .expect("folded payment meters");

        let mut closure_book = OfflineCashAcceptanceTicketBookV1::new(exact_capacity);
        closure_book
            .reserve(
                verified_authorization(&payment_request, acceptance_intent, 21),
                acceptance_ticket.clone(),
            )
            .expect("exact-capacity closure reservation");
        let closure = no_commit_closure(
            &payment_request,
            &acceptance_intent,
            &acceptance_ticket,
            [21; 32],
            21,
        );
        let verified = VerifiedOfflineCashNoCommitClosureV1::verify(
            closure,
            &AcceptingNoCommitClosureVerifier,
        )
        .expect("authenticated closure");
        let committed_before = closure_book.committed_inbox_bytes();
        closure_book
            .begin_authenticated_no_commit_recovery(&verified)
            .expect("pre-reserved pending indexes");
        assert_eq!(closure_book.committed_inbox_bytes(), committed_before);
        closure_book
            .close_authenticated_no_commit_recovery(verified)
            .expect("pre-reserved permanent tombstone");
        assert_eq!(closure_book.reserved_terminal_metadata_bytes(), 0);
        assert_eq!(
            closure_book.committed_inbox_bytes(),
            closure_book.retained_metadata_bytes()
        );
        closure_book
            .validate_recovered()
            .expect("closed ticket meters");
    }

    #[test]
    fn consumed_receiver_ticket_can_release_with_no_residual_capacity() {
        let payment_request = request(9, 0x31);
        let acceptance_intent = intent(&payment_request, 0x31, 9);
        let acceptance_ticket = ticket(&payment_request, &acceptance_intent, 0x31);
        let exact_capacity = exact_capacity_for_ticket(
            &payment_request,
            acceptance_intent,
            &acceptance_ticket,
            0x31,
        );
        let mut book = OfflineCashAcceptanceTicketBookV1::new(exact_capacity);
        book.reserve(
            verified_authorization(&payment_request, acceptance_intent, 0x31),
            acceptance_ticket.clone(),
        )
        .expect("reserve the complete receiver workflow");
        assert_eq!(book.available_inbox_bytes(), 0);

        let payment = payment_for_consumption(
            &payment_request,
            &acceptance_intent,
            &acceptance_ticket,
            0x31,
        );
        let payment_digest = payment
            .canonical_digest_against(&payment_request)
            .expect("payment digest");
        assert_eq!(
            book.consume(&payment_request, &payment),
            Ok(AcceptanceTicketUseOutcomeV1::Consumed)
        );
        assert_eq!(
            book.available_inbox_bytes(),
            0,
            "consumption must keep the complete terminal workflow pre-reserved"
        );
        let (staged_snapshot_bytes, terminal_snapshot_bytes) =
            staged_and_terminal_receiver_snapshot_bytes(&payment_request, &payment);
        let maximum_committed_bytes = book.committed_inbox_bytes();
        book.reconcile_receiver_snapshot_usage(staged_snapshot_bytes, 0, maximum_committed_bytes)
            .expect("materialize the exact staged payment and ACK-replay projection");
        assert_eq!(book.available_inbox_bytes(), 0);

        let blocked_intent = intent(&payment_request, 0x32, 9);
        let blocked_ticket = ticket(&payment_request, &blocked_intent, 0x32);
        let book_before_blocked_reservation =
            norito::encode_canonical(&book).expect("encode pre-failure ticket book");
        assert_eq!(
            book.reserve(
                verified_authorization(&payment_request, blocked_intent, 0x32),
                blocked_ticket,
            ),
            Err(OfflineCashStateErrorV1::ReceiverCapacityExhausted),
            "no unrelated admission may consume the already-reserved terminal headroom"
        );
        assert_eq!(book.available_inbox_bytes(), 0);
        assert_eq!(
            norito::encode_canonical(&book).expect("encode post-failure ticket book"),
            book_before_blocked_reservation,
            "failed capacity admission must not mutate receiver bytes"
        );

        let committed_before_release = book.committed_inbox_bytes();
        book = book
            .receiver_snapshot_folded_successor(
                0,
                terminal_snapshot_bytes,
                &[(acceptance_ticket.acceptance_ticket_id, payment_digest)],
            )
            .expect("fold the staged payment into its exact terminal receiver projection");
        assert!(book.committed_inbox_bytes() < committed_before_release);
        assert_eq!(book.reserved_terminal_metadata_bytes(), 0);
        assert!(book.available_inbox_bytes() > 0);
        book.validate_recovered()
            .expect("released receiver snapshot");

        let released = book.clone();
        book = book
            .receiver_snapshot_folded_successor(
                0,
                terminal_snapshot_bytes,
                &[(acceptance_ticket.acceptance_ticket_id, payment_digest)],
            )
            .expect("byte-identical fold completion retry");
        assert_eq!(book, released);
    }

    #[test]
    fn receiver_snapshot_reconciliation_matches_sequential_exact_metering() {
        let mut book = OfflineCashAcceptanceTicketBookV1::new(u64::MAX);
        let mut folded_tickets = Vec::new();
        for tag in 1_u8..=16 {
            let request = request(u128::from(tag), tag);
            let intent = intent(&request, tag, u128::from(tag));
            let ticket = ticket(&request, &intent, tag);
            book.reserve(
                verified_authorization(&request, intent, tag),
                ticket.clone(),
            )
            .expect("capacity-backed ticket");
            let payment_digest = [tag.wrapping_add(0x80); 32];
            let entry = book
                .tickets
                .get_mut(&ticket.acceptance_ticket_id)
                .expect("reserved ticket");
            entry.consumed_payment_digest = Some(payment_digest);
            entry.consumed_amount = Some(ticket.exact_amount);
            folded_tickets.push((ticket.acceptance_ticket_id, payment_digest));
        }

        let maximum_committed_bytes = book.committed_inbox_bytes();
        let mut sequential = book.clone();
        sequential
            .reconcile_receiver_snapshot_usage(0, 0, maximum_committed_bytes)
            .expect("materialized payments fit pre-reserved capacity");
        for &(ticket_id, payment_digest) in &folded_tickets {
            sequential
                .release_folded(ticket_id, payment_digest)
                .expect("sequential release");
        }
        sequential
            .reconcile_receiver_snapshot_usage(0, 0, maximum_committed_bytes)
            .expect("sequential final snapshot usage");

        let reconciled = book
            .receiver_snapshot_folded_successor(0, 0, &folded_tickets)
            .expect("atomic release reconciliation");
        assert_eq!(reconciled, sequential);
        reconciled
            .validate_recovered()
            .expect("reconciled meters remain exact");

        let mut invalid_tickets = folded_tickets.clone();
        invalid_tickets[8].1 = [0xFF; 32];
        assert_eq!(
            book.receiver_snapshot_folded_successor(0, 0, &invalid_tickets),
            Err(OfflineCashStateErrorV1::InvalidAcceptanceTicket)
        );
    }

    #[test]
    fn receiver_snapshot_reconciliation_matches_sequential_near_capacity() {
        let mut book = OfflineCashAcceptanceTicketBookV1::new(u64::MAX);
        let mut folded_tickets = Vec::new();
        for tag in 1_u8..=8 {
            let request = request(u128::from(tag), tag);
            let intent = intent(&request, tag, u128::from(tag));
            let ticket = ticket(&request, &intent, tag);
            book.reserve(
                verified_authorization(&request, intent, tag),
                ticket.clone(),
            )
            .expect("capacity-backed ticket");
            let payment_digest = [tag.wrapping_add(0x80); 32];
            let entry = book
                .tickets
                .get_mut(&ticket.acceptance_ticket_id)
                .expect("reserved ticket");
            entry.consumed_payment_digest = Some(payment_digest);
            entry.consumed_amount = Some(ticket.exact_amount);
            if tag <= 4 {
                folded_tickets.push((ticket.acceptance_ticket_id, payment_digest));
            }
        }

        let precommitted_ceiling = book.committed_inbox_bytes();
        book.total_inbox_bytes = precommitted_ceiling;
        assert_eq!(book.available_inbox_bytes(), 0);

        let mut final_shape = book.clone();
        for &(ticket_id, payment_digest) in &folded_tickets {
            final_shape
                .release_folded_unmetered(ticket_id, payment_digest)
                .expect("probe final ticket shape");
        }
        let live_snapshot_bytes = final_shape
            .receiver_snapshot_live_capacity_bytes()
            .expect("remaining live projection capacity");
        assert!(live_snapshot_bytes > 0);

        let mut sequential = book.clone();
        sequential
            .reconcile_receiver_snapshot_usage(live_snapshot_bytes, 0, precommitted_ceiling)
            .expect("install nonzero live projection");
        for &(ticket_id, payment_digest) in &folded_tickets {
            sequential
                .release_folded(ticket_id, payment_digest)
                .expect("sequential release");
        }
        let retained_room = precommitted_ceiling
            .checked_sub(sequential.committed_inbox_bytes())
            .expect("released capacity");
        assert!(retained_room > 1);
        let retained_snapshot_bytes = retained_room - 1;
        sequential
            .reconcile_receiver_snapshot_usage(
                live_snapshot_bytes,
                retained_snapshot_bytes,
                precommitted_ceiling,
            )
            .expect("install near-ceiling retained projection");

        let reconciled = book
            .receiver_snapshot_folded_successor(
                live_snapshot_bytes,
                retained_snapshot_bytes,
                &folded_tickets,
            )
            .expect("atomic release reconciliation");

        assert_eq!(reconciled, sequential);
        assert_eq!(reconciled.available_inbox_bytes(), 1);
        assert!(reconciled.retained_metadata_bytes() > retained_snapshot_bytes);
        assert!(reconciled.reserved_terminal_metadata_bytes() > 0);
        reconciled
            .validate_recovered_with_snapshot_usage(live_snapshot_bytes, retained_snapshot_bytes)
            .expect("exact nonzero snapshot meters");
    }

    #[test]
    fn receiver_snapshot_reconciliation_preserves_terminal_compaction_order() {
        let payment_request = request(1, 0x51);
        let mut book = OfflineCashAcceptanceTicketBookV1::new(u64::MAX);
        let mut folded_tickets = Vec::new();
        for tag in 1_u8..=80 {
            let intent = intent(&payment_request, tag, 1);
            let ticket = ticket(&payment_request, &intent, tag);
            book.reserve(
                verified_authorization(&payment_request, intent, tag),
                ticket.clone(),
            )
            .expect("distinct reusable-request ticket");
            let payment_digest = [tag.wrapping_add(0x80); 32];
            let entry = book
                .tickets
                .get_mut(&ticket.acceptance_ticket_id)
                .expect("reserved ticket");
            entry.consumed_payment_digest = Some(payment_digest);
            entry.consumed_amount = Some(ticket.exact_amount);
            folded_tickets.push((ticket.acceptance_ticket_id, payment_digest));
        }

        let mut at_retry_horizon = book;
        for &(ticket_id, payment_digest) in &folded_tickets[..64] {
            at_retry_horizon
                .release_folded_unmetered(ticket_id, payment_digest)
                .expect("seed retry horizon");
        }
        let meters =
            acceptance_ticket_capacity_meters_v1(&at_retry_horizon).expect("retry-horizon meters");
        at_retry_horizon.committed_inbox_bytes = meters.committed_inbox_bytes;
        at_retry_horizon.retained_metadata_bytes = meters.retained_metadata_bytes;
        at_retry_horizon.reserved_terminal_metadata_bytes = meters.reserved_terminal_metadata_bytes;
        assert_eq!(at_retry_horizon.terminal_retry_order.len(), 64);

        let mut sequential = at_retry_horizon.clone();
        for &(ticket_id, payment_digest) in &folded_tickets[64..] {
            sequential
                .release_folded(ticket_id, payment_digest)
                .expect("sequential compaction");
        }
        let reconciled = at_retry_horizon
            .receiver_snapshot_folded_successor(0, 0, &folded_tickets[64..])
            .expect("atomic compaction reconciliation");
        assert_eq!(reconciled, sequential);
        assert_eq!(reconciled.compacted_terminal_count(), 16);
        assert_eq!(reconciled.terminal_retry_order.len(), 64);
        reconciled
            .validate_recovered()
            .expect("compacted reconciled meters remain exact");
    }

    #[test]
    fn authenticated_no_commit_recovery_is_two_step_idempotent_and_conflict_closed() {
        let single = request(7, 1);
        let first_intent = intent(&single, 1, 7);
        let first_ticket = ticket(&single, &first_intent, 1);
        let first_capacity = exact_capacity_for_ticket(&single, first_intent, &first_ticket, 1);
        let mut book = OfflineCashAcceptanceTicketBookV1::new(first_capacity);
        book.reserve(
            verified_authorization(&single, first_intent, 1),
            first_ticket.clone(),
        )
        .expect("initial ticket reservation");
        assert_eq!(book.available_inbox_bytes(), 0);

        let closure = no_commit_closure(&single, &first_intent, &first_ticket, [1; 32], 1);
        assert!(
            VerifiedOfflineCashNoCommitClosureV1::verify(
                closure.clone(),
                &RejectingNoCommitClosureVerifier,
            )
            .is_err()
        );
        let verified = VerifiedOfflineCashNoCommitClosureV1::verify(
            closure.clone(),
            &AcceptingNoCommitClosureVerifier,
        )
        .expect("authenticated closure");
        assert_eq!(
            book.begin_authenticated_no_commit_recovery(&verified),
            Ok(AcceptanceTicketNoCommitRecoveryOutcomeV1::Begun)
        );
        assert_eq!(book.available_inbox_bytes(), 0);
        assert_eq!(book.committed_inbox_bytes(), first_capacity);
        book.validate_recovered().expect("pending snapshot");
        assert_eq!(
            book.begin_authenticated_no_commit_recovery(&verified),
            Ok(AcceptanceTicketNoCommitRecoveryOutcomeV1::AlreadyPending)
        );

        let conflicting = VerifiedOfflineCashNoCommitClosureV1::verify(
            no_commit_closure(&single, &first_intent, &first_ticket, [1; 32], 2),
            &AcceptingNoCommitClosureVerifier,
        )
        .expect("independently valid conflicting closure");
        assert_eq!(
            book.begin_authenticated_no_commit_recovery(&conflicting),
            Err(OfflineCashStateErrorV1::InvalidRecoveryMaterial)
        );
        assert_eq!(
            book.close_authenticated_no_commit_recovery(conflicting),
            Err(OfflineCashStateErrorV1::InvalidRecoveryMaterial)
        );
        assert_eq!(book.available_inbox_bytes(), 0);

        assert_eq!(
            book.close_authenticated_no_commit_recovery(verified),
            Ok(AcceptanceTicketNoCommitClosureOutcomeV1::Closed)
        );
        assert_eq!(book.committed_inbox_bytes(), book.retained_metadata_bytes());
        assert_eq!(book.reserved_terminal_metadata_bytes(), 0);
        assert!(book.available_inbox_bytes() > 0);
        book.validate_recovered().expect("closed snapshot");

        let encoded = norito::encode_canonical(&book).expect("encode ticket book");
        let recovered: OfflineCashAcceptanceTicketBookV1 =
            norito::decode_canonical(&encoded).expect("decode ticket book");
        assert_eq!(recovered, book);
        recovered
            .validate_recovered()
            .expect("validate recovered book");
        let mut tampered = recovered.clone();
        tampered
            .closed_no_commit_tombstones
            .get_mut(&first_ticket.acceptance_ticket_id)
            .expect("closed tombstone")
            .closure_digest = [0; 32];
        assert_eq!(
            tampered.validate_recovered(),
            Err(OfflineCashStateErrorV1::SnapshotIntegrity)
        );
        let mut tampered = recovered.clone();
        tampered
            .ticket_intent_ids
            .remove(&first_ticket.acceptance_ticket_id);
        assert_eq!(
            tampered.validate_recovered(),
            Err(OfflineCashStateErrorV1::SnapshotIntegrity)
        );
        let mut tampered = recovered.clone();
        tampered.no_commit_recovery_ticket_ids.clear();
        assert_eq!(
            tampered.validate_recovered(),
            Err(OfflineCashStateErrorV1::SnapshotIntegrity)
        );

        let replay = VerifiedOfflineCashNoCommitClosureV1::verify(
            closure,
            &AcceptingNoCommitClosureVerifier,
        )
        .expect("replayed authenticated closure");
        assert_eq!(
            book.begin_authenticated_no_commit_recovery(&replay),
            Ok(AcceptanceTicketNoCommitRecoveryOutcomeV1::AlreadyClosed)
        );
        assert_eq!(
            book.close_authenticated_no_commit_recovery(replay),
            Ok(AcceptanceTicketNoCommitClosureOutcomeV1::AlreadyClosed)
        );
        let closed_available = book.available_inbox_bytes();
        assert!(matches!(
            book.reserve(
                verified_authorization(&single, first_intent, 1),
                first_ticket.clone(),
            ),
            Ok(AcceptanceTicketReservationOutcomeV1::AlreadyReserved(decision))
                if decision.ticket == first_ticket
        ));
        let conflicting_ticket = ticket(&single, &first_intent, 3);
        assert_eq!(
            book.reserve(
                verified_authorization(&single, first_intent, 1),
                conflicting_ticket,
            ),
            Err(OfflineCashStateErrorV1::InvalidAcceptanceTicket)
        );
        assert_eq!(
            book.reserve(
                verified_authorization(&single, first_intent, 4),
                first_ticket,
            ),
            Err(OfflineCashStateErrorV1::InvalidAcceptanceTicket)
        );

        let second_intent = intent(&single, 2, 7);
        let second_ticket = ticket(&single, &second_intent, 2);
        let second_capacity = exact_capacity_for_ticket(&single, second_intent, &second_ticket, 2);
        book.total_inbox_bytes = book
            .committed_inbox_bytes()
            .checked_add(second_capacity)
            .expect("expanded physical inbox capacity");
        assert!(matches!(
            book.reserve(
                verified_authorization(&single, second_intent, 2),
                second_ticket.clone(),
            ),
            Ok(AcceptanceTicketReservationOutcomeV1::Reserved(_))
        ));
        let mut reused_recovery =
            no_commit_closure(&single, &second_intent, &second_ticket, [2; 32], 3);
        reused_recovery.statement.recovery_id = [0x11; 32];
        reseal_no_commit_closure_statement(&mut reused_recovery);
        let reused_recovery = VerifiedOfflineCashNoCommitClosureV1::verify(
            reused_recovery,
            &AcceptingNoCommitClosureVerifier,
        )
        .expect("independently authenticated reused recovery identity");
        assert_eq!(
            book.begin_authenticated_no_commit_recovery(&reused_recovery),
            Err(OfflineCashStateErrorV1::InvalidRecoveryMaterial)
        );
        let mut reused_cancellation =
            no_commit_closure(&single, &second_intent, &second_ticket, [2; 32], 4);
        reused_cancellation.statement.cancellation_nullifier = [0x21; 32];
        reseal_no_commit_closure_statement(&mut reused_cancellation);
        let reused_cancellation = VerifiedOfflineCashNoCommitClosureV1::verify(
            reused_cancellation,
            &AcceptingNoCommitClosureVerifier,
        )
        .expect("independently authenticated reused cancellation identity");
        assert_eq!(
            book.begin_authenticated_no_commit_recovery(&reused_cancellation),
            Err(OfflineCashStateErrorV1::InvalidRecoveryMaterial)
        );
        assert_eq!(book.available_inbox_bytes(), 0);
        assert!(closed_available > 0);
    }

    #[test]
    fn no_commit_recovery_rejects_consumed_or_tampered_ticket_state_without_reclaim() {
        let payment_request = request(9, 7);
        let acceptance_intent = intent(&payment_request, 7, 9);
        let acceptance_ticket = ticket(&payment_request, &acceptance_intent, 7);
        let closure = no_commit_closure(
            &payment_request,
            &acceptance_intent,
            &acceptance_ticket,
            [7; 32],
            7,
        );
        let ticket_capacity =
            exact_capacity_for_ticket(&payment_request, acceptance_intent, &acceptance_ticket, 7);

        let mut never_begun = OfflineCashAcceptanceTicketBookV1::new(ticket_capacity);
        never_begun
            .reserve(
                verified_authorization(&payment_request, acceptance_intent, 7),
                acceptance_ticket.clone(),
            )
            .expect("ticket reservation");
        let verified = VerifiedOfflineCashNoCommitClosureV1::verify(
            closure.clone(),
            &AcceptingNoCommitClosureVerifier,
        )
        .expect("authenticated closure");
        assert_eq!(
            never_begun.close_authenticated_no_commit_recovery(verified),
            Err(OfflineCashStateErrorV1::InvalidRecoveryMaterial)
        );
        assert_eq!(never_begun.available_inbox_bytes(), 0);

        let entry = never_begun
            .tickets
            .get_mut(&acceptance_ticket.acceptance_ticket_id)
            .expect("reserved entry");
        entry.consumed_payment_digest = Some([0xE1; 32]);
        entry.consumed_amount = Some(acceptance_ticket.exact_amount);
        let verified = VerifiedOfflineCashNoCommitClosureV1::verify(
            closure.clone(),
            &AcceptingNoCommitClosureVerifier,
        )
        .expect("authenticated closure");
        assert_eq!(
            never_begun.begin_authenticated_no_commit_recovery(&verified),
            Err(OfflineCashStateErrorV1::InvalidAcceptanceTicket)
        );
        assert_eq!(never_begun.available_inbox_bytes(), 0);

        let mut pending = OfflineCashAcceptanceTicketBookV1::new(ticket_capacity);
        pending
            .reserve(
                verified_authorization(&payment_request, acceptance_intent, 7),
                acceptance_ticket.clone(),
            )
            .expect("ticket reservation");
        let verified = VerifiedOfflineCashNoCommitClosureV1::verify(
            closure,
            &AcceptingNoCommitClosureVerifier,
        )
        .expect("authenticated closure");
        pending
            .begin_authenticated_no_commit_recovery(&verified)
            .expect("begin recovery");
        let entry = pending
            .tickets
            .get_mut(&acceptance_ticket.acceptance_ticket_id)
            .expect("pending entry");
        entry.consumed_payment_digest = Some([0xE2; 32]);
        entry.consumed_amount = Some(acceptance_ticket.exact_amount);
        assert_eq!(
            pending.close_authenticated_no_commit_recovery(verified),
            Err(OfflineCashStateErrorV1::InvalidAcceptanceTicket)
        );
        assert_eq!(pending.available_inbox_bytes(), 0);
    }

    #[test]
    fn one_exact_request_accepts_distinct_payments_and_rejects_wrong_amounts() {
        let single = request(7, 1);
        let mut book = OfflineCashAcceptanceTicketBookV1::new(u64::MAX);
        let first_intent = intent(&single, 1, 7);
        let first = ticket(&single, &first_intent, 1);
        let first_digest = first
            .canonical_digest_against(&single, &first_intent)
            .expect("ticket digest");
        assert!(matches!(
            book.reserve(
                verified_authorization(&single, first_intent, 1),
                first.clone(),
            ),
            Ok(AcceptanceTicketReservationOutcomeV1::Reserved(decision))
                if decision.ticket == first && decision.ticket_digest == first_digest
        ));
        assert!(matches!(
            book.reserve(
                verified_authorization(&single, first_intent, 1),
                first.clone(),
            ),
            Ok(AcceptanceTicketReservationOutcomeV1::AlreadyReserved(decision))
                if decision.ticket == first && decision.ticket_digest == first_digest
        ));
        let second_intent = intent(&single, 2, 7);
        book.reserve(
            verified_authorization(&single, second_intent, 2),
            ticket(&single, &second_intent, 2),
        )
        .expect("distinct exact payment against same request");

        let wrong_amount = intent(&single, 3, 6);
        assert!(matches!(
            wrong_amount.validate_shape_against(&single),
            Err(
                iroha_data_model::offline::OfflineCashValidationErrorV1::InvalidField {
                    field: "offline_cash.acceptance_intent.binding"
                }
            )
        ));
    }

    #[test]
    fn request_has_no_protocol_ticket_count_limit() {
        let payment_request = request(7, 9);
        let mut book = OfflineCashAcceptanceTicketBookV1::new(u64::MAX);
        for tag in 1_u8..=130 {
            let acceptance_intent = intent(&payment_request, tag, 7);
            book.reserve(
                verified_authorization(&payment_request, acceptance_intent, tag),
                ticket(&payment_request, &acceptance_intent, tag),
            )
            .expect("distinct reusable-request ticket");
            if matches!(tag, 127..=130) {
                book.validate_recovered()
                    .expect("collection-length threshold snapshot");
            }
        }
        assert_eq!(book.intent_ticket_decisions.len(), 130);
        book.validate_recovered()
            .expect("reusable-request snapshot");
    }

    #[test]
    fn terminal_compaction_preserves_exact_intent_ticket_decisions() {
        let payment_request = request(7, 10);
        let mut book = OfflineCashAcceptanceTicketBookV1::new(u64::MAX);
        let first_intent = intent(&payment_request, 1, 7);
        let first_ticket = ticket(&payment_request, &first_intent, 1);
        for tag in 1_u8..=65 {
            let acceptance_intent = intent(&payment_request, tag, 7);
            let acceptance_ticket = ticket(&payment_request, &acceptance_intent, tag);
            book.reserve(
                verified_authorization(&payment_request, acceptance_intent, tag),
                acceptance_ticket.clone(),
            )
            .expect("distinct reusable-request decision");
            let payment_digest = [tag.wrapping_add(0x80); 32];
            let entry = book
                .tickets
                .get_mut(&acceptance_ticket.acceptance_ticket_id)
                .expect("live ticket");
            entry.consumed_payment_digest = Some(payment_digest);
            entry.consumed_amount = Some(acceptance_ticket.exact_amount);
            book.release_folded(acceptance_ticket.acceptance_ticket_id, payment_digest)
                .expect("release folded slot");
        }
        assert_eq!(book.compacted_terminal_count(), 1);
        assert!(
            !book
                .tickets
                .contains_key(&first_ticket.acceptance_ticket_id)
        );
        assert_eq!(book.reserved_terminal_metadata_bytes(), 0);
        assert_eq!(book.committed_inbox_bytes(), book.retained_metadata_bytes());
        book.validate_recovered().expect("compacted decision state");
        assert!(matches!(
            book.reserve(
                verified_authorization(&payment_request, first_intent, 1),
                first_ticket.clone(),
            ),
            Ok(AcceptanceTicketReservationOutcomeV1::AlreadyReserved(decision))
                if decision.ticket == first_ticket
        ));
        assert_eq!(
            book.reserve(
                verified_authorization(&payment_request, first_intent, 2),
                first_ticket.clone(),
            ),
            Err(OfflineCashStateErrorV1::InvalidAcceptanceTicket)
        );
        assert_eq!(
            book.reserve(
                verified_authorization(&payment_request, first_intent, 1),
                ticket(&payment_request, &first_intent, 66),
            ),
            Err(OfflineCashStateErrorV1::InvalidAcceptanceTicket)
        );
    }

    #[test]
    fn outbox_reservation_is_idempotent_and_capacity_backed() {
        let journal = OfflineCashOutgoingCandidateJournalV1::default();
        let reservation = OfflineCashOutboxReservationV1 {
            reservation_id: [0x71; 32],
            operation_kind: OfflineCashOperationKindV1::SendSplit,
            reserved_outbox_bytes: OFFLINE_CASH_PAYMENT_OUTBOX_MIN_BYTES_V1,
            issued_at_ms: 100,
            expires_at_ms: 1_000,
        };
        let mut capacity = OfflineCashSenderOutboxCapacityV1::new(u64::MAX);
        assert_eq!(
            capacity.reserve(reservation, &journal),
            Ok(SenderOutboxReservationOutcomeV1::Reserved)
        );
        assert_eq!(
            capacity.reserve(reservation, &journal),
            Ok(SenderOutboxReservationOutcomeV1::AlreadyReserved)
        );
        assert!(capacity.retained_metadata_bytes() > 0);
        assert!(capacity.reserved_terminal_metadata_bytes() > 0);
        capacity.total_outbox_bytes = capacity.committed_outbox_bytes();
        let second = OfflineCashOutboxReservationV1 {
            reservation_id: [0x72; 32],
            ..reservation
        };
        assert_eq!(
            capacity.reserve(second, &journal),
            Err(OfflineCashStateErrorV1::SenderOutboxCapacityExhausted)
        );
        assert_eq!(capacity.available_outbox_bytes(), 0);

        let conflicting = OfflineCashOutboxReservationV1 {
            expires_at_ms: 1_001,
            ..reservation
        };
        assert_eq!(
            capacity.reserve(conflicting, &journal),
            Err(OfflineCashStateErrorV1::CandidateConflict)
        );
        assert_eq!(OFFLINE_CASH_HARDWARE_REQUIRED_CAPABILITIES_V1, u16::MAX);
    }

    #[test]
    fn durable_outbox_slot_is_precommitted_before_hardware() {
        let journal = OfflineCashOutgoingCandidateJournalV1::default();
        let reservation = OfflineCashOutboxReservationV1 {
            reservation_id: [0x75; 32],
            operation_kind: OfflineCashOperationKindV1::RedeemSplit,
            reserved_outbox_bytes: OFFLINE_CASH_REDEMPTION_OUTBOX_MIN_BYTES_V1,
            issued_at_ms: 100,
            expires_at_ms: 1_000,
        };
        let mut sizing = OfflineCashSenderOutboxCapacityV1::new(u64::MAX);
        sizing
            .reserve(reservation, &journal)
            .expect("measure complete durable slot");
        let required = sizing.committed_outbox_bytes();
        assert!(required > u64::from(reservation.reserved_outbox_bytes));

        let mut short = OfflineCashSenderOutboxCapacityV1::new(required - 1);
        assert_eq!(
            short.reserve(reservation, &journal),
            Err(OfflineCashStateErrorV1::SenderOutboxCapacityExhausted)
        );
        assert_eq!(short.committed_outbox_bytes(), 0);

        let mut exact = OfflineCashSenderOutboxCapacityV1::new(required);
        assert_eq!(
            exact.reserve(reservation, &journal),
            Ok(SenderOutboxReservationOutcomeV1::Reserved)
        );
        assert_eq!(exact.committed_outbox_bytes(), required);
        assert_eq!(exact.available_outbox_bytes(), 0);
    }

    #[test]
    fn prepared_outgoing_operation_finishes_with_no_residual_capacity() {
        let prepared = prepared_redemption_for_capacity([0xC1; 32]);
        let reservation = prepared.outbox_reservation;

        let mut sizing_journal = OfflineCashOutgoingCandidateJournalV1::default();
        sizing_journal
            .prepare(prepared.clone())
            .expect("size prepared journal");
        let mut sizing_outbox = OfflineCashSenderOutboxCapacityV1::new(u64::MAX);
        sizing_outbox
            .reserve(reservation, &sizing_journal)
            .expect("size precommitted workflow");
        let exact_capacity = sizing_outbox.committed_outbox_bytes();

        let mut journal = OfflineCashOutgoingCandidateJournalV1::default();
        journal.prepare(prepared.clone()).expect("prepare journal");
        let mut outbox = OfflineCashSenderOutboxCapacityV1::new(exact_capacity);
        assert_eq!(
            outbox.reserve(reservation, &journal),
            Ok(SenderOutboxReservationOutcomeV1::Reserved)
        );
        assert_eq!(outbox.available_outbox_bytes(), 0);
        let precommitted_outbox_bytes = outbox.committed_outbox_bytes();

        let unrelated = OfflineCashOutboxReservationV1 {
            reservation_id: [0xC2; 32],
            ..reservation
        };
        let outbox_before_blocked_reservation =
            norito::encode_canonical(&outbox).expect("encode pre-failure outbox");
        let journal_before_blocked_reservation =
            norito::encode_canonical(&journal).expect("encode pre-failure journal");
        assert_eq!(
            outbox.reserve(unrelated, &journal),
            Err(OfflineCashStateErrorV1::SenderOutboxCapacityExhausted),
            "new work cannot consume the live operation's terminal headroom"
        );
        assert_eq!(outbox.available_outbox_bytes(), 0);
        assert_eq!(
            norito::encode_canonical(&outbox).expect("encode post-failure outbox"),
            outbox_before_blocked_reservation,
            "failed capacity admission must not mutate sender bytes"
        );
        assert_eq!(
            norito::encode_canonical(&journal).expect("encode post-failure journal"),
            journal_before_blocked_reservation,
            "failed capacity admission must not mutate prepared journal bytes"
        );

        let candidate = PersistedOutgoingCandidateV1::verify_and_persist(
            prepared,
            outgoing_candidate_proof(match journal.stage() {
                OfflineCashOutgoingJournalStageV1::Prepared(prepared) => prepared,
                _ => panic!("prepared stage"),
            }),
            &AcceptingOutgoingVerifier,
        )
        .expect("persist already-reserved candidate");
        journal
            .persist_candidate(candidate.clone())
            .expect("advance candidate journal");
        outbox
            .validate_capacity_meters(&journal, false)
            .expect("candidate stays inside precommitted bytes");

        let committed = CommittedOutgoingCandidateV1::from_hardware_commit(
            candidate,
            outgoing_commit_certificate(match journal.stage() {
                OfflineCashOutgoingJournalStageV1::Candidate(candidate) => candidate,
                _ => panic!("candidate stage"),
            }),
        )
        .expect("bind terminal hardware certificate");
        journal
            .commit(committed.clone())
            .expect("advance committed journal");
        outbox
            .validate_capacity_meters(&journal, false)
            .expect("committed stage stays inside precommitted bytes");

        let public_inputs = committed
            .public_wrapper_inputs()
            .expect("wrapper public inputs");
        let finalized = DurableOutgoingEnvelopeV1::finalize(
            committed,
            OfflineCashCommitWrapperProofV1 {
                version: OFFLINE_CASH_WIRE_VERSION_V1,
                eq_protocol_digest: [0xC3; 32],
                ep_protocol_digest: [0xC4; 32],
                semantic_digest: public_inputs.semantic_digest,
                candidate_envelope_digest: public_inputs.candidate_envelope_digest,
                commit_certificate_digest: public_inputs.commit_certificate_digest,
                eq_deferred_audit: [0xC5; 32],
                ep_deferred_audit: [0xC6; 32],
                eq_proof: vec![0xC7],
                ep_proof: vec![0xC8],
                eq_history: vec![0xC9; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
                ep_history: vec![0xCA; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
            },
            [0xCB; 32],
            vec![0xCC],
            &AcceptingOutgoingVerifier,
        )
        .expect("finalize from pre-reserved terminal headroom");
        journal
            .install_finalized(finalized.clone(), &mut outbox)
            .expect("install durable retry envelope");
        assert!(outbox.committed_outbox_bytes() <= precommitted_outbox_bytes);
        let retry_bytes = finalized.retry_bytes().to_vec();
        assert_eq!(
            journal.expose(reservation.reservation_id),
            Ok(retry_bytes.as_slice())
        );
        assert_eq!(
            journal.expose(reservation.reservation_id),
            Ok(retry_bytes.as_slice()),
            "retry must remain byte-identical without a new capacity admission"
        );
        journal
            .install_finalized(finalized.clone(), &mut outbox)
            .expect("idempotent finalized-install retry");

        journal
            .release_finalized(
                reservation.reservation_id,
                finalized.envelope_digest,
                &mut outbox,
            )
            .expect("release completed retry envelope");
        assert!(outbox.committed_outbox_bytes() < precommitted_outbox_bytes);
        journal
            .release_finalized(
                reservation.reservation_id,
                finalized.envelope_digest,
                &mut outbox,
            )
            .expect("idempotent terminal release retry");
    }

    #[test]
    fn terminal_outbox_reservation_ids_are_never_reusable() {
        let journal = OfflineCashOutgoingCandidateJournalV1::default();
        let reservation = OfflineCashOutboxReservationV1 {
            reservation_id: [0x73; 32],
            operation_kind: OfflineCashOperationKindV1::SendSplit,
            reserved_outbox_bytes: OFFLINE_CASH_PAYMENT_OUTBOX_MIN_BYTES_V1,
            issued_at_ms: 100,
            expires_at_ms: 1_000,
        };
        let envelope_digest = [0x74; 32];
        let mut capacity = OfflineCashSenderOutboxCapacityV1::new(u64::MAX);
        assert_eq!(
            capacity.reserve(reservation, &journal),
            Ok(SenderOutboxReservationOutcomeV1::Reserved)
        );
        capacity
            .bind_terminal_envelope(reservation, envelope_digest)
            .expect("bind terminal envelope");
        assert_eq!(
            capacity.reserve(reservation, &journal),
            Err(OfflineCashStateErrorV1::CandidateConflict)
        );
        assert_eq!(
            capacity.require_reservation(reservation),
            Err(OfflineCashStateErrorV1::CandidateConflict)
        );

        capacity
            .mark_terminal_released(reservation.reservation_id, envelope_digest)
            .expect("release terminal envelope");
        assert_eq!(
            capacity.reserve(reservation, &journal),
            Err(OfflineCashStateErrorV1::CandidateConflict)
        );
        assert_eq!(
            capacity.require_reservation(reservation),
            Err(OfflineCashStateErrorV1::CandidateConflict)
        );
    }

    #[test]
    fn sender_capacity_uses_whole_collection_canonical_metadata() {
        fn released_record(tag: u8) -> SenderOutboxReservationRecordV1 {
            let reservation = OfflineCashOutboxReservationV1 {
                reservation_id: [tag; 32],
                operation_kind: OfflineCashOperationKindV1::SendSplit,
                reserved_outbox_bytes: OFFLINE_CASH_PAYMENT_OUTBOX_MIN_BYTES_V1,
                issued_at_ms: u64::from(tag),
                expires_at_ms: u64::from(tag) + 1,
            };
            SenderOutboxReservationRecordV1 {
                reservation,
                reservation_commitment: reservation
                    .canonical_commitment()
                    .expect("reservation commitment"),
                terminal_envelope_digest: Some([tag.wrapping_add(0x40); 32]),
                released: true,
            }
        }

        let mut singleton = OfflineCashSenderOutboxCapacityV1::new(u64::MAX);
        let mut singleton_journal = OfflineCashOutgoingCandidateJournalV1::default();
        let record = released_record(1);
        singleton
            .reservations
            .insert(record.reservation.reservation_id, record);
        singleton_journal
            .released_envelopes
            .insert([1; 32], [0x41; 32]);
        singleton
            .reconcile_capacity_meters(&singleton_journal)
            .expect("singleton meters");
        let singleton_bytes = singleton.retained_metadata_bytes();

        let mut many = OfflineCashSenderOutboxCapacityV1::new(u64::MAX);
        let mut many_journal = OfflineCashOutgoingCandidateJournalV1::default();
        for tag in 1..=32 {
            let record = released_record(tag);
            many.reservations
                .insert(record.reservation.reservation_id, record);
            many_journal
                .released_envelopes
                .insert([tag; 32], [tag.wrapping_add(0x40); 32]);
        }
        many.reconcile_capacity_meters(&many_journal)
            .expect("whole-collection meters");
        assert_eq!(
            many.retained_metadata_bytes(),
            canonical_sender_retained_metadata_bytes_v1(&many, &many_journal)
                .expect("canonical retained bytes")
        );
        assert_ne!(
            many.retained_metadata_bytes(),
            singleton_bytes
                .checked_mul(32)
                .expect("test multiplication"),
            "whole-collection framing must not be approximated by singleton entry sums"
        );
        assert_eq!(
            many.committed_outbox_bytes(),
            many.retained_metadata_bytes()
        );
        assert_eq!(many.reserved_terminal_metadata_bytes(), 0);
        many.validate_recovered(&many_journal)
            .expect("valid restored meters");
    }

    #[test]
    fn sender_capacity_restore_rejects_tampered_canonical_meters() {
        let journal = OfflineCashOutgoingCandidateJournalV1::default();
        let reservation = OfflineCashOutboxReservationV1 {
            reservation_id: [0x81; 32],
            operation_kind: OfflineCashOperationKindV1::RedeemSplit,
            reserved_outbox_bytes: OFFLINE_CASH_REDEMPTION_OUTBOX_MIN_BYTES_V1,
            issued_at_ms: 100,
            expires_at_ms: 1_000,
        };
        let mut capacity = OfflineCashSenderOutboxCapacityV1::new(u64::MAX);
        capacity
            .reserve(reservation, &journal)
            .expect("reserve with terminal headroom");
        capacity
            .validate_recovered(&journal)
            .expect("valid capacity snapshot");

        let mut tampered = capacity.clone();
        tampered.committed_outbox_bytes += 1;
        assert_eq!(
            tampered.validate_recovered(&journal),
            Err(OfflineCashStateErrorV1::SnapshotIntegrity)
        );
        let mut tampered = capacity.clone();
        tampered.retained_metadata_bytes += 1;
        assert_eq!(
            tampered.validate_recovered(&journal),
            Err(OfflineCashStateErrorV1::SnapshotIntegrity)
        );
        let mut tampered = capacity;
        tampered.reserved_terminal_metadata_bytes += 1;
        assert_eq!(
            tampered.validate_recovered(&journal),
            Err(OfflineCashStateErrorV1::SnapshotIntegrity)
        );
    }
}
