//! Independent original-selector algorithm, CS/VK identity and tiny seeded proof comparisons.
//!
//! Prepared source only: these tests require the released Axiom build and runtime lanes.

use super::*;
use crate::{
    SerdeCurveAffine, SerdeFormat, SerdePrimeField,
    circuit::{Layouter, SimpleFloorPlanner, Value},
    halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq},
    plonk::{ProvingKey, VerifyingKey, create_proof, keygen_pk2, verify_proof},
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
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use std::io;

// Test-only reference copied from pinned parent 20ab3674598d78dc before the shared refactor.
// The two algorithm bodies are unchanged and never call the new shared planner/builder.
mod original {
    use super::*;
    #[derive(Clone)]
    pub(super) struct SelectorDescription {
        pub(super) selector: usize,
        pub(super) activations: Vec<bool>,
        pub(super) max_degree: usize,
    }
    pub(super) struct SelectorAssignment<F> {
        pub(super) selector: usize,
        pub(super) combination_index: usize,
        pub(super) expression: Expression<F>,
    }
    fn plan(selectors: &[SelectorDescription], max_degree: usize) -> Vec<Vec<usize>> {
        if selectors.is_empty() {
            return vec![];
        }

        // The length of all provided selectors must be the same.
        let n = selectors[0].activations.len();
        assert!(
            selectors
                .iter()
                .all(|selector| selector.activations.len() == n)
        );

        // Complex selectors (and unused selectors) cannot participate in a combination.
        let mut combinations = selectors
            .iter()
            .enumerate()
            .filter_map(|(index, selector)| (selector.max_degree == 0).then_some(vec![index]))
            .collect::<Vec<_>>();
        let simple = selectors
            .iter()
            .enumerate()
            .filter_map(|(index, selector)| (selector.max_degree != 0).then_some(index))
            .collect::<Vec<_>>();

        // Compute the lower-triangular exclusion matrix for simple selectors. Two selectors that are
        // active on the same row cannot share a fixed column.
        let mut exclusion_matrix = (0..simple.len())
            .map(|i| vec![false; i])
            .collect::<Vec<_>>();
        for (i, &selector_index) in simple.iter().enumerate() {
            let rows = &selectors[selector_index].activations;
            for (j, &other_index) in simple.iter().enumerate().take(i) {
                if rows
                    .iter()
                    .zip(selectors[other_index].activations.iter())
                    .any(|(left, right)| left & right)
                {
                    exclusion_matrix[i][j] = true;
                }
            }
        }

        let mut added = vec![false; simple.len()];
        for (i, &selector_index) in simple.iter().enumerate() {
            if added[i] {
                continue;
            }
            added[i] = true;
            let selector = &selectors[selector_index];
            assert!(selector.max_degree <= max_degree);
            // Omit the virtual selector's own degree; it is replaced by the combination expression.
            let mut d = selector.max_degree - 1;
            let mut combination = vec![selector_index];
            let mut combination_added = vec![i];

            'try_selectors: for (j, &candidate_index) in simple.iter().enumerate().skip(i + 1) {
                if d + combination.len() == max_degree {
                    break 'try_selectors;
                }
                if added[j] {
                    continue 'try_selectors;
                }
                for &member in &combination_added {
                    if exclusion_matrix[j][member] {
                        continue 'try_selectors;
                    }
                }

                let candidate = &selectors[candidate_index];
                let new_d = std::cmp::max(d, candidate.max_degree - 1);
                if new_d + combination.len() + 1 > max_degree {
                    continue 'try_selectors;
                }

                d = new_d;
                combination.push(candidate_index);
                combination_added.push(j);
                added[j] = true;
            }
            combinations.push(combination);
        }

        combinations
    }
    pub(super) fn process<F: Field, E>(
        selectors: Vec<SelectorDescription>,
        max_degree: usize,
        mut allocate_fixed_column: E,
    ) -> (Vec<Vec<F>>, Vec<SelectorAssignment<F>>)
    where
        E: FnMut() -> Expression<F>,
    {
        let combinations = plan(&selectors, max_degree);
        let n = selectors
            .first()
            .map_or(0, |selector| selector.activations.len());
        let mut combination_assignments = vec![];
        let mut selector_assignments = vec![];
        for combination in combinations {
            // Now, compute the selector and combination assignments.
            let mut combination_assignment = vec![F::ZERO; n];
            let combination_len = combination.len();
            let combination_index = combination_assignments.len();
            let query = allocate_fixed_column();

            let mut assigned_root = F::ONE;
            selector_assignments.extend(combination.into_iter().map(|selector_index| {
                let selector = &selectors[selector_index];
                // Compute the expression for substitution. This produces an expression of the
                // form
                //     q * Prod[i = 1..=combination_len, i != assigned_root](i - q)
                //
                // which is non-zero only on rows where `combination_assignment` is set to
                // `assigned_root`. In particular, rows set to 0 correspond to all selectors
                // being disabled.
                let mut expression = query.clone();
                let mut root = F::ONE;
                for _ in 0..combination_len {
                    if root != assigned_root {
                        expression = expression * (Expression::Constant(root) - query.clone());
                    }
                    root += F::ONE;
                }

                // Update the combination assignment
                for (combination, selector) in combination_assignment
                    .iter_mut()
                    .zip(selector.activations.iter())
                {
                    // This will not overwrite another selector's activations because
                    // we have ensured that selectors are disjoint.
                    if *selector {
                        *combination = assigned_root;
                    }
                }

                assigned_root += F::ONE;

                SelectorAssignment {
                    selector: selector.selector,
                    combination_index,
                    expression,
                }
            }));
            combination_assignments.push(combination_assignment);
        }

        (combination_assignments, selector_assignments)
    }
}

fn fake_query<F: Field>(index: usize) -> Expression<F> {
    Expression::Fixed(FixedQuery {
        index: Some(index + 23),
        column_index: index + 11,
        rotation: Rotation::cur(),
    })
}

fn original_plan_comparison<F: Field>() {
    for rows in [0, 1, 7, 8, 17, 64] {
        for count in [0, 1, 3, 7] {
            for max_degree in [3, 5, 7] {
                for pattern in 0..6 {
                    let selectors = (0..count)
                        .map(|column| {
                            let activations = (0..rows)
                                .map(|row| match pattern {
                                    0 => false,
                                    1 => true,
                                    2 => row % count == column,
                                    3 => (row * 7 + column * 3) % 11 < 3,
                                    4 => row % 4 == column % 4,
                                    _ => (row + column) % 2 == 0,
                                })
                                .collect::<Vec<_>>();
                            let max_degree = match column % 4 {
                                0 => 0,
                                1 => 1,
                                2 => max_degree,
                                _ => 2,
                            };
                            original::SelectorDescription {
                                selector: column,
                                activations,
                                max_degree,
                            }
                        })
                        .collect::<Vec<_>>();
                    let source = selectors
                        .iter()
                        .map(|v| v.activations.clone())
                        .collect::<Vec<_>>();
                    let allocations = selectors
                        .iter()
                        .map(|v| {
                            (
                                v.activations.as_ptr(),
                                v.activations.len(),
                                v.activations.capacity(),
                            )
                        })
                        .collect::<Vec<_>>();
                    let mut expected_columns = 0;
                    let (expected_polys, expected_assignments) =
                        original::process::<F, _>(selectors.clone(), max_degree, || {
                            let query = fake_query(expected_columns);
                            expected_columns += 1;
                            query
                        });
                    let expected = expected_assignments
                        .iter()
                        .map(|v| {
                            (
                                v.selector,
                                v.combination_index,
                                format!("{:?}", v.expression),
                            )
                        })
                        .collect::<Vec<_>>();
                    let dense_descriptions = selectors
                        .iter()
                        .map(|v| compress_selectors::SelectorDescription {
                            selector: v.selector,
                            activations: v.activations.clone(),
                            max_degree: v.max_degree,
                        })
                        .collect::<Vec<_>>();
                    let mut dense_columns = 0;
                    let (polys, assignments) = compress_selectors::process::<F, _>(
                        dense_descriptions.clone(),
                        max_degree,
                        || {
                            let query = fake_query(dense_columns);
                            dense_columns += 1;
                            query
                        },
                    );
                    assert_eq!(polys, expected_polys);
                    assert_eq!(
                        assignments
                            .iter()
                            .map(|v| (
                                v.selector,
                                v.combination_index,
                                format!("{:?}", v.expression)
                            ))
                            .collect::<Vec<_>>(),
                        expected
                    );
                    let borrowed = selectors
                        .iter()
                        .map(|v| compress_selectors::SelectorDescriptionRef {
                            selector: v.selector,
                            activations: v.activations.as_slice(),
                            max_degree: v.max_degree,
                        })
                        .collect::<Vec<_>>();
                    let mut metadata_columns = 0;
                    let assignments = compress_selectors::process_without_polynomials::<F, _>(
                        &borrowed,
                        max_degree,
                        || {
                            let query = fake_query(metadata_columns);
                            metadata_columns += 1;
                            query
                        },
                    );
                    assert_eq!(
                        assignments
                            .iter()
                            .map(|v| (
                                v.selector,
                                v.combination_index,
                                format!("{:?}", v.expression)
                            ))
                            .collect::<Vec<_>>(),
                        expected
                    );
                    assert_eq!(
                        (dense_columns, metadata_columns),
                        (expected_columns, expected_columns)
                    );
                    assert_eq!(
                        compress_selectors::combination_count(&dense_descriptions, max_degree),
                        expected_columns
                    );
                    if rows != 0 {
                        let mut modes = FixedColumnModeCounts::default();
                        for poly in &expected_polys {
                            let mut column = FixedColumnModeAccumulator::new();
                            for value in poly {
                                column.observe(Assigned::Trivial(*value), 1);
                            }
                            modes.add(column.finish());
                        }
                        assert_eq!(
                            compress_selectors::combination_modes::<F>(
                                &dense_descriptions,
                                max_degree
                            ),
                            modes
                        );
                    }
                    assert_eq!(
                        selectors
                            .iter()
                            .map(|v| v.activations.clone())
                            .collect::<Vec<_>>(),
                        source
                    );
                    assert_eq!(
                        selectors
                            .iter()
                            .map(|v| (
                                v.activations.as_ptr(),
                                v.activations.len(),
                                v.activations.capacity()
                            ))
                            .collect::<Vec<_>>(),
                        allocations
                    );
                }
            }
        }
    }
}

