use super::super::rns_native_qpcs_prefix::{FQ2_BYTES_V1, tree_leaf_hash_v1, tree_node_hash_v1};
use super::*;
use std::sync::OnceLock;

const AGGREGATE_OPENED_OFFSET_V1: usize = 16;
const AGGREGATE_AUTHENTICATION_OFFSET_V1: usize = 18;
const AGGREGATE_VALUES_BYTES_OFFSET_V1: usize = 20;
const AGGREGATE_AUTHENTICATION_BYTES_OFFSET_V1: usize = 24;
const DOWNSTREAM_BYTES_OFFSET_V1: usize = 28;
const DESCRIPTORS_OFFSET_V1: usize = 32;
const SCHEDULE_DIGEST_OFFSET_V1: usize = 352;

#[derive(Clone, Copy)]
struct TestNodeV1 {
    index: u32,
    digest: [u8; DIGEST_BYTES_V1],
}

struct ClosureFixtureV1 {
    context: FriClosureContextV1,
    queries: [u32; QUERY_COUNT_V1],
    fri_one_indices: IndexSetV1,
    fri_one_values: Vec<u8>,
    shape: ClosureShapeV1,
    closure: Vec<u8>,
    layer_offsets: [(usize, usize); ENCODED_LAYER_COUNT_V1],
    residual: Vec<u8>,
}

fn fixture_digest_v1(label: &[u8], ordinal: usize) -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.rns-native-qpcs.fri-complete.test");
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
    layer: usize,
    length: usize,
) -> Vec<[u8; DIGEST_BYTES_V1]> {
    let layer = u8::try_from(layer).expect("test layer fits u8");
    let mut digests = Vec::with_capacity(length.ilog2() as usize + 1);
    digests.push(
        tree_leaf_hash_v1(
            parameter_digest,
            TreeRoleV1::Fri,
            layer,
            length,
            &[0_u8; LEAF_BYTES_V1],
        )
        .expect("zero leaf hash"),
    );
    for height in 1..=length.ilog2() as usize {
        digests.push(
            tree_node_hash_v1(
                parameter_digest,
                TreeRoleV1::Fri,
                layer,
                length,
                height,
                digests[height - 1],
                digests[height - 1],
            )
            .expect("zero node hash"),
        );
    }
    digests
}

fn zero_tree_root_v1(
    parameter_digest: [u8; DIGEST_BYTES_V1],
    layer: usize,
    length: usize,
) -> [u8; DIGEST_BYTES_V1] {
    *zero_tree_digests_v1(parameter_digest, layer, length)
        .last()
        .expect("nonempty zero-tree digest schedule")
}

fn build_zero_tree_v1(
    parameter_digest: [u8; DIGEST_BYTES_V1],
    layer: usize,
    length: usize,
    indices: IndexSetV1,
) -> (Vec<u8>, Vec<u8>, [u8; DIGEST_BYTES_V1]) {
    let layer_u8 = u8::try_from(layer).expect("test layer fits u8");
    let zero = zero_tree_digests_v1(parameter_digest, layer, length);
    let values = vec![0_u8; indices.len * LEAF_BYTES_V1];
    let leaf_digest = zero[0];
    let mut current: Vec<TestNodeV1> = indices.values[..indices.len]
        .iter()
        .copied()
        .map(|index| TestNodeV1 {
            index,
            digest: leaf_digest,
        })
        .collect();
    let mut authentication = Vec::new();
    let mut nodes_at_height = length;
    let mut height = 1_usize;
    while nodes_at_height > 1 {
        let mut next = Vec::with_capacity(current.len().div_ceil(2));
        let mut cursor = 0_usize;
        while cursor < current.len() {
            let node = current[cursor];
            let sibling_index = node.index ^ 1;
            let (left, right);
            if node.index.is_multiple_of(2)
                && cursor + 1 < current.len()
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
            next.push(TestNodeV1 {
                index: node.index / 2,
                digest: tree_node_hash_v1(
                    parameter_digest,
                    TreeRoleV1::Fri,
                    layer_u8,
                    length,
                    height,
                    left,
                    right,
                )
                .expect("zero-tree node hash"),
            });
        }
        current = next;
        nodes_at_height /= 2;
        height += 1;
    }
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].index, 0);
    assert_eq!(current[0].digest, zero[length.ilog2() as usize]);
    (values, authentication, current[0].digest)
}

