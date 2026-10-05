//! A small new relation expressed only through the typed interface, with honest
//! reference construction/checking and constraint-mutation controls.

use std::sync::Mutex;

use super::*;
use crate::{
    Result,
    field::{GOLDILOCKS_MODULUS_V1, GoldilocksFp4V1 as F},
};

const A: usize = 0;
const B: usize = 1;
const C: usize = 2;
const FIRST: usize = 3;
const LAST: usize = 4;
const WIDTH: usize = 5;
const CONSTRAINTS: usize = 7;
const IDENTITY: &str = "fastpq:air:test:linked-sequences:v1:5cols:7slots";

/// One deliberate change to a single equation of the test relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mutation {
    None,
    /// Slot 0: `a' = b + 1` instead of `a' = b`.
    ShiftedCopy,
    /// Slot 1: `b' = a - b` instead of `b' = a + b`.
    SubtractInsteadOfAdd,
    /// Slot 2: `c' = c^2` instead of `c' = c^2 + a`.
    DropLinearTerm,
    /// Slot 2: the wraparound gate `(1 - last)` is removed.
    DropWraparoundGate,
    /// Slot 5: the public result is compared with `a` instead of `b`.
    ResultFromWrongColumn,
    /// Slot 6: the point selector binds `c0 + 1` instead of `c0`.
    ShiftedPointBoundary,
    /// Slot 2 gains an extra term only in the quartic extension.
    ExtensionOnlyTerm,
    /// Slot 2 becomes cubic in `c` while the declared degree stays quadratic.
    UndeclaredCubic,
}

/// Three linked sequences with public boundary values.
///
/// `a' = b`, `b' = a + b` and `c' = c^2 + a` on every row but the last. The
/// first row is bound to `(a0, b0)` through the public `first` column and to
/// `c0` through the Lagrange selector of the evaluation point; the last row's
/// `b` is bound to the public result through the public `last` column.
struct LinkedSequences {
    rows: usize,
    a0: u64,
    b0: u64,
    c0: u64,
    result: u64,
    statement: Vec<u8>,
    mutation: Mutation,
    declared_degree: usize,
}

impl LinkedSequences {
    fn new(rows: usize, a0: u64, b0: u64, c0: u64, result: u64) -> Self {
        let mut statement = Vec::with_capacity(40);
        statement.extend_from_slice(&(rows as u64).to_le_bytes());
        for value in [a0, b0, c0, result] {
            statement.extend_from_slice(&value.to_le_bytes());
        }
        Self {
            rows,
            a0,
            b0,
            c0,
            result,
            statement,
            mutation: Mutation::None,
            // (1 - last) has degree < N and c^2 has degree < 2N - 1.
            declared_degree: 3 * rows - 2,
        }
    }

    fn honest(rows: usize, a0: u64, b0: u64, c0: u64) -> (Self, Vec<Vec<u64>>) {
        let trace = sequence_trace(rows, a0, b0, c0, Mutation::None);
        let result = trace[B][rows - 1];
        (Self::new(rows, a0, b0, c0, result), trace)
    }

    fn mutated(mut self, mutation: Mutation) -> Self {
        self.mutation = mutation;
        self
    }
}

/// Execute the (possibly mutated) program honestly.
fn sequence_trace(rows: usize, a0: u64, b0: u64, c0: u64, mutation: Mutation) -> Vec<Vec<u64>> {
    program_trace(rows, (a0, b0, c0), |a, b, c| {
        let next_a = match mutation {
            Mutation::ShiftedCopy => b.add(1),
            _ => b,
        };
        let next_b = match mutation {
            Mutation::SubtractInsteadOfAdd => a.sub(b),
            _ => a.add(b),
        };
        let next_c = match mutation {
            Mutation::DropLinearTerm => c.mul(c),
            Mutation::UndeclaredCubic => c.mul(c).mul(c).add(a),
            _ => c.mul(c).add(a),
        };
        (next_a, next_b, next_c)
    })
}

/// Column-major trace of one three-register program with its public selectors.
fn program_trace(
    rows: usize,
    initial: (u64, u64, u64),
    step: impl Fn(u64, u64, u64) -> (u64, u64, u64),
) -> Vec<Vec<u64>> {
    let mut registers: [Vec<u64>; 3] = core::array::from_fn(|_| Vec::with_capacity(rows));
    let (mut a, mut b, mut c) = initial;
    for _ in 0..rows {
        registers[A].push(a);
        registers[B].push(b);
        registers[C].push(c);
        (a, b, c) = step(a, b, c);
    }
    let mut first = vec![0_u64; rows];
    first[0] = 1;
    let mut last = vec![0_u64; rows];
    last[rows - 1] = 1;
    let [a_column, b_column, c_column] = registers;
    vec![a_column, b_column, c_column, first, last]
}