#[test]
fn fp_selector_metadata_matches_pinned_original_plan_and_expression_order() {
    original_plan_comparison::<Fp>();
}
#[test]
fn fq_selector_metadata_matches_pinned_original_plan_and_expression_order() {
    original_plan_comparison::<Fq>();
}

#[derive(Clone)]
struct MetadataCircuit;
#[derive(Clone, Copy)]
struct MetadataConfig {
    left: Column<Advice>,
    right: Column<Advice>,
    first: Selector,
    second: Selector,
    complex: Selector,
}
impl<F: Field + From<u64>> Circuit<F> for MetadataCircuit {
    type Config = MetadataConfig;
    type FloorPlanner = SimpleFloorPlanner;
    #[cfg(feature = "circuit-params")]
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self
    }
    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let left = meta.advice_column();
        let right = meta.advice_column();
        let first = meta.selector();
        let second = meta.selector();
        let complex = meta.complex_selector();
        let _unused = meta.selector();
        meta.enable_equality(left);
        meta.enable_equality(right);
        for selector in [first, second, complex] {
            meta.create_gate("selector metadata equality", |meta| {
                let q = meta.query_selector(selector);
                let left = meta.query_advice(left, Rotation::cur());
                let right = meta.query_advice(right, Rotation::cur());
                vec![q * (left - right)]
            });
        }
        MetadataConfig {
            left,
            right,
            first,
            second,
            complex,
        }
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        layouter.assign_region(
            || "disjoint simple and overlapping complex selectors",
            |mut region| {
                for row in 0..4 {
                    if row % 2 == 0 {
                        config.first.enable(&mut region, row)?;
                    } else {
                        config.second.enable(&mut region, row)?;
                    }
                    if row == 0 || row == 3 {
                        config.complex.enable(&mut region, row)?;
                    }
                    let left = region.assign_advice(
                        config.left,
                        row,
                        Value::known(F::from(row as u64 + 7)),
                    );
                    left.copy_advice(&mut region, config.right, row);
                }
                Ok(())
            },
        )
    }
}

