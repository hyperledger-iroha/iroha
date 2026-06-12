#![doc = "End-to-end test for the native STARK (FRI single-fold) verifier."]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "zk-tests")]
//! End-to-end test for the native STARK (FRI single-fold) verifier.

#![cfg(feature = "zk-stark")]

use expect_test::expect;
use fastpq_prover::{hash_field_elements, pack_bytes};
use iroha_core::{
    zk::verify_backend,
    zk_stark::{
        FoldDecommitV1, MerklePath, STARK_HASH_POSEIDON2_V1, STARK_HASH_SHA256_V1,
        StarkCommitmentsV1, StarkCompositionTermV1, StarkCompositionValueV1, StarkFriParamsV1,
        StarkFriVerifyingKeyV1, StarkProofV1, StarkVerifierLimits, StarkVerifyEnvelopeV1,
        prove_stark_fri_air_envelope_bytes, prove_stark_fri_composition_envelope_bytes,
        verify_stark_fri_envelope, verify_stark_fri_envelope_with_limits,
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

fn field_sub(a: u64, b: u64) -> u64 {
    if a >= b {
        a - b
    } else {
        ((a as u128 + MOD_P) - b as u128) as u64
    }
}

fn field_pow(mut base: u64, mut exponent: u128) -> u64 {
    let mut acc = 1u64;
    while exponent > 0 {
        if exponent & 1 == 1 {
            acc = field_mul(acc, base);
        }
        base = field_mul(base, base);
        exponent >>= 1;
    }
    acc
}

fn field_inv(value: u64) -> Option<u64> {
    (value != 0).then(|| field_pow(value, MOD_P - 2))
}

fn field_two_inv() -> u64 {
    ((MOD_P + 1) / 2) as u64
}

fn domain_x_for_pair(layer_domain: usize, pair_index: usize) -> u64 {
    assert!(layer_domain >= 2 && layer_domain.is_power_of_two());
    assert!(pair_index < layer_domain / 2);
    let root = field_pow(7, (MOD_P - 1) / layer_domain as u128);
    field_pow(root, pair_index as u128)
}

fn fri_fold_pair_for_test(y0: u64, y1: u64, beta: u64, x: u64) -> u64 {
    let even = field_mul(field_add(y0, y1), field_two_inv());
    let inv_2x = field_inv(field_mul(2, x)).expect("non-zero domain element");
    let odd = field_mul(field_sub(y0, y1), inv_2x);
    field_add(even, field_mul(beta, odd))
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

fn u64_to_digest_le(val: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[..8].copy_from_slice(&val.to_le_bytes());
    out
}

fn digest_le_to_u64(bytes: &[u8; 32]) -> u64 {
    assert!(
        bytes[8..].iter().all(|b| *b == 0),
        "non-canonical poseidon digest encoding"
    );
    u64::from_le_bytes(bytes[..8].try_into().expect("slice len = 8"))
}

fn poseidon_domain_hash_u64(domain: &[u8], values: &[u64]) -> u64 {
    let packed = pack_bytes(domain);
    let len_field = u64::try_from(packed.length).unwrap_or(u64::MAX);
    let mut limbs = Vec::with_capacity(1 + packed.limbs.len() + values.len());
    limbs.push(len_field);
    limbs.extend_from_slice(&packed.limbs);
    limbs.extend_from_slice(values);
    hash_field_elements(&limbs)
}

fn leaf_hash_poseidon_u64(v: u64) -> [u8; 32] {
    u64_to_digest_le(poseidon_domain_hash_u64(b"iroha:zk:stark:leaf:v1", &[v]))
}

fn node_hash_poseidon(l: &[u8; 32], r: &[u8; 32]) -> [u8; 32] {
    let l = digest_le_to_u64(l);
    let r = digest_le_to_u64(r);
    u64_to_digest_le(poseidon_domain_hash_u64(b"iroha:zk:stark:node:v1", &[l, r]))
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

fn merkle_root_from_leaves_poseidon(mut leaves: Vec<[u8; 32]>) -> ([u8; 32], Vec<Vec<[u8; 32]>>) {
    let mut levels = Vec::new();
    levels.push(leaves.clone());
    while leaves.len() > 1 {
        let mut next = Vec::with_capacity(leaves.len() / 2);
        for i in (0..leaves.len()).step_by(2) {
            next.push(node_hash_poseidon(&leaves[i], &leaves[i + 1]));
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

fn poseidon_hash_bytes(preimage: &[u8]) -> u64 {
    let packed = pack_bytes(preimage);
    let len_field = u64::try_from(packed.length).unwrap_or(u64::MAX);
    let mut limbs = Vec::with_capacity(packed.limbs.len() + 1);
    limbs.push(len_field);
    limbs.extend_from_slice(&packed.limbs);
    hash_field_elements(&limbs)
}

fn derive_query_index_for_test_poseidon(
    label: &str,
    params: &StarkFriParamsV1,
    roots: &[[u8; 32]],
    query_idx: usize,
) -> usize {
    let mut preimage = Vec::new();
    preimage.extend_from_slice(b"STARK:query-index");
    preimage.extend_from_slice(label.as_bytes());
    preimage.extend_from_slice(&params.version.to_le_bytes());
    preimage.extend_from_slice(&[
        params.n_log2,
        params.blowup_log2,
        params.fold_arity,
        params.merkle_arity,
        params.hash_fn,
    ]);
    preimage.extend_from_slice(&params.queries.to_le_bytes());
    preimage.extend_from_slice(&(params.domain_tag.len() as u32).to_le_bytes());
    preimage.extend_from_slice(params.domain_tag.as_bytes());
    preimage.extend_from_slice(&(query_idx as u64).to_le_bytes());
    for root in roots {
        preimage.extend_from_slice(root);
    }
    let digest = poseidon_hash_bytes(&preimage);
    let domain = 1usize << params.n_log2;
    (digest % (domain as u64)) as usize
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

fn challenge_poseidon_u64(label: &str, bytes: &[u8]) -> u64 {
    let mut preimage = Vec::with_capacity(label.len() + 1 + bytes.len());
    preimage.extend_from_slice(label.as_bytes());
    preimage.push(0);
    preimage.extend_from_slice(bytes);
    poseidon_hash_bytes(&preimage)
}

fn stark_open_verify_domain_tag_current(
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
        .map(|j| {
            fri_fold_pair_for_test(evals[2 * j], evals[2 * j + 1], r0, domain_x_for_pair(n, j))
        })
        .collect();
    let leaves1: Vec<[u8; 32]> = layer1.iter().map(|&v| leaf_hash_u64(v)).collect();
    let (root1, levels1) = merkle_root_from_leaves(leaves1.clone());

    // Derive r1 from label+params+root1
    let r1 = challenge_u64("stark:fri:r:k", &build_transcript(&root1));

    // Derive r2 from label+params+root2 (will be used for next fold)
    // Layer 2 with r1
    let layer2: Vec<u64> = (0..n / 4)
        .map(|j| {
            fri_fold_pair_for_test(
                layer1[2 * j],
                layer1[2 * j + 1],
                r1,
                domain_x_for_pair(n / 2, j),
            )
        })
        .collect();
    let leaves2: Vec<[u8; 32]> = layer2.iter().map(|&v| leaf_hash_u64(v)).collect();
    let (root2, levels2) = merkle_root_from_leaves(leaves2.clone());

    let r2 = challenge_u64("stark:fri:r:k", &build_transcript(&root2));

    // Layer 3 with r2 (final layer size = 1); only j=0 valid
    let layer3: Vec<u64> = (0..n / 8)
        .map(|j| {
            fri_fold_pair_for_test(
                layer2[2 * j],
                layer2[2 * j + 1],
                r2,
                domain_x_for_pair(n / 4, j),
            )
        })
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
            air: None,
        },
        transcript_label: "TEST-STARK".to_string(),
    }
}

#[allow(clippy::too_many_lines)]
fn build_sample_envelope_poseidon2_with_domain_tag(domain_tag: String) -> StarkVerifyEnvelopeV1 {
    let n_log2 = 3u8;
    let n = 1usize << n_log2;
    let evals: Vec<u64> = (0..n)
        .map(|x| field_add(field_mul(3, x as u64), 5))
        .collect();
    let leaves0: Vec<[u8; 32]> = evals.iter().map(|&v| leaf_hash_poseidon_u64(v)).collect();
    let (root0, levels0) = merkle_root_from_leaves_poseidon(leaves0.clone());

    let params = StarkFriParamsV1 {
        version: 1,
        n_log2,
        blowup_log2: 3,
        fold_arity: 2,
        queries: 1,
        merkle_arity: 2,
        hash_fn: STARK_HASH_POSEIDON2_V1,
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

    let r0 = challenge_poseidon_u64("stark:fri:r:k", &build_transcript(&root0));

    let layer1: Vec<u64> = (0..n / 2)
        .map(|j| {
            fri_fold_pair_for_test(evals[2 * j], evals[2 * j + 1], r0, domain_x_for_pair(n, j))
        })
        .collect();
    let leaves1: Vec<[u8; 32]> = layer1.iter().map(|&v| leaf_hash_poseidon_u64(v)).collect();
    let (root1, levels1) = merkle_root_from_leaves_poseidon(leaves1.clone());

    let r1 = challenge_poseidon_u64("stark:fri:r:k", &build_transcript(&root1));

    let layer2: Vec<u64> = (0..n / 4)
        .map(|j| {
            fri_fold_pair_for_test(
                layer1[2 * j],
                layer1[2 * j + 1],
                r1,
                domain_x_for_pair(n / 2, j),
            )
        })
        .collect();
    let leaves2: Vec<[u8; 32]> = layer2.iter().map(|&v| leaf_hash_poseidon_u64(v)).collect();
    let (root2, levels2) = merkle_root_from_leaves_poseidon(leaves2.clone());

    let r2 = challenge_poseidon_u64("stark:fri:r:k", &build_transcript(&root2));

    let layer3: Vec<u64> = (0..n / 8)
        .map(|j| {
            fri_fold_pair_for_test(
                layer2[2 * j],
                layer2[2 * j + 1],
                r2,
                domain_x_for_pair(n / 4, j),
            )
        })
        .collect();
    let leaves3: Vec<[u8; 32]> = layer3.iter().map(|&v| leaf_hash_poseidon_u64(v)).collect();
    let (root3, levels3) = merkle_root_from_leaves_poseidon(leaves3.clone());

    let commitments_roots = vec![root0, root1, root2, root3];
    let base_index =
        derive_query_index_for_test_poseidon("TEST-STARK", &params, &commitments_roots, 0);
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
    let comp_leaves = vec![leaf_hash_poseidon_u64(comp_leaf)];
    let (comp_root, comp_levels) = merkle_root_from_leaves_poseidon(comp_leaves);
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
            air: None,
        },
        transcript_label: "TEST-STARK".to_string(),
    }
}

fn build_sample_envelope() -> StarkVerifyEnvelopeV1 {
    build_sample_envelope_with_domain_tag("fastpq:v1:fri".to_string())
}

fn sample_air_params(domain_tag: String, hash_fn: u8) -> StarkFriParamsV1 {
    StarkFriParamsV1 {
        version: 1,
        n_log2: 3,
        blowup_log2: 3,
        fold_arity: 2,
        queries: 1,
        merkle_arity: 2,
        hash_fn,
        domain_tag,
    }
}

fn build_sample_air_envelope_with_domain_tag(
    domain_tag: String,
    hash_fn: u8,
) -> StarkVerifyEnvelopeV1 {
    let backend = if hash_fn == STARK_HASH_POSEIDON2_V1 {
        "stark/fri/poseidon2-goldilocks"
    } else {
        "stark/fri/sha256-goldilocks"
    };
    let bytes = prove_stark_fri_air_envelope_bytes(
        sample_air_params(domain_tag, hash_fn),
        "TEST-STARK".to_string(),
        format!("{backend}:test"),
        [0x42; 32],
    )
    .expect("build sample AIR envelope");
    norito::decode_from_bytes(&bytes).expect("decode sample AIR envelope")
}

fn build_sample_air_envelope_poseidon2() -> StarkVerifyEnvelopeV1 {
    build_sample_air_envelope_with_domain_tag("fastpq:v1:fri".to_string(), STARK_HASH_POSEIDON2_V1)
}

fn sample_composition_terms() -> Vec<StarkCompositionTermV1> {
    vec![
        StarkCompositionTermV1 {
            wire_index: 0,
            value: 11,
            coeff: 3,
        },
        StarkCompositionTermV1 {
            wire_index: 1,
            value: 17,
            coeff: 5,
        },
    ]
}

fn build_sample_air_composition_envelope_with_domain_tag(
    domain_tag: String,
) -> StarkVerifyEnvelopeV1 {
    let bytes = prove_stark_fri_composition_envelope_bytes(
        sample_air_params(domain_tag, STARK_HASH_SHA256_V1),
        "TEST-STARK".to_string(),
        7,
        2,
        sample_composition_terms(),
    )
    .expect("build sample AIR composition envelope");
    norito::decode_from_bytes(&bytes).expect("decode sample AIR composition envelope")
}

fn build_sample_air_composition_envelope() -> StarkVerifyEnvelopeV1 {
    build_sample_air_composition_envelope_with_domain_tag("fastpq:v1:fri".to_string())
}

fn build_stark_open_verify_envelope_bytes_for_columns(
    backend: &str,
    circuit_id: &str,
    vk_hash: [u8; 32],
    schema_descriptor: &[u8],
    public_inputs: Vec<Vec<[u8; 32]>>,
) -> Vec<u8> {
    use iroha_data_model::zk::{BackendTag, OpenVerifyEnvelope, StarkFriOpenProofV1};

    let domain_tag = stark_open_verify_domain_tag_current(
        backend,
        circuit_id,
        vk_hash,
        schema_descriptor,
        &public_inputs,
    );
    let inner = build_sample_envelope_with_domain_tag(domain_tag);
    let envelope_bytes = norito::to_bytes(&inner).expect("encode STARK inner envelope");
    let open = StarkFriOpenProofV1 {
        version: 1,
        public_inputs,
        envelope_bytes,
    };
    let proof_bytes = norito::to_bytes(&open).expect("encode STARK open proof");
    let env = OpenVerifyEnvelope {
        backend: BackendTag::Stark,
        circuit_id: circuit_id.to_string(),
        vk_hash,
        public_inputs: schema_descriptor.to_vec(),
        proof_bytes,
        aux: Vec::new(),
    };
    norito::to_bytes(&env).expect("encode OpenVerifyEnvelope")
}

fn derive_ballot_nullifier_for_test(
    domain_tag: &str,
    chain_id: &iroha_data_model::ChainId,
    election_id: &str,
    commit: &[u8; 32],
) -> [u8; 32] {
    use blake2::{Blake2b512, Digest as _};

    let mut input = Vec::with_capacity(
        domain_tag.len() + chain_id.as_str().len() + election_id.len() + commit.len() + 24,
    );
    let push_len = |buf: &mut Vec<u8>, len: usize| {
        let len_u64 = len as u64;
        buf.extend_from_slice(&len_u64.to_le_bytes());
    };
    push_len(&mut input, domain_tag.len());
    input.extend_from_slice(domain_tag.as_bytes());
    push_len(&mut input, chain_id.as_str().len());
    input.extend_from_slice(chain_id.as_str().as_bytes());
    push_len(&mut input, election_id.len());
    input.extend_from_slice(election_id.as_bytes());
    input.extend_from_slice(commit);
    let digest = Blake2b512::digest(&input);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
}

fn sample_stark_vk_box(
    backend: &str,
    circuit_id: &str,
    hash_fn: u8,
) -> iroha_data_model::proof::VerifyingKeyBox {
    let payload = StarkFriVerifyingKeyV1 {
        version: 1,
        circuit_id: circuit_id.to_string(),
        n_log2: 3,
        blowup_log2: 3,
        fold_arity: 2,
        queries: 1,
        merkle_arity: 2,
        hash_fn,
    };
    let bytes = norito::to_bytes(&payload).expect("encode STARK verifying key payload");
    iroha_data_model::proof::VerifyingKeyBox::new(backend.into(), bytes)
}

#[test]
fn stark_single_fold_roundtrip_ok_and_fail() {
    let env = build_sample_air_composition_envelope();

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
    env_bad_hash.params.hash_fn = 3;
    let bytes_bad_hash = norito::to_bytes(&env_bad_hash).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes_bad_hash),
        "unsupported hash selector must fail"
    );
}

#[test]
fn stark_rejects_duplicate_auxiliary_composition_wires() {
    let mut env = build_sample_air_composition_envelope();
    let comp_values = env
        .proof
        .comp_values
        .as_mut()
        .expect("sample composition envelope has composition values");
    let first_wire = comp_values[0].aux_terms[0].wire_index;
    comp_values[0].aux_terms[1].wire_index = first_wire;

    let bytes = norito::to_bytes(&env).expect("encode duplicate auxiliary wires");
    assert!(
        !verify_stark_fri_envelope(&bytes),
        "duplicate auxiliary composition wires must be rejected"
    );
}

#[test]
fn stark_rejects_auxiliary_composition_wire_retarget_without_digest_match() {
    let mut env = build_sample_air_composition_envelope();
    let comp_values = env
        .proof
        .comp_values
        .as_mut()
        .expect("sample composition envelope has composition values");
    comp_values[0].aux_terms[1].wire_index =
        comp_values[0].aux_terms[1].wire_index.saturating_add(1);

    let bytes = norito::to_bytes(&env).expect("encode retargeted auxiliary wire");
    assert!(
        !verify_stark_fri_envelope(&bytes),
        "auxiliary wire-index retargeting must remain bound to the AIR public digest"
    );
}

#[test]
fn stark_composition_constructor_requires_strict_auxiliary_wire_order() {
    let params = sample_air_params("fastpq:v1:fri".to_string(), STARK_HASH_SHA256_V1);
    let duplicate_terms = vec![
        StarkCompositionTermV1 {
            wire_index: 1,
            value: 11,
            coeff: 3,
        },
        StarkCompositionTermV1 {
            wire_index: 1,
            value: 17,
            coeff: 5,
        },
    ];
    let duplicate_err = prove_stark_fri_composition_envelope_bytes(
        params.clone(),
        "TEST-STARK".to_string(),
        7,
        2,
        duplicate_terms,
    )
    .expect_err("duplicate auxiliary wires must fail before proof construction");
    assert!(
        duplicate_err.contains("strictly ordered"),
        "unexpected duplicate-wire error: {duplicate_err}"
    );

    let unsorted_terms = vec![
        StarkCompositionTermV1 {
            wire_index: 2,
            value: 11,
            coeff: 3,
        },
        StarkCompositionTermV1 {
            wire_index: 1,
            value: 17,
            coeff: 5,
        },
    ];
    let unsorted_err = prove_stark_fri_composition_envelope_bytes(
        params,
        "TEST-STARK".to_string(),
        7,
        2,
        unsorted_terms,
    )
    .expect_err("unsorted auxiliary wires must fail before proof construction");
    assert!(
        unsorted_err.contains("strictly ordered"),
        "unexpected unsorted-wire error: {unsorted_err}"
    );
}

#[test]
fn stark_low_level_envelope_requires_air_section() {
    let env = build_sample_envelope();
    let bytes = norito::to_bytes(&env).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes),
        "native STARK verifier must reject V1 envelopes without AIR openings"
    );
}

#[test]
fn stark_poseidon2_roundtrip_ok() {
    let env = build_sample_air_envelope_poseidon2();
    let bytes = norito::to_bytes(&env).expect("encode");
    assert!(
        verify_stark_fri_envelope(&bytes),
        "native STARK verifier rejected poseidon2 envelope"
    );
}

#[test]
fn stark_rejects_mismatched_merkle_indices() {
    let mut env =
        build_sample_air_envelope_with_domain_tag("index-test".to_string(), STARK_HASH_SHA256_V1);
    let first = &mut env.proof.queries[0][0];
    core::mem::swap(&mut first.path_y0, &mut first.path_y1);
    let bytes = norito::to_bytes(&env).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes),
        "index-mismatched Merkle openings must be rejected"
    );
}

