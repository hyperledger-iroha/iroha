use super::super::{
    rns_native_profile::{
        ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1, ZK_AMS_MKHE_RNS_NATIVE_NEGACYCLIC_ROOTS_V1,
        ZkAmsMkheRnsNativeFamilyV1, zk_ams_mkhe_rns_native_profile_v1,
        zk_ams_mkhe_rns_native_release_candidate_digest_v1, zk_ams_mkhe_rns_native_topology_v1,
    },
    rns_native_qpcs_prefix::{FQ2_BYTES_V1, tree_leaf_hash_v1, tree_node_hash_v1},
    rns_native_source::{
        ZkAmsMkheRnsNativeSecretChunkV1, ZkAmsMkheRnsNativeSourceArenaV1,
        ZkAmsMkheRnsNativeSourceErrorV1, ZkAmsMkheRnsNativeSourceLayoutV1,
        ZkAmsMkheRnsNativeSourceSnapshotV1,
    },
    rns_native_transcript::{
        ZkAmsMkheRnsNativeOpeningCommitmentV1, ZkAmsMkheRnsNativeOpeningCommitmentsV1,
        ZkAmsMkheRnsNativePublicContextV1, ZkAmsMkheRnsNativeQpcsFriRootV1,
        ZkAmsMkheRnsNativeTerminalBoundTranscriptV1, ZkAmsMkheRnsNativeTerminalBridgeV1,
        ZkAmsMkheRnsNativeTerminalRootsV1, ZkAmsMkheRnsNativeTranscriptV1,
    },
};
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

struct JoinTestChunkV1 {
    arena: ZkAmsMkheRnsNativeSourceArenaV1,
    bytes: [u8; 1],
}

impl ZkAmsMkheRnsNativeSecretChunkV1 for JoinTestChunkV1 {
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

struct JoinTestSnapshotV1 {
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    context: u16,
}

impl ZkAmsMkheRnsNativeSourceSnapshotV1 for JoinTestSnapshotV1 {
    type Chunk = JoinTestChunkV1;

    fn layout(&self) -> ZkAmsMkheRnsNativeSourceLayoutV1 {
        self.layout
    }

    fn snapshot_digest(&self, arena: ZkAmsMkheRnsNativeSourceArenaV1) -> [u8; DIGEST_BYTES_V1] {
        let ordinal = match arena {
            ZkAmsMkheRnsNativeSourceArenaV1::Main => 5,
            ZkAmsMkheRnsNativeSourceArenaV1::Nonce => 6,
        };
        join_digest_v1(self.context, ordinal)
    }

    fn read_slot(
        &mut self,
        _arena: ZkAmsMkheRnsNativeSourceArenaV1,
        _slot: u64,
    ) -> Result<Self::Chunk, ZkAmsMkheRnsNativeSourceErrorV1> {
        Err(ZkAmsMkheRnsNativeSourceErrorV1::Storage)
    }
}

#[derive(Clone, Copy)]
enum JoinQpcsVariantV1 {
    Matching,
    Quotient,
    Fri(usize),
}

fn join_digest_v1(context: u16, ordinal: u16) -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.rns-native-qpcs.exact-state-join.test");
    hash.update(&context.to_be_bytes());
    hash.update(&ordinal.to_be_bytes());
    hash.finalize()
}

fn join_opening_role_v1(ordinal: usize) -> (ZkAmsMkheRnsNativeFamilyV1, u8) {
    match ordinal {
        0 => (ZkAmsMkheRnsNativeFamilyV1::X, 0),
        1..=16 => (ZkAmsMkheRnsNativeFamilyV1::U, (ordinal - 1) as u8),
        17..=32 => (ZkAmsMkheRnsNativeFamilyV1::E, (ordinal - 17) as u8),
        33 => (ZkAmsMkheRnsNativeFamilyV1::RE, 0),
        34..=41 => (ZkAmsMkheRnsNativeFamilyV1::W, (ordinal - 34) as u8),
        42 => (ZkAmsMkheRnsNativeFamilyV1::RW, 0),
        _ => panic!("opening ordinal outside the canonical 43-record schedule"),
    }
}

