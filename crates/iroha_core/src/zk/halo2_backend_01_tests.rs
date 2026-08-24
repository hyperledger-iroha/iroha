// Lexically included by `zk::tests` to preserve the existing libtest paths.

#[cfg(all(
    feature = "halo2-dev-tests",
    any(feature = "zk-halo2", feature = "zk-halo2-ipa")
))]
use std::sync::Arc;

#[cfg(feature = "zk-halo2")]
use super::*;
#[cfg(all(feature = "zk-tests", feature = "halo2-dev-tests"))]
use crate::zk::pasta_tiny::{
    VoteBoolCommitMerkle8, vote_bool_commit_merkle8_sample_inputs,
    vote_bool_commit_merkle8_witnesses,
};
#[cfg(feature = "zk-halo2")]
use ff::PrimeField;
#[cfg(feature = "zk-halo2")]
use halo2_proofs::poly::{
    commitment::ParamsProver,
    ipa::{commitment::IPACommitmentScheme, multiopen::ProverIPA},
};
#[cfg(feature = "zk-halo2")]
use halo2_proofs::transcript::TranscriptWriterBuffer;

#[test]
fn vote_bool_commit_merkle8_mock_prover_succeeds() {
    use halo2_proofs::dev::MockProver;

    let circuit = VoteBoolCommitMerkle8::default();
    let (v_val, rho_val, sibling_vals, dir_vals) = vote_bool_commit_merkle8_sample_inputs();
    let (commit, _witnesses, root) =
        vote_bool_commit_merkle8_witnesses(v_val, rho_val, sibling_vals, dir_vals);
    let public_inputs = vec![vec![commit], vec![root]];
    let prover = MockProver::run(8, &circuit, public_inputs).expect("mock prover");
    prover.assert_satisfied();
}

#[test]
fn constrained_pow5_vote_membership_rejects_a_forged_commitment() {
    use halo2_proofs::{dev::MockProver, halo2curves::pasta::Fp as Scalar};

    let circuit = crate::zk::pow5_depth::VoteBoolCommitMerklePow5::<8>::default();
    let commitment =
        crate::zk::pasta_tiny::constrained_pow5_pair(Scalar::from(1), Scalar::from(12_345));
    let mut root = commitment;
    for level in 0u64..8 {
        root = crate::zk::pasta_tiny::constrained_pow5_pair(root, Scalar::from(20 + level));
    }
    let prover =
        MockProver::run(8, &circuit, vec![vec![commitment], vec![root]]).expect("mock prover");
    prover.assert_satisfied();

    let forged = MockProver::run(
        8,
        &circuit,
        vec![vec![commitment + Scalar::from(1)], vec![root]],
    )
    .expect("mock prover");
    assert!(
        forged.verify().is_err(),
        "the Pow5 commitment must be constrained to the private vote and blinding"
    );
}

#[cfg(feature = "zk-halo2")]
#[test]
fn commit_open_rejects_additive_placeholder_commitment() {
    use halo2_proofs::{dev::MockProver, halo2curves::pasta::Fp as Scalar};

    let circuit = crate::zk::pasta_tiny::CommitOpen::default();
    let commitment =
        crate::zk::pasta_tiny::constrained_pow5_pair(Scalar::from(11), Scalar::from(31));
    let prover = MockProver::run(5, &circuit, vec![vec![commitment]]).expect("mock prover");
    prover.assert_satisfied();

    let additive_placeholder_commitment = Scalar::from(11 + 31);
    let stale = MockProver::run(5, &circuit, vec![vec![additive_placeholder_commitment]])
        .expect("mock prover");
    assert!(
        stale.verify().is_err(),
        "commit-open must not accept the old additive placeholder commitment"
    );
}

#[cfg(feature = "zk-halo2")]
#[test]
fn tiny_merkle2_rejects_additive_placeholder_root() {
    use halo2_proofs::{dev::MockProver, halo2curves::pasta::Fp as Scalar};

    let circuit = crate::zk::pasta_tiny::Merkle2::default();
    let first = crate::zk::pasta_tiny::constrained_pow5_pair(Scalar::from(9), Scalar::from(5));
    let root = crate::zk::pasta_tiny::constrained_pow5_pair(first, Scalar::from(7));
    let prover = MockProver::run(5, &circuit, vec![vec![root]]).expect("mock prover");
    prover.assert_satisfied();

    let additive_placeholder_root = Scalar::from(9 + 5 + 7);
    let stale =
        MockProver::run(5, &circuit, vec![vec![additive_placeholder_root]]).expect("mock prover");
    assert!(
        stale.verify().is_err(),
        "Merkle2 must not accept the old additive placeholder root"
    );
}

