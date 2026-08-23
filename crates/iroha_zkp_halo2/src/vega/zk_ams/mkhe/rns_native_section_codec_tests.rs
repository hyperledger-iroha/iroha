use super::*;
use crate::vega::zk_ams::mkhe::{
    rns_native_profile::{
        ZkAmsMkheRnsNativeFamilyV1, zk_ams_mkhe_rns_native_profile_v1,
        zk_ams_mkhe_rns_native_release_candidate_digest_v1, zk_ams_mkhe_rns_native_topology_v1,
    },
    rns_native_source::{
        ZkAmsMkheRnsNativeSecretChunkV1, ZkAmsMkheRnsNativeSourceArenaV1,
        ZkAmsMkheRnsNativeSourceErrorV1, ZkAmsMkheRnsNativeSourceLayoutV1,
        ZkAmsMkheRnsNativeSourceReceiptV1, ZkAmsMkheRnsNativeSourceSnapshotV1,
    },
    rns_native_transcript::{
        ZkAmsMkheRnsNativeOpeningCommitmentV1, ZkAmsMkheRnsNativeOpeningCommitmentsV1,
        ZkAmsMkheRnsNativePublicContextV1, ZkAmsMkheRnsNativeQpcsFriRootV1,
        ZkAmsMkheRnsNativeQpcsRootsV1, ZkAmsMkheRnsNativeTerminalBridgeV1,
        ZkAmsMkheRnsNativeTerminalRootsV1, ZkAmsMkheRnsNativeTranscriptV1,
    },
};

fn digest(label: &[u8], context: u16, ordinal: u16) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.rns-native-section-codec.test");
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

