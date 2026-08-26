// Lexically included by `zk::tests` to preserve the existing libtest paths.

#[cfg(feature = "zk-halo2")]
use halo2_proofs::{
    halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
    plonk::{VerifyingKey, keygen_pk, keygen_vk},
    poly::commitment::Params as _,
    transcript::{Blake2bWrite, Challenge255},
};
#[cfg(feature = "zk-halo2")]
use rand_core_06::OsRng;

#[cfg(all(feature = "zk-halo2-ipa", feature = "zk-halo2",))]
fn anon_transfer_pow5_fixture() -> (Vec<u8>, Vec<u8>) {
    let k = 7;
    let params: PastaParams = pasta_params_new(k);
    let circuit = pow5_depth::AnonTransfer2x2CommitMerklePow5::<8>::default();
    let vk: VerifyingKey<Curve> = keygen_vk(&params, &circuit).expect("vk");
    let pk = keygen_pk(&params, vk.clone(), &circuit).expect("pk");
    let values = ipa_fixture::anon_instances(8);
    let columns = [
        &values[0..1],
        &values[1..2],
        &values[2..3],
        &values[3..4],
        &values[4..5],
        &values[5..6],
    ];
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
        &[&columns],
        OsRng,
        &mut transcript,
    )
    .expect("proof created");
    let mut vk_envelope = zk1::wrap_start();
    zk1::wrap_append_ipa_k(&mut vk_envelope, k);
    zk1::wrap_append_vk_pasta(&mut vk_envelope, &vk);
    (vk_envelope, transcript.finalize())
}

#[cfg(all(feature = "zk-halo2-ipa", feature = "zk-halo2",))]
struct TinyCommitOpenFixture {
    vk_envelope: Vec<u8>,
    proof: Vec<u8>,
    commit: halo2_proofs::halo2curves::pasta::Fp,
}

#[cfg(all(feature = "zk-halo2-ipa", feature = "zk-halo2",))]
fn tiny_commit_open_fixture() -> TinyCommitOpenFixture {
    let k = 6;
    let params: PastaParams = pasta_params_new(k);
    let circuit = pasta_tiny::CommitOpen::default();
    let vk: VerifyingKey<Curve> = keygen_vk(&params, &circuit).expect("vk");
    let pk = keygen_pk(&params, vk.clone(), &circuit).expect("pk");
    let m = Scalar::from(11);
    let r = Scalar::from(31);
    let t0 = m + Scalar::from(7);
    let t1 = r + Scalar::from(13);
    let t0_2 = t0 * t0;
    let t1_2 = t1 * t1;
    let commit = Scalar::from(2) * t0_2 * t0_2 * t0 + Scalar::from(3) * t1_2 * t1_2 * t1;
    let instances: [&[&[Scalar]]; 1] = [&[&[commit]]];
    let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
    halo2_proofs::plonk::create_proof::<
        IPACommitmentScheme<Curve>,
        ProverIPA<'_, Curve>,
        Challenge255<Curve>,
        _,
        _,
        _,
    >(&params, &pk, &[circuit], &instances, OsRng, &mut transcript)
    .expect("proof created");
    let mut vk_envelope = zk1::wrap_start();
    zk1::wrap_append_ipa_k(&mut vk_envelope, k);
    zk1::wrap_append_vk_pasta(&mut vk_envelope, &vk);
    TinyCommitOpenFixture {
        vk_envelope,
        proof: transcript.finalize(),
        commit,
    }
}

#[cfg(feature = "zk-halo2")]
struct TinyAddPublicFixture {
    vk_envelope: Vec<u8>,
    proof: Vec<u8>,
    instance: halo2_proofs::halo2curves::pasta::Fp,
}

