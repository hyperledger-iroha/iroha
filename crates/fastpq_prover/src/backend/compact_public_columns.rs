//! Exact public-column substitution for a new compact SMT proof layout.
//!
//! The 342-cell reference AIR remains unchanged. This owner removes exactly 41
//! verifier-known columns from commitments and reconstructs them at the actual
//! base/extension evaluation point before reference AIR evaluation. Each omitted
//! column is a linear combination of fixed 512-period selectors, with degree at
//! most N-N/512 for N=65536. No off-domain point is interpreted as a row phase.
//!
//! TODO: Bind this ordered layout and its reconstruction into the new protocol's
//! commitment, mixing, degree/opening argument and transcript before admission.
//! This private candidate does not change the existing 342-column proof format,
//! query schedule, production limits, or security qualification.

use fastpq_isi::StarkParameterSet;

use super::{
    fixed_domain::FixedTraceDomain, fixed_schedule::PeriodicSelectors,
    polynomial_field::PolynomialField,
};
use crate::{
    Error, Result,
    gadgets::{
        compact_blake2b_air::ROW_COUNT as EXECUTING_ROWS,
        compact_smt_air::{COLUMN_COUNT, PHYSICAL_HASH_ROWS, PHYSICAL_ROW_COUNT, PhysicalRowIndex},
    },
};

/// Identity of the exact projection, to be bound by the new protocol descriptor.
pub(super) const LAYOUT_ID: &str = "fastpq:compact:smt-public-columns:v1:342-to-301:period512:rows65536:public32-35,53-63,276-301:node83:execute408";
/// Number of verifier-known reference columns omitted from commitments.
pub(super) const PUBLIC_COLUMN_COUNT: usize = 41;
/// Complete ordered committed width after the exact public-column substitution.
pub(super) const COMMITTED_COLUMN_COUNT: usize = COLUMN_COUNT - PUBLIC_COLUMN_COUNT;
/// Inclusive upper bound on every reconstructed public column's degree.
pub(super) const PUBLIC_POLYNOMIAL_DEGREE: usize =
    PHYSICAL_ROW_COUNT - PHYSICAL_ROW_COUNT / PHYSICAL_HASH_ROWS;
/// Omitted reference indices, in ascending compact_trace_columns schema order.
///
/// 32..35: four complete domain u32 limbs; 53..63: eleven complete zero-padding
/// limbs; 276..299: 24 byte-presence cells; 300: byte length; 301: prefix count.
/// Message limbs 4 and 20 contain variable child bytes and remain committed.
pub(super) const PUBLIC_COLUMNS: [usize; PUBLIC_COLUMN_COUNT] = [
    32, 33, 34, 35, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 276, 277, 278, 279, 280, 281, 282,
    283, 284, 285, 286, 287, 288, 289, 290, 291, 292, 293, 294, 295, 296, 297, 298, 299, 300, 301,
];
/// Reference index of each retained commitment column, in canonical order.
pub(super) const COMMITTED_COLUMNS: [usize; COMMITTED_COLUMN_COUNT] = committed_columns();

const NODE_DOMAIN: &[u8; 19] = b"fastpq:v1:smt:node|";
const NODE_BYTES: u64 = 83;
const PRESENT_OFFSET: usize = 15;
const LENGTH_OFFSET: usize = 39;
const COUNT_OFFSET: usize = 40;

const fn committed_columns() -> [usize; COMMITTED_COLUMN_COUNT] {
    assert!(COLUMN_COUNT == 342 && PUBLIC_COLUMN_COUNT == 41);
    assert!(PHYSICAL_ROW_COUNT == 65536 && PHYSICAL_HASH_ROWS == 512 && EXECUTING_ROWS == 408);
    let mut result = [0; COMMITTED_COLUMN_COUNT];
    let mut source = 0;
    let mut public = 0;
    let mut retained = 0;
    while source < COLUMN_COUNT {
        if public < PUBLIC_COLUMN_COUNT && PUBLIC_COLUMNS[public] == source {
            public += 1;
        } else {
            result[retained] = source;
            retained += 1;
        }
        source += 1;
    }
    assert!(public == PUBLIC_COLUMN_COUNT && retained == COMMITTED_COLUMN_COUNT);
    result
}

fn domain_limb(limb: usize) -> u64 {
    u64::from(u32::from_le_bytes(
        NODE_DOMAIN[4 * limb..4 * limb + 4]
            .try_into()
            .expect("four complete fixed domain limbs"),
    ))
}

fn base_values(index: PhysicalRowIndex) -> [u64; PUBLIC_COLUMN_COUNT] {
    let phase = index.phase();
    let mut values = [0; PUBLIC_COLUMN_COUNT];
    if phase >= EXECUTING_ROWS {
        return values;
    }
    for (limb, value) in values[..4].iter_mut().enumerate() {
        *value = domain_limb(limb);
    }
    for byte in 0..24 {
        values[PRESENT_OFFSET + byte] = u64::from(phase < 6 && 24 * phase + byte < 83);
    }
    values[LENGTH_OFFSET] = NODE_BYTES;
    values[COUNT_OFFSET] = if phase < 6 {
        ((24 * (phase + 1)) as u64).min(NODE_BYTES)
    } else {
        NODE_BYTES
    };
    values
}

