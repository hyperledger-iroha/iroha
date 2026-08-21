//! Deterministic fixed-two-parent `SendSplit` relation and atomic publication.

use super::*;

/// Prepared deterministic two-parent `SendSplit` relation.
#[must_use]
pub(crate) struct SendSplitPlanV1 {
    pub(super) expected: BalanceSnapshotV1,
    pub(super) context: OfflineCashStateContextV1,
    pub(super) request_digest: Digest,
    pub(super) receiver_head: Digest,
    pub(super) recipient_key_reference: Digest,
    pub(super) issued_at_ms: u64,
    pub(super) expires_at_ms: u64,
    pub(super) amount: u128,
    pub(super) remainder_amount: Zeroizing<u128>,
    pub(super) remainder_head: Digest,
    pub(super) remainder_opening: Zeroizing<Digest>,
    pub(super) next_lineage_digest: Digest,
    pub(super) split_seed: Zeroizing<Digest>,
    pub(super) credit_commitment: Digest,
    pub(super) credit_opening: Zeroizing<Digest>,
    pub(super) statement: OfflineCashTransferStatementV1,
    pub(super) challenge: GuardChallengeV1,
    pub(super) intent_challenge: HardwareIntentChallengeV1,
}

impl SendSplitPlanV1 {
    /// Return the exact challenge authorized only during canonical publication.
    pub(crate) fn guard_challenge(&self) -> &GuardChallengeV1 {
        &self.challenge
    }

    /// Return the exact rollback-resistant sender publication challenge.
    pub(crate) fn intent_challenge(&self) -> &HardwareIntentChallengeV1 {
        &self.intent_challenge
    }

    /// Return the deterministic sender remainder commitment.
    pub(crate) const fn remainder_head(&self) -> Digest {
        self.remainder_head
    }

    /// Return the deterministic receiver credit commitment.
    pub(crate) const fn credit_commitment(&self) -> Digest {
        self.credit_commitment
    }

    /// Return the exact public statement covered by both proof parities.
    pub(crate) const fn statement(&self) -> &OfflineCashTransferStatementV1 {
        &self.statement
    }
}

/// Atomically published `SendSplit` output.
#[must_use]
pub(crate) struct SendSplitOutputV1 {
    credit: OutgoingCreditOwnerV1,
    pub(super) statement: OfflineCashTransferStatementV1,
}

impl SendSplitOutputV1 {
    /// Separate the receiver credit owner from the common public statement.
    pub(crate) fn into_parts(self) -> (OutgoingCreditOwnerV1, OfflineCashTransferStatementV1) {
        (self.credit, self.statement)
    }
}

