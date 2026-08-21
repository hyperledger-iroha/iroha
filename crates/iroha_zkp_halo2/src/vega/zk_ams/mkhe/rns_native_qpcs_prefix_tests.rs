use super::*;

const PARAMETER_DIGEST_OFFSET_V1: usize = 60;
const TRANSCRIPT_DIGEST_OFFSET_V1: usize = 92;
const SECTION_BINDING_DIGEST_OFFSET_V1: usize = 380;
const EVALUATION_BINDING_DIGEST_OFFSET_V1: usize = 412;
const RESIDUAL_DIGEST_OFFSET_V1: usize = 444;

struct FixtureV1 {
    context: PrefixContextV1,
    queries: [u32; QUERY_COUNT_V1],
    initial_indices: IndexSetV1,
    fri_one_indices: IndexSetV1,
    initial_values: Vec<u8>,
    descriptors: [TreeDescriptorV1; TREE_COUNT_V1],
    prefix: Vec<u8>,
}

fn fixture_digest_v1(label: &[u8], ordinal: usize) -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.rns-native-qpcs.prefix.test-fixture");
    hash.update(
        &u16::try_from(label.len())
            .expect("test label fits u16")
            .to_be_bytes(),
    );
    hash.update(label);
    hash.update(
        &u16::try_from(ordinal)
            .expect("test ordinal fits u16")
            .to_be_bytes(),
    );
    hash.finalize()
}

fn zero_tree_digests_v1(
    parameter_digest: [u8; DIGEST_BYTES_V1],
    role: TreeRoleV1,
    layer: u8,
    length: usize,
) -> [[u8; DIGEST_BYTES_V1]; ZK_AMS_MKHE_RNS_NATIVE_LDE_DOMAIN_LOG2_V1 as usize + 1] {
    let mut digests =
        [[0_u8; DIGEST_BYTES_V1]; ZK_AMS_MKHE_RNS_NATIVE_LDE_DOMAIN_LOG2_V1 as usize + 1];
    digests[0] = tree_leaf_hash_v1(
        parameter_digest,
        role,
        layer,
        length,
        &[0_u8; LEAF_BYTES_V1],
    )
    .expect("zero leaf hash");
    let depth = length.ilog2() as usize;
    for height in 1..=depth {
        digests[height] = tree_node_hash_v1(
            parameter_digest,
            role,
            layer,
            length,
            height,
            digests[height - 1],
            digests[height - 1],
        )
        .expect("zero node hash");
    }
    digests
}

fn build_zero_authentication_v1(
    parameter_digest: [u8; DIGEST_BYTES_V1],
    role: TreeRoleV1,
    layer: u8,
    length: usize,
    indices: IndexSetV1,
) -> (Vec<u8>, [u8; DIGEST_BYTES_V1]) {
    let zero = zero_tree_digests_v1(parameter_digest, role, layer, length);
    let values = vec![0_u8; indices.len * LEAF_BYTES_V1];
    let mut current = [EMPTY_FRONTIER_NODE_V1; MAX_OPENED_LEAVES_V1];
    let mut next = [EMPTY_FRONTIER_NODE_V1; MAX_OPENED_LEAVES_V1];
    for (position, node) in current.iter_mut().enumerate().take(indices.len) {
        let start = position * LEAF_BYTES_V1;
        *node = FrontierNodeV1 {
            index: indices.values[position],
            digest: tree_leaf_hash_v1(
                parameter_digest,
                role,
                layer,
                length,
                &values[start..start + LEAF_BYTES_V1],
            )
            .expect("canonical zero leaf"),
        };
    }
    let mut authentication = Vec::new();
    let mut current_len = indices.len;
    let mut nodes_at_height = length;
    let mut height = 1_usize;
    while nodes_at_height > 1 {
        let mut cursor = 0_usize;
        let mut next_len = 0_usize;
        while cursor < current_len {
            let node = current[cursor];
            let sibling_index = node.index ^ 1;
            let (left, right);
            if node.index.is_multiple_of(2)
                && cursor + 1 < current_len
                && current[cursor + 1].index == sibling_index
            {
                left = node.digest;
                right = current[cursor + 1].digest;
                cursor += 2;
            } else {
                let sibling = zero[height - 1];
                authentication.extend_from_slice(&sibling);
                if node.index.is_multiple_of(2) {
                    left = node.digest;
                    right = sibling;
                } else {
                    left = sibling;
                    right = node.digest;
                }
                cursor += 1;
            }
            next[next_len] = FrontierNodeV1 {
                index: node.index / 2,
                digest: tree_node_hash_v1(
                    parameter_digest,
                    role,
                    layer,
                    length,
                    height,
                    left,
                    right,
                )
                .expect("canonical zero node"),
            };
            next_len += 1;
        }
        current[..next_len].copy_from_slice(&next[..next_len]);
        current_len = next_len;
        nodes_at_height /= 2;
        height += 1;
    }
    assert_eq!(current_len, 1);
    assert_eq!(current[0].index, 0);
    assert_eq!(current[0].digest, zero[length.ilog2() as usize]);
    (authentication, current[0].digest)
}

