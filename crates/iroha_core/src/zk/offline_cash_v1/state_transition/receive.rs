//! Deterministic fixed-two-parent `ReceiveFold` relation and committed ACK owner.

use super::*;

/// Prepared deterministic two-parent `(balance, credit) -> balance` fold.
#[must_use]
pub(crate) struct ReceiveFoldPlanV1 {
    pub(super) expected: BalanceSnapshotV1,
    pub(super) request_digest: Digest,
    pub(super) payment_digest: Digest,
    pub(super) credit_commitment: Digest,
    pub(super) send_transition_digest: Digest,
    pub(super) amount: u128,
    pub(super) next_amount: Zeroizing<u128>,
    pub(super) next_head: Digest,
    pub(super) next_opening: Zeroizing<Digest>,
    pub(super) next_lineage_digest: Digest,
    pub(super) transition_digest: Digest,
    pub(super) completion_digest: Digest,
    pub(super) challenge: GuardChallengeV1,
}

impl ReceiveFoldPlanV1 {
    /// Return the exact challenge that the receiver hardware must authorize.
    pub(crate) fn guard_challenge(&self) -> &GuardChallengeV1 {
        &self.challenge
    }

    /// Return the deterministic successor balance commitment.
    pub(crate) const fn next_head(&self) -> Digest {
        self.next_head
    }

    /// Return the exact two-parent fold digest.
    pub(crate) const fn transition_digest(&self) -> Digest {
        self.transition_digest
    }

    /// Return the lifecycle completion binding journalled by hardware.
    pub(crate) const fn completion_digest(&self) -> Digest {
        self.completion_digest
    }
}

fn validate_pending_credit(
    balance: &BalanceOwnerV1,
    pending: &PendingOwnerV1,
    credit: &CreditOwnerV1,
    now_ms: u64,
) -> Result<(), StateTransitionErrorV1> {
    let active = balance
        .active_request
        .ok_or(StateTransitionErrorV1::NoPendingRequest)?;
    if now_ms < pending.issued_at_ms || now_ms >= pending.expires_at_ms {
        return Err(StateTransitionErrorV1::RequestNotLive);
    }
    if pending.amount == 0 || credit.amount == 0 {
        return Err(StateTransitionErrorV1::ZeroAmount);
    }
    if pending.context != balance.context
        || credit.context != balance.context
        || pending.wallet_binding != balance.wallet_binding
        || pending.receiver_head != balance.head
        || credit.receiver_head != balance.head
        || active != pending.request_digest
        || pending.request_digest != credit.request_digest
        || pending.recipient_key_reference != credit.recipient_key_reference
        || pending.amount != credit.amount
        || credit.payment_digest == [0; 32]
    {
        return Err(StateTransitionErrorV1::RequestMismatch);
    }
    let expected_credit = credit_commitment(
        &credit.context,
        credit.request_digest,
        credit.receiver_head,
        credit.recipient_key_reference,
        credit.amount,
        &credit.opening,
    );
    if expected_credit != credit.commitment || credit.send_transition_digest == [0; 32] {
        return Err(StateTransitionErrorV1::CreditMismatch);
    }
    if !terminal_credit_matches(credit) {
        return Err(StateTransitionErrorV1::TerminalVerificationMismatch);
    }
    Ok(())
}

fn receive_completion_digest(
    request_digest: Digest,
    payment_digest: Digest,
    transition_digest: Digest,
    next_head: Digest,
) -> Digest {
    digest_framed(
        RECEIVE_COMPLETION_DOMAIN,
        &[
            &request_digest,
            &payment_digest,
            &transition_digest,
            &next_head,
        ],
    )
}

