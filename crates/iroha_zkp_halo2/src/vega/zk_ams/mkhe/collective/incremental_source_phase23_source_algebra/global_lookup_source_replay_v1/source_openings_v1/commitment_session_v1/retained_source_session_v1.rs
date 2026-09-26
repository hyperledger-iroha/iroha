//! One original live session retained alongside authenticated source openings.
//!
//! Immutable source-prefix validation is independent of stage progress. This
//! closed owner exposes no session extraction, root rebinding or point adoption.
use super::*;
use crate::vega::zk_ams::mkhe::rns_native_u15_msm::{
    RnsNativeU15MsmErrorV1, RnsNativeU15MsmTableV1,
};

use super::existing_radix_candidate_v1::{RnsNativeComparatorTopCommitmentsV1, RnsNativeDifferenceCommitmentsV1, RnsNativeComparatorContinuationV1, RnsNativeSmallSignedCommitmentsV1, RnsNativeStoredPlaneReplayV1, validate_existing_radix_candidate_ingress_v1};
use crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::PreparedLowDigitStatementV1;
use crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::PreparedComparatorStatementV1;

struct PreparedQMaskKernelV1<R> {
    // Public table allocation releases before its retained original source.
    table: RnsNativeU15MsmTableV1,
    source: RnsNativeStoredPlaneReplayV1<R>,
}

enum RetainedSourcePhaseV1<R> {
    SourceComplete(GlobalLookupCommitmentSessionV1<R, SourceOpeningCompleteStageV1>),
    ExistingLow(RnsNativeExistingRadixCandidateAssemblyV1<R>),
    ExistingLowComplete(RnsNativeExistingRadixCandidateOwnerV1<R>),
    ComparatorTop(RnsNativeComparatorTopCommitmentsV1<R>),
    Difference(RnsNativeDifferenceCommitmentsV1<R>),
    ComparatorContinuation(RnsNativeComparatorContinuationV1<R>),
    SmallSigned(RnsNativeSmallSignedCommitmentsV1<R>),
    StoredPlaneReplay(RnsNativeStoredPlaneReplayV1<R>),
    QMaskKernel(PreparedQMaskKernelV1<R>),
    QMaskFirstSampled(PreparedQMaskKernelV1<R>),
    QMaskSStreaming(PreparedQMaskKernelV1<R>),
    QMaskSComplete(PreparedQMaskKernelV1<R>),
    QMaskComplementStreaming(PreparedQMaskKernelV1<R>),
    QMaskComplete(PreparedQMaskKernelV1<R>),
}

#[must_use = "dropping this owner closes the only original proof session and retained blindings"]
pub(in super::super) struct RetainedSourceSessionV1<R> {
    phase: Option<RetainedSourcePhaseV1<R>>,
}

impl<R: crate::vega::MaskedRelaxedRandomSourceV1> RetainedSourceSessionV1<R> {
    /// The qPCS continuation may charge the original session only after the
    /// complete Q-mask phase; the inventory and blindings remain owned here.
    pub(in super::super) fn original_budget_mut_v1(
        &mut self,
    ) -> Result<
        &mut crate::vega::zk_ams::mkhe::rns_native_resource_budget::RnsNativeProofResourceBudgetV1,
        ZkAmsMkheErrorV1,
    > {
        let Some(RetainedSourcePhaseV1::QMaskComplete(owner)) = self.phase.as_mut() else {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        };
        owner.source.original_budget_mut_v1()
    }

    pub(in super::super) fn from_source_complete_v1(
        session: GlobalLookupCommitmentSessionV1<R, SourceOpeningCompleteStageV1>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        validate_existing_radix_candidate_ingress_v1(&session)?;
        Ok(Self {
            phase: Some(RetainedSourcePhaseV1::SourceComplete(session)),
        })
    }

