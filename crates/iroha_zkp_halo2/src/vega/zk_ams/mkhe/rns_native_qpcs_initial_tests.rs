use super::*;
use crate::vega::zk_ams::mkhe::{
    rns_native_profile::ZkAmsMkheRnsNativeFamilyV1,
    rns_native_section_codec::ZkAmsMkheRnsNativeRnsRelationQpcsSectionV1,
    rns_native_source::{
        ZkAmsMkheRnsNativeSecretChunkV1, ZkAmsMkheRnsNativeSourceArenaV1,
        ZkAmsMkheRnsNativeSourceErrorV1, ZkAmsMkheRnsNativeSourceLayoutV1,
        ZkAmsMkheRnsNativeSourceSnapshotV1,
    },
    rns_native_transcript::{
        ZkAmsMkheRnsNativeOpeningCommitmentV1, ZkAmsMkheRnsNativeOpeningCommitmentsV1,
        ZkAmsMkheRnsNativePublicContextV1, ZkAmsMkheRnsNativeQpcsFriRootV1,
        ZkAmsMkheRnsNativeQpcsRootsV1, ZkAmsMkheRnsNativeTerminalBridgeV1,
        ZkAmsMkheRnsNativeTerminalRootsV1, ZkAmsMkheRnsNativeTranscriptV1,
    },
};

const VALUES_BYTES_OFFSET_V1: usize = 14;
const AUTHENTICATION_BYTES_OFFSET_V1: usize = 18;
const CONTINUATION_BYTES_OFFSET_V1: usize = 22;
const PARAMETER_DIGEST_OFFSET_V1: usize = 26;
const TRANSCRIPT_DIGEST_OFFSET_V1: usize = 58;
const QUERY_SEED_OFFSET_V1: usize = 90;
const INITIAL_ROOT_OFFSET_V1: usize = 122;
const CONTINUATION_DIGEST_OFFSET_V1: usize = 154;

struct FixtureV1 {
    context: InitialContextV1,
    queries: [u32; QUERY_COUNT_V1],
    indices: IndexSetV1,
    authentication_count: usize,
    query_opening_digests: [[u8; DIGEST_BYTES_V1]; QUERY_COUNT_V1],
    proof: Vec<u8>,
}

fn test_authentication_digest_v1(height: usize, sibling_index: u32) -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.rns-native-qpcs.initial.test-authentication");
    hash.update(
        &u8::try_from(height)
            .expect("test height fits u8")
            .to_be_bytes(),
    );
    hash.update(&sibling_index.to_be_bytes());
    hash.finalize()
}

fn build_authentication_and_root_v1(
    parameter_digest: [u8; DIGEST_BYTES_V1],
    indices: IndexSetV1,
    values: &[u8],
) -> (Vec<u8>, [u8; DIGEST_BYTES_V1]) {
    build_authentication_and_root_with_v1(parameter_digest, indices, values, |height, index| {
        test_authentication_digest_v1(height, index)
    })
}

fn build_authentication_and_root_with_v1<F>(
    parameter_digest: [u8; DIGEST_BYTES_V1],
    indices: IndexSetV1,
    values: &[u8],
    mut sibling_digest: F,
) -> (Vec<u8>, [u8; DIGEST_BYTES_V1])
where
    F: FnMut(usize, u32) -> [u8; DIGEST_BYTES_V1],
{
    let mut current = [EMPTY_FRONTIER_NODE_V1; OPENED_LEAF_COUNT_V1];
    let mut next = [EMPTY_FRONTIER_NODE_V1; OPENED_LEAF_COUNT_V1];
    for (position, node) in current.iter_mut().enumerate().take(indices.len) {
        let start = position * LEAF_BYTES_V1;
        *node = FrontierNodeV1 {
            index: indices.values[position],
            digest: leaf_hash_v1(parameter_digest, &values[start..start + LEAF_BYTES_V1])
                .expect("canonical test leaf"),
        };
    }
    let mut authentication = Vec::new();
    let mut current_len = indices.len;
    let mut nodes_at_height = DOMAIN_SIZE_V1;
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
                let sibling = sibling_digest(height - 1, sibling_index);
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
                digest: node_hash_v1(parameter_digest, height, left, right)
                    .expect("test Merkle node"),
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
    (authentication, current[0].digest)
}

