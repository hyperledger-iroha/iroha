//! Pending-request installation, recovery, and expired cancellation.

use super::*;

/// Move-only unsigned request staged entirely inside Core before hardware signing.
#[derive(PartialEq, Eq)]
#[must_use]
pub(crate) struct UnsignedReceiveRequestV1 {
    release_id: Digest,
    network_id: NetworkId,
    asset: AssetDefinitionId,
    scale: u32,
    amount: u128,
    recipient: AccountId,
    receiver_balance_commitment: Digest,
    recipient_key_reference: Digest,
    recipient_encryption_public_key: Digest,
    receiver_public_key: KagemushaDevicePublicKeyV2,
    request_id: Digest,
    issued_at_ms: u64,
    expires_at_ms: u64,
    hardware_policy_id: Digest,
}

impl fmt::Debug for UnsignedReceiveRequestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UnsignedReceiveRequestV1")
            .field("release_id", &self.release_id)
            .field(
                "receiver_balance_commitment",
                &self.receiver_balance_commitment,
            )
            .field("request_id", &self.request_id)
            .field("amount", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl UnsignedReceiveRequestV1 {
    /// Stage exact unsigned request fields without creating caller-visible signed bytes.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        release_id: Digest,
        network_id: NetworkId,
        asset: AssetDefinitionId,
        scale: u32,
        amount: u128,
        recipient: AccountId,
        receiver_balance_commitment: Digest,
        recipient_key_reference: Digest,
        recipient_encryption_public_key: Digest,
        receiver_public_key: KagemushaDevicePublicKeyV2,
        request_id: Digest,
        issued_at_ms: u64,
        expires_at_ms: u64,
        hardware_policy_id: Digest,
    ) -> Result<Self, StateTransitionErrorV1> {
        let request = Self {
            release_id,
            network_id,
            asset,
            scale,
            amount,
            recipient,
            receiver_balance_commitment,
            recipient_key_reference,
            recipient_encryption_public_key,
            receiver_public_key,
            request_id,
            issued_at_ms,
            expires_at_ms,
            hardware_policy_id,
        };
        request.validate_shape()?;
        Ok(request)
    }

    pub(super) fn from_signed_request(
        request: &OfflineCashPaymentRequestV1,
    ) -> Result<Self, StateTransitionErrorV1> {
        request
            .validate()
            .map_err(|_| StateTransitionErrorV1::InvalidRequest)?;
        Self::new(
            request.release_id,
            request.network_id.clone(),
            request.asset.clone(),
            request.scale,
            request.amount,
            request.recipient.clone(),
            request.receiver_balance_commitment,
            request.recipient_key_reference,
            request.recipient_encryption_public_key,
            request.receiver_public_key,
            request.request_id,
            request.issued_at_ms,
            request.expires_at_ms,
            request.hardware_policy_id,
        )
    }

    fn validate_shape(&self) -> Result<(), StateTransitionErrorV1> {
        let ttl = self
            .expires_at_ms
            .checked_sub(self.issued_at_ms)
            .ok_or(StateTransitionErrorV1::InvalidRequest)?;
        if self.release_id == [0; 32]
            || self.receiver_balance_commitment == [0; 32]
            || self.recipient_key_reference == [0; 32]
            || self.recipient_encryption_public_key == [0; 32]
            || self.request_id == [0; 32]
            || self.hardware_policy_id == [0; 32]
            || self.network_id.as_bytes().iter().all(|byte| *byte == 0)
            || self.amount == 0
            || self.scale > KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2
            || ttl == 0
            || ttl > KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_TTL_MS_V2
            || self.recipient_key_reference
                != iroha_data_model::offline::offline_cash_receiver_key_reference_v1(
                    &self.receiver_public_key,
                    self.recipient_encryption_public_key,
                )
            || iroha_data_model::offline::validate_offline_cash_recipient_encryption_public_key_v1(
                self.recipient_encryption_public_key,
            )
            .is_err()
            || self.receiver_public_key.validate().is_err()
        {
            return Err(StateTransitionErrorV1::InvalidRequest);
        }
        Ok(())
    }

    pub(super) fn canonical_signing_bytes(
        &self,
    ) -> Result<Zeroizing<Vec<u8>>, StateTransitionErrorV1> {
        offline_cash_payment_request_signing_bytes_v1(
            OFFLINE_CASH_WIRE_VERSION_V1,
            self.release_id,
            &self.network_id,
            &self.asset,
            self.scale,
            self.amount,
            &self.recipient,
            self.receiver_balance_commitment,
            self.recipient_key_reference,
            self.recipient_encryption_public_key,
            self.receiver_public_key,
            self.request_id,
            self.issued_at_ms,
            self.expires_at_ms,
            self.hardware_policy_id,
        )
        .map(Zeroizing::new)
        .map_err(|_| StateTransitionErrorV1::InvalidRequest)
    }

    fn signing_digest(&self) -> Result<Digest, StateTransitionErrorV1> {
        let bytes = self.canonical_signing_bytes()?;
        Ok(digest_framed(
            RECEIVE_REQUEST_SIGNING_BINDING_DOMAIN,
            &[&bytes],
        ))
    }

    fn into_signed(
        self,
        signature: KagemushaDeviceSignatureV2,
    ) -> Result<OfflineCashPaymentRequestV1, StateTransitionErrorV1> {
        let request = OfflineCashPaymentRequestV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            release_id: self.release_id,
            network_id: self.network_id,
            asset: self.asset,
            scale: self.scale,
            amount: self.amount,
            recipient: self.recipient,
            receiver_balance_commitment: self.receiver_balance_commitment,
            recipient_key_reference: self.recipient_key_reference,
            recipient_encryption_public_key: self.recipient_encryption_public_key,
            receiver_public_key: self.receiver_public_key,
            request_id: self.request_id,
            issued_at_ms: self.issued_at_ms,
            expires_at_ms: self.expires_at_ms,
            hardware_policy_id: self.hardware_policy_id,
            signature,
        };
        request
            .validate()
            .map_err(|_| StateTransitionErrorV1::InvalidRequest)?;
        Ok(request)
    }

    fn matches_context(&self, context: &OfflineCashStateContextV1) -> bool {
        self.release_id == context.release_id
            && self.network_id == context.network_id
            && self.asset == context.asset
            && self.scale == context.scale
    }
}

