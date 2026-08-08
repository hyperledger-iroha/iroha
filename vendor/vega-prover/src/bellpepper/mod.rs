// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT
// This file is part of the vega-prover project.
// See the LICENSE file in the project root for full license information.
// Source repository: https://github.com/Microsoft/vega-prover

//! Support for generating R1CS from Bellpepper
pub mod r1cs;
pub mod shape_cs;
pub mod solver;
pub mod test_r1cs;
pub mod test_shape_cs;

#[cfg(test)]
mod tests {
    use crate::{
        bellpepper::{
            solver::SatisfyingAssignment,
            test_r1cs::{TestVegaShape, TestVegaWitness},
            test_shape_cs::TestShapeCS,
        },
        provider::PallasHyraxEngine,
        traits::Engine,
    };
    use bellpepper_core::{ConstraintSystem, SynthesisError, num::AllocatedNum};
    use ff::PrimeField;

    fn synthesize_alloc_bit<Fr: PrimeField, CS: ConstraintSystem<Fr>>(
        cs: &mut CS,
    ) -> Result<(), SynthesisError> {
        // get two bits as input and check that they are indeed bits
        let a = AllocatedNum::alloc(cs.namespace(|| "a"), || Ok(Fr::ONE))?;
        let _ = a.inputize(cs.namespace(|| "a is input"));
        cs.enforce(
            || "check a is 0 or 1",
            |lc| lc + CS::one() - a.get_variable(),
            |lc| lc + a.get_variable(),
            |lc| lc,
        );
        let b = AllocatedNum::alloc(cs.namespace(|| "b"), || Ok(Fr::ONE))?;
        let _ = b.inputize(cs.namespace(|| "b is input"));
        cs.enforce(
            || "check b is 0 or 1",
            |lc| lc + CS::one() - b.get_variable(),
            |lc| lc + b.get_variable(),
            |lc| lc,
        );
        Ok(())
    }

    fn test_alloc_bit_with<E: Engine>() {
        // First create the shape
        let mut cs: TestShapeCS<E> = TestShapeCS::new();
        let _ = synthesize_alloc_bit(&mut cs);
        let (shape, ck, _vk) = cs.r1cs_shape().unwrap();

        // Now get the assignment
        let mut cs = SatisfyingAssignment::<E>::new();
        let _ = synthesize_alloc_bit(&mut cs);
        let (inst, witness) = cs.r1cs_instance_and_witness(&shape, &ck, true).unwrap();

        // Make sure that this is satisfiable
        assert!(shape.is_sat(&ck, &inst, &witness).is_ok());
    }

    #[test]
    fn test_alloc_bit() {
        crate::iroha_rng::with_deterministic_test_seed(0x10, || {
            test_alloc_bit_with::<PallasHyraxEngine>();
        });
    }

    // ------------------------------------------------------------
    // Multi-round circuit test
    // ------------------------------------------------------------
    use crate::{
        bellpepper::{
            r1cs::{MultiRoundVegaShape, MultiRoundVegaWitness},
            shape_cs::ShapeCS,
        },
        traits::{circuit::MultiRoundCircuit, transcript::TranscriptEngineTrait},
    };

    #[derive(Clone)]
    struct TwoRoundBitsCircuit;

    impl<E: Engine> MultiRoundCircuit<E> for TwoRoundBitsCircuit {
        fn num_challenges(&self, _round_index: usize) -> Result<usize, SynthesisError> {
            Ok(0)
        }

        fn rounds<CS: ConstraintSystem<E::Scalar>>(
            &self,
            cs: &mut CS,
            round_index: usize,
            _prior_round_vars: &[Vec<AllocatedNum<E::Scalar>>],
            _prev_challenges: &[Vec<AllocatedNum<E::Scalar>>],
            _challenges: Option<&[E::Scalar]>,
        ) -> Result<(Vec<AllocatedNum<E::Scalar>>, Vec<AllocatedNum<E::Scalar>>), SynthesisError>
        {
            // Allocate a single bit set to 1 and enforce it is boolean
            let bit = AllocatedNum::alloc(cs.namespace(|| format!("bit_{round_index}")), || {
                Ok(<E::Scalar as ff::Field>::ONE)
            })?;
            cs.enforce(
                || format!("bit_boolean_{round_index}"),
                |lc| lc + bit.get_variable(),
                |lc| lc + CS::one() - bit.get_variable(),
                |lc| lc,
            );
            Ok((vec![bit], vec![]))
        }