fn cs_and_vk_identity<C: SerdeCurveAffine>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let mut configured = ConstraintSystem::<C::Scalar>::default();
    MetadataCircuit::configure(&mut configured);
    for rows in [0, 1, 9, 64] {
        for pattern in 0..4 {
            let selectors = (0..configured.num_selectors)
                .map(|column| {
                    (0..rows)
                        .map(|row| match pattern {
                            0 => false,
                            1 => true,
                            2 => row % 4 == column,
                            _ => (row + 2 * column) % 3 == 0,
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let source = selectors.clone();
            let (dense, _) = configured.clone().compress_selectors(selectors.clone());
            let metadata = configured
                .clone()
                .compress_selectors_without_polynomials(&selectors);
            assert_eq!(
                format!("{:?}", dense.pinned()),
                format!("{:?}", metadata.pinned())
            );
            assert_eq!(dense.selector_map, metadata.selector_map);
            assert_eq!(dense.fixed_queries, metadata.fixed_queries);
            assert_eq!(dense.num_fixed_columns, metadata.num_fixed_columns);
            assert_eq!(selectors, source);
        }
    }
    let params = ParamsIPA::<C>::new(6);
    for compressed in [false, true] {
        let key = keygen_pk2(&params, &MetadataCircuit, compressed).unwrap();
        let vk_bytes = key.get_vk().to_bytes(SerdeFormat::Processed);
        let mut framed = vk_bytes.clone();
        framed.extend_from_slice(&[0x25, 0x94]);
        let mut input = io::Cursor::new(&framed);
        let checked = VerifyingKey::<C>::read_checked::<_, MetadataCircuit>(
            &mut input,
            SerdeFormat::Processed,
            6,
            #[cfg(feature = "circuit-params")]
            (),
        )
        .unwrap();
        assert_eq!(input.position() as usize, vk_bytes.len());
        assert_eq!(checked.to_bytes(SerdeFormat::Processed), vk_bytes);
        assert_eq!(checked.transcript_repr(), key.get_vk().transcript_repr());
        assert_eq!(
            format!("{:?}", checked.pinned()),
            format!("{:?}", key.get_vk().pinned())
        );
        let key_bytes = key.to_bytes(SerdeFormat::Processed);
        let mut input = io::Cursor::new(&key_bytes);
        let restored = ProvingKey::<C>::read_checked::<_, MetadataCircuit>(
            &mut input,
            SerdeFormat::Processed,
            6,
            #[cfg(feature = "circuit-params")]
            (),
        )
        .unwrap();
        assert_eq!(input.position() as usize, key_bytes.len());
        assert_eq!(restored.to_bytes(SerdeFormat::Processed), key_bytes);
        let create = |pk: &ProvingKey<C>| {
            let columns: &[&[C::Scalar]] = &[];
            let instances = [columns];
            let mut transcript = Blake2bWrite::<_, _, Challenge255<_>>::init(Vec::new());
            create_proof::<IPACommitmentScheme<C>, ProverIPA<C>, _, _, _, _>(
                &params,
                pk,
                &[MetadataCircuit],
                &instances,
                ChaCha20Rng::from_seed([163; 32]),
                &mut transcript,
            )
            .unwrap();
            transcript.finalize()
        };
        let original_proof = create(&key);
        let restored_proof = create(&restored);
        assert_eq!(original_proof, restored_proof);
        let columns: &[&[C::Scalar]] = &[];
        let instances = [columns];
        let mut transcript = Blake2bRead::<_, _, Challenge255<_>>::init(&restored_proof[..]);
        assert!(
            verify_proof::<IPACommitmentScheme<C>, VerifierIPA<C>, _, _, _>(
                &params,
                &checked,
                SingleStrategy::new(&params),
                &instances,
                &mut transcript,
            )
            .is_ok()
        );
    }
}

#[test]
fn eq_selector_metadata_preserves_cs_processed_keys_and_seeded_proof() {
    cs_and_vk_identity::<EqAffine>();
}
#[test]
fn ep_selector_metadata_preserves_cs_processed_keys_and_seeded_proof() {
    cs_and_vk_identity::<EpAffine>();
}

#[test]
fn metadata_selector_preflights_before_fixed_allocation() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let left = [true, false];
    let right = [false];
    let shapes = [
        vec![
            compress_selectors::SelectorDescriptionRef {
                selector: 0,
                activations: &left,
                max_degree: 1,
            },
            compress_selectors::SelectorDescriptionRef {
                selector: 1,
                activations: &right,
                max_degree: 1,
            },
        ],
        vec![compress_selectors::SelectorDescriptionRef {
            selector: 0,
            activations: &left,
            max_degree: 7,
        }],
    ];
    for descriptors in shapes {
        let mut allocated = 0;
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                compress_selectors::process_without_polynomials::<Fp, _>(&descriptors, 3, || {
                    allocated += 1;
                    fake_query(allocated)
                })
            }))
            .is_err()
        );
        assert_eq!(allocated, 0);
    }
    let mut cs = ConstraintSystem::<Fp>::default();
    MetadataCircuit::configure(&mut cs);
    assert!(
        catch_unwind(AssertUnwindSafe(
            || cs.compress_selectors_without_polynomials(&[])
        ))
        .is_err()
    );
}
