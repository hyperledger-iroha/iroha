//! Exact geometry and out-of-domain AIR linkage for the compact replacement.
//!
//! Constants describe one fixed unmasked relation. They are not proof-supplied
//! parameters or a production registry entry. The 41 public columns reconstruct
//! at the actual extension points before the unchanged 923-slot AIR is checked.
//! TODO: Integrate authenticated roots, transcript order and complete bounded
//! openings before this private owner participates in proof admission.

use fastpq_isi::{FASTPQ_FINAL_V1, StarkParameterSet};

use super::{
    FriDomain, GOLDILOCKS_MODULUS,
    compact_protocol::FixedAir,
    compact_public_columns::{COMMITTED_COLUMN_COUNT, PublicColumnReconstruction},
    compact_transfer_air::CompactTransferAir,
    deep_composition::{DeepComposition, OodPair},
    field_pow,
    fixed_domain::FixedTraceDomain,
    polynomial_field::PolynomialField,
};
use crate::{Error, Result, field::GoldilocksFp4V1 as F};

/// Physical one-delta execution subgroup order.
pub(super) const TRACE_ROWS: usize = 65_536;
/// Full low-degree extension domain, at blowup 128.
pub(super) const LDE_ROWS: usize = 8_388_608;
/// Independently sampled complete Fp4 constraint coefficients.
pub(super) const CONSTRAINTS: usize = 923;
/// Exact number of distinct uniformly sampled initial positions.
pub(super) const QUERY_COUNT: usize = 64;
/// Canonical field candidates consumed from the fixed whole query tape.
pub(super) const QUERY_CANDIDATES: usize = 74;
/// Ordered reduction factors; their product is the execution subgroup order.
pub(super) const FRI_ARITIES: [usize; 5] = [16, 16, 8, 8, 4];
/// Input and successive output domain lengths, including the complete terminal.
pub(super) const FRI_LENGTHS: [usize; 6] = [8_388_608, 524_288, 32_768, 4_096, 512, 128];
/// Exclusive degree bounds for the same six domains.
pub(super) const FRI_DEGREES: [usize; 6] = [65_536, 4_096, 256, 32, 4, 1];
/// Exact order-2^23 root extending the unchanged execution subgroup orientation.
pub(super) const LDE_ROOT: u64 = 0x35c4_528b_4aa6_2eb8;
/// Fixed disjoint coset offset, shared with the canonical field parameter owner.
pub(super) const COSET_OFFSET: u64 = FASTPQ_FINAL_V1.omega_coset;

/// Exact trusted geometry and fixed public-column substitution.
pub(super) struct DeepGeometry {
    domain: FriDomain,
    trace_generator: u64,
    public_columns: PublicColumnReconstruction,
}

impl DeepGeometry {
    /// Validate the sole replacement geometry independently of proof input.
    pub(super) fn new() -> Result<Self> {
        let params = Self::polynomial_parameters();
        let trace_generator = FixedTraceDomain::new(&params, TRACE_ROWS)?.generator;
        if params.lde_log_size != 23
            || trace_generator != FASTPQ_FINAL_V1.trace_root
            || field_pow(LDE_ROOT, 16) != FASTPQ_FINAL_V1.lde_root
            || FRI_LENGTHS[0] != LDE_ROWS
            || FRI_DEGREES[0] != TRACE_ROWS
        {
            return Err(shape(
                "DEEP geometry does not extend the exact execution subgroup",
            ));
        }
        for round in 0..FRI_ARITIES.len() {
            if FRI_LENGTHS[round] / FRI_ARITIES[round] != FRI_LENGTHS[round + 1]
                || FRI_DEGREES[round] / FRI_ARITIES[round] != FRI_DEGREES[round + 1]
                || FRI_LENGTHS[round] % FRI_ARITIES[round] != 0
                || FRI_DEGREES[round] % FRI_ARITIES[round] != 0
            {
                return Err(shape("DEEP folding domains and degrees are inconsistent"));
            }
        }
        let domain = FriDomain::from_lde_parameters(LDE_ROOT, 23, LDE_ROWS, COSET_OFFSET)?;
        Ok(Self {
            domain,
            trace_generator,
            public_columns: PublicColumnReconstruction::new(&params)?,
        })
    }

    /// Private FFT geometry; the ordered FRI schedule is separately fixed above.
    ///
    /// This copy is only a polynomial-planner input. It must not be registered
    /// as an admitted `StarkParameterSet` or used as a uniform-arity FRI schedule.
    pub(super) fn polynomial_parameters() -> StarkParameterSet {
        let mut params = FASTPQ_FINAL_V1;
        params.lde_log_size = 23;
        params.lde_root = LDE_ROOT;
        params.fri.blowup_factor = 128;
        params
    }

    /// Initial row and quotient evaluation coset.
    pub(super) const fn domain(&self) -> FriDomain {
        self.domain
    }

    /// Exact trace generator used to rotate current openings to next openings.
    pub(super) const fn trace_generator(&self) -> u64 {
        self.trace_generator
    }

    /// Check the complete OOD AIR identity and prepare the bounded composition.
    ///
    /// The caller must bind row/quotient roots before z, and these exact complete
    /// OOD answers before lambda. Success is an algebra check, not proof acceptance.
    pub(super) fn check_ood(
        &self,
        relation: &CompactTransferAir,
        alphas: &[F],
        z: F,
        current: &[F],
        next: &[F],
        quotient: &[F],
    ) -> Result<DeepComposition> {
        let schema = relation.schema();
        if schema.trace_rows != TRACE_ROWS
            || schema.width != 342
            || schema.constraints != CONSTRAINTS
            || alphas.len() != CONSTRAINTS
        {
            return Err(shape(
                "DEEP OOD check requires the complete fixed transfer relation",
            ));
        }
        for (index, &alpha) in alphas.iter().enumerate() {
            alpha.validate("deep_ood_alpha", &[index])?;
        }
        let points = OodPair::new(z, self.trace_generator)?;
        // This owner checks the entire shape and all scalar coordinates before
        // the fixed-polynomial reconstruction or AIR arithmetic below.
        let composition = DeepComposition::new(points, current, next, quotient)?;
        let (current, next) = self.public_columns.reconstruct_pair_at(z, current, next)?;
        let residues = relation.evaluate_at(z, &current, &next)?;
        if residues.len() != CONSTRAINTS {
            return Err(shape(
                "DEEP reference AIR returned an incomplete numerator vector",
            ));
        }
        let numerator = residues
            .into_iter()
            .zip(alphas)
            .fold(F::ZERO, |sum, (residue, &alpha)| {
                sum.add(residue.mul(alpha))
            });
        let z_to_n = z.power(TRACE_ROWS as u64);
        let quotient_at_z = quotient[0].add(z_to_n.mul(quotient[1]));
        if numerator != z_to_n.sub(F::ONE).mul(quotient_at_z) {
            return Err(shape(
                "DEEP out-of-domain AIR quotient identity does not hold",
            ));
        }
        Ok(composition)
    }
}

fn shape(details: &'static str) -> Error {
    Error::InvalidTraceShape {
        details: details.to_owned(),
    }
}

#[cfg(test)]
#[path = "deep_geometry/tests.rs"]
mod tests;
