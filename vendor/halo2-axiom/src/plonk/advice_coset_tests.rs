//! Advice-buffer ownership, coset inverse and exact quotient/proof equivalence regressions.

use super::*;
use crate::{
    circuit::{Layouter, SimpleFloorPlanner, Value},
    helpers::{SerdeCurveAffine, SerdePrimeField},
    plonk::{
        Advice, Circuit, Column, Error, Instance, Selector, TableColumn, create_proof,
        create_proof_consuming, keygen_pk2, verify_proof,
    },
    poly::{
        VerificationStrategy as _,
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
use ff::FromUniformBytes;
use halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

fn horner<F: Field>(coefficients: &[F], point: F) -> F {
    coefficients
        .iter()
        .rev()
        .fold(F::ZERO, |accumulator, coefficient| {
            accumulator * point + coefficient
        })
}

#[test]
fn advice_coset_parts_roundtrip_both_pasta_fields_and_preserve_scalar_buffers() {
    fn check<F: WithSmallOrderMulGroup<3>>() {
        for k in [3, 5] {
            for degree in [3, 4, 7, 9] {
                let domain = EvaluationDomain::<F>::new(degree, k);
                let rows = domain.get_n() as usize;
                let parts = domain.extended_len() / rows;
                let mut factor = F::ONE;
                for part in 0..parts {
                    let original = (0..rows)
                        .map(|row| F::from(((row + 1) * (row + 3) + 19 * part + 7) as u64))
                        .collect::<Vec<_>>();
                    let coefficients = domain.coeff_from_vec(original.clone());
                    let address = coefficients.values.as_ptr();
                    let values = domain.coeff_to_extended_part(coefficients, factor);
                    assert_eq!(values.values.as_ptr(), address);
                    for row in 0..rows {
                        for rotation in [-7, -2, -1, 0, 1, 3, 8, rows as i32 + 2] {
                            let rotated = (row as i32 + rotation).rem_euclid(rows as i32) as usize;
                            let point =
                                F::ZETA * factor * domain.get_omega().pow_vartime([rotated as u64]);
                            assert_eq!(values[rotated], horner(&original, point));
                        }
                    }
                    let restored = domain.extended_part_to_coeff(values, factor);
                    assert_eq!(restored.values.as_ptr(), address);
                    assert_eq!(restored.values, original);
                    factor *= domain.get_extended_omega();
                }
            }
        }
    }
    check::<Fp>();
    check::<Fq>();
}

#[test]
fn advice_coset_inverse_rejects_wrong_domain_length_and_zero_factor() {
    fn check<F: WithSmallOrderMulGroup<3>>() {
        let domain = EvaluationDomain::<F>::new(7, 3);
        let mut short = domain.empty_lagrange();
        short.values.pop();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                domain.extended_part_to_coeff(short, F::ONE)
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                domain.extended_part_to_coeff(domain.empty_lagrange(), F::ZERO)
            }))
            .is_err()
        );
    }
    check::<Fp>();
    check::<Fq>();
}

#[derive(Clone)]
struct RotationCircuit<F: Field, const ARGUMENTS: bool, const LOOKUP: bool = false> {
    value: Value<F>,
}

#[derive(Clone, Copy, Debug)]
struct RotationConfig {
    a: Column<Advice>,
    b: Column<Advice>,
    copied: Column<Advice>,
    q: Selector,
    table: Option<TableColumn>,
    instance: Option<Column<Instance>>,
}