#[cfg(feature = "zk-halo2")]
fn tiny_add_public_fixture() -> TinyAddPublicFixture {
    let k = 5;
    let params: PastaParams = pasta_params_new(k);
    let circuit = pasta_tiny::AddPublic::default();
    let vk: VerifyingKey<Curve> = keygen_vk(&params, &circuit).expect("vk");
    let pk = keygen_pk(&params, vk.clone(), &circuit).expect("pk");
    let instance = Scalar::from(4);
    let instances: [&[&[Scalar]]; 1] = [&[&[instance]]];
    let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
    halo2_proofs::plonk::create_proof::<
        IPACommitmentScheme<Curve>,
        ProverIPA<'_, Curve>,
        Challenge255<Curve>,
        _,
        _,
        _,
    >(&params, &pk, &[circuit], &instances, OsRng, &mut transcript)
    .expect("proof created");
    let mut vk_envelope = zk1::wrap_start();
    zk1::wrap_append_ipa_k(&mut vk_envelope, k);
    zk1::wrap_append_vk_pasta(&mut vk_envelope, &vk);
    TinyAddPublicFixture {
        vk_envelope,
        proof: transcript.finalize(),
        instance,
    }
}

#[cfg(all(feature = "zk-halo2-ipa", feature = "zk-halo2",))]
#[test]
fn halo2_verify_anon_transfer_2x2_merkle8_pow5_ipa_zk1_noncanonical() {
    use ff::{Field as _, PrimeField as _};
    let (vk_envelope, proof) = anon_transfer_pow5_fixture();
    let mut prf_env = zk1::wrap_start();
    zk1::wrap_append_proof(&mut prf_env, &proof);
    prf_env.extend_from_slice(b"I10P");
    prf_env.extend_from_slice(&(6u32).to_le_bytes()); // cols
    prf_env.extend_from_slice(&(1u32).to_le_bytes()); // rows
    // Append 5 zeros, then one non-canonical
    for _ in 0..5 {
        prf_env.extend_from_slice(Scalar::ZERO.to_repr().as_ref());
    }
    prf_env.extend_from_slice(&[0xFFu8; 32]);

    let backend = backend_tag_anon_transfer_merkle(8, true);
    let vk_box = VerifyingKeyBox::new(backend.clone().into(), vk_envelope);
    let prf_box = ProofBox::new(backend.clone().into(), prf_env);
    assert!(!super::verify_halo2_ipa(&backend, &prf_box, Some(&vk_box)));
}

#[cfg(all(feature = "zk-halo2-ipa", feature = "zk-halo2",))]
#[test]
fn halo2_verify_anon_transfer_2x2_merkle8_pow5_ipa_zk1_invalid_header() {
    let (vk_envelope, proof) = anon_transfer_pow5_fixture();
    // PROF ok, I10P invalid (rows=0)
    let mut prf_env = zk1::wrap_start();
    zk1::wrap_append_proof(&mut prf_env, &proof);
    prf_env.extend_from_slice(b"I10P");
    prf_env.extend_from_slice(&(1u32).to_le_bytes()); // cols=1
    prf_env.extend_from_slice(&(0u32).to_le_bytes()); // rows=0 (invalid)

    let backend = backend_tag_anon_transfer_merkle(8, true);
    let vk_box = VerifyingKeyBox::new(backend.clone().into(), vk_envelope);
    let prf_box = ProofBox::new(backend.clone().into(), prf_env);
    assert!(!super::verify_halo2_ipa(&backend, &prf_box, Some(&vk_box)));
}

// --- Tiny constrained Pow5 circuits (base) negative ZK1 tests ---

#[cfg(all(feature = "zk-halo2-ipa", feature = "zk-halo2",))]
#[test]
fn halo2_verify_tiny_commit_open_ipa_zk1_truncated_prof() {
    let k = 6u32;
    let params: PastaParams = pasta_params_new(k);
    let vk_h2: VerifyingKey<Curve> =
        keygen_vk(&params, &pasta_tiny::CommitOpen::default()).expect("vk");
    let pk = keygen_pk(&params, vk_h2.clone(), &pasta_tiny::CommitOpen::default()).expect("pk");
    let commit = pasta_tiny::constrained_pow5_pair(Scalar::from(11), Scalar::from(31));
    let instances: [&[&[Scalar]]; 1] = [&[&[commit]]];

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
        &[pasta_tiny::CommitOpen::default()],
        &instances,
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
    assert!(!super::verify_halo2_ipa(backend, &prf_box, Some(&vk_box)));
}

