//! Sealed rollback-resistant intent and exact-next hardware capabilities.

use super::*;

/// Stable operation role covered by an exact-next hardware authorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum GuardOperationV1 {
    /// Replace a sender balance with its deterministic remainder.
    SendSplit = 1,
    /// Fold exactly one request-bound credit into the receiver balance.
    ReceiveFold = 2,
}

/// Stable active-intent role stored beside the monetary hardware counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum HardwareIntentKindV1 {
    /// Sole receiver request, bound to the unchanged current balance head.
    ReceivePending = 1,
    /// Canonical sender payment published atomically with its digest binding.
    SendPublished = 2,
}

/// Stable terminal journal operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum HardwareTerminalOperationV1 {
    /// Trusted-time cancellation of an unanswered receiver request.
    ReceiveCancelled = 1,
    /// Exact-next sender debit committed after a verified acknowledgement.
    SendCommitted = 2,
    /// Exact-next receiver credit fold committed before acknowledgement signing.
    ReceiveCommitted = 3,
}

/// Request passed only to a sealed platform hardware-counter backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HardwareGuardRequestV1 {
    pub(super) device_id: Digest,
    pub(super) hardware_policy_id: Digest,
    pub(super) wallet_binding: Digest,
    pub(super) from_sequence: u64,
    pub(super) to_sequence: u64,
    pub(super) challenge_digest: Digest,
}

impl HardwareGuardRequestV1 {
    pub(crate) const fn device_id(&self) -> Digest {
        self.device_id
    }

    pub(crate) const fn hardware_policy_id(&self) -> Digest {
        self.hardware_policy_id
    }

    pub(crate) const fn wallet_binding(&self) -> Digest {
        self.wallet_binding
    }

    pub(crate) const fn from_sequence(&self) -> u64 {
        self.from_sequence
    }

    pub(crate) const fn to_sequence(&self) -> u64 {
        self.to_sequence
    }

    pub(crate) const fn challenge_digest(&self) -> Digest {
        self.challenge_digest
    }
}

/// Exact rollback-resistant intent installed without changing monetary state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HardwareIntentRequestV1 {
    kind: HardwareIntentKindV1,
    device_id: Digest,
    hardware_policy_id: Digest,
    wallet_binding: Digest,
    context_digest: Digest,
    current_head: Digest,
    current_lineage_digest: Digest,
    from_sequence: u64,
    not_before_ms: u64,
    expires_at_ms: u64,
    intent_digest: Digest,
    challenge_digest: Digest,
}

impl HardwareIntentRequestV1 {
    pub(crate) const fn kind(&self) -> HardwareIntentKindV1 {
        self.kind
    }

    pub(crate) const fn device_id(&self) -> Digest {
        self.device_id
    }

    pub(crate) const fn hardware_policy_id(&self) -> Digest {
        self.hardware_policy_id
    }

    pub(crate) const fn wallet_binding(&self) -> Digest {
        self.wallet_binding
    }

    pub(crate) const fn context_digest(&self) -> Digest {
        self.context_digest
    }

    pub(crate) const fn current_head(&self) -> Digest {
        self.current_head
    }

    pub(crate) const fn current_lineage_digest(&self) -> Digest {
        self.current_lineage_digest
    }

    pub(crate) const fn from_sequence(&self) -> u64 {
        self.from_sequence
    }

    pub(crate) const fn not_before_ms(&self) -> u64 {
        self.not_before_ms
    }

    pub(crate) const fn expires_at_ms(&self) -> u64 {
        self.expires_at_ms
    }

    pub(crate) const fn intent_digest(&self) -> Digest {
        self.intent_digest
    }

    pub(crate) const fn challenge_digest(&self) -> Digest {
        self.challenge_digest
    }
}

/// Hardware result of atomically reserving and signing one receiver request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HardwareReceiveSigningResultV1 {
    epoch: u64,
    signature: KagemushaDeviceSignatureV2,
}

impl HardwareReceiveSigningResultV1 {
    pub(crate) const fn new(epoch: u64, signature: KagemushaDeviceSignatureV2) -> Self {
        Self { epoch, signature }
    }

    pub(crate) const fn epoch(&self) -> u64 {
        self.epoch
    }

    pub(crate) const fn signature(&self) -> KagemushaDeviceSignatureV2 {
        self.signature
    }
}

/// Exact active journal record returned only for crash recovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HardwareActiveIntentOutcomeV1 {
    intent: HardwareIntentRequestV1,
    intent_epoch: u64,
    bound_digest: Digest,
}

/// Durable receive-terminal lookup that does not depend on retained local owners.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HardwareReceiveTerminalQueryV1 {
    device_id: Digest,
    hardware_policy_id: Digest,
    wallet_binding: Digest,
    context_digest: Digest,
    request_digest: Digest,
    payment_digest: Digest,
}

impl HardwareReceiveTerminalQueryV1 {
    pub(crate) const fn device_id(&self) -> Digest {
        self.device_id
    }

    pub(crate) const fn hardware_policy_id(&self) -> Digest {
        self.hardware_policy_id
    }

    pub(crate) const fn wallet_binding(&self) -> Digest {
        self.wallet_binding
    }

    pub(crate) const fn context_digest(&self) -> Digest {
        self.context_digest
    }

    pub(crate) const fn request_digest(&self) -> Digest {
        self.request_digest
    }

