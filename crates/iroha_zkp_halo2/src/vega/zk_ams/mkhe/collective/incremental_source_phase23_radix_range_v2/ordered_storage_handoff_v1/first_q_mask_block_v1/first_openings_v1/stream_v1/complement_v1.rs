//! Consume the complete original S file into its canonical complement openings.
use super::*;
use crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::source_algebra::QMaskComplementOpeningsV1;

enum ComplementContinuationV1<R, K, P> {
    Before(SealedOriginalQMaskSV1<R, K, P>),
    During(StoredQMaskComplementV1<R, K, P>),
}

/// The sole retry owns the unchanged original state before any block read/rho.
#[must_use = "retry only the same original owner after actual resource release"]
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct QMaskComplementRefusalV1
<R, K, P> {
    reason: QMaskSErrorV1,
    original: Option<ComplementContinuationV1<R, K, P>>,
}
impl<R: crate::vega::MaskedRelaxedRandomSourceV1, K, P> QMaskComplementRefusalV1<R, K, P> {
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn reason_v1(
        &self,
    ) -> QMaskSErrorV1 {
        self.reason
    }
    #[allow(
        clippy::result_large_err,
        reason = "capacity retains the original source without allocation"
    )]
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn retry_v1(
        mut self,
    ) -> Result<CompletedOriginalQMaskOpeningsV1<R, K, P>, Self> {
        match self.original.take() {
            Some(ComplementContinuationV1::Before(original)) => {
                original.complete_complement_openings_v1()
            }
            Some(ComplementContinuationV1::During(original)) => original.complete_v1(),
            None => Err(self),
        }
    }
}

struct StoredQMaskComplementV1<R, K, P> {
    file: SealedQMaskSFileV1,
    openings: CompleteQMaskSOpeningsV1,
    complements: QMaskComplementOpeningsV1,
    original: PreparedStoredQMaskKernelV1<R, K, P>,
}

impl<R: crate::vega::MaskedRelaxedRandomSourceV1, K, P> SealedOriginalQMaskSV1<R, K, P> {
    /// One named consumer, with no caller value, rho, ordinal or replacement file.
    #[allow(
        clippy::result_large_err,
        reason = "only pre-operation capacity preserves the original owner"
    )]
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn complete_complement_openings_v1(
        mut self,
    ) -> Result<CompletedOriginalQMaskOpeningsV1<R, K, P>, QMaskComplementRefusalV1<R, K, P>> {
        let result = self
            .original
            .live
            .as_mut()
            .and_then(|owner| owner.live.source.evidence.as_mut())
            .ok_or(QMaskSErrorV1::Source)
            .and_then(|evidence| evidence.begin_q_mask_complements_v1(&self.openings, &self.file));
        let complements = match result {
            Ok(complements) => complements,
            Err(reason) => {
                return Err(QMaskComplementRefusalV1 {
                    reason,
                    original: if reason == QMaskSErrorV1::Capacity {
                        Some(ComplementContinuationV1::Before(self))
                    } else {
                        None
                    },
                });
            }
        };
        let Self {
            file,
            openings,
            original,
        } = self;
        StoredQMaskComplementV1 {
            file,
            openings,
            complements,
            original,
        }
        .complete_v1()
    }
}

impl<R: crate::vega::MaskedRelaxedRandomSourceV1, K, P> StoredQMaskComplementV1<R, K, P> {
    #[allow(
        clippy::result_large_err,
        reason = "refusal retains the same complete preceding block"
    )]
    fn complete_v1(
        mut self,
    ) -> Result<CompletedOriginalQMaskOpeningsV1<R, K, P>, QMaskComplementRefusalV1<R, K, P>> {
        while !self.complements.at_complete_v1() {
            let result = self
                .original
                .live
                .as_mut()
                .and_then(|owner| owner.live.source.evidence.as_mut())
                .ok_or(QMaskSErrorV1::Source)
                .and_then(|evidence| {
                    evidence.produce_q_mask_complement_block_v1(
                        &mut self.openings,
                        &mut self.file,
                        &mut self.complements,
                    )
                });
            if let Err(reason) = result {
                return Err(QMaskComplementRefusalV1 {
                    reason,
                    original: if reason == QMaskSErrorV1::Capacity {
                        Some(ComplementContinuationV1::During(self))
                    } else {
                        None
                    },
                });
            }
        }
        let result = self
            .original
            .live
            .as_mut()
            .and_then(|owner| owner.live.source.evidence.as_mut())
            .ok_or(QMaskSErrorV1::Source)
            .and_then(|evidence| {
                evidence.finish_q_mask_complements_v1(&self.openings, &self.file, &self.complements)
            });
        match result {
            Ok(()) => Ok(CompletedOriginalQMaskOpeningsV1 { owner: self }),
            Err(reason) => Err(QMaskComplementRefusalV1 {
                reason,
                original: None,
            }),
        }
    }
}

/// Both original6400-rho sets, original immutable S file/table/source and ledgers.
/// TODO: later P~/H~/qPCS consumers must consume this owner with new read admission;
/// no reset, extraction or production-proof authority is supplied here.
#[must_use = "retain original S and both exact opening owners for later relations"]
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct CompletedOriginalQMaskOpeningsV1
<R, K, P> {
    owner: StoredQMaskComplementV1<R, K, P>,
}