fn encode_prefix_v1(
    context: PrefixContextV1,
    descriptors: [TreeDescriptorV1; TREE_COUNT_V1],
    evaluations: &[u8],
    trees: [&[u8]; 6],
    residual: &[u8],
) -> Vec<u8> {
    let evaluation_binding =
        evaluation_binding_digest_v1(context, evaluations).expect("evaluation binding");
    let residual_digest = residual_digest_v1(context, residual).expect("residual binding");
    let mut prefix = Vec::new();
    prefix.extend_from_slice(&PREFIX_MAGIC_V1);
    prefix.push(PREFIX_VERSION_V1);
    prefix.push(ZK_AMS_MKHE_RNS_NATIVE_LDE_DOMAIN_LOG2_V1);
    prefix.push(u8::try_from(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1).expect("limbs fit u8"));
    prefix.push(u8::try_from(ROWS_PER_LIMB_V1).expect("rows fit u8"));
    prefix.extend_from_slice(&ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1.to_be_bytes());
    prefix.extend_from_slice(
        &u16::try_from(RELATION_COUNT_V1)
            .expect("relations fit u16")
            .to_be_bytes(),
    );
    prefix.extend_from_slice(
        &u16::try_from(EVALUATION_COUNT_V1)
            .expect("evaluations fit u16")
            .to_be_bytes(),
    );
    prefix.push(u8::try_from(TREE_COUNT_V1).expect("tree count fits u8"));
    prefix.push(CHECKED_FOLD_COUNT_V1);
    prefix.extend_from_slice(
        &u32::try_from(evaluations.len())
            .expect("evaluations fit u32")
            .to_be_bytes(),
    );
    prefix.extend_from_slice(
        &u32::try_from(residual.len())
            .expect("residual fits u32")
            .to_be_bytes(),
    );
    for descriptor in descriptors {
        prefix.extend_from_slice(
            &u16::try_from(descriptor.opened)
                .expect("opened count fits u16")
                .to_be_bytes(),
        );
        prefix.extend_from_slice(
            &u16::try_from(descriptor.authentication)
                .expect("authentication count fits u16")
                .to_be_bytes(),
        );
        prefix.extend_from_slice(
            &u32::try_from(descriptor.values_bytes)
                .expect("values bytes fit u32")
                .to_be_bytes(),
        );
        prefix.extend_from_slice(
            &u32::try_from(descriptor.authentication_bytes)
                .expect("authentication bytes fit u32")
                .to_be_bytes(),
        );
    }
    for digest in [
        context.parameter_digest,
        context.transcript_digest,
        context.rns_aggregation_seed,
        context.relation_seed,
        context.batching_seed,
        context.fold_zero_seed,
        context.query_seed,
        context.quotient_root,
        context.fri_zero_root,
        context.fri_one_root,
        context.section_binding_digest,
        evaluation_binding,
        residual_digest,
    ] {
        prefix.extend_from_slice(&digest);
    }
    assert_eq!(prefix.len(), PREFIX_HEADER_BYTES_V1);
    prefix.extend_from_slice(evaluations);
    for tree in trees {
        prefix.extend_from_slice(tree);
    }
    prefix.extend_from_slice(residual);
    prefix
}