/// `L_0(x) = (x^N - 1) / (N (x - 1))`, with its removable singularity at one.
fn first_row_selector<F: PolynomialField>(point: F, rows: usize) -> Result<F> {
    if point == F::ONE {
        return Ok(F::ONE);
    }
    let numerator = point.power(rows as u64).sub(F::ONE);
    point
        .sub(F::ONE)
        .scale_base(rows as u64)
        .inverse()
        .map(|inverse| numerator.mul(inverse))
        .ok_or_else(|| crate::Error::InvalidTraceShape {
            details: "test selector denominator has no inverse".to_owned(),
        })
}

impl SemanticAir for LinkedSequences {
    fn schema(&self) -> AirSchema {
        AirSchema {
            identity: IDENTITY,
            trace_rows: self.rows,
            width: WIDTH,
            constraints: CONSTRAINTS,
            numerator_degree_bound: self.declared_degree,
        }
    }

    fn statement_bytes(&self) -> &[u8] {
        &self.statement
    }

    fn public_columns(&self) -> &[usize] {
        &[FIRST, LAST]
    }

    fn public_value(&self, ordinal: usize, row: usize) -> Result<u64> {
        if row >= self.rows {
            return Err(crate::Error::QueryIndexOutOfRange {
                index: row,
                len: self.rows,
            });
        }
        match ordinal {
            0 => Ok(u64::from(row == 0)),
            1 => Ok(u64::from(row == self.rows - 1)),
            _ => Err(crate::Error::QueryIndexOutOfRange {
                index: ordinal,
                len: 2,
            }),
        }
    }

    fn evaluate<F: PolynomialField>(&self, point: F, current: &[F], next: &[F]) -> Result<Vec<F>> {
        if current.len() != WIDTH || next.len() != WIDTH {
            return Err(crate::Error::InvalidTraceShape {
                details: "linked-sequence rows need exactly five cells".to_owned(),
            });
        }
        point.validate("linked_sequence_point", &[])?;
        for (column, (&cell, &successor)) in current.iter().zip(next).enumerate() {
            cell.validate("linked_sequence_current", &[column])?;
            successor.validate("linked_sequence_next", &[column])?;
        }
        let (a, b, c) = (current[A], current[B], current[C]);
        let (first, last) = (current[FIRST], current[LAST]);
        let active = F::ONE.sub(last);
        let mutation = self.mutation;
        let copy_target = if mutation == Mutation::ShiftedCopy {
            b.add(F::ONE)
        } else {
            b
        };
        let sum_target = if mutation == Mutation::SubtractInsteadOfAdd {
            a.sub(b)
        } else {
            a.add(b)
        };
        let square = c.mul(c);
        let mut power_target = match mutation {
            Mutation::DropLinearTerm => square,
            Mutation::UndeclaredCubic => square.mul(c).add(a),
            _ => square.add(a),
        };
        if mutation == Mutation::ExtensionOnlyTerm && F::COEFFICIENTS == 4 {
            power_target = power_target.add(F::ONE);
        }
        let power_gate = if mutation == Mutation::DropWraparoundGate {
            F::ONE
        } else {
            active
        };
        let result_cell = if mutation == Mutation::ResultFromWrongColumn {
            a
        } else {
            b
        };
        let point_boundary = if mutation == Mutation::ShiftedPointBoundary {
            self.c0.add(1)
        } else {
            self.c0
        };
        Ok(vec![
            active.mul(next[A].sub(copy_target)),
            active.mul(next[B].sub(sum_target)),
            power_gate.mul(next[C].sub(power_target)),
            first.mul(a.sub(F::embed_base(self.a0))),
            first.mul(b.sub(F::embed_base(self.b0))),
            last.mul(result_cell.sub(F::embed_base(self.result))),
            first_row_selector(point, self.rows)?.mul(c.sub(F::embed_base(point_boundary))),
        ])
    }
}

/// A relation with caller-chosen schema, statement and public columns.
struct Malformed {
    schema: AirSchema,
    statement: Vec<u8>,
    public: Vec<usize>,
}
impl SemanticAir for Malformed {
    fn schema(&self) -> AirSchema {
        self.schema
    }
    fn statement_bytes(&self) -> &[u8] {
        &self.statement
    }
    fn public_columns(&self) -> &[usize] {
        &self.public
    }
    fn public_value(&self, _: usize, _: usize) -> Result<u64> {
        Ok(0)
    }
    fn evaluate<F: PolynomialField>(&self, _: F, _: &[F], _: &[F]) -> Result<Vec<F>> {
        Ok(vec![F::ZERO; self.schema.constraints])
    }
}
/// Returns two numerators whatever it declares.
struct WrongCount(Malformed);
impl SemanticAir for WrongCount {
    fn schema(&self) -> AirSchema {
        self.0.schema
    }
    fn statement_bytes(&self) -> &[u8] {
        &self.0.statement
    }
    fn public_columns(&self) -> &[usize] {
        &[]
    }
    fn public_value(&self, _: usize, _: usize) -> Result<u64> {
        Ok(0)
    }
    fn evaluate<F: PolynomialField>(&self, _: F, _: &[F], _: &[F]) -> Result<Vec<F>> {
        Ok(vec![F::ZERO; 2])
    }
}
/// Observer whose every notification panics.
struct Panicking;
impl Observer for Panicking {
    fn observe(&self, _: &Event<'_>) {
        panic!("observer failure must not affect validity");
    }
}