/// Project one genuine physical base row, rejecting inconsistent public cells.
///
/// The typed base-domain row index is used only during witness projection. An
/// LDE or extension opening must instead use [`PublicColumnReconstruction::reconstruct_at`].
/// Every input cell is canonical before any cell is dropped; projection alone
/// does not establish the remaining hash/SMT AIR or authenticate a statement.
pub(super) fn project_base_row(
    index: PhysicalRowIndex,
    complete: &[u64],
) -> Result<[u64; COMMITTED_COLUMN_COUNT]> {
    let complete = checked_cells::<u64, COLUMN_COUNT>(complete, "compact_public_base_row")?;
    for (&column, expected) in PUBLIC_COLUMNS.iter().zip(base_values(index)) {
        if complete[column] != expected {
            return Err(Error::InvalidTraceShape {
                details: format!(
                    "compact public column {column} differs at physical row {}",
                    index.get()
                ),
            });
        }
    }
    Ok(core::array::from_fn(|column| {
        complete[COMMITTED_COLUMNS[column]]
    }))
}

/// Immutable verifier-owned geometry for the exact 41 public polynomials.
///
/// Construction accepts trusted parameters, never proof-supplied fixed values.
/// Its storage and evaluation work depend on the fixed period, not witness size.
pub(super) struct PublicColumnReconstruction {
    selectors: PeriodicSelectors,
    generator: u64,
}

impl PublicColumnReconstruction {
    /// Validate the full trace/LDE geometry for exactly 65,536 physical rows.
    pub(super) fn new(params: &StarkParameterSet) -> Result<Self> {
        Ok(Self {
            selectors: PeriodicSelectors::new(params, PHYSICAL_ROW_COUNT, PHYSICAL_HASH_ROWS)?,
            generator: FixedTraceDomain::new(params, PHYSICAL_ROW_COUNT)?.generator,
        })
    }

    /// Evaluate all omitted columns at the full canonical base or Fp4 point.
    ///
    /// Only linear combinations of the fixed selectors are used. Subgroup
    /// selectors take their removable-singularity values; off-domain values
    /// remain polynomial evaluations and may have nonzero extension coordinates.
    pub(super) fn evaluate<F: PolynomialField>(
        &self,
        point: F,
    ) -> Result<[F; PUBLIC_COLUMN_COUNT]> {
        let phases = self.selectors.evaluate(point)?;
        let active = phases[..EXECUTING_ROWS]
            .iter()
            .copied()
            .fold(F::ZERO, F::add);
        let first_three = phases[0].add(phases[1]).add(phases[2]);
        let first_four = first_three.add(phases[3]);
        let mut values = [F::ZERO; PUBLIC_COLUMN_COUNT];
        for (limb, value) in values[..4].iter_mut().enumerate() {
            *value = active.scale_base(domain_limb(limb));
        }
        for byte in 0..24 {
            values[PRESENT_OFFSET + byte] = if byte < 11 { first_four } else { first_three };
        }
        values[LENGTH_OFFSET] = active.scale_base(NODE_BYTES);
        values[COUNT_OFFSET] = phases[0]
            .scale_base(24)
            .add(phases[1].scale_base(48))
            .add(phases[2].scale_base(72))
            .add(active.sub(first_three).scale_base(NODE_BYTES));
        Ok(values)
    }

    /// Reconstruct all 342 reference cells from exactly 301 canonical openings.
    ///
    /// Retained cells keep every extension coordinate. The 41 omitted values
    /// come exclusively from verifier-known polynomials at `point`; no prover
    /// phase, public-cell opening, or constant-on-the-LDE assumption is accepted.
    pub(super) fn reconstruct_at<F: PolynomialField>(
        &self,
        point: F,
        committed: &[F],
    ) -> Result<[F; COLUMN_COUNT]> {
        let committed = checked_cells::<F, COMMITTED_COLUMN_COUNT>(
            committed,
            "compact_public_committed_opening",
        )?;
        let known = self.evaluate(point)?;
        let mut complete = [F::ZERO; COLUMN_COUNT];
        for (&column, &value) in COMMITTED_COLUMNS.iter().zip(committed) {
            complete[column] = value;
        }
        for (&column, value) in PUBLIC_COLUMNS.iter().zip(known) {
            complete[column] = value;
        }
        Ok(complete)
    }

    /// Reconstruct a reference AIR pair at `x` and the actual next point `g*x`.
    ///
    /// `g` is the checked trace generator, not the LDE generator or a row label.
    /// The reference AIR retains its own noncyclic boundary selectors.
    pub(super) fn reconstruct_pair_at<F: PolynomialField>(
        &self,
        point: F,
        current: &[F],
        next: &[F],
    ) -> Result<([F; COLUMN_COUNT], [F; COLUMN_COUNT])> {
        let current = self.reconstruct_at(point, current)?;
        let next = self.reconstruct_at(point.scale_base(self.generator), next)?;
        Ok((current, next))
    }
}

fn checked_cells<'a, F: PolynomialField, const WIDTH: usize>(
    cells: &'a [F],
    context: &'static str,
) -> Result<&'a [F; WIDTH]> {
    let cells: &[F; WIDTH] = cells.try_into().map_err(|_| Error::InvalidTraceShape {
        details: format!("{context} needs exactly {WIDTH} cells"),
    })?;
    for (column, value) in cells.iter().copied().enumerate() {
        value.validate(context, &[column])?;
    }
    Ok(cells)
}

#[cfg(test)]
#[path = "compact_public_columns/tests.rs"]
mod tests;
