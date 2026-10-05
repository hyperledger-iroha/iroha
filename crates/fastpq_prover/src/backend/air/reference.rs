//! Uncommitted algebraic reference for any [`SemanticAir`].
//!
//! The producer and the checker share no trust. [`check_reference`] receives
//! the expected relation with its public statement and the complete artifact,
//! and independently (1) compares identity, statement and every verifier-known
//! public column, (2) evaluates every numerator on every row of the cyclic
//! trace subgroup in the base field, (3) reconstructs the actual
//! challenge-weighted numerator polynomial on a disjoint coset in the quartic
//! extension, (4) checks its declared degree bound and its exact division by
//! `X^N - 1`, (5) compares the artifact's quotient coefficient by coefficient
//! and (6) checks the out-of-domain identity
//! `sum_k alpha_k C_k(z, A(z), A(omega z)) = (z^N - 1) Q(z)`.
//!
//! All arithmetic uses the existing engine owners: the four-lane coset
//! transform, the exact vanishing-polynomial division and the canonical trace
//! and evaluation roots of `FASTPQ_FINAL_V1`. Nothing is committed: there is no
//! Merkle tree, FRI, transcript, masking or wire format, so the artifact is
//! nonsuccinct, non-hiding and never a transaction proof. Steps (3) to (6) do
//! not add soundness beyond step (2) for an uncommitted trace; they establish
//! that the relation's evaluator is the polynomial map the committed engine
//! requires and that its declared degree is honest.

use fastpq_isi::FASTPQ_FINAL_V1;
use rayon::prelude::*;
use sha2::{Digest, Sha256};

use super::{
    super::{
        field_pow,
        fixed_domain::FixedTraceDomain,
        masked_quotient::{checked_add, checked_mul, transform_work},
        polynomial_division::VanishingDivisionPlan,
        polynomial_transform::PolynomialDomain,
        secret_polynomial::SecretPolynomial,
    },
    AirError, AirSchema, Event, MAX_STATEMENT_BYTES, Observer, Operation, PolynomialField,
    ReferenceWork, SemanticAir, Work, WorkLimits, notify,
};
use crate::{
    Error,
    field::{GOLDILOCKS_MODULUS_V1, GoldilocksFp4V1 as F, mul_base},
};

/// Largest trace subgroup the reference transforms support.
pub const MAX_REFERENCE_TRACE_ROWS: usize = 1 << FASTPQ_FINAL_V1.trace_log_size;
/// Largest numerator interpolation domain the reference transforms support.
pub const MAX_REFERENCE_INTERPOLATION_ROWS: usize = 1 << FASTPQ_FINAL_V1.lde_log_size;

/// Fixed contiguous job count of the row and coset evaluations. Admission,
/// outputs and the reported first failure do not depend on the thread pool.
const REFERENCE_JOBS: usize = 32;

/// Domain tag of the reference-only challenge expansion.
const CHALLENGE_DOMAIN: &[u8] = b"fastpq:air:reference-challenges:v1";

/// Constraint weights and the out-of-domain point used by one reference run.
///
/// The producer and the checker must use the same values. They are explicit
/// inputs, not a Fiat-Shamir transcript: the reference commits to nothing, and
/// the checker re-evaluates every row itself, so its verdict on the trace does
/// not rest on these values being unpredictable.
#[derive(Clone, PartialEq, Eq)]
pub struct ReferenceChallenges {
    alphas: Vec<F>,
    ood_point: F,
}

impl core::fmt::Debug for ReferenceChallenges {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ReferenceChallenges")
            .field("alphas", &self.alphas.len())
            .finish_non_exhaustive()
    }
}

impl ReferenceChallenges {
    /// Use explicit weights, one per numerator, and an explicit extension point.
    ///
    /// The values are validated against the relation when they are used.
    #[must_use]
    pub fn new(alphas: Vec<F>, ood_point: F) -> Self {
        Self { alphas, ood_point }
    }

