//! The inline step relation against A's own statement, on sampled genuine witnesses.

use super::*;
use crate::{
    circuit::PrefixMode,
    vectors::{Mutation, sample_witness},
    witness::{SigmaRelation, Violation},
};
use ff::Field;
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    cs::ConstraintSystem,
    frontend::{Circuit, Layouter, SimpleFloorPlanner, Value, synthesize},
};
use iroha_plonk_recursion::verifier::VerifierConfig;

#[derive(Clone)]
struct Inline {
    step: StepWitness<Fp>,
    native: NativeStep<Fp>,
    statement: [Fp; 26],
    variant: Variant,
    relation: SigmaRelation,
    // Select A1's four-bus measurement profile and its exact probe shape.
    measured_shape: Option<RelationShape>,
    known: bool,
}

impl Circuit<Fp> for Inline {
    type Config = VerifierConfig<Ep>;
    type Params = bool;
    fn params(&self) -> bool {
        self.measured_shape.is_some()
    }
    type FloorPlanner = SimpleFloorPlanner;
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
        VerifierConfig::configure(meta)
    }
    fn configure_with_params(meta: &mut ConstraintSystem<Fp>, measured: bool) -> Self::Config {
        if measured {
            VerifierConfig::configure_serialized_foreign_tagged(
                meta,
                crate::a_relation::native::receive::INTERNAL_RANGE_BUSES,
            )
            .unwrap()
        } else {
            Self::configure(meta)
        }
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        let mut chip = VerifierChip::new(config);
        chip.load_tables(&mut layouter)?;
        layouter.assign_region(
            || "inline step relation",
            |mut region| {
                let value = |v| {
                    if self.known {
                        Value::known(v)
                    } else {
                        Value::unknown()
                    }
                };
                let fields = chip
                    .uint()
                    .glue()
                    .witnesses(&mut region, &self.statement.map(value))?
                    .try_into()
                    .map_err(|_| Error::Synthesis)?;
                let statement = StatementCells::constrain_with_verifier(
                    &mut chip,
                    &mut region,
                    self.variant,
                    &fields,
                )?;
                TransitionInputs {
                    shape: self
                        .measured_shape
                        .unwrap_or_else(|| RelationShape::new(self.relation, PrefixMode::Absorbed)),
                    witness: self.known.then_some((&self.step, &self.native)),
                }
                .constrain(&mut chip, &mut region, &statement)
            },
        )
    }
}

impl Inline {
    fn receive(seed: u64, variant: Variant) -> Self {
        let step = sample_witness::<Fp>(seed, SigmaRelation::RECEIVE, Mutation::None);
        let native = step.evaluate(SigmaRelation::RECEIVE);
        assert!(native.violations.is_empty());
        Self {
            statement: native.statement,
            step,
            native,
            variant,
            relation: SigmaRelation::RECEIVE,
            measured_shape: None,
            known: true,
        }
    }
    fn accepts(&self) -> bool {
        check_circuit(self, 16, &[], CheckMode::Strict).is_ok_and(|r| r.is_satisfied())
    }
}

#[test]
fn send_single_control_masks_bind_the_known_inline_relation() {
    for (mask, mutation, violation) in [
        (1, Mutation::QuotaExceeded, Violation::QuotaExceeded),
        (2, Mutation::LeaseExpired, Violation::LeaseExpired),
    ] {
        let seed = 17 + u64::from(mask);
        let relation = SigmaRelation::send(mask);
        let step = sample_witness::<Fp>(seed, relation, Mutation::None);
        let native = step.evaluate(relation);
        assert!(native.is_honest(), "Send mask{mask}");
        let honest = Inline {
            statement: native.statement,
            step,
            native,
            variant: Variant::Send,
            relation,
            measured_shape: None,
            known: true,
        };
        let report = check_circuit(&honest, 16, &[], CheckMode::Strict)
            .expect("honest Send inline relation must synthesize");
        assert!(report.is_satisfied(), "Send mask{mask}");
        for field in [11, 15] {
            let mut changed = honest.clone();
            changed.statement[field] += Fp::ONE;
            let report = check_circuit(&changed, 16, &[], CheckMode::Strict)
                .expect("changed Send statement must reach constraint checking");
            assert!(
                !report.is_satisfied(),
                "Send mask{mask} statement field{field}"
            );
        }
        // Match the digest reference and statement to the corrupted witness so
        // host digest checks cannot mask a missing quota/lease constraint. The
        // strict checker must obtain a report, then reject the control violation.
        let step = sample_witness::<Fp>(seed, relation, mutation);
        let native = step.evaluate(relation);
        assert!(
            native.violations.contains(&violation),
            "Send mask{mask} {mutation:?}"
        );
        let corrupted = Inline {
            statement: native.statement,
            step,
            native,
            ..honest.clone()
        };
        let report = check_circuit(&corrupted, 16, &[], CheckMode::Strict)
            .expect("corrupted Send control must reach constraint checking");
        assert!(!report.is_satisfied(), "Send mask{mask} {mutation:?}");
        let known = synthesize(&honest, 16, None).unwrap();
        let unknown = synthesize(&honest.without_witnesses(), 16, None).unwrap();
        assert_eq!(known.tables.fixed(), unknown.tables.fixed());
        assert_eq!(known.tables.permutation(), unknown.tables.permutation());
        assert_eq!(
            known.tables.advice_assigned(),
            unknown.tables.advice_assigned()
        );
    }
}