fn indexed<const N: usize>(label: &[u8], context: u16) -> [[u8; 32]; N] {
    core::array::from_fn(|ordinal| {
        digest(
            label,
            context,
            u16::try_from(ordinal).expect("test ordinal fits u16"),
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
            ZkAmsMkheRnsNativeSourceArenaV1::Main => digest(b"main-snapshot", self.context, 0),
            ZkAmsMkheRnsNativeSourceArenaV1::Nonce => digest(b"nonce-snapshot", self.context, 0),
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
    family_from_ordinal_v1(ordinal).expect("opening ordinal is canonical")
}

fn transcript_source_fixture(
    context: u16,
) -> (
    ZkAmsMkheRnsNativeSourceLayoutV1,
    ZkAmsMkheRnsNativeSourceReceiptV1,
    ZkAmsMkheRnsNativeChallengeSeedsV1,
) {
    let profile = zk_ams_mkhe_rns_native_profile_v1().expect("profile");
    let topology = zk_ams_mkhe_rns_native_topology_v1().expect("topology");
    let release = zk_ams_mkhe_rns_native_release_candidate_digest_v1().expect("candidate");
    let layout = ZkAmsMkheRnsNativeSourceLayoutV1::new(
        profile.profile_digest,
        topology.topology_digest,
        release,
        digest(b"statement", context, 0),
        digest(b"operation", context, 0),
    )
    .expect("layout");
    let receipt = TestSnapshot { layout, context }
        .structural_receipt()
        .expect("receipt");
    let public = ZkAmsMkheRnsNativePublicContextV1::new(
        digest(b"roster", context, 0),
        digest(b"ciphertext", context, 0),
    )
    .expect("public context");
    let transcript =
        ZkAmsMkheRnsNativeTranscriptV1::new(layout, receipt, public).expect("initial transcript");
    let openings = core::array::from_fn(|ordinal| {
        let (family, index) = opening_role(ordinal);
        ZkAmsMkheRnsNativeOpeningCommitmentV1::new(
            family,
            index,
            digest(
                b"source-opening",
                context,
                u16::try_from(ordinal).expect("opening fits u16"),
            ),
            digest(
                b"hyrax-opening",
                context,
                u16::try_from(ordinal).expect("opening fits u16"),
            ),
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
        digest(b"mapping-root", context, 0),
        digest(b"hyrax-root", context, 0),
        digest(b"cross-basis-root", context, 0),
    )
    .expect("bridge");
    let transcript = transcript
        .bind_terminal_bridge(bridge)
        .expect("terminal transcript");
    let fri_roots = core::array::from_fn(|layer| {
        ZkAmsMkheRnsNativeQpcsFriRootV1::new(
            u8::try_from(layer).expect("FRI layer fits u8"),
            digest(
                b"fri-root",
                context,
                u16::try_from(layer).expect("FRI layer fits u16"),
            ),
        )
        .expect("FRI root")
    });
    let qpcs = ZkAmsMkheRnsNativeQpcsRootsV1::new(
        transcript.binding_digest(),
        digest(b"qpcs-initial", context, 0),
        digest(b"q-mask-s-root", context, 0),
        digest(b"qpcs-quotient", context, 0),
        fri_roots,
    )
    .expect("qPCS roots");
    let transcript = transcript.bind_qpcs_roots(qpcs).expect("qPCS transcript");
    let roots = ZkAmsMkheRnsNativeTerminalRootsV1::new(
        transcript.binding_digest(),
        digest(b"cross-field-root", context, 0),
        digest(b"lookup-root", context, 0),
        digest(b"padding-root", context, 0),
    )
    .expect("terminal roots");
    let seeds = transcript
        .bind_terminal_roots(roots)
        .expect("complete transcript");
    (layout, receipt, seeds)
}

fn transcript_fixture(context: u16) -> ZkAmsMkheRnsNativeChallengeSeedsV1 {
    transcript_source_fixture(context).2
}

struct CodecFixture {
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    receipt: ZkAmsMkheRnsNativeSourceReceiptV1,
    transcript: ZkAmsMkheRnsNativeChallengeSeedsV1,
    equations: [[u8; 32]; EQUATION_COUNT_V1],
    qpcs_limbs: [[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
    queries: [[u8; 32]; QUERY_COUNT_V1],
    points: [[u8; 32]; POINT_COUNT_V1],
    cross_limbs: [[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
    sumcheck: [[u8; 32]; SUMCHECK_COUNT_V1],
    padding_limbs: [[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
    terminal: Vec<u8>,
    rns_qpcs: Vec<u8>,
    cross_lookup: Vec<u8>,
    zero_padding: Vec<u8>,
}

fn codec_fixture(context: u16) -> CodecFixture {
    let (layout, receipt, transcript) = transcript_source_fixture(context);
    let equations = indexed(b"equation", context);
    let qpcs_limbs = indexed(b"qpcs-limb", context);
    let queries = indexed(b"query", context);
    let points = indexed(b"point", context);
    let cross_limbs = indexed(b"cross-limb", context);
    let sumcheck = indexed(b"sumcheck", context);
    let padding_limbs = indexed(b"padding-limb", context);
    let terminal = ZkAmsMkheRnsNativeTerminalBridgeSectionV1::new(&transcript, b"terminal-proof")
        .expect("terminal")
        .to_canonical_bytes_v1()
        .expect("terminal encoding");
    let rns_qpcs = ZkAmsMkheRnsNativeRnsRelationQpcsSectionV1::new(
        &transcript,
        &equations,
        &qpcs_limbs,
        &queries,
        b"rns-qpcs-proof",
    )
    .expect("RNS/qPCS")
    .to_canonical_bytes_v1()
    .expect("RNS/qPCS encoding");
    let cross_lookup = ZkAmsMkheRnsNativeCrossFieldGlobalLookupSectionV1::new(
        &transcript,
        &points,
        &cross_limbs,
        &sumcheck,
        b"cross-lookup-proof",
    )
    .expect("cross/lookup")
    .to_canonical_bytes_v1()
    .expect("cross/lookup encoding");
    let zero_padding =
        ZkAmsMkheRnsNativeZeroPaddingSectionV1::new(&transcript, &padding_limbs, b"padding-proof")
            .expect("padding")
            .to_canonical_bytes_v1()
            .expect("padding encoding");
    CodecFixture {
        layout,
        receipt,
        transcript,
        equations,
        qpcs_limbs,
        queries,
        points,
        cross_limbs,
        sumcheck,
        padding_limbs,
        terminal,
        rns_qpcs,
        cross_lookup,
        zero_padding,
    }
}

fn envelope_fixture(fixture: &CodecFixture) -> ZkAmsMkheRnsNativeProofEnvelopeV1 {
    ZkAmsMkheRnsNativeProofEnvelopeV1::new(
        fixture.layout,
        fixture.receipt,
        fixture.terminal.clone(),
        fixture.rns_qpcs.clone(),
        fixture.cross_lookup.clone(),
        fixture.zero_padding.clone(),
    )
    .expect("canonical envelope")
}

#[test]
fn all_four_codecs_roundtrip_with_exact_counts() {
    let fixture = codec_fixture(1);
    let terminal = ZkAmsMkheRnsNativeTerminalBridgeSectionV1::from_canonical_bytes_exact_v1(
        &fixture.terminal,
        &fixture.transcript,
    )
    .expect("terminal decode");
    assert_eq!(terminal.opening_commitments().len(), 43);
    assert_eq!(terminal.proof(), b"terminal-proof");

    let rns = ZkAmsMkheRnsNativeRnsRelationQpcsSectionV1::from_canonical_bytes_exact_v1(
        &fixture.rns_qpcs,
        &fixture.transcript,
    )
    .expect("RNS decode");
    assert_eq!(
        rns.equation_commitment_digests(),
        fixture.equations.as_slice()
    );
    assert_eq!(rns.limb_commitment_digests(), fixture.qpcs_limbs.as_slice());
    assert_eq!(rns.query_opening_digests(), fixture.queries.as_slice());

    let cross = ZkAmsMkheRnsNativeCrossFieldGlobalLookupSectionV1::from_canonical_bytes_exact_v1(
        &fixture.cross_lookup,
        &fixture.transcript,
    )
    .expect("cross decode");
    assert_eq!(cross.point_evaluation_digests(), fixture.points.as_slice());
    assert_eq!(
        cross.limb_relation_digests(),
        fixture.cross_limbs.as_slice()
    );
    assert_eq!(cross.sumcheck_round_digests(), fixture.sumcheck.as_slice());

    let padding = ZkAmsMkheRnsNativeZeroPaddingSectionV1::from_canonical_bytes_exact_v1(
        &fixture.zero_padding,
        &fixture.transcript,
    )
    .expect("padding decode");
    assert_eq!(
        padding.limb_padding_digests(),
        fixture.padding_limbs.as_slice()
    );
}

#[test]
fn envelope_cross_preflight_splits_and_binds_without_a_second_parse_or_hash() {
    let fixture = codec_fixture(31);
    let envelope = envelope_fixture(&fixture);
    let cross_bytes =
        envelope.section(ZkAmsMkheRnsNativeProofSectionKindV1::CrossFieldGlobalLookup);
    let before = cross_lookup_unbound_audit_counters_v1();
    let unbound = preflight_rns_native_cross_field_global_lookup_from_envelope_v1(&envelope)
        .expect("context-free envelope preflight");
    let after_parse = cross_lookup_unbound_audit_counters_v1();
    assert_eq!(after_parse.parse_passes - before.parse_passes, 1);
    assert_eq!(after_parse.proof_hash_passes - before.proof_hash_passes, 1);
    assert_eq!(after_parse.codec_hash_passes - before.codec_hash_passes, 1);
    assert_eq!(after_parse.final_context_binds, before.final_context_binds);

    let (pending, sealed) = unbound.split_pre_qpcs_v1();
    assert!(core::ptr::eq(sealed.section.as_ptr(), cross_bytes.as_ptr()));
    assert_eq!(sealed.section.len(), cross_bytes.len());
    assert!(core::ptr::eq(
        sealed.proof.as_ptr(),
        cross_bytes[CROSS_LOOKUP_PROOF_OFFSET_V1..].as_ptr()
    ));
    let bound = pending
        .bind_final_context_v1(&fixture.transcript)
        .expect("final transcript bind");
    assert!(core::ptr::eq(bound.section.as_ptr(), cross_bytes.as_ptr()));
    assert!(core::ptr::eq(
        bound.typed.proof().as_ptr(),
        sealed.proof.as_ptr()
    ));
    let after_bind = cross_lookup_unbound_audit_counters_v1();
    assert_eq!(after_bind.parse_passes, after_parse.parse_passes);
    assert_eq!(after_bind.proof_hash_passes, after_parse.proof_hash_passes);
    assert_eq!(after_bind.codec_hash_passes, after_parse.codec_hash_passes);
    assert_eq!(
        after_bind.final_context_binds - after_parse.final_context_binds,
        1
    );
}

#[test]
fn envelope_preflight_accepts_provisional_foreign_context_but_final_bind_rejects_it() {
    let fixture = codec_fixture(32);
    let foreign = codec_fixture(33);
    let envelope = ZkAmsMkheRnsNativeProofEnvelopeV1::new(
        fixture.layout,
        fixture.receipt,
        fixture.terminal,
        fixture.rns_qpcs,
        foreign.cross_lookup,
        fixture.zero_padding,
    )
    .expect("transport-valid mixed envelope");
    let unbound = preflight_rns_native_cross_field_global_lookup_from_envelope_v1(&envelope)
        .expect("foreign final context remains provisional");
    let (pending, _sealed) = unbound.split_pre_qpcs_v1();
    assert_eq!(
        pending
            .bind_final_context_v1(&fixture.transcript)
            .map(|_| ()),
        Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::ContextMismatch)
    );
}

#[test]
fn identity_challenge_root_and_proof_mutations_are_rejected() {
    let fixture = codec_fixture(2);
    let mut changed = fixture.terminal.clone();
    changed[10] ^= 1;
    assert!(matches!(
        ZkAmsMkheRnsNativeTerminalBridgeSectionV1::from_canonical_bytes_exact_v1(
            &changed,
            &fixture.transcript,
        ),
        Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::ContextMismatch)
    ));

    let mut changed = fixture.rns_qpcs.clone();
    changed[COMMON_PREFIX_BYTES_V1 + 5] ^= 1;
    assert!(matches!(
        ZkAmsMkheRnsNativeRnsRelationQpcsSectionV1::from_canonical_bytes_exact_v1(
            &changed,
            &fixture.transcript,
        ),
        Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::ContextMismatch)
    ));

    let mut changed = fixture.cross_lookup.clone();
    changed[COMMON_PREFIX_BYTES_V1 + 3 + 2 * 32] ^= 1;
    assert!(matches!(
        ZkAmsMkheRnsNativeCrossFieldGlobalLookupSectionV1::from_canonical_bytes_exact_v1(
            &changed,
            &fixture.transcript,
        ),
        Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::ContextMismatch)
    ));

    let mut changed = fixture.zero_padding.clone();
    let proof_byte = changed.len() - CODEC_DIGEST_BYTES_V1 - b"padding-proof".len();
    changed[proof_byte] ^= 1;
    assert!(matches!(
        ZkAmsMkheRnsNativeZeroPaddingSectionV1::from_canonical_bytes_exact_v1(
            &changed,
            &fixture.transcript,
        ),
        Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::Integrity)
    ));
}

#[test]
fn truncation_trailing_and_cross_transcript_splices_are_rejected() {
    let fixture = codec_fixture(3);
    let other = transcript_fixture(4);
    let mut truncated = fixture.terminal.clone();
    truncated.pop();
    assert!(
        ZkAmsMkheRnsNativeTerminalBridgeSectionV1::from_canonical_bytes_exact_v1(
            &truncated,
            &fixture.transcript,
        )
        .is_err()
    );

    let mut trailing = fixture.rns_qpcs.clone();
    trailing.push(0);
    assert!(matches!(
        ZkAmsMkheRnsNativeRnsRelationQpcsSectionV1::from_canonical_bytes_exact_v1(
            &trailing,
            &fixture.transcript,
        ),
        Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidEncoding)
    ));

    assert!(matches!(
        ZkAmsMkheRnsNativeCrossFieldGlobalLookupSectionV1::from_canonical_bytes_exact_v1(
            &fixture.cross_lookup,
            &other,
        ),
        Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::ContextMismatch)
    ));
}

#[test]
fn explicit_opening_query_and_sumcheck_reordering_is_rejected() {
    let fixture = codec_fixture(5);
    let mut terminal = fixture.terminal.clone();
    let openings = COMMON_PREFIX_BYTES_V1 + 1 + 5 * 32;
    terminal[openings + 2] = 1;
    assert!(matches!(
        ZkAmsMkheRnsNativeTerminalBridgeSectionV1::from_canonical_bytes_exact_v1(
            &terminal,
            &fixture.transcript,
        ),
        Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidOrder)
    ));

    let mut rns = fixture.rns_qpcs.clone();
    let queries = COMMON_PREFIX_BYTES_V1
        + 5
        + 4 * 32
        + FRI_COUNT_V1 * (1 + 32)
        + 2 * 32
        + FRI_COUNT_V1 * (1 + 32)
        + EQUATION_COUNT_V1 * (1 + 32)
        + ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 * (1 + 32);
    rns[queries..queries + 2].copy_from_slice(&1_u16.to_be_bytes());
    assert!(matches!(
        ZkAmsMkheRnsNativeRnsRelationQpcsSectionV1::from_canonical_bytes_exact_v1(
            &rns,
            &fixture.transcript,
        ),
        Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidOrder)
    ));

    let mut cross = fixture.cross_lookup.clone();
    let sumcheck = COMMON_PREFIX_BYTES_V1
        + 3
        + 4 * 32
        + POINT_COUNT_V1 * (1 + 32)
        + ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 * (1 + 32);
    cross[sumcheck] = 1;
    assert!(matches!(
        ZkAmsMkheRnsNativeCrossFieldGlobalLookupSectionV1::from_canonical_bytes_exact_v1(
            &cross,
            &fixture.transcript,
        ),
        Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidOrder)
    ));
}

#[test]
fn every_outer_cap_is_checked_before_parsing() {
    let transcript = transcript_fixture(6);
    let cases = [
        ZkAmsMkheRnsNativeProofSectionKindV1::TerminalHyraxBpBridge,
        ZkAmsMkheRnsNativeProofSectionKindV1::RnsRelationQpcs,
        ZkAmsMkheRnsNativeProofSectionKindV1::CrossFieldGlobalLookup,
        ZkAmsMkheRnsNativeProofSectionKindV1::ZeroPadding,
    ];
    for kind in cases {
        let bytes = vec![0_u8; usize::try_from(kind.max_bytes()).expect("cap fits usize") + 1];
        let error = match kind {
            ZkAmsMkheRnsNativeProofSectionKindV1::TerminalHyraxBpBridge => {
                ZkAmsMkheRnsNativeTerminalBridgeSectionV1::from_canonical_bytes_exact_v1(
                    &bytes,
                    &transcript,
                )
                .expect_err("terminal cap")
            }
            ZkAmsMkheRnsNativeProofSectionKindV1::RnsRelationQpcs => {
                ZkAmsMkheRnsNativeRnsRelationQpcsSectionV1::from_canonical_bytes_exact_v1(
                    &bytes,
                    &transcript,
                )
                .expect_err("RNS cap")
            }
            ZkAmsMkheRnsNativeProofSectionKindV1::CrossFieldGlobalLookup => {
                ZkAmsMkheRnsNativeCrossFieldGlobalLookupSectionV1::from_canonical_bytes_exact_v1(
                    &bytes,
                    &transcript,
                )
                .expect_err("cross cap")
            }
            ZkAmsMkheRnsNativeProofSectionKindV1::ZeroPadding => {
                ZkAmsMkheRnsNativeZeroPaddingSectionV1::from_canonical_bytes_exact_v1(
                    &bytes,
                    &transcript,
                )
                .expect_err("padding cap")
            }
        };
        assert_eq!(
            error,
            ZkAmsMkheRnsNativeSectionCodecErrorV1::ResourceCeilingExceeded
        );
    }
}

#[test]
fn forged_length_count_and_digest_aliases_are_rejected() {
    let fixture = codec_fixture(7);
    let mut forged = fixture.rns_qpcs.clone();
    let proof_length = RNS_QPCS_FIXED_BYTES_V1 - PROOF_FRAME_BYTES_V1 - CODEC_DIGEST_BYTES_V1;
    forged[proof_length..proof_length + 4].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(
        ZkAmsMkheRnsNativeRnsRelationQpcsSectionV1::from_canonical_bytes_exact_v1(
            &forged,
            &fixture.transcript,
        )
        .is_err()
    );

    assert!(matches!(
        ZkAmsMkheRnsNativeRnsRelationQpcsSectionV1::new(
            &fixture.transcript,
            &fixture.equations[..1],
            &fixture.qpcs_limbs,
            &fixture.queries,
            b"proof",
        ),
        Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidCount)
    ));

    let mut aliased_queries = fixture.queries;
    aliased_queries[0] = fixture.equations[0];
    assert!(matches!(
        ZkAmsMkheRnsNativeRnsRelationQpcsSectionV1::new(
            &fixture.transcript,
            &fixture.equations,
            &fixture.qpcs_limbs,
            &aliased_queries,
            b"proof",
        ),
        Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::AliasedDigest)
    ));

    let mut cross_section_alias = fixture.padding_limbs;
    cross_section_alias[0] = fixture.equations[0];
    let aliased_padding = ZkAmsMkheRnsNativeZeroPaddingSectionV1::new(
        &fixture.transcript,
        &cross_section_alias,
        b"different-padding-proof",
    )
    .expect("alias is outside the standalone padding section")
    .to_canonical_bytes_v1()
    .expect("standalone padding encoding");
    let outer_section_digests: [[u8; 32]; 4] = indexed(b"outer-section", 7);
    let outer_proof_digest = digest(b"outer-proof", 7, 0);
    assert_eq!(
        validate_composite_section_set_exact_v1(
            &fixture.terminal,
            &fixture.rns_qpcs,
            &fixture.cross_lookup,
            &aliased_padding,
            &fixture.transcript,
            &outer_section_digests,
            outer_proof_digest,
        ),
        Err(CompositeSectionSetErrorV1::CrossSectionAlias)
    );
}