/// Prepare a deterministic sender-remainder and receiver-credit branch pair.
pub(crate) fn prepare_send_split_v1(
    balance: &BalanceOwnerV1,
    request: &OfflineCashPaymentRequestV1,
    now_ms: u64,
) -> Result<SendSplitPlanV1, StateTransitionErrorV1> {
    if request.amount == 0 {
        return Err(StateTransitionErrorV1::ZeroAmount);
    }
    request
        .validate()
        .map_err(|_| StateTransitionErrorV1::InvalidRequest)?;
    request_live(request, now_ms)?;
    if !balance.context.matches_request(request) {
        return Err(StateTransitionErrorV1::ContextMismatch);
    }
    if balance.active_request.is_some() {
        return Err(StateTransitionErrorV1::PendingRequestActive);
    }
    let remainder_amount = balance
        .amount
        .checked_sub(request.amount)
        .ok_or(StateTransitionErrorV1::InsufficientFunds)?;
    if remainder_amount
        .checked_add(request.amount)
        .ok_or(StateTransitionErrorV1::ArithmeticOverflow)?
        != balance.amount
    {
        return Err(StateTransitionErrorV1::ArithmeticOverflow);
    }
    let next_sequence = balance
        .guard_sequence
        .checked_add(1)
        .ok_or(StateTransitionErrorV1::GuardSequenceExhausted)?;
    let request_digest = request
        .canonical_digest()
        .map_err(|_| StateTransitionErrorV1::InvalidRequest)?;
    let split_seed = offline_cash_send_split_seed_v1(
        &balance.context.digest,
        &balance.wallet_binding,
        &balance.head,
        &balance.opening,
        balance.guard_sequence,
        &request_digest,
        &request.receiver_balance_commitment,
        &request.recipient_key_reference,
        request.amount,
    );
    let (remainder_opening, credit_opening) = offline_cash_send_split_openings_v1(&split_seed);
    let credit_commitment = credit_commitment(
        &balance.context,
        request_digest,
        request.receiver_balance_commitment,
        request.recipient_key_reference,
        request.amount,
        &credit_opening,
    );
    let next_lineage_digest = offline_cash_state_lineage_digest_v1(
        crate::zk::offline_cash_v1::state_abi::OfflineCashStateOperationV1::SendSplit,
        &balance.context.digest,
        &balance.head,
        &balance.lineage_digest,
        balance.guard_sequence,
        next_sequence,
        &request_digest,
        &request.receiver_balance_commitment,
        &credit_commitment,
        request.amount,
    );
    let remainder_head = balance_head(
        &balance.context,
        balance.wallet_binding,
        balance.guard_device_id,
        balance.hardware_policy_id,
        next_sequence,
        next_lineage_digest,
        remainder_amount,
        &remainder_opening,
    );
    let statement = OfflineCashTransferStatementV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        release_id: balance.context.release_id,
        network_id: balance.context.network_id.clone(),
        asset: balance.context.asset.clone(),
        scale: balance.context.scale,
        amount: request.amount,
        request_digest,
        sender_before: balance.head,
        sender_after: remainder_head,
        receiver_before: request.receiver_balance_commitment,
        credit_commitment,
        transition_digest: [0; 32],
    }
    .seal_transition()
    .map_err(|_| StateTransitionErrorV1::CorruptPlan)?;
    let challenge = GuardChallengeV1::new(
        GuardOperationV1::SendSplit,
        balance,
        statement.transition_digest,
    )?;
    let intent_challenge = HardwareIntentChallengeV1::new(
        HardwareIntentKindV1::SendPublished,
        balance,
        challenge.digest,
        request.issued_at_ms,
        request.expires_at_ms,
    )?;
    Ok(SendSplitPlanV1 {
        expected: BalanceSnapshotV1::capture(balance),
        context: balance.context.clone(),
        request_digest,
        receiver_head: request.receiver_balance_commitment,
        recipient_key_reference: request.recipient_key_reference,
        issued_at_ms: request.issued_at_ms,
        expires_at_ms: request.expires_at_ms,
        amount: request.amount,
        remainder_amount: Zeroizing::new(remainder_amount),
        remainder_head,
        remainder_opening,
        next_lineage_digest,
        split_seed,
        credit_commitment,
        credit_opening,
        statement,
        challenge,
        intent_challenge,
    })
}