#[cfg(all(feature = "zk-halo2-ipa", feature = "zk-halo2",))]
#[test]
fn halo2_verify_tiny_merkle2_ipa_zk1_invalid_header_extreme() {
    let k = 6u32;
    let params: PastaParams = pasta_params_new(k);
    let vk_h2: VerifyingKey<Curve> =
        keygen_vk(&params, &pasta_tiny::Merkle2::default()).expect("vk");
    let pk = keygen_pk(&params, vk_h2.clone(), &pasta_tiny::Merkle2::default()).expect("pk");
    let root = pasta_tiny::merkle2_sample_root();
    let instances: [&[&[Scalar]]; 1] = [&[&[root]]];

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
        &[pasta_tiny::Merkle2::default()],
        &instances,
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
    assert!(!super::verify_halo2_ipa(backend, &prf_box1, Some(&vk_box)));

    // Case 2: rows > MAX_INST_ROWS
    let mut prf_env2 = zk1::wrap_start();
    zk1::wrap_append_proof(&mut prf_env2, &proof_bytes);
    prf_env2.extend_from_slice(b"I10P");
    prf_env2.extend_from_slice(&(1u32).to_le_bytes());
    let rows_ext = (super::MAX_INST_ROWS as u32).saturating_add(1);
    prf_env2.extend_from_slice(&rows_ext.to_le_bytes());

    let prf_box2 = ProofBox::new(backend.into(), prf_env2);
    assert!(!super::verify_halo2_ipa(backend, &prf_box2, Some(&vk_box)));
}

#[cfg(all(feature = "zk-halo2-ipa", feature = "zk-halo2",))]
#[test]
fn halo2_verify_tiny_commit_open_ipa_zk1_positive() {
    let fixture = tiny_commit_open_fixture();
    let mut prf_env = zk1::wrap_start();
    zk1::wrap_append_proof(&mut prf_env, &fixture.proof);
    zk1::wrap_append_instances_pasta_fp_cols(&[&[fixture.commit][..]], &mut prf_env);

    let backend = "halo2/pasta/ipa/tiny-commit-open";
    let vk_box = VerifyingKeyBox::new(backend.into(), fixture.vk_envelope);
    let prf_box = ProofBox::new(backend.into(), prf_env);
    assert!(super::verify_halo2_ipa(backend, &prf_box, Some(&vk_box)));
}

#[cfg(all(feature = "zk-halo2-ipa", feature = "zk-halo2",))]
#[test]
fn halo2_verify_tiny_merkle2_ipa_zk1_positive() {
    let k = 6u32;
    let params: PastaParams = pasta_params_new(k);
    let vk_h2: VerifyingKey<Curve> =
        keygen_vk(&params, &pasta_tiny::Merkle2::default()).expect("vk");
    let pk = keygen_pk(&params, vk_h2.clone(), &pasta_tiny::Merkle2::default()).expect("pk");

    let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
    let root = pasta_tiny::merkle2_sample_root();
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
        &[pasta_tiny::Merkle2::default()],
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
    assert!(super::verify_halo2_ipa(backend, &prf_box, Some(&vk_box)));
}

// ZK1 is canonical: duplicate proof payloads and unknown tags fail closed.
#[cfg(all(feature = "zk-halo2-ipa", feature = "zk-halo2",))]
#[test]
fn halo2_verify_tiny_commit_open_ipa_zk1_multiple_prof_and_unknown_rejects() {
    let fixture = tiny_commit_open_fixture();
    let mut prf_env = zk1::wrap_start();
    // PROF #1 truncated
    prf_env.extend_from_slice(b"PROF");
    prf_env.extend_from_slice(&(fixture.proof.len() as u32).to_le_bytes());
    let cut = fixture.proof.len().saturating_sub(1);
    prf_env.extend_from_slice(&fixture.proof[..cut]);
    // Unknown TLV
    prf_env.extend_from_slice(b"UKNW");
    prf_env.extend_from_slice(&(4u32).to_le_bytes());
    prf_env.extend_from_slice(&[1, 2, 3, 4]);
    // PROF #2 correct
    zk1::wrap_append_proof(&mut prf_env, &fixture.proof);
    zk1::wrap_append_instances_pasta_fp_cols(&[&[fixture.commit][..]], &mut prf_env);

    let backend = "halo2/pasta/ipa/tiny-commit-open";
    let vk_box = VerifyingKeyBox::new(backend.into(), fixture.vk_envelope);
    let prf_box = ProofBox::new(backend.into(), prf_env);
    assert!(!super::verify_halo2_ipa(backend, &prf_box, Some(&vk_box)));
}

