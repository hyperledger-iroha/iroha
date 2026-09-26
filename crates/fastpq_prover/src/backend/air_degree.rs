//! Checked structural degree bounds for the full compact AIR polynomials.
//!
//! Bounds are exclusive, with zero denoting the identically zero polynomial.
//! They are propagated through the exact symbolic equations, not through an
//! interpolation of subgroup residues. Addition may cancel leading terms, so
//! a bound is not an assertion that the leading coefficient is nonzero.
//!
//! TODO: The masked quotient owner must authenticate declared column degrees,
//! construct the full numerator without aliasing, and establish zero remainder
//! before relying on the conditional quotient bounds. This arithmetic chooses
//! no masks, entropy, interpolation geometry, proof parameters or admission.

use super::{
    air_expression::Node,
    compact_hash_quotient::{LOCAL_SLOTS, TRANSITION_SLOTS},
    compact_smt_quotient::RESIDUE_COUNT,
};
use crate::{Error, Result};

/// Exact number and order of the complete relation's independently mixed slots.
pub(super) const SLOT_COUNT: usize = LOCAL_SLOTS + TRANSITION_SLOTS + RESIDUE_COUNT;

/// Exclusive polynomial degree upper bound; zero requires an identically zero value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PolynomialDegree(usize);

impl PolynomialDegree {
    /// Identically zero polynomial, including an omitted compiled output term.
    pub(super) const ZERO: Self = Self(0);

    /// Interpret an explicit exclusive bound without choosing a coefficient extent.
    pub(super) const fn from_exclusive(bound: usize) -> Self {
        Self(bound)
    }

    /// Convert a nonzero polynomial's inclusive degree without integer wraparound.
    pub(super) fn from_inclusive(degree: usize) -> Result<Self> {
        degree.checked_add(1).map(Self).ok_or_else(overflow)
    }

    /// Constant zero is exact; every other canonical constant has degree zero.
    pub(super) const fn constant(value: u64) -> Self {
        Self(if value == 0 { 0 } else { 1 })
    }

    /// The declared exclusive upper bound, not the actual leading-term degree.
    pub(super) const fn exclusive(self) -> usize {
        self.0
    }

    /// Both addition and subtraction are bounded by the larger operand degree.
    pub(super) fn sum(self, other: Self) -> Self {
        Self(self.0.max(other.0))
    }

    /// Full polynomial product, with checked degrees and exact zero annihilation.
    pub(super) fn product(self, other: Self) -> Result<Self> {
        if self == Self::ZERO || other == Self::ZERO {
            return Ok(Self::ZERO);
        }
        // Subtract before addition: a valid exclusive usize::MAX bound times
        // a constant remains representable and must not overflow spuriously.
        (self.0 - 1)
            .checked_add(other.0)
            .map(Self)
            .ok_or_else(overflow)
    }
}

/// Interpret a topologically ordered symbolic graph with explicitly named inputs.
///
/// Constants and graph simplifications are the actual equation compiler's. No
/// degree symbol implements field arithmetic, and equal bounds do not identify
/// two expressions or cause a false subtraction cancellation.
pub(super) fn evaluate_node_degrees(
    nodes: &[Node],
    input: &[PolynomialDegree],
) -> Result<Vec<PolynomialDegree>> {
    let mut values: Vec<PolynomialDegree> = Vec::with_capacity(nodes.len());
    for &node in nodes {
        let operand = |index: usize| {
            values
                .get(index)
                .copied()
                .ok_or_else(|| invalid("degree graph operand must precede its consumer"))
        };
        let degree = match node {
            Node::Constant(value) => PolynomialDegree::constant(value),
            Node::Input(index) => input
                .get(index)
                .copied()
                .ok_or_else(|| invalid("degree graph input exceeds the exact declared width"))?,
            Node::Add(left, right) | Node::Sub(left, right) => operand(left)?.sum(operand(right)?),
            Node::Mul(left, right) => operand(left)?.product(operand(right)?)?,
        };
        values.push(degree);
    }
    Ok(values)
}

/// Full polynomial upper bounds in hash-local, hash-edge, then SMT slot order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AirDegreeBounds {
    trace_rows: usize,
    numerators: [usize; SLOT_COUNT],
    combined_numerator: usize,
}

impl AirDegreeBounds {
    /// Bind the exact complete slot vector to the degree of its full vanishing polynomial.
    pub(super) fn new(trace_rows: usize, numerators: Vec<PolynomialDegree>) -> Result<Self> {
        if trace_rows == 0 {
            return Err(invalid(
                "degree accounting requires a nonzero vanishing degree",
            ));
        }
        let numerators: [PolynomialDegree; SLOT_COUNT] = numerators
            .try_into()
            .map_err(|_| invalid("degree accounting requires all 923 slots in canonical order"))?;
        let numerators = numerators.map(PolynomialDegree::exclusive);
        let combined_numerator = numerators.iter().copied().max().unwrap_or(0);
        Ok(Self {
            trace_rows,
            numerators,
            combined_numerator,
        })
    }

    /// All exclusive full numerator bounds, including zero slots.
    #[cfg(test)]
    pub(super) fn numerators(&self) -> &[usize; SLOT_COUNT] {
        &self.numerators
    }

    /// Exclusive bound on a constant-weight mixture of all full numerator polynomials.
    ///
    /// Any interpolation domain must contain at least this many distinct points
    /// to determine the full polynomial. Subgroup size and quotient degree alone
    /// do not establish an adequate domain. This does not select such a domain.
    pub(super) const fn combined_numerator(&self) -> usize {
        self.combined_numerator
    }

    /// Conditional bounds for division by X^N-1; this does not establish divisibility.
    ///
    /// The caller must separately prove zero remainder for the full numerator.
    /// If its bound is <=N, divisibility forces that numerator and quotient to be
    /// identically zero. These are obligations, never verified coefficient claims.
    #[cfg(test)]
    pub(super) fn conditional_quotients(&self) -> ConditionalQuotientBounds {
        ConditionalQuotientBounds {
            slots: self
                .numerators
                .map(|bound| bound.saturating_sub(self.trace_rows)),
            combined: self.combined_numerator.saturating_sub(self.trace_rows),
        }
    }
}

/// Quotient degree upper bounds that hold only after exact zero-remainder division.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg(test)]
pub(super) struct ConditionalQuotientBounds {
    /// Conditional exclusive degree of each slot's quotient, in complete AIR order.
    pub(super) slots: [usize; SLOT_COUNT],
    /// Conditional exclusive degree of the constant-weight combined quotient.
    pub(super) combined: usize,
}

fn overflow() -> Error {
    invalid("full AIR polynomial degree exceeds the addressable integer bound")
}

fn invalid(details: &'static str) -> Error {
    Error::InvalidTraceShape {
        details: details.to_owned(),
    }
}

#[cfg(test)]
#[path = "air_degree/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "air_degree/reference.rs"]
pub(super) mod reference;