fn validate_send_plan(
    balance: &BalanceOwnerV1,
    plan: &SendSplitPlanV1,
) -> Result<(), StateTransitionErrorV1> {
    if !plan.expected.matches(balance) {
        return Err(StateTransitionErrorV1::StaleState);
    }
    let conserved = (*plan.remainder_amount)
        .checked_add(plan.amount)
        .ok_or(StateTransitionErrorV1::ArithmeticOverflow)?;
    let expected_split_seed = offline_cash_send_split_seed_v1(
        &balance.context.digest,
        &balance.wallet_binding,
        &balance.head,
        &balance.opening,
        balance.guard_sequence,
        &plan.request_digest,
        &plan.receiver_head,
        &plan.recipient_key_reference,
        plan.amount,
    );
    let (expected_remainder_opening, expected_credit_opening) =
        offline_cash_send_split_openings_v1(&expected_split_seed);
    let expected_credit = credit_commitment(
        &plan.context,
        plan.request_digest,
        plan.receiver_head,
        plan.recipient_key_reference,
        plan.amount,
        &plan.credit_opening,
    );
    let expected_challenge = GuardChallengeV1::new(
        GuardOperationV1::SendSplit,
        balance,
        plan.statement.transition_digest,
    )?;
    let expected_intent = HardwareIntentChallengeV1::new(
        HardwareIntentKindV1::SendPublished,
        balance,
        expected_challenge.digest,
        plan.issued_at_ms,
        plan.expires_at_ms,
    )?;
    let expected_next_lineage = offline_cash_state_lineage_digest_v1(
        crate::zk::offline_cash_v1::state_abi::OfflineCashStateOperationV1::SendSplit,
        &balance.context.digest,
        &balance.head,
        &balance.lineage_digest,
        balance.guard_sequence,
        expected_challenge.to_sequence,
        &plan.request_digest,
        &plan.receiver_head,
        &plan.credit_commitment,
        plan.amount,
    );
    let expected_remainder_head = balance_head(
        &plan.context,
        balance.wallet_binding,
        balance.guard_device_id,
        balance.hardware_policy_id,
        expected_challenge.to_sequence,
        expected_next_lineage,
        *plan.remainder_amount,
        &plan.remainder_opening,
    );
    if conserved != balance.amount
        || plan.context != balance.context
        || plan.remainder_opening != expected_remainder_opening
        || plan.credit_opening != expected_credit_opening
        || plan.split_seed != expected_split_seed
        || plan.next_lineage_digest != expected_next_lineage
        || expected_remainder_head != plan.remainder_head
        || expected_credit != plan.credit_commitment
        || plan.statement.version != OFFLINE_CASH_WIRE_VERSION_V1
        || plan.statement.release_id != plan.context.release_id
        || plan.statement.network_id != plan.context.network_id
        || plan.statement.asset != plan.context.asset
        || plan.statement.scale != plan.context.scale
        || plan.statement.amount != plan.amount
        || plan.statement.request_digest != plan.request_digest
        || plan.statement.sender_before != balance.head
        || plan.statement.sender_after != plan.remainder_head
        || plan.statement.receiver_before != plan.receiver_head
        || plan.statement.credit_commitment != plan.credit_commitment
        || plan.challenge != expected_challenge
        || plan.intent_challenge != expected_intent
        || plan.statement.validate().is_err()
    {
        return Err(StateTransitionErrorV1::CorruptPlan);
    }
    Ok(())
}

fn canonical_payment_digest(
    plan: &SendSplitPlanV1,
    request: &OfflineCashPaymentRequestV1,
    payment: &OfflineCashPaymentV1,
) -> Result<Digest, StateTransitionErrorV1> {
    let request_digest = request
        .canonical_digest()
        .map_err(|_| StateTransitionErrorV1::InvalidRequest)?;
    if request_digest != plan.request_digest
        || request.receiver_balance_commitment != plan.receiver_head
        || request.recipient_key_reference != plan.recipient_key_reference
        || request.issued_at_ms != plan.issued_at_ms
        || request.expires_at_ms != plan.expires_at_ms
        || request.amount != plan.amount
        || payment.request_digest != plan.request_digest
        || payment.statement != plan.statement
    {
        return Err(StateTransitionErrorV1::RequestMismatch);
    }
    payment
        .canonical_digest_against(request)
        .map_err(|_| StateTransitionErrorV1::CreditMismatch)
}

