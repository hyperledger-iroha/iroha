use super::*;
use crate::vega::zk_ams::mkhe::{
    rns_native_profile::{
        ZkAmsMkheRnsNativeFamilyV1, zk_ams_mkhe_rns_native_profile_v1,
        zk_ams_mkhe_rns_native_release_candidate_digest_v1, zk_ams_mkhe_rns_native_topology_v1,
    },
    rns_native_section_codec::{
        ZkAmsMkheRnsNativeCrossFieldGlobalLookupSectionV1,
        ZkAmsMkheRnsNativeRnsRelationQpcsSectionV1, ZkAmsMkheRnsNativeTerminalBridgeSectionV1,
        ZkAmsMkheRnsNativeZeroPaddingSectionV1,
    },
    rns_native_source::{
        ZkAmsMkheRnsNativeSecretChunkV1, ZkAmsMkheRnsNativeSourceArenaV1,
        ZkAmsMkheRnsNativeSourceErrorV1, ZkAmsMkheRnsNativeSourceSnapshotV1,
    },
    rns_native_transcript::{
        ZkAmsMkheRnsNativeOpeningCommitmentV1, ZkAmsMkheRnsNativeOpeningCommitmentsV1,
        ZkAmsMkheRnsNativePublicContextV1, ZkAmsMkheRnsNativeQpcsFriRootV1,
        ZkAmsMkheRnsNativeQpcsRootsV1, ZkAmsMkheRnsNativeTerminalBridgeV1,
        ZkAmsMkheRnsNativeTerminalRootsV1, ZkAmsMkheRnsNativeTranscriptV1,
    },
};

const TYPED_SECTION_COMMON_PREFIX_BYTES_V1: usize = 4 + 1 + 1 + 4 + 5 * 32;

fn digest(label: &[u8], context: u16, ordinal: u16) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.rns-native-composite-verifier.test");
    hash.update(
        &u16::try_from(label.len())
            .expect("test label fits u16")
            .to_be_bytes(),
    );
    hash.update(label);
    hash.update(&context.to_be_bytes());
    hash.update(&ordinal.to_be_bytes());
    hash.finalize()
}