fn zero_tree_digests_v1(
    parameter_digest: [u8; DIGEST_BYTES_V1],
) -> [[u8; DIGEST_BYTES_V1]; ZK_AMS_MKHE_RNS_NATIVE_LDE_DOMAIN_LOG2_V1 as usize + 1] {
    let mut digests =
        [[0_u8; DIGEST_BYTES_V1]; ZK_AMS_MKHE_RNS_NATIVE_LDE_DOMAIN_LOG2_V1 as usize + 1];
    let zero_leaf = [0_u8; LEAF_BYTES_V1];
    digests[0] = leaf_hash_v1(parameter_digest, &zero_leaf).expect("zero leaf");
    for height in 1..digests.len() {
        digests[height] = node_hash_v1(
            parameter_digest,
            height,
            digests[height - 1],
            digests[height - 1],
        )
        .expect("zero subtree");
    }
    digests
}

fn fixture_digest_v1(label: &[u8], context: u16, ordinal: u16) -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.rns-native-qpcs.initial.canonical-fixture");
    hash.update(
        &u16::try_from(label.len())
            .expect("fixture label fits u16")
            .to_be_bytes(),
    );
    hash.update(label);
    hash.update(&context.to_be_bytes());
    hash.update(&ordinal.to_be_bytes());
    hash.finalize()
}

fn opening_role_v1(ordinal: usize) -> (ZkAmsMkheRnsNativeFamilyV1, u8) {
    match ordinal {
        0 => (ZkAmsMkheRnsNativeFamilyV1::X, 0),
        1..=16 => (
            ZkAmsMkheRnsNativeFamilyV1::U,
            u8::try_from(ordinal - 1).expect("U index fits u8"),
        ),
        17..=32 => (
            ZkAmsMkheRnsNativeFamilyV1::E,
            u8::try_from(ordinal - 17).expect("E index fits u8"),
        ),
        33 => (ZkAmsMkheRnsNativeFamilyV1::RE, 0),
        34..=41 => (
            ZkAmsMkheRnsNativeFamilyV1::W,
            u8::try_from(ordinal - 34).expect("W index fits u8"),
        ),
        42 => (ZkAmsMkheRnsNativeFamilyV1::RW, 0),
        _ => panic!("opening ordinal outside exact shape"),
    }
}

struct TestChunkV1 {
    arena: ZkAmsMkheRnsNativeSourceArenaV1,
    bytes: [u8; 1],
}

impl ZkAmsMkheRnsNativeSecretChunkV1 for TestChunkV1 {
    fn arena(&self) -> ZkAmsMkheRnsNativeSourceArenaV1 {
        self.arena
    }

    fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.bytes
    }
}

struct TestSnapshotV1 {
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    context: u16,
}

impl ZkAmsMkheRnsNativeSourceSnapshotV1 for TestSnapshotV1 {
    type Chunk = TestChunkV1;

    fn layout(&self) -> ZkAmsMkheRnsNativeSourceLayoutV1 {
        self.layout
    }

    fn snapshot_digest(&self, arena: ZkAmsMkheRnsNativeSourceArenaV1) -> [u8; DIGEST_BYTES_V1] {
        match arena {
            ZkAmsMkheRnsNativeSourceArenaV1::Main => {
                fixture_digest_v1(b"main-snapshot", self.context, 0)
            }
            ZkAmsMkheRnsNativeSourceArenaV1::Nonce => {
                fixture_digest_v1(b"nonce-snapshot", self.context, 0)
            }
        }
    }

