//! Role-separated fixed/instance scalar access without a required dense auxiliary bank.
//!
//! This trusted source contract carries no key admission. Its concrete owner must bind the
//! exact proof/domain/basis/column interpretation and destroy its complete operation after an
//! error or unwind. Getters may return owned field copies, never a backend or borrowed value.

use super::{
    StoredExpressionContextV1, StoredExpressionErrorV1, StoredRowTileV1, TILE, rotated_first,
};
use crate::poly::{
    LagrangeCoeff, Polynomial,
    stored_advice::{StoredPolynomialLayoutV1, assignment::StoredAssignmentFieldV1},
};

/// Fallible fixed/instance access owned by the same admitted proof as the expression plan.
///
/// `validate` must compare actual retained proof context, field, k, exact basis (including any
/// coset part), both column counts and representation lengths against the supplied expectation.
/// It must not simply adopt the expectation as authority. Distinct getters fix the column role;
/// both must reject missing columns and out-of-domain rows. A base-domain instance owner may
/// return ZERO only after validating an in-range instance role/column/row that lies beyond that
/// column's supplied prefix. It must reject coset interpretation of such padding.
///
/// The evaluator passes already-wrapped rows and owns arithmetic/rotation order. Implementers
/// must release backend windows before returning, guard any source-owned scalar scratch, and
/// propagate backend/validation failures. The raw evaluator does not own or poison this source;
/// a concrete complete-proof owner must enclose it. A future coset source may retain a bounded
/// cache, but this interface alone establishes neither coset provenance nor storage-read bounds.
pub(crate) trait StoredAuxiliarySourceV1<F: StoredAssignmentFieldV1> {
    /// Check exact expected domain/role dimensions before any scalar access or output exposure.
    fn validate(
        &mut self,
        expected: StoredExpressionContextV1<'_>,
    ) -> Result<(), StoredExpressionErrorV1>;

    /// Read one Fixed-role value at an already-wrapped, in-domain row, or fail.
    fn fixed_value(&mut self, column: usize, row: usize) -> Result<F, StoredExpressionErrorV1>;

    /// Read one Instance-role value at an already-wrapped, in-domain row, or fail.
    fn instance_value(&mut self, column: usize, row: usize) -> Result<F, StoredExpressionErrorV1>;
}

/// Preserve the existing dense wrapper contract without copying any polynomial or bank.
pub(super) struct DenseAuxiliarySourceV1<'fixed, 'instance, F> {
    // This is the existing caller-supplied domain label, not a fabricated auxiliary snapshot.
    domain: StoredPolynomialLayoutV1,
    fixed: &'fixed [Polynomial<F, LagrangeCoeff>],
    instance: &'instance [Polynomial<F, LagrangeCoeff>],
}
impl<'fixed, 'instance, F> DenseAuxiliarySourceV1<'fixed, 'instance, F> {
    pub(super) fn new(
        domain: StoredPolynomialLayoutV1,
        fixed: &'fixed [Polynomial<F, LagrangeCoeff>],
        instance: &'instance [Polynomial<F, LagrangeCoeff>],
    ) -> Self {
        Self {
            domain,
            fixed,
            instance,
        }
    }
}
impl<F: StoredAssignmentFieldV1> StoredAuxiliarySourceV1<F> for DenseAuxiliarySourceV1<'_, '_, F> {
    fn validate(
        &mut self,
        expected: StoredExpressionContextV1<'_>,
    ) -> Result<(), StoredExpressionErrorV1> {
        let size = self.domain.scalar_count();
        if !self.domain.same_proof_context(expected.domain)
            || self.domain.field() != F::STORED_FIELD
            || self.domain.field() != expected.domain.field()
            || self.domain.k() != expected.domain.k()
            || self.domain.basis() != expected.domain.basis()
            || self.fixed.len() != expected.fixed_columns
            || self.instance.len() != expected.instance_columns
            || self
                .fixed
                .iter()
                .chain(self.instance)
                .any(|column| column.len() != size)
        {
            return Err(StoredExpressionErrorV1::Context);
        }
        Ok(())
    }

    fn fixed_value(&mut self, column: usize, row: usize) -> Result<F, StoredExpressionErrorV1> {
        if row >= self.domain.scalar_count() {
            return Err(StoredExpressionErrorV1::Context);
        }
        self.fixed
            .get(column)
            .and_then(|column| column.get(row))
            .copied()
            .ok_or(StoredExpressionErrorV1::Context)
    }

    fn instance_value(&mut self, column: usize, row: usize) -> Result<F, StoredExpressionErrorV1> {
        if row >= self.domain.scalar_count() {
            return Err(StoredExpressionErrorV1::Context);
        }
        self.instance
            .get(column)
            .and_then(|column| column.get(row))
            .copied()
            .ok_or(StoredExpressionErrorV1::Context)
    }
}

#[derive(Clone, Copy)]
pub(super) enum AuxiliaryRoleV1 {
    Fixed,
    Instance,
}

pub(super) fn read_auxiliary<F: StoredAssignmentFieldV1, X: StoredAuxiliarySourceV1<F>>(
    auxiliary: &mut X,
    context: StoredExpressionContextV1<'_>,
    role: AuxiliaryRoleV1,
    column: usize,
    tile: StoredRowTileV1,
    rotation: i32,
    destination: &mut [F],
) -> Result<(), StoredExpressionErrorV1> {
    let size = context.domain.scalar_count();
    let columns = match role {
        AuxiliaryRoleV1::Fixed => context.fixed_columns,
        AuxiliaryRoleV1::Instance => context.instance_columns,
    };
    if column >= columns {
        return Err(StoredExpressionErrorV1::Context);
    }
    if tile.start >= size
        || tile.start % TILE != 0
        || tile.len != TILE.min(size - tile.start)
        || destination.len() < tile.len
    {
        return Err(StoredExpressionErrorV1::Tile);
    }
    auxiliary.validate(context)?;
    let first = rotated_first(tile.start, rotation, size);
    for (offset, value) in destination[..tile.len].iter_mut().enumerate() {
        let row = (first + offset) % size;
        *value = match role {
            AuxiliaryRoleV1::Fixed => auxiliary.fixed_value(column, row)?,
            AuxiliaryRoleV1::Instance => auxiliary.instance_value(column, row)?,
        };
    }
    auxiliary.validate(context)
}
