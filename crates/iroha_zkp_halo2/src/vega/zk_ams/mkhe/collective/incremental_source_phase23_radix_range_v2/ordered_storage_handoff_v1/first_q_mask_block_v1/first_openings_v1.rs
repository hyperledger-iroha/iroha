//! Consume the original first block/file/source into its four actual S openings.
use super::*;
use crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::source_algebra::QMaskSOpeningStreamV1;

/// Original S stream after a complete block and its four rho/point tickets.
/// Its consuming child advances the same source and file; complement, qPCS and
/// provider authority remain unavailable.
#[must_use = "retain the sole S preimage, original rho scalars, file and source together"]
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct StoredQMaskSStreamV1
<R, K, P> {
    file: WrittenQMaskSBlockFileV1,
    block: QMaskSOpeningStreamV1,
    original: PreparedStoredQMaskKernelV1<R, K, P>,
}

#[must_use = "only pre-rho local capacity preserves the exact original sampled owner"]
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct FirstQMaskOpeningsRefusalV1
<R, K, P> {
    reason: QMaskSErrorV1,
    original: Option<FirstStoredQMaskBlockV1<R, K, P>>,
}
impl<R: crate::vega::MaskedRelaxedRandomSourceV1, K, P> FirstQMaskOpeningsRefusalV1<R, K, P> {
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn reason_v1(
        &self,
    ) -> QMaskSErrorV1 {
        self.reason
    }
    #[allow(
        clippy::result_large_err,
        reason = "pre-rho refusal retains the whole original source without a new allocation"
    )]
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn retry_v1(
        mut self,
    ) -> Result<StoredQMaskSStreamV1<R, K, P>, Self> {
        let Some(original) = self.original.take() else {
            return Err(self);
        };
        original.commit_first_s_openings_v1()
    }
}

impl<R: crate::vega::MaskedRelaxedRandomSourceV1, K, P> FirstStoredQMaskBlockV1<R, K, P> {
    #[allow(
        clippy::result_large_err,
        reason = "pre-rho refusal retains the whole original source without a new allocation"
    )]
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn commit_first_s_openings_v1(
        mut self,
    ) -> Result<StoredQMaskSStreamV1<R, K, P>, FirstQMaskOpeningsRefusalV1<R, K, P>> {
        let admission = (|| {
            let original = self.original.live.as_mut().ok_or(QMaskSErrorV1::Source)?;
            if original.live.next_slot != SLOTS_V1
                || original
                    .live
                    .snapshot
                    .snapshot_digest_v1()
                    .map_err(QMaskSErrorV1::Storage)?
                    != original.live.identity
            {
                return Err(QMaskSErrorV1::Source);
            }
            original
                .live
                .source
                .evidence
                .as_mut()
                .ok_or(QMaskSErrorV1::Source)?
                .admit_first_q_mask_openings_v1(&self.block, &self.file)
        })();
        let admission = match admission {
            Ok(admission) => admission,
            Err(reason) => {
                return Err(FirstQMaskOpeningsRefusalV1 {
                    reason,
                    original: if reason == QMaskSErrorV1::Capacity {
                        Some(self)
                    } else {
                        None
                    },
                });
            }
        };
        // From this point no failure returns a usable owner. In particular,
        // failure after one successful rho/ticket must not rerun that draw.
        let Self {
            file,
            block,
            mut original,
        } = self;
        let result = original
            .live
            .as_mut()
            .and_then(|owner| owner.live.source.evidence.as_mut())
            .ok_or(QMaskSErrorV1::Source)
            .and_then(|evidence| evidence.produce_first_q_mask_openings_v1(block, admission));
        match result {
            Ok(block) => Ok(StoredQMaskSStreamV1 {
                file,
                block,
                original,
            }),
            Err(reason) => Err(FirstQMaskOpeningsRefusalV1 {
                reason,
                original: None,
            }),
        }
    }
}

#[path = "first_openings_v1/stream_v1.rs"]
mod stream_v1;