fn fixture_v1() -> FixtureV1 {
    let parameter_digest = fixture_digest_v1(b"parameters", 0);
    let mut queries =
        core::array::from_fn(|ordinal| u32::try_from(ordinal).expect("query fits u32"));
    // Exercise the upper member of a FRI-1 query pair. The fold output for
    // this query must remain at q, not be reduced modulo the next half.
    queries[0] = u32::try_from(FRI_ONE_SIZE_V1 / 2 + 7).expect("upper FRI-1 query fits u32");
    let initial_indices = query_pair_indices_v1(&queries, DOMAIN_SIZE_V1).expect("initial indices");
    let fri_one_indices = query_pair_indices_v1(&queries, FRI_ONE_SIZE_V1).expect("FRI-1 indices");
    let quotient = build_zero_authentication_v1(
        parameter_digest,
        TreeRoleV1::Quotient,
        0,
        DOMAIN_SIZE_V1,
        initial_indices,
    );
    let fri_zero = build_zero_authentication_v1(
        parameter_digest,
        TreeRoleV1::Fri,
        0,
        DOMAIN_SIZE_V1,
        initial_indices,
    );
    let fri_one = build_zero_authentication_v1(
        parameter_digest,
        TreeRoleV1::Fri,
        1,
        FRI_ONE_SIZE_V1,
        fri_one_indices,
    );
    let equation_commitment_digests =
        core::array::from_fn(|ordinal| fixture_digest_v1(b"equation-commitment", ordinal));
    let limb_commitment_digests =
        core::array::from_fn(|ordinal| fixture_digest_v1(b"limb-commitment", ordinal));
    let query_opening_digests: [[u8; DIGEST_BYTES_V1]; QUERY_COUNT_V1] =
        core::array::from_fn(|ordinal| fixture_digest_v1(b"query-opening", ordinal));
    let transcript_digest = fixture_digest_v1(b"transcript", 0);
    let section_binding_digest = section_binding_digest_v1(
        transcript_digest,
        &equation_commitment_digests,
        &limb_commitment_digests,
        &query_opening_digests,
    )
    .expect("section binding");
    let context = PrefixContextV1 {
        parameter_digest,
        transcript_digest,
        rns_aggregation_seed: fixture_digest_v1(b"rns-aggregation-seed", 0),
        relation_seed: fixture_digest_v1(b"relation-seed", 0),
        batching_seed: fixture_digest_v1(b"batching-seed", 0),
        fold_zero_seed: fixture_digest_v1(b"fold-zero-seed", 0),
        query_seed: fixture_digest_v1(b"query-seed", 0),
        quotient_root: quotient.1,
        fri_zero_root: fri_zero.1,
        fri_one_root: fri_one.1,
        section_binding_digest,
        equation_commitment_digests,
        limb_commitment_digests,
    };
    let descriptors = [
        descriptor_for_indices_v1(initial_indices, DOMAIN_SIZE_V1).expect("quotient descriptor"),
        descriptor_for_indices_v1(initial_indices, DOMAIN_SIZE_V1).expect("FRI-0 descriptor"),
        descriptor_for_indices_v1(fri_one_indices, FRI_ONE_SIZE_V1).expect("FRI-1 descriptor"),
    ];
    let initial_values = vec![0_u8; initial_indices.len * LEAF_BYTES_V1];
    let quotient_values = vec![0_u8; descriptors[0].values_bytes];
    let fri_zero_values = vec![0_u8; descriptors[1].values_bytes];
    let fri_one_values = vec![0_u8; descriptors[2].values_bytes];
    let prefix = encode_prefix_v1(
        context,
        descriptors,
        &[0_u8; EVALUATION_BYTES_V1],
        [
            &quotient_values,
            &quotient.0,
            &fri_zero_values,
            &fri_zero.0,
            &fri_one_values,
            &fri_one.0,
        ],
        &[0xa5],
    );
    FixtureV1 {
        context,
        queries,
        initial_indices,
        fri_one_indices,
        initial_values,
        descriptors,
        prefix,
    }
}