#[cfg(feature = "zk-halo2")]
#[test]
fn anon_transfer_commit_rejects_unshifted_placeholder_commitment() {
    use halo2_proofs::{dev::MockProver, halo2curves::pasta::Fp as Scalar};

    fn unshifted_pow5_pair(lhs: Scalar, rhs: Scalar) -> Scalar {
        let lhs2 = lhs * lhs;
        let lhs4 = lhs2 * lhs2;
        let lhs5 = lhs4 * lhs;
        let rhs2 = rhs * rhs;
        let rhs4 = rhs2 * rhs2;
        let rhs5 = rhs4 * rhs;
        Scalar::from(2) * lhs5 + Scalar::from(3) * rhs5 + Scalar::from(7)
    }

    let circuit = crate::zk::pasta_tiny::AnonTransfer2x2Commit::default();
    let cm_in0 = crate::zk::pasta_tiny::constrained_pow5_pair(Scalar::from(7), Scalar::from(11));
    let cm_in1 = crate::zk::pasta_tiny::constrained_pow5_pair(Scalar::from(5), Scalar::from(13));
    let cm_out0 = crate::zk::pasta_tiny::constrained_pow5_pair(Scalar::from(6), Scalar::from(17));
    let cm_out1 = crate::zk::pasta_tiny::constrained_pow5_pair(Scalar::from(6), Scalar::from(19));
    let nullifier =
        crate::zk::pasta_tiny::constrained_pow5_pair(Scalar::from(1_234_567), Scalar::from(42));
    let public_inputs = vec![
        vec![cm_in0],
        vec![cm_in1],
        vec![cm_out0],
        vec![cm_out1],
        vec![nullifier],
    ];
    let prover = MockProver::run(6, &circuit, public_inputs).expect("mock prover");
    prover.assert_satisfied();

    let stale_cm_in0 = unshifted_pow5_pair(Scalar::from(7), Scalar::from(11));
    let stale = MockProver::run(
        6,
        &circuit,
        vec![
            vec![stale_cm_in0],
            vec![cm_in1],
            vec![cm_out0],
            vec![cm_out1],
            vec![nullifier],
        ],
    )
    .expect("mock prover");
    assert!(
        stale.verify().is_err(),
        "anon-transfer must not accept the old unshifted placeholder commitment"
    );
}

#[cfg(feature = "zk-halo2")]
#[test]
fn vote_bool_merkle2_rejects_stale_merkle_shortcut() {
    use halo2_proofs::{dev::MockProver, halo2curves::pasta::Fp as Scalar};

    let circuit = crate::zk::pasta_tiny::VoteBoolCommitMerkle2::default();
    let commit =
        crate::zk::pasta_tiny::constrained_pow5_pair(Scalar::from(1), Scalar::from(12_345));
    let first = crate::zk::pasta_tiny::constrained_pow5_pair(commit, Scalar::from(5));
    let root = crate::zk::pasta_tiny::constrained_pow5_pair(first, Scalar::from(7));
    let prover = MockProver::run(6, &circuit, vec![vec![commit], vec![root]]).expect("mock prover");
    prover.assert_satisfied();

    let stale_shortcut_root = Scalar::from(1 + 12_345 + 5 + 7 + 11);
    let stale = MockProver::run(6, &circuit, vec![vec![commit], vec![stale_shortcut_root]])
        .expect("mock prover");
    assert!(
        stale.verify().is_err(),
        "vote Merkle2 must not accept the old additive shortcut root"
    );
}

#[cfg(all(
    feature = "halo2-dev-tests",
    any(feature = "zk-halo2", feature = "zk-halo2-ipa")
))]
#[allow(dead_code)]
fn backend_tag_vote_bool_commit_merkle(depth: usize, use_pow5: bool) -> String {
    let algorithm = if use_pow5 { "-pow5" } else { "" };
    format!("halo2/pasta/ipa/vote-bool-commit-merkle{depth}{algorithm}")
}