        fn num_rounds(&self) -> usize {
            2
        }

        fn commitment_width(&self) -> usize {
            32
        }
    }

    fn test_multiround_bits_with<E: Engine>() {
        let circuit = TwoRoundBitsCircuit;

        // Generate shape
        let (shape, ck, _vk) =
            <ShapeCS<E> as MultiRoundVegaShape<E>>::multiround_r1cs_shape(&circuit).unwrap();

        // Initialize witness state
        let mut state = SatisfyingAssignment::<E>::initialize_multiround_witness(&shape).unwrap();
        let mut transcript = <E as Engine>::TE::new(b"test");

        // Process each round
        let num_rounds = <TwoRoundBitsCircuit as MultiRoundCircuit<E>>::num_rounds(&circuit);
        for r in 0..num_rounds {
            let _ = SatisfyingAssignment::<E>::process_round(
                &mut state,
                &shape,
                &ck,
                &circuit,
                r,
                &mut transcript,
            )
            .unwrap();
        }

        // Finalize
        let (instance, witness) =
            SatisfyingAssignment::<E>::finalize_multiround_witness(&mut state, &shape).unwrap();

        // Convert to regular instance/shape for satisfiability check
        let regular_shape = shape.to_regular_shape();
        let regular_instance = instance.to_regular_instance().unwrap();

        let sat_res = regular_shape.is_sat(&ck, &regular_instance, &witness);
        assert!(sat_res.is_ok());
    }

    #[test]
    fn test_multiround_bits() {
        crate::iroha_rng::with_deterministic_test_seed(0x11, || {
            test_multiround_bits_with::<PallasHyraxEngine>();
        });
    }

    #[test]
    fn test_multiround_validate_rejects_length_mismatch() {
        crate::iroha_rng::with_deterministic_test_seed(0x12, || {
            type E = PallasHyraxEngine;
            let circuit = TwoRoundBitsCircuit;

            let (shape, ck, _vk) =
                <ShapeCS<E> as MultiRoundVegaShape<E>>::multiround_r1cs_shape(&circuit).unwrap();
            let mut state =
                SatisfyingAssignment::<E>::initialize_multiround_witness(&shape).unwrap();
            let mut transcript = <E as Engine>::TE::new(b"test");
            let num_rounds = <TwoRoundBitsCircuit as MultiRoundCircuit<E>>::num_rounds(&circuit);
            for r in 0..num_rounds {
                let _ = SatisfyingAssignment::<E>::process_round(
                    &mut state,
                    &shape,
                    &ck,
                    &circuit,
                    r,
                    &mut transcript,
                )
                .unwrap();
            }
            let (instance, _witness) =
                SatisfyingAssignment::<E>::finalize_multiround_witness(&mut state, &shape).unwrap();

            // the well-formed instance validates
            let mut t = <E as Engine>::TE::new(b"test");
            assert!(instance.validate(&shape, &mut t).is_ok());

            // an over-length public_values vector is rejected
            let mut bad = instance.clone();
            bad.public_values.push(<E as Engine>::Scalar::from(1u64));
            let mut t = <E as Engine>::TE::new(b"test");
            assert!(bad.validate(&shape, &mut t).is_err());

            // an extra per-round commitment is rejected
            let mut bad = instance.clone();
            let extra = bad.comm_w_per_round[0].clone();
            bad.comm_w_per_round.push(extra);
            let mut t = <E as Engine>::TE::new(b"test");
            assert!(bad.validate(&shape, &mut t).is_err());
        });
    }

    // ------------------------------------------------------------
    // Multi-round permutation circuit test
    // ------------------------------------------------------------
    use ff::Field;

    #[derive(Clone)]
    struct TwoRoundPermutationCircuit;

    impl<E: Engine> MultiRoundCircuit<E> for TwoRoundPermutationCircuit {
        fn num_challenges(&self, round_index: usize) -> Result<usize, SynthesisError> {
            if round_index >= 2 {
                return Err(SynthesisError::Unsatisfiable);
            }
            Ok(match round_index {
                0 => 1, // one challenges after the first round
                1 => 0, // no challenges in the second round
                _ => 0, // no challenges in other rounds
            })
        }