    pub(crate) const fn payment_digest(&self) -> Digest {
        self.payment_digest
    }
}

impl HardwareActiveIntentOutcomeV1 {
    pub(crate) const fn new(
        intent: HardwareIntentRequestV1,
        intent_epoch: u64,
        bound_digest: Digest,
    ) -> Self {
        Self {
            intent,
            intent_epoch,
            bound_digest,
        }
    }

    pub(crate) const fn intent(&self) -> &HardwareIntentRequestV1 {
        &self.intent
    }

    pub(crate) const fn intent_epoch(&self) -> u64 {
        self.intent_epoch
    }

    pub(crate) const fn bound_digest(&self) -> Digest {
        self.bound_digest
    }
}

/// Atomic active-intent consumption plus exact-next monetary commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HardwareIntentCommitRequestV1 {
    intent: HardwareIntentRequestV1,
    intent_epoch: u64,
    intent_binding_digest: Digest,
    guard: HardwareGuardRequestV1,
    payment_digest: Digest,
    acknowledgement_digest: Option<Digest>,
    completion_digest: Digest,
    successor_head: Digest,
}

impl HardwareIntentCommitRequestV1 {
    pub(crate) const fn intent(&self) -> &HardwareIntentRequestV1 {
        &self.intent
    }

    pub(crate) const fn intent_epoch(&self) -> u64 {
        self.intent_epoch
    }

    pub(crate) const fn intent_binding_digest(&self) -> Digest {
        self.intent_binding_digest
    }

    pub(crate) const fn guard(&self) -> &HardwareGuardRequestV1 {
        &self.guard
    }

    pub(crate) const fn payment_digest(&self) -> Digest {
        self.payment_digest
    }

    pub(crate) const fn acknowledgement_digest(&self) -> Option<Digest> {
        self.acknowledgement_digest
    }

    pub(crate) const fn completion_digest(&self) -> Digest {
        self.completion_digest
    }

    pub(crate) const fn successor_head(&self) -> Digest {
        self.successor_head
    }
}

/// Durable terminal journal receipt used to repair a rolled-back software cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HardwareTerminalOutcomeV1 {
    operation: HardwareTerminalOperationV1,
    intent: HardwareIntentRequestV1,
    intent_epoch: u64,
    from_sequence: u64,
    to_sequence: u64,
    intent_binding_digest: Digest,
    completion_digest: Digest,
    payment_digest: Option<Digest>,
    acknowledgement_digest: Option<Digest>,
    trusted_time_ms: u64,
    successor_head: Digest,
}

impl HardwareTerminalOutcomeV1 {
    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn new(
        operation: HardwareTerminalOperationV1,
        intent: HardwareIntentRequestV1,
        intent_epoch: u64,
        from_sequence: u64,
        to_sequence: u64,
        intent_binding_digest: Digest,
        completion_digest: Digest,
        payment_digest: Option<Digest>,
        acknowledgement_digest: Option<Digest>,
        trusted_time_ms: u64,
        successor_head: Digest,
    ) -> Self {
        Self {
            operation,
            intent,
            intent_epoch,
            from_sequence,
            to_sequence,
            intent_binding_digest,
            completion_digest,
            payment_digest,
            acknowledgement_digest,
            trusted_time_ms,
            successor_head,
        }
    }

    pub(crate) const fn operation(&self) -> HardwareTerminalOperationV1 {
        self.operation
    }

    pub(crate) const fn intent(&self) -> &HardwareIntentRequestV1 {
        &self.intent
    }

    pub(crate) const fn intent_epoch(&self) -> u64 {
        self.intent_epoch
    }

    pub(crate) const fn from_sequence(&self) -> u64 {
        self.from_sequence
    }

    pub(crate) const fn to_sequence(&self) -> u64 {
        self.to_sequence
    }

    /// Exact signed request (receive) or canonical payment (send) binding.
    pub(crate) const fn intent_binding_digest(&self) -> Digest {
        self.intent_binding_digest
    }

    pub(crate) const fn completion_digest(&self) -> Digest {
        self.completion_digest
    }

    pub(crate) const fn payment_digest(&self) -> Option<Digest> {
        self.payment_digest
    }

    pub(crate) const fn acknowledgement_digest(&self) -> Option<Digest> {
        self.acknowledgement_digest
    }

    pub(crate) const fn trusted_time_ms(&self) -> u64 {
        self.trusted_time_ms
    }

    /// Exact authenticated balance head resulting from the terminal commit.
    pub(crate) const fn successor_head(&self) -> Digest {
        self.successor_head
    }
}

/// Failure reported by the sealed platform hardware boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HardwareGuardErrorV1 {
    /// Hardware/key service is currently unavailable.
    Unavailable,
    /// Another intent or exact-next commit owns the expected state.
    StaleOrConcurrent,
    /// No matching durable active intent exists.
    IntentMismatch,
    /// The request is not live according to the backend's trusted clock.
    TrustedTimeRejected,
    /// The hardware policy, key, or counter state rejected the operation.
    PolicyRejected,
}

/// Private supertrait namespace for trusted platform adapters.
pub(crate) mod sealed {
    /// Prevents application/host crates from supplying an authorization backend.
    pub trait Sealed {}
}