    /// Admit the named buffers on the original ledger without changing phase,
    /// source axes, inventory or either entropy counter on local refusal.
    pub(in super::super) fn admit_low_digit_workspace_v1(
        &mut self,
    ) -> Result<crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::LowDigitWorkspaceV1, crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::LowDigitWorkspaceErrorV1>{
        self.require_low_digit_start_v1()
            .map_err(|_| crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::LowDigitWorkspaceErrorV1::Source)?;
        let Some(RetainedSourcePhaseV1::SourceComplete(session)) = self.phase.as_mut() else {
            return Err(crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::LowDigitWorkspaceErrorV1::Source);
        };
        let live = session.live.as_mut().ok_or(crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::LowDigitWorkspaceErrorV1::Source)?;
        crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::LowDigitWorkspaceV1::admit_v1(&mut live.proof_resources)
    }

    pub(in super::super) fn require_low_digit_start_v1(&self) -> Result<(), ZkAmsMkheErrorV1> {
        match self
            .phase
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
        {
            RetainedSourcePhaseV1::SourceComplete(session) => {
                validate_existing_radix_candidate_ingress_v1(session)
            }
            _ => Err(ZkAmsMkheErrorV1::InvalidPhase23Fold),
        }
    }

    pub(in super::super) fn validate_completed_source_prefix_v1(
        &self,
        record: [u8; 32],
        context: [u8; 32],
        points: [u8; 32],
        blindings: [u8; 32],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        match self
            .phase
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
        {
            RetainedSourcePhaseV1::SourceComplete(session) => {
                session.validate_completed_source_prefix_v1(record, context, points, blindings)
            }
            RetainedSourcePhaseV1::ExistingLow(assembly) => {
                assembly.validate_completed_source_prefix_v1(record, context, points, blindings)
            }
            RetainedSourcePhaseV1::ExistingLowComplete(owner) => {
                owner.validate_completed_source_prefix_v1(record, context, points, blindings)
            }
            RetainedSourcePhaseV1::ComparatorTop(owner) => {
                owner.validate_completed_source_prefix_v1(record, context, points, blindings)
            }
            RetainedSourcePhaseV1::Difference(owner) => {
                owner.validate_completed_source_prefix_v1(record, context, points, blindings)
            }
            RetainedSourcePhaseV1::ComparatorContinuation(owner) => {
                owner.validate_completed_source_prefix_v1(record, context, points, blindings)
            }
            RetainedSourcePhaseV1::SmallSigned(owner) => {
                owner.validate_completed_source_prefix_v1(record, context, points, blindings)
            }
            RetainedSourcePhaseV1::StoredPlaneReplay(owner) => {
                owner.validate_completed_source_prefix_v1(record, context, points, blindings)
            }
            RetainedSourcePhaseV1::QMaskKernel(owner)
            | RetainedSourcePhaseV1::QMaskFirstSampled(owner)
            | RetainedSourcePhaseV1::QMaskSStreaming(owner)
            | RetainedSourcePhaseV1::QMaskSComplete(owner)
            | RetainedSourcePhaseV1::QMaskComplementStreaming(owner)
            | RetainedSourcePhaseV1::QMaskComplete(owner) => owner
                .source
                .validate_completed_source_prefix_v1(record, context, points, blindings),
        }
    }

    pub(in super::super) fn prepare_source_packing_openings_v1(
        &mut self,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let phase = self
            .phase
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let RetainedSourcePhaseV1::SmallSigned(owner) = phase else {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        };
        self.phase = Some(RetainedSourcePhaseV1::SmallSigned(
            owner.prepare_source_packing_openings_v1()?,
        ));
        Ok(())
    }