/// Move-only owner of the wallet's sole live receiver request.
#[must_use]
pub(crate) struct PendingOwnerV1 {
    pub(super) context: OfflineCashStateContextV1,
    pub(super) wallet_binding: Digest,
    pub(super) receiver_head: Digest,
    pub(super) request_digest: Digest,
    pub(super) amount: u128,
    pub(super) issued_at_ms: u64,
    pub(super) expires_at_ms: u64,
    pub(super) recipient_key_reference: Digest,
    pub(super) recipient_encryption_public_key: Digest,
    pub(super) receiver_public_key: KagemushaDevicePublicKeyV2,
    pub(super) intent_authorization: HardwareIntentAuthorizationOwnerV1,
}

impl fmt::Debug for PendingOwnerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PendingOwnerV1")
            .field("context_digest", &self.context.digest)
            .field("receiver_head", &self.receiver_head)
            .field("request_digest", &self.request_digest)
            .field("intent_epoch", &self.intent_authorization.epoch)
            .field("amount", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

/// Prepared local request installation; it owns no signature or hardware capability.
#[must_use]
pub(crate) struct OpenPendingPlanV1 {
    expected: BalanceSnapshotV1,
    unsigned_request: UnsignedReceiveRequestV1,
    signing_digest: Digest,
    transition_digest: Digest,
    intent_challenge: HardwareIntentChallengeV1,
}

impl OpenPendingPlanV1 {
    /// Return the exact rollback-resistant receive reservation challenge.
    pub(crate) const fn intent_challenge(&self) -> &HardwareIntentChallengeV1 {
        &self.intent_challenge
    }
}

pub(super) fn request_live(
    request: &OfflineCashPaymentRequestV1,
    now_ms: u64,
) -> Result<(), StateTransitionErrorV1> {
    if now_ms < request.issued_at_ms || now_ms >= request.expires_at_ms {
        Err(StateTransitionErrorV1::RequestNotLive)
    } else {
        Ok(())
    }
}

fn unsigned_request_live(
    request: &UnsignedReceiveRequestV1,
    now_ms: u64,
) -> Result<(), StateTransitionErrorV1> {
    if now_ms < request.issued_at_ms || now_ms >= request.expires_at_ms {
        Err(StateTransitionErrorV1::RequestNotLive)
    } else {
        Ok(())
    }
}

fn open_pending_transition_digest(
    balance: &BalanceOwnerV1,
    request: &UnsignedReceiveRequestV1,
    signing_digest: Digest,
) -> Digest {
    let amount_bytes = request.amount.to_le_bytes();
    let issued_bytes = request.issued_at_ms.to_le_bytes();
    let expires_bytes = request.expires_at_ms.to_le_bytes();
    digest_framed(
        OPEN_PENDING_DOMAIN,
        &[
            &balance.context.digest,
            &balance.wallet_binding,
            &balance.head,
            &signing_digest,
            &amount_bytes,
            &issued_bytes,
            &expires_bytes,
            &request.recipient_key_reference,
        ],
    )
}

pub(super) fn receive_intent_challenge(
    balance: &BalanceOwnerV1,
    request: &UnsignedReceiveRequestV1,
) -> Result<(Digest, HardwareIntentChallengeV1), StateTransitionErrorV1> {
    let signing_digest = request.signing_digest()?;
    let transition_digest = open_pending_transition_digest(balance, request, signing_digest);
    let challenge = HardwareIntentChallengeV1::new(
        HardwareIntentKindV1::ReceivePending,
        balance,
        transition_digest,
        request.issued_at_ms,
        request.expires_at_ms,
    )?;
    Ok((transition_digest, challenge))
}

/// Prepare one current-head-bound unsigned request before hardware signing.
pub(crate) fn prepare_open_pending_v1(
    balance: &BalanceOwnerV1,
    request: UnsignedReceiveRequestV1,
    now_ms: u64,
) -> Result<OpenPendingPlanV1, StateTransitionErrorV1> {
    request.validate_shape()?;
    unsigned_request_live(&request, now_ms)?;
    if !request.matches_context(&balance.context) {
        return Err(StateTransitionErrorV1::ContextMismatch);
    }
    if request.receiver_balance_commitment != balance.head {
        return Err(StateTransitionErrorV1::RequestHeadMismatch);
    }
    if request.hardware_policy_id != balance.hardware_policy_id {
        return Err(StateTransitionErrorV1::GuardBindingMismatch);
    }
    if balance.active_request.is_some() {
        return Err(StateTransitionErrorV1::PendingRequestActive);
    }
    let signing_digest = request.signing_digest()?;
    let transition_digest = open_pending_transition_digest(balance, &request, signing_digest);
    let intent_challenge = HardwareIntentChallengeV1::new(
        HardwareIntentKindV1::ReceivePending,
        balance,
        transition_digest,
        request.issued_at_ms,
        request.expires_at_ms,
    )?;
    Ok(OpenPendingPlanV1 {
        expected: BalanceSnapshotV1::capture(balance),
        unsigned_request: request,
        signing_digest,
        transition_digest,
        intent_challenge,
    })
}

/// Atomically journal, sign, bind, and expose one receiver request.
pub(crate) fn apply_open_pending_v1<B>(
    balance: &mut BalanceOwnerV1,
    plan: OpenPendingPlanV1,
    session: &HardwareGuardSessionV1<'_, B>,
) -> Result<(PendingOwnerV1, OfflineCashPaymentRequestV1), StateTransitionErrorV1>
where
    B: ExactNextHardwareGuardBackendV1,
{
    if !plan.expected.matches(balance) || balance.active_request.is_some() {
        return Err(StateTransitionErrorV1::StaleState);
    }
    plan.unsigned_request.validate_shape()?;
    let signing_bytes = plan.unsigned_request.canonical_signing_bytes()?;
    let signing_digest = plan.unsigned_request.signing_digest()?;
    let expected_transition =
        open_pending_transition_digest(balance, &plan.unsigned_request, signing_digest);
    let expected_intent = HardwareIntentChallengeV1::new(
        HardwareIntentKindV1::ReceivePending,
        balance,
        expected_transition,
        plan.unsigned_request.issued_at_ms,
        plan.unsigned_request.expires_at_ms,
    )?;
    if signing_digest != plan.signing_digest
        || expected_transition != plan.transition_digest
        || expected_intent != plan.intent_challenge
    {
        return Err(StateTransitionErrorV1::CorruptPlan);
    }
    let signing = session.reserve_receive_signature(
        &expected_intent,
        &signing_bytes,
        &plan.unsigned_request.receiver_public_key,
    )?;
    let request = plan.unsigned_request.into_signed(signing.signature())?;
    let model_signing_bytes = request
        .canonical_signing_bytes()
        .map(Zeroizing::new)
        .map_err(|_| StateTransitionErrorV1::InvalidRequest)?;
    if model_signing_bytes.as_slice() != signing_bytes.as_slice() {
        return Err(StateTransitionErrorV1::HardwareIntentMismatch);
    }
    let request_digest = request
        .canonical_digest()
        .map_err(|_| StateTransitionErrorV1::InvalidRequest)?;
    let authorization = session.bind_receive_request(&expected_intent, signing, request_digest)?;
    if !intent_authorization_matches(&authorization, &expected_intent, request_digest) {
        return Err(StateTransitionErrorV1::HardwareIntentMismatch);
    }
    balance.active_request = Some(request_digest);
    let pending = PendingOwnerV1 {
        context: balance.context.clone(),
        wallet_binding: balance.wallet_binding,
        receiver_head: balance.head,
        request_digest,
        amount: request.amount,
        issued_at_ms: request.issued_at_ms,
        expires_at_ms: request.expires_at_ms,
        recipient_key_reference: request.recipient_key_reference,
        recipient_encryption_public_key: request.recipient_encryption_public_key,
        receiver_public_key: request.receiver_public_key,
        intent_authorization: authorization,
    };
    Ok((pending, request))
}

/// Recover the move-only pending owner and repair a rolled-back software cache.
pub(crate) fn recover_pending_v1<B>(
    balance: &mut BalanceOwnerV1,
    request: &OfflineCashPaymentRequestV1,
    session: &HardwareGuardSessionV1<'_, B>,
) -> Result<PendingOwnerV1, StateTransitionErrorV1>
where
    B: ExactNextHardwareGuardBackendV1,
{
    request
        .validate()
        .map_err(|_| StateTransitionErrorV1::InvalidRequest)?;
    let unsigned = UnsignedReceiveRequestV1::from_signed_request(request)?;
    if !unsigned.matches_context(&balance.context)
        || request.receiver_balance_commitment != balance.head
        || request.hardware_policy_id != balance.hardware_policy_id
    {
        return Err(StateTransitionErrorV1::RequestMismatch);
    }
    let request_digest = request
        .canonical_digest()
        .map_err(|_| StateTransitionErrorV1::InvalidRequest)?;
    let (_, expected_intent) = receive_intent_challenge(balance, &unsigned)?;
    let signing_bytes = unsigned.canonical_signing_bytes()?;
    let signing = session.recover_receive_signature(
        &expected_intent,
        &signing_bytes,
        &unsigned.receiver_public_key,
    )?;
    if signing.signature() != request.signature {
        return Err(StateTransitionErrorV1::HardwareIntentMismatch);
    }
    let authorization = session.bind_receive_request(&expected_intent, signing, request_digest)?;
    if !intent_authorization_matches(&authorization, &expected_intent, request_digest) {
        return Err(StateTransitionErrorV1::HardwareIntentMismatch);
    }
    if let Some(active) = balance.active_request {
        if active != request_digest {
            return Err(StateTransitionErrorV1::PendingRequestActive);
        }
    } else {
        balance.active_request = Some(request_digest);
    }
    Ok(PendingOwnerV1 {
        context: balance.context.clone(),
        wallet_binding: balance.wallet_binding,
        receiver_head: balance.head,
        request_digest,
        amount: request.amount,
        issued_at_ms: request.issued_at_ms,
        expires_at_ms: request.expires_at_ms,
        recipient_key_reference: request.recipient_key_reference,
        recipient_encryption_public_key: request.recipient_encryption_public_key,
        receiver_public_key: request.receiver_public_key,
        intent_authorization: authorization,
    })
}

/// Prepared exact cancellation of one unanswered, expired request.
#[must_use]
pub(crate) struct CancelExpiredPendingPlanV1 {
    expected: BalanceSnapshotV1,
    request_digest: Digest,
    receiver_head: Digest,
    expires_at_ms: u64,
    transition_digest: Digest,
}

impl CancelExpiredPendingPlanV1 {
    /// Return the deterministic local cancellation audit digest.
    pub(crate) const fn transition_digest(&self) -> Digest {
        self.transition_digest
    }
}

fn pending_matches_balance(balance: &BalanceOwnerV1, pending: &PendingOwnerV1) -> bool {
    pending.context == balance.context
        && pending.wallet_binding == balance.wallet_binding
        && pending.receiver_head == balance.head
        && balance.active_request == Some(pending.request_digest)
        && pending.intent_authorization.challenge.kind == HardwareIntentKindV1::ReceivePending
        && pending.intent_authorization.challenge.current_head == balance.head
        && pending.intent_authorization.challenge.from_sequence == balance.guard_sequence
        && intent_authorization_matches(
            &pending.intent_authorization,
            &pending.intent_authorization.challenge,
            pending.request_digest,
        )
}

fn cancel_pending_transition_digest(
    balance: &BalanceOwnerV1,
    request_digest: Digest,
    expires_at_ms: u64,
) -> Digest {
    let expires_bytes = expires_at_ms.to_le_bytes();
    digest_framed(
        CANCEL_PENDING_DOMAIN,
        &[
            &balance.context.digest,
            &balance.wallet_binding,
            &balance.head,
            &balance.lineage_digest,
            &request_digest,
            &expires_bytes,
        ],
    )
}

/// Prepare cancellation only at or after the request's exclusive expiry.
pub(crate) fn prepare_cancel_expired_pending_v1(
    balance: &BalanceOwnerV1,
    pending: &PendingOwnerV1,
    now_ms: u64,
) -> Result<CancelExpiredPendingPlanV1, StateTransitionErrorV1> {
    if balance.active_request.is_none() {
        return Err(StateTransitionErrorV1::NoPendingRequest);
    }
    if !pending_matches_balance(balance, pending) {
        return Err(StateTransitionErrorV1::RequestMismatch);
    }
    if now_ms < pending.expires_at_ms {
        return Err(StateTransitionErrorV1::RequestNotExpired);
    }
    let transition_digest =
        cancel_pending_transition_digest(balance, pending.request_digest, pending.expires_at_ms);
    Ok(CancelExpiredPendingPlanV1 {
        expected: BalanceSnapshotV1::capture(balance),
        request_digest: pending.request_digest,
        receiver_head: pending.receiver_head,
        expires_at_ms: pending.expires_at_ms,
        transition_digest,
    })
}

/// Successful cancellation metadata; financial head and amount are unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CancelExpiredPendingOutputV1 {
    pub(crate) request_digest: Digest,
    pub(crate) balance_head: Digest,
    pub(crate) transition_digest: Digest,
}