impl<F: Field + From<u64>, const ARGUMENTS: bool, const LOOKUP: bool> Circuit<F>
    for RotationCircuit<F, ARGUMENTS, LOOKUP>
{
    type Config = RotationConfig;
    type FloorPlanner = SimpleFloorPlanner;
    #[cfg(feature = "circuit-params")]
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self {
            value: Value::unknown(),
        }
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let a = meta.advice_column();
        let b = meta.advice_column();
        let copied = meta.advice_column();
        let q = meta.complex_selector();
        meta.create_gate(
            "advice cosets preserve five rotations and degree-seven order",
            |meta| {
                let q = meta.query_selector(q);
                let sum = [-2, -1, 0, 1, 3]
                    .into_iter()
                    .fold(Expression::Constant(F::ZERO), |sum, rotation| {
                        sum + meta.query_advice(a, Rotation(rotation))
                    });
                let delta = sum - meta.query_advice(b, Rotation::cur());
                let square = delta.clone() * delta.clone();
                let sixth = square.clone() * square.clone() * square;
                vec![
                    q.clone() * delta,
                    q.clone() * sixth,
                    q * (meta.query_advice(copied, Rotation::cur())
                        - meta.query_advice(a, Rotation::cur())),
                ]
            },
        );
        let (table, instance) = if ARGUMENTS {
            let table = if LOOKUP {
                let table = meta.lookup_table_column();
                meta.lookup("coset advice also participates in a range lookup", |meta| {
                    vec![(
                        meta.query_selector(q) * meta.query_advice(a, Rotation::cur()),
                        table,
                    )]
                });
                Some(table)
            } else {
                None
            };
            let instance = meta.instance_column();
            meta.enable_equality(a);
            meta.enable_equality(b);
            meta.enable_equality(copied);
            meta.enable_equality(instance);
            (table, Some(instance))
        } else {
            (None, None)
        };
        RotationConfig {
            a,
            b,
            copied,
            q,
            table,
            instance,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        if let Some(table_column) = config.table {
            layouter.assign_table(
                || "small range table",
                |mut table| {
                    for row in 0..16 {
                        table.assign_cell(
                            || "range element",
                            table_column,
                            row,
                            || Value::known(F::from(row as u64)),
                        )?;
                    }
                    Ok(())
                },
            )?;
        }
        let first = layouter.assign_region(
            || "overlapping rotated advice",
            |mut region| {
                let mut first = None;
                for row in 0..8 {
                    if (2..=4).contains(&row) {
                        config.q.enable(&mut region, row)?;
                    }
                    let value = self.value.map(|value| value + F::from(row as u64));
                    let a = region.assign_advice(config.a, row, value);
                    if row == 0 {
                        first = Some(a.cell());
                    }
                    if ARGUMENTS {
                        a.copy_advice(&mut region, config.copied, row);
                    } else {
                        region.assign_advice(config.copied, row, value);
                    }
                    // a[r-2]+a[r-1]+a[r]+a[r+1]+a[r+3] = 5*w + 5*r + 1.
                    region.assign_advice(
                        config.b,
                        row,
                        self.value
                            .map(|value| value * F::from(5) + F::from((5 * row + 1) as u64)),
                    );
                }
                Ok(first.expect("eight fixture rows"))
            },
        )?;
        if let Some(instance) = config.instance {
            layouter.constrain_instance(first, instance, 0);
        }
        Ok(())
    }
}

