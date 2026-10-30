//! Abort authority derived only from successful native execution of the original carrier.

use super::*;
use iroha_data_model::{
    block::consensus::{NexusFeeReceipt, SumeragiRootScope},
    nexus::PrivateSettlementAbortReceiptV1,
    query::CommittedTransaction,
    sumeragi_finality::VerifiedSumeragiBlock,
    transaction::TransactionEntrypoint,
};

/// A successful original Abort carrier, authenticated from independently selected genesis.
///
/// A public receipt, a rejected transaction, local cancellation or an expiry observation
/// cannot construct this capability. It authorizes cleanup of this exact payment's local
/// reservation; it does not establish that its inputs remain unspent by another payment.
///
/// ```compile_fail
/// use iroha::client::VerifiedPrivateSettlementAbort;
/// let _ = norito::decode_canonical::<VerifiedPrivateSettlementAbort>(&[]);
/// ```
#[derive(Clone, Debug)]
pub struct VerifiedPrivateSettlementAbort {
    manifest: AtomicPrivateSettlementV1,
    receipt: PrivateSettlementAbortReceiptV1,
    context_id: Hash,
    evidence: CommittedTransaction,
}

impl VerifiedPrivateSettlementAbort {
    /// Authenticate one known successful Abort transaction against the original manifest.
    /// # Errors
    /// Foreign root, substituted manifest/carrier/fees, rejection or invalid native evidence.
    pub fn verify(
        block: &VerifiedSumeragiBlock,
        expected: &AtomicPrivateSettlementV1,
        hash: HashOf<SignedTransaction>,
        evidence: CommittedTransaction,
    ) -> std::result::Result<Self, PrivateSettlementReceiptError> {
        let receipt = Self::verify_recovered(block, expected, evidence)?;
        if receipt.transaction_hash() != hash {
            return Err(PrivateSettlementReceiptError::BindingMismatch);
        }
        Ok(receipt)
    }

    /// Recover an Abort from native execution, using the caller's complete original manifest.
    /// # Errors
    /// The same authentication and complete-manifest checks as [`Self::verify`].
    pub fn verify_recovered(
        block: &VerifiedSumeragiBlock,
        expected: &AtomicPrivateSettlementV1,
        evidence: CommittedTransaction,
    ) -> std::result::Result<Self, PrivateSettlementReceiptError> {
        use PrivateSettlementReceiptError as Error;
        expected.validate().map_err(|_| Error::InvalidEvidence)?;
        if block.root_scope() != SumeragiRootScope::Global
            || block.commitment().schedule.current.network_id != expected.network_id
        {
            return Err(Error::BindingMismatch);
        }
        block.verify_transaction_execution(&expected.network_id, &evidence)?;
        if !evidence.result().is_ok() {
            return Err(Error::Rejected);
        }
        let TransactionEntrypoint::External(transaction) = evidence.entrypoint() else {
            return Err(Error::BindingMismatch);
        };
        verified::check_carrier_context(transaction.payload(), expected)?;
        let Executable::Instructions(instructions) = transaction.instructions() else {
            return Err(Error::BindingMismatch);
        };
        let [instruction] = instructions.as_ref() else {
            return Err(Error::BindingMismatch);
        };
        let abort = instruction
            .as_any()
            .downcast_ref::<AbortAtomicPrivateSettlementV1>()
            .ok_or(Error::BindingMismatch)?;
        if &abort.manifest != expected
            || block.height() < expected.authority_context_height
            || (block.height() > expected.expiry_height)
                != (abort.reason == PrivateSettlementAbortReasonV1::Expired)
        {
            return Err(Error::BindingMismatch);
        }
        if let Some(fee) = evidence.result().nexus_fee_receipt() {
            fee.validate_for_network_input(evidence.entrypoint(), block.height())
                .map_err(|_| Error::InvalidEvidence)?;
        }
        let receipt = PrivateSettlementAbortReceiptV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            network_id: expected.network_id,
            bundle_id: expected.bundle_id,
            manifest_digest: expected
                .manifest_digest()
                .map_err(|_| Error::InvalidEvidence)?,
            finalized_height: block.height(),
            reason: abort.reason,
        };
        receipt.validate().map_err(|_| Error::InvalidEvidence)?;
        Ok(Self {
            manifest: expected.clone(),
            receipt,
            context_id: block.context_id(),
            evidence,
        })
    }

    /// Exact original manifest authenticated by the successful Abort carrier.
    #[must_use]
    pub const fn manifest(&self) -> &AtomicPrivateSettlementV1 {
        &self.manifest
    }

    /// Authenticated public abort marker; this is not a successful payment receipt.
    #[must_use]
    pub const fn receipt(&self) -> &PrivateSettlementAbortReceiptV1 {
        &self.receipt
    }

    /// Exact original successful Abort transaction identity.
    #[must_use]
    pub fn transaction_hash(&self) -> HashOf<SignedTransaction> {
        let TransactionEntrypoint::External(transaction) = self.evidence.entrypoint() else {
            unreachable!("verified direct Abort carrier")
        };
        transaction.hash()
    }

    /// Original authenticated consensus context.
    #[must_use]
    pub const fn context_id(&self) -> Hash {
        self.context_id
    }

    /// Actual authenticated charge, preserving absence rather than fabricating zero.
    #[must_use]
    pub fn nexus_fee_receipt(&self) -> Option<&NexusFeeReceipt> {
        self.evidence.result().nexus_fee_receipt()
    }

    /// Exact native evidence; decoding this value does not recover verified authority.
    #[must_use]
    pub const fn evidence(&self) -> &CommittedTransaction {
        &self.evidence
    }
}