fn encode_closure_v1(
    context: FriClosureContextV1,
    shape: ClosureShapeV1,
    trees: &[(Vec<u8>, Vec<u8>); ENCODED_LAYER_COUNT_V1],
    residual: &[u8],
) -> (Vec<u8>, [(usize, usize); ENCODED_LAYER_COUNT_V1]) {
    let mut closure = Vec::new();
    closure.extend_from_slice(&CLOSURE_MAGIC_V1);
    closure.push(CLOSURE_VERSION_V1);
    closure.push(ZK_AMS_MKHE_RNS_NATIVE_LDE_DOMAIN_LOG2_V1);
    closure.push(u8::try_from(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1).expect("limbs fit u8"));
    closure.push(u8::try_from(ROWS_PER_LIMB_V1).expect("rows fit u8"));
    closure.extend_from_slice(&ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1.to_be_bytes());
    closure.push(u8::try_from(FIRST_ENCODED_LAYER_V1).expect("first layer fits u8"));
    closure.push(u8::try_from(LAST_LAYER_V1).expect("last layer fits u8"));
    closure.push(u8::try_from(ENCODED_LAYER_COUNT_V1).expect("layer count fits u8"));
    closure.push(FIRST_CHECKED_FOLD_V1);
    closure.push(LAST_CHECKED_FOLD_V1);
    closure.push(TERMINAL_DERIVED_V1);
    closure.extend_from_slice(
        &u16::try_from(shape.aggregate_opened)
            .expect("aggregate opened count fits u16")
            .to_be_bytes(),
    );
    closure.extend_from_slice(
        &u16::try_from(shape.aggregate_authentication)
            .expect("aggregate authentication count fits u16")
            .to_be_bytes(),
    );
    closure.extend_from_slice(
        &u32::try_from(shape.aggregate_values_bytes)
            .expect("aggregate value bytes fit u32")
            .to_be_bytes(),
    );
    closure.extend_from_slice(
        &u32::try_from(shape.aggregate_authentication_bytes)
            .expect("aggregate authentication bytes fit u32")
            .to_be_bytes(),
    );
    closure.extend_from_slice(
        &u32::try_from(residual.len())
            .expect("residual bytes fit u32")
            .to_be_bytes(),
    );
    for descriptor in shape.descriptors {
        closure.extend_from_slice(
            &u16::try_from(descriptor.opened)
                .expect("opened count fits u16")
                .to_be_bytes(),
        );
        closure.extend_from_slice(
            &u16::try_from(descriptor.authentication)
                .expect("authentication count fits u16")
                .to_be_bytes(),
        );
        closure.extend_from_slice(
            &u32::try_from(descriptor.values_bytes)
                .expect("value bytes fit u32")
                .to_be_bytes(),
        );
        closure.extend_from_slice(
            &u32::try_from(descriptor.authentication_bytes)
                .expect("authentication bytes fit u32")
                .to_be_bytes(),
        );
    }
    for digest in [
        context.parameter_digest,
        context.transcript_digest,
        context.query_seed,
        context.section_binding_digest,
        context.schedule_digest,
        residual_digest_v1(context, residual).expect("residual digest"),
    ] {
        closure.extend_from_slice(&digest);
    }
    assert_eq!(closure.len(), CLOSURE_HEADER_BYTES_V1);
    let mut offsets = [(0_usize, 0_usize); ENCODED_LAYER_COUNT_V1];
    for (ordinal, (values, authentication)) in trees.iter().enumerate() {
        offsets[ordinal].0 = closure.len();
        closure.extend_from_slice(values);
        offsets[ordinal].1 = closure.len();
        closure.extend_from_slice(authentication);
    }
    closure.extend_from_slice(residual);
    (closure, offsets)
}