#[test]
fn receive_relation_binds_every_field_of_the_own_statement() {
    for (seed, variant) in [(7, Variant::Receive), (11, Variant::ReceiveRenewed)] {
        let honest = Inline::receive(seed, variant);
        assert!(honest.accepts(), "{variant:?}");
        // σ_recv certifies exactly these 26 inputs; any other own statement fails.
        for index in 0..26 {
            let mut wrong = honest.clone();
            wrong.statement[index] += Fp::ONE;
            assert!(!wrong.accepts(), "{variant:?} field {index}");
        }
        let known = synthesize(&honest, 16, None).unwrap();
        let unknown = synthesize(&honest.without_witnesses(), 16, None).unwrap();
        assert_eq!(known.tables.fixed(), unknown.tables.fixed());
        assert_eq!(known.tables.permutation(), unknown.tables.permutation());
    }
}

#[test]
fn inline_relation_must_be_the_statement_variants_own_step() {
    let honest = Inline::receive(13, Variant::Receive);
    // A Send relation cannot stand in for Receive's own step, and Receive's
    // relation cannot prove a Send or Load statement.
    let mut send = honest.clone();
    send.relation = SigmaRelation::SEND;
    assert!(synthesize(&send, 16, None).is_err());
    for variant in [Variant::Send, Variant::Load, Variant::ArchiveReceive] {
        let mut other = honest.clone();
        other.variant = variant;
        assert!(!other.accepts(), "{variant:?}");
    }
}

/// Check the exact candidate own-transition A1 uses, against an independently supplied statement.
pub(crate) fn core30_inline_receive_accepts(step: &StepWitness<Fp>, statement: [Fp; 26]) -> bool {
    let relation = SigmaRelation::RECEIVE;
    let mut shape = RelationShape::new(relation, PrefixMode::Absorbed);
    shape.core30_candidate = true;
    Inline {
        step: step.clone(),
        native: step.evaluate(relation),
        statement,
        variant: Variant::Receive,
        relation,
        measured_shape: Some(shape),
        known: true,
    }
    .accepts()
}

#[test]
#[ignore = "measure the A1 own-Receive transition alone at k16 on its four-bus lane"]
fn candidate_lean_receive_own_transition_geometry() {
    let mut baseline = Inline::receive(51, Variant::Receive);
    let mut shape = RelationShape::new(SigmaRelation::RECEIVE, PrefixMode::Absorbed);
    baseline.measured_shape = Some(shape);
    assert!(baseline.accepts());
    let geometry = |circuit: &Inline| {
        let synthesized = synthesize(&circuit.without_witnesses(), 16, None).unwrap();
        let columns = synthesized.tables.advice_assigned();
        let rows = columns
            .iter()
            .filter_map(|col| col.iter().rposition(|used| *used))
            .max()
            .map_or(0, |last| last + 1);
        let cells: usize = columns
            .iter()
            .map(|col| col.iter().filter(|used| **used).count())
            .sum();
        (rows, cells)
    };
    let (ordinary_rows, ordinary_cells) = geometry(&baseline);
    shape.core30_candidate = true;
    let candidate = Inline {
        measured_shape: Some(shape),
        ..baseline
    };
    let (candidate_rows, candidate_cells) = geometry(&candidate);
    // A known candidate must supply the private credential root from its predecessor.
    assert!(synthesize(&candidate, 16, None).is_err());
    let core29_statement = candidate.statement;
    let mut opened = candidate;
    let root = Fp::from(0x5eed_c030_u64);
    opened.step.core30_prior_root = Some(root);
    opened.native = opened.step.evaluate(opened.relation);
    opened.statement = opened.native.statement;
    assert!(opened.accepts());
    let headers: Vec<_> = (0..26)
        .filter(|&i| core29_statement[i] != opened.statement[i])
        .collect();
    assert_eq!(headers, [14, 15]);
    for i in headers {
        let mut mismatched = opened.clone();
        mismatched.statement[i] = core29_statement[i];
        let strict = check_circuit(&mismatched, 16, &[], CheckMode::Strict).unwrap();
        assert!(!strict.is_satisfied());
    }
    let known = synthesize(&opened, 16, None).unwrap();
    let blank = synthesize(&opened.without_witnesses(), 16, None).unwrap();
    assert_eq!(known.tables.fixed(), blank.tables.fixed());
    assert_eq!(known.tables.permutation(), blank.tables.permutation());
    assert_eq!(
        known.tables.advice_assigned(),
        blank.tables.advice_assigned()
    );

    // Change both host digest references to match the new private root. Only the
    // constraints tying the recomputed heads to the original statement can reject.
    opened.step.core30_prior_root = Some(root + Fp::ONE);
    opened.native = opened.step.evaluate(opened.relation);
    let strict = check_circuit(&opened, 16, &[], CheckMode::Strict).unwrap();
    assert!(!strict.is_satisfied());
    opened.statement = opened.native.statement;
    assert!(opened.accepts());
    println!(
        "CORE30_RECEIVE_A1_TRANSITION_GEOMETRY k=16 tagged_buses=4 core29_rows={ordinary_rows} core29_advice_cells={ordinary_cells} core30_blank_rows={candidate_rows} core30_blank_advice_cells={candidate_cells} full_a_stage=false candidate_witness_admitted=true"
    );
}