#[cfg(all(feature = "zk-halo2-ipa", feature = "zk-halo2",))]
#[test]
fn halo2_verify_tiny_commit_open_ipa_zk1_unknown_tlv_stress_rejects() {
    let fixture = tiny_commit_open_fixture();
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
    zk1::wrap_append_proof(&mut prf_env, &fixture.proof);
    zk1::wrap_append_instances_pasta_fp_cols(&[&[fixture.commit][..]], &mut prf_env);

    let backend = "halo2/pasta/ipa/tiny-commit-open";
    let vk_box = VerifyingKeyBox::new(backend.into(), fixture.vk_envelope);
    let prf_box = ProofBox::new(backend.into(), prf_env);
    assert!(!super::verify_halo2_ipa(backend, &prf_box, Some(&vk_box)));
}

// ZK1 duplicate instance payloads fail closed regardless of ordering.
#[cfg(all(feature = "zk-halo2-ipa", feature = "zk-halo2",))]
#[test]
fn halo2_verify_tiny_commit_open_ipa_zk1_duplicate_i10p_last_correct_rejects() {
    let fixture = tiny_commit_open_fixture();
    let zero = fixture.commit - fixture.commit;
    let mut prf_env = zk1::wrap_start();
    zk1::wrap_append_proof(&mut prf_env, &fixture.proof);
    // I10P #1 (wrong commit = 0)
    zk1::wrap_append_instances_pasta_fp_cols(&[&[zero][..]], &mut prf_env);
    // I10P #2 (correct)
    zk1::wrap_append_instances_pasta_fp_cols(&[&[fixture.commit][..]], &mut prf_env);

    let backend = "halo2/pasta/ipa/tiny-commit-open";
    let vk_box = VerifyingKeyBox::new(backend.into(), fixture.vk_envelope);
    let prf_box = ProofBox::new(backend.into(), prf_env);
    assert!(!super::verify_halo2_ipa(backend, &prf_box, Some(&vk_box)));
}

// Reversed duplicate-instance ordering must fail identically.
#[cfg(all(feature = "zk-halo2-ipa", feature = "zk-halo2",))]
#[test]
fn halo2_verify_tiny_commit_open_ipa_zk1_duplicate_i10p_last_wrong_rejects() {
    let fixture = tiny_commit_open_fixture();
    let zero = fixture.commit - fixture.commit;
    let mut prf_env = zk1::wrap_start();
    zk1::wrap_append_proof(&mut prf_env, &fixture.proof);
    // I10P #1 correct
    zk1::wrap_append_instances_pasta_fp_cols(&[&[fixture.commit][..]], &mut prf_env);
    // I10P #2 wrong
    zk1::wrap_append_instances_pasta_fp_cols(&[&[zero][..]], &mut prf_env);

    let backend = "halo2/pasta/ipa/tiny-commit-open";
    let vk_box = VerifyingKeyBox::new(backend.into(), fixture.vk_envelope);
    let prf_box = ProofBox::new(backend.into(), prf_env);
    assert!(!super::verify_halo2_ipa(backend, &prf_box, Some(&vk_box)));
}