    /// Expand `seed` into `constraints` weights and one proper extension point.
    ///
    /// This is a deterministic reference-only expansion (SHA-256 in counter
    /// mode with rejection of noncanonical words). It is not the q77 transcript
    /// and must not be used as one.
    #[must_use]
    pub fn derive(seed: &[u8; 32], constraints: usize) -> Self {
        let mut words = ChallengeWords::new(seed);
        let alphas = (0..constraints).map(|_| words.next_field()).collect();
        let ood_point = loop {
            let candidate = words.next_field();
            if !is_base_embedded(candidate) {
                break candidate;
            }
        };
        Self { alphas, ood_point }
    }

    /// Constraint weights in canonical slot order.
    #[must_use]
    pub fn alphas(&self) -> &[F] {
        &self.alphas
    }

    /// Out-of-domain evaluation point.
    #[must_use]
    pub const fn ood_point(&self) -> F {
        self.ood_point
    }

    fn validate(&self, schema: &AirSchema) -> Result<(), AirError> {
        if self.alphas.len() != schema.constraints {
            return Err(AirError::Shape(
                "challenges need exactly one weight per constraint",
            ));
        }
        for (index, &alpha) in self.alphas.iter().enumerate() {
            alpha.validate("air_reference_alpha", &[index])?;
        }
        self.ood_point.validate("air_reference_ood_point", &[])?;
        if is_base_embedded(self.ood_point) {
            return Err(AirError::Shape(
                "out-of-domain point must lie outside the base field",
            ));
        }
        Ok(())
    }
}

/// Whether an extension element has only its base coordinate.
fn is_base_embedded(value: F) -> bool {
    value.coefficients()[1..].iter().all(|&word| word == 0)
}

/// Counter-mode SHA-256 stream of canonical base-field words.
struct ChallengeWords {
    seed: [u8; 32],
    counter: u64,
    block: [u8; 32],
    offset: usize,
}

impl ChallengeWords {
    fn new(seed: &[u8; 32]) -> Self {
        Self {
            seed: *seed,
            counter: 0,
            block: [0; 32],
            offset: 32,
        }
    }

    fn next_word(&mut self) -> u64 {
        loop {
            if self.offset == 32 {
                let mut hasher = Sha256::new();
                hasher.update(CHALLENGE_DOMAIN);
                hasher.update(self.seed);
                hasher.update(self.counter.to_le_bytes());
                self.block = hasher.finalize().into();
                self.counter = self.counter.wrapping_add(1);
                self.offset = 0;
            }
            let word = u64::from_le_bytes(
                self.block[self.offset..self.offset + 8]
                    .try_into()
                    .expect("eight-byte challenge word"),
            );
            self.offset += 8;
            if word < GOLDILOCKS_MODULUS_V1 {
                return word;
            }
        }
    }

    fn next_field(&mut self) -> F {
        let coefficients = core::array::from_fn(|_| self.next_word());
        F::new(coefficients).expect("rejection sampling yields canonical words")
    }
}

/// Complete uncommitted evidence that a trace satisfies one relation.
///
/// **Uncommitted, nonsuccinct, non-hiding and not a transaction proof.** It
/// holds the whole trace in the clear and has no wire encoding. Constructing
/// one from parts establishes nothing; only [`check_reference`] does.
#[derive(Clone, PartialEq, Eq)]
pub struct ReferenceArtifact {
    identity: String,
    statement: Vec<u8>,
    trace: Vec<Vec<u64>>,
    quotient: Vec<F>,
}

impl core::fmt::Debug for ReferenceArtifact {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ReferenceArtifact")
            .field("identity", &self.identity)
            .field("statement_bytes", &self.statement.len())
            .field("columns", &self.trace.len())
            .field("quotient_coefficients", &self.quotient.len())
            .finish()
    }
}

impl ReferenceArtifact {
    /// Assemble an artifact from explicit parts, for example a mutated copy.
    ///
    /// `trace` is column-major: one vector of `trace_rows` base values per
    /// column. No check is performed here.
    #[must_use]
    pub fn from_parts(
        identity: String,
        statement: Vec<u8>,
        trace: Vec<Vec<u64>>,
        quotient: Vec<F>,
    ) -> Self {
        Self {
            identity,
            statement,
            trace,
            quotient,
        }
    }