fn join_terminal_stage_v1(context: u16) -> ZkAmsMkheRnsNativeTerminalBoundTranscriptV1 {
    let profile = zk_ams_mkhe_rns_native_profile_v1().expect("profile");
    let topology = zk_ams_mkhe_rns_native_topology_v1().expect("topology");
    let release = zk_ams_mkhe_rns_native_release_candidate_digest_v1().expect("release");
    let layout = ZkAmsMkheRnsNativeSourceLayoutV1::new(
        profile.profile_digest,
        topology.topology_digest,
        release,
        join_digest_v1(context, 1),
        join_digest_v1(context, 2),
    )
    .expect("layout");
    let receipt = JoinTestSnapshotV1 { layout, context }
        .structural_receipt()
        .expect("source receipt");
    let public = ZkAmsMkheRnsNativePublicContextV1::new(
        join_digest_v1(context, 3),
        join_digest_v1(context, 4),
    )
    .expect("public context");
    let transcript =
        ZkAmsMkheRnsNativeTranscriptV1::new(layout, receipt, public).expect("context transcript");
    let openings = core::array::from_fn(|ordinal| {
        let (family, family_index) = join_opening_role_v1(ordinal);
        ZkAmsMkheRnsNativeOpeningCommitmentV1::new(
            family,
            family_index,
            join_digest_v1(context, 100 + 2 * ordinal as u16),
            join_digest_v1(context, 101 + 2 * ordinal as u16),
        )
        .expect("opening")
    });
    let openings =
        ZkAmsMkheRnsNativeOpeningCommitmentsV1::new(transcript.binding_digest(), openings)
            .expect("opening bundle");
    let transcript = transcript
        .bind_opening_commitments(openings)
        .expect("opening transcript");
    let bridge = ZkAmsMkheRnsNativeTerminalBridgeV1::new(
        transcript.binding_digest(),
        join_digest_v1(context, 300),
        join_digest_v1(context, 301),
        join_digest_v1(context, 302),
    )
    .expect("terminal bridge");
    transcript
        .bind_terminal_bridge(bridge)
        .expect("terminal transcript")
}

fn join_schedule_and_qpcs_v1(
    context: u16,
    variant: JoinQpcsVariantV1,
) -> (
    RnsNativeQpcsRelationScheduleV1,
    ZkAmsMkheRnsNativeQpcsBoundTranscriptV1,
) {
    let terminal = join_terminal_stage_v1(context);
    let mut relation = terminal
        .bind_qpcs_initial_root(join_digest_v1(context, 400))
        .expect("initial qPCS root")
        .bind_q_mask_s_root(join_digest_v1(context, 401))
        .expect("q-mask root");
    let binding = relation
        .take_qpcs_relation_binding()
        .expect("one-shot relation lineage");
    let schedule = RnsNativeQpcsRelationScheduleV1::from_relation_binding_v1(
        join_digest_v1(context, 402),
        binding,
    )
    .expect("lineage-bearing relation schedule");
    let quotient_ordinal = match variant {
        JoinQpcsVariantV1::Quotient => 404,
        JoinQpcsVariantV1::Matching | JoinQpcsVariantV1::Fri(_) => 403,
    };
    let mut fri = relation
        .bind_qpcs_quotient_root(join_digest_v1(context, quotient_ordinal))
        .expect("quotient root");
    for layer in 0..ZK_AMS_MKHE_RNS_NATIVE_FRI_ROUNDS_V1 as usize {
        let ordinal = if matches!(variant, JoinQpcsVariantV1::Fri(changed) if changed == layer) {
            600 + u16::try_from(layer).expect("layer fits u16")
        } else {
            500 + u16::try_from(layer).expect("layer fits u16")
        };
        let root = ZkAmsMkheRnsNativeQpcsFriRootV1::new(
            u8::try_from(layer).expect("layer fits u8"),
            join_digest_v1(context, ordinal),
        )
        .expect("typed FRI root");
        fri = fri.bind_qpcs_fri_root(root).expect("ordered FRI root");
    }
    let qpcs = fri.finish_qpcs_fri_roots().expect("qPCS-bound transcript");
    (schedule, qpcs)
}

fn join_terminal_roots_v1(
    context: u16,
    qpcs: &ZkAmsMkheRnsNativeQpcsBoundTranscriptV1,
) -> ZkAmsMkheRnsNativeTerminalRootsV1 {
    ZkAmsMkheRnsNativeTerminalRootsV1::new(
        qpcs.binding_digest(),
        join_digest_v1(context, 700),
        join_digest_v1(context, 701),
        join_digest_v1(context, 702),
    )
    .expect("terminal roots")
}

fn join_challenge_seeds_v1(context: u16) -> ZkAmsMkheRnsNativeChallengeSeedsV1 {
    let (_, qpcs) = join_schedule_and_qpcs_v1(context, JoinQpcsVariantV1::Matching);
    let roots = join_terminal_roots_v1(context, &qpcs);
    qpcs.bind_terminal_roots(roots).expect("challenge seeds")
}