fn payment_from_outbox_record(
    record: AuthenticatedPaymentOutboxRecordV1,
    key: &PaymentOutboxKeyV1,
    expected_payment_digest: Digest,
    expected_publication_digest: Option<Digest>,
    plan: &SendSplitPlanV1,
    request: &OfflineCashPaymentRequestV1,
) -> Result<OfflineCashPaymentV1, StateTransitionErrorV1> {
    if record.key() != key
        || record.payment_digest() != expected_payment_digest
        || expected_payment_digest == [0; 32]
        || expected_publication_digest
            .is_some_and(|digest| record.publication_digest() != Some(digest))
    {
        return Err(StateTransitionErrorV1::PaymentOutbox(
            AuthenticatedPaymentOutboxErrorV1::Corrupt,
        ));
    }
    let payment =
        OfflineCashPaymentV1::decode_canonical_exact_against(record.canonical_payment(), request)
            .map_err(|_| {
            StateTransitionErrorV1::PaymentOutbox(AuthenticatedPaymentOutboxErrorV1::Corrupt)
        })?;
    let canonical = Zeroizing::new(norito::encode_canonical(&payment).map_err(|_| {
        StateTransitionErrorV1::PaymentOutbox(AuthenticatedPaymentOutboxErrorV1::Corrupt)
    })?);
    if canonical.as_slice() != record.canonical_payment()
        || canonical_payment_digest(plan, request, &payment)? != expected_payment_digest
    {
        return Err(StateTransitionErrorV1::PaymentOutbox(
            AuthenticatedPaymentOutboxErrorV1::Corrupt,
        ));
    }
    Ok(payment)
}

fn send_output(plan: &SendSplitPlanV1) -> SendSplitOutputV1 {
    SendSplitOutputV1 {
        credit: OutgoingCreditOwnerV1 {
            context: plan.context.clone(),
            request_digest: plan.request_digest,
            receiver_head: plan.receiver_head,
            recipient_key_reference: plan.recipient_key_reference,
            amount: plan.amount,
            commitment: plan.credit_commitment,
            send_transition_digest: plan.statement.transition_digest,
            opening: plan.credit_opening.clone(),
        },
        statement: plan.statement.clone(),
    }
}

/// Opaque, move-only authority for one durably staged but unpublished payment.
///
/// The owner contains neither canonical payment bytes nor a decodable payment.
/// Those remain exclusively in the authenticated durable outbox until the
/// hardware journal has installed the exact send intent.
#[must_use]
pub(crate) struct UnpublishedPaymentOwnerV1 {
    plan: SendSplitPlanV1,
    outbox_key: PaymentOutboxKeyV1,
    payment_digest: Digest,
}

impl fmt::Debug for UnpublishedPaymentOwnerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UnpublishedPaymentOwnerV1")
            .field("outbox_key", &self.outbox_key.digest())
            .field("payment_digest", &self.payment_digest)
            .field("canonical_payment", &"[DURABLE OUTBOX]")
            .finish_non_exhaustive()
    }
}

/// Consume and durably stage a canonical payment without exposing it again.
pub(crate) fn stage_send_split_payment_v1<O>(
    balance: &BalanceOwnerV1,
    plan: SendSplitPlanV1,
    request: &OfflineCashPaymentRequestV1,
    payment: OfflineCashPaymentV1,
    outbox: &O,
) -> Result<UnpublishedPaymentOwnerV1, StateTransitionErrorV1>
where
    O: AuthenticatedPaymentOutboxBackendV1,
{
    validate_send_plan(balance, &plan)?;
    let payment_digest = canonical_payment_digest(&plan, request, &payment)?;
    let canonical_payment = Zeroizing::new(
        norito::encode_canonical(&payment).map_err(|_| StateTransitionErrorV1::CreditMismatch)?,
    );
    let outbox_key = PaymentOutboxKeyV1::new(balance, &plan);
    outbox
        .stage_payment_or_recover(&outbox_key, payment_digest, &canonical_payment)
        .map_err(StateTransitionErrorV1::PaymentOutbox)?;
    let staged_digest = outbox
        .recover_staged_payment_digest(&outbox_key)
        .map_err(StateTransitionErrorV1::PaymentOutbox)?;
    if staged_digest != payment_digest {
        return Err(StateTransitionErrorV1::PaymentOutbox(
            AuthenticatedPaymentOutboxErrorV1::Corrupt,
        ));
    }
    Ok(UnpublishedPaymentOwnerV1 {
        plan,
        outbox_key,
        payment_digest,
    })
}