#[cfg(all(
    feature = "halo2-dev-tests",
    any(feature = "zk-halo2", feature = "zk-halo2-ipa")
))]
#[allow(dead_code)]
fn backend_tag_anon_transfer_merkle(depth: usize, use_pow5: bool) -> String {
    let algorithm = if use_pow5 { "-pow5" } else { "" };
    format!("halo2/pasta/ipa/anon-transfer-2x2-merkle{depth}{algorithm}")
}
#[cfg(all(
    feature = "halo2-dev-tests",
    any(feature = "zk-halo2", feature = "zk-halo2-ipa")
))]
#[test]
fn vk_cache_reuses_entries() {
    let params: PastaParams = pasta_params_new(5);
    let backend = "halo2/pasta/cache-test";
    let circuit = pasta_tiny::Add;
    let first = keygen_vk_cached(backend, &params, &circuit).expect("vk");
    let second = keygen_vk_cached(backend, &params, &circuit).expect("vk");
    assert!(Arc::ptr_eq(&first, &second));

    if let Some(cache) = super::BUILTIN_VK_CACHE.get() {
        let guard = cache.lock().expect("cache poisoned");
        let key = super::BuiltinVkCacheKey {
            backend: backend.to_owned(),
            params_fingerprint: super::params_fingerprint(&params),
        };
        let Some(cached) = guard.get(&key) else {
            panic!("expected builtin verifying key cache entry for {backend}");
        };
        assert!(Arc::ptr_eq(&first, cached));
    } else {
        panic!("cache not initialized");
    }
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[test]
fn verifier_key_cache_rejects_parseable_key_for_another_circuit() {
    let params = pasta_params_new(IVM_EXECUTION_V1_IPA_K);
    let attacker_vk =
        halo2_backend::keygen_vk(&params, &pasta_tiny::Add).expect("attacker fixture vk");
    let mut attacker_bytes = zk1::wrap_start();
    zk1::wrap_append_ipa_k(&mut attacker_bytes, IVM_EXECUTION_V1_IPA_K);
    zk1::wrap_append_vk_pasta(&mut attacker_bytes, &attacker_vk);
    let attacker_vk_box =
        VerifyingKeyBox::new(IVM_EXECUTION_V1_HALO2_BACKEND.to_owned(), attacker_bytes);

    let expected_circuit = pasta_tiny::IvmExecutionBindV1::default();
    let result = resolve_vk_cached(
        IVM_EXECUTION_V1_HALO2_BACKEND,
        &params,
        &attacker_vk_box,
        &expected_circuit,
        || halo2_backend::keygen_vk(&params, &expected_circuit),
    );
    assert!(
        result.is_err(),
        "a parseable demo-circuit key must not be relabeled as ivm-execution-v1"
    );
}

#[cfg(all(
    feature = "halo2-dev-tests",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa"
))]
#[test]
fn packaged_vk_cache_rejects_unparseable_key_without_runtime_keygen() {
    let params: PastaParams = pasta_params_new(5);
    let backend = "halo2/pasta/package-only-cache-test";
    let circuit = pasta_tiny::Add;
    let vk = halo2_backend::keygen_vk(&params, &circuit).expect("vk");
    let mut valid_bytes = zk1::wrap_start();
    zk1::wrap_append_ipa_k(&mut valid_bytes, 5);
    zk1::wrap_append_vk_pasta(&mut valid_bytes, &vk);
    let valid_vk_box = VerifyingKeyBox::new(backend.to_owned(), valid_bytes);

    let packaged = resolve_packaged_vk_cached(backend, &params, &valid_vk_box, &circuit)
        .expect("valid packaged vk parses");
    let packaged_again = resolve_packaged_vk_cached(backend, &params, &valid_vk_box, &circuit)
        .expect("valid packaged vk cache hit");
    assert!(Arc::ptr_eq(&packaged, &packaged_again));

    let invalid_vk_box = VerifyingKeyBox::new(
        backend.to_owned(),
        b"not-a-zk1-packaged-verifying-key".to_vec(),
    );
    assert!(
        resolve_packaged_vk_cached::<pasta_tiny::Add>(backend, &params, &invalid_vk_box, &circuit,)
            .is_err(),
        "package-only compact verifier dispatch must reject unparsable verifier-key bytes"
    );

    let mut runtime_keygen_attempted = false;
    let runtime_keygen_result =
        resolve_vk_cached(backend, &params, &invalid_vk_box, &circuit, || {
            runtime_keygen_attempted = true;
            halo2_backend::keygen_vk(&params, &circuit)
        });
    assert!(
        runtime_keygen_result.is_err(),
        "runtime-keygen verifier resolver must reject the forged verifier-key commitment"
    );
    assert!(
        runtime_keygen_attempted,
        "runtime-keygen verifier resolver attempts runtime keygen on unparsable bytes"
    );
    assert!(
        resolve_packaged_vk_cached::<pasta_tiny::Add>(backend, &params, &invalid_vk_box, &circuit,)
            .is_err(),
        "package-only resolver must keep rejecting after a runtime-keygen attempt"
    );
}

#[cfg(all(feature = "zk-halo2-ipa", feature = "zk-halo2",))]
#[test]
fn zk1_envelope_pasta_ipa_verify_add_public() {
    use halo2_proofs::{
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        plonk::{VerifyingKey, keygen_pk, keygen_vk},
        transcript::{Blake2bWrite, Challenge255},
    };
    use rand_core_06::OsRng;

    let k = 5u32;
    let params: PastaParams = pasta_params_new(k);
    let vk_h2: VerifyingKey<Curve> =
        keygen_vk(&params, &pasta_tiny::AddPublic::default()).expect("vk");
    let pk = keygen_pk(&params, vk_h2.clone(), &pasta_tiny::AddPublic::default()).expect("pk");

    let inst_col = vec![Scalar::from(4u64)];
    let inst_cols: Vec<&[Scalar]> = vec![inst_col.as_slice()];
    let inst_proofs: Vec<&[&[Scalar]]> = vec![inst_cols.as_slice()];

    let mut transcript = Blake2bWrite::<_, _, Challenge255<_>>::init(vec![]);
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
        &[pasta_tiny::AddPublic::default()],
        &inst_proofs,
        OsRng,
        &mut transcript,
    )
    .expect("proof created");
    let proof_bytes = transcript.finalize();

    // Build ZK1 envelopes: VK has IPAK(k); proof has PROF + I10P(inst)
    let mut vk_env = zk1::wrap_start();
    zk1::wrap_append_ipa_k(&mut vk_env, k);
    zk1::wrap_append_circuit_id(&mut vk_env, "halo2/pasta/ipa/kaigi-roster-v1");
    zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);

    let mut prf_env = zk1::wrap_start();
    zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
    zk1::wrap_append_instances_pasta_fp(inst_col.as_slice(), &mut prf_env);

    let backend = "halo2/pasta/ipa/tiny-add-public";
    let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
    let prf_box = ProofBox::new(backend.into(), prf_env);
    assert!(super::verify_halo2_ipa(backend, &prf_box, Some(&vk_box)));
}