fn join_stage_v1(
    relation_schedule: RnsNativeQpcsRelationScheduleV1,
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
) -> RnsNativeQpcsFriCompleteStageV1<'static> {
    let parameter_digest = relation_schedule.parameter_digest();
    RnsNativeQpcsFriCompleteStageV1 {
        relation_schedule: Some(relation_schedule),
        qpcs_bound_transcript_state: transcript.qpcs_bound_transcript_state_v1(),
        parameter_digest,
        transcript_digest: transcript.transcript_digest(),
        query_seed: transcript.qpcs_query_challenge_seed(),
        section_binding_digest: join_digest_v1(990, 2),
        schedule_digest: join_digest_v1(990, 3),
        evaluations: &[],
        evaluation_binding_digest: join_digest_v1(990, 4),
        residual_digest: join_digest_v1(990, 5),
        rlwe_source_residual: &[],
    }
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
        qpcs_bound_transcript_state: fixture_digest_v1(b"qpcs-bound-state", 0),
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
    let mut stage = verify_closure_parts_v1(
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
        stage.qpcs_bound_transcript_state,
        fixture.context.qpcs_bound_transcript_state
    );
    assert_eq!(
        stage.residual_digest(),
        residual_digest_v1(fixture.context, &fixture.residual).expect("residual digest")
    );
    assert_eq!(stage.rlwe_source_residual(), fixture.residual.as_slice());
    let relation_schedule = stage
        .take_relation_schedule_v1()
        .expect("retained relation schedule");
    assert_eq!(relation_schedule.points().len(), 200);
    assert!(!relation_schedule.has_qpcs_relation_lineage_v1());
    assert!(matches!(
        stage.take_relation_schedule_v1(),
        Err(RnsNativeQpcsFriCompleteErrorV1::InvalidOrder)
    ));
    assert!(fixture.shape.aggregate_opened <= MAX_FRI_OPENED_LEAVES_V1);
    assert!(fixture.shape.aggregate_authentication <= MAX_FRI_AUTHENTICATION_HASHES_V1);
    assert!(
        fixture.shape.aggregate_values_bytes + fixture.shape.aggregate_authentication_bytes
            <= ZK_AMS_MKHE_RNS_NATIVE_CORRELATED_FRI_MAX_BYTES_V1 as usize
    );

    let source = include_str!("rns_native_qpcs_fri_complete.rs");
    assert!(source.contains("pub(super) struct RnsNativeQpcsCompletedLineageV1"));
    assert!(source.contains("fn from_completed_fri_v1("));
    assert!(!source.contains("pub(super) fn from_completed_fri_v1("));
    assert!(source.contains("fn take_completed_qpcs_lineage_v1("));
    assert!(source.contains("validate_qpcs_bound_lineage_v1(&qpcs_transcript)"));
    assert!(source.contains(
        "let qpcs_bound_transcript_state = transcript.qpcs_bound_transcript_state_v1();"
    ));
    assert!(source.contains("qpcs_bound_transcript_state: context.qpcs_bound_transcript_state"));
    assert!(
        source.contains("qpcs_transcript.binding_digest() != expected_qpcs_bound_transcript_state")
    );
    let joint = source
        .find("pub(super) struct RnsNativeQpcsCompletedLineageV1")
        .expect("joint completed-qPCS lineage");
    let joint_prefix = &source[joint.saturating_sub(240)..joint];
    assert!(!joint_prefix.contains("derive(Clone"));
    assert!(!joint_prefix.contains("derive(Copy"));
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
fn completed_qpcs_join_requires_the_exact_post_fri_state_and_is_one_shot() {
    let context = 810;
    let transcript = join_challenge_seeds_v1(context);
    let expected_qpcs_bound_state = transcript.qpcs_bound_transcript_state_v1();

    let (matching_schedule, matching_qpcs) =
        join_schedule_and_qpcs_v1(context, JoinQpcsVariantV1::Matching);
    assert_eq!(matching_qpcs.binding_digest(), expected_qpcs_bound_state);
    let mut matching_stage = join_stage_v1(matching_schedule, &transcript);
    let mut completed = matching_stage
        .take_completed_qpcs_lineage_v1(matching_qpcs)
        .expect("exact completed-qPCS join");
    assert!(!matching_stage.has_relation_schedule_v1());
    completed
        .take_qpcs_transcript_v1()
        .expect("matching joined qPCS transcript");

    for variant in [
        JoinQpcsVariantV1::Quotient,
        JoinQpcsVariantV1::Fri(9),
        JoinQpcsVariantV1::Fri(17),
    ] {
        let (schedule, changed_qpcs) = join_schedule_and_qpcs_v1(context, variant);
        schedule
            .validate_qpcs_bound_lineage_v1(&changed_qpcs)
            .expect("same pre-quotient relation lineage");
        assert_ne!(changed_qpcs.binding_digest(), expected_qpcs_bound_state);
        let mut stage = join_stage_v1(schedule, &transcript);
        assert!(matches!(
            stage.take_completed_qpcs_lineage_v1(changed_qpcs),
            Err(RnsNativeQpcsFriCompleteErrorV1::InvalidContext)
        ));
        assert!(!stage.has_relation_schedule_v1());
        let (_, replayed_matching_qpcs) =
            join_schedule_and_qpcs_v1(context, JoinQpcsVariantV1::Matching);
        assert!(matches!(
            stage.take_completed_qpcs_lineage_v1(replayed_matching_qpcs),
            Err(RnsNativeQpcsFriCompleteErrorV1::InvalidOrder)
        ));
    }

    let legacy_schedule = RnsNativeQpcsRelationScheduleV1::test_fixture_with_binding_v1(
        join_digest_v1(context, 402),
        join_digest_v1(context, 401),
        join_digest_v1(context, 800),
        join_digest_v1(context, 801),
        [1; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 * 5],
    );
    let (_, matching_qpcs) = join_schedule_and_qpcs_v1(context, JoinQpcsVariantV1::Matching);
    let mut legacy_stage = join_stage_v1(legacy_schedule, &transcript);
    assert!(matches!(
        legacy_stage.take_completed_qpcs_lineage_v1(matching_qpcs),
        Err(RnsNativeQpcsFriCompleteErrorV1::InvalidContext)
    ));
    assert!(!legacy_stage.has_relation_schedule_v1());
    let (_, replayed_matching_qpcs) =
        join_schedule_and_qpcs_v1(context, JoinQpcsVariantV1::Matching);
    assert!(matches!(
        legacy_stage.take_completed_qpcs_lineage_v1(replayed_matching_qpcs),
        Err(RnsNativeQpcsFriCompleteErrorV1::InvalidOrder)
    ));
}

