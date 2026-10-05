//! Typed semantic AIR interface over the fixed q77 engine.
//!
//! A relation is described once against
//! [`SemanticAir`](crate::air::SemanticAir): its trusted schema (identity,
//! trace shape, constraint count and numerator degree), its public IO
//! (canonical statement bytes plus the verifier-known public columns) and one
//! field-generic evaluator that returns every numerator at a point. The same
//! description is consumed by two owners:
//!
//! - the committed q77 engine, whose accepted relation set stays sealed
//!   ([`q77`](crate::air::q77)); the canonical ordinary and AXT quantity
//!   relations reach its out-of-domain check through this interface, and its
//!   bounded verifier and producer plan take their limits only as the typed
//!   [`q77::VerifierLimits`](crate::air::q77::VerifierLimits) and
//!   [`q77::ProducerLimits`](crate::air::q77::ProducerLimits), and
//! - the uncommitted algebraic reference
//!   ([`build_reference`](crate::air::build_reference) and
//!   [`check_reference`](crate::air::check_reference)), which any downstream
//!   relation can use today.
//!
//! All numerators apply to every row of the cyclic trace subgroup, with the
//! last row's successor being the first row. Boundary and public-input rules
//! are expressed with verifier-known public columns or with polynomials of the
//! evaluation point, exactly as the q77 relations do.
//!
//! The reference artifact is **uncommitted, nonsuccinct, non-hiding and not a
//! transaction proof**. It carries the complete trace in the clear, has no
//! Merkle commitment, FRI, transcript or wire encoding, and grants no ledger
//! authority. Its sizes and timings describe the reference only and must not be
//! reported as committed-proof measurements.
//!
//! TODO: B.2 generalizes the committed engine (composition/DEEP, masking,
//! commitments, FRI, transcript and acceleration) behind this interface; until
//! then only the sealed q77 relations have committed proofs.
//!
//! A counter that must reach a public end value, checked with the reference:
//!
//! ```
//! use fastpq_prover::air::{
//!     AirError, AirSchema, IntegerAirField, NoObserver, PolynomialField, ReferenceChallenges,
//!     SemanticAir, WorkLimits, build_reference, check_reference,
//! };
//!
//! /// `a' = a + 1` on every row but the last; the last row holds `end`.
//! struct Counter {
//!     rows: usize,
//!     end: u64,
//!     statement: [u8; 8],
//! }
//!
//! impl SemanticAir for Counter {
//!     fn schema(&self) -> AirSchema {
//!         AirSchema {
//!             identity: "example:counter:v1",
//!             trace_rows: self.rows,
//!             width: 2,
//!             constraints: 2,
//!             // A public column of degree < N times a linear term.
//!             numerator_degree_bound: 2 * self.rows - 1,
//!         }
//!     }
//!     fn statement_bytes(&self) -> &[u8] {
//!         &self.statement
//!     }
//!     fn public_columns(&self) -> &[usize] {
//!         &[1]
//!     }
//!     fn public_value(&self, _ordinal: usize, row: usize) -> fastpq_prover::Result<u64> {
//!         Ok(u64::from(row == self.rows - 1))
//!     }
//!     fn evaluate<F: PolynomialField>(
//!         &self,
//!         _point: F,
//!         current: &[F],
//!         next: &[F],
//!     ) -> fastpq_prover::Result<Vec<F>> {
//!         let (value, last) = (current[0], current[1]);
//!         Ok(vec![
//!             F::ONE.sub(last).mul(next[0].sub(value).sub(F::ONE)),
//!             last.mul(value.sub(F::embed_base(self.end))),
//!         ])
//!     }
//! }
//!
//! let rows = 8;
//! let air = Counter { rows, end: 12, statement: 12_u64.to_le_bytes() };
//! let trace = vec![(5..13).collect::<Vec<u64>>(), vec![0, 0, 0, 0, 0, 0, 0, 1]];
//! let challenges = ReferenceChallenges::derive(&[7; 32], 2);
//! let limits = WorkLimits::default();
//!
//! let artifact = build_reference(&air, trace.clone(), &challenges, limits, &NoObserver)?;
//! check_reference(&air, &artifact, &challenges, limits, &NoObserver)?;
//!
//! // A verifier expecting another end value rejects the same trace by evaluation.
//! let other = Counter { rows, end: 13, statement: 12_u64.to_le_bytes() };
//! assert!(matches!(
//!     check_reference(&other, &artifact, &challenges, limits, &NoObserver),
//!     Err(AirError::ConstraintViolated { row: 7, constraint: 1 })
//! ));
//! # Ok::<(), AirError>(())
//! ```

