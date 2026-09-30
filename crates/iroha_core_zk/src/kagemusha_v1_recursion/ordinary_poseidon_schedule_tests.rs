//! Schedule checks against the pinned full PLONK reader; fixtures are not monetary proofs.

use super::*;
use halo2_proofs::halo2curves::{
    group::Curve as _,
    pasta::{EpAffine, EqAffine},
};
use halo2_proofs::{
    plonk::keygen_vk,
    poly::{commitment::ParamsProver, ipa::commitment::ParamsIPA},
};
use snark_verifier::{
    system::halo2::{Config, compile},
    util::{
        arithmetic::{Domain, root_of_unity},
        transcript::{Transcript, TranscriptRead},
    },
};

type NativeTranscript<C, R> = PoseidonTranscript<
    C,
    NativeLoader,
    R,
    KAGEMUSHA_IPA_POSEIDON_WIDTH_V1,
    KAGEMUSHA_IPA_POSEIDON_RATE_V1,
    KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1,
    KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
>;

struct CountedTranscript<C: CurveAffine, R> {
    inner: NativeTranscript<C, R>,
    pending: usize,
    observed: Vec<usize>,
}

impl<C: CurveAffine, R: Read> Transcript<C, NativeLoader> for CountedTranscript<C, R> {
    fn loader(&self) -> &NativeLoader {
        self.inner.loader()
    }

    fn squeeze_challenge(&mut self) -> C::ScalarExt {
        self.observed.push(std::mem::take(&mut self.pending));
        self.inner.squeeze_challenge()
    }

    fn common_scalar(&mut self, scalar: &C::ScalarExt) -> Result<(), Error> {
        self.inner.common_scalar(scalar)?;
        self.pending += 1;
        Ok(())
    }

    fn common_ec_point(&mut self, point: &C) -> Result<(), Error> {
        self.inner.common_ec_point(point)?;
        self.pending += 2;
        Ok(())
    }
}

impl<C: CurveAffine, R: Read> TranscriptRead<C, NativeLoader> for CountedTranscript<C, R> {
    fn read_scalar(&mut self) -> Result<C::ScalarExt, Error> {
        let scalar = self.inner.read_scalar()?;
        self.pending += 1;
        Ok(scalar)
    }

    fn read_ec_point(&mut self) -> Result<C, Error> {
        let point = self.inner.read_ec_point()?;
        self.pending += 2;
        Ok(point)
    }
}