#[test]
fn pre_auth_claimed_qpcs_validates_before_binding_and_retains_the_one_schedule() {
    let context = 811;
    let (schedule, qpcs) = join_schedule_and_qpcs_v1(context, JoinQpcsVariantV1::Matching);
    let roots = join_terminal_roots_v1(context, &qpcs);
    let expected_qpcs_bound_state = qpcs.binding_digest();
    let claimed = prepare_rns_native_qpcs_pre_auth_claimed_v1(schedule, qpcs, roots)
        .expect("matching pre-auth claimed qPCS");
    assert_eq!(
        claimed.expected_qpcs_bound_transcript_state,
        expected_qpcs_bound_state
    );
    assert!(
        claimed
            .terminal_chronology
            .matches_qpcs_bound_transcript_state_v1(expected_qpcs_bound_state)
    );

    let (foreign_schedule, _) = join_schedule_and_qpcs_v1(context + 1, JoinQpcsVariantV1::Matching);
    let (_, local_qpcs) = join_schedule_and_qpcs_v1(context, JoinQpcsVariantV1::Matching);
    let local_roots = join_terminal_roots_v1(context, &local_qpcs);
    assert!(matches!(
        prepare_rns_native_qpcs_pre_auth_claimed_v1(foreign_schedule, local_qpcs, local_roots,),
        Err(RnsNativeQpcsFriCompleteErrorV1::InvalidContext)
    ));

    let (matching_schedule, matching_qpcs) =
        join_schedule_and_qpcs_v1(context, JoinQpcsVariantV1::Matching);
    let (_, changed_qpcs) = join_schedule_and_qpcs_v1(context, JoinQpcsVariantV1::Fri(17));
    matching_schedule
        .validate_qpcs_bound_lineage_v1(&matching_qpcs)
        .expect("matching relation lineage before root binding");
    let wrong_prior_roots = join_terminal_roots_v1(context, &changed_qpcs);
    assert!(matches!(
        prepare_rns_native_qpcs_pre_auth_claimed_v1(
            matching_schedule,
            matching_qpcs,
            wrong_prior_roots,
        ),
        Err(RnsNativeQpcsFriCompleteErrorV1::InvalidContext)
    ));

    let (matching_schedule, matching_qpcs) =
        join_schedule_and_qpcs_v1(context, JoinQpcsVariantV1::Matching);
    let matching_roots = join_terminal_roots_v1(context, &matching_qpcs);
    let (_, wrong_qpcs) = join_schedule_and_qpcs_v1(context, JoinQpcsVariantV1::Quotient);
    matching_schedule
        .validate_qpcs_bound_lineage_v1(&wrong_qpcs)
        .expect("changed qPCS still has the same pre-quotient lineage");
    assert!(matches!(
        prepare_rns_native_qpcs_pre_auth_claimed_v1(matching_schedule, wrong_qpcs, matching_roots,),
        Err(RnsNativeQpcsFriCompleteErrorV1::InvalidContext)
    ));

    let legacy_schedule = RnsNativeQpcsRelationScheduleV1::test_fixture_with_binding_v1(
        join_digest_v1(context, 402),
        join_digest_v1(context, 401),
        join_digest_v1(context, 800),
        join_digest_v1(context, 801),
        [1; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 * 5],
    );
    let (_, qpcs) = join_schedule_and_qpcs_v1(context, JoinQpcsVariantV1::Matching);
    let roots = join_terminal_roots_v1(context, &qpcs);
    assert!(matches!(
        prepare_rns_native_qpcs_pre_auth_claimed_v1(legacy_schedule, qpcs, roots),
        Err(RnsNativeQpcsFriCompleteErrorV1::InvalidContext)
    ));
}