fn quotient_equality<C: SerdeCurveAffine>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(6);
    let circuit = RotationCircuit::<C::Scalar, false> {
        value: Value::known(C::Scalar::from(3)),
    };
    let pk = keygen_pk2(&params, &circuit, true).unwrap();
    let domain = pk.get_vk().get_domain();
    assert_eq!(domain.extended_len() / domain.get_n() as usize, 8);
    let original = (0..2)
        .map(|bank| {
            (0..3)
                .map(|column| {
                    domain.coeff_from_vec(
                        (0..domain.get_n() as usize)
                            .map(|row| {
                                C::Scalar::from((101 * bank + 19 * column + row * row + 1) as u64)
                            })
                            .collect(),
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let pointers = original
        .iter()
        .map(|bank| {
            bank.iter()
                .map(|poly| poly.values.as_ptr())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let borrowed = original.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let instances: [&[Polynomial<C::Scalar, Coeff>]; 2] = [&[], &[]];
    let lookups = vec![vec![], vec![]];
    let permutations = vec![
        permutation::prover::Committed { sets: vec![] },
        permutation::prover::Committed { sets: vec![] },
    ];
    let [y, beta, gamma, theta] = [2, 3, 5, 7].map(C::Scalar::from);
    let expected = pk.ev.evaluate_h(
        &pk,
        &borrowed,
        &instances,
        &[],
        y,
        beta,
        gamma,
        theta,
        &lookups,
        &permutations,
        false,
    );
    assert!(expected.iter().any(|value| !bool::from(value.is_zero())));
    let saved = original
        .iter()
        .map(|bank| {
            bank.iter()
                .map(|poly| poly.values.clone())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let (actual, restored) = pk.ev.evaluate_h_consuming_advice(
        &pk,
        original,
        &instances,
        &[],
        y,
        beta,
        gamma,
        theta,
        &lookups,
        &permutations,
        true,
    );
    assert_eq!(actual.values, expected.values);
    assert_eq!(restored.len(), saved.len());
    assert_eq!(restored.len(), pointers.len());
    for ((bank, expected), pointers) in restored.iter().zip(saved).zip(pointers) {
        assert_eq!(bank.len(), expected.len());
        assert_eq!(bank.len(), pointers.len());
        for ((poly, expected), pointer) in bank.iter().zip(expected).zip(pointers) {
            assert_eq!(poly.values, expected);
            assert_eq!(poly.values.as_ptr(), pointer);
        }
    }
}

#[test]
fn advice_coset_owned_quotient_matches_borrowed_and_restores_two_banks() {
    quotient_equality::<EqAffine>();
    quotient_equality::<EpAffine>();
}

fn proof_equality<C: SerdeCurveAffine, const LOOKUP: bool>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(6);
    for value in [3, 5] {
        let value = C::Scalar::from(value);
        let circuit = RotationCircuit::<C::Scalar, true, LOOKUP> {
            value: Value::known(value),
        };
        let pk = keygen_pk2(&params, &circuit, true).unwrap();
        let original_vk = pk.get_vk().to_bytes(crate::SerdeFormat::Processed);
        assert_eq!(pk.get_vk().get_domain().extended_len() / 64, 8);
        assert_eq!(!pk.get_vk().cs().lookups().is_empty(), LOOKUP);
        assert!(!pk.get_vk().cs().permutation().get_columns().is_empty());
        let public = [value];
        let columns = [public.as_slice()];
        let instances = [columns.as_slice()];
        let mut borrowed = Blake2bWrite::<_, C, Challenge255<C>>::init(Vec::new());
        create_proof::<IPACommitmentScheme<C>, ProverIPA<C>, _, _, _, _>(
            &params,
            &pk,
            &[circuit.clone()],
            &instances,
            ChaCha20Rng::from_seed([91; 32]),
            &mut borrowed,
        )
        .unwrap();
        let borrowed = borrowed.finalize();
        let mut consuming = Blake2bWrite::<_, C, Challenge255<C>>::init(Vec::new());
        let vk = create_proof_consuming::<IPACommitmentScheme<C>, ProverIPA<C>, _, _, _, _>(
            &params,
            pk,
            circuit,
            &instances,
            ChaCha20Rng::from_seed([91; 32]),
            &mut consuming,
        )
        .unwrap();
        let consuming = consuming.finalize();
        if !LOOKUP {
            assert_eq!(
                consuming, borrowed,
                "rotations, copy permutation and eight coset parts"
            );
        }
        // Varied lookup values are permuted by the existing randomized HashMap
        // iteration, before quotient evaluation. Their proof bytes need not match;
        // both complete proofs and rejection paths remain independently checked.
        assert_eq!(vk.to_bytes(crate::SerdeFormat::Processed), original_vk);
        let verify = |bytes: &[u8], public: C::Scalar| {
            let public = [public];
            let columns = [public.as_slice()];
            let instances = [columns.as_slice()];
            let mut transcript = Blake2bRead::<_, C, Challenge255<C>>::init(bytes);
            verify_proof::<IPACommitmentScheme<C>, VerifierIPA<C>, _, _, _>(
                &params,
                &vk,
                SingleStrategy::new(&params),
                &instances,
                &mut transcript,
            )
        };
        for proof in [borrowed, consuming] {
            verify(&proof, value).unwrap();
            assert!(verify(&proof, value + C::Scalar::ONE).is_err());
            let mut corrupted = proof;
            let last = corrupted.len() - 1;
            corrupted[last] ^= 1;
            assert!(verify(&corrupted, value).is_err());
        }
    }
}

#[test]
fn advice_coset_consuming_proofs_match_borrowed_for_both_pasta_curves() {
    proof_equality::<EqAffine, false>();
    proof_equality::<EpAffine, false>();
    proof_equality::<EqAffine, true>();
    proof_equality::<EpAffine, true>();
}
