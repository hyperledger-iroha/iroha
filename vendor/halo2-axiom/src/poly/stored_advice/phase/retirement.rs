//! Closed post-y advice handoff that moves original blinds onto authenticated coefficients.
//!
//! Capacity preparation carries no witness authority. The consuming move validates the original
//! owner, retires Lagrange snapshots, and preserves its actual plan/challenges/context/cursor.
#[path = "retirement/blind.rs"]
mod blind;
#[path = "retirement/quotient.rs"]
mod quotient;

use super::super::{SecretBlind, StoredColumn, StoredPhaseErrorV1, StoredPhasePlanV1, reserved};
use super::{CoefficientColumnV1, CoefficientStoredAdviceV1};
use crate::{
    arithmetic::CurveAffine,
    poly::{
        commitment::Params,
        ipa::commitment::ParamsIPA,
        stored_advice::{
            STORED_MAX_K_V1, StoredPolynomialBasisV1, StoredPolynomialErrorV1,
            StoredPolynomialLayoutV1, StoredPolynomialRoleV1, StoredPolynomialSnapshotV1,
            assignment::StoredAssignmentFieldV1,
        },
    },
};

struct RetainedColumn<C: CurveAffine, S>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    original: StoredPolynomialLayoutV1,
    coefficient: CoefficientColumnV1<S>,
    blind: SecretBlind<C::Scalar>,
}
/// Reserved empty metadata, never authority to substitute a witness or proof owner.
pub(crate) struct CoefficientHandoffAllocationV1<'params, C: CurveAffine, S>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    params: &'params ParamsIPA<C>,
    k: u32,
    count: usize,
    context: Option<[u8; 32]>,
    columns: Vec<RetainedColumn<C, S>>,
}
impl<C: CurveAffine, S> CoefficientHandoffAllocationV1<'_, C, S>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    /// Logical minimum metadata payload before capacity allocation.
    pub(crate) fn minimum_payload(columns: usize) -> Result<usize, StoredPhaseErrorV1> {
        columns
            .checked_mul(std::mem::size_of::<RetainedColumn<C, S>>())
            .and_then(|v| v.checked_add(std::mem::size_of::<Self>()))
            .ok_or(StoredPhaseErrorV1::Admission)
    }
    /// Actual reserved metadata payload, excluding the inherited owner and allocator overhead.
    pub(crate) fn payload_bytes(&self) -> Result<usize, StoredPhaseErrorV1> {
        self.columns
            .capacity()
            .checked_mul(std::mem::size_of::<RetainedColumn<C, S>>())
            .and_then(|v| v.checked_add(std::mem::size_of::<Self>()))
            .ok_or(StoredPhaseErrorV1::Admission)
    }
}
/// Original coefficient snapshots with their sole advice blinds after Lagrange retirement.
pub(crate) struct CoefficientOnlyStoredAdviceV1<'params, C: CurveAffine, S>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    plan: StoredPhasePlanV1<'params, C>,
    challenges: Vec<Option<C::Scalar>>,
    columns: Vec<RetainedColumn<C, S>>,
    source_context: Option<[u8; 32]>,
    source_greatest: Option<u64>,
    proof_context: Option<[u8; 32]>,
    greatest_ordinal: Option<u64>,
}
impl<'params, C, S> CoefficientStoredAdviceV1<'params, C, S>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    S: StoredPolynomialSnapshotV1,
{
    /// Immutable already-staged coefficient identities; no source access or blind escapes.
    pub(crate) fn coefficient_layouts(
        &self,
    ) -> Result<impl ExactSizeIterator<Item = StoredPolynomialLayoutV1> + '_, StoredPhaseErrorV1>
    {
        self.validate_live_receipts()?;
        Ok(self.coefficients.iter().map(|v| v.layout))
    }
    /// Prepare empty output metadata before proof randomness; no secret or blind is copied.
    pub(crate) fn prepare_coefficient_only_handoff(
        &self,
    ) -> Result<CoefficientHandoffAllocationV1<'params, C, S>, StoredPhaseErrorV1> {
        self.validate_live_receipts()?;
        let session = self
            .lagrange
            .session
            .as_ref()
            .ok_or(StoredPhaseErrorV1::Poisoned)?;
        Ok(CoefficientHandoffAllocationV1 {
            params: session.plan.params,
            k: session.plan.k,
            count: session.plan.columns,
            context: self.proof_context,
            columns: reserved(session.plan.columns)?,
        })
    }
    /// Move every original blind exactly once; errors/unwind consume old and partial owners.
    /// The outer prover must sweep its other receipts after this method's destructor boundaries.
    pub(crate) fn into_coefficient_only(
        mut self,
        allocation: CoefficientHandoffAllocationV1<'params, C, S>,
    ) -> Result<CoefficientOnlyStoredAdviceV1<'params, C, S>, StoredPhaseErrorV1> {
        self.validate_live_receipts()?;
        let session = self
            .lagrange
            .session
            .take()
            .ok_or(StoredPhaseErrorV1::Poisoned)?;
        if !std::ptr::eq(allocation.params, session.plan.params)
            || allocation.k != session.plan.k
            || allocation.count != session.plan.columns
            || !allocation.columns.is_empty()
            || allocation.columns.capacity() < allocation.count
            || (allocation.context != self.proof_context
                && !(allocation.context.is_none() && session.plan.columns == 0))
        {
            return Err(StoredPhaseErrorV1::Admission);
        }
        let mut sources = session.columns.into_iter();
        let mut coefficients = self.coefficients.into_iter();
        let mut retained = CoefficientOnlyStoredAdviceV1 {
            plan: session.plan,
            challenges: session.challenges,
            columns: allocation.columns,
            source_context: session.proof_context,
            source_greatest: session.greatest_ordinal,
            proof_context: self.proof_context,
            greatest_ordinal: self.greatest_ordinal,
        };
        while let Some(source) = sources.next() {
            // Remaining originals, already-moved coefficients and every untouched coefficient
            // surround each externally implemented snapshot destructor.
            check_partial(&sources, &coefficients, &retained.columns)?;
            let coefficient = coefficients.next().ok_or(StoredPhaseErrorV1::Admission)?;
            if source.snapshot.layout() != source.layout
                || coefficient.snapshot.layout() != coefficient.layout
            {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
            let StoredColumn {
                layout,
                snapshot,
                blind,
            } = source;
            retained.columns.push(RetainedColumn {
                original: layout,
                coefficient,
                blind,
            });
            drop(snapshot);
            check_partial(&sources, &coefficients, &retained.columns)?;
        }
        if coefficients.len() != 0 {
            return Err(StoredPhaseErrorV1::Admission);
        }
        retained.validate_live_receipts()?;
        Ok(retained)
    }
}
fn check_partial<C: CurveAffine, S: StoredPolynomialSnapshotV1>(
    sources: &std::vec::IntoIter<StoredColumn<C, S>>,
    coefficients: &std::vec::IntoIter<CoefficientColumnV1<S>>,
    retained: &[RetainedColumn<C, S>],
) -> Result<(), StoredPhaseErrorV1>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    if sources
        .as_slice()
        .iter()
        .any(|s| s.snapshot.layout() != s.layout)
        || coefficients
            .as_slice()
            .iter()
            .any(|s| s.snapshot.layout() != s.layout)
        || retained
            .iter()
            .any(|s| s.coefficient.snapshot.layout() != s.coefficient.layout)
    {
        return Err(StoredPolynomialErrorV1::Context.into());
    }
    Ok(())
}
impl<'params, C, S> CoefficientOnlyStoredAdviceV1<'params, C, S>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    S: StoredPolynomialSnapshotV1,
{
    /// Original parameter identity, never a substituted same-k parameter reference.
    pub(crate) fn params(&self) -> Result<&'params ParamsIPA<C>, StoredPhaseErrorV1> {
        self.validate_live_receipts()?;
        Ok(self.plan.params)
    }
    /// Immutable retained coefficient labels in original advice-column order.
    pub(crate) fn layouts(
        &self,
    ) -> Result<impl ExactSizeIterator<Item = StoredPolynomialLayoutV1> + '_, StoredPhaseErrorV1>
    {
        self.validate_live_receipts()?;
        Ok(self.columns.iter().map(|v| v.coefficient.layout))
    }
    /// Original phase challenges in global challenge order.
    pub(crate) fn challenges(
        &self,
    ) -> Result<impl Iterator<Item = C::Scalar> + '_, StoredPhaseErrorV1> {
        self.validate_live_receipts()?;
        Ok(self.challenges.iter().filter_map(|v| *v))
    }
    /// Actual context established by original provider output.
    pub(crate) fn proof_context(&self) -> Result<Option<[u8; 32]>, StoredPhaseErrorV1> {
        self.validate_live_receipts()?;
        Ok(self.proof_context)
    }
    /// Original global output high-water mark after all prior argument allocations.
    pub(crate) fn greatest_ordinal(&self) -> Result<Option<u64>, StoredPhaseErrorV1> {
        self.validate_live_receipts()?;
        Ok(self.greatest_ordinal)
    }
    /// Check original phase provenance and every remaining authenticated coefficient receipt.
    pub(crate) fn validate_live_receipts(&self) -> Result<(), StoredPhaseErrorV1> {
        let plan = &self.plan;
        if plan.k > STORED_MAX_K_V1
            || plan.params.k() != plan.k
            || plan.params.n() != 1_u64 << plan.k
            || plan.params.get_g_lagrange().len() != 1_usize << plan.k
            || plan.field != C::Scalar::STORED_FIELD
            || plan.usable_rows > 1_usize << plan.k
            || self.columns.len() != plan.columns
            || plan.phases.is_empty()
            || plan.phases.len() > 3
            || self.challenges.len() != plan.challenges
            || self.challenges.iter().any(Option::is_none)
            || (plan.columns == 0 && (plan.phases.len() != 1 || plan.challenges != 0))
        {
            return Err(StoredPhaseErrorV1::Admission);
        }
        let mut scheduled_columns = 0_usize;
        let mut scheduled_challenges = 0_usize;
        let mut greatest_source = None;
        for (phase_index, phase) in plan.phases.iter().enumerate() {
            if (phase.columns.is_empty() && (plan.columns != 0 || !phase.challenges.is_empty()))
                || phase.columns.windows(2).any(|p| p[0] >= p[1])
                || phase.challenges.windows(2).any(|p| p[0] >= p[1])
            {
                return Err(StoredPhaseErrorV1::Admission);
            }
            scheduled_columns = scheduled_columns
                .checked_add(phase.columns.len())
                .ok_or(StoredPhaseErrorV1::Admission)?;
            scheduled_challenges = scheduled_challenges
                .checked_add(phase.challenges.len())
                .ok_or(StoredPhaseErrorV1::Admission)?;
            for index in &phase.challenges {
                if *index >= plan.challenges
                    || plan.phases[..phase_index]
                        .iter()
                        .any(|earlier| earlier.challenges.binary_search(index).is_ok())
                {
                    return Err(StoredPhaseErrorV1::Admission);
                }
            }
            for index in &phase.columns {
                let column = self
                    .columns
                    .get(*index)
                    .ok_or(StoredPhaseErrorV1::Admission)?;
                if column.original.role()
                    != (StoredPolynomialRoleV1::Advice {
                        column: u32::try_from(*index).map_err(|_| StoredPhaseErrorV1::Admission)?,
                        phase: phase_index as u8,
                    })
                    || greatest_source.is_some_and(|old| column.original.ordinal() <= old)
                {
                    return Err(StoredPhaseErrorV1::Admission);
                }
                greatest_source = Some(column.original.ordinal());
            }
        }
        if scheduled_columns != plan.columns
            || scheduled_challenges != plan.challenges
            || greatest_source != self.source_greatest
        {
            return Err(StoredPhaseErrorV1::Admission);
        }
        let mut greatest = greatest_source;
        for (index, column) in self.columns.iter().enumerate() {
            let original = column.original;
            let copied = column.coefficient.layout;
            if original.advice_coordinates()?.0 as usize != index
                || original.field() != plan.field
                || original.k() != plan.k
                || original.basis() != StoredPolynomialBasisV1::Lagrange
                || original.proof_context == [0; 32]
                || self.source_context != Some(original.proof_context)
                || self.proof_context != self.source_context
                || copied.field() != original.field()
                || copied.k() != original.k()
                || copied.role() != original.role()
                || copied.basis() != StoredPolynomialBasisV1::Coefficient
                || !copied.same_proof_context(original)
                || greatest.is_some_and(|old| copied.ordinal() <= old)
            {
                return Err(StoredPhaseErrorV1::Admission);
            }
            if column.coefficient.snapshot.layout() != copied {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
            greatest = Some(copied.ordinal());
        }
        if (self.columns.is_empty()
            && (self.source_context.is_some() || self.source_greatest.is_some()))
            || self.proof_context == Some([0; 32])
            || self.proof_context.is_some() != self.greatest_ordinal.is_some()
            || greatest.is_some_and(|old| self.greatest_ordinal.is_none_or(|last| last < old))
        {
            return Err(StoredPhaseErrorV1::Admission);
        }
        Ok(())
    }
}
#[cfg(test)]
#[path = "retirement_tests.rs"]
mod tests;

#[cfg(test)]
thread_local! { static BLIND_CLEARS:std::cell::Cell<(usize,bool)>=const{std::cell::Cell::new((0,true))}; }
#[cfg(test)]
pub(in crate::poly::stored_advice::phase) fn record_blind_clear(zero: bool) {
    BLIND_CLEARS.with(|s| {
        let (n, z) = s.get();
        s.set((n + 1, z && zero));
    });
}
#[cfg(test)]
pub(crate) fn take_blind_clear_observations() -> (usize, bool) {
    BLIND_CLEARS.with(|s| s.replace((0, true)))
}