        fn rounds<CS: ConstraintSystem<E::Scalar>>(
            &self,
            cs: &mut CS,
            round_index: usize,
            prior_round_vars: &[Vec<AllocatedNum<E::Scalar>>],
            _prev_challenges: &[Vec<AllocatedNum<E::Scalar>>],
            challenges: Option<&[E::Scalar]>,
        ) -> Result<(Vec<AllocatedNum<E::Scalar>>, Vec<AllocatedNum<E::Scalar>>), SynthesisError>
        {
            match round_index {
                // ---------------- Round 0 ----------------
                0 => {
                    // Allocate six witness values (a..f). The first three (a,b,c) are a permutation of the
                    // second three (d,e,f). We use distinct constants 1,2,3 but shuffle the order.
                    let one = E::Scalar::from(1u64);
                    let two = E::Scalar::from(2u64);
                    let three = E::Scalar::from(3u64);

                    let a = AllocatedNum::alloc(cs.namespace(|| "a"), || Ok(one))?;
                    let b = AllocatedNum::alloc(cs.namespace(|| "b"), || Ok(two))?;
                    let c = AllocatedNum::alloc(cs.namespace(|| "c"), || Ok(three))?;

                    // Permuted copy
                    let d = AllocatedNum::alloc(cs.namespace(|| "d"), || Ok(three))?;
                    let e = AllocatedNum::alloc(cs.namespace(|| "e"), || Ok(one))?;
                    let f = AllocatedNum::alloc(cs.namespace(|| "f"), || Ok(two))?;

                    Ok((vec![a, b, c, d, e, f], vec![]))
                }
                // ---------------- Round 1 ----------------
                1 => {
                    // Retrieve variables from round 0
                    let round0_vars = &prior_round_vars[0];
                    let a = &round0_vars[0];
                    let b = &round0_vars[1];
                    let c = &round0_vars[2];
                    let d = &round0_vars[3];
                    let e = &round0_vars[4];
                    let f = &round0_vars[5];

                    // Verifier-supplied challenge r for this round
                    let r_val = challenges.map(|v| v[0]).unwrap_or(E::Scalar::ZERO);
                    let r = AllocatedNum::alloc_input(cs.namespace(|| "r"), || Ok(r_val))?;

                    // Helper closure to create (x - r)
                    let sub_r = |cs: &mut CS, name: &'static str, x: &AllocatedNum<E::Scalar>| {
                        let val = x.get_value().zip(r.get_value()).map(|(xv, rv)| xv - rv);
                        AllocatedNum::alloc(cs.namespace(|| name), || {
                            val.ok_or(SynthesisError::AssignmentMissing)
                        })
                    };

                    let diff_a = sub_r(cs, "diff_a", a)?;
                    let diff_b = sub_r(cs, "diff_b", b)?;
                    let diff_c = sub_r(cs, "diff_c", c)?;
                    let diff_d = sub_r(cs, "diff_d", d)?;
                    let diff_e = sub_r(cs, "diff_e", e)?;
                    let diff_f = sub_r(cs, "diff_f", f)?;

                    // Enforce diff_i = x_i - r for each x in {a..f}
                    let enforce_sub =
                        |label: &'static str,
                         cs: &mut CS,
                         x: &AllocatedNum<E::Scalar>,
                         diff: &AllocatedNum<E::Scalar>| {
                            cs.enforce(
                                || label,
                                |lc| lc + x.get_variable() - r.get_variable(),
                                |lc| lc + CS::one(),
                                |lc| lc + diff.get_variable(),
                            );
                        };
                    enforce_sub("a_minus_r", cs, a, &diff_a);
                    enforce_sub("b_minus_r", cs, b, &diff_b);
                    enforce_sub("c_minus_r", cs, c, &diff_c);
                    enforce_sub("d_minus_r", cs, d, &diff_d);
                    enforce_sub("e_minus_r", cs, e, &diff_e);
                    enforce_sub("f_minus_r", cs, f, &diff_f);

                    // Compute lhs = (diff_a * diff_b) * diff_c
                    let lhs_ab = AllocatedNum::alloc(cs.namespace(|| "lhs_ab"), || {
                        diff_a
                            .get_value()
                            .zip(diff_b.get_value())
                            .map(|(x, y)| x * y)
                            .ok_or(SynthesisError::AssignmentMissing)
                    })?;
                    cs.enforce(
                        || "lhs_ab_mul",
                        |lc| lc + diff_a.get_variable(),
                        |lc| lc + diff_b.get_variable(),
                        |lc| lc + lhs_ab.get_variable(),
                    );

                    let lhs = AllocatedNum::alloc(cs.namespace(|| "lhs"), || {
                        lhs_ab
                            .get_value()
                            .zip(diff_c.get_value())
                            .map(|(x, y)| x * y)
                            .ok_or(SynthesisError::AssignmentMissing)
                    })?;
                    cs.enforce(
                        || "lhs_mul",
                        |lc| lc + lhs_ab.get_variable(),
                        |lc| lc + diff_c.get_variable(),
                        |lc| lc + lhs.get_variable(),
                    );

                    // Compute rhs = (diff_d * diff_e) * diff_f
                    let rhs_de = AllocatedNum::alloc(cs.namespace(|| "rhs_de"), || {
                        diff_d
                            .get_value()
                            .zip(diff_e.get_value())
                            .map(|(x, y)| x * y)
                            .ok_or(SynthesisError::AssignmentMissing)
                    })?;
                    cs.enforce(
                        || "rhs_de_mul",
                        |lc| lc + diff_d.get_variable(),
                        |lc| lc + diff_e.get_variable(),
                        |lc| lc + rhs_de.get_variable(),
                    );

                    let rhs = AllocatedNum::alloc(cs.namespace(|| "rhs"), || {
                        rhs_de
                            .get_value()
                            .zip(diff_f.get_value())
                            .map(|(x, y)| x * y)
                            .ok_or(SynthesisError::AssignmentMissing)
                    })?;
                    cs.enforce(
                        || "rhs_mul",
                        |lc| lc + rhs_de.get_variable(),
                        |lc| lc + diff_f.get_variable(),
                        |lc| lc + rhs.get_variable(),
                    );

                    // Enforce equality lhs = rhs
                    cs.enforce(
                        || "lhs_eq_rhs",
                        |lc| lc + lhs.get_variable(),
                        |lc| lc + CS::one(),
                        |lc| lc + rhs.get_variable(),
                    );

                    Ok((vec![lhs, rhs], vec![r]))
                }
                _ => Err(SynthesisError::Unsatisfiable),
            }
        }