/// Prepare an exact, fixed-two-parent receiver fold.
pub(crate) fn prepare_receive_fold_v1(
    balance: &BalanceOwnerV1,
    pending: &PendingOwnerV1,
    credit: &CreditOwnerV1,
    now_ms: u64,
) -> Result<ReceiveFoldPlanV1, StateTransitionErrorV1> {
    validate_pending_credit(balance, pending, credit, now_ms)?;
    let next_amount = balance
        .amount
        .checked_add(credit.amount)
        .ok_or(StateTransitionErrorV1::ArithmeticOverflow)?;
    let next_opening = offline_cash_receive_opening_v1(
        &balance.context.digest,
        &balance.opening,
        &credit.opening,
        &pending.request_digest,
        &credit.send_transition_digest,
        credit.amount,
    );
    let next_sequence = balance
        .guard_sequence
        .checked_add(1)
        .ok_or(StateTransitionErrorV1::GuardSequenceExhausted)?;
    let next_lineage_digest = offline_cash_state_lineage_digest_v1(
        crate::zk::offline_cash_v1::state_abi::OfflineCashStateOperationV1::ReceiveFold,
        &balance.context.digest,
        &balance.head,
        &balance.lineage_digest,
        balance.guard_sequence,
        next_sequence,
        &pending.request_digest,
        &credit.commitment,
        &credit.send_transition_digest,
        credit.amount,
    );
    let next_head = balance_head(
        &balance.context,
        balance.wallet_binding,
        balance.guard_device_id,
        balance.hardware_policy_id,
        next_sequence,
        next_lineage_digest,
        next_amount,
        &next_opening,
    );
    let transition_digest = offline_cash_receive_transition_digest_v1(
        &balance.context.digest,
        &balance.head,
        &credit.commitment,
        &pending.request_digest,
        &credit.send_transition_digest,
        credit.amount,
        next_amount,
        &next_head,
    );
    let completion_digest = receive_completion_digest(
        pending.request_digest,
        credit.payment_digest,
        transition_digest,
        next_head,
    );
    let challenge =
        GuardChallengeV1::new(GuardOperationV1::ReceiveFold, balance, transition_digest)?;
    Ok(ReceiveFoldPlanV1 {
        expected: BalanceSnapshotV1::capture(balance),
        request_digest: pending.request_digest,
        payment_digest: credit.payment_digest,
        credit_commitment: credit.commitment,
        send_transition_digest: credit.send_transition_digest,
        amount: credit.amount,
        next_amount: Zeroizing::new(next_amount),
        next_head,
        next_opening,
        next_lineage_digest,
        transition_digest,
        completion_digest,
        challenge,
    })
}

fn validate_receive_plan(
    balance: &BalanceOwnerV1,
    pending: &PendingOwnerV1,
    credit: &CreditOwnerV1,
    plan: &ReceiveFoldPlanV1,
    now_ms: u64,
) -> Result<(), StateTransitionErrorV1> {
    validate_pending_credit(balance, pending, credit, now_ms)?;
    if !plan.expected.matches(balance) {
        return Err(StateTransitionErrorV1::StaleState);
    }
    if pending.context != balance.context
        || credit.context != balance.context
        || pending.wallet_binding != balance.wallet_binding
        || pending.receiver_head != balance.head
        || credit.receiver_head != balance.head
        || pending.request_digest != plan.request_digest
        || credit.request_digest != plan.request_digest
        || credit.payment_digest != plan.payment_digest
        || pending.recipient_key_reference != credit.recipient_key_reference
        || pending.amount != credit.amount
        || credit.commitment != plan.credit_commitment
        || credit.send_transition_digest != plan.send_transition_digest
        || credit.amount != plan.amount
        || balance.active_request != Some(plan.request_digest)
    {
        return Err(StateTransitionErrorV1::RequestMismatch);
    }
    let expected_credit = credit_commitment(
        &credit.context,
        credit.request_digest,
        credit.receiver_head,
        credit.recipient_key_reference,
        credit.amount,
        &credit.opening,
    );
    if expected_credit != credit.commitment || credit.send_transition_digest == [0; 32] {
        return Err(StateTransitionErrorV1::CreditMismatch);
    }
    if !terminal_credit_matches(credit) {
        return Err(StateTransitionErrorV1::TerminalVerificationMismatch);
    }
    let next_amount = balance
        .amount
        .checked_add(plan.amount)
        .ok_or(StateTransitionErrorV1::ArithmeticOverflow)?;
    let expected_opening = offline_cash_receive_opening_v1(
        &balance.context.digest,
        &balance.opening,
        &credit.opening,
        &plan.request_digest,
        &plan.send_transition_digest,
        plan.amount,
    );
    let expected_sequence = balance
        .guard_sequence
        .checked_add(1)
        .ok_or(StateTransitionErrorV1::GuardSequenceExhausted)?;
    let expected_next_lineage = offline_cash_state_lineage_digest_v1(
        crate::zk::offline_cash_v1::state_abi::OfflineCashStateOperationV1::ReceiveFold,
        &balance.context.digest,
        &balance.head,
        &balance.lineage_digest,
        balance.guard_sequence,
        expected_sequence,
        &plan.request_digest,
        &plan.credit_commitment,
        &plan.send_transition_digest,
        plan.amount,
    );
    let expected_head = balance_head(
        &balance.context,
        balance.wallet_binding,
        balance.guard_device_id,
        balance.hardware_policy_id,
        expected_sequence,
        expected_next_lineage,
        next_amount,
        &expected_opening,
    );
    let expected_transition = offline_cash_receive_transition_digest_v1(
        &balance.context.digest,
        &balance.head,
        &plan.credit_commitment,
        &plan.request_digest,
        &plan.send_transition_digest,
        plan.amount,
        next_amount,
        &expected_head,
    );
    let expected_completion = receive_completion_digest(
        plan.request_digest,
        plan.payment_digest,
        expected_transition,
        expected_head,
    );
    let expected_challenge =
        GuardChallengeV1::new(GuardOperationV1::ReceiveFold, balance, expected_transition)?;
    if next_amount != *plan.next_amount
        || plan.next_opening != expected_opening
        || plan.next_lineage_digest != expected_next_lineage
        || expected_head != plan.next_head
        || expected_transition != plan.transition_digest
        || expected_completion != plan.completion_digest
        || plan.challenge != expected_challenge
    {
        return Err(StateTransitionErrorV1::CorruptPlan);
    }
    Ok(())
}