fn indexed_digests<const N: usize>(label: &[u8], context: u16) -> [[u8; 32]; N] {
    core::array::from_fn(|ordinal| {
        digest(
            label,
            context,
            u16::try_from(ordinal).expect("fixture ordinal fits u16"),
        )
    })
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

struct CompositeFixtureV1 {
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    source_receipt: ZkAmsMkheRnsNativeSourceReceiptV1,
    envelope: ZkAmsMkheRnsNativeProofEnvelopeV1,
    transcript: ZkAmsMkheRnsNativeChallengeSeedsV1,
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

fn composite_fixture(context: u16) -> CompositeFixtureV1 {
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
    let source_receipt = TestSnapshot { layout, context }
        .structural_receipt()
        .expect("source receipt");
    let public_context = ZkAmsMkheRnsNativePublicContextV1::new(
        digest(b"governed-roster", context, 0),
        digest(b"public-ciphertext", context, 0),
    )
    .expect("public context");

    let transcript = ZkAmsMkheRnsNativeTranscriptV1::new(layout, source_receipt, public_context)
        .expect("transcript context");
    let opening_records = core::array::from_fn(|ordinal| {
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
        ZkAmsMkheRnsNativeOpeningCommitmentsV1::new(transcript.binding_digest(), opening_records)
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
    .expect("bridge");
    let transcript = transcript
        .bind_terminal_bridge(bridge)
        .expect("bridge transcript");
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
    let qpcs_roots = ZkAmsMkheRnsNativeQpcsRootsV1::new(
        transcript.binding_digest(),
        digest(b"qpcs-initial-root", context, 0),
        digest(b"qpcs-quotient-root", context, 0),
        fri_roots,
    )
    .expect("qPCS roots");
    let transcript = transcript
        .bind_qpcs_roots(qpcs_roots)
        .expect("qPCS transcript");
    let terminal_roots = ZkAmsMkheRnsNativeTerminalRootsV1::new(
        transcript.binding_digest(),
        digest(b"cross-field-root", context, 0),
        digest(b"global-lookup-root", context, 0),
        digest(b"zero-padding-root", context, 0),
    )
    .expect("terminal roots");
    let transcript = transcript
        .bind_terminal_roots(terminal_roots)
        .expect("terminal transcript");

    let terminal_proof = [0x11, context.to_be_bytes()[0], context.to_be_bytes()[1]];
    let terminal_section =
        ZkAmsMkheRnsNativeTerminalBridgeSectionV1::new(&transcript, &terminal_proof)
            .expect("terminal section")
            .to_canonical_bytes_v1()
            .expect("terminal encoding");
    let equation_digests: [[u8; 32]; 2] = indexed_digests(b"equation-commitment", context);
    let qpcs_limb_digests: [[u8; 32]; 40] = indexed_digests(b"qpcs-limb-commitment", context);
    let query_digests: [[u8; 32]; 160] = indexed_digests(b"query-opening", context);
    let rns_proof = [0x22, context.to_be_bytes()[0], context.to_be_bytes()[1]];
    let rns_section = ZkAmsMkheRnsNativeRnsRelationQpcsSectionV1::new(
        &transcript,
        &equation_digests,
        &qpcs_limb_digests,
        &query_digests,
        &rns_proof,
    )
    .expect("RNS/qPCS section")
    .to_canonical_bytes_v1()
    .expect("RNS/qPCS encoding");
    let point_digests: [[u8; 32]; 5] = indexed_digests(b"cross-field-point", context);
    let cross_limb_digests: [[u8; 32]; 40] = indexed_digests(b"cross-field-limb", context);
    let sumcheck_digests: [[u8; 32]; 29] = indexed_digests(b"lookup-sumcheck", context);
    let cross_proof = [0x33, context.to_be_bytes()[0], context.to_be_bytes()[1]];
    let cross_section = ZkAmsMkheRnsNativeCrossFieldGlobalLookupSectionV1::new(
        &transcript,
        &point_digests,
        &cross_limb_digests,
        &sumcheck_digests,
        &cross_proof,
    )
    .expect("cross-field/lookup section")
    .to_canonical_bytes_v1()
    .expect("cross-field/lookup encoding");
    let padding_digests: [[u8; 32]; 40] = indexed_digests(b"padding-limb", context);
    let padding_proof = [0x44, context.to_be_bytes()[0], context.to_be_bytes()[1]];
    let padding_section =
        ZkAmsMkheRnsNativeZeroPaddingSectionV1::new(&transcript, &padding_digests, &padding_proof)
            .expect("zero-padding section")
            .to_canonical_bytes_v1()
            .expect("zero-padding encoding");
    let envelope = ZkAmsMkheRnsNativeProofEnvelopeV1::new(
        layout,
        source_receipt,
        terminal_section,
        rns_section,
        cross_section,
        padding_section,
    )
    .expect("proof envelope");
    CompositeFixtureV1 {
        layout,
        source_receipt,
        envelope,
        transcript,
    }
}

fn exact_authority(
    fixture: &CompositeFixtureV1,
) -> Result<ExactFixtureStageAuthorityV1, ZkAmsMkheRnsNativeCompositeVerificationErrorV1> {
    ExactFixtureStageAuthorityV1::new(
        &fixture.envelope,
        fixture.layout,
        fixture.source_receipt,
        &fixture.transcript,
    )
}

#[test]
fn production_boundary_rejects_an_invalid_terminal_kernel() {
    let fixture = composite_fixture(1);
    assert!(matches!(
        verify_zk_ams_mkhe_rns_native_composite_v1(
            fixture.envelope,
            fixture.layout,
            fixture.source_receipt,
            fixture.transcript,
        ),
        Err(
            ZkAmsMkheRnsNativeCompositeVerificationErrorV1::StageRejected(
                ZkAmsMkheRnsNativeVerificationStageV1::TerminalHyraxBpBridge,
            )
        )
    ));
}

#[test]
fn production_adapters_reject_invalid_subproofs_or_remain_explicitly_unavailable() {
    let fixture = composite_fixture(2);
    let axes = validate_context_v1(
        &fixture.envelope,
        fixture.layout,
        fixture.source_receipt,
        &fixture.transcript,
    )
    .expect("validated context");
    for stage in VERIFICATION_STAGE_ORDER_V1 {
        let descriptor = fixture.envelope.descriptors()[stage.index()];
        let section = fixture.envelope.section(stage.section_kind());
        let result =
            verify_production_stage_v1(stage, &axes, &fixture.transcript, descriptor, section);
        if matches!(
            stage,
            ZkAmsMkheRnsNativeVerificationStageV1::TerminalHyraxBpBridge
                | ZkAmsMkheRnsNativeVerificationStageV1::RnsRelationQpcs
                | ZkAmsMkheRnsNativeVerificationStageV1::ZeroPadding
        ) {
            assert!(matches!(
                result,
                Err(
                    ZkAmsMkheRnsNativeCompositeVerificationErrorV1::StageRejected(
                        rejected,
                    )
                ) if rejected == stage
            ));
        } else {
            assert!(matches!(
                result,
                Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::StageUnavailable(
                    unavailable,
                )) if unavailable == stage
            ));
        }
    }
}

#[test]
fn all_typed_sections_are_validated_before_first_unavailable_stage() {
    let fixture = composite_fixture(4);
    let mut malformed_rns = fixture
        .envelope
        .section(ZkAmsMkheRnsNativeProofSectionKindV1::RnsRelationQpcs)
        .to_vec();
    malformed_rns[TYPED_SECTION_COMMON_PREFIX_BYTES_V1] = 1;
    let envelope = ZkAmsMkheRnsNativeProofEnvelopeV1::new(
        fixture.layout,
        fixture.source_receipt,
        fixture
            .envelope
            .section(ZkAmsMkheRnsNativeProofSectionKindV1::TerminalHyraxBpBridge)
            .to_vec(),
        malformed_rns,
        fixture
            .envelope
            .section(ZkAmsMkheRnsNativeProofSectionKindV1::CrossFieldGlobalLookup)
            .to_vec(),
        fixture
            .envelope
            .section(ZkAmsMkheRnsNativeProofSectionKindV1::ZeroPadding)
            .to_vec(),
    )
    .expect("transport-valid malformed typed section");
    assert!(matches!(
        verify_zk_ams_mkhe_rns_native_composite_v1(
            envelope,
            fixture.layout,
            fixture.source_receipt,
            fixture.transcript,
        ),
        Err(
            ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidSection(
                ZkAmsMkheRnsNativeVerificationStageV1::RnsRelationQpcs,
            )
        )
    ));
}

#[test]
fn inner_metadata_cannot_alias_a_source_or_outer_identity() {
    let fixture = composite_fixture(5);
    let decoded = ZkAmsMkheRnsNativeRnsRelationQpcsSectionV1::from_canonical_bytes_exact_v1(
        fixture
            .envelope
            .section(ZkAmsMkheRnsNativeProofSectionKindV1::RnsRelationQpcs),
        &fixture.transcript,
    )
    .expect("typed RNS section");
    let mut equations = decoded.equation_commitment_digests().to_vec();
    equations[0] = fixture.layout.statement_digest();
    let aliased_rns = ZkAmsMkheRnsNativeRnsRelationQpcsSectionV1::new(
        &fixture.transcript,
        &equations,
        decoded.limb_commitment_digests(),
        decoded.query_opening_digests(),
        decoded.proof(),
    )
    .expect("standalone section cannot see the cross-layer alias")
    .to_canonical_bytes_v1()
    .expect("aliased standalone encoding");
    let envelope = ZkAmsMkheRnsNativeProofEnvelopeV1::new(
        fixture.layout,
        fixture.source_receipt,
        fixture
            .envelope
            .section(ZkAmsMkheRnsNativeProofSectionKindV1::TerminalHyraxBpBridge)
            .to_vec(),
        aliased_rns,
        fixture
            .envelope
            .section(ZkAmsMkheRnsNativeProofSectionKindV1::CrossFieldGlobalLookup)
            .to_vec(),
        fixture
            .envelope
            .section(ZkAmsMkheRnsNativeProofSectionKindV1::ZeroPadding)
            .to_vec(),
    )
    .expect("transport-valid cross-layer alias");
    assert!(matches!(
        verify_zk_ams_mkhe_rns_native_composite_v1(
            envelope,
            fixture.layout,
            fixture.source_receipt,
            fixture.transcript,
        ),
        Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidTranscript)
    ));
}

#[test]
fn exact_private_fixture_mints_candidate_only_after_all_stages() {
    let fixture = composite_fixture(3);
    let authority = exact_authority(&fixture).expect("exact fixture authority");
    let expected_statement = fixture.layout.statement_digest();
    let expected_source = fixture.layout.source_binding_digest();
    let expected_roster = fixture.transcript.governed_roster_digest();
    let expected_ciphertext = fixture.transcript.public_ciphertext_digest();
    let expected_transcript = fixture.transcript.transcript_digest();
    let expected_proof = fixture.envelope.proof_digest();
    let receipt = verify_with_first_party_authority_v1(
        fixture.envelope,
        fixture.layout,
        fixture.source_receipt,
        fixture.transcript,
        FirstPartyStageAuthorityV1::ExactFixture(authority),
    )
    .expect("all exact fixture stages verify");
    assert_eq!(receipt.statement_digest(), expected_statement);
    assert_eq!(receipt.source_binding_digest(), expected_source);
    assert_eq!(receipt.governed_roster_digest(), expected_roster);
    assert_eq!(receipt.public_ciphertext_digest(), expected_ciphertext);
    assert_eq!(receipt.transcript_digest(), expected_transcript);
    assert_eq!(receipt.proof_digest(), expected_proof);
    assert_ne!(receipt.candidate_digest(), [0; 32]);
    assert!(
        !receipt
            .section_digests()
            .contains(&receipt.candidate_digest())
    );
}

#[test]
fn rejection_at_any_stage_never_yields_a_candidate() {
    for (index, stage) in VERIFICATION_STAGE_ORDER_V1.into_iter().enumerate() {
        let fixture =
            composite_fixture(u16::try_from(10 + index).expect("fixture context fits u16"));
        let authority = exact_authority(&fixture)
            .expect("exact authority")
            .reject(stage);
        assert!(matches!(
            verify_with_first_party_authority_v1(
                fixture.envelope,
                fixture.layout,
                fixture.source_receipt,
                fixture.transcript,
                FirstPartyStageAuthorityV1::ExactFixture(authority),
            ),
            Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::StageRejected(
                rejected,
            )) if rejected == stage
        ));
    }
}

#[test]
fn source_and_envelope_context_substitution_is_rejected_before_stages() {
    let first = composite_fixture(20);
    let second = composite_fixture(21);
    let authority = exact_authority(&first).expect("authority");
    assert!(matches!(
        verify_with_first_party_authority_v1(
            first.envelope,
            second.layout,
            second.source_receipt,
            first.transcript,
            FirstPartyStageAuthorityV1::ExactFixture(authority),
        ),
        Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidEnvelopeContext)
    ));

    let first = composite_fixture(22);
    let second = composite_fixture(23);
    let authority = exact_authority(&first).expect("authority");
    assert!(matches!(
        verify_with_first_party_authority_v1(
            first.envelope,
            second.layout,
            first.source_receipt,
            first.transcript,
            FirstPartyStageAuthorityV1::ExactFixture(authority),
        ),
        Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidSourceContext)
    ));
}

