use super::*;
use crate::vega::zk_ams::mkhe::{
    rns_native_profile::{
        ZkAmsMkheRnsNativeFamilyV1, zk_ams_mkhe_rns_native_profile_v1,
        zk_ams_mkhe_rns_native_release_candidate_digest_v1, zk_ams_mkhe_rns_native_topology_v1,
    },
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
    terminal_cross_basis_ipa::detached_kernel_test_fixture_v2,
};
use std::sync::OnceLock;

fn digest(label: &[u8], context: u16, ordinal: u16) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.rns-native-terminal-cross-basis.test");
    hash.update(
        &u16::try_from(label.len())
            .expect("test label length fits u16")
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

fn opening_role(ordinal: usize) -> (ZkAmsMkheRnsNativeFamilyV1, u8) {
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

fn build_transcript(
    context: u16,
    cross_basis_root: [u8; 32],
) -> ZkAmsMkheRnsNativeChallengeSeedsV1 {
    let profile = zk_ams_mkhe_rns_native_profile_v1().expect("canonical profile");
    let topology = zk_ams_mkhe_rns_native_topology_v1().expect("canonical topology");
    let release = zk_ams_mkhe_rns_native_release_candidate_digest_v1().expect("candidate");
    let layout = ZkAmsMkheRnsNativeSourceLayoutV1::new(
        profile.profile_digest,
        topology.topology_digest,
        release,
        digest(b"statement", context, 0),
        digest(b"operational-context", context, 0),
    )
    .expect("source layout");
    let receipt = TestSnapshot { layout, context }
        .structural_receipt()
        .expect("source receipt");
    let public_context = ZkAmsMkheRnsNativePublicContextV1::new(
        digest(b"governed-roster", context, 0),
        digest(b"public-ciphertext", context, 0),
    )
    .expect("public context");
    let transcript = ZkAmsMkheRnsNativeTranscriptV1::new(layout, receipt, public_context)
        .expect("transcript context");
    let records = core::array::from_fn(|ordinal| {
        let (family, family_index) = opening_role(ordinal);
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
        .expect("opening")
    });
    let openings =
        ZkAmsMkheRnsNativeOpeningCommitmentsV1::new(transcript.binding_digest(), records)
            .expect("opening set");
    let transcript = transcript
        .bind_opening_commitments(openings)
        .expect("opening transcript");
    let bridge = ZkAmsMkheRnsNativeTerminalBridgeV1::new(
        transcript.binding_digest(),
        digest(b"mapping-root", context, 0),
        digest(b"terminal-hyrax-root", context, 0),
        cross_basis_root,
    )
    .expect("terminal bridge");
    let transcript = transcript
        .bind_terminal_bridge(bridge)
        .expect("terminal transcript");
    let fri_roots = core::array::from_fn(|layer| {
        ZkAmsMkheRnsNativeQpcsFriRootV1::new(
            u8::try_from(layer).expect("FRI layer fits u8"),
            digest(
                b"qpcs-fri-root",
                context,
                u16::try_from(layer).expect("FRI layer fits u16"),
            ),
        )
        .expect("FRI root")
    });
    let qpcs = ZkAmsMkheRnsNativeQpcsRootsV1::new(
        transcript.binding_digest(),
        digest(b"qpcs-initial-root", context, 0),
        digest(b"qpcs-quotient-root", context, 0),
        fri_roots,
    )
    .expect("qPCS roots");
    let transcript = transcript.bind_qpcs_roots(qpcs).expect("qPCS transcript");
    let roots = ZkAmsMkheRnsNativeTerminalRootsV1::new(
        transcript.binding_digest(),
        digest(b"cross-field-root", context, 0),
        digest(b"global-lookup-root", context, 0),
        digest(b"zero-padding-root", context, 0),
    )
    .expect("terminal roots");
    transcript
        .bind_terminal_roots(roots)
        .expect("final transcript")
}

struct FixtureV1 {
    transcript: ZkAmsMkheRnsNativeChallengeSeedsV1,
    encoded: Vec<u8>,
}

fn fixture() -> &'static FixtureV1 {
    static FIXTURE: OnceLock<FixtureV1> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let context = 71;
        let provisional = build_transcript(context, digest(b"provisional-bridge-root", context, 0));
        let binding = context_binding_digest_v1(&provisional).expect("pre-root binding");
        let kernel = detached_kernel_test_fixture_v2(binding).expect("valid detached kernel");
        let transcript = build_transcript(context, kernel.bridge_root);
        assert_eq!(
            context_binding_digest_v1(&transcript).expect("final binding"),
            binding,
            "post-root transcript stages cannot alter the kernel binding"
        );
        let encoded = encode_kernel_v1(
            &transcript,
            &kernel.hyrax_commitments,
            &kernel.bp_commitments,
            &kernel.proof,
        )
        .expect("canonical kernel encoding");
        FixtureV1 {
            transcript,
            encoded,
        }
    })
}

