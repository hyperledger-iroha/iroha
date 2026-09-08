//! Bounded multi-open arithmetic, transcript, and full-proof equivalence.

use super::*;
use crate::{
    arithmetic::kate_division,
    circuit::{Layouter, SimpleFloorPlanner, Value},
    plonk::{
        Advice, Circuit, Column, ConstraintSystem, Error, Instance, Selector, create_proof,
        keygen_pk, keygen_vk, verify_proof,
    },
    poly::{
        Rotation, VerificationStrategy as _, VerifierQuery,
        commitment::{MSM, Params, Verifier},
        ipa::{multiopen::VerifierIPA, strategy::SingleStrategy},
    },
    transcript::{
        Blake2bRead, Blake2bWrite, Challenge255, Transcript, TranscriptReadBuffer,
        TranscriptWriterBuffer,
    },
};
use ff::{FromUniformBytes, WithSmallOrderMulGroup};
use halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

mod original;
use original::OriginalProverIPA;

fn polynomial<F: Field>(values: Vec<F>) -> Polynomial<F, Coeff> {
    Polynomial {
        values,
        _marker: PhantomData,
    }
}

fn check_arithmetic<F: Field + From<u64>>() {
    for len in [1, 2, 3, 4, 7, 16, 31, 64, 127] {
        let original = (0..len)
            .map(|i| F::from((i * i + 3 * i + 17) as u64))
            .collect::<Vec<_>>();
        for point in [F::ZERO, F::ONE, -F::ONE, F::from(29)] {
            let mut actual = original.clone();
            let ptr = actual.as_ptr();
            let capacity = actual.capacity();
            let mut expected = original.clone();
            while !expected.is_empty() {
                expected = kate_division(&expected, point);
                kate_division_in_place(&mut actual, point);
                assert_eq!(actual, expected);
                assert_eq!(
                    actual.as_ptr(),
                    ptr,
                    "division reallocates no coefficient bank"
                );
                assert_eq!(actual.capacity(), capacity);
            }
        }
        let addend = polynomial(original.iter().map(|value| *value + F::ONE).collect());
        for challenge in [F::ZERO, F::ONE, -F::ONE, F::from(31)] {
            let mut actual = polynomial(original.clone());
            let expected = actual.clone() * challenge + &addend;
            let ptr = actual.values.as_ptr();
            fold_polynomial_in_place(&mut actual, challenge, &addend);
            assert_eq!(actual.values, expected.values);
            assert_eq!(actual.values.as_ptr(), ptr, "fold reuses its accumulator");
        }
    }
}

#[test]
fn division_and_fold_match_original_in_both_fields() {
    check_arithmetic::<Fp>();
    check_arithmetic::<Fq>();
}

fn query_order(order: usize) -> Vec<(usize, usize)> {
    // Four different point sets; sets 0 and 2 each combine two polynomials.
    let mut queries = vec![
        (0, 0),
        (0, 1),
        (1, 0),
        (1, 1),
        (2, 2),
        (3, 0),
        (3, 2),
        (4, 0),
        (4, 2),
        (5, 1),
        (5, 2),
        (5, 3),
    ];
    if order == 1 {
        queries.reverse();
    }
    if order == 2 {
        queries.rotate_left(5);
    }
    queries
}