// Randomized deterministic unknown-TLV stress must fail closed.
#[cfg(all(feature = "zk-halo2-ipa", feature = "zk-halo2",))]
#[test]
fn halo2_verify_tiny_commit_open_ipa_zk1_unknown_tlv_randomized_rejects() {
    let fixture = tiny_commit_open_fixture();
    let mut seed: u64 = 0xC0FFEE_F00D_BAAD;
    for _round in 0..4 {
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
        zk1::wrap_append_proof(&mut prf_env, &fixture.proof);
        zk1::wrap_append_instances_pasta_fp_cols(&[&[fixture.commit][..]], &mut prf_env);

        let backend = "halo2/pasta/ipa/tiny-commit-open";
        let vk_box = VerifyingKeyBox::new(backend.into(), fixture.vk_envelope.clone());
        let prf_box = ProofBox::new(backend.into(), prf_env);
        assert!(!super::verify_halo2_ipa(backend, &prf_box, Some(&vk_box)));
    }
}

#[cfg(all(feature = "zk-halo2-ipa", feature = "zk-halo2",))]
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
    let vk_h2: VerifyingKey<Curve> =
        keygen_vk(&params, &pasta_tiny::CommitOpen::default()).expect("vk");
    let pk = keygen_pk(&params, vk_h2.clone(), &pasta_tiny::CommitOpen::default()).expect("pk");
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
        &[pasta_tiny::CommitOpen::default()],
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
            super::verify_halo2_ipa(backend, &prf_box, Some(&vk_box)),
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
#[cfg(all(feature = "zk-halo2-ipa", feature = "zk-halo2",))]
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
    assert!(!super::verify_halo2_ipa(backend, &prf_box, Some(&vk_box)));
}