        fn num_rounds(&self) -> usize {
            2
        }

        fn commitment_width(&self) -> usize {
            32
        }
    }

    fn test_multiround_permutation_with<E: Engine>() {
        let circuit = TwoRoundPermutationCircuit;

        // Generate shape
        let (shape, ck, _vk) =
            <ShapeCS<E> as MultiRoundVegaShape<E>>::multiround_r1cs_shape(&circuit).unwrap();

        // Initialize witness state
        let mut state = SatisfyingAssignment::<E>::initialize_multiround_witness(&shape).unwrap();
        let mut transcript = <E as Engine>::TE::new(b"test");

        // Process each round
        let num_rounds = <TwoRoundPermutationCircuit as MultiRoundCircuit<E>>::num_rounds(&circuit);
        for r in 0..num_rounds {
            let _ = SatisfyingAssignment::<E>::process_round(
                &mut state,
                &shape,
                &ck,
                &circuit,
                r,
                &mut transcript,
            )
            .unwrap();
        }

        // Finalize
        let (instance, witness) =
            SatisfyingAssignment::<E>::finalize_multiround_witness(&mut state, &shape).unwrap();

        // Convert to regular instance/shape for satisfiability check
        let regular_shape = shape.to_regular_shape();
        let regular_instance = instance.to_regular_instance().unwrap();

        let sat_res = regular_shape.is_sat(&ck, &regular_instance, &witness);
        assert!(sat_res.is_ok());
    }

    #[test]
    fn test_multiround_permutation() {
        crate::iroha_rng::with_deterministic_test_seed(0x13, || {
            test_multiround_permutation_with::<PallasHyraxEngine>();
        });
    }
}
