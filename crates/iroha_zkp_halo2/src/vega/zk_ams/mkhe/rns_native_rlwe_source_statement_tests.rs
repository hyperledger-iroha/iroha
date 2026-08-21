use super::*;
use crate::vega::zk_ams::mkhe::{
    rns_native_source::ZkAmsMkheRnsNativeSourceErrorV1,
    rns_native_transcript::{
        ZkAmsMkheRnsNativeOpeningCommitmentV1, ZkAmsMkheRnsNativeOpeningCommitmentsV1,
        ZkAmsMkheRnsNativePublicContextV1, ZkAmsMkheRnsNativeQpcsFriRootV1,
        ZkAmsMkheRnsNativeQpcsRootsV1, ZkAmsMkheRnsNativeTerminalBridgeV1,
        ZkAmsMkheRnsNativeTerminalRootsV1, ZkAmsMkheRnsNativeTranscriptV1,
    },
};

fn digest(label: &[u8], context: u16, ordinal: u32) -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.rns-native-rlwe-source.test");
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

fn nonce_bytes(ordinal: usize) -> Vec<u8> {
    let mut nonce = vec![0_u8; ZK_AMS_MKHE_RNS_NATIVE_SOURCE_NONCE_PLAINTEXT_BYTES_V1 as usize];
    let last = nonce.len() - 1;
    nonce[0] = 1;
    nonce[last] = u8::try_from(ordinal + 1).expect("opening ordinal fits u8");
    nonce
}

#[derive(Clone, Copy)]
enum SnapshotFault {
    None,
    ReadFailure,
    WrongArena,
    WrongLength,
    NonCanonicalPlaintext,
    ZeroEphemeral,
    OutOfRangeEphemeral,
    OutOfRangeError,
    ZeroNonce,
}

struct TestChunk {
    arena: ZkAmsMkheRnsNativeSourceArenaV1,
    bytes: Vec<u8>,
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
    next_record: usize,
    next_main_block: usize,
    fault: SnapshotFault,
}

impl TestSnapshot {
    const fn new(
        layout: ZkAmsMkheRnsNativeSourceLayoutV1,
        context: u16,
        fault: SnapshotFault,
    ) -> Self {
        Self {
            layout,
            context,
            next_record: 0,
            next_main_block: 0,
            fault,
        }
    }
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
        arena: ZkAmsMkheRnsNativeSourceArenaV1,
        slot: u64,
    ) -> Result<Self::Chunk, ZkAmsMkheRnsNativeSourceErrorV1> {
        if matches!(self.fault, SnapshotFault::ReadFailure)
            && self.next_record == 0
            && self.next_main_block == 0
        {
            return Err(ZkAmsMkheRnsNativeSourceErrorV1::Storage);
        }
        let expected_main_slot = self
            .next_record
            .checked_mul(MAIN_BLOCKS_PER_RECORD_V1)
            .and_then(|base| base.checked_add(self.next_main_block))
            .and_then(|value| u64::try_from(value).ok());
        if self.next_record >= OPENING_COUNT_V1 {
            return Err(ZkAmsMkheRnsNativeSourceErrorV1::UnexpectedWrite);
        }
        let mut bytes;
        if self.next_main_block < MAIN_BLOCKS_PER_RECORD_V1 {
            if arena != ZkAmsMkheRnsNativeSourceArenaV1::Main || Some(slot) != expected_main_slot {
                return Err(ZkAmsMkheRnsNativeSourceErrorV1::UnexpectedWrite);
            }
            bytes = vec![0_u8; ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_PLAINTEXT_BYTES_V1 as usize];
            if self.next_main_block >= SourceComponentV1::Ephemeral.first_block()
                && self.next_main_block < SourceComponentV1::ErrorZero.first_block()
                && !matches!(self.fault, SnapshotFault::ZeroEphemeral)
            {
                bytes[SIGNED_COEFFICIENT_BYTES_V1 - 1] = 1;
            }
            if matches!(self.fault, SnapshotFault::NonCanonicalPlaintext)
                && self.next_record == 0
                && self.next_main_block == 0
            {
                bytes[..CANONICAL_COEFFICIENT_BYTES_V1]
                    .copy_from_slice(&VEGA_T256_SCALAR_MODULUS_BE_V1);
            }
            if matches!(self.fault, SnapshotFault::OutOfRangeEphemeral)
                && self.next_record == 0
                && self.next_main_block == SourceComponentV1::Ephemeral.first_block()
            {
                bytes[..SIGNED_COEFFICIENT_BYTES_V1].copy_from_slice(&2_i64.to_be_bytes());
            }
            if matches!(self.fault, SnapshotFault::OutOfRangeError)
                && self.next_record == 0
                && self.next_main_block == SourceComponentV1::ErrorZero.first_block()
            {
                bytes[..SIGNED_COEFFICIENT_BYTES_V1].copy_from_slice(&3_i64.to_be_bytes());
            }
            self.next_main_block += 1;
        } else {
            if arena != ZkAmsMkheRnsNativeSourceArenaV1::Nonce
                || slot != u64::try_from(self.next_record).expect("record ordinal fits u64")
            {
                return Err(ZkAmsMkheRnsNativeSourceErrorV1::UnexpectedWrite);
            }
            bytes = nonce_bytes(self.next_record);
            if matches!(self.fault, SnapshotFault::ZeroNonce) && self.next_record == 0 {
                bytes.fill(0);
            }
            self.next_record += 1;
            self.next_main_block = 0;
        }
        let mut returned_arena = arena;
        if matches!(self.fault, SnapshotFault::WrongArena)
            && self.next_record == 0
            && self.next_main_block == 1
        {
            returned_arena = ZkAmsMkheRnsNativeSourceArenaV1::Nonce;
        }
        if matches!(self.fault, SnapshotFault::WrongLength)
            && self.next_record == 0
            && self.next_main_block == 1
        {
            bytes.pop();
        }
        Ok(TestChunk {
            arena: returned_arena,
            bytes,
        })
    }
}