#[test]
fn rebuilt_envelope_cannot_substitute_a_cross_context_transcript_and_sections() {
    let context_a = composite_fixture(24);
    let context_b = composite_fixture(25);
    let envelope = ZkAmsMkheRnsNativeProofEnvelopeV1::new(
        context_a.layout,
        context_a.source_receipt,
        context_b
            .envelope
            .section(ZkAmsMkheRnsNativeProofSectionKindV1::TerminalHyraxBpBridge)
            .to_vec(),
        context_b
            .envelope
            .section(ZkAmsMkheRnsNativeProofSectionKindV1::RnsRelationQpcs)
            .to_vec(),
        context_b
            .envelope
            .section(ZkAmsMkheRnsNativeProofSectionKindV1::CrossFieldGlobalLookup)
            .to_vec(),
        context_b
            .envelope
            .section(ZkAmsMkheRnsNativeProofSectionKindV1::ZeroPadding)
            .to_vec(),
    )
    .expect("transport-valid rebuilt envelope");
    assert!(matches!(
        verify_zk_ams_mkhe_rns_native_composite_v1(
            envelope,
            context_a.layout,
            context_a.source_receipt,
            context_b.transcript,
        ),
        Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidTranscript)
    ));
}

#[test]
fn transcript_section_and_stage_order_splices_are_rejected() {
    let first = composite_fixture(30);
    let second = composite_fixture(31);
    let authority = exact_authority(&first).expect("authority");
    assert!(matches!(
        verify_with_first_party_authority_v1(
            first.envelope,
            first.layout,
            first.source_receipt,
            second.transcript,
            FirstPartyStageAuthorityV1::ExactFixture(authority),
        ),
        Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidTranscript)
    ));

    let baseline = composite_fixture(32);
    let authority = exact_authority(&baseline).expect("authority");
    let foreign = composite_fixture(33);
    let spliced_envelope = ZkAmsMkheRnsNativeProofEnvelopeV1::new(
        baseline.layout,
        baseline.source_receipt,
        baseline
            .envelope
            .section(ZkAmsMkheRnsNativeProofSectionKindV1::TerminalHyraxBpBridge)
            .to_vec(),
        foreign
            .envelope
            .section(ZkAmsMkheRnsNativeProofSectionKindV1::RnsRelationQpcs)
            .to_vec(),
        baseline
            .envelope
            .section(ZkAmsMkheRnsNativeProofSectionKindV1::CrossFieldGlobalLookup)
            .to_vec(),
        baseline
            .envelope
            .section(ZkAmsMkheRnsNativeProofSectionKindV1::ZeroPadding)
            .to_vec(),
    )
    .expect("transport-valid section splice");
    assert!(matches!(
        verify_with_first_party_authority_v1(
            spliced_envelope,
            baseline.layout,
            baseline.source_receipt,
            baseline.transcript,
            FirstPartyStageAuthorityV1::ExactFixture(authority),
        ),
        Err(
            ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidSection(
                ZkAmsMkheRnsNativeVerificationStageV1::RnsRelationQpcs,
            )
        )
    ));

    let fixture = composite_fixture(34);
    let mut authority = exact_authority(&fixture).expect("authority");
    authority.expectations.swap(0, 1);
    assert!(matches!(
        verify_with_first_party_authority_v1(
            fixture.envelope,
            fixture.layout,
            fixture.source_receipt,
            fixture.transcript,
            FirstPartyStageAuthorityV1::ExactFixture(authority),
        ),
        Err(
            ZkAmsMkheRnsNativeCompositeVerificationErrorV1::StageRejected(
                ZkAmsMkheRnsNativeVerificationStageV1::TerminalHyraxBpBridge,
            )
        )
    ));
}

#[test]
fn boundary_exposes_no_accept_all_or_release_authority_surface() {
    let source = include_str!("rns_native_composite_verifier.rs");
    assert!(!source.contains("pub trait"));
    assert!(!source.contains("authorizes_release"));
    assert!(!source.contains("readiness = true"));
    assert!(!source.contains("release_ready = true"));
    let receipt = source
        .find("pub struct ZkAmsMkheRnsNativeCompositeCandidateReceiptV1")
        .expect("candidate receipt declaration");
    let prefix = &source[receipt.saturating_sub(180)..receipt];
    assert!(!prefix.contains("derive(Clone"));
}
