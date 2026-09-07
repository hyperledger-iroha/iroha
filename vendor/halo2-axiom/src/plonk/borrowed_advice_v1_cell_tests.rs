//! V1 physical-cell and owned-advice regressions for compact gadget assignments.

use super::*;
use crate::{
    circuit::{Layouter, SimpleFloorPlanner, V1},
    dev::MockProver,
    plonk::{keygen_pk, keygen_vk, verifier::verify_proof},
    poly::{
        Rotation, VerificationStrategy as _,
        commitment::ParamsProver,
        ipa::{
            commitment::{IPACommitmentScheme, ParamsIPA},
            multiopen::{ProverIPA, VerifierIPA},
            strategy::SingleStrategy,
        },
    },
    transcript::{
        Blake2bRead, Blake2bWrite, Challenge255, TranscriptReadBuffer, TranscriptWriterBuffer,
    },
};
use halo2curves::pasta::{EpAffine, EqAffine};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use std::collections::BTreeMap;

#[derive(Clone)]
struct CellOnlyCircuit<F: Field> {
    value: F,
    compact: bool,
    bad_copy: bool,
    bad_gate: bool,
}

#[derive(Clone, Copy, Debug)]
struct CellOnlyConfig {
    advice: [Column<Advice>; 2],
    anchor: Column<Fixed>,
    selector: Selector,
}

impl<F: Field> Circuit<F> for CellOnlyCircuit<F> {
    type Config = CellOnlyConfig;
    type FloorPlanner = V1;
    #[cfg(feature = "circuit-params")]
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self {
            value: F::ZERO,
            ..self.clone()
        }
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let advice = [meta.advice_column(), meta.advice_column()];
        for column in advice {
            meta.enable_equality(column);
        }
        let anchor = meta.fixed_column();
        let selector = meta.selector();
        meta.create_gate("cell-only advice values remain equal", |meta| {
            let a = meta.query_advice(advice[0], Rotation::cur());
            let b = meta.query_advice(advice[1], Rotation::cur());
            vec![meta.query_selector(selector) * (a - b)]
        });
        CellOnlyConfig {
            advice,
            anchor,
            selector,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        // Both regions occupy the same columns. The larger prefix is placed first,
        // so the two-row region's fixed anchors must contain a nonzero physical offset.
        let prefix = layouter.assign_region(
            || "four-row shared-column prefix",
            |mut region| {
                let mut last = None;
                for row in 0..4 {
                    region.assign_fixed(config.anchor, row, F::ZERO);
                    config.selector.enable(&mut region, row)?;
                    for column in config.advice {
                        let cell = if self.compact {
                            region.assign_advice_discarding_value(
                                column,
                                row,
                                Value::known(self.value),
                            )
                        } else {
                            region
                                .assign_advice(column, row, Value::known(self.value))
                                .cell()
                        };
                        if row == 3 && column == config.advice[0] {
                            last = Some(cell);
                        }
                    }
                }
                Ok(last.expect("prefix endpoint"))
            },
        )?;
        layouter.assign_region(
            || "relocated cell-only copies",
            |mut region| {
                let value = self.value + if self.bad_copy { F::ONE } else { F::ZERO };
                for row in 0..2 {
                    region.assign_fixed(config.anchor, row, F::ZERO);
                    config.selector.enable(&mut region, row)?;
                    for (index, column) in config.advice.into_iter().enumerate() {
                        let value = value
                            + if self.bad_gate && index == 1 {
                                F::ONE
                            } else {
                                F::ZERO
                            };
                        let cell = if self.compact {
                            region.assign_advice_discarding_value(column, row, Value::known(value))
                        } else {
                            region
                                .assign_advice(column, row, Value::known(value))
                                .cell()
                        };
                        if index == 0 {
                            region.constrain_equal(prefix, cell);
                        }
                    }
                }
                Ok(())
            },
        )
    }
}

struct OwnershipCapture<F: Field> {
    advice: Vec<OwnedAdviceColumn<F>>,
    values: BTreeMap<(Column<Any>, usize), F>,
    copies: Vec<(Column<Any>, usize, Column<Any>, usize)>,
    references: usize,
    discarded: usize,
}

impl<F: Field> OwnershipCapture<F> {
    fn record(
        &mut self,
        column: Column<Advice>,
        row: usize,
        value: Value<Assigned<F>>,
    ) -> Assigned<F> {
        let mut known = None;
        value.map(|value| {
            known = Some(value);
        });
        let value = known.expect("known compact-assignment fixture");
        self.values.insert((column.into(), row), value.evaluate());
        value
    }
}