#[cfg(all(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[test]
fn kaigi_roster_backend_accepts_valid_proof() {
    use halo2_proofs::{
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        plonk::{keygen_pk, keygen_vk},
        transcript::{Blake2bWrite, Challenge255},
    };
    use kaigi_zk::{
        KAIGI_ROSTER_CIRCUIT_K, KAIGI_ROSTER_PUBLIC_INPUTS_SCHEMA_V1, compute_commitment,
        compute_nullifier, empty_roster_root_hash, roster_root_limbs,
    };
    use rand_core_06::OsRng;

    let k = KAIGI_ROSTER_CIRCUIT_K;
    let params: PastaParams = pasta_params_new(k);

    let account = Scalar::from(3u64);
    let domain_salt = Scalar::from(17u64);
    let nullifier_seed = Scalar::from(25u64);

    let root_hash = empty_roster_root_hash();
    let circuit = KaigiRosterJoinCircuit::new(
        account,
        domain_salt,
        nullifier_seed,
        roster_root_limbs(&root_hash),
    );
    let commitment = compute_commitment(account, domain_salt);
    let nullifier = compute_nullifier(account, nullifier_seed);

    let vk_h2 = keygen_vk(&params, &circuit).expect("vk");
    let pk = keygen_pk(&params, vk_h2.clone(), &circuit).expect("pk");

    let mut inst_cols = vec![vec![commitment], vec![nullifier]];
    for limb in roster_root_limbs(&root_hash) {
        inst_cols.push(vec![limb]);
    }
    let inst_refs: Vec<&[Scalar]> = inst_cols.iter().map(Vec::as_slice).collect();
    let proof_instances = vec![inst_refs.as_slice()];

    let mut transcript = Blake2bWrite::<_, _, Challenge255<_>>::init(vec![]);
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
        &proof_instances,
        OsRng,
        &mut transcript,
    )
    .expect("proof created");
    let proof_bytes = transcript.finalize();

    let mut vk_env = zk1::wrap_start();
    zk1::wrap_append_ipa_k(&mut vk_env, k);
    zk1::wrap_append_circuit_id(&mut vk_env, "halo2/pasta/ipa/kaigi-roster-v1");
    zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);

    let mut prf_env = zk1::wrap_start();
    zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
    zk1::wrap_append_instances_pasta_fp_cols(&inst_refs, &mut prf_env);

    let vk_box = VerifyingKeyBox::new(KAIGI_ROSTER_BACKEND.into(), vk_env);
    let envelope = iroha_data_model::zk::OpenVerifyEnvelope {
        backend: iroha_data_model::zk::BackendTag::Halo2IpaPasta,
        circuit_id: "halo2/pasta/ipa/kaigi-roster-v1".into(),
        vk_hash: super::hash_vk(&vk_box),
        public_inputs: KAIGI_ROSTER_PUBLIC_INPUTS_SCHEMA_V1.to_vec(),
        proof_bytes: prf_env,
        aux: Vec::new(),
    };
    let prf_box = ProofBox::new(
        KAIGI_ROSTER_BACKEND.into(),
        norito::encode_canonical(&envelope).expect("encode Kaigi roster OpenVerifyEnvelope"),
    );
    assert!(
        super::verify_backend(KAIGI_ROSTER_BACKEND, &prf_box, Some(&vk_box)),
        "exact Kaigi roster registry label should reach the roster verifier"
    );
}

