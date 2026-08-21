//! Outgoing, decrypted, and terminal-verified credit owners.

use super::*;

pub(super) fn credit_commitment(
    context: &OfflineCashStateContextV1,
    request_digest: Digest,
    receiver_head: Digest,
    recipient_key_reference: Digest,
    amount: u128,
    opening: &[u8; 32],
) -> Digest {
    offline_cash_credit_head_v1(
        &context.digest,
        &request_digest,
        &receiver_head,
        &recipient_key_reference,
        amount,
        opening,
    )
}

/// Move-only sender-side private credit branch awaiting encryption and proof verification.
#[must_use]
pub(crate) struct OutgoingCreditOwnerV1 {
    pub(super) context: OfflineCashStateContextV1,
    pub(super) request_digest: Digest,
    pub(super) receiver_head: Digest,
    pub(super) recipient_key_reference: Digest,
    pub(super) amount: u128,
    pub(super) commitment: Digest,
    pub(super) send_transition_digest: Digest,
    pub(super) opening: Zeroizing<Digest>,
}

impl fmt::Debug for OutgoingCreditOwnerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OutgoingCreditOwnerV1")
            .field("context_digest", &self.context.digest)
            .field("request_digest", &self.request_digest)
            .field("receiver_head", &self.receiver_head)
            .field("commitment", &self.commitment)
            .field("send_transition_digest", &self.send_transition_digest)
            .field("amount", &"[REDACTED]")
            .field("opening", &"[REDACTED]")
            .finish()
    }
}

impl OutgoingCreditOwnerV1 {
    /// Return the receiver-bound credit commitment.
    pub(crate) const fn commitment(&self) -> Digest {
        self.commitment
    }

    /// Return the common `SendSplit` digest shared with the sender remainder.
    pub(crate) const fn send_transition_digest(&self) -> Digest {
        self.send_transition_digest
    }
}

/// Move-only result of authenticated receiver-side credit decryption.
///
/// It is not a wire value and has no public constructor. The privileged
/// constructor is the hand-off point from an AEAD implementation that has
/// already authenticated the ciphertext and key reference.
#[must_use]
pub(crate) struct DecryptedCreditOpeningOwnerV1 {
    pub(super) opening: Zeroizing<Digest>,
    pub(super) encrypted_credit_digest: Digest,
    pub(super) recipient_key_reference: Digest,
}

impl fmt::Debug for DecryptedCreditOpeningOwnerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DecryptedCreditOpeningOwnerV1")
            .field("encrypted_credit_digest", &self.encrypted_credit_digest)
            .field("recipient_key_reference", &self.recipient_key_reference)
            .field("opening", &"[REDACTED]")
            .finish()
    }
}

impl DecryptedCreditOpeningOwnerV1 {
    /// Mint an opening owner only at the authenticated-decryption boundary.
    pub(super) fn from_authenticated_decryption(
        opening: Zeroizing<Digest>,
        encrypted_credit: &[u8],
        recipient_key_reference: Digest,
    ) -> Result<Self, StateTransitionErrorV1> {
        if opening.iter().all(|byte| *byte == 0)
            || encrypted_credit.is_empty()
            || recipient_key_reference == [0; 32]
        {
            return Err(StateTransitionErrorV1::CreditMismatch);
        }
        Ok(Self {
            opening,
            encrypted_credit_digest: Sha256::digest(encrypted_credit).into(),
            recipient_key_reference,
        })
    }
}

/// Receiver credit owner inseparably retaining terminal paired-proof verification.
#[must_use]
pub(crate) struct CreditOwnerV1 {
    pub(super) context: OfflineCashStateContextV1,
    pub(super) request_digest: Digest,
    pub(super) receiver_head: Digest,
    pub(super) recipient_key_reference: Digest,
    pub(super) amount: u128,
    pub(super) commitment: Digest,
    pub(super) send_transition_digest: Digest,
    pub(super) payment_digest: Digest,
    pub(super) opening: Zeroizing<Digest>,
    pub(super) verification: VerifiedOfflineCashCreditV1,
}

impl fmt::Debug for CreditOwnerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CreditOwnerV1")
            .field("context_digest", &self.context.digest)
            .field("request_digest", &self.request_digest)
            .field("receiver_head", &self.receiver_head)
            .field("commitment", &self.commitment)
            .field("send_transition_digest", &self.send_transition_digest)
            .field("payment_digest", &self.payment_digest)
            .field("terminal_verification", &"retained")
            .field("amount", &"[REDACTED]")
            .field("opening", &"[REDACTED]")
            .finish()
    }
}

impl CreditOwnerV1 {
    /// Return the proof-bound credit commitment.
    pub(crate) const fn commitment(&self) -> Digest {
        self.commitment
    }

    /// Return the proof-bound common `SendSplit` transition digest.
    pub(crate) const fn send_transition_digest(&self) -> Digest {
        self.send_transition_digest
    }

