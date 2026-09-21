//! Original sampled S block, sibling file and retained source consumed together.
use super::*;
use crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::source_algebra::{
    QMaskSErrorV1, SampledQMaskSBlockV1,
};
use crate::vega::zk_ams::mkhe::global_lookup_statement_v1::WrittenQMaskSBlockFileV1;

/// Original sampled first block. Only the consuming four-opening child may
/// advance it; no caller S/rho, next block, file seal or qPCS provider is exposed.
#[must_use = "retain the exact original S and file until its four actual openings consume them"]
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct FirstStoredQMaskBlockV1
<R, K, P> {
    // Destruction order releases actual file and sampled allocation before the
    // named memory credit and the original source/table/session.
    file: WrittenQMaskSBlockFileV1,
    block: SampledQMaskSBlockV1,
    original: PreparedStoredQMaskKernelV1<R, K, P>,
}

#[must_use = "only pre-I/O local capacity retains a retryable original owner"]
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct QMaskFirstBlockStartRefusalV1
<R, K, P> {
    reason: QMaskSErrorV1,
    original: Option<PreparedStoredQMaskKernelV1<R, K, P>>,
}
impl<R: crate::vega::MaskedRelaxedRandomSourceV1, K, P> QMaskFirstBlockStartRefusalV1<R, K, P> {
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn reason_v1(
        &self,
    ) -> QMaskSErrorV1 {
        self.reason
    }
    #[allow(
        clippy::result_large_err,
        reason = "pre-I/O refusal retains the unchanged original owner without replacement allocation"
    )]
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn retry_v1(
        mut self,
        directory: &Path,
    ) -> Result<FirstStoredQMaskBlockV1<R, K, P>, Self> {
        let Some(original) = self.original.take() else {
            return Err(self);
        };
        original.sample_first_q_mask_block_v1(directory)
    }
}

impl<R: crate::vega::MaskedRelaxedRandomSourceV1, K, P> PreparedStoredQMaskKernelV1<R, K, P> {
    #[allow(
        clippy::result_large_err,
        reason = "pre-I/O refusal retains the unchanged original owner without replacement allocation"
    )]
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn sample_first_q_mask_block_v1(
        mut self,
        directory: &Path,
    ) -> Result<FirstStoredQMaskBlockV1<R, K, P>, QMaskFirstBlockStartRefusalV1<R, K, P>> {
        let result = (|| {
            let original = self.live.as_mut().ok_or(QMaskSErrorV1::Source)?;
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
            let plan = original
                .live
                .snapshot
                .q_mask_s_file_plan_v1()
                .map_err(QMaskSErrorV1::Storage)?;
            let evidence = original
                .live
                .source
                .evidence
                .as_mut()
                .ok_or(QMaskSErrorV1::Source)?;
            // Both memory and whole-file/I/O reservations precede creation,
            // block allocation and every original proof-RNG read.
            let (memory, file_memory) = evidence.reserve_q_mask_first_memory_v1(&plan)?;
            let mut file = original
                .live
                .snapshot
                .create_q_mask_s_file_v1(directory, plan, file_memory)
                .map_err(|error| match error {
                    OrderedSnapshotErrorV1::Capacity => QMaskSErrorV1::Capacity,
                    other => QMaskSErrorV1::Storage(other),
                })?;
            let block = evidence.sample_q_mask_first_block_v1(memory)?;
            block.write_slots_v1(&mut file)?;
            let file = file.finish_block_v1().map_err(QMaskSErrorV1::Storage)?;
            Ok((file, block))
        })();
        match result {
            Ok((file, block)) => Ok(FirstStoredQMaskBlockV1 {
                file,
                block,
                original: self,
            }),
            Err(reason) => Err(QMaskFirstBlockStartRefusalV1 {
                reason,
                original: if reason == QMaskSErrorV1::Capacity {
                    Some(self)
                } else {
                    None
                },
            }),
        }
    }
}

#[path = "first_q_mask_block_v1/first_openings_v1.rs"]
mod first_openings_v1;