#[cfg(all(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[test]
fn kaigi_usage_backend_accepts_valid_proof() {
    use halo2_proofs::{
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        plonk::{keygen_pk, keygen_vk},
        transcript::{Blake2bWrite, Challenge255},
    };
    use kaigi_zk::{
        KAIGI_USAGE_BACKEND, KAIGI_USAGE_CIRCUIT_K, KAIGI_USAGE_PUBLIC_INPUTS_SCHEMA_V1,
        KaigiUsageCommitmentCircuit, compute_usage_commitment,
    };
    use rand_core_06::OsRng;

    let params: PastaParams = pasta_params_new(KAIGI_USAGE_CIRCUIT_K);
    let duration = Scalar::from(1_200u64);
    let billed = Scalar::from(345u64);
    let segment = Scalar::from(2u64);

    let circuit = KaigiUsageCommitmentCircuit::new(duration, billed, segment);
    let vk_h2 = keygen_vk(&params, &circuit).expect("vk");
    let pk = keygen_pk(&params, vk_h2.clone(), &circuit).expect("pk");

    let commitment = compute_usage_commitment(duration, billed, segment);
    let inst_cols = vec![vec![commitment]];
    let inst_refs: Vec<&[Scalar]> = inst_cols.iter().map(Vec::as_slice).collect();
    let proof_instances = vec![inst_refs.as_slice()];

    let mut transcript = Blake2bWrite::<_, _, Challenge255<_>>::init(vec![]);
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
        &proof_instances,
        OsRng,
        &mut transcript,
    )
    .expect("proof created");
    let proof_bytes = transcript.finalize();

    let mut vk_env = zk1::wrap_start();
    zk1::wrap_append_ipa_k(&mut vk_env, KAIGI_USAGE_CIRCUIT_K);
    zk1::wrap_append_circuit_id(&mut vk_env, "halo2/pasta/ipa/kaigi-usage-v1");
    zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);

    let mut prf_env = zk1::wrap_start();
    zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
    zk1::wrap_append_instances_pasta_fp_cols(&inst_refs, &mut prf_env);

    let vk_box = VerifyingKeyBox::new(KAIGI_USAGE_BACKEND.into(), vk_env);
    let envelope = iroha_data_model::zk::OpenVerifyEnvelope {
        backend: iroha_data_model::zk::BackendTag::Halo2IpaPasta,
        circuit_id: "halo2/pasta/ipa/kaigi-usage-v1".into(),
        vk_hash: super::hash_vk(&vk_box),
        public_inputs: KAIGI_USAGE_PUBLIC_INPUTS_SCHEMA_V1.to_vec(),
        proof_bytes: prf_env,
        aux: Vec::new(),
    };
    let prf_box = ProofBox::new(
        KAIGI_USAGE_BACKEND.into(),
        norito::encode_canonical(&envelope).expect("encode Kaigi usage OpenVerifyEnvelope"),
    );
    assert!(
        super::verify_backend(KAIGI_USAGE_BACKEND, &prf_box, Some(&vk_box)),
        "exact Kaigi usage registry label should reach the usage verifier"
    );
}

#[test]
fn proof_hash_stable() {
    let p1 = ProofBox::new("halo2/pasta".into(), vec![1, 2, 3, 4]);
    let p2 = ProofBox::new("halo2/pasta".into(), vec![1, 2, 3, 4]);
    assert_eq!(hash_proof(&p1), hash_proof(&p2));
}

#[test]
fn proof_and_vk_hash_domains_are_distinct() {
    let proof = ProofBox::new("halo2/pasta".into(), vec![1, 2, 3, 4]);
    let vk = VerifyingKeyBox::new("halo2/pasta".into(), vec![1, 2, 3, 4]);
    assert_ne!(hash_proof(&proof), hash_vk(&vk));
}

#[test]
fn proof_hash_length_prefixes_backend_and_payload() {
    let p1 = ProofBox::new("ab".into(), b"cdef".to_vec());
    let p2 = ProofBox::new("abc".into(), b"def".to_vec());
    assert_ne!(hash_proof(&p1), hash_proof(&p2));
}

#[test]
fn dedup_works() {
    let mut d = DedupCache::new();
    let p1 = ProofBox::new("halo2/pasta".into(), vec![9, 9, 9]);
    let p2 = ProofBox::new("halo2/pasta".into(), vec![9, 9, 9]);
    let p3 = ProofBox::new("groth16/bls12_381".into(), vec![9, 9, 9]);
    assert!(d.check_and_insert(&p1));
    assert!(!d.check_and_insert(&p2), "duplicate should be rejected");
    assert!(d.check_and_insert(&p3), "different backend is distinct");
}

#[test]
fn hash_vk_stable() {
    let v1 = VerifyingKeyBox::new("halo2/pasta".into(), vec![5, 5]);
    let v2 = VerifyingKeyBox::new("halo2/pasta".into(), vec![5, 5]);
    assert_eq!(hash_vk(&v1), hash_vk(&v2));
}