    /// Relation identity the producer bound.
    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// Public statement bytes the producer bound.
    #[must_use]
    pub fn statement(&self) -> &[u8] {
        &self.statement
    }

    /// Complete column-major base trace.
    #[must_use]
    pub fn trace(&self) -> &[Vec<u64>] {
        &self.trace
    }

    /// Coefficients of the exact challenge-weighted quotient.
    #[must_use]
    pub fn quotient(&self) -> &[F] {
        &self.quotient
    }

    /// Take the artifact apart again.
    #[must_use]
    pub fn into_parts(self) -> (String, Vec<u8>, Vec<Vec<u64>>, Vec<F>) {
        (self.identity, self.statement, self.trace, self.quotient)
    }
}

/// Declared structural work of one reference run, fixed before allocation.
///
/// These charges describe the uncommitted reference only. They are not a
/// measurement of committed-proof size, prover time or process memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReferencePlan {
    /// Coset rows on which the numerator is reconstructed.
    pub interpolation_rows: usize,
    /// Trace-sized cosets that tile the interpolation domain.
    pub stripes: usize,
    /// Exact coefficient extent of the quotient by `X^N - 1`.
    pub quotient_coefficients: usize,
    /// Declared simultaneous payload bytes.
    pub payload_bytes: usize,
    /// Declared structural field-arithmetic and inspection work.
    pub work_units: usize,
}

impl ReferencePlan {
    /// Derive the complete plan from the relation and check every caller ceiling.
    ///
    /// # Errors
    /// Returns [`AirError::Schema`] for a malformed schema or public-column
    /// list, [`AirError::Shape`] for an empty statement and [`AirError::Limit`]
    /// when a declared quantity exceeds `limits` or the supported transforms.
    pub fn new(air: &impl SemanticAir, limits: WorkLimits) -> Result<Self, AirError> {
        Prepared::new(air, limits).map(|prepared| prepared.plan)
    }
}

/// Validated relation geometry shared by the producer and the checker.
struct Prepared<'a, A: SemanticAir> {
    air: &'a A,
    schema: AirSchema,
    plan: ReferencePlan,
    division: VanishingDivisionPlan,
    trace_generator: u64,
    max_payload_bytes: usize,
}

impl<'a, A: SemanticAir> Prepared<'a, A> {
    fn new(air: &'a A, limits: WorkLimits) -> Result<Self, AirError> {
        let schema = air.schema();
        schema.validate()?;
        let rows = schema.trace_rows;
        if rows < 2 {
            return Err(AirError::Schema("reference needs at least two trace rows"));
        }
        limit(
            "max_trace_rows",
            rows,
            limits.max_trace_rows.min(MAX_REFERENCE_TRACE_ROWS),
        )?;
        limit(
            "max_trace_cells",
            schema.trace_cells(),
            limits.max_trace_cells,
        )?;
        limit(
            "max_constraints",
            schema.constraints,
            limits.max_constraints,
        )?;
        let statement = air.statement_bytes();
        if statement.is_empty() {
            return Err(AirError::Shape("public statement must be nonempty"));
        }
        limit(
            "max_statement_bytes",
            statement.len(),
            limits.max_statement_bytes.min(MAX_STATEMENT_BYTES),
        )?;
        let public = air.public_columns();
        if public.iter().any(|&column| column >= schema.width)
            || public.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(AirError::Schema(
                "public columns must be ascending indices below the width",
            ));
        }
        let degree = schema.numerator_degree_bound;
        let interpolation_rows = degree
            .checked_next_power_of_two()
            .ok_or(AirError::Schema("numerator degree bound overflows"))?
            .max(2 * rows);
        limit(
            "max_reference_interpolation_rows",
            interpolation_rows,
            MAX_REFERENCE_INTERPOLATION_ROWS,
        )?;
        let quotient_coefficients = schema.quotient_degree_bound();
        let trace_generator = FixedTraceDomain::new(&FASTPQ_FINAL_V1, rows)?.generator;
        let division = VanishingDivisionPlan::new(
            rows,
            interpolation_rows,
            degree,
            quotient_coefficients,
            limits.max_payload_bytes,
            limits.max_work_units,
        )
        .map_err(resource_error)?;
        let (payload_bytes, work_units) = charges(
            &schema,
            public.len(),
            interpolation_rows,
            quotient_coefficients,
            division,
        )?;
        limit("max_payload_bytes", payload_bytes, limits.max_payload_bytes)?;
        limit("max_work_units", work_units, limits.max_work_units)?;
        Ok(Self {
            air,
            schema,
            plan: ReferencePlan {
                interpolation_rows,
                stripes: interpolation_rows / rows,
                quotient_coefficients,
                payload_bytes,
                work_units,
            },
            division,
            trace_generator,
            max_payload_bytes: limits.max_payload_bytes,
        })
    }

    fn work(&self) -> ReferenceWork {
        ReferenceWork {
            constraint_evaluations: self.schema.trace_rows + self.plan.interpolation_rows + 1,
            payload_bytes: self.plan.payload_bytes,
            work_units: self.plan.work_units,
        }
    }
}