    fn read_slot(
        &mut self,
        _arena: ZkAmsMkheRnsNativeSourceArenaV1,
        _slot: u64,
    ) -> Result<Self::Chunk, ZkAmsMkheRnsNativeSourceErrorV1> {
        Err(ZkAmsMkheRnsNativeSourceErrorV1::Storage)
    }
}

fn canonical_transcript_v1(
    initial_root: [u8; DIGEST_BYTES_V1],
    context: u16,
) -> ZkAmsMkheRnsNativeChallengeSeedsV1 {
    let profile = zk_ams_mkhe_rns_native_profile_v1().expect("canonical profile");
    let topology = zk_ams_mkhe_rns_native_topology_v1().expect("canonical topology");
    let release = zk_ams_mkhe_rns_native_release_candidate_digest_v1().expect("candidate");
    let layout = ZkAmsMkheRnsNativeSourceLayoutV1::new(
        profile.profile_digest,
        topology.topology_digest,
        release,
        fixture_digest_v1(b"statement", context, 0),
        fixture_digest_v1(b"operational-context", context, 0),
    )
    .expect("source layout");
    let receipt = TestSnapshotV1 { layout, context }
        .structural_receipt()
        .expect("source receipt");
    let public_context = ZkAmsMkheRnsNativePublicContextV1::new(
        fixture_digest_v1(b"governed-roster", context, 0),
        fixture_digest_v1(b"public-ciphertext", context, 0),
    )
    .expect("public context");
    let transcript = ZkAmsMkheRnsNativeTranscriptV1::new(layout, receipt, public_context)
        .expect("context transcript");
    let records = core::array::from_fn(|ordinal| {
        let (family, family_index) = opening_role_v1(ordinal);
        ZkAmsMkheRnsNativeOpeningCommitmentV1::new(
            family,
            family_index,
            fixture_digest_v1(
                b"source-commitment",
                context,
                u16::try_from(ordinal).expect("opening ordinal fits u16"),
            ),
            fixture_digest_v1(
                b"hyrax-commitment",
                context,
                u16::try_from(ordinal).expect("opening ordinal fits u16"),
            ),
        )
        .expect("opening commitment")
    });
    let openings =
        ZkAmsMkheRnsNativeOpeningCommitmentsV1::new(transcript.binding_digest(), records)
            .expect("opening set");
    let transcript = transcript
        .bind_opening_commitments(openings)
        .expect("opening transcript");
    let bridge = ZkAmsMkheRnsNativeTerminalBridgeV1::new(
        transcript.binding_digest(),
        fixture_digest_v1(b"mapping-root", context, 0),
        fixture_digest_v1(b"terminal-hyrax-root", context, 0),
        fixture_digest_v1(b"cross-basis-root", context, 0),
    )
    .expect("terminal bridge");
    let transcript = transcript
        .bind_terminal_bridge(bridge)
        .expect("terminal transcript");
    let fri_roots = core::array::from_fn(|layer| {
        ZkAmsMkheRnsNativeQpcsFriRootV1::new(
            u8::try_from(layer).expect("FRI layer fits u8"),
            fixture_digest_v1(
                b"qpcs-fri-root",
                context,
                u16::try_from(layer).expect("FRI layer fits u16"),
            ),
        )
        .expect("FRI root")
    });
    let roots = ZkAmsMkheRnsNativeQpcsRootsV1::new(
        transcript.binding_digest(),
        initial_root,
        fixture_digest_v1(b"q-mask-s-root", context, 0),
        fixture_digest_v1(b"qpcs-quotient-root", context, 0),
        fri_roots,
    )
    .expect("qPCS roots");
    let transcript = transcript.bind_qpcs_roots(roots).expect("qPCS transcript");
    let roots = ZkAmsMkheRnsNativeTerminalRootsV1::new(
        transcript.binding_digest(),
        fixture_digest_v1(b"cross-field-root", context, 0),
        fixture_digest_v1(b"global-lookup-root", context, 0),
        fixture_digest_v1(b"zero-padding-root", context, 0),
    )
    .expect("terminal roots");
    transcript
        .bind_terminal_roots(roots)
        .expect("final transcript")
}