#[test]
fn preverify_basic() {
    let vk_commitment = [1u8; 32];
    let envelope = iroha_data_model::zk::OpenVerifyEnvelope {
        backend: iroha_data_model::zk::BackendTag::Halo2IpaPasta,
        circuit_id: IVM_EXECUTION_V1_CIRCUIT_ID.to_owned(),
        vk_hash: vk_commitment,
        public_inputs: IVM_EXECUTION_PUBLIC_INPUTS_SCHEMA_V1.to_vec(),
        proof_bytes: vec![2],
        aux: Vec::new(),
    };
    let p = ProofBox::new(
        ZK_BACKEND_HALO2_IPA.into(),
        norito::to_bytes(&envelope).expect("encode OpenVerifyEnvelope"),
    );
    let mut missing_expected = DedupCache::new();
    assert_eq!(
        preverify_with_budget(
            &p,
            None,
            &mut missing_expected,
            0,
            Some(vk_commitment),
            None,
            true,
        ),
        PreverifyResult::VerifyingKeyMissing
    );
    let mut zero_commitment = DedupCache::new();
    assert_eq!(
        preverify_with_budget(
            &p,
            None,
            &mut zero_commitment,
            0,
            None,
            Some([0u8; 32]),
            true,
        ),
        PreverifyResult::VerifyingKeyMismatch
    );
    assert_eq!(
        preverify_with_budget(
            &p,
            None,
            &mut zero_commitment,
            0,
            Some([0u8; 32]),
            Some(vk_commitment),
            true,
        ),
        PreverifyResult::VerifyingKeyMismatch
    );
    let mut d = DedupCache::new();
    assert_eq!(
        preverify_with_budget(&p, None, &mut d, 0, None, Some(vk_commitment), true,),
        PreverifyResult::Accepted
    );
    assert_eq!(
        preverify_with_budget(
            &p,
            None,
            &mut d,
            0,
            Some(vk_commitment),
            Some(vk_commitment),
            true,
        ),
        PreverifyResult::Duplicate
    );
    let mut wrong = vk_commitment;
    wrong[0] ^= 0x80;
    assert_eq!(
        preverify_with_budget(&p, None, &mut d, 0, Some(wrong), Some(vk_commitment), true),
        PreverifyResult::VerifyingKeyMismatch
    );
}

#[cfg(feature = "zk-halo2")]
#[test]
fn halo2_gate_requires_vk_and_valid_encoding() {
    let backend = "halo2/pasta/tiny-add";
    let proof = ProofBox::new(backend.into(), vec![1, 2, 3]);
    // Missing VK → reject
    assert!(!super::verify_halo2(backend, &proof, None));
    // Wrong backend in VK → reject
    let vk_wrong = VerifyingKeyBox::new("halo2/bls12_381".into(), vec![9]);
    assert!(!super::verify_halo2(backend, &proof, Some(&vk_wrong)));
    // Bad encoding → reject
    let vk_bad = VerifyingKeyBox::new(backend.into(), b"BAD!".to_vec());
    assert!(!super::verify_halo2(backend, &proof, Some(&vk_bad)));
}

#[cfg(feature = "zk-halo2")]
#[test]
fn halo2_end_to_end_proof_verification() {
    use halo2_proofs::{
        halo2curves::pasta::EqAffine as Curve,
        plonk::{VerifyingKey, keygen_pk, keygen_vk},
        transcript::{Blake2bWrite, Challenge255},
    };
    use rand_core_06::OsRng;

    // A tiny circuit with no public inputs: enforce 2 + 2 = 4
    // Setup params and keys
    let k = 5u32; // small-ish circuit size
    let params: PastaParams = pasta_params_new(k);
    let vk_h2: VerifyingKey<Curve> = keygen_vk(&params, &pasta_tiny::Add::default()).expect("vk");
    let pk = keygen_pk(&params, vk_h2.clone(), &pasta_tiny::Add::default()).expect("pk");

    // Create proof (no public inputs)
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
        &[pasta_tiny::Add::default()],
        &[&[][..]],
        OsRng,
        &mut transcript,
    )
    .expect("proof created");
    let proof_bytes = transcript.finalize();

    // Serialize VK/proof in a ZK1 envelope (IPAK + H2VK/H2PF)
    let mut vk_container = zk1::wrap_start();
    zk1::wrap_append_ipa_k(&mut vk_container, k);
    zk1::wrap_append_vk_pasta(&mut vk_container, &vk_h2);

    let mut proof_container = zk1::wrap_start();
    zk1::wrap_append_proof(&mut proof_container, &proof_bytes);

    // Wrap into data model boxes
    let backend = "halo2/pasta/tiny-add";
    let vk_box = VerifyingKeyBox::new(backend.into(), vk_container);
    let prf_box = ProofBox::new(backend.into(), proof_container);

    assert!(super::verify_halo2(backend, &prf_box, Some(&vk_box)));
}

#[cfg(feature = "zk-halo2")]
#[test]
fn halo2_verify_with_instance_add_kzg() {
    use halo2_proofs::{
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        plonk::{VerifyingKey, keygen_pk, keygen_vk},
        transcript::{Blake2bWrite, Challenge255},
    };
    use rand_core_06::OsRng;

    // Params and keys
    let k = 5u32;
    let params: PastaParams = pasta_params_new(k);
    let vk_h2: VerifyingKey<Curve> =
        keygen_vk(&params, &pasta_tiny::AddPublic::default()).expect("vk");
    let pk = keygen_pk(&params, vk_h2.clone(), &pasta_tiny::AddPublic::default()).expect("pk");

    // Instances: one column, one row (public value 4)
    let inst_col = vec![Scalar::from(4u64)];
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
        &[pasta_tiny::AddPublic::default()],
        &inst_proofs,
        OsRng,
        &mut transcript,
    )
    .expect("proof created");
    let proof_bytes = transcript.finalize();

    // Build VK container (ZK1)
    let mut vk_container = zk1::wrap_start();
    zk1::wrap_append_ipa_k(&mut vk_container, k);
    zk1::wrap_append_vk_pasta(&mut vk_container, &vk_h2);

    // Build proof container + INST TLV (ZK1)
    let mut proof_container = zk1::wrap_start();
    zk1::wrap_append_proof(&mut proof_container, &proof_bytes);
    zk1::wrap_append_instances_pasta_fp(inst_col.as_slice(), &mut proof_container);

    // Verify via backend dispatch
    let backend = "halo2/pasta/tiny-add-public";
    let vk_box = VerifyingKeyBox::new(backend.into(), vk_container);
    let prf_box = ProofBox::new(backend.into(), proof_container);
    assert!(super::verify_halo2(backend, &prf_box, Some(&vk_box)));
}