    pub(in super::super) fn admit_next_q_mask_s_block_v1(
        &mut self,
        stream: &QMaskSOpeningStreamV1,
        file: &crate::vega::zk_ams::mkhe::global_lookup_statement_v1::WrittenQMaskSBlockFileV1,
    ) -> Result<QMaskSBlockAdmissionV1, QMaskSErrorV1> {
        let Some(RetainedSourcePhaseV1::QMaskSStreaming(owner)) = self.phase.as_mut() else {
            return Err(QMaskSErrorV1::Source);
        };
        owner
            .source
            .admit_next_q_mask_s_block_v1(&owner.table, stream, file)
    }
    pub(in super::super) fn continue_q_mask_s_block_v1(
        &mut self,
        stream: QMaskSOpeningStreamV1,
        file: &mut crate::vega::zk_ams::mkhe::global_lookup_statement_v1::QMaskSFileV1,
        admission: QMaskSBlockAdmissionV1,
    ) -> Result<QMaskSOpeningStreamV1, QMaskSErrorV1> {
        let phase = self.phase.take().ok_or(QMaskSErrorV1::Source)?;
        let RetainedSourcePhaseV1::QMaskSStreaming(mut owner) = phase else {
            return Err(QMaskSErrorV1::Source);
        };
        let stream =
            owner
                .source
                .continue_q_mask_s_block_v1(&mut owner.table, stream, file, admission)?;
        self.phase = Some(RetainedSourcePhaseV1::QMaskSStreaming(owner));
        Ok(stream)
    }
    pub(in super::super) fn finish_q_mask_s_openings_v1(
        &mut self,
        stream: QMaskSOpeningStreamV1,
    ) -> Result<CompleteQMaskSOpeningsV1, QMaskSErrorV1> {
        let phase = self.phase.take().ok_or(QMaskSErrorV1::Source)?;
        let RetainedSourcePhaseV1::QMaskSStreaming(mut owner) = phase else {
            return Err(QMaskSErrorV1::Source);
        };
        let complete = owner
            .source
            .finish_q_mask_s_openings_v1(&owner.table, stream)?;
        self.phase = Some(RetainedSourcePhaseV1::QMaskSComplete(owner));
        Ok(complete)
    }
    pub(in super::super) fn begin_q_mask_complements_v1(
        &mut self,
        source: &CompleteQMaskSOpeningsV1,
        file: &crate::vega::zk_ams::mkhe::global_lookup_statement_v1::SealedQMaskSFileV1,
    ) -> Result<QMaskComplementOpeningsV1, QMaskSErrorV1> {
        let phase = self.phase.take().ok_or(QMaskSErrorV1::Source)?;
        let RetainedSourcePhaseV1::QMaskSComplete(mut owner) = phase else {
            return Err(QMaskSErrorV1::Source);
        };
        match owner
            .source
            .begin_q_mask_complements_v1(&owner.table, source, file)
        {
            Ok(complements) => {
                self.phase = Some(RetainedSourcePhaseV1::QMaskComplementStreaming(owner));
                Ok(complements)
            }
            Err(QMaskSErrorV1::Capacity) => {
                self.phase = Some(RetainedSourcePhaseV1::QMaskSComplete(owner));
                Err(QMaskSErrorV1::Capacity)
            }
            Err(error) => Err(error),
        }
    }
    pub(in super::super) fn produce_q_mask_complement_block_v1(
        &mut self,
        source: &mut CompleteQMaskSOpeningsV1,
        file: &mut crate::vega::zk_ams::mkhe::global_lookup_statement_v1::SealedQMaskSFileV1,
        complements: &mut QMaskComplementOpeningsV1,
    ) -> Result<(), QMaskSErrorV1> {
        let phase = self.phase.take().ok_or(QMaskSErrorV1::Source)?;
        let RetainedSourcePhaseV1::QMaskComplementStreaming(mut owner) = phase else {
            return Err(QMaskSErrorV1::Source);
        };
        let result = owner.source.produce_q_mask_complement_block_v1(
            &mut owner.table,
            source,
            file,
            complements,
        );
        if result.is_ok() || result == Err(QMaskSErrorV1::Capacity) {
            self.phase = Some(RetainedSourcePhaseV1::QMaskComplementStreaming(owner));
        }
        result
    }
    pub(in super::super) fn finish_q_mask_complements_v1(
        &mut self,
        source: &CompleteQMaskSOpeningsV1,
        file: &crate::vega::zk_ams::mkhe::global_lookup_statement_v1::SealedQMaskSFileV1,
        complements: &QMaskComplementOpeningsV1,
    ) -> Result<(), QMaskSErrorV1> {
        let phase = self.phase.take().ok_or(QMaskSErrorV1::Source)?;
        let RetainedSourcePhaseV1::QMaskComplementStreaming(mut owner) = phase else {
            return Err(QMaskSErrorV1::Source);
        };
        owner
            .source
            .finish_q_mask_complements_v1(&owner.table, source, file, complements)?;
        self.phase = Some(RetainedSourcePhaseV1::QMaskComplete(owner));
        Ok(())
    }
    pub(in super::super) fn admit_first_q_mask_openings_v1(
        &mut self,
        block: &SampledQMaskSBlockV1,
        file: &crate::vega::zk_ams::mkhe::global_lookup_statement_v1::WrittenQMaskSBlockFileV1,
    ) -> Result<QMaskSBlockAdmissionV1, QMaskSErrorV1> {
        let Some(RetainedSourcePhaseV1::QMaskFirstSampled(owner)) = self.phase.as_mut() else {
            return Err(QMaskSErrorV1::Source);
        };
        owner
            .source
            .admit_first_q_mask_openings_v1(&owner.table, block, file)
    }
    pub(in super::super) fn produce_first_q_mask_openings_v1(
        &mut self,
        block: SampledQMaskSBlockV1,
        admission: QMaskSBlockAdmissionV1,
    ) -> Result<QMaskSOpeningStreamV1, QMaskSErrorV1> {
        let phase = self.phase.take().ok_or(QMaskSErrorV1::Source)?;
        let RetainedSourcePhaseV1::QMaskFirstSampled(mut owner) = phase else {
            return Err(QMaskSErrorV1::Source);
        };
        let opened =
            owner
                .source
                .produce_first_q_mask_openings_v1(&mut owner.table, block, admission)?;
        self.phase = Some(RetainedSourcePhaseV1::QMaskSStreaming(owner));
        Ok(opened)
    }