fn expected_query_openings_v1(
    context: InitialContextV1,
    queries: &[u32; QUERY_COUNT_V1],
    indices: IndexSetV1,
    values: &[u8],
) -> [[u8; DIGEST_BYTES_V1]; QUERY_COUNT_V1] {
    let half = u32::try_from(DOMAIN_SIZE_V1 / 2).expect("half domain fits u32");
    core::array::from_fn(|ordinal| {
        let base = queries[ordinal];
        let paired = base + half;
        let first = leaf_hash_at_index_v1(context.parameter_digest, indices, values, base)
            .expect("first query leaf");
        let second = leaf_hash_at_index_v1(context.parameter_digest, indices, values, paired)
            .expect("second query leaf");
        query_opening_digest_v1(context, ordinal, base, paired, first, second)
            .expect("query opening digest")
    })
}

fn encode_proof_v1(
    context: InitialContextV1,
    values: &[u8],
    authentication: &[u8],
    continuation: &[u8],
) -> Vec<u8> {
    let continuation_digest =
        continuation_digest_v1(context, continuation).expect("continuation digest");
    let mut proof = Vec::with_capacity(
        QPCS_BODY_HEADER_BYTES_V1 + values.len() + authentication.len() + continuation.len(),
    );
    proof.extend_from_slice(&QPCS_BODY_MAGIC_V1);
    proof.push(QPCS_BODY_VERSION_V1);
    proof.push(ZK_AMS_MKHE_RNS_NATIVE_LDE_DOMAIN_LOG2_V1);
    proof.push(u8::try_from(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1).expect("limbs fit u8"));
    proof.push(u8::try_from(ROWS_PER_LIMB_V1).expect("rows fit u8"));
    proof.extend_from_slice(&ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1.to_be_bytes());
    proof.extend_from_slice(
        &u16::try_from(OPENED_LEAF_COUNT_V1)
            .expect("opened leaves fit u16")
            .to_be_bytes(),
    );
    proof.extend_from_slice(
        &u16::try_from(authentication.len() / DIGEST_BYTES_V1)
            .expect("authentication count fits u16")
            .to_be_bytes(),
    );
    proof.extend_from_slice(
        &u32::try_from(values.len())
            .expect("values fit u32")
            .to_be_bytes(),
    );
    proof.extend_from_slice(
        &u32::try_from(authentication.len())
            .expect("authentication fits u32")
            .to_be_bytes(),
    );
    proof.extend_from_slice(
        &u32::try_from(continuation.len())
            .expect("continuation fits u32")
            .to_be_bytes(),
    );
    proof.extend_from_slice(&context.parameter_digest);
    proof.extend_from_slice(&context.transcript_digest);
    proof.extend_from_slice(&context.query_seed);
    proof.extend_from_slice(&context.initial_root);
    proof.extend_from_slice(&continuation_digest);
    assert_eq!(proof.len(), QPCS_BODY_HEADER_BYTES_V1);
    proof.extend_from_slice(values);
    proof.extend_from_slice(authentication);
    proof.extend_from_slice(continuation);
    proof
}

