//! The existing complete transfer relation expressed through the typed interface.

use std::sync::{Arc, Mutex};

use fastpq_isi::FASTPQ_FINAL_V1;

use super::*;
use crate::{
    VerifyLimits,
    backend::{
        air::{AirError, ReferenceChallenges, ReferencePlan, WorkLimits, check_reference},
        compact_protocol::{FixedAir, FixedAirSchema, PreparedAir},
        compact_public_columns::PublicColumnReconstruction,
        compact_transfer_air::{
            CompactTransferAir,
            tests::{physical_fixture, statement},
        },
        deep_engine, deep_fixture,
        deep_geometry::DeepGeometry,
        deep_prover::ProducerPlan,
        deep_relation::sealed::Sealed,
        deep_trace_source::OwnedTraceSource,
        fixed_domain::FixedTraceDomain,
        polynomial_transform::PolynomialDomain,
    },
    field::{GoldilocksFp4V1 as F, mul_base},
    gadgets::{
        compact_smt_air::PHYSICAL_ROW_COUNT,
        compact_trace_columns::{smt_row_cells, smt_row_from_cells},
        transfer_integer_air::IntegerAirField,
    },
};

/// Serializes the tests that install the process observer.
static OBSERVER_TESTS: Mutex<()> = Mutex::new(());

/// A sealed relation with another outer identity and the same arithmetic owner.
struct Tagged<'a> {
    relation: &'a CompactTransferAir,
    identity: &'static str,
}

impl FixedAir for Tagged<'_> {
    fn schema(&self) -> FixedAirSchema {
        FixedAirSchema {
            identity: self.identity,
            ..self.relation.schema()
        }
    }
    fn statement_bytes(&self) -> &[u8] {
        self.relation.statement_bytes()
    }
    fn evaluate(&self, point: u64, current: &[u64], next: &[u64]) -> Result<Vec<u64>> {
        self.relation.evaluate(point, current, next)
    }
    fn prepare_prover(&self) -> Result<Box<dyn PreparedAir + '_>> {
        self.relation.prepare_prover()
    }
}

impl Sealed for Tagged<'_> {}

impl DeepRelation for Tagged<'_> {
    fn deep_relation(&self) -> &CompactTransferAir {
        self.relation
    }
}

/// A sealed wrapper that declares another geometry than its arithmetic owner.
struct Misdeclared<'a> {
    relation: &'a CompactTransferAir,
    schema: FixedAirSchema,
}

impl FixedAir for Misdeclared<'_> {
    fn schema(&self) -> FixedAirSchema {
        self.schema
    }
    fn statement_bytes(&self) -> &[u8] {
        self.relation.statement_bytes()
    }
    fn evaluate(&self, point: u64, current: &[u64], next: &[u64]) -> Result<Vec<u64>> {
        self.relation.evaluate(point, current, next)
    }
    fn prepare_prover(&self) -> Result<Box<dyn PreparedAir + '_>> {
        self.relation.prepare_prover()
    }
}

impl Sealed for Misdeclared<'_> {}

impl DeepRelation for Misdeclared<'_> {
    fn deep_relation(&self) -> &CompactTransferAir {
        self.relation
    }
}

#[derive(Default)]
struct Recorder(Mutex<Vec<(Operation, String, Option<Work>)>>);

impl Recorder {
    /// Events of one identity, in order: `(operation, kind, measured work)`.
    fn for_identity(&self, identity: &str) -> Vec<(Operation, String, Option<Work>)> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, label, _)| label.ends_with(identity))
            .cloned()
            .collect()
    }
}

impl Observer for Recorder {
    fn observe(&self, event: &Event<'_>) {
        let entry = match *event {
            Event::Admitted {
                operation,
                identity,
                ..
            } => (operation, format!("admitted:{identity}"), None),
            Event::Completed {
                operation,
                identity,
                work,
            } => (operation, format!("completed:{identity}"), Some(work)),
            Event::Rejected {
                operation,
                identity,
            } => (operation, format!("rejected:{identity}"), None),
            _ => return,
        };
        self.0.lock().unwrap().push(entry);
    }
}

/// Observer whose every notification panics.
struct Panicking;
impl Observer for Panicking {
    fn observe(&self, _: &Event<'_>) {
        panic!("observer failure must not affect validity");
    }
}

fn trace_generator() -> u64 {
    FixedTraceDomain::new(&FASTPQ_FINAL_V1, PHYSICAL_ROW_COUNT)
        .unwrap()
        .generator
}

#[test]
fn profile_and_sealed_relations_report_the_engine_owners_constants() {
    assert_eq!(PROFILE.trace_rows, 65_536);
    assert_eq!(PROFILE.columns, 342);
    assert_eq!(PROFILE.committed_columns, 301);
    assert_eq!(PROFILE.public_columns, 41);
    assert_eq!(
        PROFILE.columns,
        PROFILE.committed_columns + PROFILE.public_columns
    );
    assert_eq!(PROFILE.constraints, 923);
    assert_eq!(PROFILE.evaluation_rows, 8_388_608);
    assert_eq!(PROFILE.fri_arities, [16, 16, 8, 8, 4]);
    assert_eq!(
        PROFILE.fri_lengths,
        [8_388_608, 524_288, 32_768, 4_096, 512, 128]
    );
    assert_eq!(PROFILE.fri_degree_bounds, [131_072, 8_192, 512, 64, 8, 2]);
    assert_eq!(PROFILE.queries, 77);
    assert_eq!(PROFILE.query_candidates, 87);
    assert_eq!(PROFILE.trace_mask_coefficients, 162);
    assert_eq!(PROFILE.quotient_mask_coefficients, 78);
    assert_eq!(PROFILE.max_frame_bytes, 500_084);
    assert!(PROFILE.public_column_layout.contains("342-to-301"));

    let mut identities: Vec<_> = SEALED_RELATIONS
        .iter()
        .map(|relation| relation.identity)
        .collect();
    identities.sort_unstable();
    identities.dedup();
    assert_eq!(identities.len(), SEALED_RELATIONS.len());
    for relation in SEALED_RELATIONS {
        let schema = relation.schema();
        schema.validate().unwrap();
        assert_eq!(schema.identity, relation.identity);
        assert_eq!(
            (schema.trace_rows, schema.width, schema.constraints),
            (65_536, 342, 923)
        );
        assert!(relation.identity.starts_with("fastpq:compact:v1:"));
    }
    assert_eq!(SEALED_RELATIONS[0].role, RelationRole::CompactTransfer);
    assert_eq!(SEALED_RELATIONS[0].values, ValueDomain::None);
    assert_eq!(
        SEALED_RELATIONS
            .iter()
            .filter(|relation| relation.values == ValueDomain::Quantity)
            .map(|relation| relation.role)
            .collect::<Vec<_>>(),
        [
            RelationRole::ExecutionEffectSegment,
            RelationRole::OrdinaryTransferSegment,
            RelationRole::AxtTransferSegment
        ]
    );
}

#[test]
fn sealed_view_preserves_outer_identity_statement_and_the_degree_owner() {
    let statement = statement();
    let air = CompactTransferAir::new(&statement, Some(b"interface-context")).unwrap();
    let view = SealedView::new(&air);
    let schema = SemanticAir::schema(&view);
    assert_eq!(schema, SEALED_RELATIONS[0].schema());
    // The view reports the relation's own declaration, field by field.
    let declared = FixedAir::schema(&air);
    assert_eq!(
        (
            schema.identity,
            schema.trace_rows,
            schema.width,
            schema.constraints
        ),
        (
            declared.identity,
            declared.trace_rows,
            declared.width,
            declared.constraints
        )
    );
    assert_eq!(schema.numerator_degree_bound, 196_479);
    // The declared bound is the engine's own structural degree calculation
    // for unmasked columns, not a second derivation.
    assert_eq!(
        air.numerator_degree_bounds(&[PHYSICAL_ROW_COUNT; 342])
            .unwrap()
            .combined_numerator(),
        schema.numerator_degree_bound
    );
    assert_eq!(
        SemanticAir::statement_bytes(&view),
        FixedAir::statement_bytes(&air)
    );
    // A wrapper keeps its own identity and every statement byte while the
    // arithmetic owner is borrowed unchanged.
    let tagged = Tagged {
        relation: &air,
        identity: "fastpq:air:test:q77-tagged:v1",
    };
    let tagged_view = SealedView::new(&tagged);
    assert_eq!(
        SemanticAir::schema(&tagged_view),
        AirSchema {
            identity: "fastpq:air:test:q77-tagged:v1",
            ..schema
        }
    );
    let tagged_declared = FixedAir::schema(&tagged);
    assert_eq!(
        SemanticAir::schema(&tagged_view).identity,
        tagged_declared.identity
    );
    assert_eq!(
        (
            tagged_declared.trace_rows,
            tagged_declared.width,
            tagged_declared.constraints
        ),
        (schema.trace_rows, schema.width, schema.constraints)
    );
    assert_eq!(
        SemanticAir::statement_bytes(&tagged_view),
        SemanticAir::statement_bytes(&view)
    );
    // Public statement and context mutations change the bound bytes.
    let mut changed = statement;
    changed.new_root[3] ^= 1;
    let changed = CompactTransferAir::new(&changed, Some(b"interface-context")).unwrap();
    assert_ne!(
        SemanticAir::statement_bytes(&SealedView::new(&changed)),
        SemanticAir::statement_bytes(&view)
    );
    let other_context = CompactTransferAir::new(&statement, Some(b"other-context")).unwrap();
    assert_ne!(
        SemanticAir::statement_bytes(&SealedView::new(&other_context)),
        SemanticAir::statement_bytes(&view)
    );
    // The reference planner admits the real relation under the default ceilings.
    let plan = ReferencePlan::new(&view, WorkLimits::default()).unwrap();
    assert_eq!(plan.interpolation_rows, 262_144);
    assert_eq!(plan.stripes, 4);
    assert_eq!(plan.quotient_coefficients, 196_479 - 65_536);
    assert!(matches!(
        ReferencePlan::new(
            &view,
            WorkLimits {
                max_constraints: 922,
                ..WorkLimits::default()
            }
        ),
        Err(AirError::Limit {
            limit: "max_constraints",
            actual: 923,
            max: 922
        })
    ));
}

#[test]
fn sealed_view_public_columns_match_the_witness_and_the_closed_form() {
    let (statement, witness) = physical_fixture();
    let air = CompactTransferAir::new(&statement, None).unwrap();
    let view = SealedView::new(&air);
    let columns = view.public_columns();
    assert_eq!(columns.len(), 41);
    assert!(columns.windows(2).all(|pair| pair[0] < pair[1]));
    let rows = witness.rows();
    // Every declared verifier-known cell equals the honest physical witness.
    for row in (0..PHYSICAL_ROW_COUNT).filter(|row| row % 512 < 16 || row % 97 == 0) {
        let cells = smt_row_cells(&rows[row]);
        for (ordinal, &column) in columns.iter().enumerate() {
            assert_eq!(
                view.public_value(ordinal, row).unwrap(),
                cells[column],
                "row={row} column={column}"
            );
        }
    }
    assert!(view.public_value(41, 0).is_err());
    assert!(view.public_value(0, PHYSICAL_ROW_COUNT).is_err());
    // The declared row values interpolate to the engine's closed-form
    // reconstruction at a quartic-extension point.
    let point = F::new([11, 13, 17, 19]).unwrap();
    let closed = PublicColumnReconstruction::new(&DeepGeometry::polynomial_parameters())
        .unwrap()
        .evaluate(point)
        .unwrap();
    let subgroup =
        PolynomialDomain::new(PHYSICAL_ROW_COUNT, F::ONE, PHYSICAL_ROW_COUNT, usize::MAX).unwrap();
    for ordinal in [0, 3, 4, 14, 15, 26, 38, 39, 40] {
        let values: Vec<F> = (0..PHYSICAL_ROW_COUNT)
            .map(|row| F::embed_base(view.public_value(ordinal, row).unwrap()))
            .collect();
        let coefficients = subgroup.interpolate(&values).unwrap();
        let interpolated = coefficients
            .iter()
            .rev()
            .fold(F::ZERO, |sum, &coefficient| sum.mul(point).add(coefficient));
        assert_eq!(interpolated, closed[ordinal], "ordinal={ordinal}");
    }
}