fn decoded_fixture_v1(fixture: &FixtureV1) -> PrefixViewV1<'_> {
    decode_prefix_exact_v1(&fixture.prefix, fixture.context, fixture.descriptors)
        .expect("canonical prefix")
}

fn write_value_v1(
    indices: IndexSetV1,
    values: &mut [u8],
    index: u32,
    coordinate: usize,
    value: Fq2V1,
) {
    let position = indices.values[..indices.len]
        .binary_search(&index)
        .expect("opened test index");
    let offset = position * LEAF_BYTES_V1 + coordinate * FQ2_BYTES_V1;
    values[offset..offset + 8].copy_from_slice(&value.c0.to_be_bytes());
    values[offset + 8..offset + 16].copy_from_slice(&value.c1.to_be_bytes());
}

#[test]
fn exact_quotient_batch_and_first_fold_prefix_is_non_authorizing() {
    let fixture = fixture_v1();
    verify_prefix_parts_v1(
        fixture.context,
        &fixture.queries,
        &fixture.initial_indices.values[..fixture.initial_indices.len],
        &fixture.initial_values,
        &fixture.prefix,
    )
    .expect("valid zero qPCS prefix");

    let source = include_str!("rns_native_qpcs_prefix.rs");
    assert!(!source.contains("CandidateReceipt"));
    assert!(!source.contains("release_ready = true"));
    assert!(!source.contains("readiness = true"));
    let composite = include_str!("rns_native_composite_verifier.rs");
    assert!(composite.contains("authenticate_rns_native_qpcs_fri_complete_v1"));
    assert!(composite.contains("retained RLWE/source residual"));
    assert!(composite.contains(
        "StageUnavailable(\n            ZkAmsMkheRnsNativeVerificationStageV1::RnsRelationQpcs"
    ));
}

#[test]
fn quotient_and_fri_roots_paths_roles_and_layers_are_exact() {
    let fixture = fixture_v1();
    let view = decoded_fixture_v1(&fixture);
    authenticate_tree_v1(
        view.quotient,
        fixture.initial_indices,
        DOMAIN_SIZE_V1,
        TreeRoleV1::Quotient,
        0,
        fixture.context.parameter_digest,
        fixture.context.quotient_root,
    )
    .expect("quotient authentication");
    let mut changed_path = view.quotient.authentication.to_vec();
    changed_path[0] ^= 1;
    assert_eq!(
        authenticate_tree_v1(
            TreeViewV1 {
                values: view.quotient.values,
                authentication: &changed_path,
            },
            fixture.initial_indices,
            DOMAIN_SIZE_V1,
            TreeRoleV1::Quotient,
            0,
            fixture.context.parameter_digest,
            fixture.context.quotient_root,
        ),
        Err(RnsNativeQpcsPrefixErrorV1::InvalidMerklePath)
    );
    assert_eq!(
        authenticate_tree_v1(
            view.fri_zero,
            fixture.initial_indices,
            DOMAIN_SIZE_V1,
            TreeRoleV1::Fri,
            1,
            fixture.context.parameter_digest,
            fixture.context.fri_zero_root,
        ),
        Err(RnsNativeQpcsPrefixErrorV1::InvalidMerklePath)
    );
    let mut changed_root = fixture.context.fri_one_root;
    changed_root[0] ^= 1;
    assert_eq!(
        authenticate_tree_v1(
            view.fri_one,
            fixture.fri_one_indices,
            FRI_ONE_SIZE_V1,
            TreeRoleV1::Fri,
            1,
            fixture.context.parameter_digest,
            changed_root,
        ),
        Err(RnsNativeQpcsPrefixErrorV1::InvalidMerklePath)
    );
}

