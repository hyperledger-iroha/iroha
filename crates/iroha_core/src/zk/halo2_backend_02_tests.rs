// Lexically included by `zk::tests` to preserve the existing libtest paths.
// Chip-backed Poseidon circuits (IPA): commit-open and merkle2
#[cfg(all(
    feature = "zk-halo2-ipa",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa-poseidon"
))]
mod ipa_fixture {
    use super::*;
    use halo2_proofs::{
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        plonk::{Circuit, VerifyingKey, keygen_pk, keygen_vk},
        transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer as _},
    };
    use rand_core_06::OsRng;

    pub(super) struct IpaProofFixture {
        vk_envelope: Vec<u8>,
        proof_bytes: Vec<u8>,
    }

    impl IpaProofFixture {
        fn new(k: u32, vk: &VerifyingKey<Curve>, proof_bytes: Vec<u8>) -> Self {
            let mut vk_envelope = zk1::wrap_start();
            zk1::wrap_append_ipa_k(&mut vk_envelope, k);
            zk1::wrap_append_vk_pasta(&mut vk_envelope, vk);
            Self {
                vk_envelope,
                proof_bytes,
            }
        }

        pub(super) fn proof_envelope(&self) -> Vec<u8> {
            let mut envelope = zk1::wrap_start();
            zk1::wrap_append_proof(&mut envelope, &self.proof_bytes);
            envelope
        }

        pub(super) fn truncated_proof_envelope(&self) -> Vec<u8> {
            let mut envelope = zk1::wrap_start();
            envelope.extend_from_slice(b"PROF");
            envelope.extend_from_slice(&(self.proof_bytes.len() as u32).to_le_bytes());
            let cut = self.proof_bytes.len().saturating_sub(1);
            envelope.extend_from_slice(&self.proof_bytes[..cut]);
            envelope
        }

        pub(super) fn verify_envelope(&self, backend: &str, proof_envelope: Vec<u8>) -> bool {
            let vk_box = VerifyingKeyBox::new(backend.into(), self.vk_envelope.clone());
            let proof_box = ProofBox::new(backend.into(), proof_envelope);
            super::super::verify_halo2_ipa(backend, &proof_box, Some(&vk_box))
        }

        pub(super) fn verify_single(&self, backend: &str, instances: &[Scalar]) -> bool {
            let mut envelope = self.proof_envelope();
            zk1::wrap_append_instances_pasta_fp(instances, &mut envelope);
            self.verify_envelope(backend, envelope)
        }

        pub(super) fn verify_two_columns(&self, backend: &str, values: &[Scalar; 2]) -> bool {
            let mut envelope = self.proof_envelope();
            let columns = [&values[..1], &values[1..]];
            zk1::wrap_append_instances_pasta_fp_cols(&columns, &mut envelope);
            self.verify_envelope(backend, envelope)
        }

        pub(super) fn verify_six_columns(&self, backend: &str, values: &[Scalar; 6]) -> bool {
            let mut envelope = self.proof_envelope();
            let columns = [
                &values[0..1],
                &values[1..2],
                &values[2..3],
                &values[3..4],
                &values[4..5],
                &values[5..6],
            ];
            zk1::wrap_append_instances_pasta_fp_cols(&columns, &mut envelope);
            self.verify_envelope(backend, envelope)
        }
    }

    pub(super) fn build_with_instances<C>(
        k: u32,
        circuit: C,
        instance_columns: &[&[Scalar]],
    ) -> IpaProofFixture
    where
        C: Circuit<Scalar>,
    {
        let params: PastaParams = pasta_params_new(k);
        let vk: VerifyingKey<Curve> = keygen_vk(&params, &circuit).expect("vk");
        let pk = keygen_pk(&params, vk.clone(), &circuit).expect("pk");
        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        halo2_proofs::plonk::create_proof::<
            IPACommitmentScheme<Curve>,
            ProverIPA<'_, Curve>,
            Challenge255<Curve>,
            _,
            _,
            _,
        >(
            &params,
            &pk,
            &[circuit],
            &[instance_columns],
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        IpaProofFixture::new(k, &vk, transcript.finalize())
    }

    pub(super) fn build_without_instances<C>(k: u32, circuit: C) -> IpaProofFixture
    where
        C: Circuit<Scalar>,
    {
        let params: PastaParams = pasta_params_new(k);
        let vk: VerifyingKey<Curve> = keygen_vk(&params, &circuit).expect("vk");
        let pk = keygen_pk(&params, vk.clone(), &circuit).expect("pk");
        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        halo2_proofs::plonk::create_proof::<
            IPACommitmentScheme<Curve>,
            ProverIPA<'_, Curve>,
            Challenge255<Curve>,
            _,
            _,
            _,
        >(&params, &pk, &[circuit], &[&[]], OsRng, &mut transcript)
        .expect("proof created");
        IpaProofFixture::new(k, &vk, transcript.finalize())
    }

    fn fifth_power(value: Scalar) -> Scalar {
        let square = value * value;
        square * square * value
    }

    fn poseidon_pair(left: Scalar, right: Scalar) -> Scalar {
        Scalar::from(2) * fifth_power(left + Scalar::from(7))
            + Scalar::from(3) * fifth_power(right + Scalar::from(13))
    }

    pub(super) fn commit_open_instance() -> Scalar {
        poseidon_pair(Scalar::from(11), Scalar::from(31))
    }

    pub(super) fn vote_instances(depth: u64) -> [Scalar; 2] {
        let commitment = poseidon_pair(Scalar::from(1), Scalar::from(12_345));
        let mut root = commitment;
        for level in 0..depth {
            root = poseidon_pair(root, Scalar::from(20 + level));
        }
        [commitment, root]
    }

    fn anon_commitment(value: u64, blinding: u64) -> Scalar {
        Scalar::from(2) * fifth_power(Scalar::from(value))
            + Scalar::from(3) * fifth_power(Scalar::from(blinding))
            + Scalar::from(7)
    }

    pub(super) fn anon_instances(depth: u64) -> [Scalar; 6] {
        let input_zero = anon_commitment(7, 11);
        let input_one = anon_commitment(5, 13);
        let output_zero = anon_commitment(6, 17);
        let output_one = anon_commitment(6, 19);
        let nullifier = anon_commitment(1_234_567, 42);
        let mut root = input_zero;
        for level in 0..depth {
            root = poseidon_pair(root, Scalar::from(20 + level));
        }
        [
            input_zero,
            input_one,
            output_zero,
            output_one,
            nullifier,
            root,
        ]
    }
}
#[cfg(all(
    feature = "zk-halo2-ipa",
    feature = "zk-halo2-ipa-poseidon",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa-poseidon"
))]
#[test]
fn halo2_poseidon_commit_open_chip_ipa() {
    let instances = [ipa_fixture::commit_open_instance()];
    let proof = ipa_fixture::build_with_instances(
        6,
        pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        &[&instances],
    );
    assert!(proof.verify_single("halo2/pasta/ipa/tiny-commit-open", &instances));
}
#[cfg(all(
    feature = "zk-halo2-ipa",
    feature = "zk-halo2-ipa-poseidon",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa-poseidon"
))]
#[test]
fn halo2_poseidon_merkle2_chip_ipa() {
    let instances = [pasta_tiny::poseidon::merkle2_poseidon_sample_root()];
    let proof = ipa_fixture::build_with_instances(
        6,
        pasta_tiny::poseidon::Merkle2Poseidon::default(),
        &[&instances],
    );
    assert!(proof.verify_single("halo2/pasta/ipa/tiny-merkle2", &instances));
}
// Depth-8 end-to-end checks for vote/transfer circuits (IPA)
#[cfg(all(
    feature = "zk-halo2-ipa",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa-poseidon"
))]
#[test]
fn halo2_verify_vote_bool_commit_merkle8_ipa() {
    let instances = ipa_fixture::vote_instances(8);
    let proof = ipa_fixture::build_with_instances(
        6,
        depth::VoteBoolCommitMerkle::<8>::default(),
        &[&instances],
    );
    assert!(proof.verify_two_columns("halo2/pasta/ipa/vote-bool-commit-merkle8", &instances,));
}
#[cfg(all(
    feature = "zk-halo2-ipa",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa-poseidon"
))]
#[test]
fn halo2_verify_anon_transfer_2x2_merkle8_ipa() {
    let instances = ipa_fixture::anon_instances(8);
    let proof = ipa_fixture::build_with_instances(
        7,
        depth::AnonTransfer2x2CommitMerkle::<8>::default(),
        &[&instances],
    );
    assert!(proof.verify_six_columns("halo2/pasta/ipa/anon-transfer-2x2-merkle8", &instances,));
}
// Poseidon-tagged (runtime-selected) variants: vote/transfer @ depth-8
#[cfg(all(
    feature = "zk-halo2-ipa",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa-poseidon"
))]
#[test]
fn halo2_verify_vote_bool_commit_merkle8_poseidon_ipa() {
    let instances = ipa_fixture::vote_instances(8);
    let proof = ipa_fixture::build_with_instances(
        6,
        poseidon_depth::VoteBoolCommitMerklePoseidon::<8>::default(),
        &[&instances],
    );
    let backend = backend_tag_vote_bool_commit_merkle(8, true);
    assert!(proof.verify_two_columns(&backend, &instances));
}
#[cfg(all(
    feature = "zk-halo2-ipa",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa-poseidon"
))]
#[test]
fn halo2_verify_vote_bool_commit_merkle8_poseidon_ipa_zk1_permutation_harness() {
    use halo2_proofs::{
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        plonk::{VerifyingKey, keygen_pk, keygen_vk},
        poly::commitment::Params as _,
        transcript::{Blake2bWrite, Challenge255},
    };
    use rand_core_06::OsRng;
    #[derive(Clone, Copy)]
    enum Step2 {
        ProfGood,
        ProfBadTrunc,
        I10p2Good,
        I10p2BadShort,
        Unknown(u32),
    }
    let k = 6u32;
    let params: PastaParams = pasta_params_new(k);
    let vk_h2: VerifyingKey<Curve> = keygen_vk(
        &params,
        &poseidon_depth::VoteBoolCommitMerklePoseidon::<8>::default(),
    )
    .expect("vk");
    let pk = keygen_pk(
        &params,
        vk_h2.clone(),
        &poseidon_depth::VoteBoolCommitMerklePoseidon::<8>::default(),
    )
    .expect("pk");
    // Compute expected instances
    let v = Scalar::from(1u64);
    let rho = Scalar::from(12345u64);
    let rc0 = Scalar::from(7u64);
    let rc1 = Scalar::from(13u64);
    let two = Scalar::from(2u64);
    let three = Scalar::from(3u64);
    let a = v + rc0;
    let b = rho + rc1;
    let a2 = a * a;
    let a4 = a2 * a2;
    let a5 = a4 * a;
    let b2 = b * b;
    let b4 = b2 * b2;
    let b5 = b4 * b;
    let commit = two * a5 + three * b5;
    let mut prev = commit;
    for i in 0..8u64 {
        let sib = Scalar::from(20 + i);
        let t0 = prev + rc0;
        let t1 = sib + rc1;
        let t0_2 = t0 * t0;
        let t0_4 = t0_2 * t0_2;
        let t0_5 = t0_4 * t0;
        let t1_2 = t1 * t1;
        let t1_4 = t1_2 * t1_2;
        let t1_5 = t1_4 * t1;
        prev = two * t0_5 + three * t1_5;
    }
    let root = prev;
    let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
    let insts: [&[&[Scalar]]; 1] = [&[&[commit, root]]];
    halo2_proofs::plonk::create_proof::<
        IPACommitmentScheme<Curve>,
        ProverIPA<'_, Curve>,
        Challenge255<Curve>,
        _,
        _,
        _,
    >(
        &params,
        &pk,
        &[poseidon_depth::VoteBoolCommitMerklePoseidon::<8>::default()],
        &insts,
        OsRng,
        &mut transcript,
    )
    .expect("proof created");
    let proof_bytes = transcript.finalize();
    let backend = backend_tag_vote_bool_commit_merkle(8, true);
    let mut vk_env = zk1::wrap_start();
    zk1::wrap_append_ipa_k(&mut vk_env, k);
    zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
    let run_case = |steps: &[Step2], ok_expected: bool| {
        let mut prf_env = zk1::wrap_start();
        for s in steps {
            match *s {
                Step2::ProfGood => zk1::wrap_append_proof(&mut prf_env, &proof_bytes),
                Step2::ProfBadTrunc => {
                    prf_env.extend_from_slice(b"PROF");
                    prf_env.extend_from_slice(&(proof_bytes.len() as u32).to_le_bytes());
                    let cut = proof_bytes.len().saturating_sub(1);
                    prf_env.extend_from_slice(&proof_bytes[..cut]);
                }
                Step2::I10p2Good => zk1::wrap_append_instances_pasta_fp_cols(
                    &[&[commit][..], &[root][..]],
                    &mut prf_env,
                ),
                Step2::I10p2BadShort => {
                    prf_env.extend_from_slice(b"I10P");
                    prf_env.extend_from_slice(&(2u32).to_le_bytes());
                    prf_env.extend_from_slice(&(1u32).to_le_bytes());
                    prf_env.extend_from_slice(&[0u8; 32]);
                }
                Step2::Unknown(len) => {
                    prf_env.extend_from_slice(b"UKNW");
                    prf_env.extend_from_slice(&len.to_le_bytes());
                    prf_env.extend_from_slice(&vec![0xCC; len as usize]);
                }
            }
        }
        let vk_box = VerifyingKeyBox::new(backend.clone().into(), vk_env.clone());
        let prf_box = ProofBox::new(backend.clone().into(), prf_env);
        assert_eq!(
            super::verify_halo2_ipa(&backend, &prf_box, Some(&vk_box)),
            ok_expected
        );
    };
    let cases: &[(&[Step2], bool)] = &[
        (&[Step2::ProfGood, Step2::I10p2Good], true),
        (&[Step2::I10p2Good, Step2::ProfGood], true),
        (
            &[Step2::ProfBadTrunc, Step2::ProfGood, Step2::I10p2Good],
            true,
        ),
        (
            &[Step2::ProfGood, Step2::ProfBadTrunc, Step2::I10p2Good],
            false,
        ),
        (
            &[Step2::ProfGood, Step2::I10p2BadShort, Step2::I10p2Good],
            true,
        ),
        (
            &[Step2::ProfGood, Step2::I10p2Good, Step2::I10p2BadShort],
            false,
        ),
        (
            &[
                Step2::Unknown(0),
                Step2::ProfGood,
                Step2::Unknown(3),
                Step2::I10p2Good,
            ],
            true,
        ),
    ];
    for (steps, ok) in cases {
        run_case(steps, *ok);
    }
}
#[cfg(all(
    feature = "zk-halo2-ipa",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa-poseidon"
))]
#[test]
fn halo2_verify_vote_bool_commit_merkle8_poseidon_ipa_zk1_malformed_inst() {
    let proof = ipa_fixture::build_without_instances(
        6,
        poseidon_depth::VoteBoolCommitMerklePoseidon::<8>::default(),
    );
    let mut proof_envelope = proof.proof_envelope();
    proof_envelope.extend_from_slice(b"I10P");
    proof_envelope.extend_from_slice(&(2u32).to_le_bytes());
    proof_envelope.extend_from_slice(&(1u32).to_le_bytes());
    proof_envelope.extend_from_slice(&[0u8; 32]);
    let backend = backend_tag_vote_bool_commit_merkle(8, true);
    assert!(!proof.verify_envelope(&backend, proof_envelope));
}
#[cfg(all(
    feature = "zk-halo2-ipa",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa-poseidon"
))]
#[test]
fn halo2_verify_vote_bool_commit_merkle8_poseidon_ipa_zk1_truncated_prof() {
    let proof = ipa_fixture::build_without_instances(
        6,
        poseidon_depth::VoteBoolCommitMerklePoseidon::<8>::default(),
    );
    let backend = backend_tag_vote_bool_commit_merkle(8, true);
    assert!(!proof.verify_envelope(&backend, proof.truncated_proof_envelope()));
}
#[cfg(all(
    feature = "zk-halo2-ipa",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa-poseidon"
))]
#[test]
fn halo2_verify_vote_bool_commit_merkle16_poseidon_ipa_zk1() {
    let instances = ipa_fixture::vote_instances(16);
    let proof = ipa_fixture::build_with_instances(
        6,
        poseidon_depth::VoteBoolCommitMerklePoseidon::<16>::default(),
        &[&instances],
    );
    let backend = backend_tag_vote_bool_commit_merkle(16, true);
    assert!(proof.verify_two_columns(&backend, &instances));
}
#[cfg(all(
    feature = "zk-halo2-ipa",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa-poseidon"
))]
#[test]
fn halo2_verify_anon_transfer_2x2_merkle16_poseidon_ipa_zk1() {
    let instances = ipa_fixture::anon_instances(16);
    let proof = ipa_fixture::build_with_instances(
        7,
        poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<16>::default(),
        &[&instances],
    );
    let backend = backend_tag_anon_transfer_merkle(16, true);
    assert!(proof.verify_six_columns(&backend, &instances));
}
#[cfg(all(
    feature = "zk-halo2-ipa",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa-poseidon"
))]
#[test]
fn halo2_verify_vote_bool_commit_merkle16_poseidon_ipa_zk1_malformed_inst() {
    let proof = ipa_fixture::build_without_instances(
        6,
        poseidon_depth::VoteBoolCommitMerklePoseidon::<16>::default(),
    );
    let mut proof_envelope = proof.proof_envelope();
    proof_envelope.extend_from_slice(b"I10P");
    proof_envelope.extend_from_slice(&(2u32).to_le_bytes());
    proof_envelope.extend_from_slice(&(1u32).to_le_bytes());
    proof_envelope.extend_from_slice(&[0u8; 32]);
    let backend = backend_tag_vote_bool_commit_merkle(16, true);
    assert!(!proof.verify_envelope(&backend, proof_envelope));
}
#[cfg(all(
    feature = "zk-halo2-ipa",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa-poseidon"
))]
#[test]
fn halo2_verify_vote_bool_commit_merkle16_poseidon_ipa_zk1_truncated_prof() {
    let proof = ipa_fixture::build_without_instances(
        6,
        poseidon_depth::VoteBoolCommitMerklePoseidon::<16>::default(),
    );
    let backend = backend_tag_vote_bool_commit_merkle(16, true);
    assert!(!proof.verify_envelope(&backend, proof.truncated_proof_envelope()));
}
#[cfg(all(
    feature = "zk-halo2-ipa",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa-poseidon"
))]
#[test]
fn halo2_verify_anon_transfer_2x2_merkle16_poseidon_ipa_zk1_noncanonical() {
    use ff::PrimeField as _;
    use halo2_proofs::halo2curves::pasta::Fp as Scalar;

    let proof = ipa_fixture::build_without_instances(
        7,
        poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<16>::default(),
    );
    let mut proof_envelope = proof.proof_envelope();
    proof_envelope.extend_from_slice(b"I10P");
    proof_envelope.extend_from_slice(&(6u32).to_le_bytes());
    proof_envelope.extend_from_slice(&(1u32).to_le_bytes());
    for _ in 0..5 {
        proof_envelope.extend_from_slice(Scalar::ZERO.to_repr().as_ref());
    }
    proof_envelope.extend_from_slice(&[0xFF; 32]);
    let backend = backend_tag_anon_transfer_merkle(16, true);
    assert!(!proof.verify_envelope(&backend, proof_envelope));
}
#[cfg(all(
    feature = "zk-halo2-ipa",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa-poseidon"
))]
#[test]
fn halo2_verify_anon_transfer_2x2_merkle16_poseidon_ipa_zk1_invalid_header() {
    let proof = ipa_fixture::build_without_instances(
        7,
        poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<16>::default(),
    );
    let mut proof_envelope = proof.proof_envelope();
    proof_envelope.extend_from_slice(b"I10P");
    proof_envelope.extend_from_slice(&(0u32).to_le_bytes());
    proof_envelope.extend_from_slice(&(1u32).to_le_bytes());
    let backend = backend_tag_anon_transfer_merkle(16, true);
    assert!(!proof.verify_envelope(&backend, proof_envelope));
}
#[cfg(all(
    feature = "zk-halo2-ipa",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa-poseidon"
))]
#[test]
fn halo2_verify_anon_transfer_2x2_merkle8_poseidon_ipa_zk1_permutation_harness() {
    use halo2_proofs::{
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        plonk::{VerifyingKey, keygen_pk, keygen_vk},
        poly::commitment::Params as _,
        transcript::{Blake2bWrite, Challenge255},
    };
    use rand_core_06::OsRng;
    #[derive(Clone, Copy)]
    enum Step6 {
        ProfGood,
        ProfBadTrunc,
        I10p6Good,
        I10p6BadShort,
        Unknown(u32),
    }
    let k = 7u32;
    let params: PastaParams = pasta_params_new(k);
    let vk_h2: VerifyingKey<Curve> = keygen_vk(
        &params,
        &poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<8>::default(),
    )
    .expect("vk");
    let pk = keygen_pk(
        &params,
        vk_h2.clone(),
        &poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<8>::default(),
    )
    .expect("pk");
    let in0 = Scalar::from(7u64);
    let in1 = Scalar::from(5u64);
    let out0 = Scalar::from(6u64);
    let out1 = Scalar::from(6u64);
    let r0 = Scalar::from(11u64);
    let r1 = Scalar::from(13u64);
    let r2 = Scalar::from(17u64);
    let r3 = Scalar::from(19u64);
    let sk = Scalar::from(1_234_567u64);
    let serial = Scalar::from(42u64);
    let two = Scalar::from(2u64);
    let three = Scalar::from(3u64);
    let h2 = |x: Scalar, r: Scalar| {
        let x2 = x * x;
        let x4 = x2 * x2;
        let x5 = x4 * x;
        let r2 = r * r;
        let r4 = r2 * r2;
        let r5 = r4 * r;
        two * x5 + three * r5 + Scalar::from(7u64)
    };
    let cm_in0 = h2(in0, r0);
    let cm_in1 = h2(in1, r1);
    let cm_out0 = h2(out0, r2);
    let cm_out1 = h2(out1, r3);
    let nf = h2(sk, serial);
    let rc0 = Scalar::from(7u64);
    let rc1 = Scalar::from(13u64);
    let mut prev = cm_in0;
    for i in 0..8u64 {
        let sib = Scalar::from(20 + i);
        let t0 = prev + rc0;
        let t1 = sib + rc1;
        let t0_2 = t0 * t0;
        let t0_4 = t0_2 * t0_2;
        let t0_5 = t0_4 * t0;
        let t1_2 = t1 * t1;
        let t1_4 = t1_2 * t1_2;
        let t1_5 = t1_4 * t1;
        prev = two * t0_5 + three * t1_5;
    }
    let root = prev;
    let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
    let insts: [&[&[Scalar]]; 1] = [&[&[cm_in0, cm_in1, cm_out0, cm_out1, nf, root]]];
    halo2_proofs::plonk::create_proof::<
        IPACommitmentScheme<Curve>,
        ProverIPA<'_, Curve>,
        Challenge255<Curve>,
        _,
        _,
        _,
    >(
        &params,
        &pk,
        &[poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<8>::default()],
        &insts,
        OsRng,
        &mut transcript,
    )
    .expect("proof created");
    let proof_bytes = transcript.finalize();
    let backend = backend_tag_anon_transfer_merkle(8, true);
    let mut vk_env = zk1::wrap_start();
    zk1::wrap_append_ipa_k(&mut vk_env, k);
    zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
    let run_case = |steps: &[Step6], ok_expected: bool| {
        let mut prf_env = zk1::wrap_start();
        for s in steps {
            match *s {
                Step6::ProfGood => zk1::wrap_append_proof(&mut prf_env, &proof_bytes),
                Step6::ProfBadTrunc => {
                    prf_env.extend_from_slice(b"PROF");
                    prf_env.extend_from_slice(&(proof_bytes.len() as u32).to_le_bytes());
                    let cut = proof_bytes.len().saturating_sub(1);
                    prf_env.extend_from_slice(&proof_bytes[..cut]);
                }
                Step6::I10p6Good => zk1::wrap_append_instances_pasta_fp_cols(
                    &[
                        &[cm_in0][..],
                        &[cm_in1][..],
                        &[cm_out0][..],
                        &[cm_out1][..],
                        &[nf][..],
                        &[root][..],
                    ],
                    &mut prf_env,
                ),
                Step6::I10p6BadShort => {
                    prf_env.extend_from_slice(b"I10P");
                    prf_env.extend_from_slice(&(6u32).to_le_bytes());
                    prf_env.extend_from_slice(&(1u32).to_le_bytes());
                    prf_env.extend_from_slice(&[0u8; 32 * 3]);
                }
                Step6::Unknown(len) => {
                    prf_env.extend_from_slice(b"UKNW");
                    prf_env.extend_from_slice(&len.to_le_bytes());
                    prf_env.extend_from_slice(&vec![0x99; len as usize]);
                }
            }
        }
        let vk_box = VerifyingKeyBox::new(backend.clone().into(), vk_env.clone());
        let prf_box = ProofBox::new(backend.clone().into(), prf_env);
        assert_eq!(
            super::verify_halo2_ipa(&backend, &prf_box, Some(&vk_box)),
            ok_expected
        );
    };
    let cases: &[(&[Step6], bool)] = &[
        (&[Step6::ProfGood, Step6::I10p6Good], true),
        (&[Step6::I10p6Good, Step6::ProfGood], true),
        (
            &[Step6::ProfBadTrunc, Step6::ProfGood, Step6::I10p6Good],
            true,
        ),
        (
            &[Step6::ProfGood, Step6::ProfBadTrunc, Step6::I10p6Good],
            false,
        ),
        (
            &[Step6::ProfGood, Step6::I10p6BadShort, Step6::I10p6Good],
            true,
        ),
        (
            &[Step6::ProfGood, Step6::I10p6Good, Step6::I10p6BadShort],
            false,
        ),
        (
            &[
                Step6::Unknown(0),
                Step6::ProfGood,
                Step6::Unknown(3),
                Step6::I10p6Good,
            ],
            true,
        ),
    ];
    for (steps, ok) in cases {
        run_case(steps, *ok);
    }
}
// Minimal randomized harness for depth-16 (ZK1). Few cases to keep runtime reasonable.
#[cfg(all(
    feature = "zk-halo2-ipa",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa-poseidon"
))]
#[test]
fn halo2_verify_vote_bool_commit_merkle16_poseidon_ipa_zk1_randomized_min() {
    use halo2_proofs::{
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        plonk::{VerifyingKey, keygen_pk, keygen_vk},
        poly::commitment::Params as _,
        transcript::{Blake2bWrite, Challenge255},
    };
    use rand_core_06::OsRng;
    #[derive(Clone, Copy)]
    enum S {
        Prof,
        ProfTrunc,
        I2,
        I2Short,
        Uk(u32),
    }
    let k = 6u32;
    let params: PastaParams = pasta_params_new(k);
    let vk_h2: VerifyingKey<Curve> = keygen_vk(
        &params,
        &poseidon_depth::VoteBoolCommitMerklePoseidon::<16>::default(),
    )
    .expect("vk");
    let pk = keygen_pk(
        &params,
        vk_h2.clone(),
        &poseidon_depth::VoteBoolCommitMerklePoseidon::<16>::default(),
    )
    .expect("pk");
    // Expected [commit, root]
    let v = Scalar::from(1u64);
    let rho = Scalar::from(12345u64);
    let rc0 = Scalar::from(7u64);
    let rc1 = Scalar::from(13u64);
    let two = Scalar::from(2u64);
    let three = Scalar::from(3u64);
    let a = v + rc0;
    let b = rho + rc1;
    let a2 = a * a;
    let a4 = a2 * a2;
    let a5 = a4 * a;
    let b2 = b * b;
    let b4 = b2 * b2;
    let b5 = b4 * b;
    let commit = two * a5 + three * b5;
    let mut prev = commit;
    for i in 0..16u64 {
        let sib = Scalar::from(20 + i);
        let t0 = prev + rc0;
        let t1 = sib + rc1;
        let t0_2 = t0 * t0;
        let t0_4 = t0_2 * t0_2;
        let t0_5 = t0_4 * t0;
        let t1_2 = t1 * t1;
        let t1_4 = t1_2 * t1_2;
        let t1_5 = t1_4 * t1;
        prev = two * t0_5 + three * t1_5;
    }
    let root = prev;
    let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
    let insts: [&[&[Scalar]]; 1] = [&[&[commit, root]]];
    halo2_proofs::plonk::create_proof::<
        IPACommitmentScheme<Curve>,
        ProverIPA<'_, Curve>,
        Challenge255<Curve>,
        _,
        _,
        _,
    >(
        &params,
        &pk,
        &[poseidon_depth::VoteBoolCommitMerklePoseidon::<16>::default()],
        &insts,
        OsRng,
        &mut transcript,
    )
    .expect("proof created");
    let proof_bytes = transcript.finalize();
    let backend = backend_tag_vote_bool_commit_merkle(16, true);
    let mut vk_env = zk1::wrap_start();
    zk1::wrap_append_ipa_k(&mut vk_env, k);
    zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
    // Deterministic small random-ish scenarios
    let scenarios: &[(&[S], bool)] = &[
        (&[S::Prof, S::I2], true),
        (&[S::Uk(3), S::Prof, S::Uk(7), S::I2], true),
        (&[S::ProfTrunc, S::Uk(2), S::Prof, S::I2], true),
        (&[S::Prof, S::I2Short], false),
        (&[S::ProfTrunc, S::I2], false),
    ];
    let run = |steps: &[S], ok: bool| {
        let mut prf_env = zk1::wrap_start();
        for s in steps {
            match *s {
                S::Prof => zk1::wrap_append_proof(&mut prf_env, &proof_bytes),
                S::ProfTrunc => {
                    prf_env.extend_from_slice(b"PROF");
                    prf_env.extend_from_slice(&(proof_bytes.len() as u32).to_le_bytes());
                    let cut = proof_bytes.len().saturating_sub(1);
                    prf_env.extend_from_slice(&proof_bytes[..cut]);
                }
                S::I2 => zk1::wrap_append_instances_pasta_fp_cols(
                    &[&[commit][..], &[root][..]],
                    &mut prf_env,
                ),
                S::I2Short => {
                    prf_env.extend_from_slice(b"I10P");
                    prf_env.extend_from_slice(&(2u32).to_le_bytes());
                    prf_env.extend_from_slice(&(1u32).to_le_bytes());
                    prf_env.extend_from_slice(&[0u8; 32]);
                }
                S::Uk(n) => {
                    prf_env.extend_from_slice(b"UNKN");
                    prf_env.extend_from_slice(&n.to_le_bytes());
                    prf_env.extend_from_slice(&vec![0xAD; n as usize]);
                }
            }
        }
        let vk_box = VerifyingKeyBox::new(backend.clone().into(), vk_env.clone());
        let prf_box = ProofBox::new(backend.clone().into(), prf_env);
        assert_eq!(
            super::verify_halo2_ipa(&backend, &prf_box, Some(&vk_box)),
            ok
        );
    };
    for (steps, ok) in scenarios {
        run(steps, *ok);
    }
}
// ZK1 permutation harness for depth-16 anon (6-column), trimmed cases
#[cfg(all(
    feature = "zk-halo2-ipa",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa-poseidon"
))]
#[test]
fn halo2_verify_anon_transfer_2x2_merkle16_poseidon_ipa_zk1_permutation_harness() {
    use halo2_proofs::{
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        plonk::{VerifyingKey, keygen_pk, keygen_vk},
        poly::commitment::Params as _,
        transcript::{Blake2bWrite, Challenge255},
    };
    use rand_core_06::OsRng;
    #[derive(Clone, Copy)]
    enum S6 {
        Prof,
        ProfTrunc,
        I6,
        I6Short,
        Uk(u32),
    }
    let k = 7u32;
    let params: PastaParams = pasta_params_new(k);
    let vk_h2: VerifyingKey<Curve> = keygen_vk(
        &params,
        &poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<16>::default(),
    )
    .expect("vk");
    let pk = keygen_pk(
        &params,
        vk_h2.clone(),
        &poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<16>::default(),
    )
    .expect("pk");
    // Expected instances
    let in0 = Scalar::from(7u64);
    let in1 = Scalar::from(5u64);
    let out0 = Scalar::from(6u64);
    let out1 = Scalar::from(6u64);
    let r0 = Scalar::from(11u64);
    let r1 = Scalar::from(13u64);
    let r2 = Scalar::from(17u64);
    let r3 = Scalar::from(19u64);
    let sk = Scalar::from(1_234_567u64);
    let serial = Scalar::from(42u64);
    let two = Scalar::from(2u64);
    let three = Scalar::from(3u64);
    let h2 = |x: Scalar, r: Scalar| {
        let x2 = x * x;
        let x4 = x2 * x2;
        let x5 = x4 * x;
        let r2 = r * r;
        let r4 = r2 * r2;
        let r5 = r4 * r;
        two * x5 + three * r5 + Scalar::from(7u64)
    };
    let cm_in0 = h2(in0, r0);
    let cm_in1 = h2(in1, r1);
    let cm_out0 = h2(out0, r2);
    let cm_out1 = h2(out1, r3);
    let nf = h2(sk, serial);
    let rc0 = Scalar::from(7u64);
    let rc1 = Scalar::from(13u64);
    let mut prev = cm_in0;
    for i in 0..16u64 {
        let sib = Scalar::from(20 + i);
        let t0 = prev + rc0;
        let t1 = sib + rc1;
        let t0_2 = t0 * t0;
        let t0_4 = t0_2 * t0_2;
        let t0_5 = t0_4 * t0;
        let t1_2 = t1 * t1;
        let t1_4 = t1_2 * t1_2;
        let t1_5 = t1_4 * t1;
        prev = two * t0_5 + three * t1_5;
    }
    let root = prev;
    // Build proof
    let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
    let insts: [&[&[Scalar]]; 1] = [&[&[cm_in0, cm_in1, cm_out0, cm_out1, nf, root]]];
    halo2_proofs::plonk::create_proof::<
        IPACommitmentScheme<Curve>,
        ProverIPA<'_, Curve>,
        Challenge255<Curve>,
        _,
        _,
        _,
    >(
        &params,
        &pk,
        &[poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<16>::default()],
        &insts,
        OsRng,
        &mut transcript,
    )
    .expect("proof created");
    let proof_bytes = transcript.finalize();
    let backend = backend_tag_anon_transfer_merkle(16, true);
    let mut vk_env = zk1::wrap_start();
    zk1::wrap_append_ipa_k(&mut vk_env, k);
    zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
    // Trimmed scenarios
    let scenarios: &[(&[S6], bool)] = &[
        (&[S6::Prof, S6::I6], true),
        (&[S6::Uk(4), S6::Prof, S6::Uk(8), S6::I6], true),
        (&[S6::ProfTrunc, S6::I6], false),
        (&[S6::Prof, S6::I6Short], false),
    ];
    let run = |steps: &[S6], ok: bool| {
        let mut prf_env = zk1::wrap_start();
        for s in steps {
            match *s {
                S6::Prof => zk1::wrap_append_proof(&mut prf_env, &proof_bytes),
                S6::ProfTrunc => {
                    prf_env.extend_from_slice(b"PROF");
                    prf_env.extend_from_slice(&(proof_bytes.len() as u32).to_le_bytes());
                    let cut = proof_bytes.len().saturating_sub(1);
                    prf_env.extend_from_slice(&proof_bytes[..cut]);
                }
                S6::I6 => zk1::wrap_append_instances_pasta_fp_cols(
                    &[
                        &[cm_in0][..],
                        &[cm_in1][..],
                        &[cm_out0][..],
                        &[cm_out1][..],
                        &[nf][..],
                        &[root][..],
                    ],
                    &mut prf_env,
                ),
                S6::I6Short => {
                    prf_env.extend_from_slice(b"I10P");
                    prf_env.extend_from_slice(&(6u32).to_le_bytes());
                    prf_env.extend_from_slice(&(1u32).to_le_bytes());
                    prf_env.extend_from_slice(&[0u8; 32 * 3]);
                }
                S6::Uk(n) => {
                    prf_env.extend_from_slice(b"UKN2");
                    prf_env.extend_from_slice(&n.to_le_bytes());
                    prf_env.extend_from_slice(&vec![0x5A; n as usize]);
                }
            }
        }
        let vk_box = VerifyingKeyBox::new(backend.clone().into(), vk_env.clone());
        let prf_box = ProofBox::new(backend.clone().into(), prf_env);
        assert_eq!(
            super::verify_halo2_ipa(&backend, &prf_box, Some(&vk_box)),
            ok
        );
    };
    for (steps, ok) in scenarios {
        run(steps, *ok);
    }
}