/// Cancellation rejection returning the unconsumed pending owner.
#[must_use]
pub(crate) struct CancelExpiredPendingRejectionV1 {
    error: StateTransitionErrorV1,
    pending: PendingOwnerV1,
}

impl CancelExpiredPendingRejectionV1 {
    pub(crate) const fn error(&self) -> StateTransitionErrorV1 {
        self.error
    }

    pub(crate) fn into_pending(self) -> PendingOwnerV1 {
        self.pending
    }
}

/// Consume one hardware-confirmed cancellation without changing monetary state.
pub(crate) fn apply_cancel_expired_pending_v1(
    balance: &mut BalanceOwnerV1,
    pending: PendingOwnerV1,
    plan: CancelExpiredPendingPlanV1,
    cancellation: HardwareIntentCancellationOwnerV1,
) -> Result<CancelExpiredPendingOutputV1, CancelExpiredPendingRejectionV1> {
    let validation = (|| {
        if !plan.expected.matches(balance) {
            return Err(StateTransitionErrorV1::StaleState);
        }
        if !pending_matches_balance(balance, &pending)
            || pending.request_digest != plan.request_digest
            || pending.receiver_head != plan.receiver_head
            || pending.expires_at_ms != plan.expires_at_ms
        {
            return Err(StateTransitionErrorV1::RequestMismatch);
        }
        let expected_transition = cancel_pending_transition_digest(
            balance,
            pending.request_digest,
            pending.expires_at_ms,
        );
        if plan.transition_digest != expected_transition {
            return Err(StateTransitionErrorV1::CorruptPlan);
        }
        if !cancellation_authorization_matches(
            &cancellation,
            &pending.intent_authorization,
            expected_transition,
        ) {
            return Err(StateTransitionErrorV1::HardwareIntentMismatch);
        }
        Ok(())
    })();
    if let Err(error) = validation {
        return Err(CancelExpiredPendingRejectionV1 { error, pending });
    }
    let output = CancelExpiredPendingOutputV1 {
        request_digest: pending.request_digest,
        balance_head: balance.head,
        transition_digest: plan.transition_digest,
    };
    balance.active_request = None;
    Ok(output)
}