/// Recover the opaque owner for an exact durably staged payment after restart.
pub(crate) fn recover_unpublished_send_payment_v1<O>(
    balance: &BalanceOwnerV1,
    plan: SendSplitPlanV1,
    request: &OfflineCashPaymentRequestV1,
    outbox: &O,
) -> Result<UnpublishedPaymentOwnerV1, StateTransitionErrorV1>
where
    O: AuthenticatedPaymentOutboxBackendV1,
{
    validate_send_plan(balance, &plan)?;
    let request_digest = request
        .canonical_digest()
        .map_err(|_| StateTransitionErrorV1::InvalidRequest)?;
    if request_digest != plan.request_digest {
        return Err(StateTransitionErrorV1::RequestMismatch);
    }
    let outbox_key = PaymentOutboxKeyV1::new(balance, &plan);
    let payment_digest = outbox
        .recover_staged_payment_digest(&outbox_key)
        .map_err(StateTransitionErrorV1::PaymentOutbox)?;
    if payment_digest == [0; 32] {
        return Err(StateTransitionErrorV1::PaymentOutbox(
            AuthenticatedPaymentOutboxErrorV1::Corrupt,
        ));
    }
    Ok(UnpublishedPaymentOwnerV1 {
        plan,
        outbox_key,
        payment_digest,
    })
}

/// Move-only published sender intent awaiting one exact verified acknowledgement.
#[must_use]
pub(crate) struct PublishedSendOwnerV1 {
    pub(super) plan: SendSplitPlanV1,
    pub(super) intent_authorization: HardwareIntentAuthorizationOwnerV1,
    pub(super) payment_digest: Digest,
}

impl fmt::Debug for PublishedSendOwnerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PublishedSendOwnerV1")
            .field("request_digest", &self.plan.request_digest)
            .field("operation_id", &self.plan.statement.transition_digest)
            .field("intent_epoch", &self.intent_authorization.epoch)
            .field("payment_digest", &self.payment_digest)
            .finish_non_exhaustive()
    }
}

/// Move-only result of the existing V1 acknowledgement verifier.
#[must_use]
pub(crate) struct VerifiedAcknowledgementOwnerV1 {
    request_digest: Digest,
    payment_digest: Digest,
    receiver_head: Digest,
    acknowledgement_digest: Digest,
}

impl fmt::Debug for VerifiedAcknowledgementOwnerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedAcknowledgementOwnerV1")
            .field("request_digest", &self.request_digest)
            .field("payment_digest", &self.payment_digest)
            .field("receiver_head", &self.receiver_head)
            .finish_non_exhaustive()
    }
}

impl VerifiedAcknowledgementOwnerV1 {
    pub(crate) const fn request_digest(&self) -> Digest {
        self.request_digest
    }

    pub(crate) const fn payment_digest(&self) -> Digest {
        self.payment_digest
    }
}

/// Verify the receiver's V1 ACK and convert it into an unforgeable local owner.
pub(crate) fn bind_verified_acknowledgement_v1<V>(
    verifier: &OfflineCashTerminalVerifierV1<'_, V>,
    request: &OfflineCashPaymentRequestV1,
    payment: &OfflineCashPaymentV1,
    acknowledgement: &OfflineCashAcknowledgementV1,
    receipt: &VerifiedOfflineCashCreditV1,
) -> Result<VerifiedAcknowledgementOwnerV1, StateTransitionErrorV1>
where
    V: OfflineCashPairedProofVerifierV1,
{
    verifier
        .verify_acknowledgement(request, payment, acknowledgement, receipt)
        .map_err(|_| StateTransitionErrorV1::AcknowledgementMismatch)?;
    let acknowledgement_bytes = norito::encode_canonical(acknowledgement)
        .map_err(|_| StateTransitionErrorV1::AcknowledgementMismatch)?;
    Ok(VerifiedAcknowledgementOwnerV1 {
        request_digest: acknowledgement.request_digest,
        payment_digest: acknowledgement.payment_digest,
        receiver_head: acknowledgement.receiver_balance_commitment,
        acknowledgement_digest: digest_framed(
            ACKNOWLEDGEMENT_OWNER_DOMAIN,
            &[&acknowledgement_bytes],
        ),
    })
}