fn fixture_v1(context_byte: u8) -> FixtureV1 {
    let parameter_digest = canonical_parameter_digest_v1().expect("canonical qPCS parameters");
    let query_seed = [context_byte.wrapping_add(0x31); DIGEST_BYTES_V1];
    let transcript_digest = [context_byte.wrapping_add(0x71); DIGEST_BYTES_V1];
    let queries = derive_queries_v1(parameter_digest, query_seed).expect("canonical queries");
    let indices = query_pair_indices_v1(&queries).expect("canonical query pairs");
    let authentication_count =
        exact_authentication_count_v1(indices).expect("bounded authentication count");
    let mut values = vec![0_u8; OPENED_LEAF_COUNT_V1 * LEAF_BYTES_V1];
    for position in 0..OPENED_LEAF_COUNT_V1 {
        let value = u64::try_from(position + 1).expect("test position fits u64");
        let start = position * LEAF_BYTES_V1;
        values[start..start + 8].copy_from_slice(&value.to_be_bytes());
    }
    let (authentication, initial_root) =
        build_authentication_and_root_v1(parameter_digest, indices, &values);
    assert_eq!(authentication.len(), authentication_count * DIGEST_BYTES_V1);
    let context = InitialContextV1 {
        parameter_digest,
        transcript_digest,
        query_seed,
        initial_root,
    };
    let query_opening_digests = expected_query_openings_v1(context, &queries, indices, &values);
    let proof = encode_proof_v1(
        context,
        &values,
        &authentication,
        &[0xa5, context_byte, 0x5a],
    );
    FixtureV1 {
        context,
        queries,
        indices,
        authentication_count,
        query_opening_digests,
        proof,
    }
}

fn rewrite_context_header_v1(proof: &mut [u8], context: InitialContextV1) {
    proof[PARAMETER_DIGEST_OFFSET_V1..PARAMETER_DIGEST_OFFSET_V1 + DIGEST_BYTES_V1]
        .copy_from_slice(&context.parameter_digest);
    proof[TRANSCRIPT_DIGEST_OFFSET_V1..TRANSCRIPT_DIGEST_OFFSET_V1 + DIGEST_BYTES_V1]
        .copy_from_slice(&context.transcript_digest);
    proof[QUERY_SEED_OFFSET_V1..QUERY_SEED_OFFSET_V1 + DIGEST_BYTES_V1]
        .copy_from_slice(&context.query_seed);
    proof[INITIAL_ROOT_OFFSET_V1..INITIAL_ROOT_OFFSET_V1 + DIGEST_BYTES_V1]
        .copy_from_slice(&context.initial_root);
    let continuation_bytes = u32::from_be_bytes(
        proof[CONTINUATION_BYTES_OFFSET_V1..CONTINUATION_BYTES_OFFSET_V1 + 4]
            .try_into()
            .expect("continuation length"),
    ) as usize;
    let continuation = &proof[proof.len() - continuation_bytes..];
    let digest = continuation_digest_v1(context, continuation).expect("rewritten continuation");
    proof[CONTINUATION_DIGEST_OFFSET_V1..CONTINUATION_DIGEST_OFFSET_V1 + DIGEST_BYTES_V1]
        .copy_from_slice(&digest);
}

#[test]
fn exact_40_limb_initial_multiproof_authenticates_without_authority() {
    let fixture = fixture_v1(1);
    assert_eq!(fixture.indices.len, 320);
    assert!(fixture.authentication_count <= MAX_INITIAL_AUTHENTICATION_HASHES_V1);
    assert!(fixture.proof.len() < ZK_AMS_MKHE_RNS_NATIVE_QPCS_MAX_BYTES_V1 as usize);
    verify_initial_with_context_v1(
        fixture.context,
        &fixture.query_opening_digests,
        &fixture.proof,
    )
    .expect("valid initial qPCS authentication");

    let source = include_str!("rns_native_qpcs_initial.rs");
    assert!(!source.contains("CandidateReceipt"));
    assert!(!source.contains("readiness = true"));
    assert!(!source.contains("release_ready = true"));
    assert!(!source.contains("authorizes"));
    assert!(
        source.contains("subsequently reports the\n/// still-unavailable complete RNS/qPCS stage")
    );
    let composite = include_str!("rns_native_composite_verifier.rs");
    assert!(composite.contains("authenticate_rns_native_qpcs_fri_complete_v1"));
    assert!(composite.contains(
        "StageUnavailable(\n            ZkAmsMkheRnsNativeVerificationStageV1::RnsRelationQpcs"
    ));
}