/// Sealed rollback-resistant wallet journal and signing boundary.
///
/// A receiver signature is returned only by the same atomic operation that
/// installs its sole active intent. A sender has no hardware intent until the
/// complete canonical payment exists: publication atomically validates trusted
/// time and binds its digest. Receiver commits must also recheck trusted time;
/// sender commits intentionally remain recoverable after expiry because a valid
/// acknowledgement may arrive late. Every terminal transition is journalled and
/// queryable by its exact intent so a crash between hardware CAS and software
/// cache mutation cannot strand or replay monetary state.
pub(crate) trait ExactNextHardwareGuardBackendV1: sealed::Sealed {
    /// Atomically install a receive intent and sign its exact canonical preimage.
    fn reserve_receive_intent_and_sign_or_recover(
        &self,
        request: &HardwareIntentRequestV1,
        signing_bytes: &[u8],
        receiver_public_key: &KagemushaDevicePublicKeyV2,
    ) -> Result<HardwareReceiveSigningResultV1, HardwareGuardErrorV1>;

    /// Recover, without creating, the signature of an existing receive intent.
    fn recover_receive_intent_and_signature(
        &self,
        request: &HardwareIntentRequestV1,
        signing_bytes: &[u8],
        receiver_public_key: &KagemushaDevicePublicKeyV2,
    ) -> Result<HardwareReceiveSigningResultV1, HardwareGuardErrorV1>;

    /// Bind the exact signed request digest before its bytes can leave Core.
    fn bind_receive_request_digest_or_recover(
        &self,
        request: &HardwareIntentRequestV1,
        intent_epoch: u64,
        request_digest: Digest,
    ) -> Result<(), HardwareGuardErrorV1>;

    /// Atomically install a live send intent already bound to a canonical payment.
    fn publish_send_payment_or_recover(
        &self,
        request: &HardwareIntentRequestV1,
        payment_digest: Digest,
    ) -> Result<u64, HardwareGuardErrorV1>;

    /// Recover an exact active intent without creating or rebinding it.
    fn recover_active_intent(
        &self,
        request: &HardwareIntentRequestV1,
    ) -> Result<HardwareActiveIntentOutcomeV1, HardwareGuardErrorV1>;

    /// Cancel only a matching expired receiver reservation using trusted time.
    fn cancel_expired_receive_or_recover(
        &self,
        request: &HardwareIntentRequestV1,
        intent_epoch: u64,
        completion_digest: Digest,
    ) -> Result<HardwareTerminalOutcomeV1, HardwareGuardErrorV1>;

    /// Atomically consume one intent and advance, or recover, its exact commit.
    fn commit_intent_or_recover_exact_next(
        &self,
        request: &HardwareIntentCommitRequestV1,
    ) -> Result<HardwareTerminalOutcomeV1, HardwareGuardErrorV1>;

    /// Query the durable terminal receipt for one byte-identical intent.
    fn recover_terminal_outcome(
        &self,
        request: &HardwareIntentRequestV1,
    ) -> Result<HardwareTerminalOutcomeV1, HardwareGuardErrorV1>;

    /// Query one committed receive by its canonical request/payment bindings.
    fn recover_receive_terminal_outcome(
        &self,
        query: &HardwareReceiveTerminalQueryV1,
    ) -> Result<HardwareTerminalOutcomeV1, HardwareGuardErrorV1>;

    /// Sign an acknowledgement only for an exact committed receive receipt.
    ///
    /// The backend must durably retain the exact signing binding and signature
    /// before returning; recovery must return the byte-identical signature.
    fn sign_receive_acknowledgement_or_recover(
        &self,
        outcome: &HardwareTerminalOutcomeV1,
        acknowledgement_digest: Digest,
        signing_bytes: &[u8],
        receiver_public_key: &KagemushaDevicePublicKeyV2,
    ) -> Result<KagemushaDeviceSignatureV2, HardwareGuardErrorV1>;
}

/// Opaque exact-next challenge derived by a prepared state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GuardChallengeV1 {
    pub(super) operation: GuardOperationV1,
    pub(super) device_id: Digest,
    pub(super) hardware_policy_id: Digest,
    pub(super) wallet_binding: Digest,
    pub(super) context_digest: Digest,
    pub(super) current_head: Digest,
    pub(super) current_lineage_digest: Digest,
    pub(super) from_sequence: u64,
    pub(super) to_sequence: u64,
    pub(super) transition_digest: Digest,
    pub(super) digest: Digest,
}

impl GuardChallengeV1 {
    pub(super) fn new(
        operation: GuardOperationV1,
        balance: &BalanceOwnerV1,
        transition_digest: Digest,
    ) -> Result<Self, StateTransitionErrorV1> {
        let to_sequence = balance
            .guard_sequence
            .checked_add(1)
            .ok_or(StateTransitionErrorV1::GuardSequenceExhausted)?;
        let operation_byte = [operation as u8];
        let from_bytes = balance.guard_sequence.to_le_bytes();
        let to_bytes = to_sequence.to_le_bytes();
        let digest = digest_framed(
            GUARD_CHALLENGE_DOMAIN,
            &[
                &operation_byte,
                &balance.guard_device_id,
                &balance.hardware_policy_id,
                &balance.wallet_binding,
                &balance.context.digest,
                &balance.head,
                &balance.lineage_digest,
                &from_bytes,
                &to_bytes,
                &transition_digest,
            ],
        );
        Ok(Self {
            operation,
            device_id: balance.guard_device_id,
            hardware_policy_id: balance.hardware_policy_id,
            wallet_binding: balance.wallet_binding,
            context_digest: balance.context.digest,
            current_head: balance.head,
            current_lineage_digest: balance.lineage_digest,
            from_sequence: balance.guard_sequence,
            to_sequence,
            transition_digest,
            digest,
        })
    }

