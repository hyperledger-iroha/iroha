//! Complete the original S stream and keep authenticated replay with its rhos.
use super::*;
use crate::vega::zk_ams::mkhe::{
    collective::incremental_source::incremental_source_phase23::source_algebra::CompleteQMaskSOpeningsV1,
    global_lookup_statement_v1::SealedQMaskSFileV1,
};

/// Only pre-operation Capacity returns the entire same valid original prefix.
#[must_use = "retry only this original prefix after actual competing resource release"]
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct QMaskSStreamRefusalV1
<R, K, P> {
    reason: QMaskSErrorV1,
    original: Option<StoredQMaskSStreamV1<R, K, P>>,
}
impl<R: crate::vega::MaskedRelaxedRandomSourceV1, K, P> QMaskSStreamRefusalV1<R, K, P> {
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn reason_v1(
        &self,
    ) -> QMaskSErrorV1 {
        self.reason
    }
    #[allow(
        clippy::result_large_err,
        reason = "capacity retains the original owner without allocating"
    )]
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn retry_v1(
        mut self,
    ) -> Result<SealedOriginalQMaskSV1<R, K, P>, Self> {
        let Some(original) = self.original.take() else {
            return Err(self);
        };
        original.complete_original_s_v1()
    }
}
impl<R: crate::vega::MaskedRelaxedRandomSourceV1, K, P> StoredQMaskSStreamV1<R, K, P> {
    #[allow(
        clippy::result_large_err,
        reason = "only pre-entropy capacity preserves the original prefix"
    )]
    fn continue_block_v1(mut self) -> Result<Self, QMaskSStreamRefusalV1<R, K, P>> {
        let admission = (|| {
            self.original
                .live
                .as_mut()
                .and_then(|x| x.live.source.evidence.as_mut())
                .ok_or(QMaskSErrorV1::Source)?
                .admit_next_q_mask_s_block_v1(&self.block, &self.file)
        })();
        let admission = match admission {
            Ok(x) => x,
            Err(reason) => {
                return Err(QMaskSStreamRefusalV1 {
                    reason,
                    original: if reason == QMaskSErrorV1::Capacity {
                        Some(self)
                    } else {
                        None
                    },
                });
            }
        };
        let Self {
            file,
            block,
            mut original,
        } = self;
        let result = (|| {
            let mut file = file
                .resume_next_block_v1()
                .map_err(QMaskSErrorV1::Storage)?;
            let block = original
                .live
                .as_mut()
                .and_then(|x| x.live.source.evidence.as_mut())
                .ok_or(QMaskSErrorV1::Source)?
                .continue_q_mask_s_block_v1(block, &mut file, admission)?;
            let file = file.finish_block_v1().map_err(QMaskSErrorV1::Storage)?;
            Ok((file, block))
        })();
        match result {
            Ok((file, block)) => Ok(Self {
                file,
                block,
                original,
            }),
            Err(reason) => Err(QMaskSStreamRefusalV1 {
                reason,
                original: None,
            }),
        }
    }
    /// The sole full-stream driver. It has no caller coordinate, bound, seed,
    /// source, rho, replacement file or alternative execution algorithm.
    #[allow(
        clippy::result_large_err,
        reason = "capacity retains the same already-completed prefix"
    )]
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn complete_original_s_v1(
        mut self,
    ) -> Result<SealedOriginalQMaskSV1<R, K, P>, QMaskSStreamRefusalV1<R, K, P>> {
        while !self.block.at_full_s_v1() {
            self = self.continue_block_v1()?;
        }
        let Self {
            file,
            block,
            mut original,
        } = self;
        let result = (|| {
            let openings = original
                .live
                .as_mut()
                .and_then(|x| x.live.source.evidence.as_mut())
                .ok_or(QMaskSErrorV1::Source)?
                .finish_q_mask_s_openings_v1(block)?;
            let file = file.seal_v1().map_err(QMaskSErrorV1::Storage)?;
            Ok((file, openings))
        })();
        match result {
            Ok((file, openings)) => Ok(SealedOriginalQMaskSV1 {
                file,
                openings,
                original,
            }),
            Err(reason) => Err(QMaskSStreamRefusalV1 {
                reason,
                original: None,
            }),
        }
    }
}

/// Full actual original S file, original 6,400 rhos and sole source inventory.
/// These fields grant no native40 source, polynomial, qPCS or composite seal.
#[must_use = "retain the original file and opening ownership for later exact consumers"]
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct SealedOriginalQMaskSV1
<R, K, P> {
    file: SealedQMaskSFileV1,
    openings: CompleteQMaskSOpeningsV1,
    original: PreparedStoredQMaskKernelV1<R, K, P>,
}
#[path = "stream_v1/complement_v1.rs"]
mod complement_v1;