/// Checked payload and work of one complete reference run.
fn charges(
    schema: &AirSchema,
    public_columns: usize,
    interpolation_rows: usize,
    quotient_coefficients: usize,
    division: VanishingDivisionPlan,
) -> Result<(usize, usize), AirError> {
    let rows = schema.trace_rows;
    let width = schema.width;
    let constraints = schema.constraints;
    let cells = schema.trace_cells();
    let stripes = interpolation_rows / rows;
    // Borrowed base trace; retained column coefficients; one stripe of column
    // lanes; one embedded column and one stripe of numerator values; both
    // transform workspaces; the row buffers and numerators of every fixed job;
    // numerator values and coefficients; the exact division owner; and the
    // returned quotient copy.
    let field_cells = add(
        add(mul(2, cells)?, mul(2, rows)?)?,
        add(
            add(mul(2, rows)?, mul(2, interpolation_rows)?)?,
            add(
                mul(REFERENCE_JOBS, add(mul(2, width)?, constraints)?)?,
                add(mul(2, interpolation_rows)?, quotient_coefficients)?,
            )?,
        )?,
    )?;
    let payload_bytes = add(
        add(mul(cells, size_of::<u64>())?, mul(field_cells, F::BYTES)?)?,
        division.payload_bytes(),
    )?;
    let transforms = add(
        mul(mul(add(stripes, 1)?, width)?, work(rows)?)?,
        work(interpolation_rows)?,
    )?;
    // Canonical inspection and public columns; subgroup rows; coset points;
    // the two Horner rows and the quotient evaluation of the final identity.
    let inspection = add(cells, mul(public_columns, rows)?)?;
    let subgroup = mul(rows, add(constraints, mul(2, width)?)?)?;
    let coset = mul(
        interpolation_rows,
        add(mul(2, constraints)?, mul(2, width)?)?,
    )?;
    let identity = add(mul(2, cells)?, add(constraints, quotient_coefficients)?)?;
    let work_units = add(
        add(transforms, division.work_units())?,
        add(add(inspection, subgroup)?, add(coset, identity)?)?,
    )?;
    Ok((payload_bytes, work_units))
}