    fn hardware_request(&self) -> HardwareGuardRequestV1 {
        HardwareGuardRequestV1 {
            device_id: self.device_id,
            hardware_policy_id: self.hardware_policy_id,
            wallet_binding: self.wallet_binding,
            from_sequence: self.from_sequence,
            to_sequence: self.to_sequence,
            challenge_digest: self.digest,
        }
    }
}

/// Opaque reservation challenge for the hardware's sole active-intent slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HardwareIntentChallengeV1 {
    pub(super) kind: HardwareIntentKindV1,
    pub(super) device_id: Digest,
    pub(super) hardware_policy_id: Digest,
    pub(super) wallet_binding: Digest,
    pub(super) context_digest: Digest,
    pub(super) current_head: Digest,
    pub(super) current_lineage_digest: Digest,
    pub(super) from_sequence: u64,
    pub(super) not_before_ms: u64,
    pub(super) expires_at_ms: u64,
    pub(super) intent_digest: Digest,
    pub(super) digest: Digest,
}

impl HardwareIntentChallengeV1 {
    pub(super) fn new(
        kind: HardwareIntentKindV1,
        balance: &BalanceOwnerV1,
        intent_digest: Digest,
        not_before_ms: u64,
        expires_at_ms: u64,
    ) -> Result<Self, StateTransitionErrorV1> {
        if intent_digest == [0; 32] || not_before_ms >= expires_at_ms {
            return Err(StateTransitionErrorV1::CorruptPlan);
        }
        let kind_byte = [kind as u8];
        let sequence_bytes = balance.guard_sequence.to_le_bytes();
        let not_before_bytes = not_before_ms.to_le_bytes();
        let expires_bytes = expires_at_ms.to_le_bytes();
        let digest = digest_framed(
            INTENT_CHALLENGE_DOMAIN,
            &[
                &kind_byte,
                &balance.guard_device_id,
                &balance.hardware_policy_id,
                &balance.wallet_binding,
                &balance.context.digest,
                &balance.head,
                &balance.lineage_digest,
                &sequence_bytes,
                &not_before_bytes,
                &expires_bytes,
                &intent_digest,
            ],
        );
        Ok(Self {
            kind,
            device_id: balance.guard_device_id,
            hardware_policy_id: balance.hardware_policy_id,
            wallet_binding: balance.wallet_binding,
            context_digest: balance.context.digest,
            current_head: balance.head,
            current_lineage_digest: balance.lineage_digest,
            from_sequence: balance.guard_sequence,
            not_before_ms,
            expires_at_ms,
            intent_digest,
            digest,
        })
    }

    pub(super) fn hardware_request(&self) -> HardwareIntentRequestV1 {
        HardwareIntentRequestV1 {
            kind: self.kind,
            device_id: self.device_id,
            hardware_policy_id: self.hardware_policy_id,
            wallet_binding: self.wallet_binding,
            context_digest: self.context_digest,
            current_head: self.current_head,
            current_lineage_digest: self.current_lineage_digest,
            from_sequence: self.from_sequence,
            not_before_ms: self.not_before_ms,
            expires_at_ms: self.expires_at_ms,
            intent_digest: self.intent_digest,
            challenge_digest: self.digest,
        }
    }
}

/// Move-only proof of one successful exact-next hardware commit.
#[must_use]
pub(crate) struct GuardAuthorizationOwnerV1 {
    pub(super) challenge: GuardChallengeV1,
    pub(super) capability: Zeroizing<Digest>,
}

impl fmt::Debug for GuardAuthorizationOwnerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GuardAuthorizationOwnerV1")
            .field("operation", &self.challenge.operation)
            .field("challenge_digest", &self.challenge.digest)
            .field("capability", &"[REDACTED]")
            .finish()
    }
}

/// Move-only proof that one exact rollback-resistant intent owns the wallet slot.
#[must_use]
pub(crate) struct HardwareIntentAuthorizationOwnerV1 {
    pub(super) challenge: HardwareIntentChallengeV1,
    pub(super) epoch: u64,
    pub(super) bound_digest: Digest,
    pub(super) capability: Zeroizing<Digest>,
}

impl fmt::Debug for HardwareIntentAuthorizationOwnerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HardwareIntentAuthorizationOwnerV1")
            .field("kind", &self.challenge.kind)
            .field("intent_digest", &self.challenge.intent_digest)
            .field("epoch", &self.epoch)
            .field("bound_digest", &self.bound_digest)
            .field("capability", &"[REDACTED]")
            .finish()
    }
}

/// Move-only proof that trusted hardware cancelled one expired receive intent.
#[must_use]
pub(crate) struct HardwareIntentCancellationOwnerV1 {
    pub(super) challenge: HardwareIntentChallengeV1,
    pub(super) epoch: u64,
    pub(super) outcome: HardwareTerminalOutcomeV1,
    pub(super) capability: Zeroizing<Digest>,
}

impl fmt::Debug for HardwareIntentCancellationOwnerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HardwareIntentCancellationOwnerV1")
            .field("intent_digest", &self.challenge.intent_digest)
            .field("epoch", &self.epoch)
            .field("completion_digest", &self.outcome.completion_digest)
            .field("capability", &"[REDACTED]")
            .finish()
    }
}