fn refresh_codec(bytes: &mut [u8]) {
    let digest = codec_digest_v1(&bytes[..CODEC_DIGEST_OFFSET_V1]);
    bytes[CODEC_DIGEST_OFFSET_V1..].copy_from_slice(&digest);
}

fn refresh_hyrax_digest(bytes: &mut [u8]) {
    let digest = point_set_digest_v1(
        HYRAX_POINT_ROLE_V1,
        &bytes[HYRAX_POINTS_OFFSET_V1..BP_POINTS_OFFSET_V1],
    )
    .expect("fixed Hyrax point region");
    bytes[HYRAX_DIGEST_OFFSET_V1..HYRAX_DIGEST_OFFSET_V1 + 32].copy_from_slice(&digest);
    refresh_codec(bytes);
}

fn refresh_bp_digest(bytes: &mut [u8]) {
    let digest = point_set_digest_v1(
        BP_POINT_ROLE_V1,
        &bytes[BP_POINTS_OFFSET_V1..RAW_PROOF_OFFSET_V1],
    )
    .expect("fixed BP point region");
    bytes[BP_DIGEST_OFFSET_V1..BP_DIGEST_OFFSET_V1 + 32].copy_from_slice(&digest);
    refresh_codec(bytes);
}

fn refresh_proof_digest(bytes: &mut [u8]) {
    let digest = proof_digest_v1(&bytes[RAW_PROOF_OFFSET_V1..CODEC_DIGEST_OFFSET_V1])
        .expect("fixed proof region");
    bytes[PROOF_DIGEST_OFFSET_V1..PROOF_DIGEST_OFFSET_V1 + 32].copy_from_slice(&digest);
    refresh_codec(bytes);
}

fn assert_kernel_error(
    result: Result<
        RnsNativeTerminalCrossBasisKernelPrerequisiteV1,
        RnsNativeTerminalCrossBasisErrorV1,
    >,
    expected: RnsNativeTerminalCrossBasisErrorV1,
) {
    assert!(matches!(result, Err(error) if error == expected));
}

#[test]
fn exact_kernel_roundtrip_is_transcript_bound_and_non_authorizing() {
    let fixture = fixture();
    assert_eq!(fixture.encoded.len(), EXACT_CODEC_BYTES_V1);
    let prerequisite = authenticate_rns_native_terminal_cross_basis_kernel_v1(
        &fixture.transcript,
        &fixture.encoded,
    )
    .expect("valid representation-equality prerequisite");
    prerequisite
        .validate_context_v1(&fixture.transcript)
        .expect("retained exact context");
    let substituted = build_transcript(73, fixture.transcript.cross_basis_bridge_root());
    assert_eq!(
        prerequisite.validate_context_v1(&substituted),
        Err(RnsNativeTerminalCrossBasisErrorV1::ContextMismatch)
    );

    let source = include_str!("rns_native_terminal_cross_basis.rs");
    let token = source
        .split("pub(super) struct RnsNativeTerminalCrossBasisKernelPrerequisiteV1")
        .nth(1)
        .expect("private prerequisite")
        .split_once("\n}")
        .map(|(body, _)| body)
        .expect("prerequisite body");
    assert!(!token.contains("pub "));
    assert!(!source.contains("impl Clone for RnsNativeTerminalCrossBasisKernelPrerequisiteV1"));
    assert!(!source.contains("authorizes_release"));
    assert!(!source.contains("release_ready = true"));
}