#[cfg(feature = "zk-halo2")]
#[test]
fn halo2_verify_add_2rows_kzg() {
    use halo2_proofs::{
        halo2curves::pasta::EqAffine as Curve,
        plonk::{VerifyingKey, keygen_pk, keygen_vk},
        transcript::{Blake2bWrite, Challenge255},
    };
    use rand_core_06::OsRng;

    let k = 6u32; // two-row small circuit
    let params: PastaParams = pasta_params_new(k);
    let vk_h2: VerifyingKey<Curve> =
        keygen_vk(&params, &pasta_tiny::AddTwoRows::default()).expect("vk");
    let pk = keygen_pk(&params, vk_h2.clone(), &pasta_tiny::AddTwoRows::default()).expect("pk");

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
        &[pasta_tiny::AddTwoRows::default()],
        &[&[][..]],
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

    let backend = "halo2/pasta/tiny-add-2rows";
    let vk_box = VerifyingKeyBox::new(backend.into(), vk_container);
    let prf_box = ProofBox::new(backend.into(), proof_container);
    assert!(super::verify_halo2(backend, &prf_box, Some(&vk_box)));
}

#[cfg(feature = "zk-halo2")]
#[test]
fn halo2_verify_id_public_kzg_with_and_without_inst() {
    use halo2_proofs::{
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        plonk::{VerifyingKey, keygen_pk, keygen_vk},
        transcript::{Blake2bWrite, Challenge255},
    };
    use rand_core_06::OsRng;

    let k = 5u32;
    let params: PastaParams = pasta_params_new(k);
    let vk_h2: VerifyingKey<Curve> =
        keygen_vk(&params, &pasta_tiny::IdPublic::default()).expect("vk");
    let pk = keygen_pk(&params, vk_h2.clone(), &pasta_tiny::IdPublic::default()).expect("pk");
    // Create proof with a public instance value present (7). We will
    // construct two proof containers below: one without the INST TLV
    // (must be rejected by the verifier) and one with INST (must pass).
    let inst_binding = [Scalar::from(7u64)];
    let inst_cols: Vec<&[Scalar]> = vec![&inst_binding[..]];
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
        &[pasta_tiny::IdPublic::default()],
        &inst_proofs,
        OsRng,
        &mut transcript,
    )
    .expect("proof created");
    let proof_bytes = transcript.finalize();

    let mut vk_container = zk1::wrap_start();
    zk1::wrap_append_ipa_k(&mut vk_container, k);
    zk1::wrap_append_vk_pasta(&mut vk_container, &vk_h2);

    // Backend/tag
    let backend = "halo2/pasta/tiny-id-public";

    // Case 1: Missing INST → must fail
    let mut proof_container = zk1::wrap_start();
    zk1::wrap_append_proof(&mut proof_container, &proof_bytes);
    let vk_box = VerifyingKeyBox::new(backend.into(), vk_container.clone());
    let prf_box = ProofBox::new(backend.into(), proof_container);
    assert!(!super::verify_halo2(backend, &prf_box, Some(&vk_box)));

    // Case 2: With INST → should succeed
    let inst_val = Scalar::from(7u64);
    let mut proof_container2 = zk1::wrap_start();
    zk1::wrap_append_proof(&mut proof_container2, &proof_bytes);
    zk1::wrap_append_instances_pasta_fp(&[inst_val], &mut proof_container2);
    let vk_box2 = VerifyingKeyBox::new(backend.into(), vk_container);
    let prf_box2 = ProofBox::new(backend.into(), proof_container2);
    assert!(super::verify_halo2(backend, &prf_box2, Some(&vk_box2)));
}