#[cfg(test)]
pub(super) fn verified_acknowledgement_for_test_v1(
    request_digest: Digest,
    payment_digest: Digest,
    receiver_head: Digest,
    acknowledgement_digest: Digest,
) -> VerifiedAcknowledgementOwnerV1 {
    VerifiedAcknowledgementOwnerV1 {
        request_digest,
        payment_digest,
        receiver_head,
        acknowledgement_digest,
    }
}

/// Atomically bind a staged canonical payment before exposing any send output.
///
/// The opaque owner carries no payment bytes. Core validates the authenticated
/// staged record, performs the hardware journal CAS, durably marks the record
/// publishable, and only then returns the canonical payment.
pub(crate) fn publish_send_split_v1<B, O>(
    balance: &BalanceOwnerV1,
    unpublished: UnpublishedPaymentOwnerV1,
    request: &OfflineCashPaymentRequestV1,
    session: &HardwareGuardSessionV1<'_, B>,
    outbox: &O,
    now_ms: u64,
) -> Result<
    (
        PublishedSendOwnerV1,
        SendSplitOutputV1,
        OfflineCashPaymentV1,
    ),
    StateTransitionErrorV1,
>
where
    B: ExactNextHardwareGuardBackendV1,
    O: AuthenticatedPaymentOutboxBackendV1,
{
    let UnpublishedPaymentOwnerV1 {
        plan,
        outbox_key,
        payment_digest,
    } = unpublished;
    validate_send_plan(balance, &plan)?;
    if PaymentOutboxKeyV1::new(balance, &plan) != outbox_key {
        return Err(StateTransitionErrorV1::CorruptPlan);
    }
    if now_ms < plan.issued_at_ms || now_ms >= plan.expires_at_ms {
        return Err(StateTransitionErrorV1::RequestNotLive);
    }
    let staged_digest = outbox
        .recover_staged_payment_digest(&outbox_key)
        .map_err(StateTransitionErrorV1::PaymentOutbox)?;
    if staged_digest != payment_digest {
        return Err(StateTransitionErrorV1::PaymentOutbox(
            AuthenticatedPaymentOutboxErrorV1::Corrupt,
        ));
    }
    let authorization = session.publish_send_payment(&plan.intent_challenge, payment_digest)?;
    if !intent_authorization_matches(&authorization, &plan.intent_challenge, payment_digest) {
        return Err(StateTransitionErrorV1::HardwareIntentMismatch);
    }
    let publication = authorize_publication(outbox_key, &authorization, payment_digest)?;
    let published_record = outbox
        .publish_payment_or_recover(&publication)
        .map_err(StateTransitionErrorV1::PaymentOutbox)?;
    let payment = payment_from_outbox_record(
        published_record,
        &outbox_key,
        payment_digest,
        Some(publication.authorization_digest()),
        &plan,
        request,
    )?;
    let output = send_output(&plan);
    Ok((
        PublishedSendOwnerV1 {
            plan,
            intent_authorization: authorization,
            payment_digest,
        },
        output,
        payment,
    ))
}

