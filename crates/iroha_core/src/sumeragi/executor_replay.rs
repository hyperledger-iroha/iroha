//! Exact, process-local replay completion after retiring the original execution frame.

use super::*;
use iroha_sumeragi::types::{ChainParams, Committee, EpochConfig, HeightConfig};

/// Complete source configuration without retaining any cloned graph or payload allocation.
#[derive(Clone, Copy, PartialEq, Eq)]
struct ReplaySource {
    instance: Hash32,
    height: u64,
    block_hash: Hash32,
    epoch: EpochConfig,
    params: ChainParams,
    committee_digest: iroha_crypto::Hash,
}

impl ReplaySource {
    fn capture(source: &iroha_sumeragi::availability::AvailabilitySource) -> Self {
        // Exhaustive destructuring makes every future height-configuration field
        // an explicit obligation here. Authority alone does not bind parameters.
        let HeightConfig {
            epoch,
            committee,
            params,
        } = source.config();
        Self {
            instance: source.instance(),
            height: source.height(),
            block_hash: source.block_hash(),
            epoch: **epoch,
            params: *params,
            committee_digest: committee_digest(committee),
        }
    }
}

/// Hash the existing counted, length-prefixed committee preimage without scratch allocation.
fn committee_digest(committee: &Committee) -> iroha_crypto::Hash {
    iroha_crypto::Hash::new_from_writer(|writer| {
        writer.write_all(iroha_sumeragi::preimage::TAG_COMMITTEE)?;
        writer.write_all(
            &u32::try_from(committee.n())
                .expect("validated committee size fits u32")
                .to_be_bytes(),
        )?;
        for key in committee.members() {
            let bytes = key.as_bytes();
            writer.write_all(
                &u16::try_from(bytes.len())
                    .expect("validated public key length fits u16")
                    .to_be_bytes(),
            )?;
            writer.write_all(bytes)?;
        }
        Ok(())
    })
    .expect("incremental hash writer cannot fail")
}

/// Issued only after this worker's original replay and archive completion succeed.
/// No decoder or external constructor grants this process-local completion authority.
/// Fixed hashes retain exact canonical fields without retaining the large R frame.
#[derive(Clone, Copy)]
pub(super) struct ReplayCompletion {
    tip: crate::state::NativeExecutionTip,
    source: ReplaySource,
    header: Hash32,
    qc: Hash32,
    availability: Hash32,
}

