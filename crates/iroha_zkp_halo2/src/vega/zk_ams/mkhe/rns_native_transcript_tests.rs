use super::*;
use crate::vega::zk_ams::mkhe::{
    rns_native_claimed_successor::RnsNativeCrossFieldRlweVerifiedCoreRootV1,
    rns_native_profile::{
        zk_ams_mkhe_rns_native_profile_v1, zk_ams_mkhe_rns_native_release_candidate_digest_v1,
        zk_ams_mkhe_rns_native_topology_v1,
    },
    rns_native_source::{
        ZkAmsMkheRnsNativeSecretChunkV1, ZkAmsMkheRnsNativeSourceArenaV1,
        ZkAmsMkheRnsNativeSourceErrorV1, ZkAmsMkheRnsNativeSourceSnapshotV1,
    },
};

fn digest(label: &[u8], context: u16, ordinal: u16) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.rns-native-transcript.test");
    hash.update(
        &u16::try_from(label.len())
            .expect("test labels fit u16")
            .to_be_bytes(),
    );
    hash.update(label);
    hash.update(&context.to_be_bytes());
    hash.update(&ordinal.to_be_bytes());
    hash.finalize()
}

struct TestChunk {
    arena: ZkAmsMkheRnsNativeSourceArenaV1,
    bytes: [u8; 1],
}

impl ZkAmsMkheRnsNativeSecretChunkV1 for TestChunk {
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

struct TestSnapshot {
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    context: u16,
}

impl ZkAmsMkheRnsNativeSourceSnapshotV1 for TestSnapshot {
    type Chunk = TestChunk;

    fn layout(&self) -> ZkAmsMkheRnsNativeSourceLayoutV1 {
        self.layout
    }