fn build_fixture_v1() -> ClosureFixtureV1 {
    let parameter_digest = fixture_digest_v1(b"parameters", 0);
    let queries = core::array::from_fn(|ordinal| {
        u32::try_from(ordinal * 1_021).expect("test query fits u32")
    });
    let roots = core::array::from_fn(|layer| {
        zero_tree_root_v1(parameter_digest, layer, DOMAIN_SIZE_V1 >> layer)
    });
    let fold_seeds = core::array::from_fn(|layer| fixture_digest_v1(b"fold-seed", layer));
    let mut context = FriClosureContextV1 {
        parameter_digest,
        transcript_digest: fixture_digest_v1(b"transcript", 0),
        query_seed: fixture_digest_v1(b"query-seed", 0),
        section_binding_digest: fixture_digest_v1(b"section-binding", 0),
        roots,
        fold_seeds,
        schedule_digest: [0; DIGEST_BYTES_V1],
    };
    context.schedule_digest = schedule_digest_v1(context).expect("FRI schedule digest");
    let shape = closure_shape_v1(&queries).expect("canonical full FRI shape");
    let trees: [(Vec<u8>, Vec<u8>); ENCODED_LAYER_COUNT_V1] = core::array::from_fn(|ordinal| {
        let layer = FIRST_ENCODED_LAYER_V1 + ordinal;
        let (values, authentication, root) = build_zero_tree_v1(
            parameter_digest,
            layer,
            DOMAIN_SIZE_V1 >> layer,
            shape.indices[ordinal],
        );
        assert_eq!(root, context.roots[layer]);
        assert_eq!(values.len(), shape.descriptors[ordinal].values_bytes);
        assert_eq!(
            authentication.len(),
            shape.descriptors[ordinal].authentication_bytes
        );
        (values, authentication)
    });
    let fri_one_indices =
        query_pair_indices_v1(&queries, DOMAIN_SIZE_V1 / 2).expect("FRI-1 indices");
    let fri_one_values = vec![0_u8; fri_one_indices.len * LEAF_BYTES_V1];
    let residual = vec![0xa5, 0x5a, 0x3c];
    let (closure, layer_offsets) = encode_closure_v1(context, shape, &trees, &residual);
    assert!(closure.len() <= ZK_AMS_MKHE_RNS_NATIVE_QPCS_MAX_BYTES_V1 as usize);
    ClosureFixtureV1 {
        context,
        queries,
        fri_one_indices,
        fri_one_values,
        shape,
        closure,
        layer_offsets,
        residual,
    }
}

fn fixture_v1() -> &'static ClosureFixtureV1 {
    static FIXTURE: OnceLock<ClosureFixtureV1> = OnceLock::new();
    FIXTURE.get_or_init(build_fixture_v1)
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
    values[offset + 8..offset + FQ2_BYTES_V1].copy_from_slice(&value.c1.to_be_bytes());
}

#[test]
fn complete_fri_closure_authenticates_every_layer_and_remains_non_authorizing() {
    let fixture = fixture_v1();
    let stage = verify_closure_parts_v1(
        fixture.context,
        &fixture.queries,
        fixture.fri_one_indices,
        &fixture.fri_one_values,
        &fixture.closure,
    )
    .expect("valid all-zero correlated FRI codeword");
    assert_eq!(stage.parameter_digest(), fixture.context.parameter_digest);
    assert_eq!(stage.transcript_digest(), fixture.context.transcript_digest);
    assert_eq!(stage.query_seed(), fixture.context.query_seed);
    assert_eq!(
        stage.section_binding_digest(),
        fixture.context.section_binding_digest
    );
    assert_eq!(stage.schedule_digest(), fixture.context.schedule_digest);
    assert_eq!(
        stage.residual_digest(),
        residual_digest_v1(fixture.context, &fixture.residual).expect("residual digest")
    );
    assert_eq!(stage.rlwe_source_residual(), fixture.residual.as_slice());
    assert!(fixture.shape.aggregate_opened <= MAX_FRI_OPENED_LEAVES_V1);
    assert!(fixture.shape.aggregate_authentication <= MAX_FRI_AUTHENTICATION_HASHES_V1);
    assert!(
        fixture.shape.aggregate_values_bytes + fixture.shape.aggregate_authentication_bytes
            <= ZK_AMS_MKHE_RNS_NATIVE_CORRELATED_FRI_MAX_BYTES_V1 as usize
    );

    let source = include_str!("rns_native_qpcs_fri_complete.rs");
    assert!(!source.contains("CandidateReceipt"));
    assert!(!source.contains("release_ready = true"));
    assert!(!source.contains("readiness = true"));
    assert!(!source.contains("terminal_bytes"));
    let composite = include_str!("rns_native_composite_verifier.rs");
    assert!(composite.contains("authenticate_rns_native_qpcs_fri_complete_v1"));
    assert!(composite.contains("retained RLWE/source residual"));
    assert!(composite.contains(
        "StageUnavailable(\n            ZkAmsMkheRnsNativeVerificationStageV1::RnsRelationQpcs"
    ));
}

