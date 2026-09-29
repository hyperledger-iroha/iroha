//! Structural joins for the sole global output owner. No execution authority is minted.
use super::{
    SignedBlock,
    execution_output::{ExecutionOutputV1, validate_execution_outputs_v1},
};
use crate::{fastpq::TransferTranscript, transaction::signed::TransactionEntrypoint};
use iroha_crypto::{Hash, HashOf, MerkleTree};
use std::collections::BTreeMap;
impl SignedBlock {
    /// Check every payload commitment without changing the signed proposal header.
    /// # Errors
    /// Rejects any absent, foreign or stale payload commitment.
    pub fn validate_proposal_commitments(&self) -> Result<(), String> {
        let header = self.header();
        let external = MerkleTree::root_from_typed_leaves(
            self.external_entrypoints_slice()
                .iter()
                .map(TransactionEntrypoint::hash),
        );
        if header.merkle_root() != external
            || header.execution_context_hash() != self.execution_context().map(HashOf::new)
            || header.da_proof_policies_hash() != self.da_proof_policies().map(HashOf::new)
            || header.da_commitments_hash()
                != self
                    .da_commitments()
                    .and_then(crate::da::commitment::DaCommitmentBundle::merkle_commitment)
            || header.da_pin_intents_hash()
                != self
                    .da_pin_intents()
                    .and_then(crate::da::pin_intent::DaPinIntentBundle::merkle_commitment)
            || header.npos_effects_hash() != self.npos_consensus_effects().map(HashOf::new)
            || header.global_beacon_pulse_hash() != self.global_beacon_pulse().map(HashOf::new)
        {
            return Err("proposal header commitments differ from their actual payload".into());
        }
        Ok(())
    }
    /// Validate source/phase/cardinality and transcript-vector shape, not actual execution.
    /// # Errors
    /// Rejects missing results or any structural ownership disagreement.
    pub fn validate_execution_result_structure(&self) -> Result<(), String> {
        self.validate_proposal_commitments()?;
        let result = self
            .result
            .as_ref()
            .ok_or("block has no execution outputs")?;
        result
            .axt_policy_snapshot
            .validate()
            .map_err(|error| error.to_string())?;
        self.validate_output_rows(&result.outputs, &result.fastpq_transcripts)
    }

    pub(super) fn validate_output_rows(
        &self,
        outputs: &[ExecutionOutputV1],
        transcripts: &BTreeMap<Hash, Vec<TransferTranscript>>,
    ) -> Result<(), String> {
        validate_execution_outputs_v1(outputs, self.hash(), self.header().height().get(), self)?;
        if transcripts
            .iter()
            .any(|(owner, rows)| rows.is_empty() || rows.iter().any(|row| row.batch_hash != *owner))
        {
            return Err("FASTPQ vector is empty or differs from its actual call key".into());
        }
        // Extra protocol/nested keys remain structural, not authenticated, here.
        // Core's actual source inventory and exact global executed wire authenticate them.
        Ok(())
    }
}
