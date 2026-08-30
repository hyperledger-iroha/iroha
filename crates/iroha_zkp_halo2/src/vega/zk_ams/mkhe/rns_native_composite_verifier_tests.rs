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
    composite_fixture_with_axes(context, context, context)
}

fn composite_fixture_with_axes(
    context: u16,
    statement_context: u16,
    operational_context: u16,
) -> CompositeFixtureV1 {
    let profile = zk_ams_mkhe_rns_native_profile_v1().expect("canonical profile");
    let topology = zk_ams_mkhe_rns_native_topology_v1().expect("canonical topology");
    let release = zk_ams_mkhe_rns_native_release_candidate_digest_v1().expect("candidate");
    let layout = ZkAmsMkheRnsNativeSourceLayoutV1::new(
        profile.profile_digest,
        topology.topology_digest,
        release,
        digest(b"statement", statement_context, 0),
        digest(b"operational-context", operational_context, 0),
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
        digest(b"q-mask-s-root", context, 0),
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
    transport: &ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1,
) -> Result<ExactFixtureStageAuthorityV1, ZkAmsMkheRnsNativeCompositeVerificationErrorV1> {
    ExactFixtureStageAuthorityV1::new(transport)
}

fn authenticated_transport(
    fixture: CompositeFixtureV1,
) -> ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1 {
    let canonical = fixture
        .envelope
        .to_canonical_bytes_v1()
        .expect("canonical fixture wire");
    ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1::authenticate_canonical_exact_v1(
        &canonical,
        fixture.layout,
        fixture.source_receipt,
        fixture.transcript,
    )
    .expect("verifier-authenticated fixture transport")
}

#[test]
fn production_boundary_rejects_an_invalid_terminal_kernel() {
    let transport = authenticated_transport(composite_fixture(1));
    assert!(matches!(
        verify_zk_ams_mkhe_rns_native_composite_v1(transport),
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
fn canonical_transport_derives_nonzero_distinct_context_and_commitment_bindings() {
    let fixture = composite_fixture(60);
    let expected_bytes = fixture.envelope.total_wire_bytes();
    let expected_proof = fixture.envelope.proof_digest();
    let transport = authenticated_transport(fixture);
    assert_eq!(transport.canonical_wire_bytes(), expected_bytes);
    let derived = [
        transport.canonical_wire_digest(),
        transport.opening_commitment_root(),
        transport.verifier_context_digest(),
        transport.verifier_transport_digest(),
    ];
    assert!(!derived.contains(&[0; 32]));
    assert!(!derived.contains(&expected_proof));
    assert!(
        !derived
            .iter()
            .enumerate()
            .any(|(index, digest)| derived[index + 1..].contains(digest))
    );
}

#[test]
fn canonical_transport_rejects_wire_mutation_and_every_truncation_class() {
    let fixture = composite_fixture(61);
    let mut canonical = fixture
        .envelope
        .to_canonical_bytes_v1()
        .expect("canonical fixture wire");
    let last = canonical.len() - 1;
    canonical[last] ^= 1;
    assert!(matches!(
        ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1::authenticate_canonical_exact_v1(
            &canonical,
            fixture.layout,
            fixture.source_receipt,
            fixture.transcript,
        ),
        Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidEnvelopeContext)
    ));

    for (index, truncation) in [
        0_usize,
        ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_HEADER_BYTES_V1 - 1,
    ]
    .into_iter()
    .enumerate()
    {
        let fixture = composite_fixture(u16::try_from(62 + index).expect("test context"));
        let mut canonical = fixture
            .envelope
            .to_canonical_bytes_v1()
            .expect("canonical fixture wire");
        canonical.truncate(truncation);
        assert!(matches!(
            ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1::authenticate_canonical_exact_v1(
                &canonical,
                fixture.layout,
                fixture.source_receipt,
                fixture.transcript,
            ),
            Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidEnvelopeContext)
        ));
    }
    let fixture = composite_fixture(64);
    let mut canonical = fixture
        .envelope
        .to_canonical_bytes_v1()
        .expect("canonical fixture wire");
    canonical.truncate(canonical.len() - 1);
    assert!(matches!(
        ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1::authenticate_canonical_exact_v1(
            &canonical,
            fixture.layout,
            fixture.source_receipt,
            fixture.transcript,
        ),
        Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidEnvelopeContext)
    ));
}

#[test]
fn canonical_transport_rejects_replay_under_a_fresh_operational_context() {
    let captured = composite_fixture_with_axes(66, 66, 66);
    let fresh_context = composite_fixture_with_axes(66, 66, 67);
    assert_eq!(
        captured.layout.statement_digest(),
        fresh_context.layout.statement_digest()
    );
    assert_ne!(
        captured.layout.operational_context_digest(),
        fresh_context.layout.operational_context_digest()
    );
    let canonical = captured
        .envelope
        .to_canonical_bytes_v1()
        .expect("captured canonical wire");
    assert!(matches!(
        ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1::authenticate_canonical_exact_v1(
            &canonical,
            fresh_context.layout,
            fresh_context.source_receipt,
            fresh_context.transcript,
        ),
        Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidEnvelopeContext)
    ));
}