use super::{deep_binding, polynomial_field};
use crate::Error;

#[path = "air/q77.rs"]
pub mod q77;
#[path = "air/reference.rs"]
mod reference;

pub use crate::gadgets::transfer_integer_air::IntegerAirField;
pub use polynomial_field::PolynomialField;
pub use reference::{
    MAX_REFERENCE_INTERPOLATION_ROWS, MAX_REFERENCE_TRACE_ROWS, ReferenceArtifact,
    ReferenceChallenges, ReferencePlan, build_reference, check_reference,
};

/// Longest relation identity, in bytes, that the q77 statement context binds.
pub const MAX_RELATION_IDENTITY_BYTES: usize = deep_binding::MAX_RELATION_IDENTITY_BYTES;
/// Longest canonical public statement, in bytes, that the q77 context binds.
pub const MAX_STATEMENT_BYTES: usize = deep_binding::MAX_STATEMENT_BYTES;

/// Exact trusted relation geometry and identity; never taken from proof input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AirSchema {
    /// Versioned circuit, column order, slot order, selector and public-packing identity.
    pub identity: &'static str,
    /// Trace subgroup order including all physical padding; a power of two.
    pub trace_rows: usize,
    /// Complete base-column width in canonical order, public columns included.
    pub width: usize,
    /// Exact number of independently mixed numerators in canonical order.
    pub constraints: usize,
    /// Exclusive degree bound of every numerator polynomial when each trace
    /// column polynomial has degree below `trace_rows`.
    pub numerator_degree_bound: usize,
}

impl AirSchema {
    /// Check the declared geometry before any allocation or arithmetic.
    ///
    /// # Errors
    /// Rejects an empty or oversized identity, a trace order that is not a
    /// power of two, an empty width or constraint list, a zero numerator degree
    /// bound and a trace whose cell count overflows.
    pub fn validate(&self) -> Result<(), AirError> {
        if self.identity.is_empty() || self.identity.len() > MAX_RELATION_IDENTITY_BYTES {
            return Err(AirError::Schema(
                "relation identity must have 1..=256 bytes",
            ));
        }
        if !self.trace_rows.is_power_of_two() {
            return Err(AirError::Schema("trace rows must be a power of two"));
        }
        if self.width == 0 || self.constraints == 0 {
            return Err(AirError::Schema(
                "relation needs at least one column and one constraint",
            ));
        }
        if self.numerator_degree_bound == 0 {
            return Err(AirError::Schema("numerator degree bound must be positive"));
        }
        if self.trace_rows.checked_mul(self.width).is_none() {
            return Err(AirError::Schema("trace cell count overflows"));
        }
        Ok(())
    }

    /// Complete base trace cell count; [`Self::validate`] excludes overflow.
    #[must_use]
    pub const fn trace_cells(&self) -> usize {
        self.trace_rows.saturating_mul(self.width)
    }

    /// Exclusive degree bound of the exact quotient by `X^N - 1`.
    #[must_use]
    pub const fn quotient_degree_bound(&self) -> usize {
        self.numerator_degree_bound.saturating_sub(self.trace_rows)
    }
}

/// One relation, described once for every engine owner.
///
/// Implementations are pure functions of the relation's trusted public data.
/// The schema and statement must not depend on witness values, and
/// [`Self::evaluate`] must be a polynomial map: the same code path is run on
/// base-field trace rows and at quartic-extension points, and the reference
/// checker rejects an evaluator whose extension values do not interpolate to
/// the declared degree.
pub trait SemanticAir: Sync {
    /// Exact trusted geometry and identity of this relation.
    fn schema(&self) -> AirSchema;

    /// Exact canonical public statement, already authenticated by the caller.
    ///
    /// Every committed proof binds these bytes together with the identity.
    fn statement_bytes(&self) -> &[u8];

    /// Ascending indices of the verifier-known columns.
    ///
    /// A verifier reconstructs these columns itself from [`Self::public_value`];
    /// a prover's cells in them carry no authority.
    fn public_columns(&self) -> &[usize];

    /// Verifier-known base value of public column `ordinal` at subgroup `row`.
    ///
    /// `ordinal` indexes [`Self::public_columns`].
    ///
    /// # Errors
    /// Returns an error when `ordinal` or `row` is outside the declared schema.
    fn public_value(&self, ordinal: usize, row: usize) -> crate::Result<u64>;

