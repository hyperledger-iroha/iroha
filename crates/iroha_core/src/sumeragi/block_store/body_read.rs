//! Source-bound Kura projection, retaining exact certificate and proposal backing on refusal.

use std::{io, sync::Arc};

use iroha_allocation::AllocationBudget;
use iroha_data_model::{block::SignedBlock, sumeragi_finality::result_of_preimage};
use iroha_sumeragi::{
    availability::{AvailabilitySource, BodyRestoration},
    message::VoteKind,
};

use super::certificate_read::{CertificateRead, CertificateReadError, DecodedCertificate};
use crate::sumeragi::{
    body_read::{BodyReadError, BodyReadJob, BodyReadPoll},
    driver::{
        SharedCrypto,
        payload_build::{PayloadBuild, PayloadBuildError},
    },
};

enum Stage {
    Absent,
    Certificate(CertificateRead),
    Decoded(DecodedCertificate),
    Projecting(PayloadBuild<DecodedCertificate>),
    Consumed,
}

/// A source selected independently of the committed frame, with one retained decode/projection.
/// The caller supplies None only for real absence, never for a failed Kura read below its tip.
pub(super) struct StoredBodyRead {
    source: AvailabilitySource,
    budget: AllocationBudget,
    crypto: SharedCrypto,
    stage: Stage,
}
impl StoredBodyRead {
    #[cfg(test)]
    pub(super) fn new(
        source: AvailabilitySource,
        block: Option<Arc<SignedBlock>>,
        budget: AllocationBudget,
        crypto: SharedCrypto,
    ) -> Self {
        let stage = block.map_or(Stage::Absent, |block| {
            Stage::Certificate(CertificateRead::new(block, budget.clone()))
        });
        Self {
            source,
            budget,
            crypto,
            stage,
        }
    }

    /// Transfer the same original decoded owners from full committed-certificate verification.
    pub(super) fn from_decoded(
        source: AvailabilitySource,
        decoded: DecodedCertificate,
        budget: AllocationBudget,
        crypto: SharedCrypto,
    ) -> Self {
        Self {
            source,
            budget,
            crypto,
            stage: Stage::Decoded(decoded),
        }
    }

    fn check(&self, decoded: &DecodedCertificate) -> Result<(), BodyReadError> {
        let header = &decoded.header;
        let qc = &decoded.commit_qc;
        let certificate = decoded
            .source
            .commit_certificate()
            .expect("decoded original certificate");
        let result = result_of_preimage(certificate.result_preimage());
        if decoded.source.header().height().get() != self.source.height()
            || !decoded.source.has_results()
            || header.instance != self.source.instance()
            || header.height != self.source.height()
            || header.epoch != self.source.config().epoch.id
            || header.hash(&*self.crypto) != self.source.block_hash()
            || qc.kind != VoteKind::Commit
            || qc.instance != header.instance
            || qc.epoch != header.epoch
            || qc.height != header.height
            || qc.block_hash != self.source.block_hash()
            || qc.attest != header.attest
            || qc.result != result
        {
            return Err(BodyReadError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "stored certificate does not bind the independently requested body and result",
            )));
        }
        super::execution::validate(&decoded.source).map_err(BodyReadError::Io)?;
        Ok(())
    }
}

impl BodyReadJob for StoredBodyRead {
    fn source(&self) -> &AvailabilitySource {
        &self.source
    }

    fn poll(&mut self, budget: &AllocationBudget) -> Result<BodyReadPoll, BodyReadError> {
        if !self.budget.same_pool(budget) {
            return Err(BodyReadError::ForeignBudget);
        }
        loop {
            match std::mem::replace(&mut self.stage, Stage::Consumed) {
                Stage::Absent => return Ok(BodyReadPoll::Absent),
                Stage::Certificate(job) => match job.complete(budget) {
                    Ok(decoded) => self.stage = Stage::Decoded(decoded),
                    Err((job, error)) => {
                        self.stage = Stage::Certificate(job);
                        return match error {
                            CertificateReadError::ForeignBudget => {
                                Err(BodyReadError::ForeignBudget)
                            }
                            CertificateReadError::MissingCertificate => {
                                Err(BodyReadError::Io(io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    "stored block has no certificate",
                                )))
                            }
                            CertificateReadError::Decode(error) => {
                                Err(BodyReadError::Decode(error))
                            }
                            CertificateReadError::Admission(error) if error.is_local_refusal() => {
                                Ok(BodyReadPoll::Pending(error))
                            }
                            CertificateReadError::Admission(error) => {
                                Err(BodyReadError::Admission(error))
                            }
                        };
                    }
                },
                Stage::Decoded(decoded) => {
                    if let Err(error) = self.check(&decoded) {
                        self.stage = Stage::Decoded(decoded);
                        return Err(error);
                    }
                    self.stage = Stage::Projecting(PayloadBuild::new(
                        decoded,
                        budget.clone(),
                        self.source.config().params.max_block_bytes as usize,
                    ));
                }
                Stage::Projecting(job) => match job.finish(
                    |decoded| decoded.source.resultless_proposal_wire_len(),
                    |decoded, writer| decoded.source.write_resultless_proposal_wire(writer),
                ) {
                    Ok((decoded, payload)) => {
                        return Ok(BodyReadPoll::Ready(BodyRestoration::new(
                            self.source.clone(),
                            decoded.header,
                            decoded.availability,
                            payload,
                        )));
                    }
                    Err((job, error)) => {
                        self.stage = Stage::Projecting(job);
                        return match error {
                            PayloadBuildError::Admission(error) if error.is_local_refusal() => {
                                Ok(BodyReadPoll::Pending(error))
                            }
                            PayloadBuildError::Admission(error) => {
                                Err(BodyReadError::Admission(error))
                            }
                            PayloadBuildError::Encoding(error) => Err(BodyReadError::Decode(error)),
                            PayloadBuildError::TooLarge | PayloadBuildError::Poisoned => {
                                Err(BodyReadError::Io(io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    "stored proposal cannot produce its bounded canonical payload",
                                )))
                            }
                        };
                    }
                },
                Stage::Consumed => return Err(BodyReadError::Completed),
            }
        }
    }
}

#[cfg(test)]
#[path = "body_read_tests.rs"]
mod tests;