    /// Return restart-reconstructible verification and opening inputs.
    pub(super) fn into_recovery_inputs(
        self,
    ) -> (VerifiedOfflineCashCreditV1, DecryptedCreditOpeningOwnerV1) {
        let encrypted_credit_digest = self.verification.encrypted_credit_digest();
        let recipient_key_reference = self.recipient_key_reference;
        (
            self.verification,
            DecryptedCreditOpeningOwnerV1 {
                opening: self.opening,
                encrypted_credit_digest,
                recipient_key_reference,
            },
        )
    }
}

pub(super) fn terminal_credit_matches(credit: &CreditOwnerV1) -> bool {
    credit.verification.release_id() == credit.context.release_id
        && credit.verification.request_digest() == credit.request_digest
        && credit.verification.network_id() == &credit.context.network_id
        && credit.verification.asset() == &credit.context.asset
        && credit.verification.scale() == credit.context.scale
        && credit.verification.amount() == credit.amount
        && credit.verification.receiver_before() == credit.receiver_head
        && credit.verification.recipient_key_reference() == credit.recipient_key_reference
        && credit.verification.credit_commitment() == credit.commitment
        && credit.verification.transition_digest() == credit.send_transition_digest
        && credit.verification.payment_digest() == credit.payment_digest
        && credit.payment_digest != [0; 32]
}

/// Failed terminal-token/opening binding, retaining both move-only inputs for retry.
#[must_use]
pub(crate) struct CreditBindingRejectionV1 {
    error: StateTransitionErrorV1,
    pub(super) verification: VerifiedOfflineCashCreditV1,
    pub(super) opening: DecryptedCreditOpeningOwnerV1,
}

impl CreditBindingRejectionV1 {
    /// Return the exact binding failure.
    pub(crate) const fn error(&self) -> StateTransitionErrorV1 {
        self.error
    }

    /// Recover the terminal verification and decrypted opening owners.
    pub(crate) fn into_owners(
        self,
    ) -> (VerifiedOfflineCashCreditV1, DecryptedCreditOpeningOwnerV1) {
        (self.verification, self.opening)
    }
}

/// Bind a terminal proof decision and authenticated decryption into one credit owner.
pub(crate) fn bind_verified_credit_v1(
    pending: &PendingOwnerV1,
    statement: &OfflineCashTransferStatementV1,
    verification: VerifiedOfflineCashCreditV1,
    opening: DecryptedCreditOpeningOwnerV1,
) -> Result<CreditOwnerV1, CreditBindingRejectionV1> {
    let terminal_binding_matches = statement.validate().is_ok()
        && verification.release_id() == pending.context.release_id
        && verification.request_digest() == pending.request_digest
        && verification.network_id() == &pending.context.network_id
        && verification.asset() == &pending.context.asset
        && verification.scale() == pending.context.scale
        && verification.amount() == pending.amount
        && verification.receiver_before() == pending.receiver_head
        && verification.recipient_key_reference() == pending.recipient_key_reference
        && verification.credit_commitment() == statement.credit_commitment
        && verification.transition_digest() == statement.transition_digest
        && statement.release_id == pending.context.release_id
        && statement.network_id == pending.context.network_id
        && statement.asset == pending.context.asset
        && statement.scale == pending.context.scale
        && statement.amount == pending.amount
        && statement.request_digest == pending.request_digest
        && statement.receiver_before == pending.receiver_head;
    if !terminal_binding_matches {
        return Err(CreditBindingRejectionV1 {
            error: StateTransitionErrorV1::TerminalVerificationMismatch,
            verification,
            opening,
        });
    }
    if verification.encrypted_credit_digest() != opening.encrypted_credit_digest
        || verification.recipient_key_reference() != opening.recipient_key_reference
    {
        return Err(CreditBindingRejectionV1 {
            error: StateTransitionErrorV1::EncryptedOpeningMismatch,
            verification,
            opening,
        });
    }
    let expected_commitment = credit_commitment(
        &pending.context,
        pending.request_digest,
        pending.receiver_head,
        pending.recipient_key_reference,
        pending.amount,
        &opening.opening,
    );
    if expected_commitment != verification.credit_commitment() {
        return Err(CreditBindingRejectionV1 {
            error: StateTransitionErrorV1::CreditMismatch,
            verification,
            opening,
        });
    }
    Ok(CreditOwnerV1 {
        context: pending.context.clone(),
        request_digest: pending.request_digest,
        receiver_head: pending.receiver_head,
        recipient_key_reference: pending.recipient_key_reference,
        amount: pending.amount,
        commitment: verification.credit_commitment(),
        send_transition_digest: verification.transition_digest(),
        payment_digest: verification.payment_digest(),
        opening: opening.opening,
        verification,
    })
}