/// Recover a hardware cancellation after a crash lost the local pending owner.
pub(crate) fn recover_cancelled_pending_v1<B>(
    balance: &mut BalanceOwnerV1,
    request: &OfflineCashPaymentRequestV1,
    session: &HardwareGuardSessionV1<'_, B>,
) -> Result<CancelExpiredPendingOutputV1, StateTransitionErrorV1>
where
    B: ExactNextHardwareGuardBackendV1,
{
    request
        .validate()
        .map_err(|_| StateTransitionErrorV1::InvalidRequest)?;
    let unsigned = UnsignedReceiveRequestV1::from_signed_request(request)?;
    if !unsigned.matches_context(&balance.context)
        || request.receiver_balance_commitment != balance.head
        || request.hardware_policy_id != balance.hardware_policy_id
    {
        return Err(StateTransitionErrorV1::RequestMismatch);
    }
    let request_digest = request
        .canonical_digest()
        .map_err(|_| StateTransitionErrorV1::InvalidRequest)?;
    if balance
        .active_request
        .is_some_and(|active| active != request_digest)
    {
        return Err(StateTransitionErrorV1::PendingRequestActive);
    }
    let (_, challenge) = receive_intent_challenge(balance, &unsigned)?;
    let completion =
        cancel_pending_transition_digest(balance, request_digest, request.expires_at_ms);
    let outcome = session.recover_terminal_outcome(&challenge)?;
    if outcome.operation() != HardwareTerminalOperationV1::ReceiveCancelled
        || outcome.from_sequence() != balance.guard_sequence
        || outcome.to_sequence() != balance.guard_sequence
        || outcome.intent_binding_digest() != request_digest
        || outcome.completion_digest() != completion
        || outcome.payment_digest().is_some()
        || outcome.acknowledgement_digest().is_some()
        || outcome.trusted_time_ms() < request.expires_at_ms
    {
        return Err(StateTransitionErrorV1::HardwareIntentMismatch);
    }
    balance.active_request = None;
    Ok(CancelExpiredPendingOutputV1 {
        request_digest,
        balance_head: balance.head,
        transition_digest: completion,
    })
}