pub(super) fn expected_guard_capability(challenge: &GuardChallengeV1) -> Zeroizing<Digest> {
    let from_bytes = challenge.from_sequence.to_le_bytes();
    let to_bytes = challenge.to_sequence.to_le_bytes();
    secret_digest(
        GUARD_CAPABILITY_DOMAIN,
        &[
            &challenge.digest,
            &challenge.device_id,
            &challenge.wallet_binding,
            &from_bytes,
            &to_bytes,
        ],
    )
}

pub(super) fn guard_authorization_matches(
    authorization: &GuardAuthorizationOwnerV1,
    expected: &GuardChallengeV1,
) -> bool {
    authorization.challenge == *expected
        && authorization.capability == expected_guard_capability(expected)
}

fn expected_intent_capability(
    challenge: &HardwareIntentChallengeV1,
    epoch: u64,
    bound_digest: Digest,
) -> Zeroizing<Digest> {
    let epoch_bytes = epoch.to_le_bytes();
    secret_digest(
        INTENT_CAPABILITY_DOMAIN,
        &[
            &challenge.digest,
            &challenge.device_id,
            &challenge.wallet_binding,
            &epoch_bytes,
            &bound_digest,
        ],
    )
}

pub(super) fn intent_authorization_matches(
    authorization: &HardwareIntentAuthorizationOwnerV1,
    expected: &HardwareIntentChallengeV1,
    expected_bound_digest: Digest,
) -> bool {
    authorization.challenge == *expected
        && authorization.bound_digest == expected_bound_digest
        && expected_bound_digest != [0; 32]
        && authorization.capability
            == expected_intent_capability(expected, authorization.epoch, expected_bound_digest)
}

fn expected_cancellation_capability(
    challenge: &HardwareIntentChallengeV1,
    epoch: u64,
    completion_digest: Digest,
) -> Zeroizing<Digest> {
    let epoch_bytes = epoch.to_le_bytes();
    secret_digest(
        INTENT_CANCELLATION_DOMAIN,
        &[&challenge.digest, &epoch_bytes, &completion_digest],
    )
}

fn terminal_outcome_matches_intent(
    outcome: &HardwareTerminalOutcomeV1,
    challenge: &HardwareIntentChallengeV1,
) -> bool {
    outcome.intent == challenge.hardware_request()
        && outcome.intent_epoch != 0
        && outcome.from_sequence == challenge.from_sequence
        && outcome.intent_binding_digest != [0; 32]
        && outcome.completion_digest != [0; 32]
        && outcome.successor_head != [0; 32]
}

pub(super) fn cancellation_authorization_matches(
    cancellation: &HardwareIntentCancellationOwnerV1,
    expected: &HardwareIntentAuthorizationOwnerV1,
    completion_digest: Digest,
) -> bool {
    cancellation.challenge == expected.challenge
        && cancellation.epoch == expected.epoch
        && cancellation.outcome.operation == HardwareTerminalOperationV1::ReceiveCancelled
        && cancellation.outcome.from_sequence == cancellation.outcome.to_sequence
        && cancellation.outcome.intent_binding_digest == expected.bound_digest
        && cancellation.outcome.completion_digest == completion_digest
        && cancellation.outcome.payment_digest.is_none()
        && cancellation.outcome.acknowledgement_digest.is_none()
        && cancellation.outcome.successor_head == expected.challenge.current_head
        && terminal_outcome_matches_intent(&cancellation.outcome, &expected.challenge)
        && cancellation.capability
            == expected_cancellation_capability(
                &expected.challenge,
                expected.epoch,
                completion_digest,
            )
}

/// Bound session which mints capabilities only after the sealed backend succeeds.
#[derive(Clone, Copy)]
pub(crate) struct HardwareGuardSessionV1<'a, B> {
    backend: &'a B,
    device_id: Digest,
    hardware_policy_id: Digest,
    wallet_binding: Digest,
}