#[test]
fn stark_rejects_unbound_air_composition_root() {
    let mut env = build_sample_air_composition_envelope();
    env.proof
        .air
        .as_mut()
        .expect("AIR section")
        .composition_root[0] ^= 0x01;
    let bytes = norito::to_bytes(&env).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes),
        "AIR composition root must match FRI layer zero"
    );
}

#[test]
fn stark_rejects_tampered_air_trace_root() {
    let mut env = build_sample_air_composition_envelope();
    env.proof.air.as_mut().expect("AIR section").trace_root[0] ^= 0x01;
    let bytes = norito::to_bytes(&env).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes),
        "AIR trace root must authenticate sampled trace rows"
    );
}

#[test]
fn stark_rejects_tampered_air_public_digest() {
    let mut env = build_sample_air_composition_envelope();
    env.proof.air.as_mut().expect("AIR section").public_digest[0] ^= 0x01;
    let bytes = norito::to_bytes(&env).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes),
        "AIR public digest must remain bound to sampled rows and composition openings"
    );
}

#[test]
fn stark_rejects_air_trace_width_mismatch() {
    let mut env = build_sample_air_composition_envelope();
    let air = env.proof.air.as_mut().expect("AIR section");
    air.trace_width = air.trace_width.saturating_add(1);
    let bytes = norito::to_bytes(&env).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes),
        "AIR trace width must match the V1 AIR layout"
    );
}