/// Recover an active sender publication and its exact published bytes.
///
/// This path does not emit the private credit owner a second time. It can mark
/// a staged record publishable after a crash between the hardware CAS and the
/// outbox update, but it cannot expose bytes when no matching hardware intent
/// exists.
pub(crate) fn recover_published_send_v1<B, O>(
    balance: &BalanceOwnerV1,
    plan: SendSplitPlanV1,
    request: &OfflineCashPaymentRequestV1,
    session: &HardwareGuardSessionV1<'_, B>,
    outbox: &O,
) -> Result<(PublishedSendOwnerV1, OfflineCashPaymentV1), StateTransitionErrorV1>
where
    B: ExactNextHardwareGuardBackendV1,
    O: AuthenticatedPaymentOutboxBackendV1,
{
    validate_send_plan(balance, &plan)?;
    let outbox_key = PaymentOutboxKeyV1::new(balance, &plan);
    let payment_digest = outbox
        .recover_staged_payment_digest(&outbox_key)
        .map_err(StateTransitionErrorV1::PaymentOutbox)?;
    if payment_digest == [0; 32] {
        return Err(StateTransitionErrorV1::PaymentOutbox(
            AuthenticatedPaymentOutboxErrorV1::Corrupt,
        ));
    }
    let authorization = session.recover_send_payment(&plan.intent_challenge, payment_digest)?;
    let publication = authorize_publication(outbox_key, &authorization, payment_digest)?;
    let published = outbox
        .publish_payment_or_recover(&publication)
        .map_err(StateTransitionErrorV1::PaymentOutbox)?;
    let payment = payment_from_outbox_record(
        published,
        &outbox_key,
        payment_digest,
        Some(publication.authorization_digest()),
        &plan,
        request,
    )?;
    let recovered = outbox
        .recover_published_payment(&outbox_key)
        .map_err(StateTransitionErrorV1::PaymentOutbox)?;
    let recovered_payment = payment_from_outbox_record(
        recovered,
        &outbox_key,
        payment_digest,
        Some(publication.authorization_digest()),
        &plan,
        request,
    )?;
    if recovered_payment != payment {
        return Err(StateTransitionErrorV1::PaymentOutbox(
            AuthenticatedPaymentOutboxErrorV1::Corrupt,
        ));
    }
    Ok((
        PublishedSendOwnerV1 {
            plan,
            intent_authorization: authorization,
            payment_digest,
        },
        payment,
    ))
}

/// Successful sender finalization metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SendSplitCommitOutputV1 {
    pub(crate) request_digest: Digest,
    pub(crate) payment_digest: Digest,
    pub(crate) acknowledgement_digest: Digest,
    pub(crate) remainder_head: Digest,
}

/// Failed finalization returns both move-only recovery owners.
#[must_use]
pub(crate) struct SendSplitCommitRejectionV1 {
    error: StateTransitionErrorV1,
    published: PublishedSendOwnerV1,
    acknowledgement: VerifiedAcknowledgementOwnerV1,
}

impl SendSplitCommitRejectionV1 {
    pub(crate) const fn error(&self) -> StateTransitionErrorV1 {
        self.error
    }

    pub(crate) fn into_owners(self) -> (PublishedSendOwnerV1, VerifiedAcknowledgementOwnerV1) {
        (self.published, self.acknowledgement)
    }
}

fn validate_send_terminal(
    plan: &SendSplitPlanV1,
    outcome: &HardwareTerminalOutcomeV1,
) -> Result<(Digest, Digest), StateTransitionErrorV1> {
    let payment_digest = outcome
        .payment_digest()
        .ok_or(StateTransitionErrorV1::HardwareIntentMismatch)?;
    let acknowledgement_digest = outcome
        .acknowledgement_digest()
        .ok_or(StateTransitionErrorV1::AcknowledgementMismatch)?;
    if outcome.operation() != HardwareTerminalOperationV1::SendCommitted
        || outcome.intent() != &plan.intent_challenge.hardware_request()
        || outcome.from_sequence() != plan.challenge.from_sequence
        || outcome.to_sequence() != plan.challenge.to_sequence
        || outcome.intent_binding_digest() != payment_digest
        || outcome.completion_digest() != acknowledgement_digest
        || outcome.successor_head() != plan.remainder_head
        || payment_digest == [0; 32]
        || acknowledgement_digest == [0; 32]
    {
        return Err(StateTransitionErrorV1::HardwareIntentMismatch);
    }
    Ok((payment_digest, acknowledgement_digest))
}

