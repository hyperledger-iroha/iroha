//! Native lane frame custody retains the original prepaid selected configuration.
//!
//! This reader shares the canonical file decoder and the exact QC/RS16 verifiers. The
//! caller chooses a hash beneath an independently authenticated global frontier. Header/QC
//! metadata and BLS caches remain separate resource-accounting obligations.

use std::{borrow::Borrow, io, path::PathBuf};

use iroha_allocation::{AllocationBudget, RetainedPayload};
use iroha_sumeragi::{
    availability::{AvailabilitySource, AvailableBody, BodyRestoration},
    crypto::{Crypto, NoAttestation, Verifier},
    message::{BlockHeader, Qc},
    types::{Hash32, HeightConfig},
};

use crate::sumeragi::{
    body_read::BodyReadError,
    lanes::{
        record::LaneRecord,
        store::read::{ReadRecord, RecordPoll},
    },
};

/// Move-only original configuration; no public extraction or metadata clone detaches its ledger.
pub(in crate::sumeragi) struct FundedLaneSource(RetainedPayload<AvailabilitySource>);
impl Borrow<AvailabilitySource> for FundedLaneSource {
    fn borrow(&self) -> &AvailabilitySource {
        self.0.get()
    }
}
impl FundedLaneSource {
    #[expect(
        unsafe_code,
        reason = "only move the same prepaid config fields into a scalar source wrapper"
    )]
    pub(in crate::sumeragi) fn new(
        config: RetainedPayload<HeightConfig>,
        instance: Hash32,
        height: u64,
        hash: Hash32,
        budget: &AllocationBudget,
    ) -> Result<Self, (RetainedPayload<HeightConfig>, io::Error)> {
        if !config.belongs_to(budget) || !config.get().epoch.contains(height) {
            return Err((config, io::ErrorKind::InvalidInput.into()));
        }
        // SAFETY: AvailabilitySource adds only fixed scalar identity. Its private config is
        // exactly the moved original config; no allocation is cloned, exported or replaced.
        // Source and restoration expose only shared borrows, and FundedLaneSource is not Clone.
        let source = unsafe {
            config.map_payload(|config| {
                AvailabilitySource::new(instance, height, hash, config)
                    .expect("the exact original epoch bound was checked above")
            })
        };
        Ok(Self(source))
    }
    fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.0.belongs_to(budget)
    }
}

pub(in crate::sumeragi) struct FundedLaneBody {
    pub(in crate::sumeragi) body: AvailableBody<FundedLaneSource>,
    pub(in crate::sumeragi) qc: Qc,
}
impl Borrow<BlockHeader> for FundedLaneBody {
    fn borrow(&self) -> &BlockHeader {
        self.body.header()
    }
}

enum Stage {
    Unopened,
    Reading(ReadRecord),
    Decoded(LaneRecord),
    Restoring(BodyRestoration<FundedLaneSource>, Qc),
    Consumed,
}
/// A read never substitutes its path, source, configuration, or original finite pool on retry.
pub(in crate::sumeragi) struct FundedLaneFrameRead {
    path: PathBuf,
    budget: AllocationBudget,
    source: Option<FundedLaneSource>,
    stage: Stage,
}
impl FundedLaneFrameRead {
    pub(in crate::sumeragi) fn new(
        path: PathBuf,
        source: FundedLaneSource,
        budget: AllocationBudget,
    ) -> Self {
        Self {
            path,
            source: Some(source),
            budget,
            stage: Stage::Unopened,
        }
    }
    pub(in crate::sumeragi) fn poll(
        &mut self,
        budget: &AllocationBudget,
        crypto: &dyn Crypto,
    ) -> io::Result<FundedLaneBody> {
        if !self.budget.same_pool(budget)
            || self
                .source
                .as_ref()
                .is_some_and(|source| !source.belongs_to(budget))
        {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        // A stricter caller-owned Norito scope is not a fact about the durable frame.
        // Keep the same read owner for retry after that scope ends; intrinsic canonical
        // limits without an outer scope still report invalid storage material.
        let outer_decode_scope = norito::core::decode_limits_active();
        loop {
            match std::mem::replace(&mut self.stage, Stage::Consumed) {
                Stage::Unopened => match ReadRecord::open(&self.path, budget.clone()) {
                    Ok(read) => self.stage = Stage::Reading(read),
                    Err(error) => {
                        self.stage = Stage::Unopened;
                        return Err(error);
                    }
                },
                Stage::Reading(mut read) => match read.poll(budget) {
                    Ok(RecordPoll::Ready(record)) => self.stage = Stage::Decoded(record),
                    outcome => {
                        self.stage = Stage::Reading(read);
                        return Err(match outcome {
                            Ok(RecordPoll::Absent) => io::ErrorKind::NotFound.into(),
                            Ok(RecordPoll::Pending(_)) => io::ErrorKind::WouldBlock.into(),
                            Err(BodyReadError::Io(error)) => error,
                            Err(BodyReadError::Decode(error))
                                if matches!(error, norito::Error::AllocationFailed { .. })
                                    || (outer_decode_scope && error.is_decode_resource_limit()) =>
                            {
                                io::ErrorKind::WouldBlock.into()
                            }
                            Err(BodyReadError::Admission(error)) if error.is_local_refusal() => {
                                io::ErrorKind::WouldBlock.into()
                            }
                            Err(error) => io::Error::new(io::ErrorKind::InvalidData, error),
                            Ok(RecordPoll::Ready(_)) => unreachable!(),
                        });
                    }
                },
                Stage::Decoded(record) => {
                    let source = self
                        .source
                        .as_ref()
                        .expect("source remains before restoration");
                    let expected = source.borrow();
                    let header = record.header();
                    let valid = record.check_context(expected, crypto).is_ok()
                        && !header.attest
                        && header.control_witness.is_empty()
                        && Verifier::new(
                            crypto,
                            &expected.instance(),
                            &expected.config().epoch.id,
                            &expected.config().committee,
                        )
                        .verify_commit_qc(
                            &NoAttestation,
                            record.commit_qc(),
                            Some(header),
                        );
                    if !valid {
                        self.stage = Stage::Decoded(record);
                        return Err(io::ErrorKind::InvalidData.into());
                    }
                    let source = self
                        .source
                        .take()
                        .expect("same original owner checked above");
                    let (restoration, qc) = record.into_restoration(source);
                    self.stage = Stage::Restoring(restoration, qc);
                }
                Stage::Restoring(restoration, qc) => match restoration.complete(budget, crypto) {
                    Ok(body) => return Ok(FundedLaneBody { body, qc }),
                    Err((restoration, error)) => {
                        self.stage = Stage::Restoring(restoration, qc);
                        if error.is_local_refusal() {
                            return Err(io::ErrorKind::WouldBlock.into());
                        }
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("original lane availability restoration: {error:?}"),
                        ));
                    }
                },
                Stage::Consumed => return Err(io::ErrorKind::InvalidInput.into()),
            }
        }
    }
}

#[cfg(test)]
mod tests;