/// Successful receiver fold metadata (the private balance remains in its owner).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReceiveFoldOutputV1 {
    pub(crate) release_id: Digest,
    pub(crate) context_digest: Digest,
    pub(crate) request_digest: Digest,
    /// Exact canonical payment whose credit was persisted.
    pub(crate) payment_digest: Digest,
    pub(crate) amount: u128,
    pub(crate) scale: u32,
    pub(crate) balance_parent: Digest,
    pub(crate) credit_parent: Digest,
    pub(crate) next_head: Digest,
    pub(crate) send_transition_digest: Digest,
    pub(crate) receive_transition_digest: Digest,
}

/// Move-only ACK construction capability emitted only after committed receive.
#[must_use]
pub(crate) struct CommittedReceiveAcknowledgementOwnerV1 {
    release_id: Digest,
    request_digest: Digest,
    payment_digest: Digest,
    receiver_head: Digest,
    acknowledged_at_ms: u64,
    receiver_public_key: KagemushaDevicePublicKeyV2,
    terminal_outcome: HardwareTerminalOutcomeV1,
}

impl fmt::Debug for CommittedReceiveAcknowledgementOwnerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommittedReceiveAcknowledgementOwnerV1")
            .field("request_digest", &self.request_digest)
            .field("payment_digest", &self.payment_digest)
            .field("receiver_head", &self.receiver_head)
            .field("acknowledged_at_ms", &self.acknowledged_at_ms)
            .finish_non_exhaustive()
    }
}

impl CommittedReceiveAcknowledgementOwnerV1 {
    pub(super) fn signing_bytes(&self) -> Result<Zeroizing<Vec<u8>>, StateTransitionErrorV1> {
        offline_cash_acknowledgement_signing_bytes_v1(
            OFFLINE_CASH_WIRE_VERSION_V1,
            self.release_id,
            self.request_digest,
            self.payment_digest,
            self.receiver_head,
            self.acknowledged_at_ms,
        )
        .map(Zeroizing::new)
        .map_err(|_| StateTransitionErrorV1::AcknowledgementMismatch)
    }
}

/// Rejection that returns both unconsumed private input owners.
#[must_use]
pub(crate) struct ReceiveFoldRejectionV1 {
    error: StateTransitionErrorV1,
    pending: PendingOwnerV1,
    credit: CreditOwnerV1,
}

impl ReceiveFoldRejectionV1 {
    pub(crate) const fn error(&self) -> StateTransitionErrorV1 {
        self.error
    }

    pub(crate) fn into_owners(self) -> (PendingOwnerV1, CreditOwnerV1) {
        (self.pending, self.credit)
    }
}