#[test]
fn ordered_evaluations_and_section_metadata_are_digest_bound() {
    let fixture = fixture_v1();
    let aggregation_identity =
        rlwe_aggregation_identity_v1(fixture.context).expect("RLWE aggregation identity");
    let mut changed_evaluation = fixture.prefix.clone();
    changed_evaluation[PREFIX_HEADER_BYTES_V1] = 1;
    assert_eq!(
        decode_prefix_exact_v1(&changed_evaluation, fixture.context, fixture.descriptors)
            .map(|_| ()),
        Err(RnsNativeQpcsPrefixErrorV1::InvalidHeader)
    );

    let mut changed_context = fixture.context;
    changed_context.equation_commitment_digests.swap(0, 1);
    assert_ne!(
        aggregation_identity,
        rlwe_aggregation_identity_v1(changed_context).expect("swapped aggregation identity")
    );
    changed_context.section_binding_digest = fixture_digest_v1(b"wrong-section-binding", 0);
    assert_eq!(
        decode_prefix_exact_v1(&fixture.prefix, changed_context, fixture.descriptors).map(|_| ()),
        Err(RnsNativeQpcsPrefixErrorV1::InvalidHeader)
    );

    let mut changed_header = fixture.prefix.clone();
    changed_header[SECTION_BINDING_DIGEST_OFFSET_V1] ^= 1;
    assert_eq!(
        decode_prefix_exact_v1(&changed_header, fixture.context, fixture.descriptors).map(|_| ()),
        Err(RnsNativeQpcsPrefixErrorV1::InvalidHeader)
    );
}

#[test]
fn relation_opening_and_batch_equations_reject_mutations() {
    let fixture = fixture_v1();
    let view = decoded_fixture_v1(&fixture);
    let mut bad_relation = view.evaluations.to_vec();
    bad_relation[7] = 1;
    assert_eq!(
        verify_relations_openings_and_batch_v1(
            fixture.context,
            &fixture.initial_indices.values[..fixture.initial_indices.len],
            &fixture.initial_values,
            fixture.initial_indices,
            view.quotient.values,
            fixture.initial_indices,
            view.fri_zero.values,
            &bad_relation,
        ),
        Err(RnsNativeQpcsPrefixErrorV1::InvalidRelation)
    );

    let mut bad_quotient = view.quotient.values.to_vec();
    write_value_v1(
        fixture.initial_indices,
        &mut bad_quotient,
        fixture.initial_indices.values[0],
        0,
        Fq2V1::ONE,
    );
    assert_eq!(
        verify_relations_openings_and_batch_v1(
            fixture.context,
            &fixture.initial_indices.values[..fixture.initial_indices.len],
            &fixture.initial_values,
            fixture.initial_indices,
            &bad_quotient,
            fixture.initial_indices,
            view.fri_zero.values,
            view.evaluations,
        ),
        Err(RnsNativeQpcsPrefixErrorV1::InvalidOpeningQuotient)
    );

    let mut bad_batch = view.fri_zero.values.to_vec();
    write_value_v1(
        fixture.initial_indices,
        &mut bad_batch,
        fixture.initial_indices.values[0],
        0,
        Fq2V1::ONE,
    );
    assert_eq!(
        verify_relations_openings_and_batch_v1(
            fixture.context,
            &fixture.initial_indices.values[..fixture.initial_indices.len],
            &fixture.initial_values,
            fixture.initial_indices,
            view.quotient.values,
            fixture.initial_indices,
            &bad_batch,
            view.evaluations,
        ),
        Err(RnsNativeQpcsPrefixErrorV1::InvalidBatchEquation)
    );
}