#[test]
fn canonical_transcript_and_typed_section_reach_authenticated_incomplete_boundary() {
    let parameter_digest = canonical_parameter_digest_v1().expect("canonical parameters");
    let zero_tree = zero_tree_digests_v1(parameter_digest);
    let initial_root = zero_tree[ZK_AMS_MKHE_RNS_NATIVE_LDE_DOMAIN_LOG2_V1 as usize];
    let transcript = canonical_transcript_v1(initial_root, 91);
    let context = InitialContextV1::from_transcript_v1(&transcript).expect("canonical context");
    assert_eq!(context.parameter_digest, parameter_digest);
    assert_eq!(context.initial_root, initial_root);
    let queries = derive_queries_v1(parameter_digest, context.query_seed).expect("queries");
    let indices = query_pair_indices_v1(&queries).expect("query pairs");
    let values = vec![0_u8; OPENED_LEAF_COUNT_V1 * LEAF_BYTES_V1];
    let (authentication, rebuilt_root) =
        build_authentication_and_root_with_v1(parameter_digest, indices, &values, |height, _| {
            zero_tree[height]
        });
    assert_eq!(rebuilt_root, initial_root);
    let query_openings = expected_query_openings_v1(context, &queries, indices, &values);
    let proof = encode_proof_v1(context, &values, &authentication, &[0x51]);
    let equation_digests: [[u8; DIGEST_BYTES_V1]; 2] = core::array::from_fn(|ordinal| {
        fixture_digest_v1(
            b"equation",
            91,
            u16::try_from(ordinal).expect("equation ordinal fits u16"),
        )
    });
    let limb_digests: [[u8; DIGEST_BYTES_V1]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1] =
        core::array::from_fn(|ordinal| {
            fixture_digest_v1(
                b"limb",
                91,
                u16::try_from(ordinal).expect("limb ordinal fits u16"),
            )
        });
    let encoded = ZkAmsMkheRnsNativeRnsRelationQpcsSectionV1::new(
        &transcript,
        &equation_digests,
        &limb_digests,
        &query_openings,
        &proof,
    )
    .expect("typed qPCS section")
    .to_canonical_bytes_v1()
    .expect("canonical qPCS section");
    let decoded = ZkAmsMkheRnsNativeRnsRelationQpcsSectionV1::from_canonical_bytes_exact_v1(
        &encoded,
        &transcript,
    )
    .expect("decoded qPCS section");
    verify_rns_native_qpcs_initial_v1(
        &transcript,
        decoded.query_opening_digests(),
        decoded.proof(),
    )
    .expect("initial authentication succeeds without completing qPCS");
}

#[test]
fn deterministic_query_schedule_retries_collisions_and_bias_tail() {
    let mut collision_retried = false;
    let queries = derive_queries_with_v1(|ordinal, attempt| {
        let sampled = match (ordinal, attempt) {
            (0, 0) => 7,
            (1, 0) => {
                collision_retried = true;
                7
            }
            (1, 1) => 8,
            (_, 0) => u64::try_from(ordinal + 7).expect("ordinal fits u64"),
            _ => return Err(RnsNativeQpcsInitialErrorV1::InvalidQuerySchedule),
        };
        Ok(sampled)
    })
    .expect("collisions retry canonically");
    assert!(collision_retried);
    assert_eq!(queries[0], 7);
    assert_eq!(queries[1], 8);
    assert_eq!(
        queries
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        160
    );

    let mut attempts = 0_usize;
    assert_eq!(
        derive_queries_with_v1(|_, _| {
            attempts += 1;
            Ok(u64::MAX)
        }),
        Err(RnsNativeQpcsInitialErrorV1::InvalidQuerySchedule)
    );
    assert_eq!(attempts, usize::from(MAX_QUERY_ATTEMPTS_V1));

    assert_eq!(
        derive_queries_with_v1(|_, _| Ok(11)),
        Err(RnsNativeQpcsInitialErrorV1::InvalidQuerySchedule)
    );
}