fn validate_receive_terminal(
    pending: &PendingOwnerV1,
    plan: &ReceiveFoldPlanV1,
    outcome: &HardwareTerminalOutcomeV1,
) -> Result<(), StateTransitionErrorV1> {
    if outcome.operation() != HardwareTerminalOperationV1::ReceiveCommitted
        || outcome.intent() != &pending.intent_authorization.challenge.hardware_request()
        || outcome.intent_epoch() != pending.intent_authorization.epoch
        || outcome.from_sequence() != plan.challenge.from_sequence
        || outcome.to_sequence() != plan.challenge.to_sequence
        || outcome.intent_binding_digest() != pending.request_digest
        || outcome.completion_digest() != plan.completion_digest
        || outcome.successor_head() != plan.next_head
        || outcome.payment_digest() != Some(plan.payment_digest)
        || outcome
            .acknowledgement_digest()
            .is_some_and(|digest| digest == [0; 32])
        || outcome.trusted_time_ms() < pending.issued_at_ms
        || outcome.trusted_time_ms() >= pending.expires_at_ms
    {
        return Err(StateTransitionErrorV1::HardwareIntentMismatch);
    }
    Ok(())
}

fn apply_receive_terminal(
    balance: &mut BalanceOwnerV1,
    pending: PendingOwnerV1,
    credit: CreditOwnerV1,
    plan: ReceiveFoldPlanV1,
    outcome: HardwareTerminalOutcomeV1,
) -> (ReceiveFoldOutputV1, CommittedReceiveAcknowledgementOwnerV1) {
    let output = ReceiveFoldOutputV1 {
        release_id: balance.context.release_id,
        context_digest: balance.context.digest,
        request_digest: plan.request_digest,
        payment_digest: plan.payment_digest,
        amount: plan.amount,
        scale: balance.context.scale,
        balance_parent: balance.head,
        credit_parent: credit.commitment,
        next_head: plan.next_head,
        send_transition_digest: credit.send_transition_digest,
        receive_transition_digest: plan.transition_digest,
    };
    let acknowledgement = CommittedReceiveAcknowledgementOwnerV1 {
        release_id: balance.context.release_id,
        request_digest: plan.request_digest,
        payment_digest: plan.payment_digest,
        receiver_head: plan.next_head,
        acknowledged_at_ms: outcome.trusted_time_ms(),
        receiver_public_key: pending.receiver_public_key,
        terminal_outcome: outcome,
    };
    balance.amount = *plan.next_amount;
    balance.head = plan.next_head;
    balance.lineage_digest = plan.next_lineage_digest;
    balance.opening = plan.next_opening;
    balance.guard_sequence = plan.challenge.to_sequence;
    balance.active_request = None;
    (output, acknowledgement)
}

/// Consume both receiver parents and atomically persist their deterministic fold.
pub(crate) fn apply_receive_fold_v1<B>(
    balance: &mut BalanceOwnerV1,
    pending: PendingOwnerV1,
    credit: CreditOwnerV1,
    plan: ReceiveFoldPlanV1,
    now_ms: u64,
    session: &HardwareGuardSessionV1<'_, B>,
) -> Result<(ReceiveFoldOutputV1, CommittedReceiveAcknowledgementOwnerV1), ReceiveFoldRejectionV1>
where
    B: ExactNextHardwareGuardBackendV1,
{
    if let Err(error) = validate_receive_plan(balance, &pending, &credit, &plan, now_ms) {
        return Err(ReceiveFoldRejectionV1 {
            error,
            pending,
            credit,
        });
    }
    let (authorization, outcome) = match session.commit_intent_exact_next(
        &pending.intent_authorization,
        &plan.challenge,
        plan.payment_digest,
        None,
        plan.completion_digest,
        plan.next_head,
    ) {
        Ok(result) => result,
        Err(error) => {
            return Err(ReceiveFoldRejectionV1 {
                error,
                pending,
                credit,
            });
        }
    };
    if !guard_authorization_matches(&authorization, &plan.challenge) {
        return Err(ReceiveFoldRejectionV1 {
            error: StateTransitionErrorV1::GuardBindingMismatch,
            pending,
            credit,
        });
    }
    if let Err(error) = validate_receive_terminal(&pending, &plan, &outcome) {
        return Err(ReceiveFoldRejectionV1 {
            error,
            pending,
            credit,
        });
    }
    Ok(apply_receive_terminal(
        balance, pending, credit, plan, outcome,
    ))
}

/// Failed committed-receive recovery returns both restart inputs for retry.
#[must_use]
pub(crate) struct ReceiveFoldRecoveryRejectionV1 {
    error: StateTransitionErrorV1,
    verification: VerifiedOfflineCashCreditV1,
    opening: DecryptedCreditOpeningOwnerV1,
}

impl ReceiveFoldRecoveryRejectionV1 {
    pub(crate) const fn error(&self) -> StateTransitionErrorV1 {
        self.error
    }

    pub(crate) fn into_owners(
        self,
    ) -> (VerifiedOfflineCashCreditV1, DecryptedCreditOpeningOwnerV1) {
        (self.verification, self.opening)
    }
}