#[test]
fn canonical_transport_rejects_commitment_substitution_after_outer_digest_rebuild() {
    let fixture = composite_fixture(65);
    let mut terminal = fixture
        .envelope
        .section(ZkAmsMkheRnsNativeProofSectionKindV1::TerminalHyraxBpBridge)
        .to_vec();
    let first_source_commitment = TYPED_SECTION_COMMON_PREFIX_BYTES_V1 + 1 + 5 * 32 + 3;
    terminal[first_source_commitment] ^= 1;
    let envelope = ZkAmsMkheRnsNativeProofEnvelopeV1::new(
        fixture.layout,
        fixture.source_receipt,
        terminal,
        fixture
            .envelope
            .section(ZkAmsMkheRnsNativeProofSectionKindV1::RnsRelationQpcs)
            .to_vec(),
        fixture
            .envelope
            .section(ZkAmsMkheRnsNativeProofSectionKindV1::CrossFieldGlobalLookup)
            .to_vec(),
        fixture
            .envelope
            .section(ZkAmsMkheRnsNativeProofSectionKindV1::ZeroPadding)
            .to_vec(),
    )
    .expect("outer transport rebuilt around substituted commitment");
    let canonical = envelope
        .to_canonical_bytes_v1()
        .expect("canonical rebuilt wire");
    assert!(matches!(
        ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1::authenticate_canonical_exact_v1(
            &canonical,
            fixture.layout,
            fixture.source_receipt,
            fixture.transcript,
        ),
        Err(
            ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidSection(
                ZkAmsMkheRnsNativeVerificationStageV1::TerminalHyraxBpBridge,
            )
        )
    ));
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
    let canonical = envelope
        .to_canonical_bytes_v1()
        .expect("canonical malformed wire");
    assert!(matches!(
        ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1::authenticate_canonical_exact_v1(
            &canonical,
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
    let canonical = envelope
        .to_canonical_bytes_v1()
        .expect("canonical aliased wire");
    assert!(matches!(
        ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1::authenticate_canonical_exact_v1(
            &canonical,
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
    let expected_statement = fixture.layout.statement_digest();
    let expected_source = fixture.layout.source_binding_digest();
    let expected_roster = fixture.transcript.governed_roster_digest();
    let expected_ciphertext = fixture.transcript.public_ciphertext_digest();
    let expected_transcript = fixture.transcript.transcript_digest();
    let expected_proof = fixture.envelope.proof_digest();
    let transport = authenticated_transport(fixture);
    let expected_wire = transport.canonical_wire_digest();
    let expected_opening_root = transport.opening_commitment_root();
    let expected_context = transport.verifier_context_digest();
    let expected_transport = transport.verifier_transport_digest();
    let authority = exact_authority(&transport).expect("exact fixture authority");
    let receipt = verify_with_first_party_authority_v1(
        transport,
        FirstPartyStageAuthorityV1::ExactFixture(Box::new(authority)),
    )
    .expect("all exact fixture stages verify");
    assert_eq!(receipt.statement_digest(), expected_statement);
    assert_eq!(receipt.source_binding_digest(), expected_source);
    assert_eq!(receipt.governed_roster_digest(), expected_roster);
    assert_eq!(receipt.public_ciphertext_digest(), expected_ciphertext);
    assert_eq!(receipt.transcript_digest(), expected_transcript);
    assert_eq!(receipt.proof_digest(), expected_proof);
    assert_eq!(receipt.canonical_wire_digest(), expected_wire);
    assert_eq!(receipt.opening_commitment_root(), expected_opening_root);
    assert_eq!(receipt.verifier_context_digest(), expected_context);
    assert_eq!(receipt.verifier_transport_digest(), expected_transport);
    assert_ne!(receipt.candidate_digest(), [0; 32]);
    assert!(
        !receipt
            .section_digests()
            .contains(&receipt.candidate_digest())
    );
}

#[test]
fn exact_stage_chain_mints_one_integrity_bound_algebraic_receipt() {
    let fixture = composite_fixture(4);
    let expected_profile = fixture.envelope.profile_manifest_digest();
    let expected_topology = fixture.envelope.topology_digest();
    let expected_release = fixture.envelope.release_candidate_digest();
    let expected_statement = fixture.layout.statement_digest();
    let expected_operation = fixture.layout.operational_context_digest();
    let expected_source = fixture.layout.source_binding_digest();
    let expected_source_receipt = fixture.source_receipt.receipt_digest;
    let expected_roster = fixture.transcript.governed_roster_digest();
    let expected_ciphertext = fixture.transcript.public_ciphertext_digest();
    let expected_transcript = fixture.transcript.transcript_digest();
    let expected_proof = fixture.envelope.proof_digest();
    let transport = authenticated_transport(fixture);
    let expected_wire_bytes = transport.canonical_wire_bytes();
    let expected_wire = transport.canonical_wire_digest();
    let expected_opening_root = transport.opening_commitment_root();
    let expected_context = transport.verifier_context_digest();
    let expected_transport = transport.verifier_transport_digest();
    let authority = exact_authority(&transport).expect("exact fixture authority");
    let receipt = verify_algebraic_with_first_party_authority_v1(
        transport,
        FirstPartyStageAuthorityV1::ExactFixture(Box::new(authority)),
    )
    .expect("all exact fixture stages mint the receipt");

    receipt.validate_v1().expect("algebraic receipt validates");
    assert_eq!(receipt.profile_manifest_digest(), expected_profile);
    assert_eq!(receipt.topology_digest(), expected_topology);
    assert_eq!(receipt.release_candidate_digest(), expected_release);
    assert_eq!(receipt.statement_digest(), expected_statement);
    assert_eq!(receipt.operational_context_digest(), expected_operation);
    assert_eq!(receipt.source_binding_digest(), expected_source);
    assert_eq!(receipt.source_receipt_digest(), expected_source_receipt);
    assert_eq!(receipt.governed_roster_digest(), expected_roster);
    assert_eq!(receipt.public_ciphertext_digest(), expected_ciphertext);
    assert_eq!(receipt.transcript_digest(), expected_transcript);
    assert_eq!(receipt.proof_digest(), expected_proof);
    assert_eq!(receipt.canonical_wire_bytes(), expected_wire_bytes);
    assert_eq!(receipt.canonical_wire_digest(), expected_wire);
    assert_eq!(receipt.opening_commitment_root(), expected_opening_root);
    assert_eq!(receipt.verifier_context_digest(), expected_context);
    assert_eq!(receipt.verifier_transport_digest(), expected_transport);
    assert_ne!(receipt.composite_candidate_digest(), [0; 32]);
    assert_ne!(receipt.receipt_digest(), [0; 32]);
    assert_ne!(
        receipt.receipt_digest(),
        receipt.composite_candidate_digest()
    );
    assert!(
        !receipt
            .section_digests()
            .contains(&receipt.receipt_digest())
    );
}

#[test]
fn mutated_or_digest_rebuilt_candidates_cannot_mint_or_validate_a_receipt() {
    let transport = authenticated_transport(composite_fixture(5));
    let authority = exact_authority(&transport).expect("exact fixture authority");
    let mut candidate = verify_with_first_party_authority_v1(
        transport,
        FirstPartyStageAuthorityV1::ExactFixture(Box::new(authority)),
    )
    .expect("exact candidate");
    candidate.statement_digest[0] ^= 1;
    candidate.candidate_digest = candidate_receipt_digest_v1(&candidate);
    assert!(matches!(
        candidate.into_algebraic_receipt_v1(),
        Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidTranscript)
    ));

    let transport = authenticated_transport(composite_fixture(6));
    let authority = exact_authority(&transport).expect("exact fixture authority");
    let mut receipt = verify_algebraic_with_first_party_authority_v1(
        transport,
        FirstPartyStageAuthorityV1::ExactFixture(Box::new(authority)),
    )
    .expect("exact algebraic receipt");
    receipt.receipt_digest[0] ^= 1;
    assert!(matches!(
        receipt.validate_v1(),
        Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidTranscript)
    ));

    let transport = authenticated_transport(composite_fixture(7));
    let authority = exact_authority(&transport).expect("exact fixture authority");
    let mut receipt = verify_algebraic_with_first_party_authority_v1(
        transport,
        FirstPartyStageAuthorityV1::ExactFixture(Box::new(authority)),
    )
    .expect("exact algebraic receipt");
    receipt.composite.section_digests[0][0] ^= 1;
    receipt.composite.candidate_digest = candidate_receipt_digest_v1(&receipt.composite);
    receipt.receipt_digest = algebraic_receipt_digest_v1(&receipt.composite);
    assert!(matches!(
        receipt.validate_v1(),
        Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidTranscript)
    ));
}

#[test]
fn rejection_at_any_stage_never_yields_an_algebraic_receipt() {
    for (index, stage) in VERIFICATION_STAGE_ORDER_V1.into_iter().enumerate() {
        let transport = authenticated_transport(composite_fixture(
            u16::try_from(40 + index).expect("fixture context fits u16"),
        ));
        let authority = exact_authority(&transport)
            .expect("exact authority")
            .reject(stage);
        assert!(matches!(
            verify_algebraic_with_first_party_authority_v1(
                transport,
                FirstPartyStageAuthorityV1::ExactFixture(Box::new(authority)),
            ),
            Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::StageRejected(
                rejected,
            )) if rejected == stage
        ));
    }
}

#[test]
fn rejection_at_any_stage_never_yields_a_candidate() {
    for (index, stage) in VERIFICATION_STAGE_ORDER_V1.into_iter().enumerate() {
        let transport = authenticated_transport(composite_fixture(
            u16::try_from(10 + index).expect("fixture context fits u16"),
        ));
        let authority = exact_authority(&transport)
            .expect("exact authority")
            .reject(stage);
        assert!(matches!(
            verify_with_first_party_authority_v1(
                transport,
                FirstPartyStageAuthorityV1::ExactFixture(Box::new(authority)),
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
    let canonical = first
        .envelope
        .to_canonical_bytes_v1()
        .expect("first canonical wire");
    assert!(matches!(
        ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1::authenticate_canonical_exact_v1(
            &canonical,
            second.layout,
            second.source_receipt,
            first.transcript,
        ),
        Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidEnvelopeContext)
    ));

    let first = composite_fixture(22);
    let second = composite_fixture(23);
    let canonical = first
        .envelope
        .to_canonical_bytes_v1()
        .expect("first canonical wire");
    assert!(matches!(
        ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1::authenticate_canonical_exact_v1(
            &canonical,
            second.layout,
            first.source_receipt,
            first.transcript,
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
    let canonical = envelope
        .to_canonical_bytes_v1()
        .expect("rebuilt canonical wire");
    assert!(matches!(
        ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1::authenticate_canonical_exact_v1(
            &canonical,
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
    let canonical = first
        .envelope
        .to_canonical_bytes_v1()
        .expect("first canonical wire");
    assert!(matches!(
        ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1::authenticate_canonical_exact_v1(
            &canonical,
            first.layout,
            first.source_receipt,
            second.transcript,
        ),
        Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidTranscript)
    ));

    let baseline = composite_fixture(32);
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
    let canonical = spliced_envelope
        .to_canonical_bytes_v1()
        .expect("spliced canonical wire");
    assert!(matches!(
        ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1::authenticate_canonical_exact_v1(
            &canonical,
            baseline.layout,
            baseline.source_receipt,
            baseline.transcript,
        ),
        Err(
            ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidSection(
                ZkAmsMkheRnsNativeVerificationStageV1::RnsRelationQpcs,
            )
        )
    ));

    let transport = authenticated_transport(composite_fixture(34));
    let mut authority = exact_authority(&transport).expect("authority");
    authority.expectations.swap(0, 1);
    assert!(matches!(
        verify_with_first_party_authority_v1(
            transport,
            FirstPartyStageAuthorityV1::ExactFixture(Box::new(authority)),
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
    let transport = source
        .find("pub struct ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1")
        .expect("verifier transport declaration");
    let transport_prefix = &source[transport.saturating_sub(560)..transport];
    for forbidden_derive in [
        "derive(Clone",
        "derive(Copy",
        "derive(Default",
        "Encode",
        "Decode",
        "Serialize",
        "Deserialize",
        "Norito",
    ] {
        assert!(!transport_prefix.contains(forbidden_derive));
    }
    let transport_body = source[transport..]
        .split("impl ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1")
        .next()
        .expect("transport body");
    assert!(!transport_body.contains("pub envelope:"));
    assert!(!transport_body.contains("pub transcript:"));
    let transport_impl = source
        .split("impl ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1")
        .nth(1)
        .expect("transport implementation")
        .split("/// Move-only proof-verification candidate")
        .next()
        .expect("transport implementation boundary");
    assert!(transport_impl.contains("pub fn authenticate_canonical_exact_v1("));
    assert!(!transport_impl.contains("pub fn new("));
    assert!(!transport_impl.contains("pub fn decode("));
    assert!(!transport_impl.contains("pub fn to_canonical_bytes"));
    for verifier_name in [
        "pub fn verify_zk_ams_mkhe_rns_native_composite_v1(",
        "pub fn verify_zk_ams_mkhe_rns_native_algebraic_v1(",
    ] {
        let signature = source
            .split(verifier_name)
            .nth(1)
            .expect("public verifier")
            .split(") -> Result")
            .next()
            .expect("public verifier signature");
        assert!(
            signature.contains("transport: ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1")
        );
        assert!(!signature.contains("ZkAmsMkheRnsNativeProofEnvelopeV1"));
        assert!(!signature.contains("ZkAmsMkheRnsNativeSourceLayoutV1"));
        assert!(!signature.contains("ZkAmsMkheRnsNativeSourceReceiptV1"));
        assert!(!signature.contains("ZkAmsMkheRnsNativeChallengeSeedsV1"));
    }
    let receipt = source
        .find("pub struct ZkAmsMkheRnsNativeCompositeCandidateReceiptV1")
        .expect("candidate receipt declaration");
    let prefix = &source[receipt.saturating_sub(180)..receipt];
    assert!(!prefix.contains("derive(Clone"));

    let algebraic = source
        .find("pub struct ZkAmsMkheRnsNativeAlgebraicReceiptV1")
        .expect("algebraic receipt declaration");
    let algebraic_prefix = &source[algebraic.saturating_sub(360)..algebraic];
    for forbidden_derive in [
        "derive(Clone",
        "derive(Copy",
        "derive(Default",
        "Encode",
        "Decode",
        "Serialize",
        "Deserialize",
        "Norito",
    ] {
        assert!(!algebraic_prefix.contains(forbidden_derive));
    }
    let algebraic_body = source[algebraic..]
        .split("impl ZkAmsMkheRnsNativeAlgebraicReceiptV1")
        .next()
        .expect("algebraic receipt body");
    assert!(!algebraic_body.contains("pub composite:"));
    assert!(!algebraic_body.contains("pub receipt_digest:"));
    let algebraic_impl = source
        .split("impl ZkAmsMkheRnsNativeAlgebraicReceiptV1")
        .nth(1)
        .expect("algebraic receipt implementation")
        .split("/// Atomically verify one replacement composite proof.")
        .next()
        .expect("algebraic receipt implementation boundary");
    assert!(!algebraic_impl.contains("pub fn new"));
    assert!(!algebraic_impl.contains("pub fn from"));
    assert!(!algebraic_impl.contains("pub fn decode"));
    assert!(!algebraic_impl.contains("pub fn deserialize"));
    assert!(!source.contains("pub fn into_algebraic_receipt_v1"));
    assert!(!source.contains("pub fn from_verified_composite_v1"));
    assert!(!source.contains("impl Default for ZkAmsMkheRnsNativeAlgebraicReceiptV1"));
    assert!(!source.contains("impl Clone for ZkAmsMkheRnsNativeAlgebraicReceiptV1"));
    assert!(!source.contains("impl Copy for ZkAmsMkheRnsNativeAlgebraicReceiptV1"));
    assert!(!source.contains("impl Decode for ZkAmsMkheRnsNativeAlgebraicReceiptV1"));
    assert!(!source.contains("impl Deserialize for ZkAmsMkheRnsNativeAlgebraicReceiptV1"));
    let mint = source
        .split("fn verify_algebraic_with_first_party_authority_v1")
        .nth(1)
        .expect("sealed algebraic mint")
        .split("struct CandidateAxesV1")
        .next()
        .expect("sealed algebraic mint boundary");
    assert!(mint.contains("verify_with_first_party_authority_v1("));
    assert!(mint.contains(".into_algebraic_receipt_v1()"));
    assert!(!mint.contains("bool"));
    assert!(!mint.contains("receipt_digest:"));
}