/// Build the complete uncommitted reference artifact for `trace`.
///
/// `trace` is column-major: `width` columns of `trace_rows` canonical base
/// values. The producer performs the same checks as [`check_reference`] and
/// returns no artifact for a trace that violates the relation.
///
/// The result is uncommitted, nonsuccinct, non-hiding and not a transaction
/// proof.
///
/// # Errors
/// Returns the first failed admission, shape, public-column, constraint,
/// degree, divisibility or out-of-domain check.
pub fn build_reference<A: SemanticAir>(
    air: &A,
    trace: Vec<Vec<u64>>,
    challenges: &ReferenceChallenges,
    limits: WorkLimits,
    observer: &dyn Observer,
) -> Result<ReferenceArtifact, AirError> {
    let operation = Operation::BuildReference;
    let identity = air.schema().identity;
    let result: Result<(ReferenceWork, Vec<F>), AirError> = (|| {
        let prepared = admit(air, challenges, limits, operation, observer)?;
        let derived = derive_quotient(&prepared, &trace, challenges, operation, observer)?;
        check_identity(&prepared, &derived.columns, &derived.quotient, challenges)?;
        notify(observer, &Event::OodChecked { operation });
        Ok((prepared.work(), derived.quotient))
    })();
    match result {
        Ok((work, quotient)) => {
            notify(
                observer,
                &Event::Completed {
                    operation,
                    identity,
                    work: Work::Reference(work),
                },
            );
            Ok(ReferenceArtifact {
                identity: identity.to_owned(),
                statement: air.statement_bytes().to_vec(),
                trace,
                quotient,
            })
        }
        Err(error) => {
            notify(
                observer,
                &Event::Rejected {
                    operation,
                    identity,
                },
            );
            Err(error)
        }
    }
}

/// Independently check a reference artifact against the expected relation.
///
/// `air` is the checker's own relation, built from its own authenticated
/// public statement; nothing in the artifact selects the relation, its
/// geometry or its limits. Success means the artifact's trace satisfies that
/// relation and that the relation's evaluator is a polynomial map within its
/// declared degree. It is not a committed proof verification.
///
/// # Errors
/// Returns the first failed admission, identity, statement, shape,
/// public-column, constraint, degree, divisibility, quotient or out-of-domain
/// check.
pub fn check_reference<A: SemanticAir>(
    air: &A,
    artifact: &ReferenceArtifact,
    challenges: &ReferenceChallenges,
    limits: WorkLimits,
    observer: &dyn Observer,
) -> Result<ReferenceWork, AirError> {
    let operation = Operation::CheckReference;
    let identity = air.schema().identity;
    let result: Result<ReferenceWork, AirError> = (|| {
        let prepared = admit(air, challenges, limits, operation, observer)?;
        if artifact.identity != prepared.schema.identity {
            return Err(AirError::IdentityMismatch);
        }
        if artifact.statement != air.statement_bytes() {
            return Err(AirError::StatementMismatch);
        }
        if artifact.quotient.len() != prepared.plan.quotient_coefficients {
            return Err(AirError::Shape(
                "quotient must have exactly its declared coefficient extent",
            ));
        }
        for (index, &coefficient) in artifact.quotient.iter().enumerate() {
            coefficient.validate("air_reference_quotient", &[index])?;
        }
        let derived = derive_quotient(&prepared, &artifact.trace, challenges, operation, observer)?;
        if let Some(coefficient) = derived
            .quotient
            .iter()
            .zip(&artifact.quotient)
            .position(|(expected, actual)| expected != actual)
        {
            return Err(AirError::QuotientMismatch { coefficient });
        }
        check_identity(&prepared, &derived.columns, &artifact.quotient, challenges)?;
        notify(observer, &Event::OodChecked { operation });
        Ok(prepared.work())
    })();
    match &result {
        Ok(work) => notify(
            observer,
            &Event::Completed {
                operation,
                identity,
                work: Work::Reference(*work),
            },
        ),
        Err(_) => notify(
            observer,
            &Event::Rejected {
                operation,
                identity,
            },
        ),
    }
    result
}

/// Validate the relation, the challenges and every ceiling before allocating.
fn admit<'a, A: SemanticAir>(
    air: &'a A,
    challenges: &ReferenceChallenges,
    limits: WorkLimits,
    operation: Operation,
    observer: &dyn Observer,
) -> Result<Prepared<'a, A>, AirError> {
    let prepared = Prepared::new(air, limits)?;
    challenges.validate(&prepared.schema)?;
    notify(
        observer,
        &Event::Admitted {
            operation,
            identity: prepared.schema.identity,
            statement_bytes: air.statement_bytes().len(),
            payload_bytes: prepared.plan.payload_bytes,
            work_units: prepared.plan.work_units,
        },
    );
    Ok(prepared)
}