#[test]
fn sealed_view_accepts_honest_rows_and_evaluation_detects_cell_mutations() {
    let (statement, witness) = physical_fixture();
    let air = CompactTransferAir::new(&statement, None).unwrap();
    let view = SealedView::new(&air);
    let rows = witness.rows();
    let generator = trace_generator();
    // Whole hash blocks at both ends, every block boundary and a stride through
    // the interior, including the cyclic wrap from the last row to the first.
    let sampled = (0..PHYSICAL_ROW_COUNT).filter(|row| {
        *row < 1024 || *row >= PHYSICAL_ROW_COUNT - 1024 || row % 512 >= 510 || row % 61 == 0
    });
    let mut point = 1_u64;
    let mut position = 0;
    let mut checked = 0;
    for row in sampled {
        while position < row {
            point = mul_base(point, generator);
            position += 1;
        }
        let current = smt_row_cells(&rows[row]);
        let next = smt_row_cells(&rows[(row + 1) % PHYSICAL_ROW_COUNT]);
        let residues = view.evaluate(point, &current, &next).unwrap();
        assert_eq!(residues.len(), 923);
        assert!(
            residues.iter().all(|&value| value == 0),
            "honest row {row} violates slot {:?}",
            residues.iter().position(|&value| value != 0)
        );
        checked += 1;
    }
    assert!(checked > 3000, "sampled {checked} honest rows");
    // One changed hash cell makes the same evaluator report a violation on its
    // own row pair.
    for (row, column) in [(0, 0), (7, 100), (300, 200), (1036, 40), (40_000, 5)] {
        let mut current = smt_row_cells(&rows[row]);
        let next = smt_row_cells(&rows[row + 1]);
        current[column] = current[column].add(1);
        let point = crate::backend::field_pow(generator, row as u64);
        let residues = view.evaluate(point, &current, &next).unwrap();
        assert!(
            residues.iter().any(|&value| value != 0),
            "row={row} column={column}"
        );
    }
    // SMT cells are constrained on their scheduled rows: within one hash
    // block a changed first SMT cell is rejected as the current or the next
    // row of some pair.
    let smt_column = 310;
    let mut detected = 0;
    for row in 1..512 {
        let mut changed = smt_row_cells(&rows[row]);
        changed[smt_column] = changed[smt_column].add(1);
        let before = smt_row_cells(&rows[row - 1]);
        let after = smt_row_cells(&rows[row + 1]);
        let as_current = view
            .evaluate(
                crate::backend::field_pow(generator, row as u64),
                &changed,
                &after,
            )
            .unwrap();
        let as_next = view
            .evaluate(
                crate::backend::field_pow(generator, row as u64 - 1),
                &before,
                &changed,
            )
            .unwrap();
        if as_current.iter().chain(&as_next).any(|&value| value != 0) {
            detected += 1;
        }
    }
    assert!(detected > 0, "no SMT cell mutation was detected");
    // Another public root is caught by the evaluated SMT slots, not only by
    // the statement binding.
    let mut other = statement;
    other.new_root[0] ^= 1;
    let other = CompactTransferAir::new(&other, None).unwrap();
    let other = SealedView::new(&other);
    let mut point = 1_u64;
    let mut violated = false;
    for row in 0..PHYSICAL_ROW_COUNT {
        if row % 512 >= 400 {
            let current = smt_row_cells(&rows[row]);
            let next = smt_row_cells(&rows[(row + 1) % PHYSICAL_ROW_COUNT]);
            if other
                .evaluate(point, &current, &next)
                .unwrap()
                .iter()
                .any(|&value| value != 0)
            {
                violated = true;
                break;
            }
        }
        point = mul_base(point, generator);
    }
    assert!(violated);
}

#[test]
fn sealed_view_base_and_extension_evaluations_agree() {
    let (statement, witness) = physical_fixture();
    let air = CompactTransferAir::new(&statement, None).unwrap();
    let view = SealedView::new(&air);
    let rows = witness.rows();
    let generator = trace_generator();
    for (row, point) in [
        (0, 1),
        (407, crate::backend::field_pow(generator, 407)),
        (1000, 7),
        (65_535, FASTPQ_FINAL_V1.omega_coset),
    ] {
        let current = smt_row_cells(&rows[row]);
        let next = smt_row_cells(&rows[(row + 1) % PHYSICAL_ROW_COUNT]);
        let base = view.evaluate(point, &current, &next).unwrap();
        let extension = view
            .evaluate(
                F::embed_base(point),
                &current.map(F::embed_base),
                &next.map(F::embed_base),
            )
            .unwrap();
        assert_eq!(
            extension,
            base.into_iter().map(F::embed_base).collect::<Vec<_>>(),
            "row={row}"
        );
    }
    let zero = [0_u64; 342];
    assert!(view.evaluate(1_u64, &zero[..341], &zero).is_err());
    assert!(view.evaluate(1_u64, &zero, &zero[..341]).is_err());
}

#[test]
fn process_observer_sees_committed_rejections_and_cannot_change_them() {
    let _guard = OBSERVER_TESTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let air = CompactTransferAir::new(&statement(), Some(b"observer")).unwrap();
    let identity = "fastpq:air:test:q77-observer:v1";
    let tagged = Tagged {
        relation: &air,
        identity,
    };
    let limits = VerifierLimits::for_segment(deep_fixture::verification_limits(), 1 << 20);
    let unobserved = deep_engine::verify_committed(&tagged, &[0_u8; 64], limits)
        .unwrap_err()
        .to_string();

    let recorder = Arc::new(Recorder::default());
    set_observer(recorder.clone());
    let observed = deep_engine::verify_committed(&tagged, &[0_u8; 64], limits)
        .unwrap_err()
        .to_string();
    assert_eq!(observed, unobserved);
    assert_eq!(
        recorder.for_identity(identity),
        [(Operation::Q77Verify, format!("rejected:{identity}"), None)]
    );

    set_observer(Arc::new(Panicking));
    let panicking = deep_engine::verify_committed(&tagged, &[0_u8; 64], limits)
        .unwrap_err()
        .to_string();
    assert_eq!(panicking, unobserved);

    clear_observer();
    assert!(deep_engine::verify_committed(&tagged, &[0_u8; 64], limits).is_err());
    // Nothing was delivered after the observer was replaced and cleared.
    assert_eq!(recorder.for_identity(identity).len(), 1);
}