#[test]
fn closure_codec_rejects_caps_counts_truncation_trailing_and_context_splices() {
    let fixture = fixture_v1();
    decode_closure_exact_v1(&fixture.closure, fixture.context, fixture.shape)
        .expect("canonical closure");

    assert_eq!(
        decode_closure_exact_v1(
            &fixture.closure[..fixture.closure.len() - 1],
            fixture.context,
            fixture.shape,
        )
        .err(),
        Some(RnsNativeQpcsFriCompleteErrorV1::Truncated)
    );
    let mut trailing = fixture.closure.clone();
    trailing.push(0);
    assert_eq!(
        decode_closure_exact_v1(&trailing, fixture.context, fixture.shape).err(),
        Some(RnsNativeQpcsFriCompleteErrorV1::TrailingBytes)
    );
    let over_cap = vec![0_u8; ZK_AMS_MKHE_RNS_NATIVE_QPCS_MAX_BYTES_V1 as usize + 1];
    assert_eq!(
        decode_closure_exact_v1(&over_cap, fixture.context, fixture.shape).err(),
        Some(RnsNativeQpcsFriCompleteErrorV1::ProofCapExceeded)
    );

    for offset in [
        AGGREGATE_OPENED_OFFSET_V1,
        AGGREGATE_AUTHENTICATION_OFFSET_V1,
        AGGREGATE_VALUES_BYTES_OFFSET_V1,
        AGGREGATE_AUTHENTICATION_BYTES_OFFSET_V1,
        DESCRIPTORS_OFFSET_V1,
        SCHEDULE_DIGEST_OFFSET_V1,
    ] {
        let mut changed = fixture.closure.clone();
        changed[offset] ^= 1;
        assert_eq!(
            decode_closure_exact_v1(&changed, fixture.context, fixture.shape).err(),
            Some(RnsNativeQpcsFriCompleteErrorV1::InvalidHeader)
        );
    }
    let mut empty_residual = fixture.closure.clone();
    empty_residual[DOWNSTREAM_BYTES_OFFSET_V1..DOWNSTREAM_BYTES_OFFSET_V1 + 4]
        .copy_from_slice(&0_u32.to_be_bytes());
    assert_eq!(
        decode_closure_exact_v1(&empty_residual, fixture.context, fixture.shape).err(),
        Some(RnsNativeQpcsFriCompleteErrorV1::InvalidHeader)
    );
    let mut overflow_residual = fixture.closure.clone();
    overflow_residual[DOWNSTREAM_BYTES_OFFSET_V1..DOWNSTREAM_BYTES_OFFSET_V1 + 4]
        .copy_from_slice(&u32::MAX.to_be_bytes());
    assert_eq!(
        decode_closure_exact_v1(&overflow_residual, fixture.context, fixture.shape).err(),
        Some(RnsNativeQpcsFriCompleteErrorV1::ProofCapExceeded)
    );
    let mut changed_residual = fixture.closure.clone();
    *changed_residual.last_mut().expect("nonempty closure") ^= 1;
    assert_eq!(
        decode_closure_exact_v1(&changed_residual, fixture.context, fixture.shape).err(),
        Some(RnsNativeQpcsFriCompleteErrorV1::InvalidHeader)
    );

    let mut changed_context = fixture.context;
    changed_context.roots.swap(4, 5);
    changed_context.fold_seeds.swap(7, 8);
    changed_context.schedule_digest =
        schedule_digest_v1(changed_context).expect("changed schedule digest");
    assert_eq!(
        decode_closure_exact_v1(&fixture.closure, changed_context, fixture.shape).err(),
        Some(RnsNativeQpcsFriCompleteErrorV1::InvalidHeader)
    );
}

