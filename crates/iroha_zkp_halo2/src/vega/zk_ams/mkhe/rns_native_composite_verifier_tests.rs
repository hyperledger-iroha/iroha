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
    rns_native_zero_padding_commitment::deterministic_zero_padding_stage_fixture_v1,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
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

struct InvalidReceiptSnapshot {
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
}

impl ZkAmsMkheRnsNativeSourceSnapshotV1 for InvalidReceiptSnapshot {
    type Chunk = TestChunk;

    fn layout(&self) -> ZkAmsMkheRnsNativeSourceLayoutV1 {
        self.layout
    }

    fn snapshot_digest(&self, _arena: ZkAmsMkheRnsNativeSourceArenaV1) -> [u8; 32] {
        [0; 32]
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
    context: u16,
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    source_receipt: ZkAmsMkheRnsNativeSourceReceiptV1,
    envelope: ZkAmsMkheRnsNativeProofEnvelopeV1,
    transcript: ZkAmsMkheRnsNativeChallengeSeedsV1,
}

impl CompositeFixtureV1 {
    fn source_snapshot(&self) -> TestSnapshot {
        TestSnapshot {
            layout: self.layout,
            context: self.context,
        }
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

fn composite_fixture(context: u16) -> CompositeFixtureV1 {
    composite_fixture_with_zero_padding(context, digest(b"zero-padding-root", context, 0), None)
}

fn composite_fixture_with_zero_padding(
    context: u16,
    zero_padding_root: [u8; 32],
    zero_padding_fixture: Option<(&[[u8; 32]; 40], &[u8])>,
) -> CompositeFixtureV1 {
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
        zero_padding_root,
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
    let (padding_digests, padding_proof): (&[[u8; 32]; 40], &[u8]) =
        zero_padding_fixture.unwrap_or((&padding_digests, &padding_proof));
    let padding_section =
        ZkAmsMkheRnsNativeZeroPaddingSectionV1::new(&transcript, padding_digests, padding_proof)
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
        context,
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

fn rebuild_envelope_with_cross_and_padding(
    fixture: &CompositeFixtureV1,
    cross_section: Vec<u8>,
    padding_section: Vec<u8>,
) -> ZkAmsMkheRnsNativeProofEnvelopeV1 {
    ZkAmsMkheRnsNativeProofEnvelopeV1::new(
        fixture.layout,
        fixture.source_receipt,
        fixture
            .envelope
            .section(ZkAmsMkheRnsNativeProofSectionKindV1::TerminalHyraxBpBridge)
            .to_vec(),
        fixture
            .envelope
            .section(ZkAmsMkheRnsNativeProofSectionKindV1::RnsRelationQpcs)
            .to_vec(),
        cross_section,
        padding_section,
    )
    .expect("transport-valid rebuilt envelope")
}

fn rebuild_envelope_with_terminal_and_rns(
    fixture: &CompositeFixtureV1,
    terminal_section: Vec<u8>,
    rns_section: Vec<u8>,
) -> ZkAmsMkheRnsNativeProofEnvelopeV1 {
    ZkAmsMkheRnsNativeProofEnvelopeV1::new(
        fixture.layout,
        fixture.source_receipt,
        terminal_section,
        rns_section,
        fixture
            .envelope
            .section(ZkAmsMkheRnsNativeProofSectionKindV1::CrossFieldGlobalLookup)
            .to_vec(),
        fixture
            .envelope
            .section(ZkAmsMkheRnsNativeProofSectionKindV1::ZeroPadding)
            .to_vec(),
    )
    .expect("transport-valid rebuilt envelope")
}

struct RetainedAuthorityDropProbe {
    name: &'static str,
    finish_materialized: Rc<Cell<bool>>,
    dropped: Rc<Cell<bool>>,
}

impl Drop for RetainedAuthorityDropProbe {
    fn drop(&mut self) {
        assert!(
            self.finish_materialized.get(),
            "{} dropped before the composite result was materialized",
            self.name,
        );
        self.dropped.set(true);
    }
}

#[test]
fn production_boundary_rejects_an_invalid_terminal_kernel() {
    let fixture = composite_fixture(1);
    let source_snapshot = fixture.source_snapshot();
    assert!(matches!(
        verify_zk_ams_mkhe_rns_native_composite_v1(
            fixture.envelope,
            source_snapshot,
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
    let source_snapshot = fixture.source_snapshot();
    assert!(matches!(
        verify_zk_ams_mkhe_rns_native_composite_v1(envelope, source_snapshot, fixture.transcript,),
        Err(
            ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidSection(
                ZkAmsMkheRnsNativeVerificationStageV1::RnsRelationQpcs,
            )
        )
    ));
}

#[test]
fn source_bound_context_rejects_terminal_and_rns_framing_before_authentication() {
    let terminal_fixture = composite_fixture(41);
    let mut malformed_terminal = terminal_fixture
        .envelope
        .section(ZkAmsMkheRnsNativeProofSectionKindV1::TerminalHyraxBpBridge)
        .to_vec();
    malformed_terminal.push(0);
    let terminal_envelope = rebuild_envelope_with_terminal_and_rns(
        &terminal_fixture,
        malformed_terminal,
        terminal_fixture
            .envelope
            .section(ZkAmsMkheRnsNativeProofSectionKindV1::RnsRelationQpcs)
            .to_vec(),
    );
    let terminal_auth_called = Cell::new(false);
    let qpcs_auth_called = Cell::new(false);
    assert!(matches!(
        validate_then_authenticate_source_bound_context_v2(
            &terminal_envelope,
            terminal_fixture.layout,
            terminal_fixture.source_receipt,
            terminal_fixture.transcript,
            |_| {
                terminal_auth_called.set(true);
                Ok(())
            },
            |_| {
                qpcs_auth_called.set(true);
                Ok(())
            },
        ),
        Err(
            ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidSection(
                ZkAmsMkheRnsNativeVerificationStageV1::TerminalHyraxBpBridge,
            )
        )
    ));
    assert!(!terminal_auth_called.get());
    assert!(!qpcs_auth_called.get());

    let qpcs_fixture = composite_fixture(42);
    let mut malformed_qpcs = qpcs_fixture
        .envelope
        .section(ZkAmsMkheRnsNativeProofSectionKindV1::RnsRelationQpcs)
        .to_vec();
    malformed_qpcs.push(0);
    let qpcs_envelope = rebuild_envelope_with_terminal_and_rns(
        &qpcs_fixture,
        qpcs_fixture
            .envelope
            .section(ZkAmsMkheRnsNativeProofSectionKindV1::TerminalHyraxBpBridge)
            .to_vec(),
        malformed_qpcs,
    );
    let terminal_auth_called = Cell::new(false);
    let qpcs_auth_called = Cell::new(false);
    assert!(matches!(
        validate_then_authenticate_source_bound_context_v2(
            &qpcs_envelope,
            qpcs_fixture.layout,
            qpcs_fixture.source_receipt,
            qpcs_fixture.transcript,
            |_| {
                terminal_auth_called.set(true);
                Ok(())
            },
            |_| {
                qpcs_auth_called.set(true);
                Ok(())
            },
        ),
        Err(
            ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidSection(
                ZkAmsMkheRnsNativeVerificationStageV1::RnsRelationQpcs,
            )
        )
    ));
    assert!(!terminal_auth_called.get());
    assert!(!qpcs_auth_called.get());

    let ordered_fixture = composite_fixture(44);
    let authentication_order = RefCell::new(Vec::new());
    let checked = validate_then_authenticate_source_bound_context_v2(
        &ordered_fixture.envelope,
        ordered_fixture.layout,
        ordered_fixture.source_receipt,
        ordered_fixture.transcript,
        |_| {
            authentication_order.borrow_mut().push("terminal");
            Ok(())
        },
        |_| {
            authentication_order.borrow_mut().push("qpcs");
            Ok(())
        },
    )
    .expect("valid context reaches both authenticators");
    assert_eq!(authentication_order.into_inner(), vec!["terminal", "qpcs"]);
    drop(checked);
}

#[test]
fn source_bound_stage_router_uses_each_exact_authenticator() {
    let fixture = composite_fixture(43);
    let axes = validate_context_v1(
        &fixture.envelope,
        fixture.layout,
        fixture.source_receipt,
        &fixture.transcript,
    )
    .expect("validated source-bound context");
    let authority = FirstPartyStageAuthorityV1::ProductionSourceBoundAllStages;

    for stage in [
        ZkAmsMkheRnsNativeVerificationStageV1::TerminalHyraxBpBridge,
        ZkAmsMkheRnsNativeVerificationStageV1::RnsRelationQpcs,
    ] {
        let descriptor = fixture.envelope.descriptors()[stage.index()];
        assert!(
            authority
                .verify_v1(stage, &axes, &fixture.transcript, descriptor, &[])
                .is_ok(),
            "preauthenticated {stage:?} must not run a detached authenticator",
        );
    }

    let cross_field = ZkAmsMkheRnsNativeVerificationStageV1::CrossFieldGlobalLookup;
    let cross_descriptor = fixture.envelope.descriptors()[cross_field.index()];
    assert!(
        authority
            .verify_v1(
                cross_field,
                &axes,
                &fixture.transcript,
                cross_descriptor,
                fixture.envelope.section(cross_field.section_kind()),
            )
            .is_ok(),
        "the source-bound cross-field section must use its typed authenticator",
    );
    assert!(matches!(
        authority.verify_v1(
            cross_field,
            &axes,
            &fixture.transcript,
            cross_descriptor,
            &[],
        ),
        Err(
            ZkAmsMkheRnsNativeCompositeVerificationErrorV1::StageRejected(
                ZkAmsMkheRnsNativeVerificationStageV1::CrossFieldGlobalLookup,
            )
        )
    ));

    let (padding_root, padding_digests, padding_proof) =
        deterministic_zero_padding_stage_fixture_v1(|root| {
            composite_fixture_with_zero_padding(45, root, None).transcript
        })
        .expect("valid zero-padding stage fixture");
    let padding_fixture = composite_fixture_with_zero_padding(
        45,
        padding_root,
        Some((&padding_digests, &padding_proof)),
    );
    let padding_axes = validate_context_v1(
        &padding_fixture.envelope,
        padding_fixture.layout,
        padding_fixture.source_receipt,
        &padding_fixture.transcript,
    )
    .expect("validated zero-padding source-bound context");
    let zero_padding = ZkAmsMkheRnsNativeVerificationStageV1::ZeroPadding;
    let padding_descriptor = padding_fixture.envelope.descriptors()[zero_padding.index()];
    assert!(
        authority
            .verify_v1(
                zero_padding,
                &padding_axes,
                &padding_fixture.transcript,
                padding_descriptor,
                padding_fixture
                    .envelope
                    .section(zero_padding.section_kind()),
            )
            .is_ok(),
        "the source-bound zero-padding section must use its typed authenticator",
    );

    let mut mutated_padding_proof = padding_proof;
    let mutation_index = mutated_padding_proof.len() / 2;
    mutated_padding_proof[mutation_index] ^= 1;
    let mutated_padding_section = ZkAmsMkheRnsNativeZeroPaddingSectionV1::new(
        &padding_fixture.transcript,
        &padding_digests,
        &mutated_padding_proof,
    )
    .expect("mutated zero-padding section")
    .to_canonical_bytes_v1()
    .expect("mutated zero-padding encoding");
    assert!(matches!(
        authority.verify_v1(
            zero_padding,
            &padding_axes,
            &padding_fixture.transcript,
            padding_descriptor,
            &mutated_padding_section,
        ),
        Err(
            ZkAmsMkheRnsNativeCompositeVerificationErrorV1::StageRejected(
                ZkAmsMkheRnsNativeVerificationStageV1::ZeroPadding,
            )
        )
    ));
    assert!(matches!(
        authority.verify_v1(
            zero_padding,
            &padding_axes,
            &padding_fixture.transcript,
            padding_descriptor,
            &[],
        ),
        Err(
            ZkAmsMkheRnsNativeCompositeVerificationErrorV1::StageRejected(
                ZkAmsMkheRnsNativeVerificationStageV1::ZeroPadding,
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
    let source_snapshot = fixture.source_snapshot();
    assert!(matches!(
        verify_zk_ams_mkhe_rns_native_composite_v1(envelope, source_snapshot, fixture.transcript,),
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
    let source_snapshot = fixture.source_snapshot();
    let receipt = verify_with_first_party_authority_v1(
        fixture.envelope,
        source_snapshot,
        fixture.transcript,
        FirstPartyStageAuthorityV1::ExactFixture(Box::new(authority)),
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
        let source_snapshot = fixture.source_snapshot();
        assert!(matches!(
            verify_with_first_party_authority_v1(
                fixture.envelope,
                source_snapshot,
                fixture.transcript,
                FirstPartyStageAuthorityV1::ExactFixture(Box::new(authority)),
            ),
            Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::StageRejected(
                rejected,
            )) if rejected == stage
        ));
    }
}

#[test]
fn source_owner_and_envelope_context_substitution_is_rejected_before_stages() {
    let first = composite_fixture(20);
    let second = composite_fixture(21);
    let authority = exact_authority(&first).expect("authority");
    let foreign_source_snapshot = second.source_snapshot();
    assert!(matches!(
        verify_with_first_party_authority_v1(
            first.envelope,
            foreign_source_snapshot,
            first.transcript,
            FirstPartyStageAuthorityV1::ExactFixture(Box::new(authority)),
        ),
        Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidEnvelopeContext)
    ));

    let first = composite_fixture(22);
    let second = composite_fixture(23);
    let authority = exact_authority(&first).expect("authority");
    // A caller can substitute only a whole owner. Even when its layout matches,
    // its derived structural receipt remains bound to the foreign snapshot.
    let foreign_snapshot_state = TestSnapshot {
        layout: first.layout,
        context: second.context,
    };
    assert!(matches!(
        verify_with_first_party_authority_v1(
            first.envelope,
            foreign_snapshot_state,
            first.transcript,
            FirstPartyStageAuthorityV1::ExactFixture(Box::new(authority)),
        ),
        Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidEnvelopeContext)
    ));
}

#[test]
fn invalid_owned_snapshot_receipt_is_rejected_before_stages() {
    let fixture = composite_fixture(26);
    let authority = exact_authority(&fixture).expect("authority");
    let invalid_source_snapshot = InvalidReceiptSnapshot {
        layout: fixture.layout,
    };
    assert!(matches!(
        verify_with_first_party_authority_v1(
            fixture.envelope,
            invalid_source_snapshot,
            fixture.transcript,
            FirstPartyStageAuthorityV1::ExactFixture(Box::new(authority)),
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
    let source_snapshot = context_a.source_snapshot();
    assert!(matches!(
        verify_zk_ams_mkhe_rns_native_composite_v1(envelope, source_snapshot, context_b.transcript,),
        Err(ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidTranscript)
    ));
}

#[test]
fn transcript_section_and_stage_order_splices_are_rejected() {
    let first = composite_fixture(30);
    let second = composite_fixture(31);
    let authority = exact_authority(&first).expect("authority");
    let source_snapshot = first.source_snapshot();
    assert!(matches!(
        verify_with_first_party_authority_v1(
            first.envelope,
            source_snapshot,
            second.transcript,
            FirstPartyStageAuthorityV1::ExactFixture(Box::new(authority)),
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
    let source_snapshot = baseline.source_snapshot();
    assert!(matches!(
        verify_with_first_party_authority_v1(
            spliced_envelope,
            source_snapshot,
            baseline.transcript,
            FirstPartyStageAuthorityV1::ExactFixture(Box::new(authority)),
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
    let source_snapshot = fixture.source_snapshot();
    assert!(matches!(
        verify_with_first_party_authority_v1(
            fixture.envelope,
            source_snapshot,
            fixture.transcript,
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
fn omitted_duplicated_and_trailing_lookup_padding_frames_are_rejected_atomically() {
    let omitted = composite_fixture(35);
    let mut omitted_cross = omitted
        .envelope
        .section(ZkAmsMkheRnsNativeProofSectionKindV1::CrossFieldGlobalLookup)
        .to_vec();
    omitted_cross.pop().expect("nonempty cross section");
    let omitted_padding = omitted
        .envelope
        .section(ZkAmsMkheRnsNativeProofSectionKindV1::ZeroPadding)
        .to_vec();
    let omitted_envelope =
        rebuild_envelope_with_cross_and_padding(&omitted, omitted_cross, omitted_padding);
    let omitted_source = omitted.source_snapshot();
    assert!(matches!(
        verify_zk_ams_mkhe_rns_native_composite_v1(
            omitted_envelope,
            omitted_source,
            omitted.transcript,
        ),
        Err(
            ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidSection(
                ZkAmsMkheRnsNativeVerificationStageV1::CrossFieldGlobalLookup,
            )
        )
    ));

    let duplicated = composite_fixture(36);
    let duplicated_cross = duplicated
        .envelope
        .section(ZkAmsMkheRnsNativeProofSectionKindV1::RnsRelationQpcs)
        .to_vec();
    let duplicated_padding = duplicated
        .envelope
        .section(ZkAmsMkheRnsNativeProofSectionKindV1::ZeroPadding)
        .to_vec();
    let duplicated_envelope =
        rebuild_envelope_with_cross_and_padding(&duplicated, duplicated_cross, duplicated_padding);
    let duplicated_source = duplicated.source_snapshot();
    assert!(matches!(
        verify_zk_ams_mkhe_rns_native_composite_v1(
            duplicated_envelope,
            duplicated_source,
            duplicated.transcript,
        ),
        Err(
            ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidSection(
                ZkAmsMkheRnsNativeVerificationStageV1::CrossFieldGlobalLookup,
            )
        )
    ));

    let trailing = composite_fixture(37);
    let trailing_cross = trailing
        .envelope
        .section(ZkAmsMkheRnsNativeProofSectionKindV1::CrossFieldGlobalLookup)
        .to_vec();
    let mut trailing_padding = trailing
        .envelope
        .section(ZkAmsMkheRnsNativeProofSectionKindV1::ZeroPadding)
        .to_vec();
    trailing_padding.push(0);
    let trailing_envelope =
        rebuild_envelope_with_cross_and_padding(&trailing, trailing_cross, trailing_padding);
    let trailing_source = trailing.source_snapshot();
    assert!(matches!(
        verify_zk_ams_mkhe_rns_native_composite_v1(
            trailing_envelope,
            trailing_source,
            trailing.transcript,
        ),
        Err(
            ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidSection(
                ZkAmsMkheRnsNativeVerificationStageV1::ZeroPadding,
            )
        )
    ));
}

#[test]
fn composite_stage_cursor_rejects_skips_before_any_partial_stage_can_escape() {
    let fixture = composite_fixture(38);
    let authority = exact_authority(&fixture).expect("exact authority");
    let checked = ContextCheckedV1::new(
        &fixture.envelope,
        fixture.layout,
        fixture.source_receipt,
        fixture.transcript,
        FirstPartyStageAuthorityV1::ExactFixture(Box::new(authority)),
    )
    .expect("context-bound composite");
    assert!(matches!(
        checked.verify_stage_v1(ZkAmsMkheRnsNativeVerificationStageV1::RnsRelationQpcs),
        Err(
            ZkAmsMkheRnsNativeCompositeVerificationErrorV1::InvalidSection(
                ZkAmsMkheRnsNativeVerificationStageV1::RnsRelationQpcs,
            )
        )
    ));

    let source = include_str!("rns_native_composite_verifier.rs");
    let cursor = source
        .split_once("fn verify_stage_v1(")
        .and_then(|(_, suffix)| suffix.split_once("fn verify_terminal_bridge_v1"))
        .map(|(cursor, _)| cursor)
        .expect("stage cursor implementation");
    assert!(cursor.contains("mut self"));
    assert!(cursor.contains("stage.index() != self.next_stage"));
    assert!(cursor.contains("self.next_stage = self"));
    assert!(cursor.contains(".checked_add(1)"));
    assert!(!cursor.contains("&mut self"));
}

#[test]
fn retained_authority_set_outlives_composite_result_materialization() {
    let fixture = composite_fixture(45);
    let authority = exact_authority(&fixture).expect("exact fixture authority");
    let CompositeFixtureV1 {
        layout,
        source_receipt,
        envelope,
        transcript,
        ..
    } = fixture;
    let finish_materialized = Rc::new(Cell::new(false));
    let source_dropped = Rc::new(Cell::new(false));
    let source_packing_dropped = Rc::new(Cell::new(false));
    let qpcs_dropped = Rc::new(Cell::new(false));

    let result: Result<
        ZkAmsMkheRnsNativeCompositeCandidateReceiptV1,
        ZkAmsMkheRnsNativeCompositeVerificationErrorV1,
    > = retain_composite_authorities_through_result_v2(
        RetainedAuthorityDropProbe {
            name: "source owner",
            finish_materialized: Rc::clone(&finish_materialized),
            dropped: Rc::clone(&source_dropped),
        },
        RetainedAuthorityDropProbe {
            name: "source-packing authority",
            finish_materialized: Rc::clone(&finish_materialized),
            dropped: Rc::clone(&source_packing_dropped),
        },
        RetainedAuthorityDropProbe {
            name: "qPCS authority",
            finish_materialized: Rc::clone(&finish_materialized),
            dropped: Rc::clone(&qpcs_dropped),
        },
        |source_owner, source_packing_authority, qpcs_authority| {
            assert!(!source_owner.dropped.get());
            assert!(!source_packing_authority.dropped.get());
            assert!(!qpcs_authority.dropped.get());
            let receipt = ContextCheckedV1::new(
                &envelope,
                layout,
                source_receipt,
                transcript,
                FirstPartyStageAuthorityV1::ExactFixture(Box::new(authority)),
            )?
            .verify_terminal_bridge_v1()?
            .verify_rns_relation_qpcs_v1()?
            .verify_cross_field_global_lookup_v1()?
            .verify_zero_padding_v1()?
            .finish_v1()?;
            assert!(!source_owner.dropped.get());
            assert!(!source_packing_authority.dropped.get());
            assert!(!qpcs_authority.dropped.get());
            finish_materialized.set(true);
            Ok(receipt)
        },
    );

    let receipt = result.expect("actual finish_v1 materializes a candidate");
    assert_ne!(receipt.candidate_digest(), [0; 32]);
    assert!(finish_materialized.get());
    assert!(source_dropped.get());
    assert!(source_packing_dropped.get());
    assert!(qpcs_dropped.get());
}

#[test]
fn source_bound_all_stage_authority_is_pre_authenticated_narrow_and_consuming() {
    let source = include_str!("rns_native_composite_verifier.rs");
    let entry = source
        .split_once("verify_zk_ams_mkhe_rns_native_composite_from_source_chain_v2")
        .and_then(|(_, suffix)| {
            suffix.split_once("fn validate_then_authenticate_source_bound_context_v2")
        })
        .map(|(entry, _)| entry)
        .expect("source-bound composite entry");
    let retention = entry
        .find("retain_composite_authorities_through_result_v2(")
        .expect("retained-authority calculation");
    let context_orchestration = entry
        .find("validate_then_authenticate_source_bound_context_v2(")
        .expect("typed context orchestration");
    let terminal_pre_auth = entry
        .find("authenticate_terminal_candidate_v2")
        .expect("retained terminal preauthentication");
    let qpcs_pre_auth = entry
        .find("authenticate_qpcs_candidate_v2")
        .expect("retained qPCS preauthentication");
    let cursor = entry
        .find(".verify_terminal_bridge_v1()")
        .expect("all-stage cursor walk");
    let finish = entry
        .find(".finish_v1()")
        .expect("candidate materialization");
    assert!(retention < context_orchestration);
    assert!(context_orchestration < terminal_pre_auth);
    assert!(terminal_pre_auth < qpcs_pre_auth);
    assert!(qpcs_pre_auth < cursor);
    assert!(cursor < finish);

    let context_helper = source
        .split_once("fn validate_then_authenticate_source_bound_context_v2")
        .and_then(|(_, suffix)| {
            suffix.split_once("fn retain_composite_authorities_through_result_v2")
        })
        .map(|(context_helper, _)| context_helper)
        .expect("typed context orchestration helper");
    let context_validation = context_helper
        .find("let context_checked = ContextCheckedV1::new")
        .expect("typed context validation");
    let terminal_auth = context_helper
        .find("authenticate_terminal(&context_checked)")
        .expect("terminal authentication call");
    let qpcs_auth = context_helper
        .find("authenticate_qpcs(&context_checked)")
        .expect("qPCS authentication call");
    assert!(context_validation < terminal_auth);
    assert!(terminal_auth < qpcs_auth);
    assert!(context_helper.contains("FirstPartyStageAuthorityV1::ProductionSourceBoundAllStages"));

    let retention_helper = source
        .split_once("fn retain_composite_authorities_through_result_v2")
        .and_then(|(_, suffix)| suffix.split_once("fn verify_with_first_party_authority_v1"))
        .map(|(retention_helper, _)| retention_helper)
        .expect("retained-authority helper");
    let materialized = retention_helper
        .find("let result = calculation(")
        .expect("materialized composite result");
    for owner_drop in [
        "drop(source_owner);",
        "drop(source_packing_authority);",
        "drop(qpcs_authority);",
    ] {
        let owner_drop = retention_helper
            .find(owner_drop)
            .expect("retained authority drop");
        assert!(materialized < owner_drop);
    }
    let authority = source
        .split_once("Self::ProductionSourceBoundAllStages => match stage")
        .and_then(|(_, suffix)| suffix.split_once("#[cfg(test)]"))
        .map(|(authority, _)| authority)
        .expect("narrow source-bound authority");
    assert!(authority.contains("authenticate_source_bound_cross_field_global_lookup_v2"));
    assert!(authority.contains("authenticate_zero_padding_production_v1"));
    assert!(authority.contains("TerminalHyraxBpBridge"));
    assert!(authority.contains("RnsRelationQpcs"));
    assert_eq!(authority.matches("=> Ok(())").count(), 1);
    assert!(!authority.contains("verify_production_stage_v1"));
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

    let public_boundary = source
        .split_once("pub fn verify_zk_ams_mkhe_rns_native_composite_v1<S>")
        .and_then(|(_, suffix)| {
            suffix.split_once(
                "pub(super) fn verify_zk_ams_mkhe_rns_native_composite_from_source_chain_v2",
            )
        })
        .map(|(boundary, _)| boundary)
        .expect("generic public ownership boundary");
    assert!(public_boundary.contains("source_snapshot: S"));
    assert!(public_boundary.contains("S: ZkAmsMkheRnsNativeSourceSnapshotV1"));
    assert!(!public_boundary.contains("source_layout:"));
    assert!(!public_boundary.contains("source_receipt:"));

    let private_boundary = source
        .split_once("fn verify_with_first_party_authority_v1<S>")
        .and_then(|(_, suffix)| suffix.split_once("struct CandidateAxesV1"))
        .map(|(boundary, _)| boundary)
        .expect("private retained-owner boundary");
    let calculation = private_boundary
        .find("let result = (||")
        .expect("scoped atomic calculation");
    let owner_drop = private_boundary
        .find("drop(source_snapshot);")
        .expect("explicit source-owner drop");
    assert!(calculation < owner_drop);
    assert!(private_boundary.contains("let source_receipt = source_snapshot"));
    assert!(private_boundary.contains(".structural_receipt()"));
    assert!(!private_boundary.contains("preflight_rns_native_rlwe_source_statement_v1"));
}