impl<F: Field> Assignment<F> for OwnershipCapture<F> {
    fn enter_region<NR: Into<String>, N: FnOnce() -> NR>(&mut self, _: N) {}
    fn annotate_column<A: FnOnce() -> AR, AR: Into<String>>(&mut self, _: A, _: Column<Any>) {}
    fn exit_region(&mut self) {}
    fn enable_selector<A: FnOnce() -> AR, AR: Into<String>>(
        &mut self,
        _: A,
        _: &Selector,
        _: usize,
    ) -> Result<(), Error> {
        Ok(())
    }
    fn query_instance(&self, _: Column<Instance>, _: usize) -> Result<Value<F>, Error> {
        Err(Error::BoundsFailure)
    }
    fn assign_advice<'v>(
        &mut self,
        column: Column<Advice>,
        row: usize,
        to: Value<Assigned<F>>,
    ) -> Value<&'v Assigned<F>> {
        let value = self.record(column, row, to);
        *self.advice[column.index()]
            .get_mut_returning_reference(row)
            .expect("fixture row") = value;
        self.references += 1;
        // The fixture retains only .cell(); it never reads an Assigned reference.
        Value::unknown()
    }
    fn assign_advice_discarding_value(
        &mut self,
        column: Column<Advice>,
        row: usize,
        to: Value<Assigned<F>>,
    ) {
        let value = self.record(column, row, to);
        self.advice[column.index()]
            .assign_discarding_value(row, value)
            .expect("fixture row");
        self.discarded += 1;
    }
    fn assign_fixed(&mut self, column: Column<Fixed>, row: usize, value: Assigned<F>) {
        self.values.insert((column.into(), row), value.evaluate());
    }
    fn copy(&mut self, left: Column<Any>, left_row: usize, right: Column<Any>, right_row: usize) {
        self.copies.push((left, left_row, right, right_row));
    }
    fn fill_from_row(
        &mut self,
        _: Column<Fixed>,
        _: usize,
        _: Value<Assigned<F>>,
    ) -> Result<(), Error> {
        Ok(())
    }
    fn get_challenge(&self, _: Challenge) -> Value<F> {
        Value::unknown()
    }
    fn push_namespace<NR: Into<String>, N: FnOnce() -> NR>(&mut self, _: N) {}
    fn pop_namespace(&mut self, _: Option<String>) {}
}

fn capture<F: Field, P: FloorPlanner>(circuit: &CellOnlyCircuit<F>) -> OwnershipCapture<F> {
    let mut meta = ConstraintSystem::default();
    let config = CellOnlyCircuit::configure(&mut meta);
    let mut assignment = OwnershipCapture {
        advice: (0..meta.num_advice_columns())
            .map(|_| OwnedAdviceColumn::new(64))
            .collect(),
        values: BTreeMap::new(),
        copies: Vec::new(),
        references: 0,
        discarded: 0,
    };
    P::synthesize(&mut assignment, circuit, config, meta.constants().clone())
        .expect("actual V1 region placement and assignment");
    assignment
}

fn assert_v1_ownership<F: Field + WithSmallOrderMulGroup<3> + ff::FromUniformBytes<64> + Ord>() {
    let original = CellOnlyCircuit {
        value: F::from(7),
        compact: false,
        bad_copy: false,
        bad_gate: false,
    };
    let compact = CellOnlyCircuit {
        compact: true,
        ..original.clone()
    };
    let mut before = capture::<F, V1>(&original);
    let mut after = capture::<F, V1>(&compact);
    assert_eq!(
        before.values, after.values,
        "all physical advice and fixed values"
    );
    assert_eq!(before.copies, after.copies, "all physical copy constraints");
    assert!(
        after
            .copies
            .iter()
            .any(|(_, left, _, right)| left != right && *right >= 4)
    );
    assert_eq!((before.references, before.discarded), (12, 0));
    assert_eq!((after.references, after.discarded), (0, 12));
    for column in &after.advice {
        assert!(!column.reference_exposed);
        assert!(matches!(
            column.storage,
            OwnedAdviceStorage::Evaluated { .. }
        ));
    }
    let domain = EvaluationDomain::<F>::new(3, 6);
    for (before, after) in before.advice.iter_mut().zip(&mut after.advice) {
        assert_eq!(
            before.take_polynomial(&domain)[..],
            after.take_polynomial(&domain)[..]
        );
        assert!(
            matches!(after.storage, OwnedAdviceStorage::Empty),
            "compact allocation transferred"
        );
        assert!(
            matches!(before.storage, OwnedAdviceStorage::Assigned(_)),
            "referenced storage retained"
        );
    }
    for compact in [false, true] {
        let valid = CellOnlyCircuit {
            compact,
            ..original.clone()
        };
        MockProver::run(6, &valid, vec![])
            .expect("V1 valid compact copies")
            .assert_satisfied();
        for (bad_copy, bad_gate) in [(true, false), (false, true)] {
            let invalid = CellOnlyCircuit {
                bad_copy,
                bad_gate,
                ..valid.clone()
            };
            assert!(
                MockProver::run(6, &invalid, vec![])
                    .expect("invalid witness synthesis")
                    .verify()
                    .is_err()
            );
        }
    }
}

#[test]
fn v1_cell_only_assignment_preserves_physical_values_copies_and_transfers_storage() {
    assert_v1_ownership::<halo2curves::pasta::Fp>();
    assert_v1_ownership::<halo2curves::pasta::Fq>();
}