impl Worker<'_> {
    pub(super) fn replay(
        &mut self,
        block: &AvailableBody,
        qc: &Qc,
    ) -> Result<(), PublicationError> {
        if let Some(reason) = &self.recovery {
            return Err(PublicationError::RecoveryRequired(reason.clone()));
        }
        let budget = self.state.ivm_execution_budget();
        require_body_admission(block, &budget)?;
        require_qc_witness_admission(qc, &budget)?;
        if let Some(completed) = self.replay_completion {
            if self.applied.0 == block.header().height
                && completed.tip.height() == block.header().height
            {
                return self.verify_replay_completion(completed, block, qc);
            }
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
        // Publication may complete before a local capture refusal. The original
        // Published owner stays available until this capture can safely retry.
        let completed = self.capture_replay_completion()?;
        self.replay_completion = Some(completed);
        // Startup has no driver to retire its large completed receipt. Capture
        // only fixed evidence after archive completion, then release that owner.
        self.discard(block.header().height, &[]);
        self.context.staging.clear();
        Ok(())
    }

    fn capture_replay_completion(&self) -> Result<ReplayCompletion, PublicationError> {
        let invalid = |message: &str| PublicationError::Retryable(message.into());
        let live = self
            .live
            .as_ref()
            .ok_or_else(|| invalid("completed replay lost its original execution"))?;
        let PublicationPhase::Published { staged, qc } = &live.phase else {
            return Err(invalid("replay completion requires original publication"));
        };
        if self.pending_commit.is_some()
            || live.telemetry_origin != Some(CommitTelemetryOrigin::HistoricalReplay)
            || self.applied != (live.height, live.block_hash)
        {
            return Err(invalid(
                "replay completion precedes original archive completion",
            ));
        }
        let view = self
            .state
            .try_view_once()
            .map_err(|error| PublicationError::Retryable(error.to_string()))?
            .ok_or_else(|| invalid("committed publication is busy"))?;
        let tip = view
            .native_execution_tip()
            .ok_or_else(|| invalid("replay completion requires original State authority"))?;
        if tip.height() != live.height
            || tip.core_hash() != live.block_hash
            || tip.result() != qc.result
            || tip.iroha_hash() != staged.executed.hash()
            || view.latest_block_hash() != Some(tip.iroha_hash())
            || view.height() as u64 != tip.height()
        {
            return Err(invalid("replay completion differs from original State tip"));
        }
        let certificate = staged
            .executed
            .commit_certificate()
            .ok_or_else(|| invalid("replay completion lost its original certificate"))?;
        if !certificate.admitted_to(&self.state.ivm_execution_budget()) {
            return Err(invalid(
                "replay completion certificate lost original pool custody",
            ));
        }
        Ok(ReplayCompletion {
            tip,
            source: ReplaySource::capture(&live.source),
            header: super::super::commitment::chain_hash(certificate.consensus_header()),
            qc: super::super::commitment::chain_hash(certificate.commit_qc()),
            availability: super::super::commitment::chain_hash(certificate.availability()),
        })
    }

    fn verify_replay_completion(
        &mut self,
        completed: ReplayCompletion,
        block: &AvailableBody,
        qc: &Qc,
    ) -> Result<(), PublicationError> {
        let invalid = || {
            PublicationError::Retryable("replay differs from original completed execution".into())
        };
        let view = self
            .state
            .try_view_once()
            .map_err(|error| PublicationError::Retryable(error.to_string()))?
            .ok_or_else(|| PublicationError::Retryable("committed publication is busy".into()))?;
        if self.pending_commit.is_some()
            || self.applied != (completed.tip.height(), completed.tip.core_hash())
            || view.native_execution_tip() != Some(completed.tip)
            || view.latest_block_hash() != Some(completed.tip.iroha_hash())
            || view.height() as u64 != completed.tip.height()
        {
            return Err(invalid());
        }
        if !cfg!(all(test, sumeragi_core_mutation = "HC56"))
            && ReplaySource::capture(block.source()) != completed.source
        {
            return Err(invalid());
        }
        // Every retry funds exact canonical encoding from the same State pool.
        // Matching a private completion never waives admission or accepts only
        // selected QC fields, a digest excluding witnesses, or a decoded claim.
        let budget = self.state.ivm_execution_budget();
        for (part, expected) in [
            (
                super::super::commitment::CertificatePart::Header(block.header()),
                completed.header,
            ),
            (
                super::super::commitment::CertificatePart::Qc(qc),
                completed.qc,
            ),
            (
                super::super::commitment::CertificatePart::Availability(block.availability()),
                completed.availability,
            ),
        ] {
            let encoded = super::super::commitment::encode_certificate_part(
                part,
                &budget,
                super::super::commitment::MAX_RESULT_PREIMAGE_BYTES,
            )
            .map_err(|error| {
                if let super::super::commitment::ResultPreimageError::Allocation(
                    iroha_allocation::ChargedBufferError::Admission(ref refusal),
                ) = error
                {
                    self.routing_refusal = Some(refusal.clone().into());
                }
                PublicationError::Retryable(error.to_string())
            })?;
            if super::super::commitment::chain_hash(encoded.as_slice()) != expected {
                return Err(invalid());
            }
        }
        self.routing_refusal = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_committee_hash_streams_exact_counted_key_preimage() {
        for count in [4, 7, 31] {
            let committee = Committee::new(
                (0..count)
                    .map(|index| PublicKey::new(vec![index; 32 + usize::from(index % 3)]).unwrap())
                    .collect(),
            )
            .unwrap();
            assert_eq!(
                committee_digest(&committee),
                iroha_crypto::Hash::new(iroha_sumeragi::preimage::committee_digest_preimage(
                    &committee
                ))
            );
        }
    }
}