#[test]
fn every_encoded_tree_rejects_path_root_layer_order_and_noncanonical_leaves() {
    let fixture = fixture_v1();
    let view = decode_closure_exact_v1(&fixture.closure, fixture.context, fixture.shape)
        .expect("canonical closure");
    for ordinal in 0..ENCODED_LAYER_COUNT_V1 {
        let layer = FIRST_ENCODED_LAYER_V1 + ordinal;
        authenticate_tree_v1(
            view.layers[ordinal],
            fixture.shape.indices[ordinal],
            DOMAIN_SIZE_V1 >> layer,
            TreeRoleV1::Fri,
            u8::try_from(layer).expect("layer fits u8"),
            fixture.context.parameter_digest,
            fixture.context.roots[layer],
        )
        .expect("canonical layer authentication");
    }

    for ordinal in [0, 4, ENCODED_LAYER_COUNT_V1 / 2] {
        let layer = FIRST_ENCODED_LAYER_V1 + ordinal;
        let tree = view.layers[ordinal];
        assert!(!tree.authentication.is_empty());
        let mut changed_authentication = tree.authentication.to_vec();
        changed_authentication[0] ^= 1;
        assert_eq!(
            authenticate_tree_v1(
                TreeViewV1 {
                    values: tree.values,
                    authentication: &changed_authentication,
                },
                fixture.shape.indices[ordinal],
                DOMAIN_SIZE_V1 >> layer,
                TreeRoleV1::Fri,
                u8::try_from(layer).expect("layer fits u8"),
                fixture.context.parameter_digest,
                fixture.context.roots[layer],
            ),
            Err(
                super::super::rns_native_qpcs_prefix::RnsNativeQpcsPrefixErrorV1::InvalidMerklePath
            )
        );
    }

    let ordinal = 3;
    let layer = FIRST_ENCODED_LAYER_V1 + ordinal;
    let tree = view.layers[ordinal];
    let mut wrong_root = fixture.context.roots[layer];
    wrong_root[0] ^= 1;
    assert!(
        authenticate_tree_v1(
            tree,
            fixture.shape.indices[ordinal],
            DOMAIN_SIZE_V1 >> layer,
            TreeRoleV1::Fri,
            u8::try_from(layer).expect("layer fits u8"),
            fixture.context.parameter_digest,
            wrong_root,
        )
        .is_err()
    );
    assert!(
        authenticate_tree_v1(
            tree,
            fixture.shape.indices[ordinal],
            DOMAIN_SIZE_V1 >> layer,
            TreeRoleV1::Fri,
            u8::try_from(layer + 1).expect("changed layer fits u8"),
            fixture.context.parameter_digest,
            fixture.context.roots[layer],
        )
        .is_err()
    );
    assert!(
        authenticate_tree_v1(
            view.layers[ordinal + 1],
            fixture.shape.indices[ordinal],
            DOMAIN_SIZE_V1 >> layer,
            TreeRoleV1::Fri,
            u8::try_from(layer).expect("layer fits u8"),
            fixture.context.parameter_digest,
            fixture.context.roots[layer],
        )
        .is_err()
    );

    let final_ordinal = ENCODED_LAYER_COUNT_V1 - 1;
    let mut changed_final_values = view.layers[final_ordinal].values.to_vec();
    write_value_v1(
        fixture.shape.indices[final_ordinal],
        &mut changed_final_values,
        0,
        0,
        Fq2V1::ONE,
    );
    assert!(
        authenticate_tree_v1(
            TreeViewV1 {
                values: &changed_final_values,
                authentication: view.layers[final_ordinal].authentication,
            },
            fixture.shape.indices[final_ordinal],
            4,
            TreeRoleV1::Fri,
            u8::try_from(LAST_LAYER_V1).expect("last layer fits u8"),
            fixture.context.parameter_digest,
            fixture.context.roots[LAST_LAYER_V1],
        )
        .is_err()
    );

    let mut changed_path = fixture.closure.clone();
    changed_path[fixture.layer_offsets[0].1] ^= 1;
    assert_eq!(
        verify_closure_parts_v1(
            fixture.context,
            &fixture.queries,
            fixture.fri_one_indices,
            &fixture.fri_one_values,
            &changed_path,
        )
        .err(),
        Some(RnsNativeQpcsFriCompleteErrorV1::InvalidMerklePath)
    );
    drop(changed_path);

    let mut noncanonical = fixture.closure.clone();
    let modulus = derive_fields_v1().expect("fields")[0].modulus;
    let values_offset = fixture.layer_offsets[0].0;
    noncanonical[values_offset..values_offset + 8].copy_from_slice(&modulus.to_be_bytes());
    assert_eq!(
        verify_closure_parts_v1(
            fixture.context,
            &fixture.queries,
            fixture.fri_one_indices,
            &fixture.fri_one_values,
            &noncanonical,
        )
        .err(),
        Some(RnsNativeQpcsFriCompleteErrorV1::NonCanonicalResidue)
    );
}