    pub(in super::super) fn reserve_q_mask_first_memory_v1(
        &mut self,
        plan: &crate::vega::zk_ams::mkhe::global_lookup_statement_v1::QMaskSFilePlanV1,
    ) -> Result<
        (
            QMaskFirstBlockMemoryV1,
            crate::vega::zk_ams::mkhe::global_lookup_statement_v1::QMaskSFileMemoryV1,
        ),
        QMaskSErrorV1,
    > {
        let Some(RetainedSourcePhaseV1::QMaskKernel(owner)) = self.phase.as_mut() else {
            return Err(QMaskSErrorV1::Source);
        };
        owner.source.reserve_q_mask_first_memory_v1(plan)
    }
    pub(in super::super) fn sample_q_mask_first_block_v1(
        &mut self,
        memory: QMaskFirstBlockMemoryV1,
    ) -> Result<SampledQMaskSBlockV1, QMaskSErrorV1> {
        let phase = self.phase.take().ok_or(QMaskSErrorV1::Source)?;
        let RetainedSourcePhaseV1::QMaskKernel(mut owner) = phase else {
            return Err(QMaskSErrorV1::Source);
        };
        let block = owner.source.sample_q_mask_first_block_v1(memory)?;
        self.phase = Some(RetainedSourcePhaseV1::QMaskFirstSampled(owner));
        Ok(block)
    }

    pub(in super::super) fn begin_q_mask_kernel_v1(
        &mut self,
    ) -> Result<(), RnsNativeU15MsmErrorV1> {
        let phase = self.phase.take().ok_or(RnsNativeU15MsmErrorV1::Source)?;
        let RetainedSourcePhaseV1::StoredPlaneReplay(mut source) = phase else {
            return Err(RnsNativeU15MsmErrorV1::Source);
        };
        match source.admit_u15_table_v1() {
            Ok(table) => {
                self.phase = Some(RetainedSourcePhaseV1::QMaskKernel(PreparedQMaskKernelV1 {
                    table,
                    source,
                }));
                Ok(())
            }
            Err(RnsNativeU15MsmErrorV1::Capacity) => {
                // No table allocation or generator/secret read has occurred.
                self.phase = Some(RetainedSourcePhaseV1::StoredPlaneReplay(source));
                Err(RnsNativeU15MsmErrorV1::Capacity)
            }
            Err(error) => Err(error),
        }
    }