#[test]
fn process_observer_sees_producer_admission_and_rejection() {
    let _guard = OBSERVER_TESTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (statement, mut witness) = physical_fixture();
    // One inconsistent verifier-known cell: the producer refuses the source
    // before any masking, commitment or transcript challenge.
    let mut cells = smt_row_cells(&witness.rows()[0]);
    cells[300] = cells[300].add(1);
    witness.rows_mut()[0] = smt_row_from_cells(&cells);
    let source = OwnedTraceSource::from_rows(witness.rows()).unwrap();
    drop(witness);
    let air = CompactTransferAir::new(&statement, Some(b"producer-observer")).unwrap();
    let identity = "fastpq:air:test:q77-producer-observer:v1";
    let tagged = Tagged {
        relation: &air,
        identity,
    };
    let recorder = Arc::new(Recorder::default());
    set_observer(recorder.clone());
    let result = deep_fixture::prove(&tagged, source, 0x00b1_0001);
    clear_observer();
    assert!(matches!(result, Err(Error::InvalidTraceShape { .. })));
    assert_eq!(
        recorder.for_identity(identity),
        [
            (Operation::Q77Prove, format!("admitted:{identity}"), None),
            (Operation::Q77Prove, format!("rejected:{identity}"), None),
        ]
    );
    // The outcome hooks report exactly what they are given.
    let recorder = Arc::new(Recorder::default());
    set_observer(recorder.clone());
    observe_proof_admitted(&tagged, 7, 9);
    observe_proof(&tagged, &Ok(vec![0_u8; 5]));
    observe_proof(
        &tagged,
        &Err(Error::InvalidTraceShape {
            details: String::new(),
        }),
    );
    clear_observer();
    assert_eq!(
        recorder.for_identity(identity),
        [
            (Operation::Q77Prove, format!("admitted:{identity}"), None),
            (
                Operation::Q77Prove,
                format!("completed:{identity}"),
                Some(Work::Q77Proof { proof_bytes: 5 })
            ),
            (Operation::Q77Prove, format!("rejected:{identity}"), None),
        ]
    );
}

#[test]
#[ignore = "complete committed q77 proof under the interface's typed limits and observer, verified through the sealed view; run with --release"]
fn complete_q77_proof_under_interface_limits_with_observed_bounded_verification() {
    let _guard = OBSERVER_TESTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (statement, witness) = physical_fixture();
    let source = OwnedTraceSource::from_rows(witness.rows()).unwrap();
    drop(witness);
    let air = CompactTransferAir::new(&statement, Some(b"air-interface-q77")).unwrap();
    let identity = "fastpq:air:test:q77-interface-proof:v1";
    let tagged = Tagged {
        relation: &air,
        identity,
    };
    let recorder = Arc::new(Recorder::default());
    set_observer(recorder.clone());
    let bytes = deep_fixture::prove(&tagged, source, 0x00b1_0077).unwrap();
    let limits = deep_fixture::engine_verification_limits();
    let verified = deep_engine::verify_committed(&tagged, &bytes, limits).unwrap();
    clear_observer();
    let work = verified.work();
    assert_eq!(work.proof_bytes, bytes.len());
    assert_eq!(work.air_evaluations, 1);
    assert_eq!(work.verifier_messages, 10);
    assert!(bytes.len() <= PROFILE.max_frame_bytes);
    // Admission, the producer's own bounded self-check, the construction
    // result and the independent verification, in that order.
    assert_eq!(
        recorder.for_identity(identity),
        [
            (Operation::Q77Prove, format!("admitted:{identity}"), None),
            (
                Operation::Q77Verify,
                format!("completed:{identity}"),
                Some(Work::Q77Verification(work))
            ),
            (
                Operation::Q77Prove,
                format!("completed:{identity}"),
                Some(Work::Q77Proof {
                    proof_bytes: bytes.len()
                })
            ),
            (
                Operation::Q77Verify,
                format!("completed:{identity}"),
                Some(Work::Q77Verification(work))
            ),
        ]
    );
    // The proof is bound to the outer identity and to the complete statement.
    assert!(deep_engine::verify_committed(&air, &bytes, limits).is_err());
    let other = CompactTransferAir::new(&statement, Some(b"another-context")).unwrap();
    let other = Tagged {
        relation: &other,
        identity,
    };
    assert!(deep_engine::verify_committed(&other, &bytes, limits).is_err());
    let mut corrupted = bytes.clone();
    let last = corrupted.len() - 1;
    corrupted[last] ^= 1;
    assert!(deep_engine::verify_committed(&tagged, &corrupted, limits).is_err());
}