#[test]
fn first_fold_checks_direction_coset_sign_and_both_values() {
    let fixture = fixture_v1();
    let view = decoded_fixture_v1(&fixture);
    let mut fri_zero = view.fri_zero.values.to_vec();
    let mut fri_one = view.fri_one.values.to_vec();
    let base = fixture.queries[0];
    let fri_one_half = u32::try_from(FRI_ONE_SIZE_V1 / 2).expect("FRI-1 half fits u32");
    assert!(
        (fri_one_half..u32::try_from(FRI_ONE_SIZE_V1).expect("FRI-1 size fits u32"))
            .contains(&base)
    );
    assert!(fixture.fri_one_indices.values[..fixture.fri_one_indices.len].contains(&base));
    assert!(
        fixture.fri_one_indices.values[..fixture.fri_one_indices.len]
            .contains(&(base % fri_one_half))
    );
    let paired = base + u32::try_from(DOMAIN_SIZE_V1 / 2).expect("half fits u32");
    let positive = Fq2V1 { c0: 1, c1: 2 };
    let negative = Fq2V1 { c0: 3, c1: 4 };
    write_value_v1(fixture.initial_indices, &mut fri_zero, base, 0, positive);
    write_value_v1(fixture.initial_indices, &mut fri_zero, paired, 0, negative);
    let field = Fq2ParametersV1::derive(ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[0])
        .expect("canonical first field");
    let alpha = derive_fq2_challenge_v1(
        FOLD_CHALLENGE_DOMAIN_V1,
        fixture.context.parameter_digest,
        fixture.context.fold_zero_seed,
        0,
        0,
        0,
        field.modulus,
    )
    .expect("fold challenge");
    let x = field.pow(field.domain_root, u128::from(base));
    let next = fold_value_v1(field, x, positive, negative, alpha).expect("first fold value");
    write_value_v1(fixture.fri_one_indices, &mut fri_one, base, 0, next);
    verify_first_fold_v1(
        fixture.context,
        &fixture.queries,
        fixture.initial_indices,
        &fri_zero,
        fixture.fri_one_indices,
        &fri_one,
    )
    .expect("directed first fold");

    write_value_v1(fixture.initial_indices, &mut fri_zero, base, 0, negative);
    write_value_v1(fixture.initial_indices, &mut fri_zero, paired, 0, positive);
    assert_eq!(
        verify_first_fold_v1(
            fixture.context,
            &fixture.queries,
            fixture.initial_indices,
            &fri_zero,
            fixture.fri_one_indices,
            &fri_one,
        ),
        Err(RnsNativeQpcsPrefixErrorV1::InvalidFriEquation)
    );
}

#[test]
fn relation_points_exclude_zero_denominators_and_domain_points() {
    let fixture = fixture_v1();
    let points = derive_relation_points_v1(fixture.context).expect("relation points");
    for (limb, &modulus) in ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1.iter().enumerate() {
        let limb_points = &points[limb * REPETITIONS_V1..(limb + 1) * REPETITIONS_V1];
        for (ordinal, &point) in limb_points.iter().enumerate() {
            assert_ne!(point, 0);
            assert!(!limb_points[..ordinal].contains(&point));
            assert_ne!(mod_pow_v1(point, DOMAIN_SIZE_V1 as u64, modulus), 1);
            assert_ne!(
                mod_add_v1(
                    mod_pow_v1(point, ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 as u64, modulus,),
                    1,
                    modulus,
                ),
                0
            );
        }
    }
}