#[test]
fn path_order_root_and_leaf_mutations_are_rejected() {
    let fixture = fixture_v1(2);
    let values_bytes = u32::from_be_bytes(
        fixture.proof[VALUES_BYTES_OFFSET_V1..VALUES_BYTES_OFFSET_V1 + 4]
            .try_into()
            .expect("values length"),
    ) as usize;
    let authentication_offset = QPCS_BODY_HEADER_BYTES_V1 + values_bytes;

    let mut changed_path = fixture.proof.clone();
    changed_path[authentication_offset] ^= 1;
    assert_eq!(
        verify_initial_with_context_v1(
            fixture.context,
            &fixture.query_opening_digests,
            &changed_path,
        ),
        Err(RnsNativeQpcsInitialErrorV1::InvalidMerklePath)
    );

    let mut reordered_path = fixture.proof.clone();
    let second = authentication_offset + DIGEST_BYTES_V1;
    let (prefix, suffix) = reordered_path.split_at_mut(second);
    prefix[authentication_offset..authentication_offset + DIGEST_BYTES_V1]
        .swap_with_slice(&mut suffix[..DIGEST_BYTES_V1]);
    assert_eq!(
        verify_initial_with_context_v1(
            fixture.context,
            &fixture.query_opening_digests,
            &reordered_path,
        ),
        Err(RnsNativeQpcsInitialErrorV1::InvalidMerklePath)
    );

    let mut reordered_leaves = fixture.proof.clone();
    let second_leaf = QPCS_BODY_HEADER_BYTES_V1 + LEAF_BYTES_V1;
    let (prefix, suffix) = reordered_leaves.split_at_mut(second_leaf);
    prefix[QPCS_BODY_HEADER_BYTES_V1..QPCS_BODY_HEADER_BYTES_V1 + LEAF_BYTES_V1]
        .swap_with_slice(&mut suffix[..LEAF_BYTES_V1]);
    assert_eq!(
        verify_initial_with_context_v1(
            fixture.context,
            &fixture.query_opening_digests,
            &reordered_leaves,
        ),
        Err(RnsNativeQpcsInitialErrorV1::InvalidMerklePath)
    );

    let mut changed_context = fixture.context;
    changed_context.initial_root[0] ^= 1;
    let mut changed_root = fixture.proof.clone();
    rewrite_context_header_v1(&mut changed_root, changed_context);
    assert_eq!(
        verify_initial_with_context_v1(
            changed_context,
            &fixture.query_opening_digests,
            &changed_root,
        ),
        Err(RnsNativeQpcsInitialErrorV1::InvalidMerklePath)
    );
}

#[test]
fn noncanonical_fq2_and_query_digest_mutations_are_rejected() {
    let fixture = fixture_v1(3);
    let mut noncanonical = fixture.proof.clone();
    noncanonical[QPCS_BODY_HEADER_BYTES_V1..QPCS_BODY_HEADER_BYTES_V1 + 8]
        .copy_from_slice(&ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[0].to_be_bytes());
    assert_eq!(
        verify_initial_with_context_v1(
            fixture.context,
            &fixture.query_opening_digests,
            &noncanonical,
        ),
        Err(RnsNativeQpcsInitialErrorV1::NonCanonicalResidue)
    );

    let mut changed_digests = fixture.query_opening_digests;
    changed_digests[0][0] ^= 1;
    assert_eq!(
        verify_initial_with_context_v1(fixture.context, &changed_digests, &fixture.proof),
        Err(RnsNativeQpcsInitialErrorV1::InvalidQueryOpening)
    );
    let mut reordered_digests = fixture.query_opening_digests;
    reordered_digests.swap(0, 1);
    assert_eq!(
        verify_initial_with_context_v1(fixture.context, &reordered_digests, &fixture.proof),
        Err(RnsNativeQpcsInitialErrorV1::InvalidQueryOpening)
    );
}