    pub(in super::super) fn begin_stored_plane_replay_v1(
        &mut self,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let phase = self
            .phase
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let RetainedSourcePhaseV1::SmallSigned(owner) = phase else {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        };
        self.phase = Some(RetainedSourcePhaseV1::StoredPlaneReplay(
            owner.into_stored_plane_replay_v1()?,
        ));
        Ok(())
    }

    pub(in super::super) fn validate_stored_plane_tail_v1(
        &self,
        ordinal: u16,
        bytes: &[u8],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        match self
            .phase
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
        {
            RetainedSourcePhaseV1::StoredPlaneReplay(owner) => {
                owner.validate_stored_plane_tail_v1(ordinal, bytes)
            }
            _ => Err(ZkAmsMkheErrorV1::InvalidPhase23Fold),
        }
    }

    pub(in super::super) fn require_small_signed_position_v1(
        &self,
        ordinal: u16,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        match self
            .phase
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
        {
            RetainedSourcePhaseV1::ComparatorContinuation(owner) => {
                RnsNativeSmallSignedCommitmentsV1::validate_start_v1(owner, ordinal)
            }
            RetainedSourcePhaseV1::SmallSigned(owner) => owner.require_position_v1(ordinal),
            _ => Err(ZkAmsMkheErrorV1::InvalidPhase23Fold),
        }
    }