#[test]
fn reduced_fri_indices_are_canonical_and_deduplicate_collisions() {
    let mut queries =
        core::array::from_fn(|ordinal| u32::try_from(ordinal + 1).expect("query ordinal fits u32"));
    queries[0] = 0;
    queries[1] = u32::try_from(FRI_ONE_SIZE_V1 / 2).expect("FRI quarter fits u32");
    let initial = query_pair_indices_v1(&queries, DOMAIN_SIZE_V1).expect("initial pairs");
    let reduced = query_pair_indices_v1(&queries, FRI_ONE_SIZE_V1).expect("reduced pairs");
    assert_eq!(initial.len, MAX_OPENED_LEAVES_V1);
    assert!(reduced.len < MAX_OPENED_LEAVES_V1);
    assert!(
        reduced.values[..reduced.len]
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    );
    descriptor_for_indices_v1(reduced, FRI_ONE_SIZE_V1).expect("bounded reduced multiproof");
}

#[test]
fn caps_lengths_trailing_noncanonical_context_and_residual_fail_closed() {
    let fixture = fixture_v1();
    assert_eq!(
        preflight_prefix_v1(&vec![
            0_u8;
            ZK_AMS_MKHE_RNS_NATIVE_QPCS_MAX_BYTES_V1 as usize + 1
        ]),
        Err(RnsNativeQpcsPrefixErrorV1::ProofCapExceeded)
    );
    let mut trailing = fixture.prefix.clone();
    trailing.push(0);
    assert_eq!(
        decode_prefix_exact_v1(&trailing, fixture.context, fixture.descriptors).map(|_| ()),
        Err(RnsNativeQpcsPrefixErrorV1::TrailingBytes)
    );
    let mut truncated = fixture.prefix.clone();
    truncated.pop();
    assert_eq!(
        decode_prefix_exact_v1(&truncated, fixture.context, fixture.descriptors).map(|_| ()),
        Err(RnsNativeQpcsPrefixErrorV1::Truncated)
    );
    let mut bad_residual = fixture.prefix.clone();
    let last = bad_residual.len() - 1;
    bad_residual[last] ^= 1;
    assert_eq!(
        decode_prefix_exact_v1(&bad_residual, fixture.context, fixture.descriptors).map(|_| ()),
        Err(RnsNativeQpcsPrefixErrorV1::InvalidHeader)
    );
    let mut changed_context = fixture.context;
    changed_context.transcript_digest[0] ^= 1;
    assert_eq!(
        decode_prefix_exact_v1(&fixture.prefix, changed_context, fixture.descriptors).map(|_| ()),
        Err(RnsNativeQpcsPrefixErrorV1::InvalidHeader)
    );
    let view = decoded_fixture_v1(&fixture);
    let mut noncanonical = view.quotient.values.to_vec();
    noncanonical[..8].copy_from_slice(&ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[0].to_be_bytes());
    assert_eq!(
        validate_leaf_values_v1(&noncanonical, fixture.initial_indices.len),
        Err(RnsNativeQpcsPrefixErrorV1::NonCanonicalResidue)
    );

    assert_eq!(
        &fixture.prefix[PARAMETER_DIGEST_OFFSET_V1..PARAMETER_DIGEST_OFFSET_V1 + 32],
        fixture.context.parameter_digest.as_slice()
    );
    assert_eq!(
        &fixture.prefix[TRANSCRIPT_DIGEST_OFFSET_V1..TRANSCRIPT_DIGEST_OFFSET_V1 + 32],
        fixture.context.transcript_digest.as_slice()
    );
    assert_ne!(
        &fixture.prefix
            [EVALUATION_BINDING_DIGEST_OFFSET_V1..EVALUATION_BINDING_DIGEST_OFFSET_V1 + 32],
        &[0_u8; 32]
    );
    assert_ne!(
        &fixture.prefix[RESIDUAL_DIGEST_OFFSET_V1..RESIDUAL_DIGEST_OFFSET_V1 + 32],
        &[0_u8; 32]
    );
}
