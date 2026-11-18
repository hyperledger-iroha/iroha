//! Selective Network input and output membership under a quorum-certified execution.
//!
//! A compact certificate authenticates the certified network-input and typed-output roots,
//! not the Iroha block header. This is the compact sibling of
//! [`crate::query::CommittedTransaction::verify_selective_in_authenticated_execution`]: it
//! binds one exact external entrypoint and its joined Network output to those certified
//! roots, without the uncertified header binding and without a block body.

use super::*;
use crate::{
    block::execution_output::{ExecutionOutputV1, NetworkExecutionOutputV1, TransactionResultRef},
    transaction::{SignedTransaction, TransactionResult},
};
use iroha_crypto::MerkleProof;

/// Selective Network input/output membership under a QC-certified `ExecutionCommitment`.
///
/// Decoding is only a shape check. [`VerifiedSumeragiCommitV1::verify_external_execution`]
/// authenticates it under a certificate that an independently selected reader verified.
#[derive(
    Debug, Clone, PartialEq, Eq, Encode, Decode, iroha_schema::IntoSchema, norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::CertifiedTransactionInclusionV1")]
pub struct CertifiedTransactionInclusionV1 {
    /// Membership of the exact entrypoint in the certified network-input tree.
    pub entry_proof: MerkleProof<TransactionEntrypoint>,
    /// The complete joined output row; only a Network row is admissible.
    pub output: ExecutionOutputV1,
    /// Membership of that complete row in the certified typed-output tree.
    pub output_proof: MerkleProof<ExecutionOutputV1>,
}

impl CertifiedTransactionInclusionV1 {
    /// Check the certified network input's source position. A governance prefix shifts every
    /// network output equally. Both independent membership paths must still verify under the QC.
    ///
    /// # Errors
    /// Refuses an internal output or an impossible input/output position.
    pub fn joined_output_row(&self) -> Result<&ExecutionOutputV1, FinalityError> {
        let Some(input) = self.output.network_index() else {
            return Err(FinalityError(
                "inclusion output is not a Network row".into(),
            ));
        };
        need(
            input == self.entry_proof.leaf_index(),
            "inclusion entry leaf differs from the output input index",
        )?;
        need(
            self.output
                .matches_network_proof_positions(input, self.output_proof.leaf_index()),
            "inclusion output leaf has an impossible governance prefix",
        )?;
        Ok(&self.output)
    }

    /// Check the source join and borrow its ordinary network result.
    ///
    /// # Errors
    /// Refuses any other output family or an impossible source position.
    pub fn joined_network_output(&self) -> Result<&NetworkExecutionOutputV1, FinalityError> {
        self.joined_output_row()?
            .network()
            .ok_or_else(|| FinalityError("external execution uses another result family".into()))
    }
}

impl SumeragiCommitCertificateV1 {
    /// Read custody coordinates from the original certificate. The projected result is
    /// unauthenticated until a selected reader verifies both membership paths and the QC.
    pub(crate) fn retained_inclusion_coordinates(
        &self,
        inclusion: &CertifiedTransactionInclusionV1,
    ) -> Result<(u64, Hash32), FinalityReadError> {
        let (header, qc, result) = self.originals()?;
        let execution = &result.commitment().execution;
        execution.validate().map_err(malformed)?;
        let (Some(inputs), Some(outputs)) = (
            execution.transaction_input_commitment,
            execution.transaction_output_commitment,
        ) else {
            return Err(FinalityError("retained execution has no network inputs".into()).into());
        };
        inclusion.joined_output_row()?;
        let input = u64::from(inclusion.entry_proof.leaf_index());
        let output = u64::from(inclusion.output_proof.leaf_index());
        // The Network rows form a contiguous group. The signed inputs following this
        // one each require another output, even when the block carries a governance prefix.
        let remaining_inputs = inputs.leaf_count().get().checked_sub(input + 1);
        let remaining_outputs = outputs.leaf_count().get().checked_sub(output + 1);
        need(
            remaining_inputs
                .zip(remaining_outputs)
                .is_some_and(|(i, o)| i <= o),
            "retained Network coordinates disagree with the input and output counts",
        )?;
        Ok((header.height, qc.block_hash))
    }
}

impl VerifiedSumeragiCommitV1 {
    /// Authenticate the exact external entrypoint's membership and its joined Network output.
    ///
    /// Returns the certified result: `Ok` is an applied transaction and `Err` its
    /// authenticated rejection. The caller owns the outcome predicate over the signed wire.
    /// Genesis is excluded by construction: only `SumeragiCommitVerifierV1::verify` builds
    /// this capability, for heights from two. A commitment with more leaves than a `u32`
    /// leaf index addresses fails its membership proof in `MerkleProof::verify`.
    ///
    /// # Errors
    /// A foreign network, an invalid signature, missing commitments, a non-Network or
    /// unjoined output, or a membership proof that differs.
    pub fn verify_external_execution<'a>(
        &self,
        network: &NetworkId,
        signed: &SignedTransaction,
        inclusion: &'a CertifiedTransactionInclusionV1,
    ) -> Result<&'a TransactionResult, FinalityError> {
        match self.verify_profiled_external_execution(network, signed, inclusion)? {
            TransactionResultRef::Baseline(result) => Ok(result),
            TransactionResultRef::C10(_) => Err(FinalityError(
                "external execution uses the C10 result family".into(),
            )),
        }
    }

    /// Authenticate any selected execution family against its exact signed input and full output.
    ///
    /// # Errors
    /// Refuses an invalid signature, foreign network, source position or membership proof.
    pub fn verify_profiled_external_execution<'a>(
        &self,
        network: &NetworkId,
        signed: &SignedTransaction,
        inclusion: &'a CertifiedTransactionInclusionV1,
    ) -> Result<TransactionResultRef<'a>, FinalityError> {
        need(
            self.network == *network,
            "compact certificate network differs",
        )?;
        need(
            signed.network_id() == Some(network) && signed.verify_signature().is_ok(),
            "external entrypoint does not match the certified network",
        )?;
        let entrypoint = signed.try_hash_as_entrypoint().map_err(malformed)?;
        let execution = self.execution();
        execution.validate().map_err(malformed)?;
        let (Some(inputs), Some(outputs)) = (
            execution.transaction_input_commitment,
            execution.transaction_output_commitment,
        ) else {
            return Err(FinalityError(
                "certified execution has no network inputs".into(),
            ));
        };
        let output = inclusion.joined_output_row()?;
        need(
            inclusion.entry_proof.verify(&entrypoint, &inputs),
            "external entrypoint is not a certified network input",
        )?;
        let output_hash = HashOf::try_new(&inclusion.output).map_err(malformed)?;
        need(
            inclusion.output_proof.verify(&output_hash, &outputs),
            "joined output is not a certified execution output",
        )?;
        Ok(output.result())
    }
}

#[cfg(test)]
mod tests;
