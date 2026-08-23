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
};
use std::sync::OnceLock;

const BINDING_DIGEST_OFFSET_V1: usize = 20;
const POINT_SET_DIGEST_OFFSET_V1: usize = BINDING_DIGEST_OFFSET_V1 + 32;
const EXPECTED_ROOT_OFFSET_V1: usize = POINT_SET_DIGEST_OFFSET_V1 + 32;
const PROOF_DIGEST_OFFSET_V1: usize = EXPECTED_ROOT_OFFSET_V1 + 32;

fn digest(label: &[u8], context: u16, ordinal: u16) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.rns-native-zero-padding.test");
    hash.update(&(label.len() as u16).to_be_bytes());
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
        1..=16 => (ZkAmsMkheRnsNativeFamilyV1::U, (ordinal - 1) as u8),
        17..=32 => (ZkAmsMkheRnsNativeFamilyV1::E, (ordinal - 17) as u8),
        33 => (ZkAmsMkheRnsNativeFamilyV1::RE, 0),
        34..=41 => (ZkAmsMkheRnsNativeFamilyV1::W, (ordinal - 34) as u8),
        42 => (ZkAmsMkheRnsNativeFamilyV1::RW, 0),
        _ => panic!("opening ordinal outside exact shape"),
    }
}

fn build_transcript(
    context: u16,
    zero_padding_root: [u8; 32],
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
            digest(b"source-commitment", context, ordinal as u16),
            digest(b"hyrax-commitment", context, ordinal as u16),
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
        digest(b"cross-basis-root", context, 0),
    )
    .expect("terminal bridge");
    let transcript = transcript
        .bind_terminal_bridge(bridge)
        .expect("terminal transcript");
    let fri_roots = core::array::from_fn(|layer| {
        ZkAmsMkheRnsNativeQpcsFriRootV1::new(
            layer as u8,
            digest(b"qpcs-fri-root", context, layer as u16),
        )
        .expect("FRI root")
    });
    let qpcs = ZkAmsMkheRnsNativeQpcsRootsV1::new(
        transcript.binding_digest(),
        digest(b"qpcs-initial-root", context, 0),
        digest(b"q-mask-s-root", context, 0),
        digest(b"qpcs-quotient-root", context, 0),
        fri_roots,
    )
    .expect("qPCS roots");
    let transcript = transcript.bind_qpcs_roots(qpcs).expect("qPCS transcript");
    let roots = ZkAmsMkheRnsNativeTerminalRootsV1::new(
        transcript.binding_digest(),
        digest(b"cross-field-root", context, 0),
        digest(b"global-lookup-root", context, 0),
        zero_padding_root,
    )
    .expect("terminal roots");
    transcript
        .bind_terminal_roots(roots)
        .expect("final transcript")
}

fn commitment_bytes(commitments: &[Point]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(COMMITMENTS_BYTES_V1);
    for point in commitments {
        bytes.extend_from_slice(
            &point
                .to_non_identity_wire_bytes()
                .expect("nonidentity test commitment"),
        );
    }
    bytes
}