fn assert_v1_proof_equivalence<C: CurveAffine + crate::SerdeCurveAffine>()
where
    C::ScalarExt: WithSmallOrderMulGroup<3> + crate::SerdePrimeField + ff::FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(6);
    let original = CellOnlyCircuit {
        value: C::ScalarExt::from(7),
        compact: false,
        bad_copy: false,
        bad_gate: false,
    };
    let compact = CellOnlyCircuit {
        compact: true,
        ..original.clone()
    };
    let vk = keygen_vk(&params, &original).expect("V1 original VK");
    let pk = keygen_pk(&params, vk, &original).expect("V1 original PK");
    let compact_vk = keygen_vk(&params, &compact).expect("V1 compact VK");
    let compact_pk = keygen_pk(&params, compact_vk, &compact).expect("V1 compact PK");
    let pk_bytes = pk.to_bytes(crate::SerdeFormat::Processed);
    assert_eq!(pk_bytes, compact_pk.to_bytes(crate::SerdeFormat::Processed));
    let instances: [&[&[C::ScalarExt]]; 1] = [&[]];
    let seed = [103; 32];
    let mut before = Blake2bWrite::<_, _, Challenge255<_>>::init(Vec::new());
    super::borrowed_advice_reference::create_proof_reference::<
        IPACommitmentScheme<C>,
        ProverIPA<C>,
        _,
        _,
        _,
        _,
    >(
        &params,
        &pk,
        &[original],
        &instances,
        ChaCha20Rng::from_seed(seed),
        &mut before,
    )
    .expect("original assignment and borrowed prover");
    let mut after = Blake2bWrite::<_, _, Challenge255<_>>::init(Vec::new());
    create_proof::<IPACommitmentScheme<C>, ProverIPA<C>, _, _, _, _>(
        &params,
        &pk,
        &[compact],
        &instances,
        ChaCha20Rng::from_seed(seed),
        &mut after,
    )
    .expect("compact V1 assignment and borrowed advice");
    let proof = after.finalize();
    assert_eq!(
        proof,
        before.finalize(),
        "complete deterministic IPA transcript bytes"
    );
    let mut transcript = Blake2bRead::<_, _, Challenge255<_>>::init(&proof[..]);
    verify_proof::<IPACommitmentScheme<C>, VerifierIPA<C>, _, _, _>(
        &params,
        pk.get_vk(),
        SingleStrategy::new(&params),
        &instances,
        &mut transcript,
    )
    .expect("real compact V1 IPA proof");
    assert_eq!(pk.to_bytes(crate::SerdeFormat::Processed), pk_bytes);
}

#[test]
fn eq_v1_cell_only_assignments_preserve_real_ipa_proof_bytes() {
    assert_v1_proof_equivalence::<EqAffine>();
}
#[test]
fn ep_v1_cell_only_assignments_preserve_real_ipa_proof_bytes() {
    assert_v1_proof_equivalence::<EpAffine>();
}

#[test]
fn cell_only_poseidon_bus_out_of_order_assignments_release_storage() {
    fn check<F: Field + WithSmallOrderMulGroup<3>>() {
        let mut column = OwnedAdviceColumn::<F>::new(64);
        // Native Poseidon fills non-bridge zeros first, then writes endpoint bridges.
        // Out-of-order writes may require Assigned storage, but no reference escapes.
        for row in 12..64 {
            column.assign_discarding_value(row, Assigned::Zero).unwrap();
        }
        for row in 0..12 {
            column
                .assign_discarding_value(row, Assigned::Trivial(F::from(row as u64 + 1)))
                .unwrap();
        }
        assert!(!column.reference_exposed);
        assert!(matches!(column.storage, OwnedAdviceStorage::Assigned(_)));
        let domain = EvaluationDomain::<F>::new(3, 6);
        let polynomial = column.take_polynomial(&domain);
        for row in 0..64 {
            assert_eq!(
                polynomial[row],
                if row < 12 {
                    F::from(row as u64 + 1)
                } else {
                    F::ZERO
                }
            );
        }
        assert!(matches!(column.storage, OwnedAdviceStorage::Empty));
    }
    check::<halo2curves::pasta::Fp>();
    check::<halo2curves::pasta::Fq>();
}

#[test]
fn simple_floor_planner_cell_only_returns_exact_ordinary_coordinates() {
    fn check<F: ff::PrimeField>() {
        let original = CellOnlyCircuit {
            value: F::from(7),
            compact: false,
            bad_copy: false,
            bad_gate: false,
        };
        let compact = CellOnlyCircuit {
            compact: true,
            ..original.clone()
        };
        let before = capture::<F, SimpleFloorPlanner>(&original);
        let after = capture::<F, SimpleFloorPlanner>(&compact);
        assert_eq!(before.values, after.values);
        assert_eq!(before.copies, after.copies);
        assert_eq!((before.references, before.discarded), (12, 0));
        assert_eq!((after.references, after.discarded), (0, 12));
        assert!(after.advice.iter().all(|column| !column.reference_exposed));
    }
    check::<halo2curves::pasta::Fp>();
    check::<halo2curves::pasta::Fq>();
}