/// Repair local receiver state from durable canonical inputs after a crash.
///
/// No retained `PendingOwnerV1` or prepared plan is accepted. Core reconstructs
/// both from the authenticated old balance, canonical request/payment, retained
/// proof verification and freshly authenticated credit opening, and the
/// hardware terminal receipt. No pre-crash transition owner is required.
/// Restart-time
/// wall clock is intentionally irrelevant; the receipt's trusted commit time is
/// the sole liveness instant.
pub(crate) fn recover_committed_receive_fold_v1<B>(
    balance: &mut BalanceOwnerV1,
    request: &OfflineCashPaymentRequestV1,
    payment: &OfflineCashPaymentV1,
    verification: VerifiedOfflineCashCreditV1,
    opening: DecryptedCreditOpeningOwnerV1,
    session: &HardwareGuardSessionV1<'_, B>,
) -> Result<
    (ReceiveFoldOutputV1, CommittedReceiveAcknowledgementOwnerV1),
    ReceiveFoldRecoveryRejectionV1,
>
where
    B: ExactNextHardwareGuardBackendV1,
{
    let recovered = (|| {
        request
            .validate()
            .map_err(|_| StateTransitionErrorV1::InvalidRequest)?;
        if !balance.context.matches_request(request)
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
            return Err(StateTransitionErrorV1::RequestMismatch);
        }
        payment
            .validate_against(request)
            .map_err(|_| StateTransitionErrorV1::CreditMismatch)?;
        let payment_digest = payment
            .canonical_digest_against(request)
            .map_err(|_| StateTransitionErrorV1::CreditMismatch)?;
        let encrypted_credit_digest: Digest = Sha256::digest(&payment.encrypted_credit).into();
        if payment_digest != verification.payment_digest()
            || payment.statement.credit_commitment != verification.credit_commitment()
            || payment.statement.transition_digest != verification.transition_digest()
            || payment.statement.request_digest != request_digest
            || payment.statement.receiver_before != balance.head
            || payment.statement.amount != verification.amount()
            || verification.encrypted_credit_digest() != encrypted_credit_digest
            || opening.encrypted_credit_digest != verification.encrypted_credit_digest()
            || opening.recipient_key_reference != verification.recipient_key_reference()
        {
            return Err(StateTransitionErrorV1::CreditMismatch);
        }
        let unsigned = UnsignedReceiveRequestV1::from_signed_request(request)?;
        let (_, intent_challenge) = receive_intent_challenge(balance, &unsigned)?;
        let (intent_authorization, outcome) = session.recover_committed_receive_authorization(
            &intent_challenge,
            request_digest,
            payment_digest,
        )?;
        let pending = PendingOwnerV1 {
            context: balance.context.clone(),
            wallet_binding: balance.wallet_binding,
            receiver_head: balance.head,
            request_digest,
            amount: request.amount,
            issued_at_ms: request.issued_at_ms,
            expires_at_ms: request.expires_at_ms,
            recipient_key_reference: request.recipient_key_reference,
            receiver_public_key: request.receiver_public_key,
            intent_authorization,
        };
        Ok::<_, StateTransitionErrorV1>((pending, outcome))
    })();
    let (pending, outcome) = match recovered {
        Ok(recovered) => recovered,
        Err(error) => {
            return Err(ReceiveFoldRecoveryRejectionV1 {
                error,
                verification,
                opening,
            });
        }
    };
    let credit = match bind_verified_credit_v1(&pending, &payment.statement, verification, opening)
    {
        Ok(credit) => credit,
        Err(rejection) => {
            return Err(ReceiveFoldRecoveryRejectionV1 {
                error: rejection.error(),
                verification: rejection.verification,
                opening: rejection.opening,
            });
        }
    };
    let cached_active_request = balance.active_request;
    balance.active_request = Some(pending.request_digest);
    let plan = prepare_receive_fold_v1(balance, &pending, &credit, outcome.trusted_time_ms());
    balance.active_request = cached_active_request;
    let plan = match plan {
        Ok(plan) => plan,
        Err(error) => {
            let (verification, opening) = credit.into_recovery_inputs();
            return Err(ReceiveFoldRecoveryRejectionV1 {
                error,
                verification,
                opening,
            });
        }
    };
    if let Err(error) = validate_receive_terminal(&pending, &plan, &outcome) {
        let (verification, opening) = credit.into_recovery_inputs();
        return Err(ReceiveFoldRecoveryRejectionV1 {
            error,
            verification,
            opening,
        });
    }
    Ok(apply_receive_terminal(
        balance, pending, credit, plan, outcome,
    ))
}

