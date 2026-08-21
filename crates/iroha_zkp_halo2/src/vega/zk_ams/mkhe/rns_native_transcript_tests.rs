use super::*;
use crate::vega::zk_ams::mkhe::{
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
        ZkAmsMkheRnsNativeQpcsRootsV1::new([1; 32], [2; 32], [3; 32], fri_roots,),
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
        ZkAmsMkheRnsNativeQpcsRootsV1::new([1; 32], [2; 32], [3; 32], fri_roots),
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
        "pub struct ZkAmsMkheRnsNativeQpcsBoundTranscriptV1",
    ] {
        let declaration = source.find(stage).expect("stage declaration");
        let prefix = &source[declaration.saturating_sub(100)..declaration];
        assert!(!prefix.contains("derive(Clone"));
    }
    assert!(!source.contains("authorizes_release"));
    assert!(!source.contains("proof_verified"));
    assert!(!source.contains("readiness_authority"));
}