/// Interpolated trace columns and the exact quotient derived from them.
struct Derived {
    columns: Vec<SecretPolynomial<F>>,
    quotient: Vec<F>,
}

/// Check the whole trace and derive the exact challenge-weighted quotient.
fn derive_quotient<A: SemanticAir>(
    prepared: &Prepared<'_, A>,
    trace: &[Vec<u64>],
    challenges: &ReferenceChallenges,
    operation: Operation,
    observer: &dyn Observer,
) -> Result<Derived, AirError> {
    let schema = &prepared.schema;
    check_trace_shape(schema, trace)?;
    check_public_columns(prepared.air, schema, trace)?;
    check_subgroup_rows(prepared, trace)?;
    notify(
        observer,
        &Event::TraceChecked {
            operation,
            rows: schema.trace_rows,
            constraints: schema.constraints,
        },
    );
    let columns = interpolate_columns(prepared, trace)?;
    let numerator = reconstruct_numerator(prepared, &columns, challenges)?;
    if let Some(offset) = numerator[schema.numerator_degree_bound..]
        .iter()
        .position(|&coefficient| coefficient != F::ZERO)
    {
        return Err(AirError::DegreeBoundExceeded {
            coefficient: schema.numerator_degree_bound + offset,
        });
    }
    let quotient = prepared
        .division
        .divide(&numerator)
        .map_err(|_| AirError::NotDivisible)?
        .coefficients()
        .to_vec();
    notify(
        observer,
        &Event::QuotientChecked {
            operation,
            interpolation_rows: prepared.plan.interpolation_rows,
            quotient_coefficients: quotient.len(),
        },
    );
    Ok(Derived { columns, quotient })
}

/// Require exactly `width` columns of `trace_rows` canonical base values.
fn check_trace_shape(schema: &AirSchema, trace: &[Vec<u64>]) -> Result<(), AirError> {
    if trace.len() != schema.width {
        return Err(AirError::Shape(
            "trace must have exactly the declared column count",
        ));
    }
    for (column, values) in trace.iter().enumerate() {
        if values.len() != schema.trace_rows {
            return Err(AirError::Shape(
                "every trace column must have exactly the declared row count",
            ));
        }
        if let Some(row) = values
            .iter()
            .position(|&value| value >= GOLDILOCKS_MODULUS_V1)
        {
            return Err(Error::NonCanonicalGoldilocksElement {
                context: "air_reference_trace",
                indices: vec![column, row],
            }
            .into());
        }
    }
    Ok(())
}

/// Compare every verifier-known cell with the relation's own value.
fn check_public_columns<A: SemanticAir>(
    air: &A,
    schema: &AirSchema,
    trace: &[Vec<u64>],
) -> Result<(), AirError> {
    debug_assert_eq!(trace.len(), schema.width);
    for (ordinal, &column) in air.public_columns().iter().enumerate() {
        for (row, &cell) in trace[column].iter().enumerate() {
            let expected = air.public_value(ordinal, row)?;
            expected.validate("air_reference_public_value", &[column, row])?;
            if cell != expected {
                return Err(AirError::PublicColumnMismatch { column, row });
            }
        }
    }
    Ok(())
}