    pub(in super::super) fn commit_prepared_small_signed_v1(
        &mut self,
        statement: &crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::PreparedSmallSignedStatementV1<'_>,
    ) -> Result<PreparedPlaneOpeningTailV1, ZkAmsMkheErrorV1> {
        // Remove the only original session before every fallible transition.
        // Neither earlier openings nor a replacement phase escapes on failure.
        let phase = self
            .phase
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let owner = match phase {
            RetainedSourcePhaseV1::ComparatorContinuation(owner) => {
                RnsNativeSmallSignedCommitmentsV1::begin_v1(owner)?
            }
            RetainedSourcePhaseV1::SmallSigned(owner) => owner,
            _ => return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold),
        };
        let (owner, tail) = owner.commit_prepared_v1(statement)?;
        self.phase = Some(RetainedSourcePhaseV1::SmallSigned(owner));
        Ok(tail)
    }

    pub(in super::super) fn require_comparator_position_v1(
        &self,
        ordinal: u16,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        match self
            .phase
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
        {
            RetainedSourcePhaseV1::ExistingLowComplete(owner) => {
                RnsNativeComparatorTopCommitmentsV1::validate_start_v1(owner, ordinal)
            }
            RetainedSourcePhaseV1::ComparatorTop(owner) => owner.require_position_v1(ordinal),
            RetainedSourcePhaseV1::Difference(owner) => {
                RnsNativeComparatorContinuationV1::validate_start_v1(owner, ordinal)
            }
            RetainedSourcePhaseV1::ComparatorContinuation(owner) => {
                owner.require_position_v1(ordinal)
            }
            _ => Err(ZkAmsMkheErrorV1::InvalidPhase23Fold),
        }
    }

    pub(in super::super) fn commit_prepared_comparator_v1(
        &mut self,
        statement: &PreparedComparatorStatementV1<'_>,
    ) -> Result<PreparedPlaneOpeningTailV1, ZkAmsMkheErrorV1> {
        // No old phase is restored after a validation, entropy, MSM or adoption failure.
        let phase = self
            .phase
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let (phase, tail) = match phase {
            RetainedSourcePhaseV1::ExistingLowComplete(owner) => {
                let (owner, tail) = RnsNativeComparatorTopCommitmentsV1::begin_v1(owner)?
                    .commit_prepared_v1(statement)?;
                (RetainedSourcePhaseV1::ComparatorTop(owner), tail)
            }
            RetainedSourcePhaseV1::ComparatorTop(owner) => {
                let (owner, tail) = owner.commit_prepared_v1(statement)?;
                (RetainedSourcePhaseV1::ComparatorTop(owner), tail)
            }
            RetainedSourcePhaseV1::Difference(owner) => {
                let (owner, tail) = RnsNativeComparatorContinuationV1::begin_v1(owner)?
                    .commit_prepared_v1(statement)?;
                (RetainedSourcePhaseV1::ComparatorContinuation(owner), tail)
            }
            RetainedSourcePhaseV1::ComparatorContinuation(owner) => {
                let (owner, tail) = owner.commit_prepared_v1(statement)?;
                (RetainedSourcePhaseV1::ComparatorContinuation(owner), tail)
            }
            _ => return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold),
        };
        self.phase = Some(phase);
        Ok(tail)
    }

    pub(in super::super) fn require_difference_start_v1(&self) -> Result<(), ZkAmsMkheErrorV1> {
        match self
            .phase
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
        {
            RetainedSourcePhaseV1::ComparatorTop(owner) => {
                RnsNativeDifferenceCommitmentsV1::validate_start_v1(owner)
            }
            _ => Err(ZkAmsMkheErrorV1::InvalidPhase23Fold),
        }
    }

    pub(in super::super) fn require_difference_complete_v1(&self) -> Result<(), ZkAmsMkheErrorV1> {
        match self
            .phase
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
        {
            RetainedSourcePhaseV1::Difference(owner) => owner.require_complete_v1(),
            _ => Err(ZkAmsMkheErrorV1::InvalidPhase23Fold),
        }
    }

    pub(in super::super) fn commit_prepared_difference_digit_v1(
        &mut self,
        statement: &crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::PreparedDifferenceDigitStatementV1<'_>,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        // A failed stage, ordinal, entropy, MSM or adoption check consumes the
        // only original session; the old phase is never restored.
        let phase = self
            .phase
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let owner = match phase {
            RetainedSourcePhaseV1::ComparatorTop(owner) => {
                RnsNativeDifferenceCommitmentsV1::begin_v1(owner)?
            }
            RetainedSourcePhaseV1::Difference(owner) => owner,
            _ => return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold),
        };
        self.phase = Some(RetainedSourcePhaseV1::Difference(
            owner.commit_prepared_v1(statement)?,
        ));
        Ok(())
    }

    pub(in super::super) fn commit_prepared_low_digit_v1(
        &mut self,
        statement: &PreparedLowDigitStatementV1<'_>,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        // Remove the sole original session before order checks, allocation,
        // entropy, secret MSM or adoption. Error/unwind never restores it.
        let phase = self
            .phase
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let assembly = match phase {
            RetainedSourcePhaseV1::SourceComplete(session) => {
                session.into_existing_radix_candidate_assembly_v1()?
            }
            RetainedSourcePhaseV1::ExistingLow(assembly) => assembly,
            RetainedSourcePhaseV1::ExistingLowComplete(_)
            | RetainedSourcePhaseV1::ComparatorTop(_)
            | RetainedSourcePhaseV1::Difference(_)
            | RetainedSourcePhaseV1::ComparatorContinuation(_)
            | RetainedSourcePhaseV1::SmallSigned(_)
            | RetainedSourcePhaseV1::StoredPlaneReplay(_)
            | RetainedSourcePhaseV1::QMaskKernel(_)
            | RetainedSourcePhaseV1::QMaskFirstSampled(_)
            | RetainedSourcePhaseV1::QMaskSStreaming(_)
            | RetainedSourcePhaseV1::QMaskSComplete(_)
            | RetainedSourcePhaseV1::QMaskComplementStreaming(_)
            | RetainedSourcePhaseV1::QMaskComplete(_) => {
                return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
            }
        };
        let assembly = assembly.commit_prepared_values_v1(statement)?;
        self.phase = Some(if assembly.all_prepared_values_committed_v1()? {
            RetainedSourcePhaseV1::ExistingLowComplete(assembly.finish_v1()?)
        } else {
            RetainedSourcePhaseV1::ExistingLow(assembly)
        });
        Ok(())
    }
}

#[cfg(test)]
#[path = "retained_source_session_v1_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "retained_source_u15_kernel_v1_tests.rs"]
mod u15_tests;