#[test]
#[ignore = "complete uncommitted reference check of the 65536x342 transfer relation; run with --release"]
fn complete_reference_check_of_the_sealed_transfer_relation() {
    let (statement, witness) = physical_fixture();
    let air = CompactTransferAir::new(&statement, Some(b"air-interface-reference")).unwrap();
    let view = SealedView::new(&air);
    let mut trace = vec![vec![0_u64; PHYSICAL_ROW_COUNT]; 342];
    for (row, value) in witness.rows().iter().enumerate() {
        for (column, cell) in smt_row_cells(value).into_iter().enumerate() {
            trace[column][row] = cell;
        }
    }
    drop(witness);
    let challenges = ReferenceChallenges::derive(&[0x77; 32], 923);
    let artifact = crate::backend::air::build_reference(
        &view,
        trace,
        &challenges,
        WorkLimits::default(),
        &crate::backend::air::NoObserver,
    )
    .unwrap();
    let work = check_reference(
        &view,
        &artifact,
        &challenges,
        WorkLimits::default(),
        &crate::backend::air::NoObserver,
    )
    .unwrap();
    assert_eq!(work.constraint_evaluations, 65_536 + 262_144 + 1);
    // A changed committed cell is rejected by the checker's own evaluation.
    let (identity, bound, mut trace, quotient) = artifact.into_parts();
    trace[5][40_000] = trace[5][40_000].add(1);
    let forged =
        crate::backend::air::ReferenceArtifact::from_parts(identity, bound, trace, quotient);
    assert!(matches!(
        check_reference(
            &view,
            &forged,
            &challenges,
            WorkLimits::default(),
            &crate::backend::air::NoObserver,
        ),
        Err(AirError::ConstraintViolated { .. })
    ));
}

#[test]
fn sealed_view_reports_the_relation_geometry_and_the_engine_refuses_a_misdeclared_one() {
    let air = CompactTransferAir::new(&statement(), Some(b"misdeclared")).unwrap();
    let honest = FixedAir::schema(&air);
    let exact = VerifierLimits::exact(air.statement_bytes().len());
    deep_engine::preflight(&air, PROFILE.max_frame_bytes, exact).unwrap();
    for (changed, field) in [
        (
            FixedAirSchema {
                trace_rows: honest.trace_rows / 2,
                ..honest
            },
            "trace_rows",
        ),
        (
            FixedAirSchema {
                width: honest.width - 1,
                ..honest
            },
            "width",
        ),
        (
            FixedAirSchema {
                constraints: honest.constraints + 1,
                ..honest
            },
            "constraints",
        ),
    ] {
        let relation = Misdeclared {
            relation: &air,
            schema: changed,
        };
        // The view carries the wrapper's own declaration; it does not replace
        // it with the engine's constants.
        let viewed = SemanticAir::schema(&SealedView::new(&relation));
        assert_eq!(
            (viewed.trace_rows, viewed.width, viewed.constraints),
            (changed.trace_rows, changed.width, changed.constraints),
            "{field}"
        );
        assert_ne!(viewed, SEALED_RELATIONS[0].schema(), "{field}");
        // Both engine preflights refuse the divergent declaration.
        assert!(
            deep_engine::preflight(&relation, PROFILE.max_frame_bytes, exact).is_err(),
            "{field}"
        );
        assert!(
            ProducerPlan::new(&relation, ProducerLimits::default()).is_err(),
            "{field}"
        );
    }
}