struct Fixture {
    context: u16,
    epoch: u64,
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    receipt: ZkAmsMkheRnsNativeSourceReceiptV1,
    transcript: ZkAmsMkheRnsNativeChallengeSeedsV1,
    roster_digest: [u8; DIGEST_BYTES_V1],
    public_a: Vec<[u8; DIGEST_BYTES_V1]>,
    public_b: Vec<[u8; DIGEST_BYTES_V1]>,
    ciphertext_c0: Vec<[u8; DIGEST_BYTES_V1]>,
    ciphertext_c1: Vec<[u8; DIGEST_BYTES_V1]>,
    records: Vec<RnsNativePublicRecordMetadataV1>,
    public_bundle_digest: [u8; DIGEST_BYTES_V1],
    equation_commitments: [[u8; DIGEST_BYTES_V1]; EQUATION_COUNT_V1],
    limb_commitments: [[u8; DIGEST_BYTES_V1]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
    evaluations: [u8; QPCS_EVALUATION_BYTES_V1],
    qpcs_parameter_digest: [u8; DIGEST_BYTES_V1],
    qpcs_section_digest: [u8; DIGEST_BYTES_V1],
    qpcs_schedule_digest: [u8; DIGEST_BYTES_V1],
    qpcs_evaluation_binding_digest: [u8; DIGEST_BYTES_V1],
    qpcs_residual_digest: [u8; DIGEST_BYTES_V1],
}

impl Fixture {
    fn new(context: u16) -> Self {
        let manifest = zk_ams_mkhe_rns_native_profile_manifest_v1().expect("canonical manifest");
        let topology = zk_ams_mkhe_rns_native_topology_v1().expect("canonical topology");
        let release =
            zk_ams_mkhe_rns_native_release_candidate_digest_v1().expect("candidate identity");
        let layout = ZkAmsMkheRnsNativeSourceLayoutV1::new(
            manifest.profile_digest,
            topology.topology_digest,
            release,
            digest(b"statement", context, 0),
            digest(b"operational", context, 0),
        )
        .expect("canonical layout");
        let receipt = TestSnapshot::new(layout, context, SnapshotFault::None)
            .structural_receipt()
            .expect("structural receipt");
        let epoch = 9_u64 + u64::from(context);
        let roster_digest = digest(b"roster", context, 0);
        let public_a = (0..ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1)
            .map(|limb| {
                digest(
                    b"public-a",
                    context,
                    u32::try_from(limb).expect("limb fits"),
                )
            })
            .collect::<Vec<_>>();
        let public_b = (0..ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1)
            .map(|limb| {
                digest(
                    b"public-b",
                    context,
                    u32::try_from(limb).expect("limb fits"),
                )
            })
            .collect::<Vec<_>>();
        let ciphertext_c0 = (0..PUBLIC_LIMB_DIGEST_COUNT_V1)
            .map(|coordinate| {
                digest(
                    b"ciphertext-c0",
                    context,
                    u32::try_from(coordinate).expect("coordinate fits"),
                )
            })
            .collect::<Vec<_>>();
        let ciphertext_c1 = (0..PUBLIC_LIMB_DIGEST_COUNT_V1)
            .map(|coordinate| {
                digest(
                    b"ciphertext-c1",
                    context,
                    u32::try_from(coordinate).expect("coordinate fits"),
                )
            })
            .collect::<Vec<_>>();
        let public_key_digest =
            public_key_digest_v1(layout, epoch, roster_digest, &public_a, &public_b)
                .expect("public key identity");
        let records = (0..OPENING_COUNT_V1)
            .map(|ordinal| {
                let position = record_position_v1(ordinal).expect("record position");
                let nonce_binding_digest = nonce_binding_digest_v1(
                    layout,
                    epoch,
                    roster_digest,
                    public_key_digest,
                    position,
                    u64::from(position.ordinal),
                    &nonce_bytes(ordinal),
                )
                .expect("nonce binding");
                let start = ordinal * ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1;
                let end = start + ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1;
                let record_digest = public_record_digest_v1(
                    public_key_digest,
                    position,
                    u64::from(position.ordinal),
                    nonce_binding_digest,
                    &ciphertext_c0[start..end],
                    &ciphertext_c1[start..end],
                )
                .expect("record identity");
                RnsNativePublicRecordMetadataV1::new(
                    position.ordinal,
                    position.family,
                    position.family_index,
                    u64::from(position.ordinal),
                    nonce_binding_digest,
                    record_digest,
                )
            })
            .collect::<Vec<_>>();
        let public_bundle_digest =
            public_bundle_digest_v1(layout, epoch, roster_digest, public_key_digest, &records)
                .expect("public bundle");
        let public_context =
            ZkAmsMkheRnsNativePublicContextV1::new(roster_digest, public_bundle_digest)
                .expect("public transcript context");
        let transcript = ZkAmsMkheRnsNativeTranscriptV1::new(layout, receipt, public_context)
            .expect("context transcript");
        let opening_records = core::array::from_fn(|ordinal| {
            let position = record_position_v1(ordinal).expect("opening position");
            ZkAmsMkheRnsNativeOpeningCommitmentV1::new(
                position.family,
                position.family_index,
                digest(
                    b"source-commitment",
                    context,
                    u32::try_from(ordinal).expect("opening ordinal fits"),
                ),
                digest(
                    b"hyrax-commitment",
                    context,
                    u32::try_from(ordinal).expect("opening ordinal fits"),
                ),
            )
            .expect("typed opening")
        });
        let openings = ZkAmsMkheRnsNativeOpeningCommitmentsV1::new(
            transcript.binding_digest(),
            opening_records,
        )
        .expect("opening bundle");
        let transcript = transcript
            .bind_opening_commitments(openings)
            .expect("opening transcript");
        let bridge = ZkAmsMkheRnsNativeTerminalBridgeV1::new(
            transcript.binding_digest(),
            digest(b"mapping-root", context, 0),
            digest(b"terminal-root", context, 0),
            digest(b"cross-basis-root", context, 0),
        )
        .expect("terminal bridge");
        let transcript = transcript
            .bind_terminal_bridge(bridge)
            .expect("terminal transcript");
        let fri_roots = core::array::from_fn(|layer| {
            ZkAmsMkheRnsNativeQpcsFriRootV1::new(
                u8::try_from(layer).expect("layer fits u8"),
                digest(
                    b"fri-root",
                    context,
                    u32::try_from(layer).expect("layer fits u32"),
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
            .expect("complete transcript");
        let equation_commitments = core::array::from_fn(|ordinal| {
            digest(
                b"equation-commitment",
                context,
                u32::try_from(ordinal).expect("equation ordinal fits"),
            )
        });
        let limb_commitments = core::array::from_fn(|limb| {
            digest(
                b"limb-commitment",
                context,
                u32::try_from(limb).expect("limb ordinal fits"),
            )
        });
        Self {
            context,
            epoch,
            layout,
            receipt,
            transcript,
            roster_digest,
            public_a,
            public_b,
            ciphertext_c0,
            ciphertext_c1,
            records,
            public_bundle_digest,
            equation_commitments,
            limb_commitments,
            evaluations: [0; QPCS_EVALUATION_BYTES_V1],
            qpcs_parameter_digest: digest(b"qpcs-parameter", context, 0),
            qpcs_section_digest: digest(b"qpcs-section", context, 0),
            qpcs_schedule_digest: digest(b"qpcs-schedule", context, 0),
            qpcs_evaluation_binding_digest: digest(b"qpcs-evaluation-binding", context, 0),
            qpcs_residual_digest: digest(b"qpcs-residual", context, 0),
        }
    }

    fn public_view(&self) -> RnsNativePublicArtifactViewV1<'_> {
        RnsNativePublicArtifactViewV1::new(
            self.epoch,
            self.roster_digest,
            &self.public_a,
            &self.public_b,
            &self.ciphertext_c0,
            &self.ciphertext_c1,
            &self.records,
            self.public_bundle_digest,
        )
    }

    fn qpcs<'a>(&'a self, residual: &'a [u8]) -> QpcsBindingsV1<'a> {
        QpcsBindingsV1 {
            parameter_digest: self.qpcs_parameter_digest,
            transcript_digest: self.transcript.transcript_digest(),
            query_seed: self.transcript.qpcs_query_challenge_seed(),
            section_binding_digest: self.qpcs_section_digest,
            fri_schedule_digest: self.qpcs_schedule_digest,
            evaluations: &self.evaluations,
            evaluation_binding_digest: self.qpcs_evaluation_binding_digest,
            residual_digest: self.qpcs_residual_digest,
            residual,
        }
    }

    fn anchor(&self, downstream: &[u8]) -> Vec<u8> {
        let placeholder = [0xa5_u8];
        let qpcs = self.qpcs(&placeholder);
        let derived = derive_statement_v1(
            &self.transcript,
            self.layout,
            self.receipt,
            self.public_view(),
            &self.equation_commitments,
            &self.limb_commitments,
            qpcs,
        )
        .expect("valid statement inputs");
        let core = expected_anchor_core_v1(
            &self.transcript,
            self.layout,
            self.receipt,
            qpcs,
            derived,
            downstream,
        )
        .expect("anchor core");
        ResidualAnchorV1::from_parts_v1(self.epoch, core, downstream)
            .expect("valid residual anchor")
            .to_canonical_bytes_v1()
            .expect("canonical residual anchor")
    }
}

#[test]
fn residual_anchor_is_exact_capped_and_digest_bound() {
    let downstream = [0x5a_u8; 17];
    let mut core = core::array::from_fn(|index| {
        digest(
            b"anchor-core",
            1,
            u32::try_from(index).expect("core ordinal fits"),
        )
    });
    core[CORE_DOWNSTREAM_V1] = downstream_digest_v1(&downstream).expect("downstream digest");
    let anchor = ResidualAnchorV1::from_parts_v1(7, core, &downstream).expect("anchor");
    let encoded = anchor.to_canonical_bytes_v1().expect("encode");
    assert_eq!(encoded.len(), ANCHOR_FIXED_BYTES_V1 + downstream.len());
    assert_eq!(
        ResidualAnchorV1::from_canonical_bytes_exact_v1(&encoded).expect("decode"),
        anchor
    );

    for length in 0..encoded.len() {
        assert_eq!(
            ResidualAnchorV1::from_canonical_bytes_exact_v1(&encoded[..length]),
            Err(RnsNativeRlweSourceStatementErrorV1::InvalidAnchor)
        );
    }
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(
        ResidualAnchorV1::from_canonical_bytes_exact_v1(&trailing),
        Err(RnsNativeRlweSourceStatementErrorV1::InvalidAnchor)
    );
    let mut bad_length = encoded.clone();
    bad_length[30..34].copy_from_slice(
        &u32::try_from(downstream.len() + 1)
            .expect("test length fits u32")
            .to_be_bytes(),
    );
    assert_eq!(
        ResidualAnchorV1::from_canonical_bytes_exact_v1(&bad_length),
        Err(RnsNativeRlweSourceStatementErrorV1::InvalidAnchor)
    );
    for offset in [0, 4, 5, 6, 7, 8, 9, 10, 11, 12, 16, 18, 20] {
        let mut mutated = encoded.clone();
        mutated[offset] ^= 1;
        assert_eq!(
            ResidualAnchorV1::from_canonical_bytes_exact_v1(&mutated),
            Err(RnsNativeRlweSourceStatementErrorV1::InvalidAnchor),
            "header offset {offset}"
        );
    }
    let mut zero_epoch = encoded.clone();
    zero_epoch[22..30].fill(0);
    assert_eq!(
        ResidualAnchorV1::from_canonical_bytes_exact_v1(&zero_epoch),
        Err(RnsNativeRlweSourceStatementErrorV1::InvalidAnchor)
    );
    let mut mutated = encoded.clone();
    *mutated.last_mut().expect("downstream byte") ^= 1;
    assert_eq!(
        ResidualAnchorV1::from_canonical_bytes_exact_v1(&mutated),
        Err(RnsNativeRlweSourceStatementErrorV1::InvalidAnchor)
    );
    let oversized = vec![0_u8; RNS_NATIVE_RLWE_SOURCE_RESIDUAL_MAX_BYTES_V1 + 1];
    assert_eq!(
        ResidualAnchorV1::from_canonical_bytes_exact_v1(&oversized),
        Err(RnsNativeRlweSourceStatementErrorV1::AnchorCapExceeded)
    );
    assert_eq!(
        ResidualAnchorV1::from_parts_v1(7, core, &[]),
        Err(RnsNativeRlweSourceStatementErrorV1::InvalidAnchor)
    );
    let mut aliased = core;
    aliased[1] = aliased[0];
    assert_eq!(
        ResidualAnchorV1::from_parts_v1(7, aliased, &downstream),
        Err(RnsNativeRlweSourceStatementErrorV1::InvalidAnchor)
    );

    let maximum = vec![0x33; RNS_NATIVE_RLWE_SOURCE_DOWNSTREAM_MAX_BYTES_V1];
    core[CORE_DOWNSTREAM_V1] = downstream_digest_v1(&maximum).expect("maximum digest");
    let maximum = ResidualAnchorV1::from_parts_v1(7, core, &maximum)
        .expect("maximum anchor")
        .to_canonical_bytes_v1()
        .expect("maximum encoding");
    assert_eq!(maximum.len(), RNS_NATIVE_RLWE_SOURCE_RESIDUAL_MAX_BYTES_V1);
    ResidualAnchorV1::from_canonical_bytes_exact_v1(&maximum).expect("maximum decode");
}

#[test]
fn source_mapping_and_formula_are_frozen_and_cover_all_coordinates() {
    assert_eq!(
        record_position_v1(0).expect("X").family,
        ZkAmsMkheRnsNativeFamilyV1::X
    );
    assert_eq!(record_position_v1(16).expect("last U").family_index, 15);
    assert_eq!(record_position_v1(17).expect("first E").family_index, 0);
    assert_eq!(record_position_v1(33).expect("rE").used_slots, 1_024);
    assert_eq!(record_position_v1(42).expect("rW").used_slots, 512);
    assert_eq!(record_position_v1(43), None);
    assert_eq!(
        absolute_main_slot_v1(42, SourceComponentV1::ErrorOne, 127).expect("last main slot"),
        38_527
    );
    assert_eq!(
        u64::from(record_position_v1(42).expect("last nonce").ordinal),
        42
    );
    let formula = rlwe_formula_digest_v1().expect("formula identity");
    let mapping = record_mapping_digest_v1().expect("mapping identity");
    assert_eq!(
        formula,
        [
            0xb4, 0x7d, 0x29, 0xcb, 0xa4, 0x99, 0x6f, 0x24, 0x88, 0x01, 0x80, 0x85, 0xd8, 0xd6,
            0xee, 0x52, 0xbf, 0x43, 0xd9, 0x0c, 0x03, 0xf7, 0x9c, 0xe6, 0xb9, 0x4b, 0xf9, 0x02,
            0xdb, 0x03, 0x7a, 0x0b,
        ]
    );
    assert_eq!(
        mapping,
        [
            0x19, 0x4f, 0x70, 0x46, 0x7d, 0xc4, 0x63, 0x08, 0xdf, 0xa0, 0xea, 0x75, 0xb8, 0x2b,
            0x51, 0xa1, 0x88, 0x2f, 0xa4, 0x9e, 0x9c, 0xa1, 0xc3, 0x92, 0x09, 0xfb, 0xfa, 0x36,
            0xc6, 0x35, 0x54, 0xbe,
        ]
    );
    assert_ne!(formula, mapping);
    assert_eq!(formula, rlwe_formula_digest_v1().expect("stable formula"));
    assert_eq!(mapping, record_mapping_digest_v1().expect("stable mapping"));
    assert_eq!(RLWE_EQUATIONS_V1[0].key_role(), b'B');
    assert_eq!(RLWE_EQUATIONS_V1[0].plaintext_delta(), 1);
    assert_eq!(RLWE_EQUATIONS_V1[1].key_role(), b'A');
    assert_eq!(RLWE_EQUATIONS_V1[1].plaintext_delta(), 0);
}

#[test]
fn aggregation_challenges_are_unbiased_distinct_and_axis_separated() {
    let fixture = Fixture::new(11);
    let formula = rlwe_formula_digest_v1().expect("formula");
    let mapping = record_mapping_digest_v1().expect("mapping");
    let first = derive_aggregation_challenges_v1(
        &fixture.transcript,
        fixture.qpcs_parameter_digest,
        formula,
        mapping,
    )
    .expect("challenge schedule");
    let second = derive_aggregation_challenges_v1(
        &fixture.transcript,
        fixture.qpcs_parameter_digest,
        formula,
        mapping,
    )
    .expect("same challenge schedule");
    assert_eq!(first, second);
    let other_context = Fixture::new(12);
    let context_separated = derive_aggregation_challenges_v1(
        &other_context.transcript,
        other_context.qpcs_parameter_digest,
        formula,
        mapping,
    )
    .expect("context-separated challenge schedule");
    let parameter_separated = derive_aggregation_challenges_v1(
        &fixture.transcript,
        digest(b"changed-qpcs-parameter", fixture.context, 0),
        formula,
        mapping,
    )
    .expect("parameter-separated challenge schedule");
    assert_ne!(first, context_separated);
    assert_ne!(first, parameter_separated);
    let mut pairs = Vec::new();
    for (limb, repetitions) in first.iter().enumerate() {
        let modulus = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[limb];
        let mut coordinates = Vec::new();
        for challenge in repetitions {
            assert!(challenge.gamma > 0 && challenge.gamma < modulus);
            assert!(challenge.beta > 0 && challenge.beta < modulus);
            assert_ne!(challenge.gamma, challenge.beta);
            coordinates.extend([challenge.gamma, challenge.beta]);
            assert!(!pairs.contains(&(challenge.gamma, challenge.beta)));
            pairs.push((challenge.gamma, challenge.beta));
        }
        coordinates.sort_unstable();
        coordinates.dedup();
        assert_eq!(coordinates.len(), ROWS_PER_LIMB_V1);
    }
    let modulus = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[0];
    assert_eq!(map_challenge_candidate_v1(0, modulus, &[]), None);
    assert_eq!(map_challenge_candidate_v1(u64::MAX, modulus, &[]), None);
    let candidate = map_challenge_candidate_v1(1, modulus, &[]).expect("one maps to one");
    assert_eq!(map_challenge_candidate_v1(1, modulus, &[candidate]), None);
    let kat_gamma = derive_aggregation_challenge_coordinate_v1(
        [0x11; DIGEST_BYTES_V1],
        [0x22; DIGEST_BYTES_V1],
        formula,
        mapping,
        0,
        0,
        0,
        modulus,
        &[],
    )
    .expect("KAT gamma");
    let kat_beta = derive_aggregation_challenge_coordinate_v1(
        [0x11; DIGEST_BYTES_V1],
        [0x22; DIGEST_BYTES_V1],
        formula,
        mapping,
        0,
        0,
        1,
        modulus,
        &[kat_gamma],
    )
    .expect("KAT beta");
    assert_eq!(kat_gamma, 1_130_366_289_750_495_907);
    assert_eq!(kat_beta, 413_973_013_125_576_731);
    assert_eq!(
        derive_aggregation_challenge_coordinate_v1(
            [0x11; DIGEST_BYTES_V1],
            [0x22; DIGEST_BYTES_V1],
            formula,
            mapping,
            0,
            0,
            0,
            3,
            &[1, 2],
        ),
        Err(RnsNativeRlweSourceStatementErrorV1::InvalidChallenge)
    );
    let next_limb = derive_aggregation_challenge_coordinate_v1(
        [0x11; DIGEST_BYTES_V1],
        [0x22; DIGEST_BYTES_V1],
        formula,
        mapping,
        1,
        0,
        0,
        ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[1],
        &[],
    )
    .expect("next-limb challenge");
    let next_repetition = derive_aggregation_challenge_coordinate_v1(
        [0x11; DIGEST_BYTES_V1],
        [0x22; DIGEST_BYTES_V1],
        formula,
        mapping,
        0,
        1,
        0,
        modulus,
        &[],
    )
    .expect("next-repetition challenge");
    assert_ne!(kat_gamma, next_limb);
    assert_ne!(kat_gamma, next_repetition);
    let gamma = derive_aggregation_challenge_coordinate_v1(
        fixture.transcript.rns_aggregation_challenge_seed(),
        fixture.qpcs_parameter_digest,
        formula,
        mapping,
        0,
        0,
        0,
        modulus,
        &[],
    )
    .expect("gamma");
    let beta = derive_aggregation_challenge_coordinate_v1(
        fixture.transcript.rns_aggregation_challenge_seed(),
        fixture.qpcs_parameter_digest,
        formula,
        mapping,
        0,
        0,
        1,
        modulus,
        &[gamma],
    )
    .expect("beta");
    assert_ne!(gamma, beta);
}

#[test]
fn public_artifact_rejects_order_context_and_digest_substitution() {
    let fixture = Fixture::new(12);
    validate_public_artifact_v1(&fixture.transcript, fixture.layout, fixture.public_view())
        .expect("valid public artifact");

    let mut view = fixture.public_view();
    view.epoch += 1;
    assert_eq!(
        validate_public_artifact_v1(&fixture.transcript, fixture.layout, view),
        Err(RnsNativeRlweSourceStatementErrorV1::InvalidPublicArtifact)
    );
    let mut view = fixture.public_view();
    view.governed_roster_digest = digest(b"wrong-roster", fixture.context, 0);
    assert_eq!(
        validate_public_artifact_v1(&fixture.transcript, fixture.layout, view),
        Err(RnsNativeRlweSourceStatementErrorV1::InvalidPublicArtifact)
    );
    let mut view = fixture.public_view();
    view.public_bundle_digest = digest(b"wrong-bundle", fixture.context, 0);
    assert_eq!(
        validate_public_artifact_v1(&fixture.transcript, fixture.layout, view),
        Err(RnsNativeRlweSourceStatementErrorV1::InvalidPublicArtifact)
    );
    let mut records = fixture.records.clone();
    records.swap(0, 1);
    let mut view = fixture.public_view();
    view.records = &records;
    assert_eq!(
        validate_public_artifact_v1(&fixture.transcript, fixture.layout, view),
        Err(RnsNativeRlweSourceStatementErrorV1::InvalidSourceOrder)
    );
    let mut records = fixture.records.clone();
    records[0].sample_index = 7;
    let mut view = fixture.public_view();
    view.records = &records;
    assert_eq!(
        validate_public_artifact_v1(&fixture.transcript, fixture.layout, view),
        Err(RnsNativeRlweSourceStatementErrorV1::InvalidSourceOrder)
    );
    let mut c0 = fixture.ciphertext_c0.clone();
    c0.swap(0, 1);
    let mut view = fixture.public_view();
    view.ciphertext_c0_limb_digests = &c0;
    assert_eq!(
        validate_public_artifact_v1(&fixture.transcript, fixture.layout, view),
        Err(RnsNativeRlweSourceStatementErrorV1::InvalidPublicArtifact)
    );
}

#[test]
fn component_decoders_reject_noncanonical_plaintext_and_bad_small_secrets() {
    let mut canonical = vec![0_u8; ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_PLAINTEXT_BYTES_V1 as usize];
    validate_canonical_plaintext_chunk_v1(&canonical).expect("zero canonical plaintext");
    canonical[..CANONICAL_COEFFICIENT_BYTES_V1].copy_from_slice(&VEGA_T256_SCALAR_MODULUS_BE_V1);
    assert_eq!(
        validate_canonical_plaintext_chunk_v1(&canonical),
        Err(RnsNativeRlweSourceStatementErrorV1::NonCanonicalPlaintext)
    );

    let mut ephemeral = vec![0_u8; ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_PLAINTEXT_BYTES_V1 as usize];
    let mut nonzero = false;
    validate_signed_chunk_v1(&ephemeral, SourceComponentV1::Ephemeral, &mut nonzero)
        .expect("zero block is locally bounded");
    assert!(
        !nonzero,
        "the complete polynomial guard must reject all-zero r"
    );
    ephemeral[..SIGNED_COEFFICIENT_BYTES_V1].copy_from_slice(&2_i64.to_be_bytes());
    assert_eq!(
        validate_signed_chunk_v1(&ephemeral, SourceComponentV1::Ephemeral, &mut nonzero),
        Err(RnsNativeRlweSourceStatementErrorV1::InvalidEphemeral)
    );

    let mut error = vec![0_u8; ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_PLAINTEXT_BYTES_V1 as usize];
    error[..SIGNED_COEFFICIENT_BYTES_V1].copy_from_slice(&(-2_i64).to_be_bytes());
    validate_signed_chunk_v1(&error, SourceComponentV1::ErrorZero, &mut nonzero)
        .expect("negative eta boundary");
    error[..SIGNED_COEFFICIENT_BYTES_V1].copy_from_slice(&3_i64.to_be_bytes());
    assert_eq!(
        validate_signed_chunk_v1(&error, SourceComponentV1::ErrorOne, &mut nonzero),
        Err(RnsNativeRlweSourceStatementErrorV1::InvalidError)
    );
}

#[test]
fn source_snapshot_guards_fail_closed_before_construction_state() {
    let fixture = Fixture::new(13);
    let public = fixture.public_view();
    let validated = validate_public_artifact_v1(&fixture.transcript, fixture.layout, public)
        .expect("public artifact");
    for (fault, expected) in [
        (
            SnapshotFault::ReadFailure,
            RnsNativeRlweSourceStatementErrorV1::SourceUnavailable,
        ),
        (
            SnapshotFault::WrongArena,
            RnsNativeRlweSourceStatementErrorV1::InvalidSourceOrder,
        ),
        (
            SnapshotFault::WrongLength,
            RnsNativeRlweSourceStatementErrorV1::InvalidSourceEncoding,
        ),
        (
            SnapshotFault::NonCanonicalPlaintext,
            RnsNativeRlweSourceStatementErrorV1::NonCanonicalPlaintext,
        ),
        (
            SnapshotFault::ZeroEphemeral,
            RnsNativeRlweSourceStatementErrorV1::InvalidEphemeral,
        ),
        (
            SnapshotFault::OutOfRangeEphemeral,
            RnsNativeRlweSourceStatementErrorV1::InvalidEphemeral,
        ),
        (
            SnapshotFault::OutOfRangeError,
            RnsNativeRlweSourceStatementErrorV1::InvalidError,
        ),
        (
            SnapshotFault::ZeroNonce,
            RnsNativeRlweSourceStatementErrorV1::InvalidNonce,
        ),
    ] {
        let mut snapshot = TestSnapshot::new(fixture.layout, fixture.context, fault);
        assert_eq!(
            validate_source_snapshot_v1(
                &mut snapshot,
                fixture.layout,
                fixture.receipt,
                public,
                validated.public_key_digest,
            ),
            Err(expected)
        );
    }
    let mut wrong_receipt_snapshot =
        TestSnapshot::new(fixture.layout, fixture.context + 1, SnapshotFault::None);
    assert_eq!(
        validate_source_snapshot_v1(
            &mut wrong_receipt_snapshot,
            fixture.layout,
            fixture.receipt,
            public,
            validated.public_key_digest,
        ),
        Err(RnsNativeRlweSourceStatementErrorV1::InvalidContext)
    );
    let position = record_position_v1(0).expect("first record");
    assert_eq!(
        nonce_binding_digest_v1(
            fixture.layout,
            fixture.epoch,
            fixture.roster_digest,
            validated.public_key_digest,
            position,
            0,
            &[0; ZK_AMS_MKHE_RNS_NATIVE_SOURCE_NONCE_PLAINTEXT_BYTES_V1 as usize],
        ),
        Err(RnsNativeRlweSourceStatementErrorV1::InvalidNonce)
    );
    let mut records = fixture.records.clone();
    records[0].nonce_binding_digest = digest(b"substituted-nonce", fixture.context, 0);
    let mut substituted_public = public;
    substituted_public.records = &records;
    let mut snapshot = TestSnapshot::new(fixture.layout, fixture.context, SnapshotFault::None);
    assert_eq!(
        validate_source_snapshot_v1(
            &mut snapshot,
            fixture.layout,
            fixture.receipt,
            substituted_public,
            validated.public_key_digest,
        ),
        Err(RnsNativeRlweSourceStatementErrorV1::InvalidNonce)
    );
}

#[test]
fn complete_preflight_is_move_only_non_authorizing_and_anchor_bound() {
    let fixture = Fixture::new(14);
    let downstream = b"rlwe-relation-still-required";
    let anchor = fixture.anchor(downstream);
    let parts = validate_preflight_parts_v1(
        &fixture.transcript,
        fixture.layout,
        fixture.receipt,
        fixture.public_view(),
        &fixture.equation_commitments,
        &fixture.limb_commitments,
        TestSnapshot::new(fixture.layout, fixture.context, SnapshotFault::None),
        fixture.qpcs(&anchor),
    )
    .expect("source-statement preflight");
    assert_eq!(parts.anchor.downstream, downstream);
    assert_eq!(parts.anchor.epoch, fixture.epoch);
    assert_eq!(parts.snapshot.next_record, OPENING_COUNT_V1);
    assert_ne!(
        parts.derived.preflight_statement_digest,
        [0; DIGEST_BYTES_V1]
    );
    assert_ne!(parts.statement_anchor_digest, [0; DIGEST_BYTES_V1]);
    assert_ne!(parts.derived.formula_digest, parts.derived.mapping_digest);

    let mut corrupted = anchor.clone();
    corrupted[ANCHOR_HEADER_BYTES_V1 + CORE_PUBLIC_BUNDLE_V1 * DIGEST_BYTES_V1] ^= 1;
    assert_eq!(
        validate_preflight_parts_v1(
            &fixture.transcript,
            fixture.layout,
            fixture.receipt,
            fixture.public_view(),
            &fixture.equation_commitments,
            &fixture.limb_commitments,
            TestSnapshot::new(fixture.layout, fixture.context, SnapshotFault::None),
            fixture.qpcs(&corrupted),
        )
        .map(|_| ()),
        Err(RnsNativeRlweSourceStatementErrorV1::InvalidAnchor)
    );

    let mut equations = fixture.equation_commitments;
    equations[0] = fixture.limb_commitments[0];
    assert_eq!(
        derive_statement_v1(
            &fixture.transcript,
            fixture.layout,
            fixture.receipt,
            fixture.public_view(),
            &equations,
            &fixture.limb_commitments,
            fixture.qpcs(&anchor),
        )
        .map(|_| ()),
        Err(RnsNativeRlweSourceStatementErrorV1::InvalidPublicArtifact)
    );

    let mut equations = fixture.equation_commitments;
    equations.swap(0, 1);
    assert_eq!(
        validate_preflight_parts_v1(
            &fixture.transcript,
            fixture.layout,
            fixture.receipt,
            fixture.public_view(),
            &equations,
            &fixture.limb_commitments,
            TestSnapshot::new(fixture.layout, fixture.context, SnapshotFault::None),
            fixture.qpcs(&anchor),
        )
        .map(|_| ()),
        Err(RnsNativeRlweSourceStatementErrorV1::InvalidAnchor)
    );
    let mut limbs = fixture.limb_commitments;
    limbs.swap(0, 1);
    assert_eq!(
        validate_preflight_parts_v1(
            &fixture.transcript,
            fixture.layout,
            fixture.receipt,
            fixture.public_view(),
            &fixture.equation_commitments,
            &limbs,
            TestSnapshot::new(fixture.layout, fixture.context, SnapshotFault::None),
            fixture.qpcs(&anchor),
        )
        .map(|_| ()),
        Err(RnsNativeRlweSourceStatementErrorV1::InvalidAnchor)
    );
    let mut qpcs = fixture.qpcs(&anchor);
    qpcs.section_binding_digest = digest(b"spliced-qpcs-section", fixture.context, 0);
    assert_eq!(
        validate_preflight_parts_v1(
            &fixture.transcript,
            fixture.layout,
            fixture.receipt,
            fixture.public_view(),
            &fixture.equation_commitments,
            &fixture.limb_commitments,
            TestSnapshot::new(fixture.layout, fixture.context, SnapshotFault::None),
            qpcs,
        )
        .map(|_| ()),
        Err(RnsNativeRlweSourceStatementErrorV1::InvalidAnchor)
    );
    let other = Fixture::new(15);
    assert_eq!(
        validate_preflight_parts_v1(
            &other.transcript,
            other.layout,
            other.receipt,
            other.public_view(),
            &other.equation_commitments,
            &other.limb_commitments,
            TestSnapshot::new(other.layout, other.context, SnapshotFault::None),
            other.qpcs(&anchor),
        )
        .map(|_| ()),
        Err(RnsNativeRlweSourceStatementErrorV1::InvalidAnchor)
    );
}