fn transcript_for_commitments(
    context: u16,
    commitments: &[Point],
) -> (
    ZkAmsMkheRnsNativeChallengeSeedsV1,
    [[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
) {
    let provisional = build_transcript(
        context,
        digest(b"provisional-zero-padding-root", context, 0),
    );
    let limb_digests = limb_digests_v1(&commitment_bytes(commitments)).expect("limb digests");
    let root = padding_root_v1(&provisional, &limb_digests).expect("padding root");
    let transcript = build_transcript(context, root);
    assert_eq!(
        padding_root_v1(&transcript, &limb_digests).expect("stable pre-root formula"),
        root
    );
    (transcript, limb_digests)
}

struct FixtureV1 {
    transcript: ZkAmsMkheRnsNativeChallengeSeedsV1,
    bytes: Vec<u8>,
    limb_digests: [[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
}

fn fixture() -> &'static FixtureV1 {
    static FIXTURE: OnceLock<FixtureV1> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let (commitments, blindings) = deterministic_test_commitments_v1();
        let (transcript, expected_limb_digests) = transcript_for_commitments(83, &commitments);
        let encoded = encode_test_proof_v1(&transcript, &commitments, &blindings)
            .expect("valid zero-padding proof");
        assert_eq!(encoded.limb_digests, expected_limb_digests);
        FixtureV1 {
            transcript,
            bytes: encoded.bytes,
            limb_digests: encoded.limb_digests,
        }
    })
}

fn refresh_codec(bytes: &mut [u8]) {
    let digest = codec_digest_v1(&bytes[..CODEC_DIGEST_OFFSET_V1]);
    bytes[CODEC_DIGEST_OFFSET_V1..].copy_from_slice(&digest);
}

fn refresh_point_set(bytes: &mut [u8]) {
    let digest = point_set_digest_v1(&bytes[COMMITMENTS_OFFSET_V1..MASK_POINTS_OFFSET_V1])
        .expect("point region");
    bytes[POINT_SET_DIGEST_OFFSET_V1..POINT_SET_DIGEST_OFFSET_V1 + 32].copy_from_slice(&digest);
    refresh_codec(bytes);
}

fn refresh_proof(bytes: &mut [u8]) {
    let digest = schnorr_proof_digest_v1(
        &bytes[MASK_POINTS_OFFSET_V1..RESPONSES_OFFSET_V1],
        &bytes[RESPONSES_OFFSET_V1..CODEC_DIGEST_OFFSET_V1],
    )
    .expect("proof region");
    bytes[PROOF_DIGEST_OFFSET_V1..PROOF_DIGEST_OFFSET_V1 + 32].copy_from_slice(&digest);
    refresh_codec(bytes);
}

fn assert_error(
    result: Result<
        RnsNativeZeroPaddingCommitmentPrerequisiteV1,
        RnsNativeZeroPaddingCommitmentErrorV1,
    >,
    expected: RnsNativeZeroPaddingCommitmentErrorV1,
) {
    assert!(matches!(result, Err(error) if error == expected));
}

#[test]
fn exact_geometry_root_and_move_only_prerequisite_are_frozen() {
    assert_eq!(CHUNKS_PER_LIMB_V1, 191);
    assert_eq!(COMMITMENT_COUNT_V1, 7_640);
    assert_eq!(EXACT_CODEC_BYTES_V1, 254_900);
    let first = padding_chunk_v1(0).expect("first X padding chunk");
    assert_eq!(
        (
            first.record_ordinal,
            first.family,
            first.first_slot,
            first.slot_count
        ),
        (0, ZkAmsMkheRnsNativeFamilyV1::X, 89, 1_024)
    );
    let x_last = padding_chunk_v1(63).expect("last X padding chunk");
    assert_eq!((x_last.first_slot, x_last.slot_count), (64_601, 935));
    let re_first = padding_chunk_v1(64).expect("first rE padding chunk");
    assert_eq!(
        (
            re_first.record_ordinal,
            re_first.family,
            re_first.first_slot,
            re_first.slot_count,
        ),
        (33, ZkAmsMkheRnsNativeFamilyV1::RE, 1_024, 1_024)
    );
    let re_last = padding_chunk_v1(126).expect("last rE padding chunk");
    assert_eq!((re_last.first_slot, re_last.slot_count), (64_512, 1_024));
    let rw_first = padding_chunk_v1(127).expect("first rW padding chunk");
    assert_eq!((rw_first.first_slot, rw_first.slot_count), (512, 1_024));
    let last = padding_chunk_v1(190).expect("last rW padding chunk");
    assert_eq!(
        (
            last.record_ordinal,
            last.family,
            last.first_slot,
            last.slot_count
        ),
        (42, ZkAmsMkheRnsNativeFamilyV1::RW, 65_024, 512)
    );
    assert_eq!(
        padding_chunk_v1(191),
        Err(RnsNativeZeroPaddingCommitmentErrorV1::InvalidGeometry)
    );

    let fixture = fixture();
    let token = authenticate_rns_native_zero_padding_commitments_v1(
        &fixture.transcript,
        &fixture.limb_digests,
        &fixture.bytes,
    )
    .expect("valid hiding-only padding commitments");
    assert_eq!(token.root(), fixture.transcript.zero_padding_root());
    assert_eq!(token.limb_padding_digests(), &fixture.limb_digests);
    token
        .validate_context_v1(&fixture.transcript)
        .expect("retained exact context");
    let verified_root = token
        .verified_zero_padding_root_v1(&fixture.transcript)
        .expect("verified zero root for the exact final transcript");
    assert!(verified_root.matches_claimed_zero_padding_root_v1(
        fixture.transcript.zero_padding_root(),
        fixture.transcript.transcript_digest(),
    ));
    let substituted = build_transcript(97, fixture.transcript.zero_padding_root());
    assert_eq!(
        token.validate_context_v1(&substituted),
        Err(RnsNativeZeroPaddingCommitmentErrorV1::ContextMismatch)
    );
    assert!(matches!(
        token.verified_zero_padding_root_v1(&substituted),
        Err(RnsNativeZeroPaddingCommitmentErrorV1::ContextMismatch)
    ));
    let source = include_str!("rns_native_zero_padding_commitment.rs");
    let declaration = "pub(super) struct RnsNativeZeroPaddingCommitmentPrerequisiteV1";
    let declaration_offset = source.find(declaration).expect("token declaration");
    let attributes = source[..declaration_offset]
        .rsplit_once("\n\n")
        .map_or(&source[..declaration_offset], |(_, block)| block);
    let token_body = source[declaration_offset + declaration.len()..]
        .split_once("\n}")
        .map(|(body, _)| body)
        .expect("token body");
    assert!(!attributes.contains("derive(Clone"));
    assert!(!attributes.contains("derive(Copy"));
    assert!(!token_body.contains("pub fn"));
    assert!(!token_body.contains("candidate"));

    let verified_declaration = source
        .find("pub(super) struct RnsNativeVerifiedZeroPaddingRootV1")
        .expect("verified zero-root declaration");
    let verified_attributes =
        &source[verified_declaration.saturating_sub(420)..verified_declaration];
    assert!(!verified_attributes.contains("derive(Clone"));
    assert!(!verified_attributes.contains("derive(Copy"));
    let verified_surface = source[verified_declaration..]
        .split_once("impl RnsNativeZeroPaddingCommitmentPrerequisiteV1")
        .expect("verified zero-root surface boundary")
        .0;
    assert!(verified_surface.contains("fn matches_claimed_zero_padding_root_v1("));
    for forbidden in [
        "pub(super) fn new(",
        "pub(super) fn from",
        "pub(super) fn root(",
        "pub(super) const fn root(",
        "pub(super) fn final_transcript_tag(",
        "pub(super) const fn final_transcript_tag(",
        "pub(super) fn into_parts(",
        "impl AsRef",
        "impl core::ops::Deref",
    ] {
        assert!(!verified_surface.contains(forbidden));
    }
    let mint_surface = source
        .split_once("pub(super) fn verified_zero_padding_root_v1(")
        .expect("verified zero-root mint")
        .1
        .split_once("\n    }")
        .expect("verified zero-root mint boundary")
        .0;
    let context_validation = mint_surface
        .find("self.validate_context_v1(transcript)?;")
        .expect("exact context revalidation");
    let evidence_construction = mint_surface
        .find("Ok(RnsNativeVerifiedZeroPaddingRootV1")
        .expect("opaque evidence construction");
    assert!(context_validation < evidence_construction);
    assert_eq!(
        source
            .matches("Ok(RnsNativeVerifiedZeroPaddingRootV1 {")
            .count(),
        1
    );
}

#[test]
fn exact_codec_rejects_caps_truncation_trailing_and_header_mutations() {
    let fixture = fixture();
    for length in [0, 1, HEADER_BYTES_V1 - 1, fixture.bytes.len() - 1] {
        assert_error(
            authenticate_rns_native_zero_padding_commitments_v1(
                &fixture.transcript,
                &fixture.limb_digests,
                &fixture.bytes[..length],
            ),
            RnsNativeZeroPaddingCommitmentErrorV1::InvalidEncoding,
        );
    }
    let mut trailing = fixture.bytes.clone();
    trailing.push(0);
    assert_error(
        authenticate_rns_native_zero_padding_commitments_v1(
            &fixture.transcript,
            &fixture.limb_digests,
            &trailing,
        ),
        RnsNativeZeroPaddingCommitmentErrorV1::InvalidEncoding,
    );
    let oversized = vec![0; ZK_AMS_MKHE_RNS_NATIVE_ZERO_PADDING_SECTION_MAX_BYTES_V1 as usize + 1];
    assert_error(
        authenticate_rns_native_zero_padding_commitments_v1(
            &fixture.transcript,
            &fixture.limb_digests,
            &oversized,
        ),
        RnsNativeZeroPaddingCommitmentErrorV1::CapExceeded,
    );
    for offset in 0..20 {
        let mut changed = fixture.bytes.clone();
        changed[offset] ^= 1;
        assert_error(
            authenticate_rns_native_zero_padding_commitments_v1(
                &fixture.transcript,
                &fixture.limb_digests,
                &changed,
            ),
            RnsNativeZeroPaddingCommitmentErrorV1::InvalidEncoding,
        );
    }
}

#[test]
fn context_limb_root_and_commitment_splices_are_rejected() {
    let fixture = fixture();
    let (commitments, _) = deterministic_test_commitments_v1();
    let (other_context, _) = transcript_for_commitments(84, &commitments);
    assert_error(
        authenticate_rns_native_zero_padding_commitments_v1(
            &other_context,
            &fixture.limb_digests,
            &fixture.bytes,
        ),
        RnsNativeZeroPaddingCommitmentErrorV1::ContextMismatch,
    );

    let mut reordered_limbs = fixture.limb_digests;
    reordered_limbs.swap(0, 1);
    assert_error(
        authenticate_rns_native_zero_padding_commitments_v1(
            &fixture.transcript,
            &reordered_limbs,
            &fixture.bytes,
        ),
        RnsNativeZeroPaddingCommitmentErrorV1::ContextMismatch,
    );

    let mut changed_root = fixture.bytes.clone();
    changed_root[EXPECTED_ROOT_OFFSET_V1] ^= 1;
    refresh_codec(&mut changed_root);
    assert_error(
        authenticate_rns_native_zero_padding_commitments_v1(
            &fixture.transcript,
            &fixture.limb_digests,
            &changed_root,
        ),
        RnsNativeZeroPaddingCommitmentErrorV1::ContextMismatch,
    );

    let mut reordered_points = fixture.bytes.clone();
    let first: [u8; POINT_BYTES_V1] = reordered_points
        [COMMITMENTS_OFFSET_V1..COMMITMENTS_OFFSET_V1 + POINT_BYTES_V1]
        .try_into()
        .expect("first point");
    let second: [u8; POINT_BYTES_V1] = reordered_points
        [COMMITMENTS_OFFSET_V1 + POINT_BYTES_V1..COMMITMENTS_OFFSET_V1 + 2 * POINT_BYTES_V1]
        .try_into()
        .expect("second point");
    reordered_points[COMMITMENTS_OFFSET_V1..COMMITMENTS_OFFSET_V1 + POINT_BYTES_V1]
        .copy_from_slice(&second);
    reordered_points
        [COMMITMENTS_OFFSET_V1 + POINT_BYTES_V1..COMMITMENTS_OFFSET_V1 + 2 * POINT_BYTES_V1]
        .copy_from_slice(&first);
    refresh_point_set(&mut reordered_points);
    assert!(
        authenticate_rns_native_zero_padding_commitments_v1(
            &fixture.transcript,
            &fixture.limb_digests,
            &reordered_points,
        )
        .is_err()
    );

    let mut corrupt_binding = fixture.bytes.clone();
    corrupt_binding[BINDING_DIGEST_OFFSET_V1] ^= 1;
    refresh_codec(&mut corrupt_binding);
    assert_error(
        authenticate_rns_native_zero_padding_commitments_v1(
            &fixture.transcript,
            &fixture.limb_digests,
            &corrupt_binding,
        ),
        RnsNativeZeroPaddingCommitmentErrorV1::ContextMismatch,
    );
}

#[test]
fn point_scalar_and_schnorr_mutations_are_rejected_after_integrity_refresh() {
    let fixture = fixture();
    let mut invalid_mask = fixture.bytes.clone();
    invalid_mask[MASK_POINTS_OFFSET_V1..MASK_POINTS_OFFSET_V1 + POINT_BYTES_V1].fill(0xff);
    refresh_proof(&mut invalid_mask);
    assert_error(
        authenticate_rns_native_zero_padding_commitments_v1(
            &fixture.transcript,
            &fixture.limb_digests,
            &invalid_mask,
        ),
        RnsNativeZeroPaddingCommitmentErrorV1::InvalidPoint,
    );

    let mut invalid_scalar = fixture.bytes.clone();
    invalid_scalar[RESPONSES_OFFSET_V1..RESPONSES_OFFSET_V1 + SCALAR_BYTES_V1].fill(0xff);
    refresh_proof(&mut invalid_scalar);
    assert_error(
        authenticate_rns_native_zero_padding_commitments_v1(
            &fixture.transcript,
            &fixture.limb_digests,
            &invalid_scalar,
        ),
        RnsNativeZeroPaddingCommitmentErrorV1::InvalidScalar,
    );

    let mut changed_response = fixture.bytes.clone();
    let encoded: [u8; SCALAR_BYTES_V1] = changed_response
        [RESPONSES_OFFSET_V1..RESPONSES_OFFSET_V1 + SCALAR_BYTES_V1]
        .try_into()
        .expect("response scalar");
    let changed = Scalar::from_le_bytes_exact(encoded).expect("canonical response") + Scalar::one();
    changed_response[RESPONSES_OFFSET_V1..RESPONSES_OFFSET_V1 + SCALAR_BYTES_V1]
        .copy_from_slice(&changed.to_le_bytes());
    refresh_proof(&mut changed_response);
    assert_error(
        authenticate_rns_native_zero_padding_commitments_v1(
            &fixture.transcript,
            &fixture.limb_digests,
            &changed_response,
        ),
        RnsNativeZeroPaddingCommitmentErrorV1::InvalidProof,
    );

    let mut swapped_masks = fixture.bytes.clone();
    let first: [u8; POINT_BYTES_V1] = swapped_masks
        [MASK_POINTS_OFFSET_V1..MASK_POINTS_OFFSET_V1 + POINT_BYTES_V1]
        .try_into()
        .expect("first mask");
    let second: [u8; POINT_BYTES_V1] = swapped_masks
        [MASK_POINTS_OFFSET_V1 + POINT_BYTES_V1..MASK_POINTS_OFFSET_V1 + 2 * POINT_BYTES_V1]
        .try_into()
        .expect("second mask");
    swapped_masks[MASK_POINTS_OFFSET_V1..MASK_POINTS_OFFSET_V1 + POINT_BYTES_V1]
        .copy_from_slice(&second);
    swapped_masks
        [MASK_POINTS_OFFSET_V1 + POINT_BYTES_V1..MASK_POINTS_OFFSET_V1 + 2 * POINT_BYTES_V1]
        .copy_from_slice(&first);
    refresh_proof(&mut swapped_masks);
    assert_error(
        authenticate_rns_native_zero_padding_commitments_v1(
            &fixture.transcript,
            &fixture.limb_digests,
            &swapped_masks,
        ),
        RnsNativeZeroPaddingCommitmentErrorV1::InvalidProof,
    );
}

#[test]
fn a_nonzero_padding_component_cannot_use_the_hiding_only_opening() {
    let (mut commitments, blindings) = deterministic_test_commitments_v1();
    commitments[0] += ZkAmsT256BulletproofSuiteV1::generators().g_bold[0];
    let (transcript, _) = transcript_for_commitments(85, &commitments);
    assert_eq!(
        encode_test_proof_v1(&transcript, &commitments, &blindings).map(|_| ()),
        Err(RnsNativeZeroPaddingCommitmentErrorV1::InvalidProof)
    );

    let (commitments, mut wrong_blindings) = deterministic_test_commitments_v1();
    let (transcript, _) = transcript_for_commitments(86, &commitments);
    wrong_blindings[17] += Scalar::one();
    assert_eq!(
        encode_test_proof_v1(&transcript, &commitments, &wrong_blindings).map(|_| ()),
        Err(RnsNativeZeroPaddingCommitmentErrorV1::InvalidProof)
    );
}

#[test]
fn post_root_challenges_are_deterministic_and_context_separated() {
    let (commitments, _) = deterministic_test_commitments_v1();
    let bytes = commitment_bytes(&commitments);
    let limb_digests = limb_digests_v1(&bytes).expect("limb digests");
    let provisional_a = build_transcript(87, digest(b"placeholder-a", 87, 0));
    let provisional_b = build_transcript(87, digest(b"placeholder-b", 87, 0));
    let root_a = padding_root_v1(&provisional_a, &limb_digests).expect("root A");
    let root_b = padding_root_v1(&provisional_b, &limb_digests).expect("root B");
    assert_eq!(root_a, root_b, "the root formula excludes post-root state");
    let transcript = build_transcript(87, root_a);
    let point_digest = point_set_digest_v1(&bytes).expect("point digest");
    let binding = context_binding_digest_v1(&transcript, point_digest, &limb_digests)
        .expect("context binding");
    let first = derive_aggregation_challenge_v1(&transcript, binding, point_digest, 0)
        .expect("first challenge");
    assert_eq!(
        first,
        derive_aggregation_challenge_v1(&transcript, binding, point_digest, 0)
            .expect("deterministic challenge")
    );
    assert_ne!(
        first,
        derive_aggregation_challenge_v1(&transcript, binding, point_digest, 1)
            .expect("limb-separated challenge")
    );
    let (other, other_limb_digests) = transcript_for_commitments(88, &commitments);
    let other_binding = context_binding_digest_v1(&other, point_digest, &other_limb_digests)
        .expect("other binding");
    assert_ne!(
        first,
        derive_aggregation_challenge_v1(&other, other_binding, point_digest, 0)
            .expect("context-separated challenge")
    );
}
