use super::*;
const SOURCE_DIGEST: [u8; 32] = [0x72; 32];
const ALGEBRA_DIGEST: [u8; 32] = [0x83; 32];
#[derive(Clone, Copy)]
enum UniformCase {
    Valid,
    OpeningMismatch,
    BatchMismatch,
    FriMismatch,
}
fn context() -> ExpectedPublicContextV2 {
    ExpectedPublicContextV2 {
        sealed_source_transcript_digest: SOURCE_DIGEST,
        source_algebra_binding_digest: ALGEBRA_DIGEST,
    }
}
fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}
fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}
fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_be_bytes());
}
fn uniform_tree_levels(
    kind: TreeKindV2,
    layer: usize,
    length: usize,
    leaf: &[u8; LEAF_BYTES_V2],
) -> Vec<[u8; 32]> {
    let parameter_digest = parameter_digest_v2().unwrap();
    let mut levels = Vec::with_capacity(length.ilog2() as usize + 1);
    levels.push(merkle_leaf_hash_v2(parameter_digest, kind, layer, length, leaf).unwrap());
    for height in 1..=length.ilog2() as usize {
        levels.push(
            merkle_node_hash_v2(
                parameter_digest,
                kind,
                layer,
                height,
                levels[height - 1],
                levels[height - 1],
            )
            .unwrap(),
        );
    }
    levels
}
fn uniform_root(
    kind: TreeKindV2,
    layer: usize,
    length: usize,
    leaf: &[u8; LEAF_BYTES_V2],
) -> [u8; 32] {
    *uniform_tree_levels(kind, layer, length, leaf)
        .last()
        .unwrap()
}
fn uniform_leaf(case: UniformCase, kind: TreeKindV2, layer: usize) -> [u8; LEAF_BYTES_V2] {
    let mut leaf = [0_u8; LEAF_BYTES_V2];
    let make_nonzero = match case {
        UniformCase::Valid => false,
        UniformCase::OpeningMismatch => matches!(kind, TreeKindV2::Initial),
        UniformCase::BatchMismatch => matches!(kind, TreeKindV2::Fri) && layer == 0,
        UniformCase::FriMismatch => matches!(kind, TreeKindV2::Fri) && layer == 1,
    };
    if make_nonzero {
        leaf[7] = 1;
    }
    leaf
}
fn append_uniform_section(
    wire: &mut Vec<u8>,
    queries: &[u32; QUERY_COUNT_V2],
    length: usize,
    kind: TreeKindV2,
    layer: usize,
    expected_root: [u8; 32],
    leaf: &[u8; LEAF_BYTES_V2],
) {
    let indices = query_pair_indices_v2(queries, length);
    let authentication = exact_authentication_count_v2(&indices, length).unwrap();
    wire.extend_from_slice(&(indices.len as u32).to_be_bytes());
    wire.extend_from_slice(&(authentication as u32).to_be_bytes());
    for _ in 0..indices.len {
        wire.extend_from_slice(leaf);
    }
    let levels = uniform_tree_levels(kind, layer, length, leaf);
    assert_eq!(*levels.last().unwrap(), expected_root);
    let mut current = indices.values[..indices.len].to_vec();
    let mut height = 0_usize;
    let mut written = 0_usize;
    while current.len() != 1 || current[0] != 0 {
        let mut parents = Vec::with_capacity(current.len());
        let mut cursor = 0_usize;
        while cursor < current.len() {
            let node = current[cursor];
            let sibling = node ^ 1;
            if node.is_multiple_of(2)
                && cursor + 1 < current.len()
                && current[cursor + 1] == sibling
            {
                cursor += 2;
            } else {
                wire.extend_from_slice(&levels[height]);
                written += 1;
                cursor += 1;
            }
            parents.push(node / 2);
        }
        parents.sort_unstable();
        parents.dedup();
        current = parents;
        height += 1;
    }
    assert_eq!(written, authentication);
}
fn authentication_cap_witness_queries() -> [u32; QUERY_COUNT_V2] {
    let mut queries = [0_u32; QUERY_COUNT_V2];
    let mut next = 0_usize;
    for parity in [1_u32, 0] {
        for state in 0_u16..=u8::MAX as u16 {
            let state = state as u8;
            if state.count_ones() % 2 == parity {
                for bit in 1_usize..18 {
                    let state_bit = (bit - 1) % 8;
                    queries[next] |= u32::from((state >> state_bit) & 1) << bit;
                }
                next += 1;
                if next == QUERY_COUNT_V2 {
                    return queries;
                }
            }
        }
    }
    unreachable!("the 8-bit parity partition contains 160 requested states")
}
fn authenticated_uniform_wire_for_queries(
    case: UniformCase,
    query_override: Option<[u32; QUERY_COUNT_V2]>,
) -> Vec<u8> {
    let parameter_digest = parameter_digest_v2().unwrap();
    let initial_leaf = uniform_leaf(case, TreeKindV2::Initial, 0);
    let quotient_leaf = uniform_leaf(case, TreeKindV2::OpeningQuotient, 0);
    let initial_root = uniform_root(TreeKindV2::Initial, 0, DOMAIN_SIZE_V2, &initial_leaf);
    let quotient_root = uniform_root(
        TreeKindV2::OpeningQuotient,
        0,
        DOMAIN_SIZE_V2,
        &quotient_leaf,
    );
    let mut fri_roots = [[0_u8; 32]; FRI_ROUNDS_V2];
    let mut length = DOMAIN_SIZE_V2;
    for (layer, root) in fri_roots.iter_mut().enumerate() {
        let leaf = uniform_leaf(case, TreeKindV2::Fri, layer);
        *root = uniform_root(TreeKindV2::Fri, layer, length, &leaf);
        length /= 2;
    }
    let mut wire = vec![0_u8; FIXED_BEFORE_SECTIONS_V2];
    wire[..16].copy_from_slice(&MAGIC_V2);
    wire[16..24].copy_from_slice(&[2, 17, 19, 38, 5, 10, 18, 2]);
    put_u32(&mut wire, 24, N_V2 as u32);
    put_u32(&mut wire, 28, DOMAIN_SIZE_V2 as u32);
    put_u16(&mut wire, 32, QUERY_COUNT_V2 as u16);
    put_u16(&mut wire, 34, MAX_INITIAL_OPENED_LEAVES_V2 as u16);
    put_u32(&mut wire, 36, MAX_FRI_OPENED_LEAVES_V2 as u32);
    put_u32(&mut wire, 40, MAX_INITIAL_AUTH_HASHES_PER_TREE_V2 as u32);
    put_u32(&mut wire, 44, MAX_FRI_AUTH_HASHES_V2 as u32);
    put_u32(&mut wire, 48, LEAF_BYTES_V2 as u32);
    put_u32(&mut wire, 52, FQ2_BYTES_V2 as u32);
    put_u64(&mut wire, 56, MAX_PROOF_BYTES_V2 as u64);
    wire[64..96].copy_from_slice(&parameter_digest);
    wire[96..128].copy_from_slice(&SOURCE_DIGEST);
    wire[128..160].copy_from_slice(&ALGEBRA_DIGEST);
    wire[160..192].copy_from_slice(&initial_root);
    let quotient_offset = HEADER_BYTES_V2 + EVALUATION_BYTES_V2;
    wire[quotient_offset..quotient_offset + 32].copy_from_slice(&quotient_root);
    let fri_root_offset = quotient_offset + QUOTIENT_ROOT_BYTES_V2;
    for (layer, root) in fri_roots.iter().enumerate() {
        wire[fri_root_offset + layer * 32..fri_root_offset + (layer + 1) * 32]
            .copy_from_slice(root);
    }
    let mut header = begin_v2(&wire, context(), SourceReplaySealV2::TestOnly).unwrap();
    let mut points = header.derive_points_v2().unwrap();
    let mut relations = points.check_relations_v2().unwrap();
    let mut quotient = relations.bind_quotient_root_v2().unwrap();
    let fri = quotient.bind_fri_transcript_v2().unwrap();
    let queries = query_override.unwrap_or(fri.live.as_ref().unwrap().queries);
    append_uniform_section(
        &mut wire,
        &queries,
        DOMAIN_SIZE_V2,
        TreeKindV2::Initial,
        0,
        initial_root,
        &initial_leaf,
    );
    append_uniform_section(
        &mut wire,
        &queries,
        DOMAIN_SIZE_V2,
        TreeKindV2::OpeningQuotient,
        0,
        quotient_root,
        &quotient_leaf,
    );
    let mut layer_queries = queries;
    let mut length = DOMAIN_SIZE_V2;
    for (layer, root) in fri_roots.iter().copied().enumerate() {
        let leaf = uniform_leaf(case, TreeKindV2::Fri, layer);
        append_uniform_section(
            &mut wire,
            &layer_queries,
            length,
            TreeKindV2::Fri,
            layer,
            root,
            &leaf,
        );
        let half = length / 2;
        for query in &mut layer_queries {
            *query %= half as u32;
        }
        length = half;
    }
    assert_eq!(length, 2);
    assert!(wire.len() <= MAX_PROOF_BYTES_V2);
    wire
}
fn authenticated_uniform_wire(case: UniformCase) -> Vec<u8> {
    authenticated_uniform_wire_for_queries(case, None)
}
fn authenticated_zero_wire() -> Vec<u8> {
    authenticated_uniform_wire(UniformCase::Valid)
}
fn through_fri(wire: &[u8]) -> FriTranscriptBoundV2<'_> {
    let mut header = begin_v2(wire, context(), SourceReplaySealV2::TestOnly).unwrap();
    let mut points = header.derive_points_v2().unwrap();
    let mut relations = points.check_relations_v2().unwrap();
    let mut quotient = relations.bind_quotient_root_v2().unwrap();
    quotient.bind_fri_transcript_v2().unwrap()
}
fn section_len(wire: &[u8], offset: usize) -> usize {
    let opened = read_u32_v2(wire, offset).unwrap() as usize;
    let authentication = read_u32_v2(wire, offset + 4).unwrap() as usize;
    SECTION_HEADER_BYTES_V2 + opened * LEAF_BYTES_V2 + authentication * 32
}
#[test]
fn literal_hash_field_and_equation_kats_are_stable() {
    const ZERO_LEAF_KAT: [u8; 32] = [
        0x63, 0x79, 0x0d, 0xc7, 0x3d, 0xdb, 0xcc, 0x0e, 0xce, 0xe1, 0xd0, 0xc3, 0xbb, 0xa0, 0x96,
        0x3f, 0x49, 0x8c, 0x76, 0x0b, 0xf1, 0x35, 0xee, 0xec, 0xce, 0x94, 0xba, 0x0c, 0x1a, 0xb0,
        0xee, 0xf3,
    ];
    const ZERO_PARENT_KAT: [u8; 32] = [
        0x93, 0x8a, 0xe4, 0x48, 0xcb, 0x8d, 0xb1, 0x67, 0x2f, 0x9b, 0xe3, 0x62, 0xe8, 0x5e, 0xa7,
        0xf4, 0x31, 0xe6, 0xf7, 0x35, 0xe9, 0x2d, 0x9f, 0xb9, 0x09, 0x1d, 0x1e, 0x92, 0xb6, 0xd5,
        0x68, 0x55,
    ];
    const EVEN_BATCH_KAT: Fq2V1 = Fq2V1 {
        c0: 105_475,
        c1: 74_853,
    };
    const ODD_BATCH_KAT: Fq2V1 = Fq2V1 {
        c0: 128_415_966_054_073_974,
        c1: 99_349_605_014_868_052,
    };
    const FOLD_KAT: Fq2V1 = Fq2V1 {
        c0: 418_867_165_342_680_109,
        c1: 659_404_745_440_456_809,
    };
    let parameter_digest = parameter_digest_v2().unwrap();
    let leaf = [0_u8; LEAF_BYTES_V2];
    let leaf_hash = merkle_leaf_hash_v2(
        parameter_digest,
        TreeKindV2::Initial,
        0,
        DOMAIN_SIZE_V2,
        &leaf,
    )
    .unwrap();
    assert_eq!(leaf_hash, ZERO_LEAF_KAT);
    assert_eq!(
        merkle_node_hash_v2(
            parameter_digest,
            TreeKindV2::Initial,
            0,
            1,
            leaf_hash,
            leaf_hash,
        )
        .unwrap(),
        ZERO_PARENT_KAT
    );
    let field = field_parameters_v2().unwrap()[0];
    assert_eq!(field.nonresidue, 5);
    assert_eq!(
        field.domain_root,
        Fq2V1 {
            c0: 0,
            c1: 843_015_778_799_891_155,
        }
    );
    let x = Fq2V1 { c0: 7, c1: 11 };
    let quotient = Fq2V1 { c0: 13, c1: 17 };
    let point = 19;
    let evaluation = 23;
    let committed = field.add(
        Fq2V1::base(evaluation),
        field.mul(field.sub(x, Fq2V1::base(point)), quotient),
    );
    assert_eq!(
        committed,
        Fq2V1 {
            c0: 802,
            c1: 1_152_921_504_606_584_772,
        }
    );
    assert!(opening_equation_holds_v2(
        field, x, point, evaluation, committed, quotient
    ));
    assert!(!opening_equation_holds_v2(
        field,
        x,
        point,
        evaluation + 1,
        committed,
        quotient
    ));
    let a = Fq2V1 { c0: 29, c1: 31 };
    let b = Fq2V1 { c0: 37, c1: 41 };
    assert_eq!(
        batch_value_v2(field, x, committed, quotient, a, b, 0),
        EVEN_BATCH_KAT
    );
    assert_eq!(
        batch_value_v2(field, x, committed, quotient, a, b, 1),
        ODD_BATCH_KAT
    );
    assert_ne!(EVEN_BATCH_KAT, ODD_BATCH_KAT);
    let positive = Fq2V1 { c0: 43, c1: 47 };
    let negative = Fq2V1 { c0: 53, c1: 59 };
    let alpha = Fq2V1 { c0: 61, c1: 67 };
    assert_eq!(
        fold_value_v2(field, x, positive, negative, alpha).unwrap(),
        FOLD_KAT
    );
}
#[test]
fn authenticated_verifier_accepts_the_exact_fri_authentication_maximum() {
    let queries = authentication_cap_witness_queries();
    let wire = authenticated_uniform_wire_for_queries(UniformCase::Valid, Some(queries));
    assert_eq!(wire.len(), 27_322_528);
    let mut offset = FIXED_BEFORE_SECTIONS_V2;
    let mut fri_opened = 0_usize;
    let mut fri_authentication = 0_usize;
    for section in 0..SECTION_COUNT_V2 {
        if section >= FRI_SECTION_START_V2 {
            fri_opened += read_u32_v2(&wire, offset).unwrap() as usize;
            fri_authentication += read_u32_v2(&wire, offset + 4).unwrap() as usize;
        }
        offset += section_len(&wire, offset);
    }
    assert_eq!(offset, wire.len());
    assert_eq!((fri_opened, fri_authentication), (3_710, 20_030));
    let mut fri = through_fri(&wire);
    // Test authenticated geometry independently of a transcript-preimage search.
    fri.live.as_mut().unwrap().queries = queries;
    let verified = fri.verify_authenticated_equations_v2().unwrap();
    assert!(verified.live.is_some());
}
#[test]
fn authenticated_zero_codeword_verifies_and_transition_poison_is_sticky() {
    let wire = authenticated_zero_wire();
    let mut fri = through_fri(&wire);
    let verified = fri.verify_authenticated_equations_v2().unwrap();
    assert!(verified.live.is_some());
    assert!(matches!(
        fri.verify_authenticated_equations_v2(),
        Err(SoundnessErrorV2::Poisoned)
    ));
    const {
        assert!(TEN_ROW_MERKLE_PATHS_VERIFIED_V2);
        assert!(OPENING_QUOTIENT_EQUATIONS_VERIFIED_V2);
        assert!(TEN_ROW_BATCHING_EQUATIONS_VERIFIED_V2);
        assert!(TEN_ROW_FRI_EQUATIONS_VERIFIED_V2);
        assert!(!AUTHENTICATED_MULTIPASS_REPLAY_INTEGRATED_V2);
        assert!(!RELEASE_READY_V2);
    }
}
#[test]
fn authenticated_hostile_opening_batch_and_fold_equations_fail_at_their_gate() {
    for (case, expected) in [
        (
            UniformCase::OpeningMismatch,
            SoundnessErrorV2::InvalidOpeningQuotient,
        ),
        (
            UniformCase::BatchMismatch,
            SoundnessErrorV2::InvalidBatchEquation,
        ),
        (
            UniformCase::FriMismatch,
            SoundnessErrorV2::InvalidFriEquation,
        ),
    ] {
        let wire = authenticated_uniform_wire(case);
        let mut fri = through_fri(&wire);
        let error = match fri.verify_authenticated_equations_v2() {
            Ok(_) => panic!("hostile authenticated equation was accepted"),
            Err(error) => error,
        };
        assert_eq!(error, expected);
    }
}
#[test]
fn hostile_value_path_and_tree_role_order_fail_closed() {
    let wire = authenticated_zero_wire();
    let first_section = FIXED_BEFORE_SECTIONS_V2;
    let opened = read_u32_v2(&wire, first_section).unwrap() as usize;
    let authentication_start = first_section + SECTION_HEADER_BYTES_V2 + opened * LEAF_BYTES_V2;
    let mut changed_value = wire.clone();
    changed_value[first_section + SECTION_HEADER_BYTES_V2 + 15] ^= 1;
    let mut fri = through_fri(&changed_value);
    assert!(matches!(
        fri.verify_authenticated_equations_v2(),
        Err(SoundnessErrorV2::InvalidMerklePath)
    ));
    let mut changed_path = wire.clone();
    changed_path[authentication_start] ^= 1;
    let mut fri = through_fri(&changed_path);
    assert!(matches!(
        fri.verify_authenticated_equations_v2(),
        Err(SoundnessErrorV2::InvalidMerklePath)
    ));
    let first_len = section_len(&wire, first_section);
    let second_section = first_section + first_len;
    let second_len = section_len(&wire, second_section);
    assert_eq!(first_len, second_len);
    let mut reordered = wire;
    let first = reordered[first_section..second_section].to_vec();
    let second = reordered[second_section..second_section + second_len].to_vec();
    reordered[first_section..second_section].copy_from_slice(&second);
    reordered[second_section..second_section + second_len].copy_from_slice(&first);
    let mut fri = through_fri(&reordered);
    assert!(matches!(
        fri.verify_authenticated_equations_v2(),
        Err(SoundnessErrorV2::InvalidMerklePath)
    ));
}
#[test]
fn source_guards_keep_verifier_borrowed_private_bounded_and_non_authorizing() {
    let source = include_str!("phase23_rns_link_q_pcs_v2_verifier.rs");
    let parent = include_str!("phase23_rns_link_q_pcs_v2_soundness.rs");
    assert!(source.lines().count() <= 1_200);
    assert!(!source.contains("Vec<"));
    assert!(!source.contains(".to_vec()"));
    assert!(!source.contains("pub struct"));
    assert!(!source.contains("pub enum"));
    assert!(!source.contains("pub fn"));
    assert!(!source.contains("caller_challenge"));
    assert!(source.contains("values: &'a [u8]"));
    assert!(source.contains("[EMPTY_FRONTIER_NODE_V2; MAX_FRONTIER_NODES_V2]"));
    assert!(source.contains("checked_fri_multiproof_bytes_v2(fri_opened, fri_authentication)?"));
    assert!(parent.contains("const MAX_FRI_AUTH_HASHES_V2: usize = 20_030;"));
    assert!(parent.contains("const MAX_FRI_MULTIPROOF_BYTES_V2: usize = 25_121_024;"));
    assert!(source.contains("self.live.take().ok_or(SoundnessErrorV2::Poisoned)"));
    assert!(!source.contains("derive(Clone, Debug"));
    for true_gate in [
        "TEN_ROW_MERKLE_PATHS_VERIFIED_V2: bool = true",
        "OPENING_QUOTIENT_EQUATIONS_VERIFIED_V2: bool = true",
        "TEN_ROW_BATCHING_EQUATIONS_VERIFIED_V2: bool = true",
        "TEN_ROW_FRI_EQUATIONS_VERIFIED_V2: bool = true",
    ] {
        assert!(parent.contains(true_gate));
    }
    for false_gate in [
        "SOURCE_AGGREGATION_LINKED_V2: bool = false",
        "CROSS_SET_ALGEBRA_VERIFIED_V2: bool = false",
        "HYRAX_LINKED_V2: bool = false",
        "PRODUCTION_SAMPLER_QUALIFIED_V2: bool = false",
        "ZERO_KNOWLEDGE_THEOREM_INSTANTIATED_V2: bool = false",
        "AUTHENTICATED_MULTIPASS_REPLAY_INTEGRATED_V2: bool = false",
        "COEFFICIENT_TOP_ZERO_REPLAY_VERIFIED_V2: bool = false",
        "COMPLETE_WORK_BOUND_DERIVED_V2: bool = false",
        "MEASURED_RSS_WITHIN_CAP_V2: bool = false",
        "OPERATIONAL_RECEIPT_ACCEPTED_V2: bool = false",
        "RELEASE_READY_V2: bool = false",
    ] {
        assert!(parent.contains(false_gate));
    }
}
