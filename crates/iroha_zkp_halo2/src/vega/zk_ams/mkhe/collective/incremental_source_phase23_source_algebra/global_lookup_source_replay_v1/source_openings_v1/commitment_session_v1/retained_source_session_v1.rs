//! One original live session retained alongside authenticated source openings.
//!
//! Immutable source-prefix validation is independent of stage progress. This
//! closed owner exposes no session extraction, root rebinding or point adoption.
use super::*;
use super::existing_radix_candidate_v1::{RnsNativeComparatorTopCommitmentsV1, RnsNativeDifferenceCommitmentsV1, RnsNativeComparatorContinuationV1, RnsNativeSmallSignedCommitmentsV1, validate_existing_radix_candidate_ingress_v1};
use crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::PreparedLowDigitStatementV1;
use crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::PreparedComparatorStatementV1;

enum RetainedSourcePhaseV1<R> {
    SourceComplete(GlobalLookupCommitmentSessionV1<R, SourceOpeningCompleteStageV1>),
    ExistingLow(RnsNativeExistingRadixCandidateAssemblyV1<R>),
    ExistingLowComplete(RnsNativeExistingRadixCandidateOwnerV1<R>),
    ComparatorTop(RnsNativeComparatorTopCommitmentsV1<R>),
    Difference(RnsNativeDifferenceCommitmentsV1<R>),
    ComparatorContinuation(RnsNativeComparatorContinuationV1<R>),
    SmallSigned(RnsNativeSmallSignedCommitmentsV1<R>),
}

#[must_use = "dropping this owner closes the only original proof session and retained blindings"]
pub(in super::super) struct RetainedSourceSessionV1<R> {
    phase: Option<RetainedSourcePhaseV1<R>>,
}

impl<R: crate::vega::MaskedRelaxedRandomSourceV1> RetainedSourceSessionV1<R> {
    pub(in super::super) fn from_source_complete_v1(
        session: GlobalLookupCommitmentSessionV1<R, SourceOpeningCompleteStageV1>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        validate_existing_radix_candidate_ingress_v1(&session)?;
        Ok(Self {
            phase: Some(RetainedSourcePhaseV1::SourceComplete(session)),
        })
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
    ) -> Result<(), ZkAmsMkheErrorV1> {
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
        self.phase = Some(RetainedSourcePhaseV1::SmallSigned(
            owner.commit_prepared_v1(statement)?,
        ));
        Ok(())
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
    ) -> Result<(), ZkAmsMkheErrorV1> {
        // No old phase is restored after a validation, entropy, MSM or adoption failure.
        let phase = self
            .phase
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        self.phase = Some(match phase {
            RetainedSourcePhaseV1::ExistingLowComplete(owner) => {
                RetainedSourcePhaseV1::ComparatorTop(
                    RnsNativeComparatorTopCommitmentsV1::begin_v1(owner)?
                        .commit_prepared_v1(statement)?,
                )
            }
            RetainedSourcePhaseV1::ComparatorTop(owner) => {
                RetainedSourcePhaseV1::ComparatorTop(owner.commit_prepared_v1(statement)?)
            }
            RetainedSourcePhaseV1::Difference(owner) => {
                RetainedSourcePhaseV1::ComparatorContinuation(
                    RnsNativeComparatorContinuationV1::begin_v1(owner)?
                        .commit_prepared_v1(statement)?,
                )
            }
            RetainedSourcePhaseV1::ComparatorContinuation(owner) => {
                RetainedSourcePhaseV1::ComparatorContinuation(owner.commit_prepared_v1(statement)?)
            }
            _ => return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold),
        });
        Ok(())
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
            | RetainedSourcePhaseV1::SmallSigned(_) => {
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