/// Recreate the post-commit ACK owner from a persisted successor balance.
///
/// This remains valid both before the first hardware ACK signature and after a
/// crash following signing. The sealed backend returns the identical retained
/// signature for the identical signing bytes.
pub(crate) fn recover_receive_acknowledgement_owner_v1<B>(
    balance: &BalanceOwnerV1,
    request: &OfflineCashPaymentRequestV1,
    payment: &OfflineCashPaymentV1,
    session: &HardwareGuardSessionV1<'_, B>,
) -> Result<CommittedReceiveAcknowledgementOwnerV1, StateTransitionErrorV1>
where
    B: ExactNextHardwareGuardBackendV1,
{
    request
        .validate()
        .map_err(|_| StateTransitionErrorV1::InvalidRequest)?;
    if !balance.context.matches_request(request)
        || request.hardware_policy_id != balance.hardware_policy_id
        || balance.active_request.is_some()
    {
        return Err(StateTransitionErrorV1::RequestMismatch);
    }
    payment
        .validate_against(request)
        .map_err(|_| StateTransitionErrorV1::CreditMismatch)?;
    let request_digest = request
        .canonical_digest()
        .map_err(|_| StateTransitionErrorV1::InvalidRequest)?;
    let payment_digest = payment
        .canonical_digest_against(request)
        .map_err(|_| StateTransitionErrorV1::CreditMismatch)?;
    let outcome =
        session.recover_receive_terminal_for_balance(balance, request_digest, payment_digest)?;
    if outcome.intent().current_head() != request.receiver_balance_commitment
        || outcome.intent().hardware_policy_id() != request.hardware_policy_id
        || outcome.intent().not_before_ms() != request.issued_at_ms
        || outcome.intent().expires_at_ms() != request.expires_at_ms
        || outcome.trusted_time_ms() < request.issued_at_ms
        || outcome.trusted_time_ms() >= request.expires_at_ms
        || outcome.successor_head() == request.receiver_balance_commitment
        || outcome
            .acknowledgement_digest()
            .is_some_and(|digest| digest == [0; 32])
    {
        return Err(StateTransitionErrorV1::HardwareIntentMismatch);
    }
    Ok(CommittedReceiveAcknowledgementOwnerV1 {
        release_id: request.release_id,
        request_digest,
        payment_digest,
        receiver_head: outcome.successor_head(),
        acknowledged_at_ms: outcome.trusted_time_ms(),
        receiver_public_key: request.receiver_public_key,
        terminal_outcome: outcome,
    })
}

/// Ask sealed hardware to sign the exact ACK for one committed receive.
pub(crate) fn issue_receive_acknowledgement_v1<B>(
    owner: &CommittedReceiveAcknowledgementOwnerV1,
    session: &HardwareGuardSessionV1<'_, B>,
) -> Result<OfflineCashAcknowledgementV1, StateTransitionErrorV1>
where
    B: ExactNextHardwareGuardBackendV1,
{
    let signing_bytes = owner.signing_bytes()?;
    let acknowledgement_digest = digest_framed(RECEIVE_ACK_AUTHORIZATION_DOMAIN, &[&signing_bytes]);
    let signature = session.sign_receive_acknowledgement(
        &owner.terminal_outcome,
        acknowledgement_digest,
        &signing_bytes,
        &owner.receiver_public_key,
    )?;
    signature
        .verify(&owner.receiver_public_key, &signing_bytes)
        .map_err(|_| StateTransitionErrorV1::AcknowledgementMismatch)?;
    let acknowledgement = OfflineCashAcknowledgementV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        release_id: owner.release_id,
        request_digest: owner.request_digest,
        payment_digest: owner.payment_digest,
        receiver_balance_commitment: owner.receiver_head,
        acknowledged_at_ms: owner.acknowledged_at_ms,
        signature,
    };
    let model_signing_bytes = acknowledgement
        .canonical_signing_bytes()
        .map(Zeroizing::new)
        .map_err(|_| StateTransitionErrorV1::AcknowledgementMismatch)?;
    if model_signing_bytes.as_slice() != signing_bytes.as_slice() {
        return Err(StateTransitionErrorV1::AcknowledgementMismatch);
    }
    Ok(acknowledgement)
}