#[test]
fn cap_trailing_truncation_and_forged_lengths_fail_closed() {
    let fixture = fixture_v1(4);
    let oversized = vec![0_u8; ZK_AMS_MKHE_RNS_NATIVE_QPCS_MAX_BYTES_V1 as usize + 1];
    assert_eq!(
        verify_initial_with_context_v1(fixture.context, &fixture.query_opening_digests, &oversized,),
        Err(RnsNativeQpcsInitialErrorV1::ProofCapExceeded)
    );

    let mut trailing = fixture.proof.clone();
    trailing.push(0);
    assert_eq!(
        verify_initial_with_context_v1(fixture.context, &fixture.query_opening_digests, &trailing,),
        Err(RnsNativeQpcsInitialErrorV1::TrailingBytes)
    );

    let mut truncated = fixture.proof.clone();
    truncated.pop();
    assert_eq!(
        verify_initial_with_context_v1(fixture.context, &fixture.query_opening_digests, &truncated,),
        Err(RnsNativeQpcsInitialErrorV1::Truncated)
    );

    let mut forged_values = fixture.proof.clone();
    forged_values[VALUES_BYTES_OFFSET_V1..VALUES_BYTES_OFFSET_V1 + 4]
        .copy_from_slice(&u32::MAX.to_be_bytes());
    assert_eq!(
        verify_initial_with_context_v1(
            fixture.context,
            &fixture.query_opening_digests,
            &forged_values,
        ),
        Err(RnsNativeQpcsInitialErrorV1::InvalidHeader)
    );

    let mut forged_authentication = fixture.proof.clone();
    forged_authentication[AUTHENTICATION_BYTES_OFFSET_V1..AUTHENTICATION_BYTES_OFFSET_V1 + 4]
        .copy_from_slice(&u32::MAX.to_be_bytes());
    assert_eq!(
        verify_initial_with_context_v1(
            fixture.context,
            &fixture.query_opening_digests,
            &forged_authentication,
        ),
        Err(RnsNativeQpcsInitialErrorV1::InvalidHeader)
    );

    let mut empty_continuation = fixture.proof.clone();
    empty_continuation[CONTINUATION_BYTES_OFFSET_V1..CONTINUATION_BYTES_OFFSET_V1 + 4]
        .copy_from_slice(&0_u32.to_be_bytes());
    assert_eq!(
        verify_initial_with_context_v1(
            fixture.context,
            &fixture.query_opening_digests,
            &empty_continuation,
        ),
        Err(RnsNativeQpcsInitialErrorV1::InvalidHeader)
    );
}

#[test]
fn context_and_unverified_continuation_are_digest_bound() {
    let fixture = fixture_v1(5);
    let mut changed_context = fixture.context;
    changed_context.transcript_digest[0] ^= 1;
    assert_eq!(
        verify_initial_with_context_v1(
            changed_context,
            &fixture.query_opening_digests,
            &fixture.proof,
        ),
        Err(RnsNativeQpcsInitialErrorV1::InvalidHeader)
    );

    let mut changed_continuation = fixture.proof.clone();
    let last = changed_continuation.len() - 1;
    changed_continuation[last] ^= 1;
    assert_eq!(
        verify_initial_with_context_v1(
            fixture.context,
            &fixture.query_opening_digests,
            &changed_continuation,
        ),
        Err(RnsNativeQpcsInitialErrorV1::InvalidHeader)
    );

    assert_eq!(fixture.queries.len(), QUERY_COUNT_V1);
    assert_eq!(fixture.indices.len, OPENED_LEAF_COUNT_V1);
}
