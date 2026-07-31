// Lexically included by `zk::tests` to preserve the existing libtest paths.


    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_anon_transfer_2x2_merkle8_poseidon_ipa_zk1_noncanonical() {
        use ff::PrimeField as _;
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

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

        // Valid proof
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
            &[poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<8>::default()],
            &[&[]],
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // ZK1: VK with IPAK; proof with I10P (6 cols) where one scalar is non-canonical (0xFF..)
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut prf_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
        prf_env.extend_from_slice(b"I10P");
        prf_env.extend_from_slice(&(6u32).to_le_bytes()); // cols
        prf_env.extend_from_slice(&(1u32).to_le_bytes()); // rows
        // Append 5 zeros, then one non-canonical
        for _ in 0..5 {
            prf_env.extend_from_slice(Scalar::ZERO.to_repr().as_ref());
        }
        prf_env.extend_from_slice(&[0xFFu8; 32]);

        let backend = backend_tag_anon_transfer_merkle(8, true);
        let vk_box = VerifyingKeyBox::new(backend.clone().into(), vk_env);
        let prf_box = ProofBox::new(backend.clone().into(), prf_env);
        assert!(!super::verify_backend(&backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_anon_transfer_2x2_merkle8_poseidon_ipa_zk1_invalid_header() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

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
            &[poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<8>::default()],
            &[&[]],
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        // PROF ok, I10P invalid (rows=0)
        let mut prf_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
        prf_env.extend_from_slice(b"I10P");
        prf_env.extend_from_slice(&(1u32).to_le_bytes()); // cols=1
        prf_env.extend_from_slice(&(0u32).to_le_bytes()); // rows=0 (invalid)

        let backend = backend_tag_anon_transfer_merkle(8, true);
        let vk_box = VerifyingKeyBox::new(backend.clone().into(), vk_env);
        let prf_box = ProofBox::new(backend.clone().into(), prf_env);
        assert!(!super::verify_backend(&backend, &prf_box, Some(&vk_box)));
    }

    // --- Tiny Poseidon circuits (base) negative ZK1 tests ---

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_tiny_commit_open_ipa_zk1_truncated_prof() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("pk");

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
            &[pasta_tiny::poseidon::CommitOpenPoseidon::default()],
            &[&[]],
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // VK: ZK1 IPAK; Proof: truncated PROF
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut prf_env = zk1::wrap_start();
        prf_env.extend_from_slice(b"PROF");
        prf_env.extend_from_slice(&(proof_bytes.len() as u32).to_le_bytes());
        let cut = proof_bytes.len().saturating_sub(1);
        prf_env.extend_from_slice(&proof_bytes[..cut]);

        let backend = "halo2/pasta/ipa/tiny-commit-open";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), prf_env);
        assert!(!super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_tiny_merkle2_ipa_zk1_invalid_header_extreme() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> =
            keygen_vk(&params, &pasta_tiny::poseidon::Merkle2Poseidon::default()).expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &pasta_tiny::poseidon::Merkle2Poseidon::default(),
        )
        .expect("pk");

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
            &[pasta_tiny::poseidon::Merkle2Poseidon::default()],
            &[&[]],
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // VK: ZK1 IPAK; Proof: PROF ok, I10P with extreme cols/rows beyond caps
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);

        // Case 1: cols > MAX_INST_COLS
        let mut prf_env1 = zk1::wrap_start();
        zk1::wrap_append_proof(&mut prf_env1, &proof_bytes);
        prf_env1.extend_from_slice(b"I10P");
        let cols_ext = (super::MAX_INST_COLS as u32).saturating_add(1);
        prf_env1.extend_from_slice(&cols_ext.to_le_bytes());
        prf_env1.extend_from_slice(&(1u32).to_le_bytes());
        // No data appended (should fail on header alone)

        let backend = "halo2/pasta/ipa/tiny-merkle2";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env.clone());
        let prf_box1 = ProofBox::new(backend.into(), prf_env1);
        assert!(!super::verify_backend(backend, &prf_box1, Some(&vk_box)));

        // Case 2: rows > MAX_INST_ROWS
        let mut prf_env2 = zk1::wrap_start();
        zk1::wrap_append_proof(&mut prf_env2, &proof_bytes);
        prf_env2.extend_from_slice(b"I10P");
        prf_env2.extend_from_slice(&(1u32).to_le_bytes());
        let rows_ext = (super::MAX_INST_ROWS as u32).saturating_add(1);
        prf_env2.extend_from_slice(&rows_ext.to_le_bytes());

        let prf_box2 = ProofBox::new(backend.into(), prf_env2);
        assert!(!super::verify_backend(backend, &prf_box2, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_tiny_commit_open_ipa_zk1_positive() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("pk");

        // Create proof with expected commitment as instance (computed in-circuit)
        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let insts: [&[&[Scalar]]; 1] = [&[&[{
            // Same commit as in circuit synthesize: m=11, r=31; Pow5 compressor
            let m = Scalar::from(11u64);
            let r = Scalar::from(31u64);
            let t0 = m + Scalar::from(7u64);
            let t1 = r + Scalar::from(13u64);
            let t0_2 = t0 * t0;
            let t0_4 = t0_2 * t0_2;
            let t0_5 = t0_4 * t0;
            let t1_2 = t1 * t1;
            let t1_4 = t1_2 * t1_2;
            let t1_5 = t1_4 * t1;
            Scalar::from(2u64) * t0_5 + Scalar::from(3u64) * t1_5
        }][..]]];
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
            &[pasta_tiny::poseidon::CommitOpenPoseidon::default()],
            &insts,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // ZK1: VK IPAK; Proof PROF + I10P with 1 col (commit)
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut prf_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
        let col: [&[Scalar]; 1] = [&[insts[0][0][0]][..]];
        zk1::wrap_append_instances_pasta_fp_cols(&col, &mut prf_env);

        let backend = "halo2/pasta/ipa/tiny-commit-open";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), prf_env);
        assert!(super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_tiny_merkle2_ipa_zk1_positive() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> =
            keygen_vk(&params, &pasta_tiny::poseidon::Merkle2Poseidon::default()).expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &pasta_tiny::poseidon::Merkle2Poseidon::default(),
        )
        .expect("pk");

        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let root = pasta_tiny::poseidon::merkle2_poseidon_sample_root();
        let insts: [&[&[Scalar]]; 1] = [&[&[root]]];
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
            &[pasta_tiny::poseidon::Merkle2Poseidon::default()],
            &insts,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut prf_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
        let col: [&[Scalar]; 1] = [&[root][..]];
        zk1::wrap_append_instances_pasta_fp_cols(&col, &mut prf_env);

        let backend = "halo2/pasta/ipa/tiny-merkle2";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), prf_env);
        assert!(super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    // ZK1 is canonical: duplicate proof payloads and unknown tags fail closed.
    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_tiny_commit_open_ipa_zk1_multiple_prof_and_unknown_rejects() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("pk");

        // Create proof with 1 public instance
        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let insts: [&[&[Scalar]]; 1] = [&[&[{
            let m = Scalar::from(11u64);
            let r = Scalar::from(31u64);
            let t0 = m + Scalar::from(7u64);
            let t1 = r + Scalar::from(13u64);
            let t0_2 = t0 * t0;
            let t0_4 = t0_2 * t0_2;
            let t0_5 = t0_4 * t0;
            let t1_2 = t1 * t1;
            let t1_4 = t1_2 * t1_2;
            let t1_5 = t1_4 * t1;
            Scalar::from(2u64) * t0_5 + Scalar::from(3u64) * t1_5
        }][..]]];
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
            &[pasta_tiny::poseidon::CommitOpenPoseidon::default()],
            &insts,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // ZK1: multiple PROF (first truncated), unknown TLV in between, then correct PROF
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut prf_env = zk1::wrap_start();
        // PROF #1 truncated
        prf_env.extend_from_slice(b"PROF");
        prf_env.extend_from_slice(&(proof_bytes.len() as u32).to_le_bytes());
        let cut = proof_bytes.len().saturating_sub(1);
        prf_env.extend_from_slice(&proof_bytes[..cut]);
        // Unknown TLV
        prf_env.extend_from_slice(b"UKNW");
        prf_env.extend_from_slice(&(4u32).to_le_bytes());
        prf_env.extend_from_slice(&[1, 2, 3, 4]);
        // PROF #2 correct
        zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
        // Instances (1 col)
        let col: [&[Scalar]; 1] = [&[insts[0][0][0]][..]];
        zk1::wrap_append_instances_pasta_fp_cols(&col, &mut prf_env);

        let backend = "halo2/pasta/ipa/tiny-commit-open";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), prf_env);
        assert!(!super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_tiny_commit_open_ipa_zk1_unknown_tlv_stress_rejects() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("pk");

        // Proof with 1 instance (commit)
        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let insts: [&[&[Scalar]]; 1] = [&[&[{
            let m = Scalar::from(11u64);
            let r = Scalar::from(31u64);
            let t0 = m + Scalar::from(7u64);
            let t1 = r + Scalar::from(13u64);
            let t0_2 = t0 * t0;
            let t0_4 = t0_2 * t0_2;
            let t0_5 = t0_4 * t0;
            let t1_2 = t1 * t1;
            let t1_4 = t1_2 * t1_2;
            let t1_5 = t1_4 * t1;
            Scalar::from(2u64) * t0_5 + Scalar::from(3u64) * t1_5
        }][..]]];
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
            &[pasta_tiny::poseidon::CommitOpenPoseidon::default()],
            &insts,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // ZK1: many unknown TLVs interleaved, then valid PROF + I10P
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut prf_env = zk1::wrap_start();
        // Unknown 0-len
        prf_env.extend_from_slice(b"U0__");
        prf_env.extend_from_slice(&(0u32).to_le_bytes());
        // Unknown 1-len
        prf_env.extend_from_slice(b"U1__");
        prf_env.extend_from_slice(&(1u32).to_le_bytes());
        prf_env.extend_from_slice(&[0xAB]);
        // Unknown 4-len
        prf_env.extend_from_slice(b"U4__");
        prf_env.extend_from_slice(&(4u32).to_le_bytes());
        prf_env.extend_from_slice(&[1, 2, 3, 4]);
        // Unknown 17-len
        prf_env.extend_from_slice(b"UQ__");
        prf_env.extend_from_slice(&(17u32).to_le_bytes());
        prf_env.extend_from_slice(&[0x55; 17]);
        // Unknown 256-len
        prf_env.extend_from_slice(b"UB__");
        prf_env.extend_from_slice(&(256u32).to_le_bytes());
        prf_env.extend_from_slice(&vec![0x42; 256]);
        // Valid PROF
        zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
        // Instances
        let col: [&[Scalar]; 1] = [&[insts[0][0][0]][..]];
        zk1::wrap_append_instances_pasta_fp_cols(&col, &mut prf_env);

        let backend = "halo2/pasta/ipa/tiny-commit-open";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), prf_env);
        assert!(!super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    // ZK1 duplicate instance payloads fail closed regardless of ordering.
    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_tiny_commit_open_ipa_zk1_duplicate_i10p_last_correct_rejects() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("pk");

        // Create proof with expected commitment as instance
        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let commit = {
            let m = Scalar::from(11u64);
            let r = Scalar::from(31u64);
            let t0 = m + Scalar::from(7u64);
            let t1 = r + Scalar::from(13u64);
            let t0_2 = t0 * t0;
            let t0_4 = t0_2 * t0_2;
            let t0_5 = t0_4 * t0;
            let t1_2 = t1 * t1;
            let t1_4 = t1_2 * t1_2;
            let t1_5 = t1_4 * t1;
            Scalar::from(2u64) * t0_5 + Scalar::from(3u64) * t1_5
        };
        let insts: [&[&[Scalar]]; 1] = [&[&[commit]]];
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
            &[pasta_tiny::poseidon::CommitOpenPoseidon::default()],
            &insts,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // ZK1: duplicate I10P — first wrong, second correct — must reject.
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut prf_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
        // I10P #1 (wrong commit = 0)
        zk1::wrap_append_instances_pasta_fp_cols(&[&[Scalar::ZERO][..]], &mut prf_env);
        // I10P #2 (correct)
        zk1::wrap_append_instances_pasta_fp_cols(&[&[commit][..]], &mut prf_env);

        let backend = "halo2/pasta/ipa/tiny-commit-open";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), prf_env);
        assert!(!super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    // Reversed duplicate-instance ordering must fail identically.
    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_tiny_commit_open_ipa_zk1_duplicate_i10p_last_wrong_rejects() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("pk");
        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let commit = {
            let m = Scalar::from(11u64);
            let r = Scalar::from(31u64);
            let t0 = m + Scalar::from(7u64);
            let t1 = r + Scalar::from(13u64);
            let t0_2 = t0 * t0;
            let t0_4 = t0_2 * t0_2;
            let t0_5 = t0_4 * t0;
            let t1_2 = t1 * t1;
            let t1_4 = t1_2 * t1_2;
            let t1_5 = t1_4 * t1;
            Scalar::from(2u64) * t0_5 + Scalar::from(3u64) * t1_5
        };
        let insts: [&[&[Scalar]]; 1] = [&[&[commit]]];
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
            &[pasta_tiny::poseidon::CommitOpenPoseidon::default()],
            &insts,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // ZK1: duplicate I10P — first correct, second wrong → reject
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut prf_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
        // I10P #1 correct
        zk1::wrap_append_instances_pasta_fp_cols(&[&[commit][..]], &mut prf_env);
        // I10P #2 wrong
        zk1::wrap_append_instances_pasta_fp_cols(&[&[Scalar::ZERO][..]], &mut prf_env);

        let backend = "halo2/pasta/ipa/tiny-commit-open";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), prf_env);
        assert!(!super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    // Randomized deterministic unknown-TLV stress must fail closed.
    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_tiny_commit_open_ipa_zk1_unknown_tlv_randomized_rejects() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("pk");

        // Proof with 1 instance
        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let commit = {
            let m = Scalar::from(11u64);
            let r = Scalar::from(31u64);
            let t0 = m + Scalar::from(7u64);
            let t1 = r + Scalar::from(13u64);
            let t0_2 = t0 * t0;
            let t0_4 = t0_2 * t0_2;
            let t0_5 = t0_4 * t0;
            let t1_2 = t1 * t1;
            let t1_4 = t1_2 * t1_2;
            let t1_5 = t1_4 * t1;
            Scalar::from(2u64) * t0_5 + Scalar::from(3u64) * t1_5
        };
        let insts: [&[&[Scalar]]; 1] = [&[&[commit]]];
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
            &[pasta_tiny::poseidon::CommitOpenPoseidon::default()],
            &insts,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // Deterministic PRNG
        let mut seed: u64 = 0xC0FFEE_F00D_BAAD;
        for _round in 0..4 {
            let mut vk_env = zk1::wrap_start();
            zk1::wrap_append_ipa_k(&mut vk_env, k);
            zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
            let mut prf_env = zk1::wrap_start();
            // Generate N unknown TLVs with varying lengths ≤ 64
            let n = (seed as usize % 5) + 1;
            for i in 0..n {
                // xorshift64*
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                let len = (seed as usize % 64) as u32;
                let tag = [
                    b'U',
                    b'A' + (i as u8 % 26),
                    b'0' + ((seed as u8) % 10),
                    b'X',
                ];
                prf_env.extend_from_slice(&tag);
                prf_env.extend_from_slice(&len.to_le_bytes());
                prf_env.extend_from_slice(&vec![0xA5; len as usize]);
            }
            // Valid PROF + I10P
            zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
            let col: [&[Scalar]; 1] = [&[commit][..]];
            zk1::wrap_append_instances_pasta_fp_cols(&col, &mut prf_env);

            let backend = "halo2/pasta/ipa/tiny-commit-open";
            let vk_box = VerifyingKeyBox::new(backend.into(), vk_env.clone());
            let prf_box = ProofBox::new(backend.into(), prf_env);
            assert!(!super::verify_backend(backend, &prf_box, Some(&vk_box)));
        }
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_tiny_commit_open_ipa_zk1_permutation_harness() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        #[derive(Clone, Copy)]
        enum Step {
            ProfGood,
            ProfBadTrunc,
            I10pGood,
            I10pBadShort,
            Unknown(u32),
        }

        // Prepare circuit, proof and expected instance (commit)
        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("pk");

        // Expected commitment (same as in synthesize)
        let commit = {
            let m = Scalar::from(11u64);
            let r = Scalar::from(31u64);
            let t0 = m + Scalar::from(7u64);
            let t1 = r + Scalar::from(13u64);
            let t0_2 = t0 * t0;
            let t0_4 = t0_2 * t0_2;
            let t0_5 = t0_4 * t0;
            let t1_2 = t1 * t1;
            let t1_4 = t1_2 * t1_2;
            let t1_5 = t1_4 * t1;
            Scalar::from(2u64) * t0_5 + Scalar::from(3u64) * t1_5
        };

        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let insts: [&[&[Scalar]]; 1] = [&[&[commit]]];
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
            &[pasta_tiny::poseidon::CommitOpenPoseidon::default()],
            &insts,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        let backend = "halo2/pasta/ipa/tiny-commit-open";
        let mut vk_env_base = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env_base, k);
        zk1::wrap_append_vk_pasta(&mut vk_env_base, &vk_h2);

        // Helper to build envelope from a sequence of steps and check expected outcome
        let run_case = |steps: &[Step], expect_ok: bool| {
            let mut prf_env = zk1::wrap_start();
            for s in steps {
                match *s {
                    Step::ProfGood => zk1::wrap_append_proof(&mut prf_env, &proof_bytes),
                    Step::ProfBadTrunc => {
                        prf_env.extend_from_slice(b"PROF");
                        prf_env.extend_from_slice(&(proof_bytes.len() as u32).to_le_bytes());
                        let cut = proof_bytes.len().saturating_sub(1);
                        prf_env.extend_from_slice(&proof_bytes[..cut]);
                    }
                    Step::I10pGood => {
                        zk1::wrap_append_instances_pasta_fp_cols(&[&[commit][..]], &mut prf_env)
                    }
                    Step::I10pBadShort => {
                        prf_env.extend_from_slice(b"I10P");
                        prf_env.extend_from_slice(&(1u32).to_le_bytes());
                        prf_env.extend_from_slice(&(1u32).to_le_bytes());
                        // no scalar payload -> short
                    }
                    Step::Unknown(len) => {
                        prf_env.extend_from_slice(b"UKNW");
                        prf_env.extend_from_slice(&len.to_le_bytes());
                        prf_env.extend_from_slice(&vec![0xEE; len as usize]);
                    }
                }
            }
            let vk_box = VerifyingKeyBox::new(backend.into(), vk_env_base.clone());
            let prf_box = ProofBox::new(backend.into(), prf_env);
            assert_eq!(
                super::verify_backend(backend, &prf_box, Some(&vk_box)),
                expect_ok
            );
        };

        // Cases
        let cases: &[(&[Step], bool)] = &[
            (&[Step::ProfGood, Step::I10pGood], true),
            (&[Step::I10pGood, Step::ProfGood], true),
            (&[Step::ProfBadTrunc, Step::ProfGood, Step::I10pGood], false),
            (&[Step::ProfGood, Step::ProfBadTrunc, Step::I10pGood], false),
            (&[Step::ProfGood, Step::I10pBadShort, Step::I10pGood], false),
            (&[Step::ProfGood, Step::I10pGood, Step::I10pBadShort], false),
            (
                &[
                    Step::Unknown(0),
                    Step::ProfGood,
                    Step::Unknown(3),
                    Step::I10pGood,
                ],
                false,
            ),
            (
                &[Step::Unknown(8), Step::ProfBadTrunc, Step::I10pGood],
                false,
            ),
            (
                &[Step::ProfGood, Step::Unknown(16), Step::I10pBadShort],
                false,
            ),
            (
                &[Step::I10pGood, Step::ProfBadTrunc, Step::Unknown(1)],
                false,
            ),
        ];

        for (steps, expect_ok) in cases {
            run_case(steps, *expect_ok);
        }
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_zk1_prof_length_exceeds_cap_rejected() {
        // Build minimal ZK1 with PROF len > MAX_PROOF_LEN. Parser must reject.
        let k = 5u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2 =
            keygen_vk_cached("halo2/pasta/ipa/tiny-add", &params, &pasta_tiny::Add).expect("vk");
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut prf_env = zk1::wrap_start();
        prf_env.extend_from_slice(b"PROF");
        let too_big = (super::MAX_PROOF_LEN as u32).saturating_add(1);
        prf_env.extend_from_slice(&too_big.to_le_bytes());
        // no payload appended

        let backend = "halo2/pasta/ipa/tiny-add"; // any recognized halo2 backend tag
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), prf_env);
        assert!(!super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_add2inst_public_ipa() {
        use halo2_proofs::{
            circuit::{Layouter, SimpleFloorPlanner, Value},
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{
                Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey, keygen_pk, keygen_vk,
            },
            poly::{Rotation, commitment::Params as _},
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        #[derive(Clone, Default)]
        struct AddTwoInstPublic;
        impl Circuit<Scalar> for AddTwoInstPublic {
            type Config = (
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>,
                halo2_proofs::plonk::Selector,
            );
            type FloorPlanner = SimpleFloorPlanner;

            type Params = ();
            fn without_witnesses(&self) -> Self {
                Self
            }
            fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
                let a = meta.advice_column();
                let b = meta.advice_column();
                let c = meta.advice_column();
                let i0 = meta.instance_column();
                let i1 = meta.instance_column();
                let s = meta.selector();
                meta.create_gate("add2inst_pub", |meta| {
                    let s = meta.query_selector(s);
                    let a = meta.query_advice(a, Rotation::cur());
                    let b = meta.query_advice(b, Rotation::cur());
                    let c = meta.query_advice(c, Rotation::cur());
                    let inst0 = meta.query_instance(i0, Rotation::cur());
                    let inst1 = meta.query_instance(i1, Rotation::cur());
                    vec![
                        s.clone() * (a.clone() + b.clone() - c),
                        s.clone() * (a - inst0),
                        s * (b - inst1),
                    ]
                });
                (a, b, c, i0, i1, s)
            }
            fn synthesize(
                &self,
                (a, b, c, _i0, _i1, s): Self::Config,
                mut layouter: impl Layouter<Scalar>,
            ) -> Result<(), PlonkError> {
                layouter.assign_region(
                    || "add2inst_pub",
                    |mut region| {
                        s.enable(&mut region, 0)?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "a",
                            a,
                            0,
                            || Value::known(Scalar::from(5)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "b",
                            b,
                            0,
                            || Value::known(Scalar::from(8)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c",
                            c,
                            0,
                            || Value::known(Scalar::from(13)),
                        )?;
                        Ok(())
                    },
                )
            }
        }

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> =
            keygen_vk(&params, &AddTwoInstPublic::default()).expect("vk");
        let pk = keygen_pk(&params, vk_h2.clone(), &AddTwoInstPublic::default()).expect("pk");

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
            &[AddTwoInstPublic::default()],
            &[&[&[Scalar::from(5u64)][..], &[Scalar::from(8u64)][..]]],
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // ZK1 envelopes
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut proof_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_env, &proof_bytes);
        let cols: [&[Scalar]; 2] = [&[Scalar::from(5u64)][..], &[Scalar::from(8u64)][..]];
        zk1::wrap_append_instances_pasta_fp_cols(&cols, &mut proof_env);

        let backend = "halo2/pasta/ipa/tiny-add2inst-public";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), proof_env);
        assert!(super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_anon_transfer_ipa() {
        use halo2_proofs::{
            circuit::{Layouter, SimpleFloorPlanner, Value},
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{
                Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey, keygen_pk, keygen_vk,
            },
            poly::{Rotation, commitment::Params as _},
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        #[derive(Clone, Default)]
        struct AT;
        impl Circuit<Scalar> for AT {
            type Config = (
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Selector,
            );
            type FloorPlanner = SimpleFloorPlanner;

            type Params = ();
            fn without_witnesses(&self) -> Self {
                Self
            }
            fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
                let in0 = meta.advice_column();
                let in1 = meta.advice_column();
                let out0 = meta.advice_column();
                let out1 = meta.advice_column();
                let s = meta.selector();
                meta.create_gate("anon_transfer", |meta| {
                    let s = meta.query_selector(s);
                    let in0 = meta.query_advice(in0, Rotation::cur());
                    let in1 = meta.query_advice(in1, Rotation::cur());
                    let out0 = meta.query_advice(out0, Rotation::cur());
                    let out1 = meta.query_advice(out1, Rotation::cur());
                    vec![s * (in0 + in1 - (out0 + out1))]
                });
                (in0, in1, out0, out1, s)
            }
            fn synthesize(
                &self,
                (in0, in1, out0, out1, s): Self::Config,
                mut layouter: impl Layouter<Scalar>,
            ) -> Result<(), PlonkError> {
                layouter.assign_region(
                    || "anon_transfer",
                    |mut region| {
                        s.enable(&mut region, 0)?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "in0",
                            in0,
                            0,
                            || Value::known(Scalar::from(7)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "in1",
                            in1,
                            0,
                            || Value::known(Scalar::from(5)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "out0",
                            out0,
                            0,
                            || Value::known(Scalar::from(6)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "out1",
                            out1,
                            0,
                            || Value::known(Scalar::from(6)),
                        )?;
                        Ok(())
                    },
                )
            }
        }

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(&params, &AT::default()).expect("vk");
        let pk = keygen_pk(&params, vk_h2.clone(), &AT::default()).expect("pk");

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
            &[AT::default()],
            &[&[][..]],
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // ZK1 envelopes
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut proof_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_env, &proof_bytes);

        let backend = "halo2/pasta/ipa/tiny-anon-transfer-2x2";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), proof_env);
        assert!(super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_vote_bool_ipa() {
        use halo2_proofs::{
            circuit::{Layouter, SimpleFloorPlanner, Value},
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{
                Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey, keygen_pk, keygen_vk,
            },
            poly::{Rotation, commitment::Params as _},
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        #[derive(Clone, Default)]
        struct VoteBool;
        impl Circuit<Scalar> for VoteBool {
            type Config = (
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Selector,
            );
            type FloorPlanner = SimpleFloorPlanner;

            type Params = ();
            fn without_witnesses(&self) -> Self {
                Self
            }
            fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
                let v = meta.advice_column();
                let s = meta.selector();
                meta.create_gate("vote_bool", |meta| {
                    let s = meta.query_selector(s);
                    let v = meta.query_advice(v, Rotation::cur());
                    let one = halo2_proofs::plonk::Expression::Constant(Scalar::from(1u64));
                    vec![s * (v.clone() * (v - one))]
                });
                (v, s)
            }
            fn synthesize(
                &self,
                (v, s): Self::Config,
                mut layouter: impl Layouter<Scalar>,
            ) -> Result<(), PlonkError> {
                layouter.assign_region(
                    || "vote_bool",
                    |mut region| {
                        s.enable(&mut region, 0)?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "v",
                            v,
                            0,
                            || Value::known(Scalar::from(1u64)),
                        )?;
                        Ok(())
                    },
                )
            }
        }

        let k = 5u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(&params, &VoteBool::default()).expect("vk");
        let pk = keygen_pk(&params, vk_h2.clone(), &VoteBool::default()).expect("pk");

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
            &[VoteBool::default()],
            &[&[][..]],
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // ZK1 envelopes
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut proof_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_env, &proof_bytes);

        let backend = "halo2/pasta/ipa/tiny-vote-bool";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), proof_env);
        assert!(super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_id_public_ipa_with_and_without_inst() {
        use halo2_proofs::{
            circuit::{Layouter, SimpleFloorPlanner, Value},
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{
                Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey, keygen_pk, keygen_vk,
            },
            poly::{Rotation, commitment::Params as _},
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        #[derive(Clone, Default)]
        struct IdPub;
        impl Circuit<Scalar> for IdPub {
            type Config = (
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>,
                halo2_proofs::plonk::Selector,
            );
            type FloorPlanner = SimpleFloorPlanner;

            type Params = ();
            fn without_witnesses(&self) -> Self {
                Self
            }
            fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
                let c = meta.advice_column();
                let inst = meta.instance_column();
                let s = meta.selector();
                meta.create_gate("id_pub", |meta| {
                    let s = meta.query_selector(s);
                    let c = meta.query_advice(c, Rotation::cur());
                    let pubv = meta.query_instance(inst, Rotation::cur());
                    vec![s * (c - pubv)]
                });
                (c, inst, s)
            }
            fn synthesize(
                &self,
                (c, _inst, s): Self::Config,
                mut layouter: impl Layouter<Scalar>,
            ) -> Result<(), PlonkError> {
                layouter.assign_region(
                    || "id_pub",
                    |mut region| {
                        s.enable(&mut region, 0)?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c",
                            c,
                            0,
                            || Value::known(Scalar::from(7)),
                        )?;
                        Ok(())
                    },
                )
            }
        }

        let k = 5u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(&params, &IdPub::default()).expect("vk");
        let pk = keygen_pk(&params, vk_h2.clone(), &IdPub::default()).expect("pk");

        let mut t = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
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
            &[IdPub::default()],
            &[&[&[Scalar::from(7u64)][..]][..]],
            OsRng,
            &mut t,
        )
        .expect("proof created");
        let proof_bytes = t.finalize();

        // ZK1 envelopes
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);

        let backend = "halo2/pasta/ipa/tiny-id-public";
        // Case 1: Missing INST → must fail
        let mut proof_env1 = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_env1, &proof_bytes);
        let vk_box1 = VerifyingKeyBox::new(backend.into(), vk_env.clone());
        let prf_box1 = ProofBox::new(backend.into(), proof_env1);
        assert!(!super::verify_backend(backend, &prf_box1, Some(&vk_box1)));

        // Case 2: With INST → should succeed
        let mut proof_env2 = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_env2, &proof_bytes);
        zk1::wrap_append_instances_pasta_fp(&[Scalar::from(7u64)], &mut proof_env2);
        let vk_box2 = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box2 = ProofBox::new(backend.into(), proof_env2);
        assert!(super::verify_backend(backend, &prf_box2, Some(&vk_box2)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_with_instance_add_ipa() {
        use std::io::Cursor;

        use halo2_proofs::{
            circuit::{Layouter, SimpleFloorPlanner, Value},
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{
                Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey, keygen_pk, keygen_vk,
            },
            poly::Rotation,
            transcript::{Blake2bRead, Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        #[derive(Clone, Default)]
        struct TinyAddPublic;
        impl Circuit<Scalar> for TinyAddPublic {
            type Config = (
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>,
                halo2_proofs::plonk::Selector,
            );
            type FloorPlanner = SimpleFloorPlanner;

            type Params = ();
            fn without_witnesses(&self) -> Self {
                Self
            }
            fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
                let a = meta.advice_column();
                let b = meta.advice_column();
                let c = meta.advice_column();
                let inst = meta.instance_column();
                let s = meta.selector();
                meta.create_gate("add_pub", |meta| {
                    let s = meta.query_selector(s);
                    let a = meta.query_advice(a, Rotation::cur());
                    let b = meta.query_advice(b, Rotation::cur());
                    let c = meta.query_advice(c, Rotation::cur());
                    let pubv = meta.query_instance(inst, Rotation::cur());
                    vec![s.clone() * (a + b - c.clone()), s * (c - pubv)]
                });
                (a, b, c, inst, s)
            }
            fn synthesize(
                &self,
                (a, b, c, _inst, s): Self::Config,
                mut layouter: impl Layouter<Scalar>,
            ) -> Result<(), PlonkError> {
                layouter.assign_region(
                    || "tiny_pub",
                    |mut region| {
                        s.enable(&mut region, 0)?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "a",
                            a,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "b",
                            b,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c",
                            c,
                            0,
                            || Value::known(Scalar::from(4)),
                        )?;
                        Ok(())
                    },
                )
            }
        }

        // ZK1 envelopes
        let k = 5u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(&params, &TinyAddPublic::default()).expect("vk");
        let pk = keygen_pk(&params, vk_h2.clone(), &TinyAddPublic::default()).expect("pk");

        let inst_col = vec![Scalar::from(4u64)];
        let inst_cols: Vec<&[Scalar]> = vec![inst_col.as_slice()];
        let inst_proofs: Vec<&[&[Scalar]]> = vec![inst_cols.as_slice()];

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
            &[TinyAddPublic::default()],
            &inst_proofs,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut proof_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_env, &proof_bytes);
        zk1::wrap_append_instances_pasta_fp(inst_col.as_slice(), &mut proof_env);

        let backend = "halo2/pasta/ipa/tiny-add-public";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), proof_env);
        assert!(super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(feature = "zk-halo2")]
    #[test]
    fn halo2_verify_with_instance_mul_kzg() {
        use std::io::Cursor;

        use halo2_proofs::{
            circuit::{Layouter, SimpleFloorPlanner, Value},
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{
                Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey, keygen_pk, keygen_vk,
            },
            poly::{Rotation, commitment::Params as _},
            transcript::{Blake2bRead, Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        #[derive(Clone, Default)]
        struct TinyMulPublic;
        impl Circuit<Scalar> for TinyMulPublic {
            type Config = (
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>,
                halo2_proofs::plonk::Selector,
            );
            type FloorPlanner = SimpleFloorPlanner;

            type Params = ();
            fn without_witnesses(&self) -> Self {
                Self
            }
            fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
                let a = meta.advice_column();
                let b = meta.advice_column();
                let c = meta.advice_column();
                let inst = meta.instance_column();
                let s = meta.selector();
                meta.create_gate("mul_pub", |meta| {
                    let s = meta.query_selector(s);
                    let a = meta.query_advice(a, Rotation::cur());
                    let b = meta.query_advice(b, Rotation::cur());
                    let c = meta.query_advice(c, Rotation::cur());
                    let pubv = meta.query_instance(inst, Rotation::cur());
                    vec![s.clone() * (a * b - c.clone()), s * (c - pubv)]
                });
                (a, b, c, inst, s)
            }
            fn synthesize(
                &self,
                (a, b, c, _inst, s): Self::Config,
                mut layouter: impl Layouter<Scalar>,
            ) -> Result<(), PlonkError> {
                layouter.assign_region(
                    || "tiny_pub",
                    |mut region| {
                        s.enable(&mut region, 0)?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "a",
                            a,
                            0,
                            || Value::known(Scalar::from(3)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "b",
                            b,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c",
                            c,
                            0,
                            || Value::known(Scalar::from(6)),
                        )?;
                        Ok(())
                    },
                )
            }
        }

        // Params and keys
        let k = 5u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(&params, &TinyMulPublic::default()).expect("vk");
        let pk = keygen_pk(&params, vk_h2.clone(), &TinyMulPublic::default()).expect("pk");

        // Instance: public value 6
        let inst_col = vec![Scalar::from(6u64)];
        let inst_cols: Vec<&[Scalar]> = vec![inst_col.as_slice()];
        let inst_proofs: Vec<&[&[Scalar]]> = vec![inst_cols.as_slice()];

        // Create proof
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
            &[TinyMulPublic::default()],
            &inst_proofs,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // VK container
        let mut vk_container = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_container, k);
        zk1::wrap_append_vk_pasta(&mut vk_container, &vk_h2);

        // Proof + INST
        let mut proof_container = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_container, &proof_bytes);
        zk1::wrap_append_instances_pasta_fp(inst_col.as_slice(), &mut proof_container);

        let backend = "halo2/pasta/tiny-mul-public";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_container);
        let prf_box = ProofBox::new(backend.into(), proof_container);
        assert!(super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(feature = "zk-halo2")]
    #[test]
    fn halo2_verify_with_instance_malformed_length_kzg() {
        // Generate a valid proof for add-public, but craft INST with wrong lengths
        use std::io::Cursor;

        use halo2_proofs::{
            circuit::{Layouter, SimpleFloorPlanner, Value},
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{
                Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey, keygen_pk, keygen_vk,
            },
            poly::{Rotation, commitment::Params as _},
            transcript::{Blake2bRead, Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        #[derive(Clone, Default)]
        struct TinyAddPublic;
        impl Circuit<Scalar> for TinyAddPublic {
            type Config = (
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>,
                halo2_proofs::plonk::Selector,
            );
            type FloorPlanner = SimpleFloorPlanner;

            type Params = ();
            fn without_witnesses(&self) -> Self {
                Self
            }
            fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
                let a = meta.advice_column();
                let b = meta.advice_column();
                let c = meta.advice_column();
                let inst = meta.instance_column();
                let s = meta.selector();
                meta.create_gate("add_pub", |meta| {
                    let s = meta.query_selector(s);
                    let a = meta.query_advice(a, Rotation::cur());
                    let b = meta.query_advice(b, Rotation::cur());
                    let c = meta.query_advice(c, Rotation::cur());
                    let pubv = meta.query_instance(inst, Rotation::cur());
                    vec![s.clone() * (a + b - c.clone()), s * (c - pubv)]
                });
                (a, b, c, inst, s)
            }
            fn synthesize(
                &self,
                (a, b, c, _inst, s): Self::Config,
                mut layouter: impl Layouter<Scalar>,
            ) -> Result<(), PlonkError> {
                layouter.assign_region(
                    || "tiny_pub",
                    |mut region| {
                        s.enable(&mut region, 0)?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "a",
                            a,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "b",
                            b,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c",
                            c,
                            0,
                            || Value::known(Scalar::from(4)),
                        )?;
                        Ok(())
                    },
                )
            }
        }

        let k = 5u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(&params, &TinyAddPublic::default()).expect("vk");
        let pk = keygen_pk(&params, vk_h2.clone(), &TinyAddPublic::default()).expect("pk");

        let inst_col = vec![Scalar::from(4u64)];
        let inst_cols: Vec<&[Scalar]> = vec![inst_col.as_slice()];
        let inst_proofs: Vec<&[&[Scalar]]> = vec![inst_cols.as_slice()];

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
            &[TinyAddPublic::default()],
            &inst_proofs,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        let mut vk_container = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_container, k);
        zk1::wrap_append_vk_pasta(&mut vk_container, &vk_h2);

        let mut proof_container = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_container, &proof_bytes);
        // Malformed INST: declare 2 columns, 1 row but provide only 1 value (should fail)
        proof_container.extend_from_slice(b"I10P");
        proof_container.extend_from_slice(&(2u32).to_le_bytes());
        proof_container.extend_from_slice(&(1u32).to_le_bytes());
        proof_container.extend_from_slice(inst_col[0].to_repr().as_ref());

        let backend = "halo2/pasta/tiny-add-public";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_container);
        let prf_box = ProofBox::new(backend.into(), proof_container);
        assert!(!super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(feature = "zk-halo2")]
    #[test]
    fn halo2_verify_with_instance_noncanonical_kzg() {
        // Generate a valid proof, but use non-canonical instance encoding (all 0xFF)
        use std::io::Cursor;

        use halo2_proofs::{
            circuit::{Layouter, SimpleFloorPlanner, Value},
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{
                Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey, keygen_pk, keygen_vk,
            },
            poly::{Rotation, commitment::Params as _},
            transcript::{Blake2bRead, Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        #[derive(Clone, Default)]
        struct TinyAddPublic;
        impl Circuit<Scalar> for TinyAddPublic {
            type Config = (
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>,
                halo2_proofs::plonk::Selector,
            );
            type FloorPlanner = SimpleFloorPlanner;

            type Params = ();
            fn without_witnesses(&self) -> Self {
                Self
            }
            fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
                let a = meta.advice_column();
                let b = meta.advice_column();
                let c = meta.advice_column();
                let inst = meta.instance_column();
                let s = meta.selector();
                meta.create_gate("add_pub", |meta| {
                    let s = meta.query_selector(s);
                    let a = meta.query_advice(a, Rotation::cur());
                    let b = meta.query_advice(b, Rotation::cur());
                    let c = meta.query_advice(c, Rotation::cur());
                    let pubv = meta.query_instance(inst, Rotation::cur());
                    vec![s.clone() * (a + b - c.clone()), s * (c - pubv)]
                });
                (a, b, c, inst, s)
            }
            fn synthesize(
                &self,
                (a, b, c, _inst, s): Self::Config,
                mut layouter: impl Layouter<Scalar>,
            ) -> Result<(), PlonkError> {
                layouter.assign_region(
                    || "tiny_pub",
                    |mut region| {
                        s.enable(&mut region, 0)?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "a",
                            a,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "b",
                            b,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c",
                            c,
                            0,
                            || Value::known(Scalar::from(4)),
                        )?;
                        Ok(())
                    },
                )
            }
        }

        let k = 5u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(&params, &TinyAddPublic::default()).expect("vk");
        let pk = keygen_pk(&params, vk_h2.clone(), &TinyAddPublic::default()).expect("pk");

        let inst_col = vec![Scalar::from(4u64)];
        let inst_cols: Vec<&[Scalar]> = vec![inst_col.as_slice()];
        let inst_proofs: Vec<&[&[Scalar]]> = vec![inst_cols.as_slice()];

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
            &[TinyAddPublic::default()],
            &inst_proofs,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        let mut vk_container = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_container, k);
        zk1::wrap_append_vk_pasta(&mut vk_container, &vk_h2);

        // Proof with non-canonical INST value
        let mut proof_container = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_container, &proof_bytes);
        proof_container.extend_from_slice(b"I10P");
        proof_container.extend_from_slice(&(1u32).to_le_bytes());
        proof_container.extend_from_slice(&(1u32).to_le_bytes());
        proof_container.extend_from_slice(&[0xFFu8; 32]); // invalid field repr

        let backend = "halo2/pasta/tiny-add-public";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_container);
        let prf_box = ProofBox::new(backend.into(), proof_container);
        assert!(!super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }
