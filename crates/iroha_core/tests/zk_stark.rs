#![doc = "End-to-end test for the native STARK (FRI single-fold) verifier."]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "zk-tests")]
//! End-to-end test for the native STARK (FRI single-fold) verifier.

#![cfg(feature = "zk-stark")]

use expect_test::expect;
use iroha_core::{
    zk::verify_backend,
    zk_stark::{
        FoldDecommitV1, MerklePath, STARK_HASH_SHA256_V1, StarkCommitmentsV1,
        StarkCompositionTermV1, StarkCompositionValueV1, StarkFriParamsV1, StarkProofV1,
        StarkVerifierLimits, StarkVerifyEnvelopeV1, verify_stark_fri_envelope,
        verify_stark_fri_envelope_with_limits,
    },
};
use sha2::{Digest, Sha256};

const MOD_P: u128 = (1u128 << 64) - (1u128 << 32) + 1;

fn field_add(a: u64, b: u64) -> u64 {
    let sum = (a as u128) + (b as u128);
    (sum % MOD_P) as u64
}

fn field_mul(a: u64, b: u64) -> u64 {
    let prod = (a as u128) * (b as u128);
    (prod % MOD_P) as u64
}

fn leaf_hash_u64(v: u64) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"LEAF");
    h.update(&v.to_le_bytes());
    h.finalize().into()
}

fn node_hash(l: &[u8; 32], r: &[u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(l);
    h.update(r);
    h.finalize().into()
}

fn merkle_root_from_leaves(mut leaves: Vec<[u8; 32]>) -> ([u8; 32], Vec<Vec<[u8; 32]>>) {
    // Build full binary tree and return root and per-level nodes
    let mut levels = Vec::new();
    levels.push(leaves.clone());
    while leaves.len() > 1 {
        let mut next = Vec::with_capacity(leaves.len() / 2);
        for i in (0..leaves.len()).step_by(2) {
            next.push(node_hash(&leaves[i], &leaves[i + 1]));
        }
        levels.push(next.clone());
        leaves = next;
    }
    (leaves[0], levels)
}

fn path_for(index: usize, levels: &[Vec<[u8; 32]>]) -> MerklePath {
    let mut dirs = Vec::new();
    let mut siblings = Vec::new();
    let mut idx = index;
    // From leaf level (0) up to the level before the root
    for lvl in 0..levels.len() - 1 {
        let nodes = &levels[lvl];
        let bit = (idx & 1) as u8; // 0 => current hash is left, 1 => right
        if lvl % 8 == 0 {
            dirs.push(0);
        }
        let last = dirs.len() - 1;
        dirs[last] |= bit << (lvl % 8);
        siblings.push(nodes[idx ^ 1]);
        idx >>= 1;
    }
    MerklePath { dirs, siblings }
}

fn derive_query_index_for_test(
    label: &str,
    params: &StarkFriParamsV1,
    roots: &[[u8; 32]],
    query_idx: usize,
) -> usize {
    let mut h = Sha256::new();
    h.update(b"STARK:query-index");
    h.update(label.as_bytes());
    h.update(&params.version.to_le_bytes());
    h.update(&[
        params.n_log2,
        params.blowup_log2,
        params.fold_arity,
        params.merkle_arity,
        params.hash_fn,
    ]);
    h.update(&params.queries.to_le_bytes());
    h.update(&(params.domain_tag.len() as u32).to_le_bytes());
    h.update(params.domain_tag.as_bytes());
    h.update(&(query_idx as u64).to_le_bytes());
    for root in roots {
        h.update(root);
    }
    let out = h.finalize();
    let mut w = [0u8; 8];
    w.copy_from_slice(&out[..8]);
    let domain = 1usize << params.n_log2;
    (u64::from_le_bytes(w) % (domain as u64)) as usize
}

fn challenge_u64(label: &str, bytes: &[u8]) -> u64 {
    let mut h = Sha256::new();
    h.update(label.as_bytes());
    h.update(&[0u8]);
    h.update(bytes);
    let out = h.finalize();
    let mut w = [0u8; 8];
    w.copy_from_slice(&out[..8]);
    let v = u64::from_le_bytes(w);
    (v as u128 % MOD_P) as u64
}

fn stark_open_verify_domain_tag_v1(
    backend: &str,
    circuit_id: &str,
    vk_hash: [u8; 32],
    env_public_inputs: &[u8],
    public_inputs: &[Vec<[u8; 32]>],
) -> String {
    let mut preimage = Vec::new();
    preimage.extend_from_slice(b"iroha:zk:stark-fri-open-proof:v1");
    preimage.extend_from_slice(&(backend.len() as u64).to_le_bytes());
    preimage.extend_from_slice(backend.as_bytes());
    preimage.extend_from_slice(&(circuit_id.len() as u64).to_le_bytes());
    preimage.extend_from_slice(circuit_id.as_bytes());
    preimage.extend_from_slice(&vk_hash);
    preimage.extend_from_slice(&(env_public_inputs.len() as u64).to_le_bytes());
    preimage.extend_from_slice(env_public_inputs);
    preimage.extend_from_slice(&(public_inputs.len() as u64).to_le_bytes());
    for column in public_inputs {
        preimage.extend_from_slice(&(column.len() as u64).to_le_bytes());
        for value in column {
            preimage.extend_from_slice(value);
        }
    }
    let digest = Sha256::digest(&preimage);
    hex::encode(digest)
}

#[allow(clippy::too_many_lines)]
fn build_sample_envelope_with_domain_tag(domain_tag: String) -> StarkVerifyEnvelopeV1 {
    // Domain size 8, degree-1 poly f(x) = 3x+5 over u64 (no modular wrap for small x)
    let n_log2 = 3u8;
    let n = 1usize << n_log2;
    let evals: Vec<u64> = (0..n)
        .map(|x| field_add(field_mul(3, x as u64), 5))
        .collect();
    let leaves0: Vec<[u8; 32]> = evals.iter().map(|&v| leaf_hash_u64(v)).collect();
    let (root0, levels0) = merkle_root_from_leaves(leaves0.clone());

    let params = StarkFriParamsV1 {
        version: 1,
        n_log2,
        blowup_log2: 3,
        fold_arity: 2,
        queries: 1,
        merkle_arity: 2,
        hash_fn: STARK_HASH_SHA256_V1,
        domain_tag,
    };

    let build_transcript = |root: &[u8; 32]| {
        let mut tb = Vec::new();
        tb.extend_from_slice(b"TEST-STARK");
        tb.extend_from_slice(&params.version.to_le_bytes());
        tb.extend_from_slice(&[
            params.n_log2,
            params.blowup_log2,
            params.fold_arity,
            params.merkle_arity,
            params.hash_fn,
        ]);
        tb.extend_from_slice(&params.queries.to_le_bytes());
        tb.extend_from_slice(&(params.domain_tag.len() as u32).to_le_bytes());
        tb.extend_from_slice(params.domain_tag.as_bytes());
        tb.extend_from_slice(root);
        tb
    };

    // Transcript-derived r (mirror the verifier logic)
    let r0 = challenge_u64("stark:fri:r:k", &build_transcript(&root0));

    // Layer 1 with r0
    let layer1: Vec<u64> = (0..n / 2)
        .map(|j| field_add(evals[2 * j], field_mul(r0, evals[2 * j + 1])))
        .collect();
    let leaves1: Vec<[u8; 32]> = layer1.iter().map(|&v| leaf_hash_u64(v)).collect();
    let (root1, levels1) = merkle_root_from_leaves(leaves1.clone());

    // Derive r1 from label+params+root1
    let r1 = challenge_u64("stark:fri:r:k", &build_transcript(&root1));

    // Derive r2 from label+params+root2 (will be used for next fold)
    // Layer 2 with r1
    let layer2: Vec<u64> = (0..n / 4)
        .map(|j| field_add(layer1[2 * j], field_mul(r1, layer1[2 * j + 1])))
        .collect();
    let leaves2: Vec<[u8; 32]> = layer2.iter().map(|&v| leaf_hash_u64(v)).collect();
    let (root2, levels2) = merkle_root_from_leaves(leaves2.clone());

    let r2 = challenge_u64("stark:fri:r:k", &build_transcript(&root2));

    // Layer 3 with r2 (final layer size = 1); only j=0 valid
    let layer3: Vec<u64> = (0..n / 8)
        .map(|j| field_add(layer2[2 * j], field_mul(r2, layer2[2 * j + 1])))
        .collect();
    let leaves3: Vec<[u8; 32]> = layer3.iter().map(|&v| leaf_hash_u64(v)).collect();
    let (root3, levels3) = merkle_root_from_leaves(leaves3.clone());

    // Prepare a single query chain (j0 = 0) covering three folds (layers 0->1->2->3)
    let commitments_roots = vec![root0, root1, root2, root3];
    let base_index = derive_query_index_for_test("TEST-STARK", &params, &commitments_roots, 0);
    let mut idx_layer = base_index;
    let mut chain = Vec::new();
    let layer_values: [&[u64]; 4] = [&evals, &layer1, &layer2, &layer3];
    let level_refs: [&[Vec<[u8; 32]>]; 4] = [&levels0, &levels1, &levels2, &levels3];
    let mut domain = n;
    let fold = params.fold_arity as usize;
    for k in 0..commitments_roots.len() - 1 {
        assert!(domain >= fold, "domain must have pairs at layer {k}");
        let j = idx_layer / fold;
        let y0 = layer_values[k][2 * j];
        let y1 = layer_values[k][2 * j + 1];
        let z = layer_values[k + 1][j];
        let path_y0 = path_for(2 * j, level_refs[k]);
        let path_y1 = path_for(2 * j + 1, level_refs[k]);
        let path_z = path_for(j, level_refs[k + 1]);
        chain.push(FoldDecommitV1 {
            j: j as u32,
            y0,
            y1,
            path_y0,
            path_y1,
            z,
            path_z,
        });
        idx_layer = j;
        domain /= fold;
    }
    let queries: Vec<Vec<FoldDecommitV1>> = vec![chain];

    // Richer composition: comp_value = c + a0 * z_final + sum coeff_i * aux_i
    let comp_constant = 7u64;
    let comp_z_coeff = 2u64;
    let aux_wire0 = layer2[0];
    let aux_wire1 = layer2[1];
    let comp_aux_terms = vec![
        StarkCompositionTermV1 {
            wire_index: 0,
            value: aux_wire0,
            coeff: 3,
        },
        StarkCompositionTermV1 {
            wire_index: 1,
            value: aux_wire1,
            coeff: 5,
        },
    ];
    let comp_leaf = field_add(
        field_add(
            field_add(comp_constant, field_mul(comp_z_coeff, layer3[0])),
            field_mul(3, aux_wire0),
        ),
        field_mul(5, aux_wire1),
    );
    let expected_comp = field_add(
        field_add(
            field_add(comp_constant, field_mul(comp_z_coeff, layer3[0])),
            field_mul(comp_aux_terms[0].coeff, comp_aux_terms[0].value),
        ),
        field_mul(comp_aux_terms[1].coeff, comp_aux_terms[1].value),
    );
    assert_eq!(comp_leaf, expected_comp, "composition leaf mismatch");
    let comp_leaves = vec![leaf_hash_u64(comp_leaf)];
    let (comp_root, comp_levels) = merkle_root_from_leaves(comp_leaves);
    let comp_values = Some(vec![StarkCompositionValueV1 {
        leaf: comp_leaf,
        constant: comp_constant,
        z_coeff: comp_z_coeff,
        aux_terms: comp_aux_terms,
        path: path_for(0, &comp_levels),
    }]);

    StarkVerifyEnvelopeV1 {
        params,
        proof: StarkProofV1 {
            version: 1,
            commits: StarkCommitmentsV1 {
                version: 1,
                roots: commitments_roots,
                comp_root: Some(comp_root),
            },
            queries,
            comp_values,
        },
        transcript_label: "TEST-STARK".to_string(),
    }
}

fn build_sample_envelope() -> StarkVerifyEnvelopeV1 {
    build_sample_envelope_with_domain_tag("fastpq:v1:fri".to_string())
}

#[test]
fn stark_single_fold_roundtrip_ok_and_fail() {
    let env = build_sample_envelope();

    let bytes = norito::to_bytes(&env).expect("encode");
    let native_ok = iroha_core::zk_stark::verify_stark_fri_envelope(&bytes);
    assert!(native_ok, "native STARK verifier rejected sample envelope");

    // Tamper auxiliary term and expect rejection
    let mut env_bad_comp = env.clone();
    if let Some(ref mut entries) = env_bad_comp.proof.comp_values {
        entries[0].aux_terms[0].coeff = entries[0].aux_terms[0].coeff.wrapping_add(1);
    }
    let bytes_bad_comp = norito::to_bytes(&env_bad_comp).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes_bad_comp),
        "tampered composition term should fail"
    );

    // Tamper with the derived index and expect rejection
    let mut env_bad_index = env.clone();
    env_bad_index.proof.queries[0][0].j = env_bad_index.proof.queries[0][0].j.wrapping_add(1);
    let bytes_bad_index = norito::to_bytes(&env_bad_index).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes_bad_index),
        "tampered query index should fail"
    );

    // Corrupt one z1 value and expect failure
    let mut env_bad = env.clone();
    env_bad.proof.queries[0][1].z = env_bad.proof.queries[0][1].z.wrapping_add(1);
    let bytes_bad = norito::to_bytes(&env_bad).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes_bad),
        "tampered STARK proof should fail"
    );

    // Non-canonical field encoding should be rejected (value equal to modulus)
    let mut env_bad_field = env.clone();
    env_bad_field.proof.queries[0][0].y0 = 0xFFFF_FFFF_0000_0001u64;
    let bytes_bad_field = norito::to_bytes(&env_bad_field).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes_bad_field),
        "non-canonical Goldilocks encoding must fail"
    );

    // Wrong root should fail deterministically
    let mut env_bad_root = env.clone();
    env_bad_root.proof.commits.roots[0][0] ^= 0x01;
    let bytes_bad_root = norito::to_bytes(&env_bad_root).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes_bad_root),
        "tampered root must fail"
    );

    // Broken Merkle path should fail
    let mut env_bad_path = env.clone();
    env_bad_path.proof.queries[0][0].path_y0.siblings[0][0] ^= 0x02;
    let bytes_bad_path = norito::to_bytes(&env_bad_path).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes_bad_path),
        "broken Merkle path should fail"
    );

    // Round-count/roots mismatch should fail
    let mut env_bad_rounds = env.clone();
    env_bad_rounds.proof.commits.roots.pop();
    env_bad_rounds.proof.queries[0].pop();
    let bytes_bad_rounds = norito::to_bytes(&env_bad_rounds).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes_bad_rounds),
        "mismatched round count should fail validation"
    );

    // Query-count/header mismatch should fail
    let mut env_bad_query_header = env.clone();
    env_bad_query_header.params.queries = 2;
    let bytes_bad_query_header = norito::to_bytes(&env_bad_query_header).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes_bad_query_header),
        "mismatched query count in params should fail"
    );

    // Unsupported hash selector should be rejected
    let mut env_bad_hash = env.clone();
    env_bad_hash.params.hash_fn = iroha_core::zk_stark::STARK_HASH_POSEIDON2_V1;
    let bytes_bad_hash = norito::to_bytes(&env_bad_hash).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes_bad_hash),
        "unsupported hash selector must fail"
    );
}

#[test]
fn stark_rejects_mismatched_merkle_indices() {
    // Minimal two-point domain with a single fold. Use identical leaf values so the Merkle roots
    // remain valid even when the proof swaps indices; the verifier must still reject due to the
    // index mismatch itself.
    let transcript_label = "INDEX-MISMATCH";

    let a = 7u64;
    let leaves0 = vec![leaf_hash_u64(a), leaf_hash_u64(a)];
    let (root0, levels0) = merkle_root_from_leaves(leaves0);

    let params = StarkFriParamsV1 {
        version: 1,
        n_log2: 1,
        blowup_log2: 1,
        fold_arity: 2,
        queries: 1,
        merkle_arity: 2,
        hash_fn: STARK_HASH_SHA256_V1,
        domain_tag: "fastpq:v1:fri".to_string(),
    };

    let build_transcript = |root: &[u8; 32]| {
        let mut tb = Vec::new();
        tb.extend_from_slice(transcript_label.as_bytes());
        tb.extend_from_slice(&params.version.to_le_bytes());
        tb.extend_from_slice(&[
            params.n_log2,
            params.blowup_log2,
            params.fold_arity,
            params.merkle_arity,
            params.hash_fn,
        ]);
        tb.extend_from_slice(&params.queries.to_le_bytes());
        tb.extend_from_slice(&(params.domain_tag.len() as u32).to_le_bytes());
        tb.extend_from_slice(params.domain_tag.as_bytes());
        tb.extend_from_slice(root);
        tb
    };

    let r0 = challenge_u64("stark:fri:r:k", &build_transcript(&root0));
    let z = field_add(a, field_mul(r0, a));

    let leaves1 = vec![leaf_hash_u64(z)];
    let (root1, levels1) = merkle_root_from_leaves(leaves1);

    // Intentionally swap the indices for y0 and y1; without index binding this could pass.
    let chain = vec![FoldDecommitV1 {
        j: 0,
        y0: a,
        y1: a,
        path_y0: path_for(1, &levels0),
        path_y1: path_for(0, &levels0),
        z,
        path_z: path_for(0, &levels1),
    }];

    let env = StarkVerifyEnvelopeV1 {
        params,
        proof: StarkProofV1 {
            version: 1,
            commits: StarkCommitmentsV1 {
                version: 1,
                roots: vec![root0, root1],
                comp_root: None,
            },
            queries: vec![chain],
            comp_values: None,
        },
        transcript_label: transcript_label.to_string(),
    };
    let bytes = norito::to_bytes(&env).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes),
        "index-mismatched Merkle openings must be rejected"
    );
}

#[test]
fn stark_open_verify_envelope_binds_domain_tag_to_metadata() {
    use iroha_data_model::{
        proof::{ProofBox, VerifyingKeyBox},
        zk::{BackendTag, OpenVerifyEnvelope, StarkFriOpenProofV1},
    };

    let backend = "stark/fri-v1/sha256-goldilocks-v1";
    let circuit_id = "ivm-execution-v1";

    let vk_box = VerifyingKeyBox::new(backend.into(), vec![1, 2, 3, 4, 5, 6]);
    let vk_hash = iroha_core::zk::hash_vk(&vk_box);

    // Two columns, one row each (matches the instance-column shape used by other backends).
    let public_inputs = vec![vec![[0xAA; 32]], vec![[0xBB; 32]]];
    let env_public_inputs = b"schema:test".to_vec();

    let domain_tag = stark_open_verify_domain_tag_v1(
        backend,
        circuit_id,
        vk_hash,
        &env_public_inputs,
        &public_inputs,
    );
    let inner = build_sample_envelope_with_domain_tag(domain_tag);
    let envelope_bytes = norito::to_bytes(&inner).expect("encode stark envelope");

    let open = StarkFriOpenProofV1 {
        version: 1,
        public_inputs: public_inputs.clone(),
        envelope_bytes,
    };
    let proof_bytes = norito::to_bytes(&open).expect("encode open proof");

    let env = OpenVerifyEnvelope {
        backend: BackendTag::Stark,
        circuit_id: circuit_id.to_string(),
        vk_hash,
        public_inputs: env_public_inputs,
        proof_bytes,
        aux: Vec::new(),
    };

    let proof = ProofBox::new(
        backend.into(),
        norito::to_bytes(&env).expect("encode OpenVerifyEnvelope"),
    );
    assert!(
        verify_backend(backend, &proof, Some(&vk_box)),
        "wrapped STARK OpenVerifyEnvelope should verify"
    );

    // Changing circuit_id without updating the inner envelope's `domain_tag` must fail.
    let mut env_bad = env;
    env_bad.circuit_id = "other-circuit".to_string();
    let proof_bad = ProofBox::new(
        backend.into(),
        norito::to_bytes(&env_bad).expect("encode tampered OpenVerifyEnvelope"),
    );
    assert!(
        !verify_backend(backend, &proof_bad, Some(&vk_box)),
        "STARK OpenVerifyEnvelope must bind circuit_id via domain_tag"
    );
}

fn hash_to_u64_limbs_le(hash: &iroha_crypto::Hash) -> [u64; 4] {
    let bytes: &[u8; 32] = hash.as_ref();
    let mut limbs = [0u64; 4];
    for (i, limb) in limbs.iter_mut().enumerate() {
        let start = i * 8;
        let end = start + 8;
        *limb = u64::from_le_bytes(bytes[start..end].try_into().expect("slice len = 8"));
    }
    limbs
}

fn limb_as_instance_bytes(limb: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[..8].copy_from_slice(&limb.to_le_bytes());
    out
}

fn expected_ivm_exec_public_inputs(
    code_hash: iroha_crypto::Hash,
    overlay_hash: iroha_crypto::Hash,
    events_commitment: iroha_crypto::Hash,
    gas_policy_commitment: iroha_crypto::Hash,
) -> Vec<[u8; 32]> {
    let code_limbs = hash_to_u64_limbs_le(&code_hash);
    let overlay_limbs = hash_to_u64_limbs_le(&overlay_hash);
    let events_limbs = hash_to_u64_limbs_le(&events_commitment);
    let gas_limbs = hash_to_u64_limbs_le(&gas_policy_commitment);
    code_limbs
        .into_iter()
        .chain(overlay_limbs)
        .chain(events_limbs)
        .chain(gas_limbs)
        .map(limb_as_instance_bytes)
        .collect()
}

#[test]
fn stark_ivm_proved_execution_admission_accepts_valid_proof() {
    use std::str::FromStr;
    use std::sync::Arc;

    use iroha_crypto::{Hash, KeyPair};
    use iroha_data_model::{
        Registrable,
        account::Account,
        confidential::ConfidentialStatus,
        domain::Domain,
        metadata::Metadata,
        name::Name,
        prelude::{AccountId, IvmBytecode, TransactionBuilder},
        proof::{
            ProofAttachment, ProofAttachmentList, ProofBox, VerifyingKeyBox, VerifyingKeyId,
            VerifyingKeyRecord,
        },
        transaction::{Executable, IvmProved},
        zk::{BackendTag, OpenVerifyEnvelope, StarkFriOpenProofV1},
    };
    use iroha_primitives::json::Json;

    let backend = "stark/fri-v1/sha256-goldilocks-v1";
    let circuit_id = "ivm-execution-v1";

    // Minimal ZK-mode IVM program: metadata + `HALT`.
    let meta = ivm::ProgramMetadata {
        max_cycles: 1,
        mode: ivm::ivm_mode::ZK,
        ..ivm::ProgramMetadata::default()
    };
    let mut program = meta.encode();
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let bytecode = IvmBytecode::from_compiled(program);

    let kp = KeyPair::random();
    let authority = AccountId::new(
        "wonderland".parse().expect("domain"),
        kp.public_key().clone(),
    );
    let domain = Domain::new("wonderland".parse().unwrap()).build(&authority);
    let account = Account::new(authority.clone()).build(&authority);

    let world = iroha_core::state::World::with([domain], [account], []);

    let vk_id = VerifyingKeyId::new(backend, "ivm_execution_stark");
    let vk_box = VerifyingKeyBox::new(backend.into(), vec![1, 2, 3, 4, 5, 6]);
    let vk_hash = iroha_core::zk::hash_vk(&vk_box);

    let mut vk_record = VerifyingKeyRecord::new(
        1,
        format!("{backend}:{circuit_id}"),
        BackendTag::Stark,
        "goldilocks",
        iroha_core::zk::ivm_execution_public_inputs_schema_hash(),
        vk_hash,
    );
    vk_record.status = ConfidentialStatus::Active;
    vk_record.gas_schedule_id = Some("sched_0".to_owned());
    vk_record.key = Some(vk_box.clone());

    {
        let mut wb = world.block();
        wb.verifying_keys_mut_for_testing()
            .insert(vk_id.clone(), vk_record.clone());
        wb.verifying_keys_by_circuit_mut_for_testing().insert(
            (vk_record.circuit_id.clone(), vk_record.version),
            vk_id.clone(),
        );
        wb.commit();
    }

    let kura = Arc::new(iroha_core::kura::Kura::blank_kura_for_testing());
    let query = iroha_core::query::store::LiveQueryStore::start_test();
    let mut state = iroha_core::state::State::new_for_testing(world, Arc::clone(&kura), query);
    state.zk.halo2.enabled = false;
    state.zk.stark.enabled = true;
    state.pipeline.ivm_proved.enabled = true;
    state.pipeline.ivm_proved.allowed_circuits = vec![vk_record.circuit_id.clone()];

    const TEST_GAS_LIMIT: u64 = 50_000_000;
    let mut metadata = Metadata::default();
    metadata.insert(
        Name::from_str("gas_limit").expect("static gas_limit key"),
        Json::new(TEST_GAS_LIMIT),
    );

    // Derive the proved payload by executing the IVM program once.
    let tx = TransactionBuilder::new(state.chain_id.clone(), authority.clone())
        .with_metadata(metadata.clone())
        .with_executable(Executable::Ivm(bytecode.clone()))
        .sign(kp.private_key());
    let proved = iroha_core::pipeline::overlay::derive_ivm_proved_payload_from_ivm_execution(
        &state.view(),
        &tx,
        &vk_record,
    )
    .expect("derive proved payload");

    // Compute the ivm-execution-v1 public inputs and package them as STARK wrapper columns.
    let mut ivm_cache = iroha_core::smartcontracts::ivm::cache::IvmCache::new();
    let summary = ivm_cache
        .summarize_program(proved.bytecode.as_ref())
        .expect("summarize IVM program");
    let overlay_hash = {
        let bytes = norito::to_bytes(&proved.overlay).expect("encode overlay");
        Hash::new(&bytes)
    };
    let inputs = expected_ivm_exec_public_inputs(
        summary.code_hash,
        overlay_hash,
        proved.events_commitment,
        proved.gas_policy_commitment,
    );
    let public_inputs = inputs.into_iter().map(|v| vec![v]).collect::<Vec<_>>();

    // Public-input schema descriptor is the same for both Halo2 and STARK wrappers.
    let env_public_inputs =
        iroha_core::zk::ivm_execution_public_inputs_schema_descriptor().to_vec();

    let domain_tag = stark_open_verify_domain_tag_v1(
        backend,
        circuit_id,
        vk_hash,
        &env_public_inputs,
        &public_inputs,
    );
    let inner = build_sample_envelope_with_domain_tag(domain_tag);
    let envelope_bytes = norito::to_bytes(&inner).expect("encode stark envelope");
    let open = StarkFriOpenProofV1 {
        version: 1,
        public_inputs: public_inputs.clone(),
        envelope_bytes,
    };
    let proof_bytes = norito::to_bytes(&open).expect("encode open proof");
    let env = OpenVerifyEnvelope {
        backend: iroha_data_model::zk::BackendTag::Stark,
        circuit_id: circuit_id.to_string(),
        vk_hash,
        public_inputs: env_public_inputs,
        proof_bytes,
        aux: Vec::new(),
    };

    let proof_box = ProofBox::new(
        backend.into(),
        norito::to_bytes(&env).expect("encode OpenVerifyEnvelope"),
    );
    let attachment = ProofAttachment::new_ref(backend.into(), proof_box, vk_id);
    let attachments = ProofAttachmentList(vec![attachment]);

    let tx_proved = TransactionBuilder::new(state.chain_id.clone(), authority)
        .with_metadata(metadata)
        .with_executable(Executable::IvmProved(IvmProved {
            bytecode: proved.bytecode.clone(),
            overlay: proved.overlay.clone(),
            events_commitment: proved.events_commitment,
            gas_policy_commitment: proved.gas_policy_commitment,
        }))
        .with_attachments(attachments)
        .sign(kp.private_key());

    let overlay_built =
        iroha_core::pipeline::overlay::build_overlay_for_transaction(&tx_proved, &state.view())
            .expect("proved execution overlay must be accepted");
    let built: Vec<_> = overlay_built.instructions().cloned().collect();
    assert_eq!(built.as_slice(), proved.overlay.as_ref());
}

#[test]
fn stark_envelope_respects_limits() {
    let env = build_sample_envelope();
    let bytes = norito::to_bytes(&env).expect("encode");
    assert!(
        verify_stark_fri_envelope(&bytes),
        "default limits should accept the sample envelope"
    );

    // Apply a stricter domain-tag limit to force rejection.
    let mut tight_limits = StarkVerifierLimits::default();
    tight_limits.max_domain_tag_len = 4;

    let mut env_bad_tag = env.clone();
    env_bad_tag.params.domain_tag = "TOO-LONG-TAG".into();
    let bytes_bad_tag = norito::to_bytes(&env_bad_tag).expect("encode");
    assert!(
        !verify_stark_fri_envelope_with_limits(&bytes_bad_tag, &tight_limits),
        "envelope with oversized domain tag must fail under stricter limits"
    );

    // Apply envelope byte budget lower than payload size to confirm size guard triggers.
    tight_limits.max_envelope_bytes = bytes.len().saturating_sub(1);
    assert!(
        !verify_stark_fri_envelope_with_limits(&bytes, &tight_limits),
        "envelope larger than allowed byte budget must fail"
    );
}

#[test]
fn stark_single_fold_envelope_golden_vector() {
    let env = build_sample_envelope();
    let bytes = norito::to_bytes(&env).expect("encode");
    let hex = hex::encode(bytes);
    println!("{hex}");
    expect!["4e5254300000a8d457cce6a10e02a8d457cce6a10e02002a1b0000000000008ed112274a40b4aa005e00000000000000020000000000000001000100000000000000030100000000000000030100000000000000020200000000000000010001000000000000000201000000000000000115000000000000000d000000000000006661737470713a76313a667269a21a00000000000002000000000000000100eb0500000000000002000000000000000100a804000000000000040000000000000020010000000000000100000000000000820100000000000000e901000000000000008a0100000000000000900100000000000000750100000000000000460100000000000000ce0100000000000000810100000000000000e201000000000000001e0100000000000000f20100000000000000330100000000000000920100000000000000f10100000000000000d20100000000000000d50100000000000000d701000000000000003901000000000000004e0100000000000000310100000000000000090100000000000000dc0100000000000000d10100000000000000110100000000000000af0100000000000000220100000000000000640100000000000000ae0100000000000000270100000000000000b20100000000000000330100000000000000fb200100000000000001000000000000008601000000000000002e0100000000000000d901000000000000001e0100000000000000e801000000000000009b0100000000000000840100000000000000550100000000000000d501000000000000004a01000000000000000601000000000000001801000000000000004201000000000000003d0100000000000000fa01000000000000008e01000000000000005d01000000000000008d01000000000000001b0100000000000000220100000000000000ae0100000000000000dd0100000000000000630100000000000000f00100000000000000840100000000000000e80100000000000000d70100000000000000d40100000000000000a60100000000000000460100000000000000fe01000000000000004a20010000000000000100000000000000f701000000000000006901000000000000009001000000000000004b0100000000000000260100000000000000a701000000000000000501000000000000009d01000000000000001b01000000000000003a01000000000000001901000000000000009201000000000000003a0100000000000000b601000000000000000c0100000000000000fc0100000000000000290100000000000000920100000000000000b60100000000000000b30100000000000000940100000000000000da01000000000000003501000000000000009c01000000000000000b01000000000000000d0100000000000000750100000000000000250100000000000000170100000000000000bc010000000000000045010000000000000048200100000000000001000000000000003c0100000000000000210100000000000000850100000000000000080100000000000000300100000000000000210100000000000000df0100000000000000f901000000000000002e01000000000000005301000000000000006f01000000000000009a0100000000000000710100000000000000ef0100000000000000500100000000000000f30100000000000000ec0100000000000000350100000000000000530100000000000000bb01000000000000006901000000000000000301000000000000003501000000000000008501000000000000003101000000000000009c01000000000000002301000000000000002301000000000000007901000000000000001f0100000000000000fa0100000000000000bd290100000000000001200100000000000001000000000000006a01000000000000002e0100000000000000020100000000000000200100000000000000b40100000000000000a401000000000000006f01000000000000000b01000000000000009301000000000000008d0100000000000000b60100000000000000e401000000000000005e0100000000000000d70100000000000000170100000000000000250100000000000000820100000000000000a10100000000000000170100000000000000560100000000000000c70100000000000000c10100000000000000210100000000000000660100000000000000c101000000000000007d0100000000000000850100000000000000d701000000000000009c01000000000000004e0100000000000000bb01000000000000001aac1300000000000001000000000000009c130000000000000300000000000000f70900000000000004000000000000000100000008000000000000000b0000000000000008000000000000000e000000000000009903000000000000090000000000000001000000000000000280030000000000000300000000000000200100000000000001000000000000009b0100000000000000150100000000000000a40100000000000000b20100000000000000cd01000000000000005401000000000000002401000000000000003f01000000000000007b01000000000000001a01000000000000008f01000000000000006501000000000000005801000000000000008f01000000000000004701000000000000009201000000000000006e0100000000000000270100000000000000a601000000000000002a0100000000000000e90100000000000000bb0100000000000000ad0100000000000000ff0100000000000000c001000000000000001101000000000000006e01000000000000006101000000000000001001000000000000000e01000000000000000f010000000000000086200100000000000001000000000000008c0100000000000000f301000000000000005201000000000000001301000000000000009301000000000000004f01000000000000001501000000000000007f0100000000000000900100000000000000580100000000000000be0100000000000000a80100000000000000cd0100000000000000800100000000000000c301000000000000008201000000000000001f0100000000000000cb0100000000000000650100000000000000cc01000000000000001101000000000000007f0100000000000000550100000000000000e00100000000000000d501000000000000004e01000000000000000b01000000000000002a0100000000000000900100000000000000b10100000000000000bf01000000000000001620010000000000000100000000000000ed01000000000000000d0100000000000000350100000000000000e201000000000000004b01000000000000005a0100000000000000c901000000000000009f0100000000000000980100000000000000430100000000000000bf0100000000000000590100000000000000b101000000000000006901000000000000003401000000000000007b0100000000000000890100000000000000be01000000000000007601000000000000000d01000000000000003d0100000000000000f80100000000000000a20100000000000000300100000000000000d601000000000000008501000000000000000c0100000000000000570100000000000000a0010000000000000024010000000000000034010000000000000049990300000000000009000000000000000100000000000000038003000000000000030000000000000020010000000000000100000000000000fb01000000000000003701000000000000003b0100000000000000e60100000000000000890100000000000000560100000000000000ce0100000000000000300100000000000000ba0100000000000000aa01000000000000007f0100000000000000730100000000000000480100000000000000030100000000000000a80100000000000000500100000000000000d80100000000000000b601000000000000009b0100000000000000da0100000000000000fe0100000000000000910100000000000000850100000000000000e501000000000000003a0100000000000000390100000000000000360100000000000000820100000000000000d90100000000000000ce01000000000000000b0100000000000000ba200100000000000001000000000000008c0100000000000000f301000000000000005201000000000000001301000000000000009301000000000000004f01000000000000001501000000000000007f0100000000000000900100000000000000580100000000000000be0100000000000000a80100000000000000cd0100000000000000800100000000000000c301000000000000008201000000000000001f0100000000000000cb0100000000000000650100000000000000cc01000000000000001101000000000000007f0100000000000000550100000000000000e00100000000000000d501000000000000004e01000000000000000b01000000000000002a0100000000000000900100000000000000b10100000000000000bf01000000000000001620010000000000000100000000000000ed01000000000000000d0100000000000000350100000000000000e201000000000000004b01000000000000005a0100000000000000c901000000000000009f0100000000000000980100000000000000430100000000000000bf0100000000000000590100000000000000b101000000000000006901000000000000003401000000000000007b0100000000000000890100000000000000be01000000000000007601000000000000000d01000000000000003d0100000000000000f80100000000000000a20100000000000000300100000000000000d601000000000000008501000000000000000c0100000000000000570100000000000000a0010000000000000024010000000000000034010000000000000049080000000000000032994bbe082aafce710200000000000009000000000000000100000000000000015802000000000000020000000000000020010000000000000100000000000000790100000000000000a901000000000000003a0100000000000000990100000000000000b70100000000000000a70100000000000000510100000000000000e00100000000000000390100000000000000f10100000000000000780100000000000000cf01000000000000000401000000000000002501000000000000004901000000000000005c0100000000000000020100000000000000fa01000000000000006a01000000000000000d0100000000000000940100000000000000d60100000000000000d10100000000000000b501000000000000002b01000000000000003f0100000000000000790100000000000000660100000000000000830100000000000000e7010000000000000022010000000000000096200100000000000001000000000000006a01000000000000000b0100000000000000040100000000000000e601000000000000002d0100000000000000120100000000000000840100000000000000450100000000000000df0100000000000000e801000000000000002501000000000000007e0100000000000000c80100000000000000d401000000000000004e0100000000000000260100000000000000540100000000000000000100000000000000a401000000000000004a0100000000000000780100000000000000c60100000000000000410100000000000000470100000000000000aa01000000000000000b0100000000000000650100000000000000c90100000000000000b301000000000000007a01000000000000007a0100000000000000ea7f060000000000000400000000000000000000000800000000000000aea006ffdfced1e3080000000000000032994bbe082aafce710200000000000009000000000000000100000000000000005802000000000000020000000000000020010000000000000100000000000000280100000000000000840100000000000000270100000000000000ae0100000000000000d401000000000000005e0100000000000000970100000000000000cc0100000000000000150100000000000000450100000000000000d401000000000000004601000000000000008a01000000000000002d0100000000000000440100000000000000ab01000000000000004301000000000000007301000000000000009301000000000000007c0100000000000000ca0100000000000000e80100000000000000d20100000000000000b601000000000000008101000000000000008501000000000000008e01000000000000005a01000000000000001501000000000000009401000000000000007701000000000000008e200100000000000001000000000000006a01000000000000000b0100000000000000040100000000000000e601000000000000002d0100000000000000120100000000000000840100000000000000450100000000000000df0100000000000000e801000000000000002501000000000000007e0100000000000000c80100000000000000d401000000000000004e0100000000000000260100000000000000540100000000000000000100000000000000a401000000000000004a0100000000000000780100000000000000c60100000000000000410100000000000000470100000000000000aa01000000000000000b0100000000000000650100000000000000c90100000000000000b301000000000000007a01000000000000007a0100000000000000ea710200000000000009000000000000000100000000000000015802000000000000020000000000000020010000000000000100000000000000790100000000000000a901000000000000003a0100000000000000990100000000000000b70100000000000000a70100000000000000510100000000000000e00100000000000000390100000000000000f10100000000000000780100000000000000cf01000000000000000401000000000000002501000000000000004901000000000000005c0100000000000000020100000000000000fa01000000000000006a01000000000000000d0100000000000000940100000000000000d60100000000000000d10100000000000000b501000000000000002b01000000000000003f0100000000000000790100000000000000660100000000000000830100000000000000e7010000000000000022010000000000000096200100000000000001000000000000006a01000000000000000b0100000000000000040100000000000000e601000000000000002d0100000000000000120100000000000000840100000000000000450100000000000000df0100000000000000e801000000000000002501000000000000007e0100000000000000c80100000000000000d401000000000000004e0100000000000000260100000000000000540100000000000000000100000000000000a401000000000000004a0100000000000000780100000000000000c60100000000000000410100000000000000470100000000000000aa01000000000000000b0100000000000000650100000000000000c90100000000000000b301000000000000007a01000000000000007a0100000000000000ea08000000000000001a05340bd4fdf4d0490100000000000009000000000000000100000000000000003001000000000000010000000000000020010000000000000100000000000000760100000000000000e20100000000000000440100000000000000570100000000000000c90100000000000000330100000000000000390100000000000000c301000000000000007e0100000000000000480100000000000000fe0100000000000000820100000000000000c60100000000000000700100000000000000220100000000000000580100000000000000f101000000000000005401000000000000006e0100000000000000af01000000000000005f0100000000000000770100000000000000440100000000000000620100000000000000b601000000000000002201000000000000009d0100000000000000f001000000000000000b0100000000000000420100000000000000a201000000000000002c060300000000000004000000000000000000000008000000000000001a05340bd4fdf4d0080000000000000034c5d317bfe29b1a490100000000000009000000000000000100000000000000003001000000000000010000000000000020010000000000000100000000000000760100000000000000e20100000000000000440100000000000000570100000000000000c90100000000000000330100000000000000390100000000000000c301000000000000007e0100000000000000480100000000000000fe0100000000000000820100000000000000c60100000000000000700100000000000000220100000000000000580100000000000000f101000000000000005401000000000000006e0100000000000000af01000000000000005f0100000000000000770100000000000000440100000000000000620100000000000000b601000000000000002201000000000000009d0100000000000000f001000000000000000b0100000000000000420100000000000000a201000000000000002c490100000000000009000000000000000100000000000000013001000000000000010000000000000020010000000000000100000000000000540100000000000000100100000000000000be0100000000000000ee01000000000000009801000000000000005f01000000000000008a01000000000000009b0100000000000000b501000000000000005a0100000000000000720100000000000000c901000000000000003a0100000000000000460100000000000000b60100000000000000760100000000000000f90100000000000000f901000000000000001701000000000000005801000000000000007801000000000000009b0100000000000000e00100000000000000c10100000000000000490100000000000000980100000000000000340100000000000000f601000000000000007301000000000000001601000000000000000c01000000000000006b080000000000000066e4102520642cde20000000000000000800000000000000000000000000000008000000000000000000000000000000e90000000000000001e0000000000000000100000000000000d000000000000000080000000000000021b2e0e27b2f43b40800000000000000070000000000000008000000000000000200000000000000700000000000000002000000000000002c0000000000000004000000000000000000000008000000000000001a05340bd4fdf4d0080000000000000003000000000000002c00000000000000040000000000000001000000080000000000000034c5d317bfe29b1a080000000000000005000000000000002000000000000000080000000000000000000000000000000800000000000000000000000000000012000000000000000a00000000000000544553542d535441524b"].assert_eq(&hex);
}
