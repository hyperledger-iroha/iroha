//! Structural native source/output joins for the canonical global result owner.
//!
//! Additional protocol/nested FASTPQ call keys remain untrusted structural output;
//! Core's actual captured-source inventory and execution witness authenticate them.
//! This module grants neither native source authority nor global execution validity.

use std::collections::{BTreeMap, BTreeSet};

use iroha_crypto::{Hash, HashOf};

use super::SignedBlock;
use crate::{
    fastpq::TransferTranscript, transaction::signed::TransactionResult,
    trigger::TimeTriggerEntrypoint,
};

impl SignedBlock {
    /// Check the native input projection and its actual canonical full outputs.
    ///
    /// Results remain stored once in `BlockResult`. This checks cardinality
    /// and structural ownership only: first-carrier finality, Decision signatures,
    /// execution replay and every captured FASTPQ source require Core validation.
    ///
    /// # Errors
    /// Rejects mixed/malformed native source, absent/misaligned results, duplicate canonical call owners or malformed transcript vectors.
    pub fn validate_native_lane_results(&self) -> Result<(), String> {
        if self
            .execution_context()
            .and_then(|context| context.native_lane_decisions.as_ref())
            .is_none()
        {
            return Ok(());
        }
        let result = self
            .result
            .as_ref()
            .ok_or_else(|| "native carrier has no actual global results".to_owned())?;
        self.validate_native_output_rows(
            &result.time_triggers,
            &result.transaction_results,
            &result.fastpq_transcripts,
        )
    }

    pub(super) fn validate_native_lane_source(&self) -> Result<(), String> {
        let Some(context) = self.execution_context() else {
            return Ok(());
        };
        let Some(batch) = context.native_lane_decisions.as_ref() else {
            return Ok(());
        };
        context.validate_native_lane_decisions_shape()?;
        if self.external_entrypoint_count() != 0
            || self.header().execution_context_hash() != Some(HashOf::new(context))
            || self.header().merkle_root().is_some()
            || batch.base_state_height.checked_add(1) != Some(self.header().height().get())
            || self.header().prev_block_hash().is_none()
            || self.header().creation_time().is_zero()
        {
            return Err("native carrier differs from its sole input/header context".into());
        }
        Ok(())
    }

    pub(super) fn validate_native_output_rows(
        &self,
        time: &[TimeTriggerEntrypoint],
        results: &[TransactionResult],
        transcripts: &BTreeMap<Hash, Vec<TransferTranscript>>,
    ) -> Result<(), String> {
        let Some(context) = self.execution_context() else {
            return Ok(());
        };
        let Some(batch) = context.native_lane_decisions.as_ref() else {
            return Ok(());
        };
        self.validate_native_lane_source()?;
        if results.len() != batch.groups.len() + time.len() {
            return Err("native and Time results do not align with their sole input vector".into());
        }
        // Every vector has one actual execution-call owner. Additional protocol
        // and nested execution keys are allowed here, not authenticated here.
        if transcripts.iter().any(|(owner, values)| {
            values.is_empty()
                || values
                    .iter()
                    .any(|transcript| transcript.batch_hash != *owner)
        }) {
            return Err(
                "global FASTPQ vector is empty or differs from its execution-call key".into(),
            );
        }
        // Actual Time call identities are assigned by Core per invocation. Equal
        // Time display entries are valid and cannot be used as evidence-map keys.
        let mut native_calls = BTreeSet::new();
        for group in &batch.groups {
            if !native_calls.insert(Hash::from(
                group.payload.input.entrypoint.execution_call_hash(),
            )) {
                return Err("native inputs repeat an execution-call owner".into());
            }
        }
        // There is deliberately no proposal output claim to compare here. Missing
        // or substituted actual output is rejected by canonical execution replay
        // and the globally certified executed-wire commitment, not this shape check.
        Ok(())
    }
}