#[test]
fn folds_one_through_sixteen_bind_both_values_and_the_upper_output_member() {
    let fixture = fixture_v1();
    let fields = derive_fields_v1().expect("canonical fields");
    for layer in [1_usize, 8, 16] {
        let length = DOMAIN_SIZE_V1 >> layer;
        let quarter = u32::try_from(length / 4).expect("quarter length fits u32");
        let query = quarter + 1;
        let queries = [query; QUERY_COUNT_V1];
        let current_indices = query_pair_indices_v1(&queries, length).expect("current indices");
        let next_indices = query_pair_indices_v1(&queries, length / 2).expect("next indices");
        let mut current_values = vec![0_u8; current_indices.len * LEAF_BYTES_V1];
        let mut next_values = vec![0_u8; next_indices.len * LEAF_BYTES_V1];
        write_value_v1(current_indices, &mut current_values, query, 0, Fq2V1::ONE);
        write_value_v1(
            current_indices,
            &mut current_values,
            query + u32::try_from(length / 2).expect("half length fits u32"),
            0,
            Fq2V1::ONE,
        );
        // Equal source values fold to one. The output remains at `query`,
        // which is the upper member of the next layer's opened pair.
        write_value_v1(next_indices, &mut next_values, query, 0, Fq2V1::ONE);
        verify_fold_v1(
            fixture.context,
            &fields,
            &queries,
            layer,
            current_indices,
            &current_values,
            next_indices,
            &next_values,
        )
        .expect("upper-member fold output");

        write_value_v1(next_indices, &mut next_values, query, 0, Fq2V1::ZERO);
        write_value_v1(next_indices, &mut next_values, 1, 0, Fq2V1::ONE);
        assert_eq!(
            verify_fold_v1(
                fixture.context,
                &fields,
                &queries,
                layer,
                current_indices,
                &current_values,
                next_indices,
                &next_values,
            ),
            Err(RnsNativeQpcsFriCompleteErrorV1::InvalidFriEquation)
        );
    }

    let layer = 7_usize;
    let current_indices =
        query_pair_indices_v1(&fixture.queries, DOMAIN_SIZE_V1 >> layer).expect("current indices");
    let next_indices = query_pair_indices_v1(&fixture.queries, DOMAIN_SIZE_V1 >> (layer + 1))
        .expect("next indices");
    let mut current_values = vec![0_u8; current_indices.len * LEAF_BYTES_V1];
    let next_values = vec![0_u8; next_indices.len * LEAF_BYTES_V1];
    let base = fixture.queries[0]
        % u32::try_from(DOMAIN_SIZE_V1 >> (layer + 1)).expect("fold half fits u32");
    let half = u32::try_from(DOMAIN_SIZE_V1 >> (layer + 1)).expect("fold half fits u32");
    write_value_v1(current_indices, &mut current_values, base, 0, Fq2V1::ONE);
    write_value_v1(
        current_indices,
        &mut current_values,
        base + half,
        0,
        Fq2V1::ONE,
    );
    assert_eq!(
        verify_fold_v1(
            fixture.context,
            &fields,
            &fixture.queries,
            layer,
            current_indices,
            &current_values,
            next_indices,
            &next_values,
        ),
        Err(RnsNativeQpcsFriCompleteErrorV1::InvalidFriEquation)
    );
}