#[test]
fn authenticated_claimed_qpcs_requires_the_exact_state_and_unescaped_schedule() {
    let context = 812;
    let (schedule, qpcs) = join_schedule_and_qpcs_v1(context, JoinQpcsVariantV1::Matching);
    let roots = join_terminal_roots_v1(context, &qpcs);
    let claimed = prepare_rns_native_qpcs_pre_auth_claimed_v1(schedule, qpcs, roots)
        .expect("matching pre-auth claimed qPCS");
    let RnsNativeQpcsPreAuthClaimedV1 {
        relation_schedule,
        expected_qpcs_bound_transcript_state,
        terminal_chronology,
    } = claimed;
    let qpcs = join_stage_v1(
        relation_schedule,
        terminal_chronology.final_challenge_seeds_v1(),
    );
    let authenticated = finish_rns_native_qpcs_pre_auth_claimed_v1(
        qpcs,
        expected_qpcs_bound_transcript_state,
        terminal_chronology,
    )
    .expect("exact authenticated claimed qPCS state");
    assert!(authenticated.qpcs.has_relation_schedule_v1());
    assert!(
        authenticated
            .terminal_chronology
            .matches_qpcs_bound_transcript_state_v1(expected_qpcs_bound_transcript_state)
    );

    let (schedule, qpcs) = join_schedule_and_qpcs_v1(context, JoinQpcsVariantV1::Matching);
    let roots = join_terminal_roots_v1(context, &qpcs);
    let claimed = prepare_rns_native_qpcs_pre_auth_claimed_v1(schedule, qpcs, roots)
        .expect("replayed pre-auth claimed qPCS");
    let RnsNativeQpcsPreAuthClaimedV1 {
        relation_schedule,
        expected_qpcs_bound_transcript_state,
        terminal_chronology,
    } = claimed;
    let mut wrong_state_stage = join_stage_v1(
        relation_schedule,
        terminal_chronology.final_challenge_seeds_v1(),
    );
    wrong_state_stage.qpcs_bound_transcript_state = join_digest_v1(context, 899);
    assert!(matches!(
        finish_rns_native_qpcs_pre_auth_claimed_v1(
            wrong_state_stage,
            expected_qpcs_bound_transcript_state,
            terminal_chronology,
        ),
        Err(RnsNativeQpcsFriCompleteErrorV1::InvalidContext)
    ));

    let (schedule, qpcs) = join_schedule_and_qpcs_v1(context, JoinQpcsVariantV1::Matching);
    let roots = join_terminal_roots_v1(context, &qpcs);
    let claimed = prepare_rns_native_qpcs_pre_auth_claimed_v1(schedule, qpcs, roots)
        .expect("third replayed pre-auth claimed qPCS");
    let RnsNativeQpcsPreAuthClaimedV1 {
        relation_schedule,
        expected_qpcs_bound_transcript_state: _,
        terminal_chronology,
    } = claimed;
    let exact_stage = join_stage_v1(
        relation_schedule,
        terminal_chronology.final_challenge_seeds_v1(),
    );
    assert!(matches!(
        finish_rns_native_qpcs_pre_auth_claimed_v1(
            exact_stage,
            join_digest_v1(context, 898),
            terminal_chronology,
        ),
        Err(RnsNativeQpcsFriCompleteErrorV1::InvalidContext)
    ));

    let (schedule, qpcs) = join_schedule_and_qpcs_v1(context, JoinQpcsVariantV1::Matching);
    let roots = join_terminal_roots_v1(context, &qpcs);
    let claimed = prepare_rns_native_qpcs_pre_auth_claimed_v1(schedule, qpcs, roots)
        .expect("second replayed pre-auth claimed qPCS");
    let RnsNativeQpcsPreAuthClaimedV1 {
        relation_schedule,
        expected_qpcs_bound_transcript_state,
        terminal_chronology,
    } = claimed;
    let mut escaped_stage = join_stage_v1(
        relation_schedule,
        terminal_chronology.final_challenge_seeds_v1(),
    );
    escaped_stage
        .take_relation_schedule_v1()
        .expect("test-only attempted schedule escape");
    assert!(matches!(
        finish_rns_native_qpcs_pre_auth_claimed_v1(
            escaped_stage,
            expected_qpcs_bound_transcript_state,
            terminal_chronology,
        ),
        Err(RnsNativeQpcsFriCompleteErrorV1::InvalidContext)
    ));

    let (schedule, qpcs) = join_schedule_and_qpcs_v1(context, JoinQpcsVariantV1::Matching);
    let roots = join_terminal_roots_v1(context, &qpcs);
    let claimed = prepare_rns_native_qpcs_pre_auth_claimed_v1(schedule, qpcs, roots)
        .expect("authentication-entry pre-auth owner");
    assert!(
        authenticate_rns_native_qpcs_pre_auth_claimed_v1(claimed, &[], &[], &[], &[],).is_err()
    );
}

