//! Complete retained Kura authentication: original frame, full QC, and signed body restoration.
use super::body_read::StoredBodyReadPoll;
use super::*;
use certificate_read::{CertificateRead, CertificateReadError, DecodedCertificate};
use iroha_sumeragi::availability::{BodyRestoration, RestorationError};
#[derive(Debug, thiserror::Error)]
#[error("availability restoration: {0:?}")]
struct RestoreFailure(RestorationError);

enum Phase {
    Certificate(CertificateRead),
    Decoded(DecodedCertificate),
    Projecting(body_read::StoredBodyRead),
    Restoring(BodyRestoration, Qc),
    Consumed,
}
pub(super) struct CommittedRead {
    height: u64,
    budget: AllocationBudget,
    crypto: SharedCrypto,
    schedule: Arc<dyn AvailabilitySchedule>,
    verifier: Arc<dyn AttestationVerifier + Send + Sync>,
    phase: Phase,
}
impl CommittedRead {
    pub(super) fn new(
        block: Arc<SignedBlock>,
        height: u64,
        budget: AllocationBudget,
        crypto: SharedCrypto,
        schedule: Arc<dyn AvailabilitySchedule>,
        verifier: Arc<dyn AttestationVerifier + Send + Sync>,
    ) -> Self {
        Self {
            height,
            phase: Phase::Certificate(CertificateRead::new(block, budget.clone())),
            budget,
            crypto,
            schedule,
            verifier,
        }
    }
    pub(super) fn height(&self) -> u64 {
        self.height
    }
    /// Observe a refused original certificate read without progressing or replacing its owner.
    #[cfg(test)]
    pub(super) fn retained_certificate_owners_for_test(
        &self,
    ) -> Option<(*const SignedBlock, Option<*const u8>, Option<*const u8>)> {
        match &self.phase {
            Phase::Certificate(job) => Some(job.retained_owners_for_test()),
            _ => None,
        }
    }
    pub(super) fn poll(&mut self) -> io::Result<(AvailableBody, Qc)> {
        loop {
            match std::mem::replace(&mut self.phase, Phase::Consumed) {
                Phase::Certificate(job) => match job.complete(&self.budget) {
                    Ok(decoded) => self.phase = Phase::Decoded(decoded),
                    Err((job, error)) => {
                        self.phase = Phase::Certificate(job);
                        if matches!(&error, CertificateReadError::Admission(error) if error.is_local_refusal())
                            || (matches!(
                                &error,
                                CertificateReadError::Decode(
                                    norito::Error::AllocationFailed { .. }
                                )
                            ) && !cfg!(all(test, sumeragi_core_mutation = "HC25")))
                        {
                            return Err(io::ErrorKind::WouldBlock.into());
                        }
                        return Err(io::Error::new(io::ErrorKind::InvalidData, error));
                    }
                },
                Phase::Decoded(decoded) => {
                    let source = match certified_source(
                        &*self.schedule,
                        &*self.crypto,
                        &*self.verifier,
                        self.height,
                        &decoded.header,
                        &decoded.commit_qc,
                    ) {
                        Ok(source) => source,
                        Err(error) => {
                            self.phase = Phase::Decoded(decoded);
                            return Err(error);
                        }
                    };
                    self.phase = Phase::Projecting(body_read::StoredBodyRead::from_decoded(
                        source,
                        decoded,
                        self.budget.clone(),
                        self.crypto.clone(),
                    ));
                }
                Phase::Projecting(mut job) => match job.poll_with_qc(&self.budget) {
                    Ok(StoredBodyReadPoll::Ready(restoration, qc)) => {
                        self.phase = Phase::Restoring(restoration, qc)
                    }
                    Ok(StoredBodyReadPoll::Pending(_)) => {
                        self.phase = Phase::Projecting(job);
                        return Err(io::ErrorKind::WouldBlock.into());
                    }
                    Ok(StoredBodyReadPoll::Absent) => {
                        return Err(invalid("retained committed block cannot become absent"));
                    }
                    Err(error) => {
                        self.phase = Phase::Projecting(job);
                        // Keep the original operational category and typed cause so the
                        // outer Kura read slot retains this exact owner across local refusal.
                        return Err(match error {
                            BodyReadError::Io(error) => error,
                            error => io::Error::new(io::ErrorKind::InvalidData, error),
                        });
                    }
                },
                Phase::Restoring(job, qc) => match job.complete(&self.budget, &*self.crypto) {
                    Ok(body) => return Ok((body, qc)),
                    Err((job, error)) => {
                        self.phase = Phase::Restoring(job, qc);
                        if error.is_local_refusal() {
                            return Err(io::ErrorKind::WouldBlock.into());
                        }
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            RestoreFailure(error),
                        ));
                    }
                },
                Phase::Consumed => return Err(invalid("committed read already consumed")),
            }
        }
    }
}
/// A claimed QC never supplies its own authority. Full verification precedes source construction.
pub(super) fn certified_source(
    schedule: &dyn AvailabilitySchedule,
    crypto: &dyn iroha_sumeragi::crypto::Crypto,
    attestations: &dyn AttestationVerifier,
    height: u64,
    header: &BlockHeader,
    qc: &Qc,
) -> io::Result<AvailabilitySource> {
    let instance = schedule.instance();
    if header.height != height || header.instance != instance || qc.height != height {
        return Err(invalid("committed source height or instance mismatch"));
    }
    let config = schedule
        .height_config(height)?
        .ok_or_else(|| busy("authenticated historical authority unavailable"))?;
    if !Verifier::new(crypto, &instance, &config.epoch.id, &config.committee).verify_commit_qc(
        attestations,
        qc,
        Some(header),
    ) {
        return Err(invalid("original commit certificate does not verify"));
    }
    AvailabilitySource::new(instance, height, qc.block_hash, config)
        .map_err(|error| invalid(format!("invalid committed source: {error:?}")))
}

#[cfg(test)]
#[path = "committed_read_tests.rs"]
mod tests;