fn challenges() -> ReferenceChallenges {
    ReferenceChallenges::derive(&[0x5a; 32], CONSTRAINTS)
}

fn build(
    air: &LinkedSequences,
    trace: Vec<Vec<u64>>,
) -> core::result::Result<ReferenceArtifact, AirError> {
    build_reference(
        air,
        trace,
        &challenges(),
        WorkLimits::default(),
        &NoObserver,
    )
}

fn check(
    air: &LinkedSequences,
    artifact: &ReferenceArtifact,
) -> core::result::Result<ReferenceWork, AirError> {
    check_reference(
        air,
        artifact,
        &challenges(),
        WorkLimits::default(),
        &NoObserver,
    )
}

fn honest_artifact(rows: usize) -> (LinkedSequences, ReferenceArtifact) {
    let (air, trace) = LinkedSequences::honest(rows, 3, 5, 7);
    let artifact = build(&air, trace).expect("honest reference");
    (air, artifact)
}

#[derive(Default)]
struct Recorder(Mutex<Vec<String>>);

impl Recorder {
    fn labels(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

impl Observer for Recorder {
    fn observe(&self, event: &Event<'_>) {
        let label = match *event {
            Event::Admitted { operation, .. } => format!("{operation:?}:admitted"),
            Event::TraceChecked { operation, .. } => format!("{operation:?}:trace"),
            Event::QuotientChecked { operation, .. } => format!("{operation:?}:quotient"),
            Event::OodChecked { operation } => format!("{operation:?}:ood"),
            Event::Completed { operation, .. } => format!("{operation:?}:completed"),
            Event::Rejected { operation, .. } => format!("{operation:?}:rejected"),
        };
        self.0.lock().unwrap().push(label);
    }
}

#[test]
fn honest_reference_builds_and_is_independently_checked() {
    for rows in [2, 4, 16, 64, 256] {
        let (air, trace) = LinkedSequences::honest(rows, 3, 5, 7);
        let plan = ReferencePlan::new(&air, WorkLimits::default()).unwrap();
        assert_eq!(
            plan.interpolation_rows,
            (3 * rows - 2).next_power_of_two().max(2 * rows)
        );
        assert_eq!(plan.stripes, plan.interpolation_rows / rows);
        assert_eq!(plan.quotient_coefficients, 2 * rows - 2);
        let artifact = build(&air, trace.clone()).unwrap();
        assert_eq!(artifact.identity(), IDENTITY);
        assert_eq!(artifact.statement(), air.statement_bytes());
        assert_eq!(artifact.trace(), trace.as_slice());
        assert_eq!(artifact.quotient().len(), plan.quotient_coefficients);
        // A nontrivial quotient: the relation is not satisfied identically.
        assert!(artifact.quotient().iter().any(|&value| value != F::ZERO));
        let work = check(&air, &artifact).unwrap();
        assert_eq!(
            work,
            ReferenceWork {
                constraint_evaluations: rows + plan.interpolation_rows + 1,
                payload_bytes: plan.payload_bytes,
                work_units: plan.work_units,
            }
        );
        // An independent checker instance built from its own public statement.
        let checker = LinkedSequences::new(rows, 3, 5, 7, trace[B][rows - 1]);
        assert_eq!(check(&checker, &artifact).unwrap(), work);
    }
}

#[test]
fn reference_construction_is_deterministic_and_bound_to_its_challenges() {
    let (air, trace) = LinkedSequences::honest(32, 11, 13, 17);
    let first = build(&air, trace.clone()).unwrap();
    let second = build(&air, trace.clone()).unwrap();
    assert_eq!(first, second);
    let other = ReferenceChallenges::derive(&[0x5b; 32], CONSTRAINTS);
    let third = build_reference(&air, trace, &other, WorkLimits::default(), &NoObserver).unwrap();
    assert_ne!(first.quotient(), third.quotient());
    // The checker must use the producer's weights: another set is a mismatch.
    assert!(matches!(
        check(&air, &third),
        Err(AirError::QuotientMismatch { .. })
    ));
    assert!(check_reference(&air, &third, &other, WorkLimits::default(), &NoObserver).is_ok());
}

#[test]
fn derived_challenges_are_canonical_distinct_and_outside_the_base_field() {
    let first = ReferenceChallenges::derive(&[1; 32], 64);
    let again = ReferenceChallenges::derive(&[1; 32], 64);
    let other = ReferenceChallenges::derive(&[2; 32], 64);
    assert_eq!(first, again);
    assert_ne!(first, other);
    assert_eq!(first.alphas().len(), 64);
    for alpha in first.alphas() {
        assert!(
            alpha
                .coefficients()
                .iter()
                .all(|&word| word < GOLDILOCKS_MODULUS_V1)
        );
    }
    assert!(
        first.ood_point().coefficients()[1..]
            .iter()
            .any(|&word| word != 0)
    );
    // The debug form never prints challenge values.
    assert_eq!(
        format!("{first:?}"),
        "ReferenceChallenges { alphas: 64, .. }"
    );
}

#[test]
fn violated_trace_cells_are_rejected_at_their_exact_row_and_slot() {
    let rows = 16;
    let (air, trace) = LinkedSequences::honest(rows, 3, 5, 7);
    let artifact = build(&air, trace.clone()).unwrap();
    // (column, row, first violated row, first violated slot)
    for (column, row, bad_row, bad_slot) in [
        (A, 5, 4, 0),
        (B, 9, 8, 1),
        (C, 12, 11, 2),
        (A, 0, 0, 1),
        (B, 0, 0, 0),
        (C, 0, 0, 2),
        (B, rows - 1, rows - 2, 1),
    ] {
        let mut corrupted = trace.clone();
        corrupted[column][row] = corrupted[column][row].add(1);
        assert!(
            matches!(
                build(&air, corrupted.clone()),
                Err(AirError::ConstraintViolated { row, constraint })
                    if row == bad_row && constraint == bad_slot
            ),
            "producer column={column} row={row}"
        );
        // A forged artifact that keeps the honest quotient is rejected by the
        // checker's own row evaluation, not by a producer self-check.
        let forged = ReferenceArtifact::from_parts(
            IDENTITY.to_owned(),
            air.statement_bytes().to_vec(),
            corrupted,
            artifact.quotient().to_vec(),
        );
        assert!(
            matches!(
                check(&air, &forged),
                Err(AirError::ConstraintViolated { row, constraint })
                    if row == bad_row && constraint == bad_slot
            ),
            "checker column={column} row={row}"
        );
    }
}

#[test]
fn point_selector_boundary_is_enforced_by_evaluation() {
    // Changing only c0 keeps slots 0..=5 satisfied on row zero's predecessors
    // and is caught by the selector polynomial of the evaluation point.
    let rows = 16;
    let (air, _) = LinkedSequences::honest(rows, 3, 5, 7);
    let other = sequence_trace(rows, 3, 5, 8, Mutation::None);
    assert!(matches!(
        build(&air, other),
        Err(AirError::ConstraintViolated {
            row: 0,
            constraint: 6
        })
    ));
}

#[test]
fn altered_public_io_is_rejected_by_binding_and_by_evaluation() {
    let rows = 16;
    let (air, artifact) = honest_artifact(rows);
    let result = artifact.trace()[B][rows - 1];
    // Another public result: first the statement binding differs.
    let other = LinkedSequences::new(rows, 3, 5, 7, result.add(1));
    assert!(matches!(
        check(&other, &artifact),
        Err(AirError::StatementMismatch)
    ));
    // Relabelling the artifact with the other statement passes the binding and
    // is then rejected by the last-row public-result equation itself.
    let (identity, _, trace, quotient) = artifact.clone().into_parts();
    let relabelled =
        ReferenceArtifact::from_parts(identity, other.statement_bytes().to_vec(), trace, quotient);
    assert!(matches!(
        check(&other, &relabelled),
        Err(AirError::ConstraintViolated { row, constraint: 5 }) if row == rows - 1
    ));
    // Another public initial value is caught on the first row.
    let other = LinkedSequences::new(rows, 4, 5, 7, result);
    let (identity, _, trace, quotient) = artifact.clone().into_parts();
    let relabelled =
        ReferenceArtifact::from_parts(identity, other.statement_bytes().to_vec(), trace, quotient);
    assert!(matches!(
        check(&other, &relabelled),
        Err(AirError::ConstraintViolated {
            row: 0,
            constraint: 3
        })
    ));
    // Another relation identity is refused before any arithmetic.
    let (_, statement, trace, quotient) = artifact.clone().into_parts();
    let renamed =
        ReferenceArtifact::from_parts("another-relation".to_owned(), statement, trace, quotient);
    assert!(matches!(
        check(&air, &renamed),
        Err(AirError::IdentityMismatch)
    ));
    // Verifier-known columns come from the relation, never from the artifact.
    for (column, row) in [(FIRST, 0), (FIRST, 3), (LAST, rows - 1), (LAST, 0)] {
        let (identity, statement, mut trace, quotient) = artifact.clone().into_parts();
        trace[column][row] ^= 1;
        let forged = ReferenceArtifact::from_parts(identity, statement, trace, quotient);
        assert!(matches!(
            check(&air, &forged),
            Err(AirError::PublicColumnMismatch { column: c, row: r }) if c == column && r == row
        ));
    }
}

#[test]
fn quotient_corruption_and_malformed_extents_are_rejected() {
    let (air, artifact) = honest_artifact(16);
    let extent = artifact.quotient().len();
    for coefficient in [0, 1, extent / 2, extent - 1] {
        let (identity, statement, trace, mut quotient) = artifact.clone().into_parts();
        quotient[coefficient] = quotient[coefficient].add(F::ONE);
        let forged = ReferenceArtifact::from_parts(identity, statement, trace, quotient);
        assert!(matches!(
            check(&air, &forged),
            Err(AirError::QuotientMismatch { coefficient: found }) if found == coefficient
        ));
    }
    for length in [0, extent - 1, extent + 1] {
        let (identity, statement, trace, mut quotient) = artifact.clone().into_parts();
        quotient.resize(length, F::ZERO);
        let forged = ReferenceArtifact::from_parts(identity, statement, trace, quotient);
        assert!(matches!(check(&air, &forged), Err(AirError::Shape(_))));
    }
    let (identity, statement, trace, mut quotient) = artifact.clone().into_parts();
    quotient[3] = F::from_coefficients_unchecked_for_test([0, GOLDILOCKS_MODULUS_V1, 0, 0]);
    let forged = ReferenceArtifact::from_parts(identity, statement, trace, quotient);
    assert!(matches!(
        check(&air, &forged),
        Err(AirError::Engine(
            crate::Error::NonCanonicalGoldilocksElement {
                context: "air_reference_quotient",
                ..
            }
        ))
    ));
}

#[test]
fn malformed_trace_shapes_and_cells_are_rejected_before_arithmetic() {
    let rows = 16;
    let (air, trace) = LinkedSequences::honest(rows, 3, 5, 7);
    let mut missing = trace.clone();
    missing.pop();
    assert!(matches!(build(&air, missing), Err(AirError::Shape(_))));
    let mut extra = trace.clone();
    extra.push(vec![0; rows]);
    assert!(matches!(build(&air, extra), Err(AirError::Shape(_))));
    for length in [0, rows - 1, rows + 1] {
        let mut ragged = trace.clone();
        ragged[B].resize(length, 0);
        assert!(matches!(build(&air, ragged), Err(AirError::Shape(_))));
    }
    let mut noncanonical = trace.clone();
    noncanonical[C][7] = GOLDILOCKS_MODULUS_V1;
    assert!(matches!(
        build(&air, noncanonical),
        Err(AirError::Engine(crate::Error::NonCanonicalGoldilocksElement {
            context: "air_reference_trace",
            indices,
        })) if indices == [C, 7]
    ));
}

#[test]
fn malformed_challenges_schemas_and_statements_are_rejected() {
    let rows = 16;
    let (air, trace) = LinkedSequences::honest(rows, 3, 5, 7);
    let honest = challenges();
    let run = |challenges: &ReferenceChallenges| {
        build_reference(
            &air,
            trace.clone(),
            challenges,
            WorkLimits::default(),
            &NoObserver,
        )
    };
    let mut short = honest.alphas().to_vec();
    short.pop();
    assert!(matches!(
        run(&ReferenceChallenges::new(short, honest.ood_point())),
        Err(AirError::Shape(_))
    ));
    let mut bad = honest.alphas().to_vec();
    bad[2] = F::from_coefficients_unchecked_for_test([0, 0, GOLDILOCKS_MODULUS_V1, 0]);
    assert!(matches!(
        run(&ReferenceChallenges::new(bad, honest.ood_point())),
        Err(AirError::Engine(
            crate::Error::NonCanonicalGoldilocksElement {
                context: "air_reference_alpha",
                ..
            }
        ))
    ));
    // A base-field point could lie on the trace subgroup; it is never accepted.
    assert!(matches!(
        run(&ReferenceChallenges::new(
            honest.alphas().to_vec(),
            F::embed_base(9)
        )),
        Err(AirError::Shape(_))
    ));

    let valid = AirSchema {
        identity: "fastpq:air:test:malformed:v1",
        trace_rows: 8,
        width: 2,
        constraints: 1,
        numerator_degree_bound: 8,
    };
    let plan = |schema: AirSchema, statement: &[u8], public: &[usize]| {
        ReferencePlan::new(
            &Malformed {
                schema,
                statement: statement.to_vec(),
                public: public.to_vec(),
            },
            WorkLimits::default(),
        )
    };
    assert!(plan(valid, b"s", &[1]).is_ok());
    for schema in [
        AirSchema {
            identity: "",
            ..valid
        },
        AirSchema {
            trace_rows: 12,
            ..valid
        },
        AirSchema {
            trace_rows: 0,
            ..valid
        },
        AirSchema {
            trace_rows: 1,
            ..valid
        },
        AirSchema { width: 0, ..valid },
        AirSchema {
            constraints: 0,
            ..valid
        },
        AirSchema {
            numerator_degree_bound: 0,
            ..valid
        },
    ] {
        assert!(
            matches!(plan(schema, b"s", &[]), Err(AirError::Schema(_))),
            "{schema:?}"
        );
    }
    let long: &'static str =
        Box::leak("i".repeat(MAX_RELATION_IDENTITY_BYTES + 1).into_boxed_str());
    assert!(matches!(
        plan(
            AirSchema {
                identity: long,
                ..valid
            },
            b"s",
            &[]
        ),
        Err(AirError::Schema(_))
    ));
    assert!(matches!(plan(valid, b"", &[]), Err(AirError::Shape(_))));
    for public in [&[2_usize][..], &[1, 1], &[1, 0]] {
        assert!(matches!(
            plan(valid, b"s", public),
            Err(AirError::Schema(_))
        ));
    }
    // A relation that returns another slot count than it declares is refused.
    let wrong = WrongCount(Malformed {
        schema: valid,
        statement: b"s".to_vec(),
        public: Vec::new(),
    });
    assert!(matches!(
        build_reference(
            &wrong,
            vec![vec![0; 8]; 2],
            &ReferenceChallenges::derive(&[7; 32], 1),
            WorkLimits::default(),
            &NoObserver,
        ),
        Err(AirError::ConstraintCount {
            expected: 1,
            actual: 2
        })
    ));
}

#[test]
fn every_declared_work_limit_is_enforced_before_allocation() {
    let rows = 64;
    let (air, trace) = LinkedSequences::honest(rows, 3, 5, 7);
    let generous = WorkLimits::default();
    let plan = ReferencePlan::new(&air, generous).unwrap();
    let exact = WorkLimits {
        max_trace_rows: rows,
        max_trace_cells: rows * WIDTH,
        max_constraints: CONSTRAINTS,
        max_statement_bytes: air.statement_bytes().len(),
        max_payload_bytes: plan.payload_bytes,
        max_work_units: plan.work_units,
    };
    assert_eq!(ReferencePlan::new(&air, exact).unwrap(), plan);
    let artifact = build_reference(&air, trace.clone(), &challenges(), exact, &NoObserver).unwrap();
    for (name, limits) in [
        (
            "max_trace_rows",
            WorkLimits {
                max_trace_rows: rows - 1,
                ..exact
            },
        ),
        (
            "max_trace_cells",
            WorkLimits {
                max_trace_cells: rows * WIDTH - 1,
                ..exact
            },
        ),
        (
            "max_constraints",
            WorkLimits {
                max_constraints: CONSTRAINTS - 1,
                ..exact
            },
        ),
        (
            "max_statement_bytes",
            WorkLimits {
                max_statement_bytes: air.statement_bytes().len() - 1,
                ..exact
            },
        ),
        (
            "max_payload_bytes",
            WorkLimits {
                max_payload_bytes: plan.payload_bytes - 1,
                ..exact
            },
        ),
        (
            "max_work_units",
            WorkLimits {
                max_work_units: plan.work_units - 1,
                ..exact
            },
        ),
    ] {
        let recorder = Recorder::default();
        assert!(
            matches!(
                build_reference(&air, trace.clone(), &challenges(), limits, &recorder),
                Err(AirError::Limit { limit, actual, max }) if limit == name && actual > max
            ),
            "{name}"
        );
        // Refused before admission: no work was started.
        assert_eq!(recorder.labels(), ["BuildReference:rejected"]);
        assert!(matches!(
            check_reference(&air, &artifact, &challenges(), limits, &NoObserver),
            Err(AirError::Limit { limit, .. }) if limit == name
        ));
    }
    // Declared work grows with the trace; it is never read from an artifact.
    let (larger, _) = LinkedSequences::honest(2 * rows, 3, 5, 7);
    let larger = ReferencePlan::new(&larger, generous).unwrap();
    assert!(larger.payload_bytes > plan.payload_bytes && larger.work_units > plan.work_units);
    // The supported transforms bound the trace and the numerator domain.
    let (huge, _) = LinkedSequences::honest(8, 3, 5, 7);
    let huge = LinkedSequences {
        declared_degree: MAX_REFERENCE_INTERPOLATION_ROWS + 1,
        ..huge
    };
    assert!(matches!(
        ReferencePlan::new(&huge, generous),
        Err(AirError::Limit {
            limit: "max_reference_interpolation_rows",
            ..
        })
    ));
}

#[test]
fn constraint_mutations_are_caught_by_evaluation_in_both_directions() {
    let rows = 32;
    let (air, trace) = LinkedSequences::honest(rows, 3, 5, 7);
    let artifact = build(&air, trace).unwrap();
    // (mutation, first violated row and slot of the honest trace under it)
    for (mutation, bad_row, bad_slot) in [
        (Mutation::ShiftedCopy, 0, 0),
        (Mutation::SubtractInsteadOfAdd, 0, 1),
        (Mutation::DropLinearTerm, 0, 2),
        (Mutation::DropWraparoundGate, rows - 1, 2),
        (Mutation::ResultFromWrongColumn, rows - 1, 5),
        (Mutation::ShiftedPointBoundary, 0, 6),
    ] {
        // Same identity and statement: only the evaluated equation differs.
        let mutated = LinkedSequences::new(rows, 3, 5, 7, air.result).mutated(mutation);
        assert_eq!(mutated.statement_bytes(), air.statement_bytes());
        assert!(
            matches!(
                check(&mutated, &artifact),
                Err(AirError::ConstraintViolated { row, constraint })
                    if row == bad_row && constraint == bad_slot
            ),
            "{mutation:?}"
        );
    }
    // A trace that is honest for a mutated program is accepted by exactly that
    // relation and rejected by the original one.
    for mutation in [
        Mutation::ShiftedCopy,
        Mutation::SubtractInsteadOfAdd,
        Mutation::DropLinearTerm,
    ] {
        let trace = sequence_trace(rows, 3, 5, 7, mutation);
        let result = trace[B][rows - 1];
        let mutated = LinkedSequences::new(rows, 3, 5, 7, result).mutated(mutation);
        let artifact = build(&mutated, trace).unwrap();
        check(&mutated, &artifact).unwrap();
        let original = LinkedSequences::new(rows, 3, 5, 7, result);
        assert!(
            matches!(
                check(&original, &artifact),
                Err(AirError::ConstraintViolated { .. })
            ),
            "{mutation:?}"
        );
    }
}

#[test]
fn base_and_extension_evaluations_agree_on_embedded_inputs() {
    let rows = 16;
    let (air, trace) = LinkedSequences::honest(rows, 3, 5, 7);
    let generator = {
        // The trace generator is the engine's canonical root for this order.
        use crate::backend::fixed_domain::FixedTraceDomain;
        FixedTraceDomain::new(&fastpq_isi::FASTPQ_FINAL_V1, rows)
            .unwrap()
            .generator
    };
    let mut point = 1_u64;
    for row in 0..rows {
        let current: Vec<u64> = trace.iter().map(|column| column[row]).collect();
        let next: Vec<u64> = trace
            .iter()
            .map(|column| column[(row + 1) % rows])
            .collect();
        let base = air.evaluate(point, &current, &next).unwrap();
        assert!(base.iter().all(|&value| value == 0), "row={row}");
        point = crate::field::mul_base(point, generator);
    }
    // Arbitrary cells and points, including the selector's removable singularity.
    let current: Vec<u64> = (0..WIDTH as u64).map(|cell| 1000 + 37 * cell).collect();
    let next: Vec<u64> = (0..WIDTH as u64).map(|cell| 9000 + 53 * cell).collect();
    for mutation in [
        Mutation::None,
        Mutation::UndeclaredCubic,
        Mutation::DropWraparoundGate,
    ] {
        let air = LinkedSequences::new(rows, 3, 5, 7, 11).mutated(mutation);
        for point in [1, 2, 7, generator, GOLDILOCKS_MODULUS_V1 - 1] {
            let base = air.evaluate(point, &current, &next).unwrap();
            let extension = air
                .evaluate(
                    F::embed_base(point),
                    &current
                        .iter()
                        .map(|&cell| F::embed_base(cell))
                        .collect::<Vec<_>>(),
                    &next
                        .iter()
                        .map(|&cell| F::embed_base(cell))
                        .collect::<Vec<_>>(),
                )
                .unwrap();
            assert_eq!(base.len(), CONSTRAINTS);
            assert_eq!(
                extension,
                base.iter()
                    .map(|&value| F::embed_base(value))
                    .collect::<Vec<_>>(),
                "{mutation:?} point={point}"
            );
        }
    }
    // Malformed rows, points and cells are refused by the evaluator itself.
    assert!(air.evaluate(1_u64, &current[..WIDTH - 1], &next).is_err());
    assert!(
        air.evaluate(GOLDILOCKS_MODULUS_V1, &current, &next)
            .is_err()
    );
    let mut bad = current.clone();
    bad[C] = GOLDILOCKS_MODULUS_V1;
    assert!(air.evaluate(1_u64, &bad, &next).is_err());
}

#[test]
fn an_evaluator_that_is_not_one_polynomial_map_is_rejected() {
    // The base-field rows are satisfied, but the extension evaluation differs
    // from the base equation, so no committed engine could prove the relation.
    let rows = 16;
    let (air, trace) = LinkedSequences::honest(rows, 3, 5, 7);
    let inconsistent =
        LinkedSequences::new(rows, 3, 5, 7, air.result).mutated(Mutation::ExtensionOnlyTerm);
    assert!(matches!(
        build(&inconsistent, trace.clone()),
        Err(AirError::NotDivisible | AirError::DegreeBoundExceeded { .. })
    ));
    let artifact = build(&air, trace).unwrap();
    assert!(matches!(
        check(&inconsistent, &artifact),
        Err(AirError::NotDivisible | AirError::DegreeBoundExceeded { .. })
    ));
}

#[test]
fn an_understated_numerator_degree_is_rejected() {
    let rows = 16;
    // The cubic program needs degree < 4N - 3; declare the quadratic bound.
    let trace = sequence_trace(rows, 3, 5, 7, Mutation::UndeclaredCubic);
    let result = trace[B][rows - 1];
    let understated =
        LinkedSequences::new(rows, 3, 5, 7, result).mutated(Mutation::UndeclaredCubic);
    assert_eq!(understated.declared_degree, 3 * rows - 2);
    assert!(matches!(
        build(&understated, trace.clone()),
        Err(AirError::DegreeBoundExceeded { .. } | AirError::NotDivisible | AirError::OodIdentity)
    ));
    // With the honest declaration the same relation and trace are accepted.
    let declared = LinkedSequences {
        declared_degree: 4 * rows - 3,
        ..LinkedSequences::new(rows, 3, 5, 7, result).mutated(Mutation::UndeclaredCubic)
    };
    let artifact = build(&declared, trace).unwrap();
    assert_eq!(artifact.quotient().len(), 3 * rows - 3);
    check(&declared, &artifact).unwrap();
}

#[test]
fn observers_see_every_phase_and_cannot_change_a_result() {
    let rows = 16;
    let (air, trace) = LinkedSequences::honest(rows, 3, 5, 7);
    let recorder = Recorder::default();
    let artifact = build_reference(
        &air,
        trace.clone(),
        &challenges(),
        WorkLimits::default(),
        &recorder,
    )
    .unwrap();
    check_reference(
        &air,
        &artifact,
        &challenges(),
        WorkLimits::default(),
        &recorder,
    )
    .unwrap();
    assert_eq!(
        recorder.labels(),
        [
            "BuildReference:admitted",
            "BuildReference:trace",
            "BuildReference:quotient",
            "BuildReference:ood",
            "BuildReference:completed",
            "CheckReference:admitted",
            "CheckReference:trace",
            "CheckReference:quotient",
            "CheckReference:ood",
            "CheckReference:completed",
        ]
    );
    // A rejection reports no later phase.
    let recorder = Recorder::default();
    let mut corrupted = trace.clone();
    corrupted[A][4] = corrupted[A][4].add(1);
    assert!(
        build_reference(
            &air,
            corrupted,
            &challenges(),
            WorkLimits::default(),
            &recorder
        )
        .is_err()
    );
    assert_eq!(
        recorder.labels(),
        ["BuildReference:admitted", "BuildReference:rejected"]
    );

    let observed = build_reference(
        &air,
        trace.clone(),
        &challenges(),
        WorkLimits::default(),
        &Panicking,
    )
    .unwrap();
    assert_eq!(observed, artifact);
    assert_eq!(
        check_reference(
            &air,
            &artifact,
            &challenges(),
            WorkLimits::default(),
            &Panicking
        )
        .unwrap(),
        check(&air, &artifact).unwrap()
    );
    let mut corrupted = trace;
    corrupted[B][2] = corrupted[B][2].add(1);
    assert!(matches!(
        build_reference(
            &air,
            corrupted,
            &challenges(),
            WorkLimits::default(),
            &Panicking
        ),
        Err(AirError::ConstraintViolated { .. })
    ));
}

#[test]
fn schema_helpers_and_default_limits_match_the_engine_profile() {
    let schema = AirSchema {
        identity: IDENTITY,
        trace_rows: 64,
        width: WIDTH,
        constraints: CONSTRAINTS,
        numerator_degree_bound: 190,
    };
    schema.validate().unwrap();
    assert_eq!(schema.trace_cells(), 320);
    assert_eq!(schema.quotient_degree_bound(), 126);
    assert_eq!(
        AirSchema {
            numerator_degree_bound: 10,
            ..schema
        }
        .quotient_degree_bound(),
        0
    );
    let limits = WorkLimits::default();
    assert_eq!(limits.max_trace_rows, q77::PROFILE.trace_rows);
    assert_eq!(limits.max_trace_cells, 65_536 * 342);
    assert_eq!(limits.max_constraints, q77::PROFILE.constraints);
    assert_eq!(limits.max_statement_bytes, MAX_STATEMENT_BYTES);
    assert_eq!(limits.max_payload_bytes, 2 << 30);
    // The payload and work ceilings are the default proving policy's own
    // values, read from that owner: a changed policy default changes these.
    let proving = crate::offline_compact::ProvingLimits::default();
    assert_eq!(limits.max_payload_bytes, proving.max_segment_charge_bytes);
    assert_eq!(limits.max_work_units, proving.max_segment_work_units);
    assert_eq!(limits, q77::ProducerLimits::default().work);
    assert_eq!(MAX_REFERENCE_TRACE_ROWS, 65_536);
    assert_eq!(MAX_REFERENCE_INTERPOLATION_ROWS, 524_288);
    // The debug form of an artifact reports sizes, never cells.
    let (_, artifact) = honest_artifact(4);
    assert_eq!(
        format!("{artifact:?}"),
        format!(
            "ReferenceArtifact {{ identity: {IDENTITY:?}, statement_bytes: 40, columns: 5, \
             quotient_coefficients: 6 }}"
        )
    );
}