#[test]
fn terminal_is_derived_from_exactly_four_authenticated_leaves() {
    let fixture = fixture_v1();
    let fields = derive_fields_v1().expect("canonical fields");
    let final_indices = fixture.shape.indices[ENCODED_LAYER_COUNT_V1 - 1];
    assert_eq!(final_indices.len, 4);
    assert_eq!(final_indices.values[..4], [0, 1, 2, 3]);
    let zero_values = vec![0_u8; 4 * LEAF_BYTES_V1];
    verify_terminal_degree_v1(fixture.context, &fields, final_indices, &zero_values)
        .expect("constant terminal codeword");

    for changed_leaf in 0_u32..4 {
        let mut changed = zero_values.clone();
        write_value_v1(final_indices, &mut changed, changed_leaf, 0, Fq2V1::ONE);
        assert_eq!(
            verify_terminal_degree_v1(fixture.context, &fields, final_indices, &changed),
            Err(RnsNativeQpcsFriCompleteErrorV1::InvalidTerminalDegree)
        );
    }

    let missing_parity_queries = [0_u32; QUERY_COUNT_V1];
    let missing = query_pair_indices_v1(&missing_parity_queries, 4).expect("two final leaves");
    assert_eq!(missing.len, 2);
    assert_eq!(
        verify_terminal_degree_v1(
            fixture.context,
            &fields,
            missing,
            &[0_u8; 2 * LEAF_BYTES_V1],
        ),
        Err(RnsNativeQpcsFriCompleteErrorV1::InvalidTerminalCoverage)
    );
    let mut reordered = final_indices;
    reordered.values.swap(0, 1);
    assert_eq!(
        verify_terminal_degree_v1(fixture.context, &fields, reordered, &zero_values),
        Err(RnsNativeQpcsFriCompleteErrorV1::InvalidTerminalCoverage)
    );
}

#[test]
fn every_fold_challenge_is_canonical_domain_separated_and_transcript_seed_bound() {
    let fixture = fixture_v1();
    let fields = derive_fields_v1().expect("canonical fields");
    let first = derive_fold_challenge_v1(fixture.context, 1, 0, 0, fields[0].modulus)
        .expect("first challenge");
    assert_ne!(first, Fq2V1::ZERO);
    assert!(first.c0 < fields[0].modulus);
    assert!(first.c1 < fields[0].modulus);
    assert_ne!(
        first,
        derive_fold_challenge_v1(fixture.context, 2, 0, 0, fields[0].modulus)
            .expect("layer-separated challenge")
    );
    assert_ne!(
        first,
        derive_fold_challenge_v1(fixture.context, 1, 0, 1, fields[0].modulus)
            .expect("row-separated challenge")
    );
    assert_ne!(
        first,
        derive_fold_challenge_v1(fixture.context, 1, 1, 0, fields[1].modulus)
            .expect("limb-separated challenge")
    );
    let mut changed_context = fixture.context;
    changed_context.fold_seeds.swap(1, 2);
    assert_ne!(
        first,
        derive_fold_challenge_v1(changed_context, 1, 0, 0, fields[0].modulus)
            .expect("schedule-bound challenge")
    );
    assert_eq!(
        derive_fold_challenge_v1(fixture.context, LAST_LAYER_V1 + 1, 0, 0, fields[0].modulus),
        Err(RnsNativeQpcsFriCompleteErrorV1::InvalidChallenge)
    );
}