    /// Evaluate every numerator at `point` from complete current and next rows.
    ///
    /// `current` and `next` hold all `width` cells at `point` and at `point`
    /// times the trace generator. The result has exactly `constraints` values
    /// in canonical slot order. On a trace row every value must be zero.
    ///
    /// # Errors
    /// Returns an error for a wrong row width or a noncanonical point or cell.
    fn evaluate<F: PolynomialField>(
        &self,
        point: F,
        current: &[F],
        next: &[F],
    ) -> crate::Result<Vec<F>>;
}

/// Caller ceilings checked against a relation's declared work before allocation.
///
/// These are structural accounting limits, not reserved memory, process RSS or
/// elapsed time. No limit is read from an artifact or from proof bytes.
///
/// Every owner behind the interface checks the same value. The uncommitted
/// reference takes it directly. The committed q77 engine takes it inside
/// [`q77::VerifierLimits`] and [`q77::ProducerLimits`], which add the
/// ceilings only a committed proof has. Each operation declares its own
/// payload and work; the unit of work is one field value inspected or one
/// numerator evaluated, plus the operation's transform work.
#[allow(
    clippy::struct_field_names,
    reason = "every field is an inclusive ceiling, named like the crate's other limit types"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkLimits {
    /// Maximum trace subgroup order.
    pub max_trace_rows: usize,
    /// Maximum complete base trace cells.
    pub max_trace_cells: usize,
    /// Maximum numerator count.
    pub max_constraints: usize,
    /// Maximum canonical public statement bytes.
    pub max_statement_bytes: usize,
    /// Maximum simultaneous declared payload bytes of one operation.
    pub max_payload_bytes: usize,
    /// Maximum declared structural field-arithmetic and inspection work.
    pub max_work_units: usize,
}

impl Default for WorkLimits {
    /// The committed q77 producer's default segment ceilings.
    ///
    /// This is specifically the producer's projection,
    /// [`q77::ProducerLimits::default`]'s `work`: the fixed profile's one
    /// 65,536 x 342 trace and 923 numerators, the statement envelope the
    /// transcript context binds, and the default proving policy's segment
    /// payload charge and work ceiling. No number is restated here. Bounded
    /// verification declares other payload and work; its defaults are
    /// [`q77::VerifierLimits::default`]'s `work`.
    fn default() -> Self {
        q77::ProducerLimits::default().work
    }
}

/// Which interface operation an [`Event`] belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    /// [`build_reference`].
    BuildReference,
    /// [`check_reference`].
    CheckReference,
    /// Committed q77 proof construction for a sealed relation.
    Q77Prove,
    /// Committed q77 bounded verification for a sealed relation.
    Q77Verify,
}

/// Measured work of one completed operation.
///
/// Each variant reports only what its owner actually measured. Reference
/// figures describe the uncommitted reference and must not be reported as
/// committed-proof size or prover cost.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Work {
    /// Uncommitted reference build or check.
    Reference(ReferenceWork),
    /// Committed q77 construction, already checked by the bounded verifier.
    Q77Proof {
        /// Canonical child proof frame bytes.
        proof_bytes: usize,
    },
    /// Committed q77 bounded verification.
    Q77Verification(q77::VerificationWork),
}

/// Structural work of one uncommitted reference run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReferenceWork {
    /// Complete evaluations of all numerators at one point: every subgroup
    /// row, every interpolation point and the out-of-domain point.
    pub constraint_evaluations: usize,
    /// Declared payload charge that was admitted for the run.
    pub payload_bytes: usize,
    /// Declared structural work that was admitted for the run.
    pub work_units: usize,
}