#[test]
fn pre_auth_claimed_qpcs_surface_is_move_only_source_only_and_fail_closed() {
    assert!(PRE_AUTH_CLAIMED_QPCS_TYPESTATE_SOURCE_IMPLEMENTED_V1);
    assert!(!PRE_AUTH_CLAIMED_QPCS_INTEGRATED_V1);
    assert!(!PRE_AUTH_CLAIMED_QPCS_VERIFICATION_AUTHORITY_V1);
    assert!(!PRE_AUTH_CLAIMED_QPCS_READINESS_V1);
    assert!(!PRE_AUTH_CLAIMED_QPCS_RELEASE_READY_V1);

    let source = include_str!("rns_native_qpcs_fri_complete.rs");
    for type_name in [
        "RnsNativeQpcsPreAuthClaimedV1",
        "RnsNativeQpcsAuthenticatedClaimedV1",
    ] {
        let declaration = source
            .find(&format!("pub(super) struct {type_name}"))
            .expect("claimed qPCS typestate declaration");
        let prefix = &source[declaration.saturating_sub(320)..declaration];
        assert!(!prefix.contains("derive(Clone"));
        assert!(!prefix.contains("derive(Copy"));
        assert!(!source.contains(&format!("impl Clone for {type_name}")));
        assert!(!source.contains(&format!("impl Copy for {type_name}")));
        assert!(!source.contains(&format!("impl {type_name}")));
    }

    let pre_auth_declaration = source
        .split_once("pub(super) struct RnsNativeQpcsPreAuthClaimedV1")
        .expect("pre-auth owner declaration")
        .1
        .split_once("/// Move-only non-authorizing owner after qPCS authenticates")
        .expect("pre-auth owner boundary")
        .0;
    assert_eq!(
        pre_auth_declaration.matches("relation_schedule:").count(),
        1
    );
    assert!(!pre_auth_declaration.contains("Option<RnsNativeQpcsRelationScheduleV1>"));
    assert!(!pre_auth_declaration.contains("pub relation_schedule"));
    let authenticated_declaration = source
        .split_once("pub(super) struct RnsNativeQpcsAuthenticatedClaimedV1")
        .expect("authenticated claimed owner declaration")
        .1
        .split_once("/// Move-only joint owner proving that a completed qPCS")
        .expect("authenticated claimed owner boundary")
        .0;
    assert!(!authenticated_declaration.contains("relation_schedule:"));

    let prepare = source
        .split_once("pub(super) fn prepare_rns_native_qpcs_pre_auth_claimed_v1(")
        .expect("pre-auth constructor")
        .1
        .split_once("/// Authenticate qPCS using the final seeds")
        .expect("pre-auth constructor boundary")
        .0;
    let lineage_validation = prepare
        .find(".validate_qpcs_bound_lineage_v1(&qpcs_transcript)")
        .expect("lineage validation");
    let root_binding = prepare
        .find(".bind_provisional_terminal_chronology_v1(terminal_roots)")
        .expect("provisional terminal bind");
    assert!(lineage_validation < root_binding);
    assert!(prepare.contains("    relation_schedule: RnsNativeQpcsRelationScheduleV1,"));
    assert!(prepare.contains("    qpcs_transcript: ZkAmsMkheRnsNativeQpcsBoundTranscriptV1,"));
    assert!(!prepare.contains("&RnsNativeQpcsRelationScheduleV1"));
    assert!(!prepare.contains("&ZkAmsMkheRnsNativeQpcsBoundTranscriptV1"));

    let authenticate = source
        .split_once("pub(super) fn authenticate_rns_native_qpcs_pre_auth_claimed_v1")
        .expect("claimed qPCS authentication entry")
        .1
        .split_once("fn finish_rns_native_qpcs_pre_auth_claimed_v1")
        .expect("claimed qPCS authentication boundary")
        .0;
    assert!(authenticate.contains("    claimed: RnsNativeQpcsPreAuthClaimedV1,"));
    assert!(authenticate.contains("relation_schedule,"));
    assert!(authenticate.contains("authenticate_rns_native_qpcs_fri_complete_with_schedule_v1("));
    assert!(authenticate.contains("terminal_chronology.final_challenge_seeds_v1(),"));
    assert!(!authenticate.contains("relation_schedule.clone()"));
    assert!(!authenticate.contains("relation_schedule_v1()"));
    assert!(!authenticate.contains("take_relation_schedule_v1()"));

    let finish = source
        .split_once("fn finish_rns_native_qpcs_pre_auth_claimed_v1")
        .expect("exact-state finish")
        .1
        .split_once("/// Consume the authenticated fold-zero stage")
        .expect("exact-state finish boundary")
        .0;
    assert!(
        finish.contains("qpcs.qpcs_bound_transcript_state != expected_qpcs_bound_transcript_state")
    );
    assert!(finish.contains("qpcs.relation_schedule.is_none()"));
    assert!(finish.contains("matches_qpcs_bound_transcript_state_v1("));
    assert!(!source.contains("pub(super) fn claimed_cross_field_root_v1("));
    assert!(!source.contains("pub(super) fn claimed_global_lookup_root_v1("));
    assert!(!source.contains("pub(super) fn claimed_zero_padding_root_v1("));
    let composite = include_str!("rns_native_composite_verifier.rs");
    assert!(!composite.contains("authenticate_rns_native_qpcs_pre_auth_claimed_v1"));
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

#[test]
fn claimed_source_pair_decode_is_exact_big_endian_and_limb_major() {
    let mut encoded = [0_u8; CLAIMED_SOURCE_QPCS_EVALUATION_BYTES_V1];
    for relation in 0..CLAIMED_SOURCE_RELATIONS_V1 {
        let product = 0x0102_0304_0506_0708_u64 ^ relation as u64;
        let opening_quotient = 0x8899_aabb_ccdd_eeff_u64 ^ (relation as u64).rotate_left(17);
        let offset = relation * CLAIMED_SOURCE_QPCS_PAIR_BYTES_V1;
        encoded[offset..offset + 8].copy_from_slice(&product.to_be_bytes());
        encoded[offset + 8..offset + 16].copy_from_slice(&opening_quotient.to_be_bytes());
    }
    for limb in 0..ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 {
        for repetition in 0..CLAIMED_SOURCE_REPETITIONS_V1 {
            let relation = limb * CLAIMED_SOURCE_REPETITIONS_V1 + repetition;
            assert_eq!(
                claimed_source_qpcs_pair_v1(&encoded, relation),
                Ok((
                    0x0102_0304_0506_0708_u64 ^ relation as u64,
                    0x8899_aabb_ccdd_eeff_u64 ^ (relation as u64).rotate_left(17),
                ))
            );
        }
    }
    assert_eq!(
        claimed_source_qpcs_pair_v1(&encoded[..encoded.len() - 1], 0),
        Err(RnsNativeQpcsClaimedSourceErrorV1::InvalidCount)
    );
    assert_eq!(
        claimed_source_qpcs_pair_v1(&encoded, CLAIMED_SOURCE_RELATIONS_V1),
        Err(RnsNativeQpcsClaimedSourceErrorV1::InvalidCount)
    );
}

fn claimed_source_public_evaluation_kat_v1(
    relation: usize,
) -> RnsNativePublicPolynomialEvaluationV1 {
    RnsNativePublicPolynomialEvaluationV1 {
        public_a: 10_000 + relation as u64,
        public_b: 20_000 + relation as u64,
        ciphertext_c0: core::array::from_fn(|record| 30_000 + (relation * 43 + record) as u64),
        ciphertext_c1: core::array::from_fn(|record| 50_000 + (relation * 43 + record) as u64),
    }
}

fn claimed_source_nonzero_factor_point_kat_v1(relation: usize, modulus: u64) -> (u64, u64) {
    for delta in 0_u64..64 {
        let point = 2 + relation as u64 % 97 + delta;
        let factor =
            claimed_source_mod_add_v1(claimed_source_ring_power_v1(point, modulus), 1, modulus);
        if factor != 0 {
            return (point, factor);
        }
    }
    panic!("bounded KAT could not find a nonzero relation factor")
}

#[test]
fn claimed_source_numeric_tail_rejects_canonical_factor_and_relation_faults() {
    let limb = 0;
    let modulus = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[limb];
    let public = claimed_source_public_evaluation_kat_v1(0);
    let (point, factor) = claimed_source_nonzero_factor_point_kat_v1(0, modulus);
    let opening_quotient = 7;
    let product = claimed_source_mod_mul_v1(factor, opening_quotient, modulus);
    assert_eq!(
        validate_claimed_source_numeric_tail_v1(limb, 0, point, public, product, opening_quotient,)
            .map(RnsNativeQpcsAuthenticatedNumericTailV1::values_v1),
        Ok((point, product, opening_quotient))
    );

    let mut noncanonical_public = public;
    noncanonical_public.public_a = modulus;
    assert!(matches!(
        validate_claimed_source_numeric_tail_v1(
            limb,
            0,
            point,
            noncanonical_public,
            product,
            opening_quotient,
        ),
        Err(RnsNativeQpcsClaimedSourceErrorV1::NonCanonicalResidue)
    ));
    assert!(matches!(
        validate_claimed_source_numeric_tail_v1(limb, 0, point, public, product, modulus,),
        Err(RnsNativeQpcsClaimedSourceErrorV1::NonCanonicalResidue)
    ));
    assert!(matches!(
        validate_claimed_source_numeric_tail_v1(limb, 0, 0, public, product, opening_quotient,),
        Err(RnsNativeQpcsClaimedSourceErrorV1::InvalidPoint)
    ));

    let zero_factor_point = ZK_AMS_MKHE_RNS_NATIVE_NEGACYCLIC_ROOTS_V1[limb];
    assert_eq!(
        claimed_source_ring_power_v1(zero_factor_point, modulus),
        modulus - 1
    );
    assert!(matches!(
        validate_claimed_source_numeric_tail_v1(limb, 0, zero_factor_point, public, 0, 1,),
        Err(RnsNativeQpcsClaimedSourceErrorV1::ZeroFactor)
    ));

    let wrong_product = claimed_source_mod_add_v1(product, 1, modulus);
    assert_ne!(wrong_product, product);
    assert!(matches!(
        validate_claimed_source_numeric_tail_v1(
            limb,
            0,
            point,
            public,
            wrong_product,
            opening_quotient,
        ),
        Err(RnsNativeQpcsClaimedSourceErrorV1::InvalidRelation)
    ));
}

#[test]
fn claimed_source_numeric_tail_kat_materializes_all_200_ordered_rows() {
    let mut encoded = [0_u8; CLAIMED_SOURCE_QPCS_EVALUATION_BYTES_V1];
    let mut points = [0_u64; CLAIMED_SOURCE_RELATIONS_V1];
    let mut public_evaluations = Vec::with_capacity(CLAIMED_SOURCE_RELATIONS_V1);
    for limb in 0..ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 {
        let modulus = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[limb];
        for repetition in 0..CLAIMED_SOURCE_REPETITIONS_V1 {
            let relation = limb * CLAIMED_SOURCE_REPETITIONS_V1 + repetition;
            let (point, factor) = claimed_source_nonzero_factor_point_kat_v1(relation, modulus);
            let opening_quotient = 100 + relation as u64;
            let product = claimed_source_mod_mul_v1(factor, opening_quotient, modulus);
            points[relation] = point;
            public_evaluations.push(claimed_source_public_evaluation_kat_v1(relation));
            let offset = relation * CLAIMED_SOURCE_QPCS_PAIR_BYTES_V1;
            encoded[offset..offset + 8].copy_from_slice(&product.to_be_bytes());
            encoded[offset + 8..offset + 16].copy_from_slice(&opening_quotient.to_be_bytes());
        }
    }
    assert_eq!(public_evaluations.len(), CLAIMED_SOURCE_RELATIONS_V1);

    let mut tails =
        [RnsNativeQpcsAuthenticatedNumericTailV1::UNFILLED; CLAIMED_SOURCE_RELATIONS_V1];
    for limb in 0..ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 {
        for repetition in 0..CLAIMED_SOURCE_REPETITIONS_V1 {
            let relation = limb * CLAIMED_SOURCE_REPETITIONS_V1 + repetition;
            let (product, opening_quotient) =
                claimed_source_qpcs_pair_v1(&encoded, relation).expect("KAT pair");
            tails[relation] = validate_claimed_source_numeric_tail_v1(
                limb,
                repetition,
                points[relation],
                public_evaluations[relation],
                product,
                opening_quotient,
            )
            .expect("KAT numeric tail");
            assert_eq!(
                tails[relation].values_v1(),
                (points[relation], product, opening_quotient)
            );
        }
    }
    assert!(tails.iter().all(|tail| {
        let (a, product, opening_quotient) = tail.values_v1();
        a != u64::MAX && product != u64::MAX && opening_quotient != u64::MAX
    }));
}
