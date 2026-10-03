//! Exact startup completion acknowledgement after the original execution owner retires.

use super::super::commitment::{
    CertificatePart, MAX_RESULT_PREIMAGE_BYTES, ResultPreimageError, encode_certificate_part,
};
use super::*;
use crate::state::native_execution_tip::NativeExecutionTip;
use iroha_allocation::{AllocationBudget, ChargedBuffer};
use iroha_crypto::Hash;
use iroha_sumeragi::availability::AvailabilitySource;

/// Only the original serialized worker can move its completed authority into this receipt.
/// There is no decoder, clone, retained witness, result preimage or executed block here.
pub(super) struct CompletedReplay {
    source: AvailabilitySource,
    tip: NativeExecutionTip,
    header: Hash,
    qc: Hash,
    availability: Hash,
    payload: Hash,
}

fn encode(
    part: CertificatePart<'_>,
    budget: &AllocationBudget,
) -> Result<ChargedBuffer<u8>, ResultPreimageError> {
    encode_certificate_part(part, budget, MAX_RESULT_PREIMAGE_BYTES)
}

impl Worker<'_> {
    pub(super) fn replay(
        &mut self,
        block: &AvailableBody,
        qc: &Qc,
    ) -> Result<(), PublicationError> {
        match catch_unwind(AssertUnwindSafe(|| {
            self.replay_with_encoder(block, qc, encode)
        })) {
            Ok(result) => result,
            Err(_) => {
                let reason = "startup replay completion panicked; recovery required".to_owned();
                self.recovery = Some(reason.clone());
                Err(PublicationError::RecoveryRequired(reason))
            }
        }
    }

    fn replay_with_encoder(
        &mut self,
        block: &AvailableBody,
        qc: &Qc,
        mut encode: impl FnMut(
            CertificatePart<'_>,
            &AllocationBudget,
        ) -> Result<ChargedBuffer<u8>, ResultPreimageError>,
    ) -> Result<(), PublicationError> {
        if let Some(reason) = &self.recovery {
            return Err(PublicationError::RecoveryRequired(reason.clone()));
        }
        let budget = self.state.ivm_execution_budget();
        require_body_admission(block, &budget)?;
        require_qc_witness_admission(qc, &budget)?;
        if let Some(completed) = &self.completed_replay
            && completed.tip.height() == block.header().height
        {
            if self.pending_commit.is_some()
                || self.state.view().native_execution_tip() != Some(completed.tip)
                || self.applied != (completed.tip.height(), completed.tip.core_hash())
            {
                return Err(PublicationError::Retryable(
                    "replay completion is not the current fully applied native tip".into(),
                ));
            }
            return completed.acknowledge(block, qc, &budget, &mut encode);
        }
        match self.prepare_with_origin(block, qc, CommitTelemetryOrigin::HistoricalReplay)? {
            Some(result) if result == qc.result => {}
            Some(_) => {
                return Err(PublicationError::Retryable(
                    "replayed block diverges from its certified result".into(),
                ));
            }
            None => {
                return Err(PublicationError::Retryable(
                    "replayed block no longer executes".into(),
                ));
            }
        }
        self.commit(block, qc)?;
        // The same serialized request includes archive completion and retirement. No driver
        // request can replace the original Published owner between those operations.
        if let Err(reason) = self.retire_completed_replay(block, qc) {
            self.recovery = Some(reason.clone());
            return Err(PublicationError::RecoveryRequired(reason));
        }
        Ok(())
    }

    fn retire_completed_replay(&mut self, block: &AvailableBody, qc: &Qc) -> Result<(), String> {
        if self.pending_commit.is_some() || self.finishing.is_some() {
            return Err("replay execution has not completed all publication effects".into());
        }
        let tip = self
            .state
            .view()
            .native_execution_tip()
            .ok_or("replay has no original native execution tip")?;
        let live = self
            .live
            .as_ref()
            .ok_or("replay lost its original published execution")?;
        let PublicationPhase::Published {
            staged,
            qc: original,
        } = &live.phase
        else {
            return Err("replay execution is not published".into());
        };
        if original != qc
            || live.header != *block.header()
            || live.availability != *block.availability()
            || live.source != *block.source()
            || live.telemetry_origin != Some(CommitTelemetryOrigin::HistoricalReplay)
            || live.overlay.is_some()
            || self.applied != (tip.height(), tip.core_hash())
            || tip.height() != live.height
            || tip.core_hash() != live.block_hash
            || tip.result() != live.result
            || tip.iroha_hash() != staged.executed.hash()
        {
            return Err("replay completion differs from its original published source".into());
        }
        let certificate = staged
            .executed
            .commit_certificate()
            .ok_or("published replay lost its certificate")?;
        let header = Hash::new(certificate.consensus_header());
        let qc = Hash::new(certificate.commit_qc());
        let availability = Hash::new(certificate.availability());
        let payload = Hash::new(block.payload().as_slice());
        self.clear_local_attestation()?;
        let live = self.live.take().expect("same serialized published owner");
        self.completed_replay = Some(CompletedReplay {
            source: live.source,
            tip,
            header,
            qc,
            availability,
            payload,
        });
        self.context.staging.clear();
        Ok(())
    }
}

impl CompletedReplay {
    fn acknowledge(
        &self,
        block: &AvailableBody,
        qc: &Qc,
        budget: &AllocationBudget,
        encode: &mut impl FnMut(
            CertificatePart<'_>,
            &AllocationBudget,
        ) -> Result<ChargedBuffer<u8>, ResultPreimageError>,
    ) -> Result<(), PublicationError> {
        // Scratch belongs to the original State pool and is released after each digest.
        // A local refusal changes neither this completion nor any execution state.
        let mut digest = |part| {
            encode(part, budget)
                .map(|bytes| Hash::new(bytes.as_slice()))
                .map_err(|error| {
                    if cfg!(all(test, sumeragi_core_mutation = "HC79")) {
                        PublicationError::Retryable(error.to_string())
                    } else {
                        preparation::encoding_failure(&error)
                    }
                })
        };
        let header = digest(CertificatePart::Header(block.header()))?;
        let qc = digest(CertificatePart::Qc(qc))?;
        let availability = digest(CertificatePart::Availability(block.availability()))?;
        if !cfg!(all(test, sumeragi_core_mutation = "HC57"))
            && (self.source != *block.source()
                || self.payload != Hash::new(block.payload().as_slice())
                || self.header != header
                || self.qc != qc
                || self.availability != availability)
        {
            return Err(PublicationError::Retryable("replay completion differs from the exact original certificate, availability or source".into()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