/// Public structural progress of an interface operation.
///
/// Events never carry witness cells, quotient coefficients or challenge values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Event<'a> {
    /// Schema, statement and declared work passed every caller ceiling.
    Admitted {
        /// Operation being admitted.
        operation: Operation,
        /// Relation identity bound by the operation.
        identity: &'a str,
        /// Canonical statement length.
        statement_bytes: usize,
        /// Declared payload charge.
        payload_bytes: usize,
        /// Declared structural work.
        work_units: usize,
    },
    /// Every public column matched and every subgroup row satisfied every numerator.
    TraceChecked {
        /// Operation that checked the trace.
        operation: Operation,
        /// Checked subgroup rows.
        rows: usize,
        /// Numerators evaluated on each row.
        constraints: usize,
    },
    /// The complete numerator was reconstructed and exactly divided.
    QuotientChecked {
        /// Operation that built or re-derived the quotient.
        operation: Operation,
        /// Interpolation domain rows used for the numerator.
        interpolation_rows: usize,
        /// Exact quotient coefficient extent.
        quotient_coefficients: usize,
    },
    /// The out-of-domain quotient identity held at the challenge point.
    OodChecked {
        /// Operation that checked the identity.
        operation: Operation,
    },
    /// The operation finished successfully.
    Completed {
        /// Finished operation.
        operation: Operation,
        /// Relation identity bound by the operation.
        identity: &'a str,
        /// Measured structural work.
        work: Work,
    },
    /// The operation was refused or failed; no partial result exists.
    Rejected {
        /// Refused operation.
        operation: Operation,
        /// Relation identity the caller expected.
        identity: &'a str,
    },
}

/// Receives public progress events; it cannot change any result.
///
/// Observers run synchronously on the calling thread. A panic inside an
/// observer is caught and logged, so a hook can neither turn a rejection into
/// success nor abort a valid operation.
pub trait Observer: Sync {
    /// Record one public event.
    fn observe(&self, event: &Event<'_>);
}

/// Observer that records nothing.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoObserver;

impl Observer for NoObserver {
    fn observe(&self, _event: &Event<'_>) {}
}

/// Deliver one event, containing any observer panic.
pub(super) fn notify(observer: &dyn Observer, event: &Event<'_>) {
    let delivered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        observer.observe(event);
    }));
    if delivered.is_err() {
        tracing::warn!(
            target: "fastpq::air",
            "AIR observer panicked; the operation result is unaffected"
        );
    }
}

/// Typed failure of an interface operation.
#[derive(Debug, thiserror::Error)]
pub enum AirError {
    /// The relation's declared schema is unusable.
    #[error("AIR schema is malformed: {0}")]
    Schema(&'static str),
    /// A declared quantity exceeds the caller's ceiling.
    #[error("AIR limit `{limit}` exceeded: {actual} > {max}")]
    Limit {
        /// Name of the exceeded limit.
        limit: &'static str,
        /// Declared or actual quantity.
        actual: usize,
        /// Caller ceiling.
        max: usize,
    },
    /// A trace, quotient or challenge input has the wrong dimensions.
    #[error("AIR reference input is malformed: {0}")]
    Shape(&'static str),
    /// The artifact names a different relation than the checker expects.
    #[error("AIR reference artifact binds another relation identity")]
    IdentityMismatch,
    /// The artifact binds different public statement bytes than expected.
    #[error("AIR reference artifact binds another public statement")]
    StatementMismatch,
    /// A verifier-known public column cell differs from the relation's value.
    #[error("public column {column} differs from the relation at row {row}")]
    PublicColumnMismatch {
        /// Reference index of the public column.
        column: usize,
        /// Subgroup row of the first difference.
        row: usize,
    },
    /// A numerator is nonzero on a trace row.
    #[error("constraint {constraint} is violated at row {row}")]
    ConstraintViolated {
        /// Subgroup row of the first violation.
        row: usize,
        /// Canonical slot of the first violated numerator on that row.
        constraint: usize,
    },
    /// The evaluator returned another number of numerators than it declared.
    #[error("relation returned {actual} numerators but declares {expected}")]
    ConstraintCount {
        /// Declared numerator count.
        expected: usize,
        /// Returned numerator count.
        actual: usize,
    },
    /// The reconstructed numerator has a coefficient at or above its declared bound.
    #[error("numerator coefficient {coefficient} is nonzero above the declared degree bound")]
    DegreeBoundExceeded {
        /// Lowest offending coefficient index.
        coefficient: usize,
    },
    /// The reconstructed numerator is not a multiple of `X^N - 1`.
    #[error("numerator is not divisible by the trace vanishing polynomial")]
    NotDivisible,
    /// The artifact's quotient differs from the independently derived quotient.
    #[error("quotient coefficient {coefficient} differs from the reconstructed quotient")]
    QuotientMismatch {
        /// Lowest differing coefficient index.
        coefficient: usize,
    },
    /// The out-of-domain quotient identity does not hold.
    #[error("out-of-domain AIR quotient identity does not hold")]
    OodIdentity,
    /// An arithmetic owner or the relation's evaluator reported an error.
    #[error(transparent)]
    Engine(#[from] Error),
}

#[cfg(test)]
#[path = "air/tests.rs"]
mod tests;
