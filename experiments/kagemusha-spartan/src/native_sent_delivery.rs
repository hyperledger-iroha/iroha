//! Read-only delivery evidence for an outgoing Send and its pending descriptor.
use super::*;
use crate::{bootstrap_context as bc, send_maps};
use iroha_data_model::kagemusha::KagemushaWalletIndexedRemoveV1;

/// Receiver originals that one CreditStatus authenticated at a folded receiver head: the
/// quoted credential of the recipient-signed open Request, the head's restored receiver
/// hardware receipt checked under that credential's payment key, the head's statement,
/// the 64 public words of the decided folded pair and the exact credit-digest opening
/// under its credit root. The head is either the Receive of this Payment (`KGSCST3`) or a
/// later receiver Send that covers its credit. `statement[16]` names the kind, and
/// `statement[17]` is the credit that the head's signed operation id names. Read-only data.
/// Only the native CreditStatus verifiers fill it, and only
/// [`NativeEngine::verify_sent_delivery`] places it in a [`VerifiedSentDelivery`].
#[derive(Clone)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct ReceiverEvidence {
    pub(crate) credential: Credential,
    pub(crate) receipt: bootstrap::Receipt,
    pub(crate) statement: [Fp; 26],
    pub(crate) profile: [Fp; 64],
    pub(crate) exact_pair: Vec<u8>,
    /// The receiver pair decided under the signed program for this exact projected head.
    #[cfg(test)]
    pub(crate) pair: crate::full::UniformPair,
    pub(crate) next: Fp,
    pub(crate) slot: u32,
    pub(crate) siblings: [Fp; 32],
}

/// A locally released Send and the recipient's authenticated folded delivery evidence.
/// The exact peer originals are borrowed for a caller that retains or rechecks the evidence.
pub struct VerifiedSentDelivery<'a> {
    open_request: &'a [u8],
    credit_status: &'a [u8],
    payment: VerifiedIncomingPayment,
    outcome: VerifiedCreditStatus,
    operation_id: [u8; 32],
    pending_descriptor: [u8; 32],
    receiver: ReceiverEvidence,
}

impl VerifiedSentDelivery<'_> {
    /// The delivery verdict proven by the receiver.
    pub fn outcome(&self) -> VerifiedCreditStatus {
        self.outcome
    }

    /// Receiver originals authenticated by either CreditStatus projection.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn receiver_evidence(&self) -> &ReceiverEvidence {
        &self.receiver
    }

    /// The historical local Send operation id.
    pub fn operation_id(&self) -> &[u8; 32] {
        &self.operation_id
    }

    /// The exact recipient-signed open Request bytes.
    pub fn open_request(&self) -> &[u8] {
        self.open_request
    }

    /// The verified original Payment released by the selected Send.
    pub fn payment(&self) -> &VerifiedIncomingPayment {
        &self.payment
    }

    /// The exact receiver-folded CreditStatus bytes.
    pub fn credit_status(&self) -> &[u8] {
        self.credit_status
    }

    /// Check the two indexed-map openings against a released payer state at or after
    /// this Send. The caller owns selection of the current release. Returns its
    /// hypothetical post-removal root without changing the wallet.
    pub fn verify_pending_removal(
        &self,
        released: &CompletedOperation,
        removal: &KagemushaWalletIndexedRemoveV1,
    ) -> Result<[u8; 32]> {
        let head = released.head();
        let current_sequence = u128::from_le_bytes(
            released.completion.exact_receipt()[98..114]
                .try_into()
                .unwrap(),
        );
        let send_sequence =
            u128::from_le_bytes(self.payment.receipt.body[98..114].try_into().unwrap());
        // Scheme, asset and wallet id stay fixed across a credential renewal.
        if current_sequence < send_sequence
            || head.parameters_id != self.payment.parameters_id
            || head.manifest_digest != self.payment.manifest_digest
            || head.credential.body[2..98] != self.payment.payer.body[2..98]
        {
            return Err(Error::AuthorityMismatch("delivery pending wallet"));
        }
        if removal.leaf.value != self.pending_descriptor {
            return Err(Error::AuthorityMismatch("delivery pending descriptor"));
        }
        let root = head.profile[bc::LINEAGE_IO + bc::PENDING_ROOT].to_repr();
        removal
            .verify(&root, &self.payment.statement[17].to_repr())
            .map_err(|_| Error::AuthorityMismatch("delivery pending openings"))
    }
}

impl NativeEngine {
    /// Join a verified local Send to its original Payment, signed open Request and
    /// folded receiver verdict. A successor can recheck its pending leaf at a later head.
    pub fn verify_sent_delivery<'a>(
        &self,
        prepared: &PreparedOperation,
        released: &CompletedOperation,
        open_request: &'a [u8],
        credit_status: &'a [u8],
        parameters: &AuthenticatedParameters,
    ) -> Result<VerifiedSentDelivery<'a>> {
        let ExactOperationWitness::Send(send) = prepared.witness.as_ref() else {
            return Err(Error::AuthorityMismatch(
                "delivery requires a released Send",
            ));
        };
        released.head.require_parameters(parameters)?;
        released.released_core(prepared)?;
        let payment = self.verify_payment(&released.completion.output_part(1)?, parameters)?;
        let descriptor = iroha_pasta::poseidon::hash_with_domain(
            send_maps::PENDING_DOMAIN,
            &payment.statement[17..24],
        );
        if send.statement != payment.statement
            || send.maps.pending.key != payment.statement[17]
            || send.maps.pending.value != descriptor
            || released.head.profile != payment.profile
        {
            return Err(Error::AuthorityMismatch(
                "delivery released Send descriptor",
            ));
        }
        let (outcome, receiver) =
            self.verify_credit_status_evidence(credit_status, open_request, &payment, parameters)?;
        Ok(VerifiedSentDelivery {
            open_request,
            credit_status,
            payment,
            outcome,
            operation_id: *prepared.capsule.operation_id(),
            pending_descriptor: descriptor.to_repr(),
            receiver,
        })
    }
}