    fn snapshot_digest(&self, arena: ZkAmsMkheRnsNativeSourceArenaV1) -> [u8; 32] {
        match arena {
            ZkAmsMkheRnsNativeSourceArenaV1::Main => {
                digest(b"main-source-snapshot", self.context, 0)
            }
            ZkAmsMkheRnsNativeSourceArenaV1::Nonce => {
                digest(b"nonce-source-snapshot", self.context, 0)
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

struct ContextFixture {
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    receipt: ZkAmsMkheRnsNativeSourceReceiptV1,
    public: ZkAmsMkheRnsNativePublicContextV1,
}

fn context_fixture(context: u16) -> ContextFixture {
    let profile = zk_ams_mkhe_rns_native_profile_v1().expect("canonical profile");
    let topology = zk_ams_mkhe_rns_native_topology_v1().expect("canonical topology");
    let release = zk_ams_mkhe_rns_native_release_candidate_digest_v1().expect("candidate digest");
    let layout = ZkAmsMkheRnsNativeSourceLayoutV1::new(
        profile.profile_digest,
        topology.topology_digest,
        release,
        digest(b"statement", context, 0),
        digest(b"operational-context", context, 0),
    )
    .expect("canonical source layout");
    let receipt = TestSnapshot { layout, context }
        .structural_receipt()
        .expect("structural source receipt");
    let public = ZkAmsMkheRnsNativePublicContextV1::new(
        digest(b"governed-roster", context, 0),
        digest(b"public-ciphertext", context, 0),
    )
    .expect("public context");
    ContextFixture {
        layout,
        receipt,
        public,
    }
}

fn opening_records(context: u16) -> [ZkAmsMkheRnsNativeOpeningCommitmentV1; OPENING_COUNT_V1] {
    core::array::from_fn(|ordinal| {
        let (family, family_index) = opening_role_v1(ordinal).expect("canonical opening ordinal");
        ZkAmsMkheRnsNativeOpeningCommitmentV1::new(
            family,
            family_index,
            digest(
                b"source-commitment",
                context,
                u16::try_from(ordinal).expect("opening ordinal fits u16"),
            ),
            digest(
                b"hyrax-commitment",
                context,
                u16::try_from(ordinal).expect("opening ordinal fits u16"),
            ),
        )
        .expect("canonical commitment record")
    })
}

fn opening_bundle(binding: [u8; 32], context: u16) -> ZkAmsMkheRnsNativeOpeningCommitmentsV1 {
    ZkAmsMkheRnsNativeOpeningCommitmentsV1::new(binding, opening_records(context))
        .expect("ordered openings")
}

fn terminal_bridge(binding: [u8; 32], context: u16) -> ZkAmsMkheRnsNativeTerminalBridgeV1 {
    ZkAmsMkheRnsNativeTerminalBridgeV1::new(
        binding,
        digest(b"mapping-root", context, 0),
        digest(b"terminal-hyrax-root", context, 0),
        digest(b"cross-basis-root", context, 0),
    )
    .expect("terminal bridge")
}

fn qpcs_roots(binding: [u8; 32], context: u16) -> ZkAmsMkheRnsNativeQpcsRootsV1 {
    let fri_roots = core::array::from_fn(|layer| {
        ZkAmsMkheRnsNativeQpcsFriRootV1::new(
            u8::try_from(layer).expect("FRI layer fits u8"),
            digest(
                b"qpcs-fri-root",
                context,
                u16::try_from(layer).expect("FRI layer fits u16"),
            ),
        )
        .expect("typed FRI root")
    });
    ZkAmsMkheRnsNativeQpcsRootsV1::new(
        binding,
        digest(b"qpcs-initial-root", context, 0),
        digest(b"q-mask-s-root", context, 0),
        digest(b"qpcs-quotient-root", context, 0),
        fri_roots,
    )
    .expect("qPCS root schedule")
}

fn terminal_roots(binding: [u8; 32], context: u16) -> ZkAmsMkheRnsNativeTerminalRootsV1 {
    ZkAmsMkheRnsNativeTerminalRootsV1::new(
        binding,
        digest(b"cross-field-root", context, 0),
        digest(b"global-lookup-root", context, 0),
        digest(b"zero-padding-root", context, 0),
    )
    .expect("terminal roots")
}

fn context_stage(fixture: &ContextFixture) -> ZkAmsMkheRnsNativeTranscriptV1 {
    ZkAmsMkheRnsNativeTranscriptV1::new(fixture.layout, fixture.receipt, fixture.public)
        .expect("context transcript")
}

fn commitment_stage(
    fixture: &ContextFixture,
    opening_context: u16,
) -> ZkAmsMkheRnsNativeCommitmentsBoundTranscriptV1 {
    let transcript = context_stage(fixture);
    let openings = opening_bundle(transcript.binding_digest(), opening_context);
    transcript
        .bind_opening_commitments(openings)
        .expect("commitment transcript")
}

fn terminal_stage(
    fixture: &ContextFixture,
    opening_context: u16,
    bridge_context: u16,
) -> ZkAmsMkheRnsNativeTerminalBoundTranscriptV1 {
    let transcript = commitment_stage(fixture, opening_context);
    let bridge = terminal_bridge(transcript.binding_digest(), bridge_context);
    transcript
        .bind_terminal_bridge(bridge)
        .expect("terminal transcript")
}

fn qpcs_stage(
    fixture: &ContextFixture,
    opening_context: u16,
    bridge_context: u16,
    qpcs_context: u16,
) -> ZkAmsMkheRnsNativeQpcsBoundTranscriptV1 {
    let transcript = terminal_stage(fixture, opening_context, bridge_context);
    let roots = qpcs_roots(transcript.binding_digest(), qpcs_context);
    transcript.bind_qpcs_roots(roots).expect("qPCS transcript")
}

fn finish(
    fixture: &ContextFixture,
    opening_context: u16,
    bridge_context: u16,
    qpcs_context: u16,
    terminal_context: u16,
) -> ZkAmsMkheRnsNativeChallengeSeedsV1 {
    let transcript = qpcs_stage(fixture, opening_context, bridge_context, qpcs_context);
    let roots = terminal_roots(transcript.binding_digest(), terminal_context);
    transcript
        .bind_terminal_roots(roots)
        .expect("complete transcript")
}

#[test]
fn canonical_transcript_is_deterministic_and_all_challenges_are_distinct() {
    let fixture = context_fixture(1);
    let first = finish(&fixture, 10, 20, 30, 40);
    let second = finish(&fixture, 10, 20, 30, 40);
    assert_eq!(first, second);

    let ordered = first.ordered_challenge_seeds();
    assert_eq!(
        ordered.len(),
        ZK_AMS_MKHE_RNS_NATIVE_TRANSCRIPT_CHALLENGE_COUNT_V1
    );
    assert!(ordered.iter().all(|seed| *seed != [0; 32]));
    assert!(
        ordered
            .iter()
            .enumerate()
            .all(|(index, seed)| !ordered[index + 1..].contains(seed))
    );
    assert_ne!(first.transcript_digest(), [0; 32]);
    assert!(!ordered.contains(&first.transcript_digest()));
}

#[test]
fn context_and_every_major_proof_stage_bind_the_terminal_digest() {
    let fixture = context_fixture(2);
    let baseline = finish(&fixture, 11, 21, 31, 41).transcript_digest();
    assert_ne!(
        baseline,
        finish(&context_fixture(3), 11, 21, 31, 41).transcript_digest()
    );
    assert_ne!(
        baseline,
        finish(&fixture, 12, 21, 31, 41).transcript_digest()
    );
    assert_ne!(
        baseline,
        finish(&fixture, 11, 22, 31, 41).transcript_digest()
    );
    assert_ne!(
        baseline,
        finish(&fixture, 11, 21, 32, 41).transcript_digest()
    );
    assert_ne!(
        baseline,
        finish(&fixture, 11, 21, 31, 42).transcript_digest()
    );
}

#[test]
fn qpcs_prover_chronology_is_sequential_one_shot_and_matches_convenience() {
    let fixture = context_fixture(4);
    let terminal = terminal_stage(&fixture, 13, 23);
    let roots = qpcs_roots(terminal.binding_digest(), 33);
    let initial_root = roots.initial_root;
    let q_mask_s_root = roots.q_mask_s_root;
    let quotient_root = roots.quotient_root;
    let fri_roots = roots.fri_roots;

    let initial = terminal
        .bind_qpcs_initial_root(initial_root)
        .expect("initial-root stage");
    let state_after_initial = initial.binding_digest();
    let pre_qpcs_rns_seed = initial.rns_aggregation_challenge_seed();
    assert_ne!(pre_qpcs_rns_seed, [0; 32]);
    let mut relation = initial
        .bind_q_mask_s_root(q_mask_s_root)
        .expect("q-mask relation stage");
    let relation_binding = relation
        .take_qpcs_relation_binding()
        .expect("sole relation binding");
    assert_eq!(relation_binding.q_mask_s_root(), q_mask_s_root);
    assert_eq!(
        relation_binding.qpcs_pre_relation_transcript_digest(),
        state_after_initial
    );
    let relation_seed = relation_binding.qpcs_relation_challenge_seed();
    let relation_schedule =
        super::super::rns_native_qpcs_prefix::RnsNativeQpcsRelationScheduleV1::from_relation_binding_v1(
            digest(b"qpcs-parameter", 33, 0),
            relation_binding,
        )
        .expect("schedule from one-shot relation binding");
    assert_eq!(relation_schedule.q_mask_s_root(), q_mask_s_root);
    assert_eq!(relation_schedule.relation_seed(), relation_seed);
    assert_eq!(relation_schedule.points().len(), 200);
    assert!(relation_schedule.has_qpcs_relation_lineage_v1());
    assert!(matches!(
        relation.take_qpcs_relation_binding(),
        Err(ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidChallenge)
    ));

    let mut fri = relation
        .bind_qpcs_quotient_root(quotient_root)
        .expect("quotient/batching stage");
    assert_ne!(fri.qpcs_batching_challenge_seed(), [0; 32]);
    for (layer, root) in fri_roots.into_iter().enumerate() {
        assert_eq!(usize::from(fri.next_fri_layer()), layer);
        fri = fri.bind_qpcs_fri_root(root).expect("ordered FRI root");
        assert_ne!(
            fri.qpcs_fri_fold_challenge_seed(layer as u8)
                .expect("fold seed"),
            [0; 32]
        );
    }
    let split = fri.finish_qpcs_fri_roots().expect("complete split qPCS");
    relation_schedule
        .validate_qpcs_bound_lineage_v1(&split)
        .expect("matching one-shot qPCS lineage");
    assert_ne!(split.qpcs_query_challenge_seed(), [0; 32]);
    assert_ne!(split.cross_field_challenge_seed(), [0; 32]);

    let convenience_terminal = terminal_stage(&fixture, 13, 23);
    let convenience_roots = qpcs_roots(convenience_terminal.binding_digest(), 33);
    let convenience = convenience_terminal
        .bind_qpcs_roots(convenience_roots)
        .expect("convenience qPCS transcript");
    assert_eq!(split.state, convenience.state);
    assert_eq!(split.qpcs_relation_challenge_seed, relation_seed);
    assert_eq!(split.rns_aggregation_challenge_seed, pre_qpcs_rns_seed);

    let second_terminal = terminal_stage(&fixture, 13, 23);
    let second_initial = second_terminal
        .bind_qpcs_initial_root(initial_root)
        .expect("same initial stage");
    assert_eq!(second_initial.binding_digest(), state_after_initial);
    let mut second_relation = second_initial
        .bind_q_mask_s_root(digest(b"changed-q-mask-s-root", 33, 0))
        .expect("changed q-mask relation stage");
    let second_binding = second_relation
        .take_qpcs_relation_binding()
        .expect("changed relation binding");
    assert_ne!(relation_seed, second_binding.qpcs_relation_challenge_seed());
    let second_schedule =
        super::super::rns_native_qpcs_prefix::RnsNativeQpcsRelationScheduleV1::from_relation_binding_v1(
            digest(b"qpcs-parameter", 33, 0),
            second_binding,
        )
        .expect("changed relation schedule");
    assert!(matches!(
        second_schedule.validate_qpcs_bound_lineage_v1(&split),
        Err(super::super::rns_native_qpcs_prefix::RnsNativeQpcsPrefixErrorV1::InvalidContext)
    ));

    let unissued = terminal_stage(&fixture, 13, 23)
        .bind_qpcs_initial_root(initial_root)
        .expect("unissued initial stage")
        .bind_q_mask_s_root(q_mask_s_root)
        .expect("unissued relation stage");
    assert!(matches!(
        unissued.bind_qpcs_quotient_root(quotient_root),
        Err(ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidChallenge)
    ));

    let source = include_str!("rns_native_transcript.rs");
    for required in [
        "bind_qpcs_initial_root(",
        "bind_q_mask_s_root(",
        "take_qpcs_relation_binding(",
        "bind_qpcs_quotient_root(",
        "bind_qpcs_fri_root(",
        "finish_qpcs_fri_roots(",
    ] {
        assert!(source.contains(required), "missing staged API: {required}");
    }
    assert!(
        !source
            .contains("qpcs_pre_relation_transcript_digest(\n        &self,\n        initial_root")
    );
}

#[test]
fn split_qpcs_rejects_skipped_fri_and_early_finish() {
    let fixture = context_fixture(5);
    let terminal = terminal_stage(&fixture, 14, 24);
    let roots = qpcs_roots(terminal.binding_digest(), 34);
    let mut relation = terminal
        .bind_qpcs_initial_root(roots.initial_root)
        .expect("initial stage")
        .bind_q_mask_s_root(roots.q_mask_s_root)
        .expect("relation stage");
    let _ = relation
        .take_qpcs_relation_binding()
        .expect("relation binding");
    let fri = relation
        .bind_qpcs_quotient_root(roots.quotient_root)
        .expect("FRI stage");
    assert!(matches!(
        fri.bind_qpcs_fri_root(roots.fri_roots[1]),
        Err(ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidRootOrder)
    ));

    let terminal = terminal_stage(&fixture, 14, 24);
    let roots = qpcs_roots(terminal.binding_digest(), 34);
    let mut relation = terminal
        .bind_qpcs_initial_root(roots.initial_root)
        .expect("second initial stage")
        .bind_q_mask_s_root(roots.q_mask_s_root)
        .expect("second relation stage");
    let _ = relation
        .take_qpcs_relation_binding()
        .expect("second relation binding");
    let fri = relation
        .bind_qpcs_quotient_root(roots.quotient_root)
        .expect("second FRI stage");
    assert!(matches!(
        fri.finish_qpcs_fri_roots(),
        Err(ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidRootOrder)
    ));
}

#[test]
fn terminal_producer_chronology_is_sequential_and_matches_convenience() {
    let fixture = context_fixture(6);
    let qpcs = qpcs_stage(&fixture, 15, 25, 35);
    let qpcs_bound_state = qpcs.binding_digest();
    let roots = terminal_roots(qpcs.binding_digest(), 45);

    let cross_field = qpcs
        .bind_cross_field_root(roots.cross_field_root)
        .expect("cross-field stage");
    let global_lookup_binding = cross_field.binding_digest();
    let global_lookup_seed = cross_field.global_lookup_challenge_seed();
    assert_ne!(global_lookup_binding, [0; 32]);
    assert_ne!(global_lookup_seed, [0; 32]);

    let global_lookup = cross_field
        .bind_global_lookup_root(roots.global_lookup_root)
        .expect("global-lookup stage");
    let zero_padding_binding = global_lookup.binding_digest();
    let zero_padding_seed = global_lookup.zero_padding_challenge_seed();
    assert_ne!(zero_padding_binding, [0; 32]);
    assert_ne!(zero_padding_seed, [0; 32]);
    assert_ne!(global_lookup_binding, zero_padding_binding);
    assert_ne!(global_lookup_seed, zero_padding_seed);

    let staged = global_lookup
        .bind_zero_padding_root(roots.zero_padding_root)
        .expect("zero-padding stage");
    let convenience_qpcs = qpcs_stage(&fixture, 15, 25, 35);
    assert_eq!(convenience_qpcs.binding_digest(), qpcs_bound_state);
    let convenience_roots = terminal_roots(convenience_qpcs.binding_digest(), 45);
    let convenience = convenience_qpcs
        .bind_terminal_roots(convenience_roots)
        .expect("terminal-root convenience");
    assert_eq!(staged, convenience);
    assert_eq!(staged.qpcs_bound_transcript_state_v1(), qpcs_bound_state);
    assert_eq!(staged.global_lookup_challenge_seed(), global_lookup_seed);
    assert_eq!(staged.zero_padding_challenge_seed(), zero_padding_seed);
}

#[test]
fn claimed_cross_field_root_is_provisional_and_equality_is_one_shot() {
    let fixture = context_fixture(61);
    let qpcs = qpcs_stage(&fixture, 71, 81, 91);
    let roots = terminal_roots(qpcs.binding_digest(), 101);
    let claimed_root = roots.cross_field_root();
    let expected_global = roots.global_lookup_root();
    let expected_zero = roots.zero_padding_root();
    let (claim, remaining) = roots.into_cross_field_claim_v1();
    assert_eq!(remaining.global_lookup_root(), expected_global);
    assert_eq!(remaining.zero_padding_root(), expected_zero);
    let (provisional, obligation) = qpcs
        .bind_claimed_cross_field_root_v1(claim)
        .expect("typed cross-field claim");
    assert_ne!(provisional.binding_digest(), [0; 32]);
    let wrong_root = RnsNativeCrossFieldRlweVerifiedCoreRootV1::test_fixture_v1(digest(
        b"wrong-cross-field-root",
        101,
        0,
    ))
    .expect("opaque wrong-root fixture");
    assert!(matches!(
        obligation.discharge_v1(wrong_root),
        Err(ZkAmsMkheRnsNativeTranscriptErrorV1::ContextMismatch)
    ));

    let qpcs = qpcs_stage(&fixture, 71, 81, 91);
    let roots = terminal_roots(qpcs.binding_digest(), 101);
    let (claim, _) = roots.into_cross_field_claim_v1();
    let (_, obligation) = qpcs
        .bind_claimed_cross_field_root_v1(claim)
        .expect("replayed typed cross-field claim");
    let verified_root = RnsNativeCrossFieldRlweVerifiedCoreRootV1::test_fixture_v1(claimed_root)
        .expect("opaque verified-root fixture");
    obligation
        .discharge_v1(verified_root)
        .expect("matching recomputed root");

    let first_qpcs = qpcs_stage(&fixture, 72, 82, 92);
    let foreign_qpcs = qpcs_stage(&fixture, 73, 83, 93);
    let foreign_roots = terminal_roots(foreign_qpcs.binding_digest(), 102);
    let (foreign_claim, _) = foreign_roots.into_cross_field_claim_v1();
    assert!(matches!(
        first_qpcs.bind_claimed_cross_field_root_v1(foreign_claim),
        Err(ZkAmsMkheRnsNativeTranscriptErrorV1::ContextMismatch)
    ));
}

#[test]
fn pre_global_capability_snapshots_exact_stage_and_binds_remaining_roots() {
    let fixture = context_fixture(62);
    let qpcs = qpcs_stage(&fixture, 72, 82, 92);
    let qpcs_binding = qpcs.binding_digest();
    let roots = terminal_roots(qpcs_binding, 102);
    let claimed_root = roots.cross_field_root();
    let expected_global_root = roots.global_lookup_root();
    let expected_zero_root = roots.zero_padding_root();
    let (claim, remaining_roots) = roots.into_cross_field_claim_v1();
    let (cross_field, obligation) = qpcs
        .bind_claimed_cross_field_root_v1(claim)
        .expect("matching claimed root");
    let expected_pre_global_binding = cross_field.binding_digest();
    let expected_global_seed = cross_field.global_lookup_challenge_seed();

    let (capability, final_seeds) = cross_field
        .bind_remaining_terminal_roots_v1(remaining_roots)
        .expect("canonical remaining terminal roots");
    assert_eq!(
        capability.test_post_cross_field_binding_digest_v1(),
        expected_pre_global_binding
    );
    assert_eq!(
        capability.test_global_lookup_challenge_seed_v1(),
        expected_global_seed
    );
    let sole_z_binding = capability
        .sole_z_binding_digest_v1()
        .expect("opaque sole-z binding");
    assert_ne!(sole_z_binding, [0; 32]);
    let matching_fixture = ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1::test_fixture_v1(
        expected_pre_global_binding,
        expected_global_seed,
    )
    .expect("matching capability fixture");
    assert_eq!(
        matching_fixture
            .sole_z_binding_digest_v1()
            .expect("matching opaque sole-z binding"),
        sole_z_binding
    );
    let changed_fixture = ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1::test_fixture_v1(
        expected_pre_global_binding,
        digest(b"foreign-global-seed", 62, 0),
    )
    .expect("changed capability fixture");
    assert_ne!(
        changed_fixture
            .sole_z_binding_digest_v1()
            .expect("changed opaque sole-z binding"),
        sole_z_binding
    );
    assert!(
        ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1::test_fixture_v1(
            [0; 32],
            expected_global_seed,
        )
        .is_err()
    );
    assert!(
        ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1::test_fixture_v1(
            expected_global_seed,
            expected_global_seed,
        )
        .is_err()
    );
    assert_eq!(final_seeds.cross_field_root(), claimed_root);
    assert_eq!(final_seeds.global_lookup_root(), expected_global_root);
    assert_eq!(final_seeds.zero_padding_root(), expected_zero_root);
    assert_eq!(
        final_seeds.global_lookup_challenge_seed(),
        expected_global_seed
    );

    let convenience_qpcs = qpcs_stage(&fixture, 72, 82, 92);
    assert_eq!(convenience_qpcs.binding_digest(), qpcs_binding);
    let convenience_roots = terminal_roots(convenience_qpcs.binding_digest(), 102);
    let convenience = convenience_qpcs
        .bind_terminal_roots(convenience_roots)
        .expect("terminal-root convenience");
    assert_eq!(final_seeds, convenience);

    obligation
        .discharge_v1(
            RnsNativeCrossFieldRlweVerifiedCoreRootV1::test_fixture_v1(claimed_root)
                .expect("matching opaque direct root"),
        )
        .expect("matching equality obligation");
}

#[test]
fn terminal_challenges_are_derived_before_and_sensitive_to_dependent_roots() {
    let fixture = context_fixture(7);
    let cross_field_root = digest(b"cross-field-root", 46, 0);
    let changed_cross_field_root = digest(b"changed-cross-field-root", 46, 0);
    let global_lookup_root = digest(b"global-lookup-root", 46, 0);
    let changed_global_lookup_root = digest(b"changed-global-lookup-root", 46, 0);
    let zero_padding_root = digest(b"zero-padding-root", 46, 0);
    let changed_zero_padding_root = digest(b"changed-zero-padding-root", 46, 0);

    let cross_field = qpcs_stage(&fixture, 16, 26, 36)
        .bind_cross_field_root(cross_field_root)
        .expect("baseline cross-field stage");
    let repeated_cross_field = qpcs_stage(&fixture, 16, 26, 36)
        .bind_cross_field_root(cross_field_root)
        .expect("repeated cross-field stage");
    let changed_cross_field = qpcs_stage(&fixture, 16, 26, 36)
        .bind_cross_field_root(changed_cross_field_root)
        .expect("changed cross-field stage");
    assert_eq!(
        cross_field.global_lookup_challenge_seed(),
        repeated_cross_field.global_lookup_challenge_seed()
    );
    assert_ne!(
        cross_field.global_lookup_challenge_seed(),
        changed_cross_field.global_lookup_challenge_seed()
    );
    assert_ne!(
        cross_field.binding_digest(),
        changed_cross_field.binding_digest()
    );

    let global_lookup = cross_field
        .bind_global_lookup_root(global_lookup_root)
        .expect("baseline global-lookup stage");
    let repeated_global_lookup = repeated_cross_field
        .bind_global_lookup_root(global_lookup_root)
        .expect("repeated global-lookup stage");
    let changed_global_lookup = qpcs_stage(&fixture, 16, 26, 36)
        .bind_cross_field_root(cross_field_root)
        .expect("third cross-field stage")
        .bind_global_lookup_root(changed_global_lookup_root)
        .expect("changed global-lookup stage");
    assert_eq!(
        global_lookup.zero_padding_challenge_seed(),
        repeated_global_lookup.zero_padding_challenge_seed()
    );
    assert_ne!(
        global_lookup.zero_padding_challenge_seed(),
        changed_global_lookup.zero_padding_challenge_seed()
    );
    assert_ne!(
        global_lookup.binding_digest(),
        changed_global_lookup.binding_digest()
    );

    let baseline = global_lookup
        .bind_zero_padding_root(zero_padding_root)
        .expect("baseline final stage");
    let changed = repeated_global_lookup
        .bind_zero_padding_root(changed_zero_padding_root)
        .expect("changed final stage");
    assert_ne!(
        baseline.composite_binding_challenge_seed(),
        changed.composite_binding_challenge_seed()
    );
    assert_ne!(baseline.transcript_digest(), changed.transcript_digest());
}

#[test]
fn terminal_stages_reject_digest_reuse_and_expose_no_out_of_order_path() {
    let fixture = context_fixture(8);
    let qpcs = qpcs_stage(&fixture, 17, 27, 37);
    let reused_qpcs_root = qpcs.qpcs_initial_root;
    assert!(matches!(
        qpcs.bind_cross_field_root(reused_qpcs_root),
        Err(ZkAmsMkheRnsNativeTranscriptErrorV1::DuplicateDigest)
    ));

    let cross_field_root = digest(b"cross-field-root", 47, 0);
    let cross_field = qpcs_stage(&fixture, 17, 27, 37)
        .bind_cross_field_root(cross_field_root)
        .expect("cross-field stage");
    assert!(matches!(
        cross_field.bind_global_lookup_root(cross_field_root),
        Err(ZkAmsMkheRnsNativeTranscriptErrorV1::DuplicateDigest)
    ));

    let global_lookup_root = digest(b"global-lookup-root", 47, 0);
    let global_lookup = qpcs_stage(&fixture, 17, 27, 37)
        .bind_cross_field_root(cross_field_root)
        .expect("second cross-field stage")
        .bind_global_lookup_root(global_lookup_root)
        .expect("global-lookup stage");
    assert!(matches!(
        global_lookup.bind_zero_padding_root(global_lookup_root),
        Err(ZkAmsMkheRnsNativeTranscriptErrorV1::DuplicateDigest)
    ));

    let source = include_str!("rns_native_transcript.rs");
    for consuming_signature in [
        "fn bind_cross_field_root(\n        mut self,",
        "fn bind_global_lookup_root(\n        mut self,",
        "fn bind_zero_padding_root(\n        mut self,",
    ] {
        assert!(
            source.contains(consuming_signature),
            "missing move-only transition: {consuming_signature}"
        );
    }
    let qpcs_impl = source
        .split_once("impl ZkAmsMkheRnsNativeQpcsBoundTranscriptV1 {")
        .expect("qPCS-bound implementation")
        .1
        .split_once("/// Move-only transcript after the cross-field root")
        .expect("cross-field stage boundary")
        .0;
    assert!(qpcs_impl.contains("fn bind_cross_field_root("));
    assert!(!qpcs_impl.contains("fn bind_global_lookup_root("));
    assert!(!qpcs_impl.contains("fn bind_zero_padding_root("));

    let cross_field_impl = source
        .split_once("impl ZkAmsMkheRnsNativeCrossFieldBoundTranscriptV1 {")
        .expect("cross-field implementation")
        .1
        .split_once("/// Move-only transcript after the global-lookup root")
        .expect("global-lookup stage boundary")
        .0;
    assert!(cross_field_impl.contains("fn bind_global_lookup_root("));
    assert!(!cross_field_impl.contains("fn bind_zero_padding_root("));

    let global_lookup_impl = source
        .split_once("impl ZkAmsMkheRnsNativeGlobalLookupBoundTranscriptV1 {")
        .expect("global-lookup implementation")
        .1
        .split_once("/// Final domain-separated challenge seeds")
        .expect("final challenge boundary")
        .0;
    assert!(global_lookup_impl.contains("fn bind_zero_padding_root("));
    assert!(source.contains(
        ".bind_cross_field_root(roots.cross_field_root)?\n            .bind_global_lookup_root(roots.global_lookup_root)?\n            .bind_zero_padding_root(roots.zero_padding_root)"
    ));
}

#[test]
fn zero_and_duplicate_semantic_digests_fail_closed() {
    assert!(matches!(
        ZkAmsMkheRnsNativePublicContextV1::new([0; 32], digest(b"ciphertext", 1, 0)),
        Err(ZkAmsMkheRnsNativeTranscriptErrorV1::ZeroDigest)
    ));
    assert!(matches!(
        ZkAmsMkheRnsNativePublicContextV1::new([7; 32], [7; 32]),
        Err(ZkAmsMkheRnsNativeTranscriptErrorV1::DuplicateDigest)
    ));
    assert!(matches!(
        ZkAmsMkheRnsNativeOpeningCommitmentV1::new(
            ZkAmsMkheRnsNativeFamilyV1::X,
            0,
            [0; 32],
            [9; 32],
        ),
        Err(ZkAmsMkheRnsNativeTranscriptErrorV1::ZeroDigest)
    ));

    let mut records = opening_records(50);
    records[1].source_commitment_digest = records[0].source_commitment_digest;
    assert!(matches!(
        ZkAmsMkheRnsNativeOpeningCommitmentsV1::new([0xa5; 32], records),
        Err(ZkAmsMkheRnsNativeTranscriptErrorV1::DuplicateDigest)
    ));
    assert!(matches!(
        ZkAmsMkheRnsNativeTerminalBridgeV1::new([1; 32], [2; 32], [2; 32], [3; 32]),
        Err(ZkAmsMkheRnsNativeTranscriptErrorV1::DuplicateDigest)
    ));

    let mut fri_roots = core::array::from_fn(|layer| {
        ZkAmsMkheRnsNativeQpcsFriRootV1::new(
            u8::try_from(layer).expect("FRI layer fits u8"),
            digest(
                b"duplicate-test-fri",
                1,
                u16::try_from(layer).expect("FRI layer fits u16"),
            ),
        )
        .expect("root")
    });
    fri_roots[1].root = fri_roots[0].root;
    assert!(matches!(
        ZkAmsMkheRnsNativeQpcsRootsV1::new([1; 32], [2; 32], [3; 32], [4; 32], fri_roots,),
        Err(ZkAmsMkheRnsNativeTranscriptErrorV1::DuplicateDigest)
    ));
    assert!(matches!(
        ZkAmsMkheRnsNativeTerminalRootsV1::new([1; 32], [2; 32], [3; 32], [2; 32]),
        Err(ZkAmsMkheRnsNativeTranscriptErrorV1::DuplicateDigest)
    ));
}

#[test]
fn opening_and_fri_reordering_is_rejected() {
    let mut records = opening_records(60);
    records.swap(0, 1);
    assert!(matches!(
        ZkAmsMkheRnsNativeOpeningCommitmentsV1::new([0xb5; 32], records),
        Err(ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidOpeningOrder)
    ));
    assert!(matches!(
        ZkAmsMkheRnsNativeOpeningCommitmentV1::new(
            ZkAmsMkheRnsNativeFamilyV1::X,
            1,
            [1; 32],
            [2; 32],
        ),
        Err(ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidOpening)
    ));

    let mut fri_roots = core::array::from_fn(|layer| {
        ZkAmsMkheRnsNativeQpcsFriRootV1::new(
            u8::try_from(layer).expect("FRI layer fits u8"),
            digest(
                b"reorder-test-fri",
                1,
                u16::try_from(layer).expect("FRI layer fits u16"),
            ),
        )
        .expect("root")
    });
    fri_roots.swap(3, 4);
    assert!(matches!(
        ZkAmsMkheRnsNativeQpcsRootsV1::new([1; 32], [2; 32], [3; 32], [4; 32], fri_roots,),
        Err(ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidRootOrder)
    ));
    assert!(matches!(
        ZkAmsMkheRnsNativeQpcsFriRootV1::new(18, [1; 32]),
        Err(ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidRoot)
    ));
}

#[test]
fn source_receipt_and_all_prior_stage_splices_are_rejected() {
    let first = context_fixture(70);
    let second = context_fixture(71);
    assert!(matches!(
        ZkAmsMkheRnsNativeTranscriptV1::new(second.layout, first.receipt, second.public),
        Err(ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidSourceContext)
    ));

    let first_context = context_stage(&first);
    let foreign_openings = opening_bundle(first_context.binding_digest(), 72);
    let second_context = context_stage(&second);
    assert!(matches!(
        second_context.bind_opening_commitments(foreign_openings),
        Err(ZkAmsMkheRnsNativeTranscriptErrorV1::ContextMismatch)
    ));

    let first_commitments = commitment_stage(&first, 73);
    let foreign_bridge = terminal_bridge(first_commitments.binding_digest(), 74);
    let second_commitments = commitment_stage(&first, 75);
    assert!(matches!(
        second_commitments.bind_terminal_bridge(foreign_bridge),
        Err(ZkAmsMkheRnsNativeTranscriptErrorV1::ContextMismatch)
    ));

    let first_terminal = terminal_stage(&first, 76, 77);
    let foreign_qpcs = qpcs_roots(first_terminal.binding_digest(), 78);
    let second_terminal = terminal_stage(&first, 76, 79);
    assert!(matches!(
        second_terminal.bind_qpcs_roots(foreign_qpcs),
        Err(ZkAmsMkheRnsNativeTranscriptErrorV1::ContextMismatch)
    ));

    let first_qpcs = qpcs_stage(&first, 80, 81, 82);
    let foreign_terminal = terminal_roots(first_qpcs.binding_digest(), 83);
    let second_qpcs = qpcs_stage(&first, 80, 81, 84);
    assert!(matches!(
        second_qpcs.bind_terminal_roots(foreign_terminal),
        Err(ZkAmsMkheRnsNativeTranscriptErrorV1::ContextMismatch)
    ));
}

#[test]
fn duplicate_digest_across_stage_boundaries_is_rejected() {
    let fixture = context_fixture(90);
    let transcript = commitment_stage(&fixture, 91);
    let reused = opening_records(91)[0].source_commitment_digest;
    let bridge = ZkAmsMkheRnsNativeTerminalBridgeV1::new(
        transcript.binding_digest(),
        reused,
        digest(b"fresh-terminal-root", 91, 0),
        digest(b"fresh-cross-basis-root", 91, 0),
    )
    .expect("locally distinct bridge");
    assert!(matches!(
        transcript.bind_terminal_bridge(bridge),
        Err(ZkAmsMkheRnsNativeTranscriptErrorV1::DuplicateDigest)
    ));
}

#[test]
fn transcript_contract_remains_non_authorizing_and_stages_are_move_only() {
    let source = include_str!("rns_native_transcript.rs");
    for stage in [
        "pub struct ZkAmsMkheRnsNativeTranscriptV1",
        "pub struct ZkAmsMkheRnsNativeCommitmentsBoundTranscriptV1",
        "pub struct ZkAmsMkheRnsNativeTerminalBoundTranscriptV1",
        "pub(super) struct ZkAmsMkheRnsNativeQpcsInitialBoundTranscriptV1",
        "pub(super) struct ZkAmsMkheRnsNativeQpcsRelationBindingV1",
        "pub(super) struct ZkAmsMkheRnsNativeQpcsRelationLineageV1",
        "pub(super) struct ZkAmsMkheRnsNativeCrossFieldRootClaimV1",
        "pub(super) struct ZkAmsMkheRnsNativeCrossFieldRootEqualityObligationV1",
        "pub(super) struct ZkAmsMkheRnsNativeRemainingTerminalRootsV1",
        "pub(super) struct ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1",
        "pub(super) struct ZkAmsMkheRnsNativeQpcsPreRelationTranscriptV1",
        "pub(super) struct ZkAmsMkheRnsNativeQpcsFriTranscriptV1",
        "pub struct ZkAmsMkheRnsNativeQpcsBoundTranscriptV1",
        "pub(super) struct ZkAmsMkheRnsNativeCrossFieldBoundTranscriptV1",
        "pub(super) struct ZkAmsMkheRnsNativeGlobalLookupBoundTranscriptV1",
    ] {
        let declaration = source.find(stage).expect("stage declaration");
        let prefix = &source[declaration.saturating_sub(100)..declaration];
        assert!(!prefix.contains("derive(Clone"));
        assert!(!prefix.contains("derive(Copy"));
    }
    assert!(!source.contains("authorizes_release"));
    assert!(!source.contains("proof_verified"));
    assert!(!source.contains("readiness_authority"));
    let cross_root_bind = source
        .split_once("pub(super) fn bind_cross_field_root(")
        .expect("qPCS cross-root bind")
        .1
        .split_once("/// Verifier convenience: consume the qPCS stage")
        .expect("qPCS cross-root bind boundary")
        .0;
    let exact_qpcs_state = cross_root_bind
        .find("let qpcs_bound_transcript_state = self.state;")
        .expect("exact pre-cross-root qPCS state capture");
    let cross_root_absorb = cross_root_bind
        .find("AbsorbKindV1::CrossField")
        .expect("cross-root absorption");
    assert!(exact_qpcs_state < cross_root_absorb);
    for propagation in [
        "qpcs_bound_transcript_state: self.qpcs_bound_transcript_state",
        "pub(super) const fn qpcs_bound_transcript_state_v1(&self)",
    ] {
        assert!(
            source.contains(propagation),
            "missing exact qPCS-state propagation: {propagation}"
        );
    }
    let equality_discharge = source
        .split_once("impl ZkAmsMkheRnsNativeCrossFieldRootEqualityObligationV1")
        .expect("root equality obligation implementation")
        .1
        .split_once("/// Move-only transcript after canonical context/source validation.")
        .expect("root equality obligation boundary")
        .0;
    assert!(
        equality_discharge.contains("recomputed_root: RnsNativeCrossFieldRlweVerifiedCoreRootV1")
    );
    assert!(!equality_discharge.contains("fn discharge_v1<R>"));
    assert!(!equality_discharge.contains("recomputed_root: [u8; 32]"));
    assert!(!source.contains("trait ZkAmsMkheRnsNativeVerifiedCrossFieldCoreRootCapabilityV1"));
    assert!(!source.contains("into_verified_cross_field_core_root_v1"));
    assert!(!source.contains("struct ZkAmsMkheRnsNativeVerifiedCrossFieldCoreRootV1"));

    let claimed_facade = include_str!("rns_native_claimed_successor.rs");
    let verified_root_declaration = claimed_facade
        .find("pub(super) struct RnsNativeCrossFieldRlweVerifiedCoreRootV1(")
        .expect("sealed verified-root typestate");
    let verified_root_prefix =
        &claimed_facade[verified_root_declaration.saturating_sub(320)..verified_root_declaration];
    assert!(!verified_root_prefix.contains("derive(Clone"));
    assert!(!verified_root_prefix.contains("derive(Copy"));
    let verified_root_surface = claimed_facade[verified_root_declaration..]
        .split_once("pub(super) struct RnsNativeCrossFieldRlweClaimedInventoryParentV1")
        .expect("sealed verified-root implementation boundary")
        .0;
    assert!(verified_root_surface.contains("fn matches_claimed_cross_field_root_v1("));
    assert!(!verified_root_surface.contains("pub(super) fn new"));
    assert!(!verified_root_surface.contains("pub(super) const fn root"));
    let fixture = verified_root_surface
        .find("pub(super) fn test_fixture_v1(")
        .expect("cfg(test)-only sealed verified-root fixture");
    assert!(verified_root_surface[fixture.saturating_sub(32)..fixture].contains("#[cfg(test)]"));
}

#[test]
fn pre_global_capability_surface_is_opaque_move_only_snapshot_and_source_ordered() {
    let source = include_str!("rns_native_transcript.rs");
    let declaration = source
        .find("pub(super) struct ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1")
        .expect("pre-global capability declaration");
    let prefix = &source[declaration.saturating_sub(320)..declaration];
    assert!(!prefix.contains("derive(Clone"));
    assert!(!prefix.contains("derive(Copy"));
    assert!(!source.contains("impl Clone for ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1"));
    assert!(!source.contains("impl Copy for ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1"));
    let surface = source[declaration..]
        .split_once("/// Opaque one-shot obligation equating an encoded cross-field root claim")
        .expect("pre-global capability boundary")
        .0;
    assert!(!surface.contains("pub(super) fn new"));
    assert!(!surface.contains("pub(super) fn from"));
    assert!(!surface.contains("fn post_cross_field_binding_digest_v1("));
    assert!(!surface.contains("fn global_lookup_challenge_seed_v1("));
    let digest_method = surface
        .split_once("pub(super) fn sole_z_binding_digest_v1(")
        .expect("opaque sole-z digest")
        .1
        .split_once("#[cfg(test)]")
        .expect("production/test capability boundary")
        .0;
    let domain = digest_method
        .find("PRE_GLOBAL_CAPABILITY_BINDING_DOMAIN_V1")
        .expect("domain-separated capability digest");
    let binding = digest_method
        .find("self.post_cross_field_binding_digest")
        .expect("exact post-cross binding");
    let seed = digest_method
        .find("self.global_lookup_challenge_seed")
        .expect("exact global seed");
    assert!(domain < binding && binding < seed);
    let fixture = surface
        .find("pub(super) fn test_fixture_v1(")
        .expect("test-only capability fixture");
    let test_impl = surface[..fixture]
        .rfind("impl ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1")
        .expect("test-only capability implementation");
    assert!(surface[test_impl.saturating_sub(24)..test_impl].contains("#[cfg(test)]"));
    assert!(source.contains("#[cfg(test)]\nimpl ZkAmsMkheRnsNativeRemainingTerminalRootsV1"));

    let transition = source
        .split_once("pub(super) fn bind_remaining_terminal_roots_v1(")
        .expect("atomic remaining-root transition")
        .1
        .split_once("/// Consume this stage, bind the global-lookup root")
        .expect("atomic transition boundary")
        .0;
    assert!(transition.contains("        self,"));
    assert!(!transition.contains("&mut self"));
    let snapshot = transition
        .find("let pre_global_capability")
        .expect("pre-global snapshot");
    let binding = transition
        .find("post_cross_field_binding_digest: self.state")
        .expect("exact post-cross binding");
    let global_seed = transition
        .find("global_lookup_challenge_seed: self.global_lookup_challenge_seed")
        .expect("exact global seed");
    let global_root = transition
        .find(".bind_global_lookup_root(roots.global_lookup_root)")
        .expect("global-root bind");
    let zero_root = transition
        .find(".bind_zero_padding_root(roots.zero_padding_root)")
        .expect("zero-root bind");
    let paired_return = transition
        .find("Ok((pre_global_capability, final_challenge_seeds))")
        .expect("paired return");
    assert!(
        snapshot < binding
            && binding < global_seed
            && global_seed < global_root
            && global_root < zero_root
            && zero_root < paired_return
    );
}

#[test]
fn qpcs_staging_is_settled_while_direct_activation_remains_fail_closed() {
    let transcript = include_str!("rns_native_transcript.rs");
    for staged_transition in [
        "bind_qpcs_initial_root(",
        "bind_q_mask_s_root(",
        "take_qpcs_relation_binding(",
        "bind_qpcs_quotient_root(",
        "bind_qpcs_fri_root(",
        "finish_qpcs_fri_roots(",
    ] {
        assert!(transcript.contains(staged_transition));
    }

    let facade = include_str!("../mkhe.rs");
    assert!(facade.contains(
        "#[path = \"mkhe/rns_native_claimed_successor.rs\"]\nmod rns_native_claimed_successor;"
    ));
    assert!(!facade.contains("pub mod rns_native_claimed_successor;"));
    assert!(!facade.contains("rns_native_cross_field_rlwe_direct"));
    for retired_module in [
        "rns_native_centering_subtraction_relation",
        "rns_native_existing_radix_commitment_view",
        "rns_native_global_lookup_z_commitment_view",
        "rns_native_public_polynomial_publisher",
        "rns_native_public_polynomial_reader",
        "rns_native_q_mask_linear_relations",
        "rns_native_radix_complement_linear_relation",
        "rns_native_source_packing_same_opening",
    ] {
        assert!(!facade.contains(&format!("mod {retired_module};")));
    }
    let composite = include_str!("rns_native_composite_verifier.rs");
    let cross_field_adapter = composite
        .split_once("fn verify_cross_field_global_lookup_production_v1(")
        .expect("cross-field production adapter")
        .1
        .split_once("fn verify_zero_padding_production_v1(")
        .expect("cross-field production adapter boundary")
        .0;
    assert!(cross_field_adapter.contains("StageUnavailable"));
    assert!(cross_field_adapter.contains("CrossFieldGlobalLookup"));
    assert!(!cross_field_adapter.contains("Ok(())"));
}