#[test]
fn fixed_width_preflight_rejects_every_truncation_trailing_cap_and_header_mutation() {
    let fixture = fixture();
    for length in 0..EXACT_CODEC_BYTES_V1 {
        assert_kernel_error(
            authenticate_rns_native_terminal_cross_basis_kernel_v1(
                &fixture.transcript,
                &fixture.encoded[..length],
            ),
            RnsNativeTerminalCrossBasisErrorV1::InvalidEncoding,
        );
    }

    let mut trailing = fixture.encoded.clone();
    trailing.push(0);
    assert_kernel_error(
        authenticate_rns_native_terminal_cross_basis_kernel_v1(&fixture.transcript, &trailing),
        RnsNativeTerminalCrossBasisErrorV1::InvalidEncoding,
    );

    let oversized =
        vec![0_u8; ZK_AMS_MKHE_RNS_NATIVE_TERMINAL_BRIDGE_SECTION_MAX_BYTES_V1 as usize + 1];
    assert_kernel_error(
        authenticate_rns_native_terminal_cross_basis_kernel_v1(&fixture.transcript, &oversized),
        RnsNativeTerminalCrossBasisErrorV1::CapExceeded,
    );

    for offset in [0, 4, 5, 6, 8, 10, 12, 13] {
        let mut mutated = fixture.encoded.clone();
        mutated[offset] ^= 1;
        let result =
            authenticate_rns_native_terminal_cross_basis_kernel_v1(&fixture.transcript, &mutated);
        assert!(
            matches!(
                result,
                Err(RnsNativeTerminalCrossBasisErrorV1::InvalidEncoding)
            ),
            "header offset {offset}"
        );
    }
}