/// Evaluate every numerator on every cyclic subgroup row in the base field.
///
/// Rows are split into fixed contiguous jobs. Each job stops at its own first
/// violation, and the earliest job's result is reported, so the returned row
/// and slot are the first in row order whatever the thread schedule.
fn check_subgroup_rows<A: SemanticAir>(
    prepared: &Prepared<'_, A>,
    trace: &[Vec<u64>],
) -> Result<(), AirError> {
    let schema = &prepared.schema;
    let rows = schema.trace_rows;
    let chunk = rows.div_ceil(REFERENCE_JOBS);
    let outcomes: Vec<Result<(), AirError>> = (0..rows.div_ceil(chunk))
        .into_par_iter()
        .map(|job| {
            let start = job * chunk;
            let mut current = vec![0_u64; schema.width];
            let mut next = vec![0_u64; schema.width];
            let mut point = field_pow(prepared.trace_generator, start as u64);
            for row in start..rows.min(start + chunk) {
                let successor = (row + 1) % rows;
                for (column, values) in trace.iter().enumerate() {
                    current[column] = values[row];
                    next[column] = values[successor];
                }
                let residues = prepared.air.evaluate(point, &current, &next)?;
                check_count(schema, residues.len())?;
                if let Some(constraint) = residues.iter().position(|&value| value != 0) {
                    return Err(AirError::ConstraintViolated { row, constraint });
                }
                point = mul_base(point, prepared.trace_generator);
            }
            Ok(())
        })
        .collect();
    outcomes.into_iter().collect()
}

/// Interpolate every base column over the trace subgroup.
fn interpolate_columns<A: SemanticAir>(
    prepared: &Prepared<'_, A>,
    trace: &[Vec<u64>],
) -> Result<Vec<SecretPolynomial<F>>, AirError> {
    let rows = prepared.schema.trace_rows;
    let subgroup = PolynomialDomain::new(rows, F::ONE, rows, prepared.max_payload_bytes)
        .map_err(resource_error)?;
    if subgroup.generator() != prepared.trace_generator {
        return Err(AirError::Schema(
            "trace subgroup generator differs from the canonical trace root",
        ));
    }
    let mut embedded = vec![F::ZERO; rows];
    let mut columns = Vec::with_capacity(trace.len());
    for values in trace {
        for (target, &value) in embedded.iter_mut().zip(values) {
            *target = F::embed_base(value);
        }
        columns.push(subgroup.interpolate(&embedded)?);
    }
    Ok(columns)
}

/// Evaluate the weighted numerator on a disjoint coset and interpolate it.
///
/// The interpolation domain is tiled by trace-sized cosets, so only one
/// stripe of column evaluations is alive at a time. Each stripe is evaluated
/// by fixed contiguous jobs; the result does not depend on the thread pool.
fn reconstruct_numerator<A: SemanticAir>(
    prepared: &Prepared<'_, A>,
    columns: &[SecretPolynomial<F>],
    challenges: &ReferenceChallenges,
) -> Result<SecretPolynomial<F>, AirError> {
    let schema = &prepared.schema;
    let rows = schema.trace_rows;
    let interpolation_rows = prepared.plan.interpolation_rows;
    let offset = F::embed_base(FASTPQ_FINAL_V1.omega_coset);
    let domain = PolynomialDomain::new(
        interpolation_rows,
        offset,
        interpolation_rows,
        prepared.max_payload_bytes,
    )
    .map_err(resource_error)?;
    let stripes = domain.numerator_rotation(rows)?;
    let mut values = SecretPolynomial::<F>::zeroed(interpolation_rows)?;
    let mut stripe_values = SecretPolynomial::<F>::zeroed(rows)?;
    let chunk = rows.div_ceil(REFERENCE_JOBS);
    for stripe in 0..stripes {
        let stripe_offset = offset.mul_base(field_pow(domain.generator(), stripe as u64));
        let coset = PolynomialDomain::new(rows, stripe_offset, rows, prepared.max_payload_bytes)
            .map_err(resource_error)?;
        // A stripe is one coset of the trace subgroup, so the next row of its
        // point `i` is its point `i + 1`, cyclically.
        if coset.numerator_rotation(rows)? != 1 {
            return Err(AirError::Schema(
                "numerator stripe is not a coset of the trace subgroup",
            ));
        }
        let mut lanes = Vec::with_capacity(columns.len());
        for column in columns {
            lanes.push(coset.evaluate(column, rows)?);
        }
        // Fixed contiguous jobs write disjoint output slices; the earliest
        // job's error is reported, independent of the thread schedule.
        let outcomes: Vec<Result<(), AirError>> = stripe_values
            .par_chunks_mut(chunk)
            .enumerate()
            .map(|(job, output)| {
                let start = job * chunk;
                let mut current = vec![F::ZERO; schema.width];
                let mut next = vec![F::ZERO; schema.width];
                let mut point =
                    stripe_offset.mul_base(field_pow(prepared.trace_generator, start as u64));
                for (index, slot) in (start..).zip(output.iter_mut()) {
                    let successor = (index + 1) % rows;
                    for (column, lane) in lanes.iter().enumerate() {
                        current[column] = lane.value(index)?;
                        next[column] = lane.value(successor)?;
                    }
                    let residues = prepared.air.evaluate(point, &current, &next)?;
                    check_count(schema, residues.len())?;
                    *slot = weighted(&residues, challenges.alphas());
                    point = point.mul_base(prepared.trace_generator);
                }
                Ok(())
            })
            .collect();
        outcomes.into_iter().collect::<Result<(), AirError>>()?;
        for (index, &value) in stripe_values.iter().enumerate() {
            values[stripe + index * stripes] = value;
        }
    }
    Ok(domain.interpolate(&values)?)
}

