//! Authentication of existing QueuePlan custody, without fresh admission authority.

use iroha_core::tx::{
    AcceptTransactionFail, AcceptedTransaction, SignatureRejectionCode, SignatureVerificationFail,
};
use iroha_crypto::HashOf;
use iroha_data_model::{
    isi::error::Mismatch,
    transaction::{
        SignedTransaction, TransactionAdmissionIntent, TransactionDomain, TransactionEntrypoint,
    },
};

use crate::{Error, NetworkId};

/// A verified signed intent in one network. It cannot enter the transaction Queue.
///
/// Transaction hashes exclude authorization proofs, so registry membership alone
/// cannot authenticate a retry. Fresh policy (TTL, clock health, limits and allowed
/// algorithms) is deliberately absent from this boundary. The registry must still
/// establish existing custody before these identities authorize an acknowledgment.
pub(super) struct AuthenticatedQueuePlanRetry {
    entrypoint_hash: HashOf<TransactionEntrypoint>,
    signed_transaction_hash: HashOf<SignedTransaction>,
}

impl AuthenticatedQueuePlanRetry {
    pub(super) fn from_signed(
        network_id: &NetworkId,
        signed: &SignedTransaction,
    ) -> Result<Option<Self>, Error> {
        if signed.admission_intent() != TransactionAdmissionIntent::QueuePlanSynced {
            return Ok(None);
        }
        Self::check_network(network_id, signed)?;
        signed.verify_signature().map_err(|error| {
            Error::AcceptTransaction(AcceptTransactionFail::SignatureVerification(
                SignatureVerificationFail::new(
                    signed.signature().clone(),
                    SignatureRejectionCode::InvalidSignature,
                    error.to_string(),
                ),
            ))
        })?;
        Ok(Some(Self {
            entrypoint_hash: signed.hash_as_entrypoint(),
            signed_transaction_hash: signed.hash(),
        }))
    }

    pub(super) fn from_entrypoint(
        network_id: &NetworkId,
        entrypoint: &TransactionEntrypoint,
    ) -> Result<Option<Self>, Error> {
        let signed = match entrypoint {
            TransactionEntrypoint::External(signed) => signed,
            TransactionEntrypoint::SealedReveal(reveal) => reveal.signed_transaction(),
            TransactionEntrypoint::SealedCommitment(_) => return Ok(None),
        };
        let Some(mut authenticated) = Self::from_signed(network_id, signed)? else {
            return Ok(None);
        };
        // Reveal custody belongs to the complete outer entrypoint, not merely
        // the enclosed signed transaction. No execution or reveal policy runs.
        authenticated.entrypoint_hash = entrypoint.hash();
        Ok(Some(authenticated))
    }

    /// Reuse the proof already owned by a fully accepted transaction. This path
    /// serves the final race check after fresh admission; it grants no bypass to
    /// an unaccepted input and does not repeat cryptography outside its worker.
    pub(super) fn from_accepted(
        network_id: &NetworkId,
        accepted: &AcceptedTransaction<'_>,
    ) -> Result<Option<Self>, Error> {
        let entrypoint = accepted.entrypoint();
        if entrypoint.admission_intent() != TransactionAdmissionIntent::QueuePlanSynced {
            return Ok(None);
        }
        let signed = match entrypoint {
            TransactionEntrypoint::External(signed) => signed,
            TransactionEntrypoint::SealedReveal(reveal) => reveal.signed_transaction(),
            TransactionEntrypoint::SealedCommitment(_) => return Ok(None),
        };
        Self::check_network(network_id, signed)?;
        Ok(Some(Self {
            entrypoint_hash: entrypoint.hash(),
            signed_transaction_hash: signed.hash(),
        }))
    }

    fn check_network(network_id: &NetworkId, signed: &SignedTransaction) -> Result<(), Error> {
        let expected = TransactionDomain::Network(*network_id);
        if signed.domain() != &expected {
            return Err(Error::AcceptTransaction(
                AcceptTransactionFail::TransactionDomainMismatch(Mismatch {
                    expected,
                    actual: *signed.domain(),
                }),
            ));
        }
        Ok(())
    }

    pub(super) fn entrypoint_hash(&self) -> HashOf<TransactionEntrypoint> {
        self.entrypoint_hash
    }

    pub(super) fn signed_transaction_hash(&self) -> HashOf<SignedTransaction> {
        self.signed_transaction_hash
    }
}