impl<'a, B> HardwareGuardSessionV1<'a, B>
where
    B: ExactNextHardwareGuardBackendV1,
{
    pub(crate) const fn new(
        backend: &'a B,
        device_id: Digest,
        hardware_policy_id: Digest,
        wallet_binding: Digest,
    ) -> Self {
        Self {
            backend,
            device_id,
            hardware_policy_id,
            wallet_binding,
        }
    }

    fn matches_intent(&self, challenge: &HardwareIntentChallengeV1) -> bool {
        challenge.device_id == self.device_id
            && challenge.hardware_policy_id == self.hardware_policy_id
            && challenge.wallet_binding == self.wallet_binding
    }

    pub(super) fn reserve_receive_signature(
        &self,
        challenge: &HardwareIntentChallengeV1,
        signing_bytes: &[u8],
        receiver_public_key: &KagemushaDevicePublicKeyV2,
    ) -> Result<HardwareReceiveSigningResultV1, StateTransitionErrorV1> {
        if !self.matches_intent(challenge)
            || challenge.kind != HardwareIntentKindV1::ReceivePending
            || signing_bytes.is_empty()
        {
            return Err(StateTransitionErrorV1::GuardBindingMismatch);
        }
        self.backend
            .reserve_receive_intent_and_sign_or_recover(
                &challenge.hardware_request(),
                signing_bytes,
                receiver_public_key,
            )
            .map_err(StateTransitionErrorV1::HardwareGuard)
    }

    pub(super) fn recover_receive_signature(
        &self,
        challenge: &HardwareIntentChallengeV1,
        signing_bytes: &[u8],
        receiver_public_key: &KagemushaDevicePublicKeyV2,
    ) -> Result<HardwareReceiveSigningResultV1, StateTransitionErrorV1> {
        if !self.matches_intent(challenge)
            || challenge.kind != HardwareIntentKindV1::ReceivePending
            || signing_bytes.is_empty()
        {
            return Err(StateTransitionErrorV1::GuardBindingMismatch);
        }
        self.backend
            .recover_receive_intent_and_signature(
                &challenge.hardware_request(),
                signing_bytes,
                receiver_public_key,
            )
            .map_err(StateTransitionErrorV1::HardwareGuard)
    }

    pub(super) fn bind_receive_request(
        &self,
        challenge: &HardwareIntentChallengeV1,
        signing: HardwareReceiveSigningResultV1,
        request_digest: Digest,
    ) -> Result<HardwareIntentAuthorizationOwnerV1, StateTransitionErrorV1> {
        if !self.matches_intent(challenge)
            || challenge.kind != HardwareIntentKindV1::ReceivePending
            || signing.epoch == 0
            || request_digest == [0; 32]
        {
            return Err(StateTransitionErrorV1::HardwareIntentMismatch);
        }
        self.backend
            .bind_receive_request_digest_or_recover(
                &challenge.hardware_request(),
                signing.epoch,
                request_digest,
            )
            .map_err(StateTransitionErrorV1::HardwareGuard)?;
        Ok(HardwareIntentAuthorizationOwnerV1 {
            challenge: *challenge,
            epoch: signing.epoch,
            bound_digest: request_digest,
            capability: expected_intent_capability(challenge, signing.epoch, request_digest),
        })
    }

    /// Atomically publish a complete canonical payment under trusted time.
    pub(crate) fn publish_send_payment(
        &self,
        challenge: &HardwareIntentChallengeV1,
        payment_digest: Digest,
    ) -> Result<HardwareIntentAuthorizationOwnerV1, StateTransitionErrorV1> {
        if !self.matches_intent(challenge)
            || challenge.kind != HardwareIntentKindV1::SendPublished
            || payment_digest == [0; 32]
        {
            return Err(StateTransitionErrorV1::HardwareIntentMismatch);
        }
        let epoch = self
            .backend
            .publish_send_payment_or_recover(&challenge.hardware_request(), payment_digest)
            .map_err(StateTransitionErrorV1::HardwareGuard)?;
        if epoch == 0 {
            return Err(StateTransitionErrorV1::HardwareIntentMismatch);
        }
        Ok(HardwareIntentAuthorizationOwnerV1 {
            challenge: *challenge,
            epoch,
            bound_digest: payment_digest,
            capability: expected_intent_capability(challenge, epoch, payment_digest),
        })
    }

    /// Recover an active, already-published canonical sender payment.
    pub(crate) fn recover_send_payment(
        &self,
        challenge: &HardwareIntentChallengeV1,
        payment_digest: Digest,
    ) -> Result<HardwareIntentAuthorizationOwnerV1, StateTransitionErrorV1> {
        if !self.matches_intent(challenge)
            || challenge.kind != HardwareIntentKindV1::SendPublished
            || payment_digest == [0; 32]
        {
            return Err(StateTransitionErrorV1::HardwareIntentMismatch);
        }
        let active = self
            .backend
            .recover_active_intent(&challenge.hardware_request())
            .map_err(StateTransitionErrorV1::HardwareGuard)?;
        if active.intent != challenge.hardware_request()
            || active.intent_epoch == 0
            || active.bound_digest != payment_digest
        {
            return Err(StateTransitionErrorV1::HardwareIntentMismatch);
        }
        Ok(HardwareIntentAuthorizationOwnerV1 {
            challenge: *challenge,
            epoch: active.intent_epoch,
            bound_digest: payment_digest,
            capability: expected_intent_capability(challenge, active.intent_epoch, payment_digest),
        })
    }

    /// Cancel a receive intent only after the backend's trusted expiry decision.
    pub(crate) fn cancel_expired_receive(
        &self,
        authorization: &HardwareIntentAuthorizationOwnerV1,
        completion_digest: Digest,
    ) -> Result<HardwareIntentCancellationOwnerV1, StateTransitionErrorV1> {
        if !self.matches_intent(&authorization.challenge)
            || authorization.challenge.kind != HardwareIntentKindV1::ReceivePending
            || !intent_authorization_matches(
                authorization,
                &authorization.challenge,
                authorization.bound_digest,
            )
            || completion_digest == [0; 32]
        {
            return Err(StateTransitionErrorV1::HardwareIntentMismatch);
        }
        let outcome = self
            .backend
            .cancel_expired_receive_or_recover(
                &authorization.challenge.hardware_request(),
                authorization.epoch,
                completion_digest,
            )
            .map_err(StateTransitionErrorV1::HardwareGuard)?;
        if outcome.operation != HardwareTerminalOperationV1::ReceiveCancelled
            || outcome.intent != authorization.challenge.hardware_request()
            || outcome.intent_epoch != authorization.epoch
            || outcome.from_sequence != authorization.challenge.from_sequence
            || outcome.to_sequence != authorization.challenge.from_sequence
            || outcome.intent_binding_digest != authorization.bound_digest
            || outcome.completion_digest != completion_digest
            || outcome.payment_digest.is_some()
            || outcome.acknowledgement_digest.is_some()
            || outcome.successor_head != authorization.challenge.current_head
            || outcome.trusted_time_ms < authorization.challenge.expires_at_ms
        {
            return Err(StateTransitionErrorV1::HardwareIntentMismatch);
        }
        Ok(HardwareIntentCancellationOwnerV1 {
            challenge: authorization.challenge,
            epoch: authorization.epoch,
            outcome,
            capability: expected_cancellation_capability(
                &authorization.challenge,
                authorization.epoch,
                completion_digest,
            ),
        })
    }

    /// Consume a matching intent and atomically advance its exact monetary successor.
    pub(crate) fn commit_intent_exact_next(
        &self,
        authorization: &HardwareIntentAuthorizationOwnerV1,
        challenge: &GuardChallengeV1,
        payment_digest: Digest,
        acknowledgement_digest: Option<Digest>,
        completion_digest: Digest,
        successor_head: Digest,
    ) -> Result<(GuardAuthorizationOwnerV1, HardwareTerminalOutcomeV1), StateTransitionErrorV1>
    {
        let (expected_operation, terminal_operation, bound_digest_valid, acknowledgement_valid) =
            match authorization.challenge.kind {
                HardwareIntentKindV1::ReceivePending => (
                    GuardOperationV1::ReceiveFold,
                    HardwareTerminalOperationV1::ReceiveCommitted,
                    authorization.bound_digest != [0; 32],
                    acknowledgement_digest.is_none(),
                ),
                HardwareIntentKindV1::SendPublished => (
                    GuardOperationV1::SendSplit,
                    HardwareTerminalOperationV1::SendCommitted,
                    authorization.bound_digest == payment_digest,
                    acknowledgement_digest.is_some_and(|digest| digest != [0; 32]),
                ),
            };
        if !self.matches_intent(&authorization.challenge)
            || !intent_authorization_matches(
                authorization,
                &authorization.challenge,
                authorization.bound_digest,
            )
            || challenge.operation != expected_operation
            || challenge.device_id != authorization.challenge.device_id
            || challenge.hardware_policy_id != authorization.challenge.hardware_policy_id
            || challenge.wallet_binding != authorization.challenge.wallet_binding
            || challenge.context_digest != authorization.challenge.context_digest
            || challenge.current_head != authorization.challenge.current_head
            || challenge.current_lineage_digest != authorization.challenge.current_lineage_digest
            || challenge.from_sequence != authorization.challenge.from_sequence
            || payment_digest == [0; 32]
            || !bound_digest_valid
            || !acknowledgement_valid
            || completion_digest == [0; 32]
            || successor_head == [0; 32]
        {
            return Err(StateTransitionErrorV1::HardwareIntentMismatch);
        }
        let request = HardwareIntentCommitRequestV1 {
            intent: authorization.challenge.hardware_request(),
            intent_epoch: authorization.epoch,
            intent_binding_digest: authorization.bound_digest,
            guard: challenge.hardware_request(),
            payment_digest,
            acknowledgement_digest,
            completion_digest,
            successor_head,
        };
        let outcome = self
            .backend
            .commit_intent_or_recover_exact_next(&request)
            .map_err(StateTransitionErrorV1::HardwareGuard)?;
        if outcome.operation != terminal_operation
            || outcome.intent != request.intent
            || outcome.intent_epoch != request.intent_epoch
            || outcome.from_sequence != challenge.from_sequence
            || outcome.to_sequence != challenge.to_sequence
            || outcome.intent_binding_digest != authorization.bound_digest
            || outcome.completion_digest != completion_digest
            || outcome.payment_digest != Some(payment_digest)
            || outcome.acknowledgement_digest != acknowledgement_digest
            || outcome.successor_head != successor_head
            || (terminal_operation == HardwareTerminalOperationV1::ReceiveCommitted
                && (outcome.trusted_time_ms < authorization.challenge.not_before_ms
                    || outcome.trusted_time_ms >= authorization.challenge.expires_at_ms))
        {
            return Err(StateTransitionErrorV1::HardwareIntentMismatch);
        }
        Ok((
            GuardAuthorizationOwnerV1 {
                challenge: *challenge,
                capability: expected_guard_capability(challenge),
            },
            outcome,
        ))
    }

    /// Recover one durable terminal receipt without recreating its active intent.
    pub(crate) fn recover_terminal_outcome(
        &self,
        challenge: &HardwareIntentChallengeV1,
    ) -> Result<HardwareTerminalOutcomeV1, StateTransitionErrorV1> {
        if !self.matches_intent(challenge) {
            return Err(StateTransitionErrorV1::GuardBindingMismatch);
        }
        let outcome = self
            .backend
            .recover_terminal_outcome(&challenge.hardware_request())
            .map_err(StateTransitionErrorV1::HardwareGuard)?;
        if !terminal_outcome_matches_intent(&outcome, challenge) {
            return Err(StateTransitionErrorV1::HardwareIntentMismatch);
        }
        Ok(outcome)
    }

    /// Recover the terminal receive receipt and its former exact intent owner.
    pub(crate) fn recover_committed_receive_authorization(
        &self,
        challenge: &HardwareIntentChallengeV1,
        request_digest: Digest,
        payment_digest: Digest,
    ) -> Result<
        (
            HardwareIntentAuthorizationOwnerV1,
            HardwareTerminalOutcomeV1,
        ),
        StateTransitionErrorV1,
    > {
        if challenge.kind != HardwareIntentKindV1::ReceivePending
            || request_digest == [0; 32]
            || payment_digest == [0; 32]
        {
            return Err(StateTransitionErrorV1::HardwareIntentMismatch);
        }
        let outcome = self.recover_terminal_outcome(challenge)?;
        if outcome.operation != HardwareTerminalOperationV1::ReceiveCommitted
            || outcome.intent_binding_digest != request_digest
            || outcome.payment_digest != Some(payment_digest)
            || outcome.intent_epoch == 0
            || outcome.trusted_time_ms < challenge.not_before_ms
            || outcome.trusted_time_ms >= challenge.expires_at_ms
        {
            return Err(StateTransitionErrorV1::HardwareIntentMismatch);
        }
        let authorization = HardwareIntentAuthorizationOwnerV1 {
            challenge: *challenge,
            epoch: outcome.intent_epoch,
            bound_digest: request_digest,
            capability: expected_intent_capability(challenge, outcome.intent_epoch, request_digest),
        };
        Ok((authorization, outcome))
    }

    /// Recover a committed receive after the successor balance was persisted.
    pub(crate) fn recover_receive_terminal_for_balance(
        &self,
        balance: &BalanceOwnerV1,
        request_digest: Digest,
        payment_digest: Digest,
    ) -> Result<HardwareTerminalOutcomeV1, StateTransitionErrorV1> {
        if balance.guard_device_id != self.device_id
            || balance.hardware_policy_id != self.hardware_policy_id
            || balance.wallet_binding != self.wallet_binding
            || request_digest == [0; 32]
            || payment_digest == [0; 32]
        {
            return Err(StateTransitionErrorV1::GuardBindingMismatch);
        }
        let query = HardwareReceiveTerminalQueryV1 {
            device_id: self.device_id,
            hardware_policy_id: self.hardware_policy_id,
            wallet_binding: self.wallet_binding,
            context_digest: balance.context.digest,
            request_digest,
            payment_digest,
        };
        let outcome = self
            .backend
            .recover_receive_terminal_outcome(&query)
            .map_err(StateTransitionErrorV1::HardwareGuard)?;
        let exact_next = outcome
            .from_sequence
            .checked_add(1)
            .is_some_and(|next| next == outcome.to_sequence);
        if outcome.operation != HardwareTerminalOperationV1::ReceiveCommitted
            || outcome.intent.kind != HardwareIntentKindV1::ReceivePending
            || outcome.intent.device_id != query.device_id
            || outcome.intent.hardware_policy_id != query.hardware_policy_id
            || outcome.intent.wallet_binding != query.wallet_binding
            || outcome.intent.context_digest != query.context_digest
            || outcome.intent_binding_digest != query.request_digest
            || outcome.payment_digest != Some(query.payment_digest)
            || outcome.to_sequence != balance.guard_sequence
            || outcome.successor_head != balance.head
            || !exact_next
            || balance.active_request.is_some()
            || !terminal_outcome_matches_intent(
                &outcome,
                &HardwareIntentChallengeV1 {
                    kind: outcome.intent.kind,
                    device_id: outcome.intent.device_id,
                    hardware_policy_id: outcome.intent.hardware_policy_id,
                    wallet_binding: outcome.intent.wallet_binding,
                    context_digest: outcome.intent.context_digest,
                    current_head: outcome.intent.current_head,
                    current_lineage_digest: outcome.intent.current_lineage_digest,
                    from_sequence: outcome.intent.from_sequence,
                    not_before_ms: outcome.intent.not_before_ms,
                    expires_at_ms: outcome.intent.expires_at_ms,
                    intent_digest: outcome.intent.intent_digest,
                    digest: outcome.intent.challenge_digest,
                },
            )
        {
            return Err(StateTransitionErrorV1::HardwareIntentMismatch);
        }
        Ok(outcome)
    }

    /// Sign a receiver ACK only after its exact receive commit is durable.
    pub(crate) fn sign_receive_acknowledgement(
        &self,
        outcome: &HardwareTerminalOutcomeV1,
        acknowledgement_digest: Digest,
        signing_bytes: &[u8],
        receiver_public_key: &KagemushaDevicePublicKeyV2,
    ) -> Result<KagemushaDeviceSignatureV2, StateTransitionErrorV1> {
        if outcome.operation != HardwareTerminalOperationV1::ReceiveCommitted
            || outcome.intent.device_id != self.device_id
            || outcome.intent.hardware_policy_id != self.hardware_policy_id
            || outcome.intent.wallet_binding != self.wallet_binding
            || outcome.payment_digest.is_none()
            || acknowledgement_digest == [0; 32]
            || signing_bytes.is_empty()
            || outcome
                .acknowledgement_digest
                .is_some_and(|existing| existing != acknowledgement_digest)
        {
            return Err(StateTransitionErrorV1::HardwareIntentMismatch);
        }
        self.backend
            .sign_receive_acknowledgement_or_recover(
                outcome,
                acknowledgement_digest,
                signing_bytes,
                receiver_public_key,
            )
            .map_err(StateTransitionErrorV1::HardwareGuard)
    }
}