/// Check `sum_k alpha_k C_k(z, A(z), A(omega z)) = (z^N - 1) Q(z)`.
fn check_identity<A: SemanticAir>(
    prepared: &Prepared<'_, A>,
    columns: &[SecretPolynomial<F>],
    quotient: &[F],
    challenges: &ReferenceChallenges,
) -> Result<(), AirError> {
    let schema = &prepared.schema;
    let point = challenges.ood_point();
    let rotated = point.mul_base(prepared.trace_generator);
    let current: Vec<F> = columns.iter().map(|column| horner(column, point)).collect();
    let next: Vec<F> = columns
        .iter()
        .map(|column| horner(column, rotated))
        .collect();
    let residues = prepared.air.evaluate(point, &current, &next)?;
    check_count(schema, residues.len())?;
    let numerator = weighted(&residues, challenges.alphas());
    let vanishing = point.power(schema.trace_rows as u64).sub(F::ONE);
    if numerator != vanishing.mul(horner(quotient, point)) {
        return Err(AirError::OodIdentity);
    }
    Ok(())
}

/// Challenge-weighted sum of one complete numerator vector.
fn weighted(residues: &[F], alphas: &[F]) -> F {
    residues
        .iter()
        .zip(alphas)
        .fold(F::ZERO, |sum, (&residue, &alpha)| {
            sum.add(residue.mul(alpha))
        })
}

/// Evaluate ascending coefficients at an extension point.
fn horner(coefficients: &[F], point: F) -> F {
    coefficients
        .iter()
        .rev()
        .fold(F::ZERO, |sum, &coefficient| sum.mul(point).add(coefficient))
}

fn check_count(schema: &AirSchema, actual: usize) -> Result<(), AirError> {
    if actual != schema.constraints {
        return Err(AirError::ConstraintCount {
            expected: schema.constraints,
            actual,
        });
    }
    Ok(())
}

fn limit(name: &'static str, actual: usize, max: usize) -> Result<(), AirError> {
    if actual > max {
        return Err(AirError::Limit {
            limit: name,
            actual,
            max,
        });
    }
    Ok(())
}

/// Report an arithmetic owner's ceiling as the interface's typed limit.
fn resource_error(error: Error) -> AirError {
    match error {
        Error::VerifierLimitExceeded { limit, actual, max } => {
            AirError::Limit { limit, actual, max }
        }
        other => AirError::Engine(other),
    }
}

fn add(left: usize, right: usize) -> Result<usize, AirError> {
    Ok(checked_add(left, right)?)
}

fn mul(left: usize, right: usize) -> Result<usize, AirError> {
    Ok(checked_mul(left, right)?)
}

fn work(rows: usize) -> Result<usize, AirError> {
    Ok(transform_work(rows)?)
}