fn apply_send_terminal(
    balance: &mut BalanceOwnerV1,
    plan: SendSplitPlanV1,
    payment_digest: Digest,
    acknowledgement_digest: Digest,
) -> SendSplitCommitOutputV1 {
    let output = SendSplitCommitOutputV1 {
        request_digest: plan.request_digest,
        payment_digest,
        acknowledgement_digest,
        remainder_head: plan.remainder_head,
    };
    balance.amount = *plan.remainder_amount;
    balance.head = plan.remainder_head;
    balance.lineage_digest = plan.next_lineage_digest;
    balance.opening = plan.remainder_opening;
    balance.guard_sequence = plan.challenge.to_sequence;
    output
}

/// Commit the deterministic sender remainder only after an exact verified ACK.
pub(crate) fn finalize_send_split_v1<B>(
    balance: &mut BalanceOwnerV1,
    published: PublishedSendOwnerV1,
    acknowledgement: VerifiedAcknowledgementOwnerV1,
    session: &HardwareGuardSessionV1<'_, B>,
) -> Result<SendSplitCommitOutputV1, SendSplitCommitRejectionV1>
where
    B: ExactNextHardwareGuardBackendV1,
{
    let validation = validate_send_plan(balance, &published.plan).and_then(|()| {
        if acknowledgement.request_digest != published.plan.request_digest
            || acknowledgement.payment_digest != published.payment_digest
            || acknowledgement.receiver_head == [0; 32]
            || acknowledgement.receiver_head == published.plan.receiver_head
            || acknowledgement.acknowledgement_digest == [0; 32]
        {
            return Err(StateTransitionErrorV1::AcknowledgementMismatch);
        }
        Ok(())
    });
    if let Err(error) = validation {
        return Err(SendSplitCommitRejectionV1 {
            error,
            published,
            acknowledgement,
        });
    }
    let (authorization, outcome) = match session.commit_intent_exact_next(
        &published.intent_authorization,
        &published.plan.challenge,
        published.payment_digest,
        Some(acknowledgement.acknowledgement_digest),
        acknowledgement.acknowledgement_digest,
        published.plan.remainder_head,
    ) {
        Ok(result) => result,
        Err(error) => {
            return Err(SendSplitCommitRejectionV1 {
                error,
                published,
                acknowledgement,
            });
        }
    };
    if !guard_authorization_matches(&authorization, &published.plan.challenge) {
        return Err(SendSplitCommitRejectionV1 {
            error: StateTransitionErrorV1::GuardBindingMismatch,
            published,
            acknowledgement,
        });
    }
    let (payment_digest, acknowledgement_digest) =
        match validate_send_terminal(&published.plan, &outcome) {
            Ok(bindings) => bindings,
            Err(error) => {
                return Err(SendSplitCommitRejectionV1 {
                    error,
                    published,
                    acknowledgement,
                });
            }
        };
    Ok(apply_send_terminal(
        balance,
        published.plan,
        payment_digest,
        acknowledgement_digest,
    ))
}

/// Repair local sender state after hardware committed but the process crashed.
pub(crate) fn recover_committed_send_split_v1<B>(
    balance: &mut BalanceOwnerV1,
    plan: SendSplitPlanV1,
    session: &HardwareGuardSessionV1<'_, B>,
) -> Result<SendSplitCommitOutputV1, StateTransitionErrorV1>
where
    B: ExactNextHardwareGuardBackendV1,
{
    validate_send_plan(balance, &plan)?;
    let outcome = session.recover_terminal_outcome(&plan.intent_challenge)?;
    let (payment_digest, acknowledgement_digest) = validate_send_terminal(&plan, &outcome)?;
    Ok(apply_send_terminal(
        balance,
        plan,
        payment_digest,
        acknowledgement_digest,
    ))
}