#[test]
fn stark_rejects_air_opening_count_mismatch() {
    let mut env = build_sample_air_composition_envelope();
    let air = env.proof.air.as_mut().expect("AIR section");
    assert_eq!(air.openings.len(), env.proof.queries.len());
    air.openings.clear();
    let bytes = norito::to_bytes(&env).expect("encode");
    assert!(
        !verify_stark_fri_envelope(&bytes),
        "AIR opening count must match verifier query count"
    );
}

#[test]
fn stark_air_width_limit_is_enforced() {
    let env = build_sample_air_composition_envelope();
    let bytes = norito::to_bytes(&env).expect("encode");
    let trace_width = env.proof.air.as_ref().expect("AIR section").trace_width as usize;
    assert!(trace_width > 1, "sample AIR trace must have width");
    let mut limits = StarkVerifierLimits::default();
    limits.max_air_width = trace_width - 1;
    assert!(
        !verify_stark_fri_envelope_with_limits(&bytes, &limits),
        "AIR trace width must respect verifier limits"
    );
}

#[test]
fn stark_open_verify_envelope_rejects_synthetic_air_proof() {
    use iroha_data_model::{
        proof::ProofBox,
        zk::{BackendTag, OpenVerifyEnvelope, StarkFriOpenProofV1},
    };

    let backend = "stark/fri/sha256-goldilocks";
    let circuit_id = "ivm-execution-v1";

    let vk_box = sample_stark_vk_box(backend, circuit_id, STARK_HASH_SHA256_V1);
    let vk_hash = iroha_core::zk::hash_vk(&vk_box);

    // Two columns, one row each (matches the instance-column shape used by other backends).
    let public_inputs = vec![vec![[0xAA; 32]], vec![[0xBB; 32]]];
    let env_public_inputs = b"schema:test".to_vec();

    let domain_tag = stark_open_verify_domain_tag_current(
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
        !verify_backend(backend, &proof, Some(&vk_box)),
        "wrapped STARK OpenVerifyEnvelope must fail closed when the AIR section is missing"
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

#[test]
fn stark_open_verify_envelope_poseidon2_variant_rejects_synthetic_air_proof() {
    use iroha_data_model::{
        proof::ProofBox,
        zk::{BackendTag, OpenVerifyEnvelope, StarkFriOpenProofV1},
    };

    let backend = "stark/fri/poseidon2-goldilocks";
    let circuit_id = "ivm-execution-v1";

    let vk_box = sample_stark_vk_box(backend, circuit_id, STARK_HASH_POSEIDON2_V1);
    let vk_hash = iroha_core::zk::hash_vk(&vk_box);

    let public_inputs = vec![vec![[0x11; 32]], vec![[0x22; 32]]];
    let env_public_inputs = b"schema:test".to_vec();
    let domain_tag = stark_open_verify_domain_tag_current(
        backend,
        circuit_id,
        vk_hash,
        &env_public_inputs,
        &public_inputs,
    );
    let inner = build_sample_envelope_poseidon2_with_domain_tag(domain_tag);
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
        !verify_backend(backend, &proof, Some(&vk_box)),
        "wrapped STARK OpenVerifyEnvelope must fail closed when the AIR section is missing"
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
fn stark_ivm_proved_execution_admission_rejects_synthetic_air_proof() {
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
            ProofAttachment, ProofAttachmentList, ProofBox, VerifyingKeyId, VerifyingKeyRecord,
        },
        transaction::{Executable, IvmProved},
        zk::{BackendTag, OpenVerifyEnvelope, StarkFriOpenProofV1},
    };
    use iroha_primitives::json::Json;

    let backend = "stark/fri/sha256-goldilocks";
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
    let authority = AccountId::new(kp.public_key().clone());
    let domain_id: iroha_data_model::domain::DomainId =
        iroha_data_model::domain::DomainId::try_new("wonderland", "universal").unwrap();
    let domain = Domain::new(domain_id.clone()).build(&authority);
    let account = Account::new(authority.clone()).build(&authority);

    let world = iroha_core::state::World::with([domain], [account], []);

    let vk_id = VerifyingKeyId::new(backend, "ivm_execution_stark");
    let vk_box = sample_stark_vk_box(backend, circuit_id, STARK_HASH_SHA256_V1);
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

    let domain_tag = stark_open_verify_domain_tag_current(
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

    let err =
        iroha_core::pipeline::overlay::build_overlay_for_transaction(&tx_proved, &state.view())
            .expect_err("synthetic STARK proved execution must be rejected");
    let err_text = format!("{err:?}");
    assert!(
        err_text.contains("proof rejected"),
        "unexpected proved execution rejection: {err:?}"
    );
}

#[test]
fn stark_governance_submit_rejects_synthetic_air_proof() {
    use core::num::NonZeroU64;

    use iroha_core::{
        kura::Kura,
        query::store::LiveQueryStore,
        smartcontracts::Execute,
        state::{State, World},
    };
    use iroha_data_model::{
        Registrable,
        account::Account,
        block::BlockHeader,
        confidential::ConfidentialStatus,
        domain::Domain,
        isi::{
            Grant, verifying_keys,
            zk::{CreateElection, SubmitBallot},
        },
        permission::Permission,
        proof::{ProofAttachment, ProofBox, VerifyingKeyId, VerifyingKeyRecord},
        zk::BackendTag,
    };
    use iroha_executor_data_model::permission::governance::{
        CanManageParliament, CanSubmitGovernanceBallot,
    };
    use iroha_primitives::json::Json;
    use iroha_test_samples::ALICE_ID;

    let backend = "stark/fri/sha256-goldilocks";
    let ballot_circuit_id = "stark/fri/sha256-goldilocks:vote-ballot";
    let tally_circuit_id = "stark/fri/sha256-goldilocks:vote-tally";
    let election_id = "stark-vote-e2e".to_string();
    let nullifier_domain = "gov:ballot:v1";

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let domain_id: iroha_data_model::domain::DomainId =
        iroha_data_model::domain::DomainId::try_new("wonderland", "universal").expect("domain");
    let domain: Domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
    let account: Account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let world = World::with([domain], [account], Vec::new());
    let mut state = State::new_for_testing(world, kura, query);
    state.zk.stark.enabled = true;
    state.zk.halo2.enabled = false;
    state.zk.verify_timeout = std::time::Duration::ZERO;
    state.gov.citizenship_bond_amount = 0;
    state.gov.min_bond_amount = 0;

    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let perm_vk = Permission::new("CanManageVerifyingKeys".to_string(), Json::new(()));
    Grant::account_permission(perm_vk, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant CanManageVerifyingKeys");
    let perm_parliament: Permission = CanManageParliament.into();
    Grant::account_permission(perm_parliament, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant CanManageParliament");
    let perm_ballot: Permission = CanSubmitGovernanceBallot {
        referendum_id: election_id.clone(),
    }
    .into();
    Grant::account_permission(perm_ballot, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant CanSubmitGovernanceBallot");
    let ballot_vk_id = VerifyingKeyId::new(backend, "vote_ballot");
    let ballot_vk_box = sample_stark_vk_box(backend, ballot_circuit_id, STARK_HASH_SHA256_V1);
    let ballot_vk_hash = iroha_core::zk::hash_vk(&ballot_vk_box);
    let ballot_schema = b"gov:vote:ballot:schema:v1".to_vec();
    let ballot_schema_hash: [u8; 32] = iroha_crypto::Hash::new(&ballot_schema).into();
    let mut ballot_vk_record = VerifyingKeyRecord::new(
        1,
        ballot_circuit_id,
        BackendTag::Stark,
        "goldilocks",
        ballot_schema_hash,
        ballot_vk_hash,
    );
    ballot_vk_record.status = ConfidentialStatus::Active;
    ballot_vk_record.gas_schedule_id = Some("sched_ballot".to_string());
    ballot_vk_record.key = Some(ballot_vk_box.clone());
    verifying_keys::RegisterVerifyingKey {
        id: ballot_vk_id.clone(),
        record: ballot_vk_record,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("register ballot vk");

    let tally_vk_id = VerifyingKeyId::new(backend, "vote_tally");
    let tally_vk_box = sample_stark_vk_box(backend, tally_circuit_id, STARK_HASH_SHA256_V1);
    let tally_vk_hash = iroha_core::zk::hash_vk(&tally_vk_box);
    let tally_schema = b"gov:vote:tally:schema:v1".to_vec();
    let tally_schema_hash: [u8; 32] = iroha_crypto::Hash::new(&tally_schema).into();
    let mut tally_vk_record = VerifyingKeyRecord::new(
        1,
        tally_circuit_id,
        BackendTag::Stark,
        "goldilocks",
        tally_schema_hash,
        tally_vk_hash,
    );
    tally_vk_record.status = ConfidentialStatus::Active;
    tally_vk_record.gas_schedule_id = Some("sched_tally".to_string());
    tally_vk_record.key = Some(tally_vk_box.clone());
    verifying_keys::RegisterVerifyingKey {
        id: tally_vk_id.clone(),
        record: tally_vk_record,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("register tally vk");

    let eligible_root = [0x22; 32];
    CreateElection {
        election_id: election_id.clone(),
        options: 2,
        eligible_root,
        start_ts: 0,
        end_ts: 0,
        vk_ballot: ballot_vk_id.clone(),
        vk_tally: tally_vk_id.clone(),
        domain_tag: nullifier_domain.to_string(),
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("create election");

    let commit = [0x11; 32];
    let ballot_columns = vec![vec![commit], vec![eligible_root]];
    let ballot_proof_bytes = build_stark_open_verify_envelope_bytes_for_columns(
        backend,
        ballot_circuit_id,
        ballot_vk_hash,
        &ballot_schema,
        ballot_columns,
    );
    let ballot_attachment = ProofAttachment::new_ref(
        backend.to_string(),
        ProofBox::new(backend.to_string(), ballot_proof_bytes),
        ballot_vk_id,
    );
    let nullifier =
        derive_ballot_nullifier_for_test(nullifier_domain, &state.chain_id, &election_id, &commit);
    let err = SubmitBallot {
        election_id: election_id.clone(),
        ciphertext: commit.to_vec(),
        ballot_proof: ballot_attachment,
        nullifier,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect_err("synthetic STARK ballot must be rejected");
    let err_text = format!("{err:?}");
    assert!(
        err_text.contains("invalid ballot proof"),
        "unexpected ballot rejection: {err:?}"
    );
}

#[test]
fn create_election_rejects_stark_vk_with_wrong_vote_circuit_role() {
    use core::num::NonZeroU64;

    use iroha_core::{
        kura::Kura, query::store::LiveQueryStore, smartcontracts::Execute, state::State,
    };
    use iroha_data_model::{
        Registrable,
        account::Account,
        block::BlockHeader,
        confidential::ConfidentialStatus,
        domain::Domain,
        isi::{Grant, verifying_keys, zk::CreateElection},
        permission::Permission,
        proof::{VerifyingKeyId, VerifyingKeyRecord},
        zk::BackendTag,
    };
    use iroha_executor_data_model::permission::governance::CanManageParliament;
    use iroha_primitives::json::Json;
    use iroha_test_samples::ALICE_ID;

    let backend = "stark/fri/sha256-goldilocks";
    let bad_ballot_circuit_id = "stark/fri/sha256-goldilocks:not-a-ballot-circuit";
    let tally_circuit_id = "stark/fri/sha256-goldilocks:vote-tally";
    let ballot_schema_hash: [u8; 32] = iroha_crypto::Hash::new(b"gov:vote:ballot:schema:v1").into();
    let tally_schema_hash: [u8; 32] = iroha_crypto::Hash::new(b"gov:vote:tally:schema:v1").into();

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let domain_id: iroha_data_model::domain::DomainId =
        iroha_data_model::domain::DomainId::try_new("wonderland", "universal").expect("domain");
    let domain: Domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
    let account: Account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let mut state = State::new_for_testing(
        iroha_core::state::World::with([domain], [account], Vec::new()),
        kura,
        query,
    );
    state.zk.stark.enabled = true;
    state.zk.halo2.enabled = false;
    state.zk.verify_timeout = std::time::Duration::ZERO;

    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let perm_vk = Permission::new("CanManageVerifyingKeys".to_string(), Json::new(()));
    Grant::account_permission(perm_vk, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant CanManageVerifyingKeys");
    let perm_parliament: Permission = CanManageParliament.into();
    Grant::account_permission(perm_parliament, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant CanManageParliament");

    let ballot_vk_id = VerifyingKeyId::new(backend, "bad_vote_ballot");
    let ballot_vk_box = sample_stark_vk_box(backend, bad_ballot_circuit_id, STARK_HASH_SHA256_V1);
    let mut ballot_vk_record = VerifyingKeyRecord::new(
        1,
        bad_ballot_circuit_id,
        BackendTag::Stark,
        "goldilocks",
        ballot_schema_hash,
        iroha_core::zk::hash_vk(&ballot_vk_box),
    );
    ballot_vk_record.status = ConfidentialStatus::Active;
    ballot_vk_record.gas_schedule_id = Some("sched_bad_ballot".to_owned());
    ballot_vk_record.key = Some(ballot_vk_box);
    verifying_keys::RegisterVerifyingKey {
        id: ballot_vk_id.clone(),
        record: ballot_vk_record,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("register bad ballot vk");

    let tally_vk_id = VerifyingKeyId::new(backend, "vote_tally");
    let tally_vk_box = sample_stark_vk_box(backend, tally_circuit_id, STARK_HASH_SHA256_V1);
    let mut tally_vk_record = VerifyingKeyRecord::new(
        1,
        tally_circuit_id,
        BackendTag::Stark,
        "goldilocks",
        tally_schema_hash,
        iroha_core::zk::hash_vk(&tally_vk_box),
    );
    tally_vk_record.status = ConfidentialStatus::Active;
    tally_vk_record.gas_schedule_id = Some("sched_tally".to_owned());
    tally_vk_record.key = Some(tally_vk_box);
    verifying_keys::RegisterVerifyingKey {
        id: tally_vk_id.clone(),
        record: tally_vk_record,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("register tally vk");

    let err = CreateElection {
        election_id: "stark-vote-role-check".to_owned(),
        options: 2,
        eligible_root: [0x22; 32],
        start_ts: 0,
        end_ts: 0,
        vk_ballot: ballot_vk_id,
        vk_tally: tally_vk_id,
        domain_tag: "gov:ballot:v1".to_owned(),
    }
    .execute(&ALICE_ID, &mut stx)
    .expect_err("create election must reject wrong STARK ballot role");
    let err_text = format!("{err:?}");
    assert!(
        err_text.contains("ballot verifying key circuit mismatch"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn create_election_rejects_stark_tally_vk_with_wrong_vote_circuit_role() {
    use core::num::NonZeroU64;

    use iroha_core::{
        kura::Kura, query::store::LiveQueryStore, smartcontracts::Execute, state::State,
    };
    use iroha_data_model::{
        Registrable,
        account::Account,
        block::BlockHeader,
        confidential::ConfidentialStatus,
        domain::Domain,
        isi::{Grant, verifying_keys, zk::CreateElection},
        permission::Permission,
        proof::{VerifyingKeyId, VerifyingKeyRecord},
        zk::BackendTag,
    };
    use iroha_executor_data_model::permission::governance::CanManageParliament;
    use iroha_primitives::json::Json;
    use iroha_test_samples::ALICE_ID;

    let backend = "stark/fri/sha256-goldilocks";
    let ballot_circuit_id = "stark/fri/sha256-goldilocks:vote-ballot";
    let bad_tally_circuit_id = "stark/fri/sha256-goldilocks:not-a-tally-circuit";
    let ballot_schema_hash: [u8; 32] = iroha_crypto::Hash::new(b"gov:vote:ballot:schema:v1").into();
    let tally_schema_hash: [u8; 32] = iroha_crypto::Hash::new(b"gov:vote:tally:schema:v1").into();

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let domain_id: iroha_data_model::domain::DomainId =
        iroha_data_model::domain::DomainId::try_new("wonderland", "universal").expect("domain");
    let domain: Domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
    let account: Account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let mut state = State::new_for_testing(
        iroha_core::state::World::with([domain], [account], Vec::new()),
        kura,
        query,
    );
    state.zk.stark.enabled = true;
    state.zk.halo2.enabled = false;
    state.zk.verify_timeout = std::time::Duration::ZERO;

    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let perm_vk = Permission::new("CanManageVerifyingKeys".to_string(), Json::new(()));
    Grant::account_permission(perm_vk, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant CanManageVerifyingKeys");
    let perm_parliament: Permission = CanManageParliament.into();
    Grant::account_permission(perm_parliament, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant CanManageParliament");

    let ballot_vk_id = VerifyingKeyId::new(backend, "vote_ballot");
    let ballot_vk_box = sample_stark_vk_box(backend, ballot_circuit_id, STARK_HASH_SHA256_V1);
    let mut ballot_vk_record = VerifyingKeyRecord::new(
        1,
        ballot_circuit_id,
        BackendTag::Stark,
        "goldilocks",
        ballot_schema_hash,
        iroha_core::zk::hash_vk(&ballot_vk_box),
    );
    ballot_vk_record.status = ConfidentialStatus::Active;
    ballot_vk_record.gas_schedule_id = Some("sched_ballot".to_owned());
    ballot_vk_record.key = Some(ballot_vk_box);
    verifying_keys::RegisterVerifyingKey {
        id: ballot_vk_id.clone(),
        record: ballot_vk_record,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("register ballot vk");

    let tally_vk_id = VerifyingKeyId::new(backend, "bad_vote_tally");
    let tally_vk_box = sample_stark_vk_box(backend, bad_tally_circuit_id, STARK_HASH_SHA256_V1);
    let mut tally_vk_record = VerifyingKeyRecord::new(
        1,
        bad_tally_circuit_id,
        BackendTag::Stark,
        "goldilocks",
        tally_schema_hash,
        iroha_core::zk::hash_vk(&tally_vk_box),
    );
    tally_vk_record.status = ConfidentialStatus::Active;
    tally_vk_record.gas_schedule_id = Some("sched_bad_tally".to_owned());
    tally_vk_record.key = Some(tally_vk_box);
    verifying_keys::RegisterVerifyingKey {
        id: tally_vk_id.clone(),
        record: tally_vk_record,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("register bad tally vk");

    let err = CreateElection {
        election_id: "stark-vote-role-check-tally".to_owned(),
        options: 2,
        eligible_root: [0x22; 32],
        start_ts: 0,
        end_ts: 0,
        vk_ballot: ballot_vk_id,
        vk_tally: tally_vk_id,
        domain_tag: "gov:ballot:v1".to_owned(),
    }
    .execute(&ALICE_ID, &mut stx)
    .expect_err("create election must reject wrong STARK tally role");
    let err_text = format!("{err:?}");
    assert!(
        err_text.contains("tally verifying key circuit mismatch"),
        "unexpected error: {err:?}"
    );
}

#[test]
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn governance_accepts_halo2_and_rejects_synthetic_stark_ballot() {
    use core::num::NonZeroU64;

    use iroha_core::{
        kura::Kura,
        query::store::LiveQueryStore,
        smartcontracts::Execute,
        state::{State, World, WorldReadOnly},
        zk::test_utils::halo2_fixture_envelope,
    };
    use iroha_data_model::{
        Registrable,
        account::Account,
        block::BlockHeader,
        confidential::ConfidentialStatus,
        domain::Domain,
        isi::{
            Grant, verifying_keys,
            zk::{CreateElection, SubmitBallot},
        },
        permission::Permission,
        proof::{ProofAttachment, ProofBox, VerifyingKeyId, VerifyingKeyRecord},
        zk::BackendTag,
    };
    use iroha_executor_data_model::permission::governance::{
        CanManageParliament, CanSubmitGovernanceBallot,
    };
    use iroha_primitives::json::Json;
    use iroha_test_samples::ALICE_ID;
    use mv::storage::StorageReadOnly;

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let domain_id: iroha_data_model::domain::DomainId =
        iroha_data_model::domain::DomainId::try_new("wonderland", "universal").expect("domain");
    let domain: Domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
    let account: Account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let world = World::with([domain], [account], Vec::new());
    let mut state = State::new_for_testing(world, kura, query);
    state.zk.stark.enabled = true;
    state.zk.halo2.enabled = true;
    state.zk.verify_timeout = std::time::Duration::ZERO;
    state.gov.citizenship_bond_amount = 0;
    state.gov.min_bond_amount = 0;

    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let perm_vk = Permission::new("CanManageVerifyingKeys".to_string(), Json::new(()));
    Grant::account_permission(perm_vk, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant CanManageVerifyingKeys");
    let perm_parliament: Permission = CanManageParliament.into();
    Grant::account_permission(perm_parliament, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant CanManageParliament");

    let halo2_election_id = "mixed-backend-halo2".to_string();
    let stark_election_id = "mixed-backend-stark".to_string();
    let perm_halo2_ballot: Permission = CanSubmitGovernanceBallot {
        referendum_id: halo2_election_id.clone(),
    }
    .into();
    Grant::account_permission(perm_halo2_ballot, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant halo2 ballot permission");
    let perm_stark_ballot: Permission = CanSubmitGovernanceBallot {
        referendum_id: stark_election_id.clone(),
    }
    .into();
    Grant::account_permission(perm_stark_ballot, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant stark ballot permission");

    // Register a Halo2 VK/circuit pair and submit a valid Halo2 ballot.
    let halo2_backend = "halo2/ipa";
    let halo2_circuit_id = "halo2/ipa:tiny-add2inst-public";
    let halo2_vk_id = VerifyingKeyId::new(halo2_backend, "mixed_halo2_ballot");
    let halo2_fixture = halo2_fixture_envelope(halo2_circuit_id, [0u8; 32]);
    let halo2_vk_box = halo2_fixture
        .vk_box(halo2_backend)
        .expect("halo2 fixture must include vk bytes");
    let halo2_vk_hash = halo2_fixture
        .vk_hash(halo2_backend)
        .expect("halo2 fixture must include vk hash");
    let mut halo2_vk_record = VerifyingKeyRecord::new(
        1,
        halo2_circuit_id,
        BackendTag::Halo2IpaPasta,
        "pallas",
        halo2_fixture.schema_hash,
        halo2_vk_hash,
    );
    halo2_vk_record.status = ConfidentialStatus::Active;
    halo2_vk_record.gas_schedule_id = Some("sched_halo2_ballot".to_string());
    halo2_vk_record.key = Some(halo2_vk_box);
    verifying_keys::RegisterVerifyingKey {
        id: halo2_vk_id.clone(),
        record: halo2_vk_record,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("register halo2 ballot vk");

    let mut halo2_commit = [0u8; 32];
    halo2_commit.copy_from_slice(&halo2_fixture.public_inputs[..32]);
    let mut halo2_root = [0u8; 32];
    halo2_root.copy_from_slice(&halo2_fixture.public_inputs[32..64]);
    CreateElection {
        election_id: halo2_election_id.clone(),
        options: 2,
        eligible_root: halo2_root,
        start_ts: 0,
        end_ts: 0,
        vk_ballot: halo2_vk_id.clone(),
        vk_tally: halo2_vk_id.clone(),
        domain_tag: "gov:ballot:v1".to_string(),
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("create halo2 election");

    let halo2_ballot_attachment = ProofAttachment::new_ref(
        halo2_backend.to_string(),
        ProofBox::new(halo2_backend.to_string(), halo2_fixture.proof_bytes.clone()),
        halo2_vk_id.clone(),
    );
    let halo2_nullifier = derive_ballot_nullifier_for_test(
        "gov:ballot:v1",
        &state.chain_id,
        &halo2_election_id,
        &halo2_commit,
    );
    SubmitBallot {
        election_id: halo2_election_id.clone(),
        ciphertext: halo2_commit.to_vec(),
        ballot_proof: halo2_ballot_attachment,
        nullifier: halo2_nullifier,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("submit halo2 ballot");

    // Register a STARK VK/circuit pair and reject a synthetic STARK ballot.
    let stark_backend = "stark/fri/sha256-goldilocks";
    let stark_ballot_circuit_id = "stark/fri/sha256-goldilocks:vote-ballot";
    let stark_tally_circuit_id = "stark/fri/sha256-goldilocks:vote-tally";
    let stark_ballot_vk_id = VerifyingKeyId::new(stark_backend, "mixed_stark_ballot");
    let stark_ballot_vk_box =
        sample_stark_vk_box(stark_backend, stark_ballot_circuit_id, STARK_HASH_SHA256_V1);
    let stark_ballot_vk_hash = iroha_core::zk::hash_vk(&stark_ballot_vk_box);
    let stark_ballot_schema = b"gov:vote:ballot:schema:v1".to_vec();
    let stark_ballot_schema_hash: [u8; 32] = iroha_crypto::Hash::new(&stark_ballot_schema).into();
    let mut stark_ballot_vk_record = VerifyingKeyRecord::new(
        1,
        stark_ballot_circuit_id,
        BackendTag::Stark,
        "goldilocks",
        stark_ballot_schema_hash,
        stark_ballot_vk_hash,
    );
    stark_ballot_vk_record.status = ConfidentialStatus::Active;
    stark_ballot_vk_record.gas_schedule_id = Some("sched_stark_ballot".to_string());
    stark_ballot_vk_record.key = Some(stark_ballot_vk_box.clone());
    verifying_keys::RegisterVerifyingKey {
        id: stark_ballot_vk_id.clone(),
        record: stark_ballot_vk_record,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("register stark ballot vk");

    let stark_tally_vk_id = VerifyingKeyId::new(stark_backend, "mixed_stark_tally");
    let stark_tally_vk_box =
        sample_stark_vk_box(stark_backend, stark_tally_circuit_id, STARK_HASH_SHA256_V1);
    let stark_tally_vk_hash = iroha_core::zk::hash_vk(&stark_tally_vk_box);
    let stark_tally_schema = b"gov:vote:tally:schema:v1".to_vec();
    let stark_tally_schema_hash: [u8; 32] = iroha_crypto::Hash::new(&stark_tally_schema).into();
    let mut stark_tally_vk_record = VerifyingKeyRecord::new(
        1,
        stark_tally_circuit_id,
        BackendTag::Stark,
        "goldilocks",
        stark_tally_schema_hash,
        stark_tally_vk_hash,
    );
    stark_tally_vk_record.status = ConfidentialStatus::Active;
    stark_tally_vk_record.gas_schedule_id = Some("sched_stark_tally".to_string());
    stark_tally_vk_record.key = Some(stark_tally_vk_box);
    verifying_keys::RegisterVerifyingKey {
        id: stark_tally_vk_id.clone(),
        record: stark_tally_vk_record,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("register stark tally vk");

    let stark_commit = [0x11; 32];
    let stark_root = [0x22; 32];
    CreateElection {
        election_id: stark_election_id.clone(),
        options: 2,
        eligible_root: stark_root,
        start_ts: 0,
        end_ts: 0,
        vk_ballot: stark_ballot_vk_id.clone(),
        vk_tally: stark_tally_vk_id,
        domain_tag: "gov:ballot:v1".to_string(),
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("create stark election");

    let stark_ballot_proof_bytes = build_stark_open_verify_envelope_bytes_for_columns(
        stark_backend,
        stark_ballot_circuit_id,
        stark_ballot_vk_hash,
        &stark_ballot_schema,
        vec![vec![stark_commit], vec![stark_root]],
    );
    let stark_ballot_attachment = ProofAttachment::new_ref(
        stark_backend.to_string(),
        ProofBox::new(stark_backend.to_string(), stark_ballot_proof_bytes),
        stark_ballot_vk_id,
    );
    let stark_nullifier = derive_ballot_nullifier_for_test(
        "gov:ballot:v1",
        &state.chain_id,
        &stark_election_id,
        &stark_commit,
    );
    let err = SubmitBallot {
        election_id: stark_election_id.clone(),
        ciphertext: stark_commit.to_vec(),
        ballot_proof: stark_ballot_attachment,
        nullifier: stark_nullifier,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect_err("synthetic STARK ballot must be rejected");
    let err_text = format!("{err:?}");
    assert!(
        err_text.contains("invalid ballot proof"),
        "unexpected stark ballot rejection: {err:?}"
    );

    let halo2_election = stx
        .world
        .elections()
        .get(&halo2_election_id)
        .cloned()
        .expect("halo2 election exists");
    assert_eq!(
        halo2_election.ciphertexts.len(),
        1,
        "halo2 ballot must be accepted"
    );
    let stark_election = stx
        .world
        .elections()
        .get(&stark_election_id)
        .cloned()
        .expect("stark election exists");
    assert_eq!(
        stark_election.ciphertexts.len(),
        0,
        "synthetic stark ballot must be rejected"
    );
}

#[test]
fn stark_envelope_respects_limits() {
    let env = build_sample_air_composition_envelope();
    let bytes = norito::to_bytes(&env).expect("encode");
    assert!(
        verify_stark_fri_envelope(&bytes),
        "default limits should accept the sample envelope"
    );

    let default_limits = StarkVerifierLimits::default();

    // Apply a stricter domain-tag limit to force rejection.
    let mut tight_limits = default_limits;
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

    let mut relaxed_limits = default_limits;
    relaxed_limits.max_domain_tag_len = default_limits.max_domain_tag_len + 1;
    relaxed_limits.max_transcript_label_len = default_limits.max_transcript_label_len + 1;
    relaxed_limits.max_envelope_bytes = default_limits.max_envelope_bytes + 1;

    let oversized_envelope_bytes = vec![0_u8; default_limits.max_envelope_bytes + 1];
    assert!(
        !verify_stark_fri_envelope_with_limits(&oversized_envelope_bytes, &relaxed_limits),
        "raised public limits must not relax the native encoded-envelope byte cap"
    );

    let over_canonical_domain_tag = "d".repeat(default_limits.max_domain_tag_len + 1);
    let err = prove_stark_fri_composition_envelope_bytes(
        sample_air_params(over_canonical_domain_tag.clone(), STARK_HASH_SHA256_V1),
        "TEST-STARK".to_string(),
        7,
        2,
        sample_composition_terms(),
    )
    .expect_err("public STARK prover must reject over-canonical domain tags");
    assert!(
        err.contains("domain tag"),
        "domain-tag rejection should be explicit, got: {err}"
    );

    let over_canonical_transcript_label = "T".repeat(default_limits.max_transcript_label_len + 1);
    let err = prove_stark_fri_composition_envelope_bytes(
        sample_air_params("fastpq:v1:fri".to_string(), STARK_HASH_SHA256_V1),
        over_canonical_transcript_label.clone(),
        7,
        2,
        sample_composition_terms(),
    )
    .expect_err("public STARK prover must reject over-canonical transcript labels");
    assert!(
        err.contains("transcript label"),
        "transcript-label rejection should be explicit, got: {err}"
    );

    let mut env_over_canonical_tag = env.clone();
    env_over_canonical_tag.params.domain_tag = over_canonical_domain_tag;
    let bytes_over_canonical_tag =
        norito::to_bytes(&env_over_canonical_tag).expect("encode over-canonical domain tag");
    assert!(
        !verify_stark_fri_envelope_with_limits(&bytes_over_canonical_tag, &relaxed_limits),
        "raised public limits must not relax the canonical domain-tag cap"
    );

    let mut env_over_canonical_label = env;
    env_over_canonical_label.transcript_label = over_canonical_transcript_label;
    let bytes_over_canonical_label = norito::to_bytes(&env_over_canonical_label)
        .expect("encode over-canonical transcript label");
    assert!(
        !verify_stark_fri_envelope_with_limits(&bytes_over_canonical_label, &relaxed_limits),
        "raised public limits must not relax the canonical transcript-label cap"
    );
}

#[test]
fn stark_single_fold_envelope_golden_vector() {
    let env = build_sample_air_composition_envelope();
    let bytes = norito::to_bytes(&env).expect("encode");
    let hex = hex::encode(bytes);
    expect!["4e5254300000a8d457cce6a10e02a8d457cce6a10e0200a10a0000000000009d6fc6edf58f5ba2021f020100010301030102020100010201010e0d6661737470713a76313a667269f314020100d4020201008c02040000000000000040013c010501780128015b017d01af014501b301bf013f016c017e016e01fd01a2014f01de01b3017201630172012901c30195017a018a01340179019d01a301184001eb017e018001810159015f019201a901da012f01430103011d014901c60169014801cc01bb01a301c301a40199013b013501c70139017e01a301660156013e4001eb0119017a0185013901ac01b60199019701fa017201e9015d0186019101c5019601500190016e010801880118012e01f101ed01af015c018401520146013d4001ae019e017e01bf01bd01f701260148010d01df017c0113011601f1019401b8016701c70144019901ae0141011f017f015e0146013601b1018001f8013b0102420140015d014c0112016f01fc018b01cb0105014a01240177011b015901d0010c01d7015101f6016e016801e7017a0125011701580172014a017b01b701ff019f0144840a0100000000000000fa090300000000000000ea040400000000080000000000000000080000000000000000d70109010000000000000000cb0103000000000000004001ae019e017e01bf01bd01f701260148010d01df017c0113011601f1019401b8016701c70144019901ae0141011f017f015e0146013601b1018001f8013b01024001eb0119017a0185013901ac01b60199019701fa017201e9015d0186019101c5019601500190016e010801880118012e01f101ed01af015c018401520146013d4001eb017e018001810159015f019201a901da012f01430103011d014901c60169014801cc01bb01a301c301a40199013b013501c70139017e01a301660156013ed70109010000000000000001cb0103000000000000004001ae019e017e01bf01bd01f701260148010d01df017c0113011601f1019401b8016701c70144019901ae0141011f017f015e0146013601b1018001f8013b01024001eb0119017a0185013901ac01b60199019701fa017201e9015d0186019101c5019601500190016e010801880118012e01f101ed01af015c018401520146013d4001eb017e018001810159015f019201a901da012f01430103011d014901c60169014801cc01bb01a301c301a40199013b013501c70139017e01a301660156013e0800000000000000009601090100000000000000008a0102000000000000004001ae019e017e01bf01bd01f701260148010d01df017c0113011601f1019401b8016701c70144019901ae0141011f017f015e0146013601b1018001f8013b01024001eb0119017a0185013901ac01b60199019701fa017201e9015d0186019101c5019601500190016e010801880118012e01f101ed01af015c018401520146013da50304000000000800000000000000000800000000000000009601090100000000000000008a0102000000000000004001ae019e017e01bf01bd01f701260148010d01df017c0113011601f1019401b8016701c70144019901ae0141011f017f015e0146013601b1018001f8013b01024001eb0119017a0185013901ac01b60199019701fa017201e9015d0186019101c5019601500190016e010801880118012e01f101ed01af015c018401520146013d9601090100000000000000018a0102000000000000004001ae019e017e01bf01bd01f701260148010d01df017c0113011601f1019401b8016701c70144019901ae0141011f017f015e0146013601b1018001f8013b01024001eb0119017a0185013901ac01b60199019701fa017201e9015d0186019101c5019601500190016e010801880118012e01f101ed01af015c018401520146013d08000000000000000054090100000000000000004901000000000000004001ae019e017e01bf01bd01f701260148010d01df017c0113011601f1019401b8016701c70144019901ae0141011f017f015e0146013601b1018001f8013b0102dd01040000000008000000000000000008000000000000000054090100000000000000004901000000000000004001ae019e017e01bf01bd01f701260148010d01df017c0113011601f1019401b8016701c70144019901ae0141011f017f015e0146013601b1018001f8013b010254090100000000000000014901000000000000004001ae019e017e01bf01bd01f701260148010d01df017c0113011601f1019401b8016701c70144019901ae0141011f017f015e0146013601b1018001f8013b010208000000000000000012080000000000000000080000000000000000720170010000000000000067087d00000000000000080700000000000000080200000000000000380200000000000000170400000000080b00000000000000080300000000000000170401000000081100000000000000080500000000000000120800000000000000000800000000000000009f07019c070201000f0e636f6d706f736974696f6e2d763120c8fdd241c9e2ea9761a6aa6300a15507f7e9cc36bb877728217ebfedf6571f442072629918b773e586de7035b7e57850f3e4aa4785db2eb712edbce53285982922203c0578285b7daf45b3bf3f6c7e6efda24fdeb372637229c3957a8a34799da318020600a1060100000000000000970604010000003e060000000000000008010000000000000008c8fdd241c9e2ea970861a6aa6300a1550708f7e9cc36bb87772808217ebfedf6571f440806000000000000003e060000000000000008020000000000000008c8fdd241c9e2ea970861a6aa6300a1550708f7e9cc36bb87772808217ebfedf6571f44080600000000000000d70109010000000000000001cb0103000000000000004001c00131013d01550174019201ce01ba01de01b601d401f901d8011a013401fd0118013b013301ae016701bf01f101d301590106013601b6010601ff0195015e400178018d01df017a01060183014b0125018a01d8015001ed011b01ee0139013c010501d7016501f401e1014e013b011d01cd014301e3018a013401630144010840015e01240177016b019e01270178015801aa010e010901220186014f01bf01b4013101be015801ef016401a2018f01cd018f0134011a017c018701f301b10168d70109010000000000000002cb010300000000000000400124018201d0017f01a5013001a201b9014a0101013e011c017001b801a40179016f016901180128012d013801d7012b0178016a01f4018701b901b0019a01074001fc0188012701cf01e701cd01350112015d01c1019f019001d2015301ac01b1010401da0144014c01ab010b0151012b01cf0182018e01af01f501de0156016d40015e01240177016b019e01270178015801aa010e010901220186014f01bf01b4013101be015801ef016401a2018f01cd018f0134011a017c018701f301b10168080000000000000000d70109010000000000000001cb0103000000000000004001ae019e017e01bf01bd01f701260148010d01df017c0113011601f1019401b8016701c70144019901ae0141011f017f015e0146013601b1018001f8013b01024001eb0119017a0185013901ac01b60199019701fa017201e9015d0186019101c5019601500190016e010801880118012e01f101ed01af015c018401520146013d4001eb017e018001810159015f019201a901da012f01430103011d014901c60169014801cc01bb01a301c301a40199013b013501c70139017e01a301660156013e0b0a544553542d535441524b"].assert_eq(&hex);
}