#[test]
fn default_and_mapped_engine_limits_are_read_from_their_owners() {
    use crate::backend::offline_compact::{ProvingLimits, VerificationLimits};

    let proving = ProvingLimits::default();
    let verification = VerificationLimits::default();
    let segment = verification.bundle.segment;

    // Verifier defaults: the default public policy, mapped by the same function
    // the facade uses, with the statement capped at the binding envelope.
    let mapped =
        VerifierLimits::for_segment(segment, verification.max_segment_decode_allocation_charges);
    let verifier = VerifierLimits::default();
    assert_eq!(
        verifier,
        VerifierLimits {
            work: WorkLimits {
                max_statement_bytes: segment
                    .max_batch_bytes
                    .min(crate::backend::air::MAX_STATEMENT_BYTES),
                ..mapped.work
            },
            ..mapped
        }
    );
    assert_eq!(
        verifier.work.max_statement_bytes,
        crate::backend::air::MAX_STATEMENT_BYTES
    );

    // Producer and interface defaults: the default proving policy, not copies.
    let producer = ProducerLimits::default();
    assert_eq!(producer, ProducerLimits::for_segment(&proving, &verifier));
    assert_eq!(producer.digest_execution, proving.digest_execution);
    assert_eq!(
        producer.work.max_payload_bytes,
        proving.max_segment_charge_bytes
    );
    assert_eq!(producer.work.max_work_units, proving.max_segment_work_units);
    assert_eq!(producer.max_hash_calls, proving.max_segment_work_units);
    assert_eq!(producer.max_proof_bytes, segment.max_proof_bytes);
    assert_eq!(WorkLimits::default(), producer.work);
    // The interface default is the producer's projection. Verification
    // declares its own payload and work charges under the same policy.
    assert_eq!(
        (
            verifier.work.max_payload_bytes,
            verifier.work.max_work_units
        ),
        (PROFILE.max_frame_bytes + 8 * 1024 * 1024, 29_751)
    );
    assert_ne!(verifier.work, producer.work);
    assert_eq!(
        WorkLimits {
            max_payload_bytes: producer.work.max_payload_bytes,
            max_work_units: producer.work.max_work_units,
            ..verifier.work
        },
        producer.work
    );

    // A changed policy value reaches the engine limits unchanged.
    let changed = ProvingLimits {
        max_segment_charge_bytes: 12_345,
        max_segment_work_units: 678,
        ..proving
    };
    let changed = ProducerLimits::for_segment(&changed, &verifier);
    assert_eq!(
        (
            changed.work.max_payload_bytes,
            changed.work.max_work_units,
            changed.max_hash_calls
        ),
        (12_345, 678, 678)
    );
    assert_eq!(
        WorkLimits {
            max_payload_bytes: producer.work.max_payload_bytes,
            max_work_units: producer.work.max_work_units,
            ..changed.work
        },
        producer.work
    );

    // Every facade segment field maps to exactly one engine ceiling; the
    // transition ceiling stays with the enclosing bundle.
    let distinct = VerifyLimits {
        max_transitions: 1,
        max_batch_bytes: 2,
        max_proof_bytes: 3,
        max_fri_layers: 4,
        max_queries: 5,
        max_query_chunk_values: 6,
        max_query_path_len: 7,
        max_fri_round_values: 8,
        max_air_row_values: 9,
    };
    assert_eq!(
        VerifierLimits::for_segment(distinct, 10),
        VerifierLimits {
            work: WorkLimits {
                max_statement_bytes: 2,
                ..VerifierLimits::exact(0).work
            },
            max_proof_bytes: 3,
            max_fri_layers: 4,
            max_queries: 5,
            max_query_chunk_values: 6,
            max_query_path_len: 7,
            max_fri_round_values: 8,
            max_air_row_values: 9,
            max_decode_allocation_charges: 10,
        }
    );

    // The exact ceilings restate the published profile and nothing else.
    let exact = VerifierLimits::exact(77);
    assert_eq!(
        exact,
        VerifierLimits {
            work: WorkLimits {
                max_trace_rows: PROFILE.trace_rows,
                max_trace_cells: PROFILE.trace_rows * PROFILE.columns,
                max_constraints: PROFILE.constraints,
                max_statement_bytes: 77,
                max_payload_bytes: PROFILE.max_frame_bytes + 8 * 1024 * 1024,
                max_work_units: 29_751,
            },
            max_proof_bytes: PROFILE.max_frame_bytes,
            max_fri_layers: PROFILE.fri_lengths.len(),
            max_queries: PROFILE.queries,
            max_query_chunk_values: 2,
            max_query_path_len: PROFILE.evaluation_rows.ilog2() as usize,
            max_fri_round_values: *PROFILE.fri_arities.iter().max().unwrap(),
            max_air_row_values: PROFILE.committed_columns,
            max_decode_allocation_charges: 8 * 1024 * 1024,
        }
    );
}