#[test]
fn context_point_order_root_proof_and_digest_mutations_are_rejected() {
    let fixture = fixture();

    let mut codec_mutation = fixture.encoded.clone();
    codec_mutation[CODEC_DIGEST_OFFSET_V1] ^= 1;
    assert_kernel_error(
        authenticate_rns_native_terminal_cross_basis_kernel_v1(
            &fixture.transcript,
            &codec_mutation,
        ),
        RnsNativeTerminalCrossBasisErrorV1::Integrity,
    );

    let mut point_digest_mutation = fixture.encoded.clone();
    point_digest_mutation[HYRAX_DIGEST_OFFSET_V1] ^= 1;
    refresh_codec(&mut point_digest_mutation);
    assert_kernel_error(
        authenticate_rns_native_terminal_cross_basis_kernel_v1(
            &fixture.transcript,
            &point_digest_mutation,
        ),
        RnsNativeTerminalCrossBasisErrorV1::Integrity,
    );

    let mut noncanonical_point = fixture.encoded.clone();
    noncanonical_point[HYRAX_POINTS_OFFSET_V1..HYRAX_POINTS_OFFSET_V1 + BRIDGE_POINT_BYTES_V2]
        .fill(0);
    refresh_hyrax_digest(&mut noncanonical_point);
    assert_kernel_error(
        authenticate_rns_native_terminal_cross_basis_kernel_v1(
            &fixture.transcript,
            &noncanonical_point,
        ),
        RnsNativeTerminalCrossBasisErrorV1::InvalidPoint,
    );

    let mut reordered_hyrax = fixture.encoded.clone();
    let first: [u8; BRIDGE_POINT_BYTES_V2] = reordered_hyrax
        [HYRAX_POINTS_OFFSET_V1..HYRAX_POINTS_OFFSET_V1 + BRIDGE_POINT_BYTES_V2]
        .try_into()
        .expect("first point");
    let second: [u8; BRIDGE_POINT_BYTES_V2] = reordered_hyrax[HYRAX_POINTS_OFFSET_V1
        + BRIDGE_POINT_BYTES_V2
        ..HYRAX_POINTS_OFFSET_V1 + 2 * BRIDGE_POINT_BYTES_V2]
        .try_into()
        .expect("second point");
    reordered_hyrax[HYRAX_POINTS_OFFSET_V1..HYRAX_POINTS_OFFSET_V1 + BRIDGE_POINT_BYTES_V2]
        .copy_from_slice(&second);
    reordered_hyrax[HYRAX_POINTS_OFFSET_V1 + BRIDGE_POINT_BYTES_V2
        ..HYRAX_POINTS_OFFSET_V1 + 2 * BRIDGE_POINT_BYTES_V2]
        .copy_from_slice(&first);
    refresh_hyrax_digest(&mut reordered_hyrax);
    assert_kernel_error(
        authenticate_rns_native_terminal_cross_basis_kernel_v1(
            &fixture.transcript,
            &reordered_hyrax,
        ),
        RnsNativeTerminalCrossBasisErrorV1::InvalidProof,
    );

    let mut reordered_bp = fixture.encoded.clone();
    let first: [u8; BRIDGE_POINT_BYTES_V2] = reordered_bp
        [BP_POINTS_OFFSET_V1..BP_POINTS_OFFSET_V1 + BRIDGE_POINT_BYTES_V2]
        .try_into()
        .expect("first point");
    let second: [u8; BRIDGE_POINT_BYTES_V2] = reordered_bp[BP_POINTS_OFFSET_V1
        + BRIDGE_POINT_BYTES_V2
        ..BP_POINTS_OFFSET_V1 + 2 * BRIDGE_POINT_BYTES_V2]
        .try_into()
        .expect("second point");
    reordered_bp[BP_POINTS_OFFSET_V1..BP_POINTS_OFFSET_V1 + BRIDGE_POINT_BYTES_V2]
        .copy_from_slice(&second);
    reordered_bp[BP_POINTS_OFFSET_V1 + BRIDGE_POINT_BYTES_V2
        ..BP_POINTS_OFFSET_V1 + 2 * BRIDGE_POINT_BYTES_V2]
        .copy_from_slice(&first);
    refresh_bp_digest(&mut reordered_bp);
    assert_kernel_error(
        authenticate_rns_native_terminal_cross_basis_kernel_v1(&fixture.transcript, &reordered_bp),
        RnsNativeTerminalCrossBasisErrorV1::InvalidProof,
    );

    let mut proof_mutation = fixture.encoded.clone();
    proof_mutation[RAW_PROOF_OFFSET_V1 + 2 * BRIDGE_POINT_BYTES_V2] ^= 1;
    refresh_proof_digest(&mut proof_mutation);
    assert_kernel_error(
        authenticate_rns_native_terminal_cross_basis_kernel_v1(
            &fixture.transcript,
            &proof_mutation,
        ),
        RnsNativeTerminalCrossBasisErrorV1::InvalidProof,
    );

    let mut noncanonical_response = fixture.encoded.clone();
    noncanonical_response[RAW_PROOF_OFFSET_V1 + 2 * BRIDGE_POINT_BYTES_V2
        ..RAW_PROOF_OFFSET_V1 + 2 * BRIDGE_POINT_BYTES_V2 + 32]
        .fill(0xff);
    refresh_proof_digest(&mut noncanonical_response);
    assert_kernel_error(
        authenticate_rns_native_terminal_cross_basis_kernel_v1(
            &fixture.transcript,
            &noncanonical_response,
        ),
        RnsNativeTerminalCrossBasisErrorV1::InvalidProof,
    );

    let mut binding_mutation = fixture.encoded.clone();
    binding_mutation[BINDING_DIGEST_OFFSET_V1] ^= 1;
    refresh_codec(&mut binding_mutation);
    assert_kernel_error(
        authenticate_rns_native_terminal_cross_basis_kernel_v1(
            &fixture.transcript,
            &binding_mutation,
        ),
        RnsNativeTerminalCrossBasisErrorV1::ContextMismatch,
    );

    let other_context = build_transcript(72, fixture.transcript.cross_basis_bridge_root());
    assert_kernel_error(
        authenticate_rns_native_terminal_cross_basis_kernel_v1(&other_context, &fixture.encoded),
        RnsNativeTerminalCrossBasisErrorV1::ContextMismatch,
    );

    let other_root = digest(b"wrong-cross-basis-root", 71, 0);
    let other_root_transcript = build_transcript(71, other_root);
    let mut wrong_root = fixture.encoded.clone();
    wrong_root[EXPECTED_ROOT_OFFSET_V1..EXPECTED_ROOT_OFFSET_V1 + 32].copy_from_slice(&other_root);
    refresh_codec(&mut wrong_root);
    assert_kernel_error(
        authenticate_rns_native_terminal_cross_basis_kernel_v1(&other_root_transcript, &wrong_root),
        RnsNativeTerminalCrossBasisErrorV1::RootMismatch,
    );
}