#[cfg(all(feature = "zk-halo2-ipa", feature = "zk-halo2",))]
#[test]
fn halo2_verify_add2inst_public_ipa() {
    let k = 6u32;
    let params: PastaParams = pasta_params_new(k);
    let vk_h2: VerifyingKey<Curve> =
        keygen_vk(&params, &pasta_tiny::AddTwoInstPublic::default()).expect("vk");
    let pk = keygen_pk(
        &params,
        vk_h2.clone(),
        &pasta_tiny::AddTwoInstPublic::default(),
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
        &[pasta_tiny::AddTwoInstPublic::default()],
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
    assert!(super::verify_halo2_ipa(backend, &prf_box, Some(&vk_box)));
}

#[cfg(all(feature = "zk-halo2-ipa", feature = "zk-halo2",))]
#[test]
fn halo2_verify_anon_transfer_ipa() {
    let k = 6u32;
    let params: PastaParams = pasta_params_new(k);
    let vk_h2: VerifyingKey<Curve> = keygen_vk(&params, &pasta_tiny::AnonTransfer2x2).expect("vk");
    let pk = keygen_pk(&params, vk_h2.clone(), &pasta_tiny::AnonTransfer2x2).expect("pk");

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
        &[pasta_tiny::AnonTransfer2x2],
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
    assert!(super::verify_halo2_ipa(backend, &prf_box, Some(&vk_box)));
}

#[cfg(all(feature = "zk-halo2-ipa", feature = "zk-halo2",))]
#[test]
fn halo2_verify_vote_bool_ipa() {
    let k = 5u32;
    let params: PastaParams = pasta_params_new(k);
    let vk_h2: VerifyingKey<Curve> =
        keygen_vk(&params, &pasta_tiny::VoteBool::default()).expect("vk");
    let pk = keygen_pk(&params, vk_h2.clone(), &pasta_tiny::VoteBool::default()).expect("pk");

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
        &[pasta_tiny::VoteBool::default()],
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
    assert!(super::verify_halo2_ipa(backend, &prf_box, Some(&vk_box)));
}

#[cfg(all(feature = "zk-halo2-ipa", feature = "zk-halo2",))]
#[test]
fn halo2_verify_id_public_ipa_with_and_without_inst() {
    let k = 5u32;
    let params: PastaParams = pasta_params_new(k);
    let vk_h2: VerifyingKey<Curve> =
        keygen_vk(&params, &pasta_tiny::IdPublic::default()).expect("vk");
    let pk = keygen_pk(&params, vk_h2.clone(), &pasta_tiny::IdPublic::default()).expect("pk");

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
        &[pasta_tiny::IdPublic::default()],
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
    assert!(!super::verify_halo2_ipa(backend, &prf_box1, Some(&vk_box1)));

    // Case 2: With INST → should succeed
    let mut proof_env2 = zk1::wrap_start();
    zk1::wrap_append_proof(&mut proof_env2, &proof_bytes);
    zk1::wrap_append_instances_pasta_fp(&[Scalar::from(7u64)], &mut proof_env2);
    let vk_box2 = VerifyingKeyBox::new(backend.into(), vk_env);
    let prf_box2 = ProofBox::new(backend.into(), proof_env2);
    assert!(super::verify_halo2_ipa(backend, &prf_box2, Some(&vk_box2)));
}

#[cfg(all(feature = "zk-halo2-ipa", feature = "zk-halo2",))]
#[test]
fn halo2_verify_with_instance_add_ipa() {
    let fixture = tiny_add_public_fixture();
    let mut proof_env = zk1::wrap_start();
    zk1::wrap_append_proof(&mut proof_env, &fixture.proof);
    zk1::wrap_append_instances_pasta_fp(&[fixture.instance], &mut proof_env);

    let backend = "halo2/pasta/ipa/tiny-add-public";
    let vk_box = VerifyingKeyBox::new(backend.into(), fixture.vk_envelope);
    let prf_box = ProofBox::new(backend.into(), proof_env);
    assert!(super::verify_halo2_ipa(backend, &prf_box, Some(&vk_box)));
}

#[cfg(feature = "zk-halo2")]
#[test]
fn halo2_verify_with_instance_mul_kzg() {
    // Params and keys
    let k = 5u32;
    let params: PastaParams = pasta_params_new(k);
    let vk_h2: VerifyingKey<Curve> =
        keygen_vk(&params, &pasta_tiny::MulPublic::default()).expect("vk");
    let pk = keygen_pk(&params, vk_h2.clone(), &pasta_tiny::MulPublic::default()).expect("pk");

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
        &[pasta_tiny::MulPublic::default()],
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
    assert!(super::verify_halo2(backend, &prf_box, Some(&vk_box)));
}

#[cfg(feature = "zk-halo2")]
#[test]
fn halo2_verify_with_instance_malformed_length_kzg() {
    let fixture = tiny_add_public_fixture();
    let mut proof_container = zk1::wrap_start();
    zk1::wrap_append_proof(&mut proof_container, &fixture.proof);
    // Malformed INST: declare 2 columns, 1 row but provide only 1 value (should fail)
    proof_container.extend_from_slice(b"I10P");
    proof_container.extend_from_slice(&(2u32).to_le_bytes());
    proof_container.extend_from_slice(&(1u32).to_le_bytes());
    proof_container.extend_from_slice(fixture.instance.to_repr().as_ref());

    let backend = "halo2/pasta/tiny-add-public";
    let vk_box = VerifyingKeyBox::new(backend.into(), fixture.vk_envelope);
    let prf_box = ProofBox::new(backend.into(), proof_container);
    assert!(!super::verify_halo2(backend, &prf_box, Some(&vk_box)));
}

#[cfg(feature = "zk-halo2")]
#[test]
fn halo2_verify_with_instance_noncanonical_kzg() {
    let fixture = tiny_add_public_fixture();
    let mut proof_container = zk1::wrap_start();
    zk1::wrap_append_proof(&mut proof_container, &fixture.proof);
    proof_container.extend_from_slice(b"I10P");
    proof_container.extend_from_slice(&(1u32).to_le_bytes());
    proof_container.extend_from_slice(&(1u32).to_le_bytes());
    proof_container.extend_from_slice(&[0xFFu8; 32]); // invalid field repr

    let backend = "halo2/pasta/tiny-add-public";
    let vk_box = VerifyingKeyBox::new(backend.into(), fixture.vk_envelope);
    let prf_box = ProofBox::new(backend.into(), proof_container);
    assert!(!super::verify_halo2(backend, &prf_box, Some(&vk_box)));
}