#[test]
fn exceeded_work_limits_are_refused_by_the_committed_engine_and_the_reference() {
    type Lower = fn(&mut WorkLimits);
    let air = CompactTransferAir::new(&statement(), Some(b"limits")).unwrap();
    let view = SealedView::new(&air);
    let schema = SemanticAir::schema(&view);
    let bytes = SemanticAir::statement_bytes(&view).len();

    // The relation's declared work, as one interface value.
    let declared = WorkLimits {
        max_trace_rows: schema.trace_rows,
        max_trace_cells: schema.trace_cells(),
        max_constraints: schema.constraints,
        max_statement_bytes: bytes,
        ..WorkLimits::default()
    };
    assert_eq!(
        WorkLimits {
            max_statement_bytes: WorkLimits::default().max_statement_bytes,
            ..declared
        },
        WorkLimits::default()
    );

    // Committed verification: the exact ceilings admit, and one below any
    // `WorkLimits` field is refused by the engine's own preflight before a
    // proof byte is decoded.
    let verifier = VerifierLimits::exact(bytes);
    deep_engine::preflight(&air, PROFILE.max_frame_bytes, verifier).unwrap();
    let verifier_cases: [(&str, usize, Lower); 6] = [
        ("max_trace_rows", schema.trace_rows, |w| {
            w.max_trace_rows -= 1;
        }),
        ("max_trace_cells", schema.trace_cells(), |w| {
            w.max_trace_cells -= 1;
        }),
        ("max_constraints", schema.constraints, |w| {
            w.max_constraints -= 1;
        }),
        ("max_compact_statement_bytes", bytes, |w| {
            w.max_statement_bytes -= 1;
        }),
        (
            "max_verifier_payload_bytes",
            verifier.work.max_payload_bytes,
            |w| w.max_payload_bytes -= 1,
        ),
        (
            "max_verifier_work_units",
            verifier.work.max_work_units,
            |w| w.max_work_units -= 1,
        ),
    ];
    for (name, required, lower) in verifier_cases {
        let mut limited = verifier;
        lower(&mut limited.work);
        assert!(
            matches!(
                deep_engine::preflight(&air, PROFILE.max_frame_bytes, limited),
                Err(Error::VerifierLimitExceeded { limit, actual, max })
                    if limit == name && actual == required && max == required - 1
            ),
            "{name}"
        );
        // The complete bounded verifier refuses the same way and decodes nothing.
        assert!(
            matches!(
                deep_engine::verify_committed(
                    &air,
                    &vec![0_u8; PROFILE.max_frame_bytes],
                    limited
                ),
                Err(Error::VerifierLimitExceeded { limit, .. }) if limit == name
            ),
            "{name}"
        );
    }

    // Committed construction: the same four relation ceilings are refused by
    // the producer's whole-attempt plan before any private work.
    let producer = ProducerLimits {
        work: declared,
        ..ProducerLimits::default()
    };
    ProducerPlan::new(&air, producer).unwrap();
    let relation_cases: [(&str, &str, Lower); 4] = [
        ("max_deep_producer_trace_rows", "max_trace_rows", |w| {
            w.max_trace_rows -= 1;
        }),
        ("max_deep_producer_trace_cells", "max_trace_cells", |w| {
            w.max_trace_cells -= 1;
        }),
        ("max_deep_producer_constraints", "max_constraints", |w| {
            w.max_constraints -= 1;
        }),
        (
            "max_deep_producer_statement_bytes",
            "max_statement_bytes",
            |w| w.max_statement_bytes -= 1,
        ),
    ];
    for (engine_name, reference_name, lower) in relation_cases {
        let mut work = declared;
        lower(&mut work);
        assert!(
            matches!(
                ProducerPlan::new(&air, ProducerLimits { work, ..producer }),
                Err(Error::VerifierLimitExceeded { limit, .. }) if limit == engine_name
            ),
            "{engine_name}"
        );
        // The uncommitted reference refuses the identical interface value.
        assert!(
            matches!(
                ReferencePlan::new(&view, work),
                Err(AirError::Limit { limit, .. }) if limit == reference_name
            ),
            "{reference_name}"
        );
    }
    ReferencePlan::new(&view, declared).unwrap();

    // The producer's payload and work ceilings are the interface's as well.
    let charge_cases: [Lower; 2] = [|w| w.max_payload_bytes = 0, |w| w.max_work_units = 0];
    for lower in charge_cases {
        let mut work = declared;
        lower(&mut work);
        assert!(ProducerPlan::new(&air, ProducerLimits { work, ..producer }).is_err());
    }
}

#[test]
fn committed_verifier_limits_are_checked_through_the_sealed_relation() {
    // The interface statement ceiling and the engine's preflight agree on the
    // exact bound: one byte below the relation's statement is refused. The
    // facade's segment policy reaches the engine only as typed limits.
    let air = CompactTransferAir::new(&statement(), Some(b"limits")).unwrap();
    let bytes = SemanticAir::statement_bytes(&SealedView::new(&air)).len();
    let segment = VerifyLimits {
        max_batch_bytes: bytes - 1,
        ..deep_fixture::verification_limits()
    };
    let limits = VerifierLimits::for_segment(segment, 32 << 20);
    assert_eq!(limits.work.max_statement_bytes, bytes - 1);
    assert!(matches!(
        deep_engine::preflight(&air, PROFILE.max_frame_bytes, limits),
        Err(Error::VerifierLimitExceeded {
            limit: "max_compact_statement_bytes",
            actual,
            max
        }) if actual == bytes && max == bytes - 1
    ));
    assert!(
        deep_engine::preflight(
            &air,
            PROFILE.max_frame_bytes,
            VerifierLimits::for_segment(
                VerifyLimits {
                    max_batch_bytes: bytes,
                    ..segment
                },
                32 << 20
            )
        )
        .is_ok()
    );
    assert!(matches!(
        ReferencePlan::new(
            &SealedView::new(&air),
            WorkLimits {
                max_statement_bytes: bytes - 1,
                ..WorkLimits::default()
            }
        ),
        Err(AirError::Limit {
            limit: "max_statement_bytes",
            ..
        })
    ));
}