#[test]
fn sealed_envelope_surface_is_move_only_private_and_fail_closed() {
    let source = include_str!("rns_native_section_codec.rs");
    for declaration in [
        "pub(super) struct RnsNativeUnboundCrossFieldGlobalLookupEnvelopeV1",
        "pub(super) struct RnsNativePendingCrossFieldGlobalLookupContextV1",
        "pub(super) struct RnsNativeSealedCrossProofLeaseV1",
        "pub(super) struct RnsNativeSealedCrossProofInventoryPermitV1",
        "pub(super) struct RnsNativeBoundCrossFieldGlobalLookupV1",
    ] {
        let offset = source.find(declaration).expect("sealed type declaration");
        let attributes = source[..offset]
            .rsplit_once("\n\n")
            .map_or(&source[..offset], |(_, block)| block);
        assert!(!attributes.contains("derive(Clone"));
        assert!(!attributes.contains("derive(Copy"));
    }
    let sealed_impl = source
        .split_once("impl<'env> RnsNativeSealedCrossProofLeaseV1")
        .expect("sealed proof implementation")
        .1
        .split_once("/// Parse and hash the exact envelope-owned cross section once")
        .expect("sealed proof implementation boundary")
        .0;
    assert_eq!(sealed_impl.matches("pub(super) fn ").count(), 1);
    assert!(sealed_impl.contains("preflight_q_mask_inventory_v1"));
    for forbidden in [
        "fn proof(",
        "fn bytes(",
        "fn section(",
        "fn digest(",
        "fn root(",
        "fn into_parts",
        "AsRef",
        "Deref",
    ] {
        assert!(
            !sealed_impl.contains(forbidden),
            "sealed proof escape present: {forbidden}"
        );
    }
    assert!(source.contains("CROSS_LOOKUP_FIXED_BYTES_V1 == 2_811"));
    assert!(source.contains("CROSS_LOOKUP_PROOF_OFFSET_V1 == 2_779"));
    assert!(source.contains("CROSS_LOOKUP_PROOF_MAX_BYTES_V1 == 8_385_797"));
    assert!(source.contains("CROSS_LOOKUP_UNBOUND_HASH_ABSORPTION_MAX_BYTES_V1 == 16_774_480"));
    assert!(source.contains("CROSS_LOOKUP_DIGEST_REGISTRY_STACK_BYTES_V1 == 16_384"));
    assert!(source.contains("PRE_QPCS_CROSS_ENVELOPE_SOURCE_IMPLEMENTED_V1: bool = true"));
    for false_gate in [
        "PRE_QPCS_CROSS_ENVELOPE_LIVE_INTEGRATED_V1: bool = false",
        "PRE_QPCS_CROSS_ENVELOPE_RESOURCE_QUALIFIED_V1: bool = false",
        "PRE_QPCS_CROSS_ENVELOPE_RELEASE_READY_V1: bool = false",
    ] {
        assert!(source.contains(false_gate));
    }
    let identity = source
        .split_once("pub(super) fn authenticate_bound_cross_field_global_lookup_for_inventory_v1")
        .expect("sealed identity consumer")
        .1
        .split_once("/// Borrowed, typed forty-limb zero-padding proof section")
        .expect("sealed identity consumer boundary")
        .0;
    for required in [
        "core::ptr::eq(bound.envelope, seal.envelope)",
        "core::ptr::eq(bound.section.as_ptr(), seal.section.as_ptr())",
        "bound.section.len() == seal.section.len()",
        "bound.section == seal.section",
        "core::ptr::eq(exact_proof.as_ptr(), seal.proof.as_ptr())",
        "bound.codec_digest == exact_codec_digest",
    ] {
        assert!(
            identity.contains(required),
            "missing identity guard: {required}"
        );
    }
    for forbidden_repeat in [
        "decode_unbound_cross_field_global_lookup_v1",
        "proof_body_digest_v1",
        "codec_digest_v1",
        "DecoderV1",
    ] {
        assert!(
            !identity.contains(forbidden_repeat),
            "identity consumer repeats parsing or hashing: {forbidden_repeat}"
        );
    }
}