#[cfg(all(feature = "zk-halo2-ipa", feature = "zk-halo2",))]
#[test]
fn halo2_verify_ipa_acceptance_variants() {
    use halo2_proofs::{
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        plonk::{VerifyingKey, keygen_pk, keygen_vk},
        transcript::{Blake2bWrite, Challenge255},
    };
    use rand_core_06::OsRng;

    // pasta_tiny::Add add (no INST)
    // IdPublic (needs INST to truly verify; IPA accepts if well formed)
    let k = 5u32;
    let params: PastaParams = pasta_params_new(k);

    // pasta_tiny::Add
    let vk_add: VerifyingKey<Curve> = keygen_vk(&params, &pasta_tiny::Add::default()).expect("vk");
    let pk_add = keygen_pk(&params, vk_add.clone(), &pasta_tiny::Add::default()).expect("pk");
    let mut t_add = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
    halo2_proofs::plonk::create_proof::<
        IPACommitmentScheme<Curve>,
        ProverIPA<'_, Curve>,
        Challenge255<Curve>,
        _,
        _,
        _,
    >(
        &params,
        &pk_add,
        &[pasta_tiny::Add::default()],
        &[&[][..]],
        OsRng,
        &mut t_add,
    )
    .expect("proof add");
    let p_add = t_add.finalize();

    // pasta_tiny::IdPublic (+INST)
    let vk_id: VerifyingKey<Curve> =
        keygen_vk(&params, &pasta_tiny::IdPublic::default()).expect("vk");
    let pk_id = keygen_pk(&params, vk_id.clone(), &pasta_tiny::IdPublic::default()).expect("pk");
    let mut t_id = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
    halo2_proofs::plonk::create_proof::<
        IPACommitmentScheme<Curve>,
        ProverIPA<'_, Curve>,
        Challenge255<Curve>,
        _,
        _,
        _,
    >(
        &params,
        &pk_id,
        &[pasta_tiny::IdPublic::default()],
        &[&[&[Scalar::from(7u64)][..]][..]],
        OsRng,
        &mut t_id,
    )
    .expect("proof id");
    let p_id = t_id.finalize();

    // ZK1 envelopes
    let mut vk_add_env = zk1::wrap_start();
    zk1::wrap_append_ipa_k(&mut vk_add_env, k);
    zk1::wrap_append_vk_pasta(&mut vk_add_env, &vk_add);
    let mut vk_id_env = zk1::wrap_start();
    zk1::wrap_append_ipa_k(&mut vk_id_env, k);
    zk1::wrap_append_vk_pasta(&mut vk_id_env, &vk_id);

    let mut pr_add_env = zk1::wrap_start();
    zk1::wrap_append_proof(&mut pr_add_env, &p_add);

    let mut pr_id_env = zk1::wrap_start();
    zk1::wrap_append_proof(&mut pr_id_env, &p_id);
    zk1::wrap_append_instances_pasta_fp(&[Scalar::from(7u64)], &mut pr_id_env);

    let b_add = "halo2/pasta/ipa/tiny-add";
    let b_id = "halo2/pasta/ipa/tiny-id-public";
    let vk_add_box = VerifyingKeyBox::new(b_add.into(), vk_add_env);
    let vk_id_box = VerifyingKeyBox::new(b_id.into(), vk_id_env);
    let pr_add_box = ProofBox::new(b_add.into(), pr_add_env);
    let pr_id_box = ProofBox::new(b_id.into(), pr_id_env);

    assert!(super::verify_halo2_ipa(
        b_add,
        &pr_add_box,
        Some(&vk_add_box)
    ));
    assert!(super::verify_halo2_ipa(b_id, &pr_id_box, Some(&vk_id_box)));
}

#[cfg(all(feature = "zk-halo2-ipa", feature = "zk-halo2",))]
#[test]
fn halo2_verify_add_2rows_ipa() {
    use halo2_proofs::{
        halo2curves::pasta::EqAffine as Curve,
        plonk::{VerifyingKey, keygen_pk, keygen_vk},
        transcript::{Blake2bWrite, Challenge255},
    };
    use rand_core_06::OsRng;

    let k = 6u32;
    let params: PastaParams = pasta_params_new(k);
    let vk_h2: VerifyingKey<Curve> =
        keygen_vk(&params, &pasta_tiny::AddTwoRows::default()).expect("vk");
    let pk = keygen_pk(&params, vk_h2.clone(), &pasta_tiny::AddTwoRows::default()).expect("pk");

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
        &[pasta_tiny::AddTwoRows::default()],
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

    let backend = "halo2/pasta/ipa/tiny-add-2rows";
    let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
    let prf_box = ProofBox::new(backend.into(), proof_env);
    assert!(super::verify_halo2_ipa(backend, &prf_box, Some(&vk_box)));
}

#[cfg(all(feature = "zk-halo2-ipa", feature = "zk-halo2",))]
#[test]
fn halo2_verify_add3_ipa() {
    use halo2_proofs::{
        halo2curves::pasta::EqAffine as Curve,
        plonk::{VerifyingKey, keygen_pk, keygen_vk},
        transcript::{Blake2bWrite, Challenge255},
    };
    use rand_core_06::OsRng;

    let k = 6u32;
    let params: PastaParams = pasta_params_new(k);
    let vk_h2: VerifyingKey<Curve> =
        keygen_vk(&params, &pasta_tiny::AddThree::default()).expect("vk");
    let pk = keygen_pk(&params, vk_h2.clone(), &pasta_tiny::AddThree::default()).expect("pk");

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
        &[pasta_tiny::AddThree::default()],
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

    let backend = "halo2/pasta/ipa/tiny-add3";
    let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
    let prf_box = ProofBox::new(backend.into(), proof_env);
    assert!(super::verify_halo2_ipa(backend, &prf_box, Some(&vk_box)));
}