fn check_reconstruction<C: CurveAffine>() {
    let polys = (0..6)
        .map(|p| {
            polynomial(
                (0..16)
                    .map(|r| C::Scalar::from((p * 100 + r + 1) as u64))
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    for order in 0..3 {
        let queries: Vec<ProverQuery<'_, C>> = query_order(order)
            .into_iter()
            .map(|(p, point)| ProverQuery {
                point: C::Scalar::from((point + 2) as u64),
                poly: &polys[p],
                blind: Blind(C::Scalar::from((p + 7) as u64)),
            })
            .collect::<Vec<_>>();
        let (map, sets) = construct_intermediate_sets(queries).unwrap();
        assert_eq!(sets.len(), 4);
        for x in [C::Scalar::ZERO, C::Scalar::ONE, C::Scalar::from(11)] {
            // Independent original all-set accumulation, including blind order.
            let mut bank: Vec<Option<Polynomial<C::Scalar, Coeff>>> = vec![None; sets.len()];
            let mut blinds = vec![Blind(C::Scalar::ZERO); sets.len()];
            for data in &map {
                bank[data.set_index] = Some(match &bank[data.set_index] {
                    Some(poly) => poly.clone() * x + data.commitment.poly,
                    None => data.commitment.poly.clone(),
                });
                blinds[data.set_index] *= x;
                blinds[data.set_index] += data.commitment.blind;
            }
            let mut scratch = polynomial(Vec::with_capacity(16));
            let ptr = scratch.values.as_ptr();
            let capacity = scratch.values.capacity();
            for (set, expected) in bank.iter().enumerate() {
                let blind = reconstruct_q(&map, set, x, &mut scratch);
                assert_eq!(scratch.values, expected.as_ref().unwrap().values);
                assert_eq!(blind.0, blinds[set].0);
                assert_eq!(scratch.values.as_ptr(), ptr);
                assert_eq!(scratch.values.capacity(), capacity);
            }
        }
    }
}

#[test]
fn reconstruction_preserves_set_horner_and_blind_order() {
    check_reconstruction::<EqAffine>();
    check_reconstruction::<EpAffine>();
}

fn prefix<C: CurveAffine, T: Transcript<C, Challenge255<C>>>(transcript: &mut T, commitments: &[C])
where
    C::Scalar: FromUniformBytes<64>,
{
    for commitment in commitments {
        transcript.common_point(*commitment).unwrap();
    }
}

fn check_multiopen<C: CurveAffine>()
where
    C::Scalar: FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let polys = (0..6)
        .map(|p| {
            polynomial(
                (0..16)
                    .map(|r| C::Scalar::from((p * 100 + r * r + 17) as u64))
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    let blinds = (0..6)
        .map(|p| Blind(C::Scalar::from((p + 1) as u64)))
        .collect::<Vec<_>>();
    let commitments = polys
        .iter()
        .zip(&blinds)
        .map(|(poly, blind)| params.commit(poly, *blind).to_affine())
        .collect::<Vec<_>>();
    for order in 0..3 {
        let queries: Vec<ProverQuery<'_, C>> = query_order(order)
            .into_iter()
            .map(|(p, point)| ProverQuery {
                point: C::Scalar::from((point + 2) as u64),
                poly: &polys[p],
                blind: blinds[p],
            })
            .collect::<Vec<_>>();
        let verifier_queries = query_order(order)
            .into_iter()
            .map(|(p, point)| {
                let point = C::Scalar::from((point + 2) as u64);
                VerifierQuery::new_commitment(
                    &commitments[p],
                    point,
                    eval_polynomial(&polys[p], point),
                )
            })
            .collect::<Vec<_>>();
        let mut original_rng = ChaCha20Rng::from_seed([37; 32]);
        let mut actual_rng = original_rng.clone();
        let mut reference = Blake2bWrite::<_, C, Challenge255<C>>::init(Vec::new());
        let mut actual = Blake2bWrite::<_, C, Challenge255<C>>::init(Vec::new());
        prefix(&mut reference, &commitments);
        prefix(&mut actual, &commitments);
        OriginalProverIPA::<C>::new(&params)
            .create_proof(&mut original_rng, &mut reference, queries.clone())
            .unwrap();
        ProverIPA::<C>::new(&params)
            .create_proof(&mut actual_rng, &mut actual, queries.clone())
            .unwrap();
        let proof = actual.finalize();
        assert_eq!(proof, reference.finalize(), "seeded proof order {order}");
        assert_eq!(
            actual_rng.next_u64(),
            original_rng.next_u64(),
            "RNG consumption differs"
        );
        let verify = |bytes: &[u8], queries| {
            let mut transcript = Blake2bRead::<_, C, Challenge255<C>>::init(bytes);
            prefix(&mut transcript, &commitments);
            VerifierIPA::<C>::new(&params)
                .verify_proof(&mut transcript, queries, params.empty_msm())
                .map(|guard| guard.use_challenges().check())
                .unwrap_or(false)
        };
        assert!(verify(&proof, verifier_queries.clone()));
        let mut altered = verifier_queries.clone();
        altered[0].eval += C::Scalar::ONE;
        assert!(!verify(&proof, altered), "altered evaluation accepted");
        let mut duplicate = verifier_queries.clone();
        duplicate.push(duplicate[0].clone());
        assert!(
            !verify(&proof, duplicate),
            "duplicate verifier query accepted"
        );
        let mut corrupt = proof.clone();
        let last = corrupt.len() - 1;
        corrupt[last] ^= 1;
        assert!(
            !verify(&corrupt, verifier_queries.clone()),
            "corrupt proof accepted"
        );
        assert!(
            !verify(&proof[..proof.len() - 1], verifier_queries),
            "truncated proof accepted"
        );
        let mut duplicate = queries;
        duplicate.push(duplicate[0].clone());
        let mut reference = Blake2bWrite::<_, C, Challenge255<C>>::init(Vec::new());
        let mut actual = Blake2bWrite::<_, C, Challenge255<C>>::init(Vec::new());
        let reference_error = OriginalProverIPA::<C>::new(&params)
            .create_proof(
                ChaCha20Rng::from_seed([37; 32]),
                &mut reference,
                duplicate.clone(),
            )
            .unwrap_err();
        let actual_error = ProverIPA::<C>::new(&params)
            .create_proof(ChaCha20Rng::from_seed([37; 32]), &mut actual, duplicate)
            .unwrap_err();
        assert_eq!(actual_error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(actual_error.to_string(), reference_error.to_string());
        assert_eq!(actual.finalize(), reference.finalize());
    }
}

#[test]
fn multiple_reordered_sets_match_original_eq() {
    check_multiopen::<EqAffine>();
}
#[test]
fn multiple_reordered_sets_match_original_ep() {
    check_multiopen::<EpAffine>();
}

#[derive(Clone)]
struct InstanceCircuit<F: Field> {
    values: [Value<F>; 2],
}
#[derive(Clone, Copy, Debug)]
struct InstanceConfig {
    advice: [Column<Advice>; 2],
    instance: [Column<Instance>; 2],
    selector: Selector,
}
impl<F: Field> Circuit<F> for InstanceCircuit<F> {
    type Config = InstanceConfig;
    type FloorPlanner = SimpleFloorPlanner;
    #[cfg(feature = "circuit-params")]
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            values: [Value::unknown(); 2],
        }
    }
    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let advice = [meta.advice_column(), meta.advice_column()];
        let instance = [meta.instance_column(), meta.instance_column()];
        for column in advice {
            meta.enable_equality(column);
        }
        for column in instance {
            meta.enable_equality(column);
        }
        let selector = meta.selector();
        meta.create_gate("next row increments the public value", |meta| {
            let selector = meta.query_selector(selector);
            let cur = meta.query_advice(advice[0], Rotation::cur());
            let next = meta.query_advice(advice[0], Rotation::next());
            vec![selector * (next - cur - crate::plonk::Expression::Constant(F::ONE))]
        });
        InstanceConfig {
            advice,
            instance,
            selector,
        }
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        let cells = layouter.assign_region(
            || "two instance columns and rotated advice",
            |mut region| {
                config.selector.enable(&mut region, 0)?;
                let first = region
                    .assign_advice(config.advice[0], 0, self.values[0])
                    .cell();
                region.assign_advice(config.advice[0], 1, self.values[0] + Value::known(F::ONE));
                let second = region
                    .assign_advice(config.advice[1], 0, self.values[1])
                    .cell();
                Ok([first, second])
            },
        )?;
        for (cell, column) in cells.into_iter().zip(config.instance) {
            layouter.constrain_instance(cell, column, 0);
        }
        Ok(())
    }
}

fn check_plonk<C: CurveAffine + crate::SerdeCurveAffine, const QUERY: bool, const MASK: u64>()
where
    C::Scalar: FromUniformBytes<64> + WithSmallOrderMulGroup<3> + crate::SerdePrimeField,
{
    let semantic = [C::Scalar::from(7)];
    let carrier = [
        C::Scalar::from(11),
        C::Scalar::from(12),
        C::Scalar::from(13),
    ];
    let columns: [&[C::Scalar]; 2] = [&semantic, &carrier];
    let instances: [&[&[C::Scalar]]; 1] = [&columns];
    let circuit = InstanceCircuit {
        values: [Value::known(semantic[0]), Value::known(carrier[0])],
    };
    let params = ParamsIPA::<C>::new(5);
    let vk = keygen_vk(&params, &circuit).unwrap();
    let pk = keygen_pk(&params, vk, &circuit).unwrap();
    let mut reference = Blake2bWrite::<_, C, Challenge255<C>>::init(Vec::new());
    let mut actual = Blake2bWrite::<_, C, Challenge255<C>>::init(Vec::new());
    let mut original_rng = ChaCha20Rng::from_seed([41; 32]);
    let mut actual_rng = original_rng.clone();
    create_proof::<IPACommitmentScheme<C>, OriginalProverIPA<C, QUERY, MASK>, _, _, _, _>(
        &params,
        &pk,
        &[circuit.clone()],
        &instances,
        &mut original_rng,
        &mut reference,
    )
    .unwrap();
    create_proof::<IPACommitmentScheme<C>, ProverIPA<C, QUERY, MASK>, _, _, _, _>(
        &params,
        &pk,
        &[circuit],
        &instances,
        &mut actual_rng,
        &mut actual,
    )
    .unwrap();
    let proof = actual.finalize();
    assert_eq!(
        proof,
        reference.finalize(),
        "full proof mode ({QUERY}, {MASK})"
    );
    assert_eq!(actual_rng.next_u64(), original_rng.next_u64());
    let verify = |bytes: &[u8], columns: &[&[C::Scalar]]| {
        let mut transcript = Blake2bRead::<_, C, Challenge255<C>>::init(bytes);
        verify_proof::<IPACommitmentScheme<C>, VerifierIPA<C, QUERY, MASK>, _, _, _>(
            &params,
            pk.get_vk(),
            SingleStrategy::<C, QUERY, MASK>::new(&params),
            &[columns],
            &mut transcript,
        )
        .is_ok()
    };
    assert!(verify(&proof, &columns));
    let altered_semantic = [semantic[0] + C::Scalar::ONE];
    assert!(!verify(&proof, &[&altered_semantic, &carrier]));
    let altered_carrier = [carrier[0], carrier[1], carrier[2] + C::Scalar::ONE];
    assert!(
        !verify(&proof, &[&semantic, &altered_carrier]),
        "full carrier tail must remain bound"
    );
    let mut corrupt = proof.clone();
    let last = corrupt.len() - 1;
    corrupt[last] ^= 1;
    assert!(!verify(&corrupt, &columns));
    assert!(!verify(&proof[..proof.len() - 1], &columns));
}

#[test]
fn full_plonk_regular_direct_hybrid_match_original_eq() {
    check_plonk::<EqAffine, true, 0>();
    check_plonk::<EqAffine, false, 0>();
    check_plonk::<EqAffine, true, 2>();
}
#[test]
fn full_plonk_regular_direct_hybrid_match_original_ep() {
    check_plonk::<EpAffine, true, 0>();
    check_plonk::<EpAffine, false, 0>();
    check_plonk::<EpAffine, true, 2>();
}