fn check_actual_reader<C: CurveAffine>()
where
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    let point = |index| (C::generator() * C::ScalarExt::from(index)).to_affine();
    for k in [12, 16] {
        let mut builder = BaseCircuitBuilder::<C::ScalarExt>::new(false)
            .use_k(k)
            .use_instance_columns(2);
        let gate = halo2_base::gates::GateChip::default();
        let x = builder.main(0).load_witness(C::ScalarExt::from(11));
        let y = builder.main(0).load_witness(C::ScalarExt::from(12));
        let z = gate.add(builder.main(0), x, y);
        builder.assigned_instances = vec![vec![x, y], vec![z]];
        builder.calculate_params(Some(9));
        let params = ParamsIPA::<C>::new(k as u32);
        let vk = keygen_vk(&params, &builder).expect("small actual schedule-fixture VK");
        let template = compile(&params, &vk, Config::ipa().with_num_instance(vec![2, 1]));
        assert!(template.instance_committing_key.is_some());
        assert!(!template.queries.is_empty());
        assert!(ordinary_poseidon_squeeze_inputs_v1(&template, k + 1).is_err());
        let mut invalid = template.clone();
        invalid.num_witness = vec![1];
        invalid.num_challenge = vec![3];
        assert!(ordinary_poseidon_squeeze_inputs_v1(&invalid, k).is_err());
        invalid.num_witness = vec![1, 1, 1];
        invalid.num_challenge = vec![2, 2, 1];
        assert!(ordinary_poseidon_squeeze_inputs_v1(&invalid, k).is_err());
        invalid.num_challenge.pop();
        assert!(ordinary_poseidon_squeeze_inputs_v1(&invalid, k).is_err());
        invalid = template.clone();
        invalid.instance_committing_key = None;
        invalid.num_instance = vec![usize::MAX, 1];
        assert!(ordinary_poseidon_squeeze_inputs_v1(&invalid, k).is_err());
        invalid = template.clone();
        invalid.num_witness = vec![usize::MAX];
        invalid.num_challenge = vec![1];
        assert!(ordinary_poseidon_squeeze_inputs_v1(&invalid, k).is_err());
        if k == 16 {
            let mut shard = template.clone();
            shard.domain = Domain::new(12, root_of_unity::<C::ScalarExt>(12));
            for (parent_points, shard_points) in
                [(1, 1), (200, 31), (400, 100), (873, 1), (1_008, 1)]
            {
                let mut parent = template.clone();
                parent.preprocessed = vec![point(1); parent_points];
                shard.preprocessed = vec![point(2); shard_points];
                let plan = ClaimProofTranscriptPlanV1::new(&parent, &shard)
                    .expect("optional native selection preserves the complete Base path");
                let available =
                    1_984_usize.saturating_sub(parent_points + shard_points + 14 + 1_054);
                assert!(plan.native_permutation_count() <= available);
                if let Some(schedule) = plan.parent() {
                    assert_eq!(
                        schedule,
                        ordinary_poseidon_squeeze_inputs_v1(&parent, 16).unwrap()
                    );
                }
                if let Some(schedule) = plan.shard() {
                    assert_eq!(
                        schedule,
                        ordinary_poseidon_squeeze_inputs_v1(&shard, 12).unwrap()
                    );
                }
                assert!(
                    ClaimFoldTranscriptPlanV1::with_reserved_ordinary(
                        parent_points,
                        shard_points,
                        available + 1
                    )
                    .is_err()
                );
                if available == 0 {
                    assert!(plan.parent().is_none() && plan.shard().is_none());
                    assert_eq!(
                        plan.folds(),
                        ClaimFoldTranscriptPlanV1::new(parent_points, shard_points).unwrap()
                    );
                }
            }
        }
        for committed in [false, true] {
            for initial in [false, true] {
                for alias in [false, true] {
                    // Exercise zero-challenge phases that retain pending fields, an empty
                    // witness phase that squeezes twice, and consecutive empty squeezes.
                    for (witnesses, challenges) in [
                        (vec![2, 0, 1, 0], vec![0, 1, 2, 1]),
                        (vec![0, 3, 0], vec![0, 0, 0]),
                        (vec![1], vec![1]),
                    ] {
                        // Compile a real tiny configuration so no private dependency types
                        // or fabricated verifier-key format are needed for these parser fixtures.
                        let mut protocol = template.clone();
                        let mut first = protocol.queries[0];
                        first.poly = 0;
                        first.rotation.0 = -1;
                        let mut second = first;
                        second.poly = 1;
                        second.rotation.0 = if alias { (1_i32 << k) - 1 } else { -1 };
                        protocol.queries = vec![first, second];
                        protocol.evaluations = protocol.queries.clone();
                        protocol.num_witness = witnesses;
                        protocol.num_challenge = challenges;
                        protocol.transcript_initial_state =
                            initial.then_some(C::ScalarExt::from(9));
                        if !committed {
                            protocol.instance_committing_key = None;
                        }
                        let key = IpaSuccinctVerifyingKey::new(
                            Domain::new(k, root_of_unity::<C::ScalarExt>(k)),
                            point(1),
                            point(2),
                            Some(point(3)),
                        );
                        let instances = vec![
                            vec![C::ScalarExt::from(11), C::ScalarExt::from(12)],
                            vec![C::ScalarExt::from(23)],
                        ];
                        let predicted = ordinary_poseidon_squeeze_inputs_v1(&protocol, k)
                            .expect("bounded authenticated ordinary schedule");
                        let profile = ordinary_ipa_proof_profile_at_k_v1(&protocol, k).unwrap();
                        let mut bytes = Vec::new();
                        for index in 0..protocol.num_witness.iter().sum::<usize>()
                            + protocol.quotient.num_chunk()
                        {
                            bytes.extend_from_slice(point(20 + index as u64).to_bytes().as_ref());
                        }
                        // Two PLONK evaluations, F, one or two BGH19 rotation-set evaluations,
                        // S, k pairs of round points, c, blind, and final G.
                        for value in [31, 32] {
                            bytes.extend_from_slice(C::ScalarExt::from(value).to_repr().as_ref());
                        }
                        bytes.extend_from_slice(point(40).to_bytes().as_ref());
                        for index in 0..if alias { 2 } else { 1 } {
                            bytes.extend_from_slice(
                                C::ScalarExt::from(41 + index).to_repr().as_ref(),
                            );
                        }
                        bytes.extend_from_slice(point(50).to_bytes().as_ref());
                        for index in 0..2 * k {
                            bytes.extend_from_slice(point(60 + index as u64).to_bytes().as_ref());
                        }
                        for value in [101, 102] {
                            bytes.extend_from_slice(C::ScalarExt::from(value).to_repr().as_ref());
                        }
                        bytes.extend_from_slice(point(103).to_bytes().as_ref());
                        assert_eq!(bytes.len(), profile.byte_len);
                        let (reader, position) = ExactReader::new(&bytes);
                        let mut transcript = CountedTranscript {
                            inner: NativeTranscript::<C, _>::new::<
                                KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1,
                            >(reader),
                            pending: 0,
                            observed: Vec::new(),
                        };
                        PlonkSuccinctVerifier::<IpaAs<C, Bgh19>>::read_proof(
                            &key,
                            &protocol,
                            &instances,
                            &mut transcript,
                        )
                        .expect("pinned PLONK/BGH19 parser accepts canonical fixture bytes");
                        let _ = transcript.squeeze_challenge();
                        assert_eq!(position.get(), bytes.len());
                        assert_eq!(transcript.observed, predicted);
                        assert_eq!(transcript.pending, 0);
                    }
                }
            }
        }
    }
}

#[test]
fn eq_ordinary_schedule_matches_actual_plonk_reader() {
    check_actual_reader::<EqAffine>();
}

#[test]
fn ep_ordinary_schedule_matches_actual_plonk_reader() {
    check_actual_reader::<EpAffine>();
}

#[test]
fn complete_transcript_selection_keeps_the_best_fitting_inventory() {
    // Taking the predecessor greedily would retain only 175 permutations here; the shard
    // retains 190. At 270 slots the predecessor plus both folds is the better complete set.
    assert_eq!(select_ordinary_native_mask(200, 100, 190), 1);
    assert_eq!(select_ordinary_native_mask(270, 120, 190), 2);
    assert_eq!(select_ordinary_native_mask(270, 130, 140), 3);
    // Optional small ordinary transcripts must not displace two fuller complete folds.
    assert_eq!(select_ordinary_native_mask(151, 4, 100), 0);
    assert_eq!(select_ordinary_native_mask(0, 1, 1), 0);
    assert_eq!(select_ordinary_native_mask(150, usize::MAX, usize::MAX), 0);
    assert_eq!(select_ordinary_native_mask(150, usize::MAX, 75), 1);
    assert_eq!(schedule_permutations(&[41, 0, 2, 1]).unwrap(), 25);
    assert!(schedule_permutations(&[usize::MAX, usize::MAX]).is_err());
}
