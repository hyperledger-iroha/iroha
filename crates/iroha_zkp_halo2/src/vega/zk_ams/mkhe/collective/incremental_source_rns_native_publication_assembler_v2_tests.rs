use super::*;

use std::{
    cell::Cell,
    convert::Infallible,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

const FIXTURE_COMPOSITE_PROVIDER_DIGEST_HEX_V2: &str =
    "6d950e61317cf676f663b6c2a269d4531e8b9b856335d2600092715d7d713bb2";
const FIXTURE_MANIFEST_DIGEST_HEX_V2: &str =
    "8e68b022c469dc8c5f4dabd243d7b2c2872ae9580b6a827d48e9b2c8c8639ff0";
const FIXTURE_HANDOFF_DIGEST_HEX_V2: &str =
    "e391e7f26a7a02e4176473760fd59cec41d11ec8cb5be0768de11ce76be95a87";
const FIXTURE_CONSUMING_OWNER_COUNT_V2: usize = 1 + RNS_NATIVE_RECORD_COUNT_V2 + 1;

#[derive(Default)]
struct FixtureDigestEngineV2;

impl sealed::PublicationDigestEngineV2 for FixtureDigestEngineV2 {}

impl RnsNativePublicationDigestEngineV2 for FixtureDigestEngineV2 {
    fn digest_v2(&mut self, domain: &'static [u8], transcript: &[u8]) -> RnsNativeContractDigestV2 {
        let mut lanes = [
            0xcbf2_9ce4_8422_2325_u64,
            0x8422_2325_cbf2_9ce4_u64,
            0x9e37_79b1_85eb_ca87_u64,
            0xd6e8_feb8_6659_fd93_u64,
        ];
        for byte in domain
            .iter()
            .copied()
            .chain(core::iter::once(0xff))
            .chain(transcript.iter().copied())
        {
            for (lane_ordinal, lane) in lanes.iter_mut().enumerate() {
                let mixed = (*lane ^ (u64::from(byte) | ((lane_ordinal as u64) << 8)))
                    .wrapping_mul(0x0000_0100_0000_01b3);
                *lane = mixed.rotate_left(5 + 7 * lane_ordinal as u32)
                    ^ (mixed >> (29 + lane_ordinal as u32));
            }
        }
        let mut output = [0_u8; 32];
        for (lane_ordinal, lane) in lanes.into_iter().enumerate() {
            output[lane_ordinal * 8..(lane_ordinal + 1) * 8].copy_from_slice(&lane.to_be_bytes());
        }
        RnsNativeContractDigestV2(output)
    }
}

struct ZeroDigestEngineV2;

impl sealed::PublicationDigestEngineV2 for ZeroDigestEngineV2 {}

impl RnsNativePublicationDigestEngineV2 for ZeroDigestEngineV2 {
    fn digest_v2(
        &mut self,
        _domain: &'static [u8],
        _transcript: &[u8],
    ) -> RnsNativeContractDigestV2 {
        RnsNativeContractDigestV2::ZERO
    }
}

struct ZeroOnDigestCallV2 {
    call_count: usize,
    zero_on_call: usize,
}

impl sealed::PublicationDigestEngineV2 for ZeroOnDigestCallV2 {}

impl RnsNativePublicationDigestEngineV2 for ZeroOnDigestCallV2 {
    fn digest_v2(&mut self, domain: &'static [u8], transcript: &[u8]) -> RnsNativeContractDigestV2 {
        self.call_count += 1;
        if self.call_count == self.zero_on_call {
            RnsNativeContractDigestV2::ZERO
        } else {
            let mut fixture = FixtureDigestEngineV2;
            fixture.digest_v2(domain, transcript)
        }
    }
}

struct PanicOnDigestCallV2 {
    call_count: usize,
    panic_on_call: usize,
}

impl sealed::PublicationDigestEngineV2 for PanicOnDigestCallV2 {}

impl RnsNativePublicationDigestEngineV2 for PanicOnDigestCallV2 {
    fn digest_v2(&mut self, domain: &'static [u8], transcript: &[u8]) -> RnsNativeContractDigestV2 {
        self.call_count += 1;
        assert_ne!(
            self.call_count, self.panic_on_call,
            "fixture digest-engine panic"
        );
        let mut fixture = FixtureDigestEngineV2;
        fixture.digest_v2(domain, transcript)
    }
}

#[derive(Debug, PartialEq, Eq)]
struct TranscriptCallV2 {
    domain: &'static [u8],
    transcript: Vec<u8>,
}

#[derive(Default)]
struct TranscriptSpyDigestEngineV2 {
    calls: Vec<TranscriptCallV2>,
}

impl sealed::PublicationDigestEngineV2 for TranscriptSpyDigestEngineV2 {}

impl RnsNativePublicationDigestEngineV2 for TranscriptSpyDigestEngineV2 {
    fn digest_v2(&mut self, domain: &'static [u8], transcript: &[u8]) -> RnsNativeContractDigestV2 {
        self.calls.push(TranscriptCallV2 {
            domain,
            transcript: transcript.to_vec(),
        });
        let mut fixture = FixtureDigestEngineV2;
        fixture.digest_v2(domain, transcript)
    }
}

fn fixture_digest_v2(namespace: u8, ordinal: u64) -> RnsNativeContractDigestV2 {
    let mut bytes = [0_u8; 32];
    bytes[0] = namespace;
    bytes[1..9].copy_from_slice(&ordinal.to_be_bytes());
    for (offset, byte) in bytes[9..].iter_mut().enumerate() {
        *byte = namespace
            .wrapping_mul(17)
            .wrapping_add((offset as u8).wrapping_mul(29))
            .wrapping_add(ordinal as u8);
    }
    RnsNativeContractDigestV2(bytes)
}

fn digest_hex_v2(digest: RnsNativeContractDigestV2) -> String {
    let mut output = String::with_capacity(64);
    for byte in digest.0 {
        use core::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn append_expected_u64_v2(transcript: &mut Vec<u8>, value: u64) {
    transcript.extend_from_slice(&value.to_be_bytes());
}

fn append_expected_digest_v2(transcript: &mut Vec<u8>, digest: RnsNativeContractDigestV2) {
    transcript.extend_from_slice(&digest.0);
}

fn fixture_hash_v2(domain: &'static [u8], transcript: &[u8]) -> RnsNativeContractDigestV2 {
    let mut fixture = FixtureDigestEngineV2;
    fixture.digest_v2(domain, transcript)
}

fn expected_composite_provider_transcript_v2() -> Vec<u8> {
    let mut transcript = Vec::new();
    append_expected_u64_v2(&mut transcript, 2);
    for namespace in 1_u8..=6 {
        append_expected_digest_v2(&mut transcript, fixture_digest_v2(namespace, 0));
    }
    transcript
}

fn append_expected_composite_provider_identity_v2(
    transcript: &mut Vec<u8>,
    composite_identity: RnsNativeContractDigestV2,
) {
    for namespace in 1_u8..=6 {
        append_expected_digest_v2(transcript, fixture_digest_v2(namespace, 0));
    }
    append_expected_digest_v2(transcript, composite_identity);
}

fn expected_canonical_position_frame_v2(ordinal: usize) -> (u8, u64, u64, u8) {
    if ordinal < RNS_NATIVE_TARGET_LIMB_COUNT_V2 {
        return (0, u64::MAX, ordinal as u64, 0);
    }
    if ordinal < 2 * RNS_NATIVE_TARGET_LIMB_COUNT_V2 {
        return (
            1,
            u64::MAX,
            (ordinal - RNS_NATIVE_TARGET_LIMB_COUNT_V2) as u64,
            0,
        );
    }
    let c0_end = 2 * RNS_NATIVE_TARGET_LIMB_COUNT_V2
        + RNS_NATIVE_RECORD_COUNT_V2 * RNS_NATIVE_TARGET_LIMB_COUNT_V2;
    if ordinal < c0_end {
        let relative = ordinal - 2 * RNS_NATIVE_TARGET_LIMB_COUNT_V2;
        return (
            2,
            (relative / RNS_NATIVE_TARGET_LIMB_COUNT_V2) as u64,
            (relative % RNS_NATIVE_TARGET_LIMB_COUNT_V2) as u64,
            1,
        );
    }
    assert!(ordinal < RNS_NATIVE_CANONICAL_DESCRIPTOR_COUNT_V2);
    let relative = ordinal - c0_end;
    (
        3,
        (relative / RNS_NATIVE_TARGET_LIMB_COUNT_V2) as u64,
        (relative % RNS_NATIVE_TARGET_LIMB_COUNT_V2) as u64,
        1,
    )
}

fn expected_canonical_manifest_transcript_v2(
    composite_identity: RnsNativeContractDigestV2,
) -> Vec<u8> {
    let mut transcript = Vec::new();
    for value in [
        2,
        RNS_NATIVE_LEGACY_LIMB_COUNT_V2 as u64,
        RNS_NATIVE_TARGET_LIMB_COUNT_V2 as u64,
        RNS_NATIVE_RECORD_COUNT_V2 as u64,
        RNS_NATIVE_CANONICAL_DESCRIPTOR_COUNT_V2 as u64,
        RNS_NATIVE_PREFIX_RECEIPT_COUNT_V2 as u64,
        RNS_NATIVE_TAIL_RECEIPT_COUNT_V2 as u64,
        RNS_NATIVE_OBJECT_ENCODED_BYTE_COUNT_V2 as u64,
    ] {
        append_expected_u64_v2(&mut transcript, value);
    }
    for namespace in 7_u8..=10 {
        append_expected_digest_v2(&mut transcript, fixture_digest_v2(namespace, 0));
    }
    append_expected_digest_v2(&mut transcript, fixture_digest_v2(11, 0));
    append_expected_digest_v2(&mut transcript, fixture_digest_v2(12, 0));
    append_expected_composite_provider_identity_v2(&mut transcript, composite_identity);
    append_expected_u64_v2(&mut transcript, RNS_NATIVE_RECORD_COUNT_V2 as u64);
    for record_ordinal in 0..RNS_NATIVE_RECORD_COUNT_V2 {
        append_expected_u64_v2(&mut transcript, record_ordinal as u64);
        append_expected_u64_v2(&mut transcript, record_ordinal as u64);
        append_expected_digest_v2(
            &mut transcript,
            fixture_digest_v2(13, record_ordinal as u64),
        );
        append_expected_digest_v2(
            &mut transcript,
            fixture_digest_v2(14, record_ordinal as u64),
        );
    }
    for ordinal in 0..RNS_NATIVE_CANONICAL_DESCRIPTOR_COUNT_V2 {
        let (role, record_ordinal, limb_ordinal, provider_route) =
            expected_canonical_position_frame_v2(ordinal);
        transcript.push(role);
        append_expected_u64_v2(&mut transcript, record_ordinal);
        append_expected_u64_v2(&mut transcript, limb_ordinal);
        transcript.push(provider_route);
        for namespace in 0x40_u8..=0x43 {
            append_expected_digest_v2(
                &mut transcript,
                fixture_digest_v2(namespace, ordinal as u64),
            );
        }
    }
    transcript
}

fn expected_reader_handoff_transcript_v2(
    canonical_manifest_digest: RnsNativeContractDigestV2,
    composite_identity: RnsNativeContractDigestV2,
) -> Vec<u8> {
    let mut transcript = Vec::new();
    append_expected_u64_v2(&mut transcript, 2);
    append_expected_u64_v2(
        &mut transcript,
        RNS_NATIVE_CANONICAL_DESCRIPTOR_COUNT_V2 as u64,
    );
    append_expected_digest_v2(&mut transcript, canonical_manifest_digest);
    append_expected_composite_provider_identity_v2(&mut transcript, composite_identity);
    transcript
}

fn key_provider_v2() -> RnsNativeProviderSnapshotReadbackV2 {
    RnsNativeProviderSnapshotReadbackV2 {
        provider_identity: fixture_digest_v2(1, 0),
        snapshot_identity: fixture_digest_v2(2, 0),
        readback_identity: fixture_digest_v2(3, 0),
    }
}

fn ciphertext_provider_v2() -> RnsNativeProviderSnapshotReadbackV2 {
    RnsNativeProviderSnapshotReadbackV2 {
        provider_identity: fixture_digest_v2(4, 0),
        snapshot_identity: fixture_digest_v2(5, 0),
        readback_identity: fixture_digest_v2(6, 0),
    }
}

fn authority_identity_v2() -> RnsNativeFinalizedStreamingAuthorityIdentityV2 {
    RnsNativeFinalizedStreamingAuthorityIdentityV2 {
        governed_context_digest: fixture_digest_v2(7, 0),
        collective_key_digest: fixture_digest_v2(8, 0),
        streaming_binding_digest: fixture_digest_v2(9, 0),
        authority_digest: fixture_digest_v2(10, 0),
    }
}

fn receipt_v2(
    position: RnsNativeCanonicalPositionV2,
    binding: RnsNativeProviderSnapshotReadbackV2,
    origin: RnsNativeContractDigestV2,
) -> RnsNativeObjectPublicationEvidenceV2 {
    let ordinal = position
        .canonical_ordinal_v2()
        .expect("fixture positions are canonical") as u64;
    let artifact_digest = fixture_digest_v2(0x41, ordinal);
    RnsNativeObjectPublicationEvidenceV2 {
        position,
        provider_route: position.provider_route_v2(),
        provider_binding: binding,
        origin_binding_digest: origin,
        object_pointer_digest: fixture_digest_v2(0x40, ordinal),
        artifact_digest,
        publication_receipt_digest: fixture_digest_v2(0x42, ordinal),
        read_receipt_digest: fixture_digest_v2(0x43, ordinal),
        readback_artifact_digest: artifact_digest,
        encoded_byte_count: RNS_NATIVE_OBJECT_ENCODED_BYTE_COUNT_V2,
    }
}

fn position_v2(
    role: RnsNativePublicPolynomialRoleV2,
    record_ordinal: Option<usize>,
    limb_ordinal: usize,
) -> RnsNativeCanonicalPositionV2 {
    RnsNativeCanonicalPositionV2 {
        role,
        record_ordinal,
        limb_ordinal,
    }
}

fn fixture_evidence_v2() -> (
    RnsNativeFinalizedV1PublicationEvidenceV2,
    RnsNativeBasisExtensionTailLifecycleEvidenceV2,
) {
    let authority = authority_identity_v2();
    let key_provider = key_provider_v2();
    let ciphertext_provider = ciphertext_provider_v2();
    let key_tail_owner_digest = fixture_digest_v2(11, 0);
    let lifecycle_digest = fixture_digest_v2(12, 0);

    let mut key_prefix_receipts = Vec::with_capacity(RNS_NATIVE_KEY_PREFIX_RECEIPT_COUNT_V2);
    for role in [
        RnsNativePublicPolynomialRoleV2::A,
        RnsNativePublicPolynomialRoleV2::B,
    ] {
        for limb in 0..RNS_NATIVE_LEGACY_LIMB_COUNT_V2 {
            key_prefix_receipts.push(receipt_v2(
                position_v2(role, None, limb),
                key_provider,
                authority.authority_digest,
            ));
        }
    }

    let mut manifests = Vec::with_capacity(RNS_NATIVE_RECORD_COUNT_V2);
    for record in 0..RNS_NATIVE_RECORD_COUNT_V2 {
        let manifest_digest = fixture_digest_v2(13, record as u64);
        let mut prefix_receipts = Vec::with_capacity(2 * RNS_NATIVE_LEGACY_LIMB_COUNT_V2);
        for role in [
            RnsNativePublicPolynomialRoleV2::C0,
            RnsNativePublicPolynomialRoleV2::C1,
        ] {
            for limb in 0..RNS_NATIVE_LEGACY_LIMB_COUNT_V2 {
                prefix_receipts.push(receipt_v2(
                    position_v2(role, Some(record), limb),
                    ciphertext_provider,
                    manifest_digest,
                ));
            }
        }
        manifests.push(RnsNativeV1CiphertextManifestEvidenceV2 {
            exact_manifest_owner: RnsNativeExactV1CiphertextManifestOwnerV2::Fixture,
            record_ordinal: record,
            sample_index: record,
            authority_digest: authority.authority_digest,
            manifest_digest,
            provider_binding: ciphertext_provider,
            prefix_receipts: prefix_receipts.into_boxed_slice(),
        });
    }

    let mut key_tail_receipts = Vec::with_capacity(RNS_NATIVE_KEY_TAIL_RECEIPT_COUNT_V2);
    for role in [
        RnsNativePublicPolynomialRoleV2::A,
        RnsNativePublicPolynomialRoleV2::B,
    ] {
        for limb in RNS_NATIVE_LEGACY_LIMB_COUNT_V2..RNS_NATIVE_TARGET_LIMB_COUNT_V2 {
            key_tail_receipts.push(receipt_v2(
                position_v2(role, None, limb),
                key_provider,
                key_tail_owner_digest,
            ));
        }
    }

    let mut tail_records = Vec::with_capacity(RNS_NATIVE_RECORD_COUNT_V2);
    for record in 0..RNS_NATIVE_RECORD_COUNT_V2 {
        let completion_digest = fixture_digest_v2(14, record as u64);
        let mut tail_receipts = Vec::with_capacity(4);
        for role in [
            RnsNativePublicPolynomialRoleV2::C0,
            RnsNativePublicPolynomialRoleV2::C1,
        ] {
            for limb in RNS_NATIVE_LEGACY_LIMB_COUNT_V2..RNS_NATIVE_TARGET_LIMB_COUNT_V2 {
                tail_receipts.push(receipt_v2(
                    position_v2(role, Some(record), limb),
                    ciphertext_provider,
                    completion_digest,
                ));
            }
        }
        tail_records.push(RnsNativeV2TailRecordEvidenceV2 {
            record_ordinal: record,
            sample_index: record,
            v1_manifest_digest: fixture_digest_v2(13, record as u64),
            lifecycle_digest,
            completion_digest,
            tail_receipts: tail_receipts.into_boxed_slice(),
        });
    }

    (
        RnsNativeFinalizedV1PublicationEvidenceV2 {
            exact_authority_owner: RnsNativeExactFinalizedV1AuthorityOwnerV2::Fixture,
            authority_identity: authority,
            collective_key_provider: key_provider,
            ciphertext_provider,
            key_prefix_receipts: key_prefix_receipts.into_boxed_slice(),
            ciphertext_manifests: manifests.into_boxed_slice(),
        },
        RnsNativeBasisExtensionTailLifecycleEvidenceV2 {
            exact_lifecycle_owner: RnsNativeExactBasisExtensionLifecycleOwnerV2::Fixture,
            authority_identity: authority,
            collective_key_provider: key_provider,
            ciphertext_provider,
            key_tail_owner_digest,
            lifecycle_digest,
            key_tail_receipts: key_tail_receipts.into_boxed_slice(),
            records: tail_records.into_boxed_slice(),
        },
    )
}

fn fixture_evidence_with_owner_drop_probes_v2(
    drops: &Rc<Cell<usize>>,
) -> (
    RnsNativeFinalizedV1PublicationEvidenceV2,
    RnsNativeBasisExtensionTailLifecycleEvidenceV2,
) {
    let (mut v1, mut tails) = fixture_evidence_v2();
    let probe = || RnsNativeTestOwnerDropProbeV2 {
        drops: Rc::clone(drops),
    };
    v1.exact_authority_owner = RnsNativeExactFinalizedV1AuthorityOwnerV2::DropProbe(probe());
    for manifest in &mut v1.ciphertext_manifests {
        manifest.exact_manifest_owner =
            RnsNativeExactV1CiphertextManifestOwnerV2::DropProbe(probe());
    }
    tails.exact_lifecycle_owner = RnsNativeExactBasisExtensionLifecycleOwnerV2::DropProbe(probe());
    (v1, tails)
}

fn assemble_fixture_v2() -> RnsNativePublicPolynomialPublishedSetV2 {
    let (v1, tails) = fixture_evidence_v2();
    RnsNativePublicPolynomialPublicationAssemblerV2::from_contract_evidence_v2(v1, tails)
        .assemble_v2(&mut FixtureDigestEngineV2)
        .expect("valid fixture must assemble")
}

fn assemble_fixture_with_owner_drop_probes_v2(
    drops: &Rc<Cell<usize>>,
) -> RnsNativePublicPolynomialPublishedSetV2 {
    let (v1, tails) = fixture_evidence_with_owner_drop_probes_v2(drops);
    RnsNativePublicPolynomialPublicationAssemblerV2::from_contract_evidence_v2(v1, tails)
        .assemble_v2(&mut FixtureDigestEngineV2)
        .expect("valid drop-probed fixture must assemble")
}

fn receipt_mut_at_v2<'a>(
    v1: &'a mut RnsNativeFinalizedV1PublicationEvidenceV2,
    tails: &'a mut RnsNativeBasisExtensionTailLifecycleEvidenceV2,
    position: RnsNativeCanonicalPositionV2,
) -> &'a mut RnsNativeObjectPublicationEvidenceV2 {
    match position.role {
        RnsNativePublicPolynomialRoleV2::A => {
            if position.limb_ordinal < RNS_NATIVE_LEGACY_LIMB_COUNT_V2 {
                &mut v1.key_prefix_receipts[position.limb_ordinal]
            } else {
                &mut tails.key_tail_receipts
                    [position.limb_ordinal - RNS_NATIVE_LEGACY_LIMB_COUNT_V2]
            }
        }
        RnsNativePublicPolynomialRoleV2::B => {
            if position.limb_ordinal < RNS_NATIVE_LEGACY_LIMB_COUNT_V2 {
                &mut v1.key_prefix_receipts[RNS_NATIVE_LEGACY_LIMB_COUNT_V2 + position.limb_ordinal]
            } else {
                &mut tails.key_tail_receipts
                    [2 + position.limb_ordinal - RNS_NATIVE_LEGACY_LIMB_COUNT_V2]
            }
        }
        RnsNativePublicPolynomialRoleV2::C0 => {
            let record = position.record_ordinal.expect("C0 record");
            if position.limb_ordinal < RNS_NATIVE_LEGACY_LIMB_COUNT_V2 {
                &mut v1.ciphertext_manifests[record].prefix_receipts[position.limb_ordinal]
            } else {
                &mut tails.records[record].tail_receipts
                    [position.limb_ordinal - RNS_NATIVE_LEGACY_LIMB_COUNT_V2]
            }
        }
        RnsNativePublicPolynomialRoleV2::C1 => {
            let record = position.record_ordinal.expect("C1 record");
            if position.limb_ordinal < RNS_NATIVE_LEGACY_LIMB_COUNT_V2 {
                &mut v1.ciphertext_manifests[record].prefix_receipts
                    [RNS_NATIVE_LEGACY_LIMB_COUNT_V2 + position.limb_ordinal]
            } else {
                &mut tails.records[record].tail_receipts
                    [2 + position.limb_ordinal - RNS_NATIVE_LEGACY_LIMB_COUNT_V2]
            }
        }
    }
}

fn assert_assembly_error_v2(
    v1: RnsNativeFinalizedV1PublicationEvidenceV2,
    tails: RnsNativeBasisExtensionTailLifecycleEvidenceV2,
    expected: impl FnOnce(RnsNativePublicationAssemblyErrorV2) -> bool,
) {
    let error =
        RnsNativePublicPolynomialPublicationAssemblerV2::from_contract_evidence_v2(v1, tails)
            .assemble_v2(&mut FixtureDigestEngineV2)
            .err()
            .expect("mutated fixture must fail closed");
    assert!(expected(error), "unexpected error: {error:?}");
}

#[test]
fn exact_counts_are_internally_consistent_v2() {
    assert_eq!(
        RNS_NATIVE_KEY_PREFIX_RECEIPT_COUNT_V2,
        2 * RNS_NATIVE_LEGACY_LIMB_COUNT_V2
    );
    assert_eq!(
        RNS_NATIVE_CIPHERTEXT_PREFIX_RECEIPT_COUNT_V2,
        2 * RNS_NATIVE_RECORD_COUNT_V2 * RNS_NATIVE_LEGACY_LIMB_COUNT_V2
    );
    assert_eq!(RNS_NATIVE_PREFIX_RECEIPT_COUNT_V2, 3_344);
    assert_eq!(RNS_NATIVE_KEY_TAIL_RECEIPT_COUNT_V2, 4);
    assert_eq!(RNS_NATIVE_CIPHERTEXT_TAIL_RECEIPT_COUNT_V2, 172);
    assert_eq!(RNS_NATIVE_TAIL_RECEIPT_COUNT_V2, 176);
    assert_eq!(
        RNS_NATIVE_PREFIX_RECEIPT_COUNT_V2 + RNS_NATIVE_TAIL_RECEIPT_COUNT_V2,
        RNS_NATIVE_CANONICAL_DESCRIPTOR_COUNT_V2
    );
    assert_eq!(RNS_NATIVE_OBJECT_ENCODED_BYTE_COUNT_V2, 1_048_580);
}

#[test]
fn canonical_ordinal_is_a_total_bijection_v2() {
    let mut seen = BTreeSet::new();
    for ordinal in 0..RNS_NATIVE_CANONICAL_DESCRIPTOR_COUNT_V2 {
        let position = RnsNativeCanonicalPositionV2::from_canonical_ordinal_v2(ordinal)
            .expect("all canonical ordinals exist");
        assert_eq!(position.canonical_ordinal_v2(), Some(ordinal));
        assert!(seen.insert(position));
    }
    assert_eq!(seen.len(), RNS_NATIVE_CANONICAL_DESCRIPTOR_COUNT_V2);
    assert!(
        RnsNativeCanonicalPositionV2::from_canonical_ordinal_v2(
            RNS_NATIVE_CANONICAL_DESCRIPTOR_COUNT_V2
        )
        .is_none()
    );
    assert_eq!(
        RnsNativeCanonicalPositionV2::from_canonical_ordinal_v2(0),
        Some(position_v2(RnsNativePublicPolynomialRoleV2::A, None, 0))
    );
    assert_eq!(
        RnsNativeCanonicalPositionV2::from_canonical_ordinal_v2(39),
        Some(position_v2(RnsNativePublicPolynomialRoleV2::A, None, 39))
    );
    assert_eq!(
        RnsNativeCanonicalPositionV2::from_canonical_ordinal_v2(40),
        Some(position_v2(RnsNativePublicPolynomialRoleV2::B, None, 0))
    );
    assert_eq!(
        RnsNativeCanonicalPositionV2::from_canonical_ordinal_v2(79),
        Some(position_v2(RnsNativePublicPolynomialRoleV2::B, None, 39))
    );
    assert_eq!(
        RnsNativeCanonicalPositionV2::from_canonical_ordinal_v2(80),
        Some(position_v2(RnsNativePublicPolynomialRoleV2::C0, Some(0), 0))
    );
    assert_eq!(
        RnsNativeCanonicalPositionV2::from_canonical_ordinal_v2(1_799),
        Some(position_v2(
            RnsNativePublicPolynomialRoleV2::C0,
            Some(42),
            39
        ))
    );
    assert_eq!(
        RnsNativeCanonicalPositionV2::from_canonical_ordinal_v2(1_800),
        Some(position_v2(RnsNativePublicPolynomialRoleV2::C1, Some(0), 0))
    );
    assert_eq!(
        RnsNativeCanonicalPositionV2::from_canonical_ordinal_v2(3_519),
        Some(position_v2(
            RnsNativePublicPolynomialRoleV2::C1,
            Some(42),
            39
        ))
    );
}

#[test]
fn assembly_reuses_every_exact_receipt_in_canonical_order_v2() {
    let published = assemble_fixture_v2();
    assert_eq!(
        published.descriptors_v2().len(),
        RNS_NATIVE_CANONICAL_DESCRIPTOR_COUNT_V2
    );
    for ordinal in 0..RNS_NATIVE_CANONICAL_DESCRIPTOR_COUNT_V2 {
        let expected = RnsNativeCanonicalPositionV2::from_canonical_ordinal_v2(ordinal)
            .expect("canonical ordinal");
        assert_eq!(published.descriptors_v2()[ordinal].position, expected);
        let receipt = published.receipt_at_v2(ordinal).expect("receipt retained");
        assert_eq!(receipt.position, expected);
        assert_eq!(
            published.descriptors_v2()[ordinal].object_pointer_digest,
            receipt.object_pointer_digest
        );
        assert_eq!(
            published.descriptors_v2()[ordinal].publication_receipt_digest,
            receipt.publication_receipt_digest
        );
        assert_eq!(
            published.descriptors_v2()[ordinal].read_receipt_digest,
            receipt.read_receipt_digest
        );
    }
    assert_eq!(
        digest_hex_v2(published.composite_provider.composite_identity),
        FIXTURE_COMPOSITE_PROVIDER_DIGEST_HEX_V2
    );
    assert_eq!(
        digest_hex_v2(published.canonical_manifest_digest_v2()),
        FIXTURE_MANIFEST_DIGEST_HEX_V2
    );
    assert_eq!(
        digest_hex_v2(published.reader_handoff_digest_v2()),
        FIXTURE_HANDOFF_DIGEST_HEX_V2
    );
}

#[test]
fn transcript_spy_pins_domains_call_order_and_every_framed_component_v2() {
    let expected_composite_transcript = expected_composite_provider_transcript_v2();
    assert_eq!(expected_composite_transcript.len(), 200);
    let expected_composite_digest =
        fixture_hash_v2(COMPOSITE_PROVIDER_DOMAIN_V2, &expected_composite_transcript);

    let expected_manifest_transcript =
        expected_canonical_manifest_transcript_v2(expected_composite_digest);
    assert_eq!(expected_manifest_transcript.len(), 517_848);
    let expected_manifest_digest =
        fixture_hash_v2(CANONICAL_MANIFEST_DOMAIN_V2, &expected_manifest_transcript);

    let expected_handoff_transcript =
        expected_reader_handoff_transcript_v2(expected_manifest_digest, expected_composite_digest);
    assert_eq!(expected_handoff_transcript.len(), 272);

    let (v1, tails) = fixture_evidence_v2();
    let mut spy = TranscriptSpyDigestEngineV2::default();
    let published =
        RnsNativePublicPolynomialPublicationAssemblerV2::from_contract_evidence_v2(v1, tails)
            .assemble_v2(&mut spy)
            .expect("valid fixture must assemble through transcript spy");

    assert_eq!(spy.calls.len(), 3);
    assert_eq!(spy.calls[0].domain, COMPOSITE_PROVIDER_DOMAIN_V2);
    assert_eq!(spy.calls[0].transcript, expected_composite_transcript);
    assert_eq!(spy.calls[1].domain, CANONICAL_MANIFEST_DOMAIN_V2);
    assert_eq!(spy.calls[1].transcript, expected_manifest_transcript);
    assert_eq!(spy.calls[2].domain, READER_HANDOFF_DOMAIN_V2);
    assert_eq!(spy.calls[2].transcript, expected_handoff_transcript);

    assert_eq!(
        published.composite_provider.composite_identity,
        expected_composite_digest
    );
    assert_eq!(
        published.canonical_manifest_digest_v2(),
        expected_manifest_digest
    );
    assert_eq!(
        digest_hex_v2(published.composite_provider.composite_identity),
        FIXTURE_COMPOSITE_PROVIDER_DIGEST_HEX_V2
    );
    assert_eq!(
        digest_hex_v2(published.canonical_manifest_digest_v2()),
        FIXTURE_MANIFEST_DIGEST_HEX_V2
    );
    assert_eq!(
        digest_hex_v2(published.reader_handoff_digest_v2()),
        FIXTURE_HANDOFF_DIGEST_HEX_V2
    );
}

#[test]
fn every_owner_count_is_exact_v2() {
    let (mut v1, tails) = fixture_evidence_v2();
    let mut receipts = v1.key_prefix_receipts.into_vec();
    receipts.pop();
    v1.key_prefix_receipts = receipts.into_boxed_slice();
    assert_assembly_error_v2(v1, tails, |error| {
        matches!(
            error,
            RnsNativePublicationAssemblyErrorV2::Count {
                component: "v1_key_prefix_receipts",
                expected: 76,
                actual: 75
            }
        )
    });

    let (mut v1, tails) = fixture_evidence_v2();
    let duplicate = v1.key_prefix_receipts[0].clone();
    let mut receipts = v1.key_prefix_receipts.into_vec();
    receipts.push(duplicate);
    v1.key_prefix_receipts = receipts.into_boxed_slice();
    assert_assembly_error_v2(v1, tails, |error| {
        matches!(
            error,
            RnsNativePublicationAssemblyErrorV2::Count {
                component: "v1_key_prefix_receipts",
                expected: 76,
                actual: 77
            }
        )
    });

    let (mut v1, tails) = fixture_evidence_v2();
    let mut manifests = v1.ciphertext_manifests.into_vec();
    manifests.pop();
    v1.ciphertext_manifests = manifests.into_boxed_slice();
    assert_assembly_error_v2(v1, tails, |error| {
        matches!(
            error,
            RnsNativePublicationAssemblyErrorV2::Count {
                component: "v1_ciphertext_manifests",
                expected: 43,
                actual: 42
            }
        )
    });

    let (mut v1, tails) = fixture_evidence_v2();
    let mut receipts = v1.ciphertext_manifests[17]
        .prefix_receipts
        .clone()
        .into_vec();
    receipts.pop();
    v1.ciphertext_manifests[17].prefix_receipts = receipts.into_boxed_slice();
    assert_assembly_error_v2(v1, tails, |error| {
        matches!(
            error,
            RnsNativePublicationAssemblyErrorV2::Count {
                component: "v1_ciphertext_prefix_receipts_per_manifest",
                expected: 76,
                actual: 75
            }
        )
    });

    let (v1, mut tails) = fixture_evidence_v2();
    let mut receipts = tails.key_tail_receipts.into_vec();
    receipts.pop();
    tails.key_tail_receipts = receipts.into_boxed_slice();
    assert_assembly_error_v2(v1, tails, |error| {
        matches!(
            error,
            RnsNativePublicationAssemblyErrorV2::Count {
                component: "v2_key_tail_receipts",
                expected: 4,
                actual: 3
            }
        )
    });

    let (v1, mut tails) = fixture_evidence_v2();
    let mut records = tails.records.into_vec();
    records.pop();
    tails.records = records.into_boxed_slice();
    assert_assembly_error_v2(v1, tails, |error| {
        matches!(
            error,
            RnsNativePublicationAssemblyErrorV2::Count {
                component: "v2_tail_records",
                expected: 43,
                actual: 42
            }
        )
    });

    let (v1, mut tails) = fixture_evidence_v2();
    let mut receipts = tails.records[31].tail_receipts.clone().into_vec();
    receipts.pop();
    tails.records[31].tail_receipts = receipts.into_boxed_slice();
    assert_assembly_error_v2(v1, tails, |error| {
        matches!(
            error,
            RnsNativePublicationAssemblyErrorV2::Count {
                component: "v2_ciphertext_tail_receipts_per_record",
                expected: 4,
                actual: 3
            }
        )
    });
}

#[test]
fn record_sample_and_physical_order_are_exact_v2() {
    let (mut v1, tails) = fixture_evidence_v2();
    v1.ciphertext_manifests[7].record_ordinal = 8;
    assert_assembly_error_v2(v1, tails, |error| {
        matches!(
            error,
            RnsNativePublicationAssemblyErrorV2::RecordOrder {
                component: "v1_ciphertext_manifest",
                expected: 7,
                actual: 8
            }
        )
    });

    let (mut v1, tails) = fixture_evidence_v2();
    v1.ciphertext_manifests[7].sample_index = 8;
    assert_assembly_error_v2(v1, tails, |error| {
        matches!(
            error,
            RnsNativePublicationAssemblyErrorV2::SampleOrder {
                component: "v1_ciphertext_manifest",
                record_ordinal: 7,
                expected: 7,
                actual: 8
            }
        )
    });

    let (v1, mut tails) = fixture_evidence_v2();
    tails.records[19].record_ordinal = 20;
    assert_assembly_error_v2(v1, tails, |error| {
        matches!(
            error,
            RnsNativePublicationAssemblyErrorV2::RecordOrder {
                component: "v2_tail_record",
                expected: 19,
                actual: 20
            }
        )
    });

    let (v1, mut tails) = fixture_evidence_v2();
    tails.records[19].sample_index = 20;
    assert_assembly_error_v2(v1, tails, |error| {
        matches!(
            error,
            RnsNativePublicationAssemblyErrorV2::SampleOrder {
                component: "v2_tail_record",
                record_ordinal: 19,
                expected: 19,
                actual: 20
            }
        )
    });

    let (mut v1, tails) = fixture_evidence_v2();
    v1.key_prefix_receipts.swap(37, 38);
    assert_assembly_error_v2(v1, tails, |error| {
        matches!(
            error,
            RnsNativePublicationAssemblyErrorV2::PositionMismatch { .. }
        )
    });

    let (v1, mut tails) = fixture_evidence_v2();
    tails.key_tail_receipts.swap(1, 2);
    assert_assembly_error_v2(v1, tails, |error| {
        matches!(
            error,
            RnsNativePublicationAssemblyErrorV2::PositionMismatch { .. }
        )
    });

    let (mut v1, tails) = fixture_evidence_v2();
    v1.ciphertext_manifests[5].prefix_receipts.swap(37, 38);
    assert_assembly_error_v2(v1, tails, |error| {
        matches!(
            error,
            RnsNativePublicationAssemblyErrorV2::PositionMismatch { .. }
        )
    });

    let (v1, mut tails) = fixture_evidence_v2();
    tails.records[5].tail_receipts.swap(1, 2);
    assert_assembly_error_v2(v1, tails, |error| {
        matches!(
            error,
            RnsNativePublicationAssemblyErrorV2::PositionMismatch { .. }
        )
    });
}

#[test]
fn prefix_reuse_requires_exact_authority_manifest_lifecycle_and_provider_v2() {
    let (v1, mut tails) = fixture_evidence_v2();
    tails.authority_identity.collective_key_digest = fixture_digest_v2(0x80, 0);
    assert_assembly_error_v2(v1, tails, |error| {
        error == RnsNativePublicationAssemblyErrorV2::AuthorityIdentityMismatch
    });

    let (mut v1, mut tails) = fixture_evidence_v2();
    let replacement = fixture_digest_v2(0x81, 0);
    v1.authority_identity.authority_digest = replacement;
    tails.authority_identity.authority_digest = replacement;
    for manifest in &mut v1.ciphertext_manifests {
        manifest.authority_digest = replacement;
    }
    assert_assembly_error_v2(v1, tails, |error| {
        matches!(
            error,
            RnsNativePublicationAssemblyErrorV2::OriginBindingMismatch { .. }
        )
    });

    let (v1, mut tails) = fixture_evidence_v2();
    tails.records[9].v1_manifest_digest = fixture_digest_v2(0x82, 9);
    assert_assembly_error_v2(v1, tails, |error| {
        matches!(
            error,
            RnsNativePublicationAssemblyErrorV2::AuthorityBindingMismatch {
                component: "v2_tail_record_to_v1_manifest",
                record_ordinal: Some(9)
            }
        )
    });

    let (v1, mut tails) = fixture_evidence_v2();
    tails.records[9].lifecycle_digest = fixture_digest_v2(0x83, 9);
    assert_assembly_error_v2(v1, tails, |error| {
        matches!(
            error,
            RnsNativePublicationAssemblyErrorV2::AuthorityBindingMismatch {
                component: "v2_tail_record_to_lifecycle",
                record_ordinal: Some(9)
            }
        )
    });

    let (v1, mut tails) = fixture_evidence_v2();
    tails.collective_key_provider.snapshot_identity = fixture_digest_v2(0x84, 0);
    assert_assembly_error_v2(v1, tails, |error| {
        matches!(
            error,
            RnsNativePublicationAssemblyErrorV2::ProviderBindingMismatch {
                component: "tail_collective_key_provider",
                position: None
            }
        )
    });

    let (v1, mut tails) = fixture_evidence_v2();
    tails.ciphertext_provider.readback_identity = fixture_digest_v2(0x85, 0);
    assert_assembly_error_v2(v1, tails, |error| {
        matches!(
            error,
            RnsNativePublicationAssemblyErrorV2::ProviderBindingMismatch {
                component: "tail_ciphertext_provider",
                position: None
            }
        )
    });
}

#[test]
fn every_owner_root_digest_rejects_zero_v2() {
    for field in 0..4_u8 {
        let (mut v1, tails) = fixture_evidence_v2();
        let expected_field = match field {
            0 => {
                v1.authority_identity.governed_context_digest = RnsNativeContractDigestV2::ZERO;
                "governed_context_digest"
            }
            1 => {
                v1.authority_identity.collective_key_digest = RnsNativeContractDigestV2::ZERO;
                "collective_key_digest"
            }
            2 => {
                v1.authority_identity.streaming_binding_digest = RnsNativeContractDigestV2::ZERO;
                "streaming_binding_digest"
            }
            3 => {
                v1.authority_identity.authority_digest = RnsNativeContractDigestV2::ZERO;
                "authority_digest"
            }
            _ => unreachable!(),
        };
        assert_assembly_error_v2(
            v1,
            tails,
            |error| matches!(error, RnsNativePublicationAssemblyErrorV2::ZeroDigest { component: "v1_authority", field, position: None } if field == expected_field),
        );
    }

    for field in 0..3_u8 {
        let (mut v1, tails) = fixture_evidence_v2();
        let expected_field = match field {
            0 => {
                v1.collective_key_provider.provider_identity = RnsNativeContractDigestV2::ZERO;
                "provider_identity"
            }
            1 => {
                v1.collective_key_provider.snapshot_identity = RnsNativeContractDigestV2::ZERO;
                "snapshot_identity"
            }
            2 => {
                v1.collective_key_provider.readback_identity = RnsNativeContractDigestV2::ZERO;
                "readback_identity"
            }
            _ => unreachable!(),
        };
        assert_assembly_error_v2(
            v1,
            tails,
            |error| matches!(error, RnsNativePublicationAssemblyErrorV2::ZeroDigest { component: "v1_collective_key_provider", field, position: None } if field == expected_field),
        );
    }

    let (v1, mut tails) = fixture_evidence_v2();
    tails.key_tail_owner_digest = RnsNativeContractDigestV2::ZERO;
    assert_assembly_error_v2(v1, tails, |error| {
        matches!(
            error,
            RnsNativePublicationAssemblyErrorV2::ZeroDigest {
                component: "v2_tails",
                field: "key_tail_owner_digest",
                position: None
            }
        )
    });

    let (v1, mut tails) = fixture_evidence_v2();
    tails.lifecycle_digest = RnsNativeContractDigestV2::ZERO;
    assert_assembly_error_v2(v1, tails, |error| {
        matches!(
            error,
            RnsNativePublicationAssemblyErrorV2::ZeroDigest {
                component: "v2_tails",
                field: "lifecycle_digest",
                position: None
            }
        )
    });

    let (mut v1, tails) = fixture_evidence_v2();
    v1.ciphertext_manifests[3].manifest_digest = RnsNativeContractDigestV2::ZERO;
    assert_assembly_error_v2(v1, tails, |error| {
        matches!(
            error,
            RnsNativePublicationAssemblyErrorV2::ZeroDigest {
                component: "v1_ciphertext_manifest",
                field: "manifest_digest",
                position: None
            }
        )
    });

    let (v1, mut tails) = fixture_evidence_v2();
    tails.records[3].completion_digest = RnsNativeContractDigestV2::ZERO;
    assert_assembly_error_v2(v1, tails, |error| {
        matches!(
            error,
            RnsNativePublicationAssemblyErrorV2::ZeroDigest {
                component: "v2_tail_record",
                field: "completion_digest",
                position: None
            }
        )
    });
}

#[test]
fn every_receipt_ordinal_rejects_every_invalid_field_class_v2() {
    let (v1, tails) = fixture_evidence_v2();
    for ordinal in 0..RNS_NATIVE_CANONICAL_DESCRIPTOR_COUNT_V2 {
        let expected = RnsNativeCanonicalPositionV2::from_canonical_ordinal_v2(ordinal)
            .expect("canonical position");
        let (receipt, origin) = receipt_and_origin_at_v2(&v1, &tails, expected);
        let binding = receipt.provider_binding;
        validate_receipt_v2(receipt, expected, binding, origin).expect("fixture receipt valid");

        let mut bad = receipt.clone();
        bad.position = RnsNativeCanonicalPositionV2::from_canonical_ordinal_v2(
            (ordinal + 1) % RNS_NATIVE_CANONICAL_DESCRIPTOR_COUNT_V2,
        )
        .expect("alternate position");
        assert!(matches!(
            validate_receipt_v2(&bad, expected, binding, origin),
            Err(RnsNativePublicationAssemblyErrorV2::PositionMismatch { .. })
        ));

        let mut bad = receipt.clone();
        bad.provider_route = match bad.provider_route {
            RnsNativeProviderRouteV2::CollectiveKey => RnsNativeProviderRouteV2::Ciphertext,
            RnsNativeProviderRouteV2::Ciphertext => RnsNativeProviderRouteV2::CollectiveKey,
        };
        assert!(matches!(
            validate_receipt_v2(&bad, expected, binding, origin),
            Err(RnsNativePublicationAssemblyErrorV2::ProviderRouteMismatch { .. })
        ));

        for provider_field in 0..3_u8 {
            let mut bad = receipt.clone();
            let replacement = fixture_digest_v2(0x90 + provider_field, ordinal as u64);
            match provider_field {
                0 => bad.provider_binding.provider_identity = replacement,
                1 => bad.provider_binding.snapshot_identity = replacement,
                2 => bad.provider_binding.readback_identity = replacement,
                _ => unreachable!(),
            }
            assert!(matches!(
                validate_receipt_v2(&bad, expected, binding, origin),
                Err(RnsNativePublicationAssemblyErrorV2::ProviderBindingMismatch { .. })
            ));
        }

        let mut bad = receipt.clone();
        bad.origin_binding_digest = fixture_digest_v2(0x94, ordinal as u64);
        assert!(matches!(
            validate_receipt_v2(&bad, expected, binding, origin),
            Err(RnsNativePublicationAssemblyErrorV2::OriginBindingMismatch { .. })
        ));

        for field in [
            "object_pointer_digest",
            "artifact_digest",
            "publication_receipt_digest",
            "read_receipt_digest",
            "readback_artifact_digest",
        ] {
            let mut bad = receipt.clone();
            match field {
                "object_pointer_digest" => {
                    bad.object_pointer_digest = RnsNativeContractDigestV2::ZERO
                }
                "artifact_digest" => bad.artifact_digest = RnsNativeContractDigestV2::ZERO,
                "publication_receipt_digest" => {
                    bad.publication_receipt_digest = RnsNativeContractDigestV2::ZERO;
                }
                "read_receipt_digest" => bad.read_receipt_digest = RnsNativeContractDigestV2::ZERO,
                "readback_artifact_digest" => {
                    bad.readback_artifact_digest = RnsNativeContractDigestV2::ZERO;
                }
                _ => unreachable!(),
            }
            assert!(matches!(
                validate_receipt_v2(&bad, expected, binding, origin),
                Err(RnsNativePublicationAssemblyErrorV2::ZeroDigest { field: actual, .. }) if actual == field
            ));
        }

        let mut bad = receipt.clone();
        bad.readback_artifact_digest = fixture_digest_v2(0x95, ordinal as u64);
        assert!(matches!(
            validate_receipt_v2(&bad, expected, binding, origin),
            Err(RnsNativePublicationAssemblyErrorV2::ReadbackArtifactMismatch { .. })
        ));

        let mut bad = receipt.clone();
        bad.encoded_byte_count -= 1;
        assert!(matches!(
            validate_receipt_v2(&bad, expected, binding, origin),
            Err(RnsNativePublicationAssemblyErrorV2::EncodedByteCount { .. })
        ));
    }
}

#[test]
fn duplicate_pointer_artifact_publication_and_read_receipts_are_rejected_v2() {
    let first = position_v2(RnsNativePublicPolynomialRoleV2::A, None, 0);
    let second = position_v2(RnsNativePublicPolynomialRoleV2::A, None, 1);
    let final_tail = position_v2(
        RnsNativePublicPolynomialRoleV2::C1,
        Some(RNS_NATIVE_RECORD_COUNT_V2 - 1),
        RNS_NATIVE_TARGET_LIMB_COUNT_V2 - 1,
    );

    let (mut v1, mut tails) = fixture_evidence_v2();
    let duplicate = receipt_mut_at_v2(&mut v1, &mut tails, first).object_pointer_digest;
    receipt_mut_at_v2(&mut v1, &mut tails, second).object_pointer_digest = duplicate;
    assert_assembly_error_v2(
        v1,
        tails,
        |error| matches!(error, RnsNativePublicationAssemblyErrorV2::DuplicateObjectPointer { position } if position == second),
    );

    let (mut v1, mut tails) = fixture_evidence_v2();
    let duplicate = receipt_mut_at_v2(&mut v1, &mut tails, first).artifact_digest;
    let receipt = receipt_mut_at_v2(&mut v1, &mut tails, final_tail);
    receipt.artifact_digest = duplicate;
    receipt.readback_artifact_digest = duplicate;
    assert_assembly_error_v2(
        v1,
        tails,
        |error| matches!(error, RnsNativePublicationAssemblyErrorV2::DuplicateArtifact { position } if position == final_tail),
    );

    let (mut v1, mut tails) = fixture_evidence_v2();
    let duplicate = receipt_mut_at_v2(&mut v1, &mut tails, first).publication_receipt_digest;
    receipt_mut_at_v2(&mut v1, &mut tails, second).publication_receipt_digest = duplicate;
    assert_assembly_error_v2(
        v1,
        tails,
        |error| matches!(error, RnsNativePublicationAssemblyErrorV2::DuplicatePublicationReceipt { position } if position == second),
    );

    let (mut v1, mut tails) = fixture_evidence_v2();
    let duplicate = receipt_mut_at_v2(&mut v1, &mut tails, first).read_receipt_digest;
    receipt_mut_at_v2(&mut v1, &mut tails, second).read_receipt_digest = duplicate;
    assert_assembly_error_v2(
        v1,
        tails,
        |error| matches!(error, RnsNativePublicationAssemblyErrorV2::DuplicateReadReceipt { position } if position == second),
    );
}

#[test]
fn record_owner_digests_are_unique_within_and_across_v1_v2_boundaries_v2() {
    let last = RNS_NATIVE_RECORD_COUNT_V2 - 1;

    let (mut v1, mut tails) = fixture_evidence_v2();
    let duplicate = v1.ciphertext_manifests[0].manifest_digest;
    v1.ciphertext_manifests[last].manifest_digest = duplicate;
    for receipt in &mut v1.ciphertext_manifests[last].prefix_receipts {
        receipt.origin_binding_digest = duplicate;
    }
    tails.records[last].v1_manifest_digest = duplicate;
    assert_assembly_error_v2(
        v1,
        tails,
        |error| matches!(error, RnsNativePublicationAssemblyErrorV2::DuplicateV1ManifestDigest { record_ordinal } if record_ordinal == last),
    );

    let (v1, mut tails) = fixture_evidence_v2();
    let duplicate = tails.records[0].completion_digest;
    tails.records[last].completion_digest = duplicate;
    for receipt in &mut tails.records[last].tail_receipts {
        receipt.origin_binding_digest = duplicate;
    }
    assert_assembly_error_v2(
        v1,
        tails,
        |error| matches!(error, RnsNativePublicationAssemblyErrorV2::DuplicateV2CompletionDigest { record_ordinal } if record_ordinal == last),
    );

    let (v1, mut tails) = fixture_evidence_v2();
    let v1_digest = v1.ciphertext_manifests[0].manifest_digest;
    tails.records[last].completion_digest = v1_digest;
    for receipt in &mut tails.records[last].tail_receipts {
        receipt.origin_binding_digest = v1_digest;
    }
    assert_assembly_error_v2(
        v1,
        tails,
        |error| matches!(error, RnsNativePublicationAssemblyErrorV2::DuplicateV2CompletionDigest { record_ordinal } if record_ordinal == last),
    );

    let (mut v1, mut tails) = fixture_evidence_v2();
    let v2_digest = tails.records[0].completion_digest;
    v1.ciphertext_manifests[last].manifest_digest = v2_digest;
    for receipt in &mut v1.ciphertext_manifests[last].prefix_receipts {
        receipt.origin_binding_digest = v2_digest;
    }
    tails.records[last].v1_manifest_digest = v2_digest;
    assert_assembly_error_v2(
        v1,
        tails,
        |error| matches!(error, RnsNativePublicationAssemblyErrorV2::DuplicateV1ManifestDigest { record_ordinal } if record_ordinal == last),
    );
}

#[test]
fn every_descriptor_digest_class_is_manifest_bound_across_all_boundaries_v2() {
    let baseline = assemble_fixture_v2().canonical_manifest_digest_v2();
    let boundary_ordinals = [
        0, 37, 38, 39, 40, 77, 78, 79, 80, 117, 118, 1_799, 1_800, 3_519,
    ];
    for ordinal in boundary_ordinals {
        for field in 0..4_u8 {
            let (mut v1, mut tails) = fixture_evidence_v2();
            let position = RnsNativeCanonicalPositionV2::from_canonical_ordinal_v2(ordinal)
                .expect("boundary ordinal");
            let receipt = receipt_mut_at_v2(&mut v1, &mut tails, position);
            let replacement = fixture_digest_v2(0xa0 + field, ordinal as u64);
            match field {
                0 => receipt.object_pointer_digest = replacement,
                1 => {
                    receipt.artifact_digest = replacement;
                    receipt.readback_artifact_digest = replacement;
                }
                2 => receipt.publication_receipt_digest = replacement,
                3 => receipt.read_receipt_digest = replacement,
                _ => unreachable!(),
            }
            let mutated =
                RnsNativePublicPolynomialPublicationAssemblerV2::from_contract_evidence_v2(
                    v1, tails,
                )
                .assemble_v2(&mut FixtureDigestEngineV2)
                .expect("valid mutation remains structurally valid")
                .canonical_manifest_digest_v2();
            assert_ne!(mutated, baseline, "ordinal {ordinal}, field {field}");
        }
    }
}

#[test]
fn every_owner_lineage_digest_is_manifest_bound_v2() {
    let baseline = assemble_fixture_v2().canonical_manifest_digest_v2();

    let (mut v1, mut tails) = fixture_evidence_v2();
    let replacement = fixture_digest_v2(0xc0, 17);
    v1.ciphertext_manifests[17].manifest_digest = replacement;
    for receipt in &mut v1.ciphertext_manifests[17].prefix_receipts {
        receipt.origin_binding_digest = replacement;
    }
    tails.records[17].v1_manifest_digest = replacement;
    let mutated =
        RnsNativePublicPolynomialPublicationAssemblerV2::from_contract_evidence_v2(v1, tails)
            .assemble_v2(&mut FixtureDigestEngineV2)
            .expect("consistently rebound manifest remains structurally valid")
            .canonical_manifest_digest_v2();
    assert_ne!(mutated, baseline);

    let (v1, mut tails) = fixture_evidence_v2();
    let replacement = fixture_digest_v2(0xc1, 17);
    tails.records[17].completion_digest = replacement;
    for receipt in &mut tails.records[17].tail_receipts {
        receipt.origin_binding_digest = replacement;
    }
    let mutated =
        RnsNativePublicPolynomialPublicationAssemblerV2::from_contract_evidence_v2(v1, tails)
            .assemble_v2(&mut FixtureDigestEngineV2)
            .expect("consistently rebound completion remains structurally valid")
            .canonical_manifest_digest_v2();
    assert_ne!(mutated, baseline);

    let (v1, mut tails) = fixture_evidence_v2();
    let replacement = fixture_digest_v2(0xc2, 0);
    tails.key_tail_owner_digest = replacement;
    for receipt in &mut tails.key_tail_receipts {
        receipt.origin_binding_digest = replacement;
    }
    let mutated =
        RnsNativePublicPolynomialPublicationAssemblerV2::from_contract_evidence_v2(v1, tails)
            .assemble_v2(&mut FixtureDigestEngineV2)
            .expect("consistently rebound key-tail owner remains structurally valid")
            .canonical_manifest_digest_v2();
    assert_ne!(mutated, baseline);

    let (v1, mut tails) = fixture_evidence_v2();
    let replacement = fixture_digest_v2(0xc3, 0);
    tails.lifecycle_digest = replacement;
    for record in &mut tails.records {
        record.lifecycle_digest = replacement;
    }
    let mutated =
        RnsNativePublicPolynomialPublicationAssemblerV2::from_contract_evidence_v2(v1, tails)
            .assemble_v2(&mut FixtureDigestEngineV2)
            .expect("consistently rebound lifecycle remains structurally valid")
            .canonical_manifest_digest_v2();
    assert_ne!(mutated, baseline);

    let (mut v1, mut tails) = fixture_evidence_v2();
    let replacement = fixture_digest_v2(0xc4, 0);
    v1.collective_key_provider.snapshot_identity = replacement;
    tails.collective_key_provider.snapshot_identity = replacement;
    for receipt in &mut v1.key_prefix_receipts {
        receipt.provider_binding.snapshot_identity = replacement;
    }
    for receipt in &mut tails.key_tail_receipts {
        receipt.provider_binding.snapshot_identity = replacement;
    }
    let mutated =
        RnsNativePublicPolynomialPublicationAssemblerV2::from_contract_evidence_v2(v1, tails)
            .assemble_v2(&mut FixtureDigestEngineV2)
            .expect("consistently rebound key snapshot remains structurally valid")
            .canonical_manifest_digest_v2();
    assert_ne!(mutated, baseline);

    let (mut v1, mut tails) = fixture_evidence_v2();
    let replacement = fixture_digest_v2(0xc5, 0);
    v1.ciphertext_provider.readback_identity = replacement;
    tails.ciphertext_provider.readback_identity = replacement;
    for manifest in &mut v1.ciphertext_manifests {
        manifest.provider_binding.readback_identity = replacement;
        for receipt in &mut manifest.prefix_receipts {
            receipt.provider_binding.readback_identity = replacement;
        }
    }
    for record in &mut tails.records {
        for receipt in &mut record.tail_receipts {
            receipt.provider_binding.readback_identity = replacement;
        }
    }
    let mutated =
        RnsNativePublicPolynomialPublicationAssemblerV2::from_contract_evidence_v2(v1, tails)
            .assemble_v2(&mut FixtureDigestEngineV2)
            .expect("consistently rebound ciphertext readback remains structurally valid")
            .canonical_manifest_digest_v2();
    assert_ne!(mutated, baseline);
}

#[test]
fn zero_hash_output_never_mints_a_manifest_v2() {
    let drops = Rc::new(Cell::new(0));
    let (v1, tails) = fixture_evidence_with_owner_drop_probes_v2(&drops);
    assert_eq!(
        RnsNativePublicPolynomialPublicationAssemblerV2::from_contract_evidence_v2(v1, tails)
            .assemble_v2(&mut ZeroDigestEngineV2)
            .err(),
        Some(
            RnsNativePublicationAssemblyErrorV2::DigestEngineReturnedZero {
                component: "composite_provider_identity"
            }
        )
    );
    assert_eq!(drops.get(), FIXTURE_CONSUMING_OWNER_COUNT_V2);

    for (zero_on_call, expected_component) in [
        (2, "canonical_manifest_digest"),
        (3, "reader_handoff_digest"),
    ] {
        let drops = Rc::new(Cell::new(0));
        let (v1, tails) = fixture_evidence_with_owner_drop_probes_v2(&drops);
        let mut digest_engine = ZeroOnDigestCallV2 {
            call_count: 0,
            zero_on_call,
        };
        let error =
            RnsNativePublicPolynomialPublicationAssemblerV2::from_contract_evidence_v2(v1, tails)
                .assemble_v2(&mut digest_engine)
                .err()
                .expect("zero digest output must fail closed");
        assert_eq!(
            error,
            RnsNativePublicationAssemblyErrorV2::DigestEngineReturnedZero {
                component: expected_component
            }
        );
        assert_eq!(drops.get(), FIXTURE_CONSUMING_OWNER_COUNT_V2);
    }
}

#[test]
fn digest_engine_unwind_destroys_every_consumed_owner_v2() {
    for panic_on_call in 1..=3 {
        let drops = Rc::new(Cell::new(0));
        let (v1, tails) = fixture_evidence_with_owner_drop_probes_v2(&drops);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let mut digest_engine = PanicOnDigestCallV2 {
                call_count: 0,
                panic_on_call,
            };
            let _ = RnsNativePublicPolynomialPublicationAssemblerV2::from_contract_evidence_v2(
                v1, tails,
            )
            .assemble_v2(&mut digest_engine);
        }));
        assert!(outcome.is_err(), "digest call {panic_on_call} must unwind");
        assert_eq!(
            drops.get(),
            FIXTURE_CONSUMING_OWNER_COUNT_V2,
            "digest call {panic_on_call} leaked a consumed owner"
        );
    }
}

struct FixtureProviderV2 {
    identity: RnsNativeCompositeProviderIdentityV2,
    drops: Rc<Cell<usize>>,
    panic_on_identity: bool,
}

impl sealed::CompositeReadProviderV2 for FixtureProviderV2 {}

impl Drop for FixtureProviderV2 {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

impl RnsNativeCompositeReadProviderV2 for FixtureProviderV2 {
    type Error = Infallible;

    fn composite_provider_identity_v2(&self) -> RnsNativeCompositeProviderIdentityV2 {
        assert!(!self.panic_on_identity, "fixture identity panic");
        self.identity
    }

    fn read_exact_at_v2(
        &mut self,
        _route: RnsNativeProviderRouteV2,
        _object_pointer_digest: RnsNativeContractDigestV2,
        _byte_offset: usize,
        destination: &mut [u8],
    ) -> Result<(), Self::Error> {
        destination.fill(0);
        Ok(())
    }
}

struct FixtureReaderV2 {
    provider: FixtureProviderV2,
}

struct AcceptingReaderAdapterV2 {
    calls: Rc<Cell<usize>>,
}

impl sealed::ExistingReaderAdapterV2 for AcceptingReaderAdapterV2 {}

impl RnsNativeExistingPublicReaderAdapterV2<FixtureProviderV2> for AcceptingReaderAdapterV2 {
    type Reader = FixtureReaderV2;
    type Error = Infallible;

    fn try_build_existing_reader_v2(
        self,
        provider: FixtureProviderV2,
        request: RnsNativeExistingReaderBuildRequestV2<'_>,
    ) -> Result<Self::Reader, (Self::Error, FixtureProviderV2)> {
        self.calls.set(self.calls.get() + 1);
        assert_eq!(
            request.descriptors.len(),
            RNS_NATIVE_CANONICAL_DESCRIPTOR_COUNT_V2
        );
        assert_eq!(
            request
                .receipt_locator
                .receipt_at_v2(0)
                .expect("first exact receipt")
                .position,
            request.descriptors[0].position
        );
        assert_eq!(
            request
                .receipt_locator
                .receipt_at_v2(RNS_NATIVE_CANONICAL_DESCRIPTOR_COUNT_V2 - 1)
                .expect("last exact receipt")
                .position,
            request.descriptors[RNS_NATIVE_CANONICAL_DESCRIPTOR_COUNT_V2 - 1].position
        );
        assert_eq!(request.composite_provider, provider.identity);
        assert!(!request.canonical_manifest_digest.is_zero_v2());
        assert!(!request.reader_handoff_digest.is_zero_v2());
        Ok(FixtureReaderV2 { provider })
    }
}

struct RejectingReaderAdapterV2;

impl sealed::ExistingReaderAdapterV2 for RejectingReaderAdapterV2 {}

impl RnsNativeExistingPublicReaderAdapterV2<FixtureProviderV2> for RejectingReaderAdapterV2 {
    type Reader = FixtureReaderV2;
    type Error = &'static str;

    fn try_build_existing_reader_v2(
        self,
        provider: FixtureProviderV2,
        _request: RnsNativeExistingReaderBuildRequestV2<'_>,
    ) -> Result<Self::Reader, (Self::Error, FixtureProviderV2)> {
        Err(("fixture rejection", provider))
    }
}

struct PanickingReaderAdapterV2;

impl sealed::ExistingReaderAdapterV2 for PanickingReaderAdapterV2 {}

impl RnsNativeExistingPublicReaderAdapterV2<FixtureProviderV2> for PanickingReaderAdapterV2 {
    type Reader = FixtureReaderV2;
    type Error = Infallible;

    fn try_build_existing_reader_v2(
        self,
        _provider: FixtureProviderV2,
        _request: RnsNativeExistingReaderBuildRequestV2<'_>,
    ) -> Result<Self::Reader, (Self::Error, FixtureProviderV2)> {
        panic!("fixture reader construction panic")
    }
}

#[test]
fn handoff_and_integrated_reader_capability_are_one_shot_and_move_only_v2() {
    assert!(core::mem::needs_drop::<
        RnsNativePublicPolynomialPublicationAssemblerV2,
    >());
    assert!(core::mem::needs_drop::<
        RnsNativePublicPolynomialPublishedSetV2,
    >());
    assert!(core::mem::needs_drop::<
        RnsNativePublicPolynomialReaderHandoffV2<FixtureProviderV2>,
    >());
    assert!(core::mem::needs_drop::<
        RnsNativeIntegratedPublicReaderCapabilityV2<FixtureReaderV2>,
    >());

    let published = assemble_fixture_v2();
    let identity = published.composite_provider;
    let drops = Rc::new(Cell::new(0));
    let calls = Rc::new(Cell::new(0));
    let handoff = published
        .into_reader_handoff_v2(FixtureProviderV2 {
            identity,
            drops: Rc::clone(&drops),
            panic_on_identity: false,
        })
        .unwrap_or_else(|_| panic!("matching provider must be accepted"));
    let capability = handoff
        .try_into_existing_reader_v2(AcceptingReaderAdapterV2 {
            calls: Rc::clone(&calls),
        })
        .unwrap_or_else(|_| panic!("fixture reader must be built"));
    assert_eq!(calls.get(), 1);
    assert!(!capability.canonical_manifest_digest_v2().is_zero_v2());
    assert_eq!(drops.get(), 0);
    drop(capability);
    assert_eq!(drops.get(), 1);
}

#[test]
fn provider_and_reader_errors_destroy_authority_without_a_capability_v2() {
    let owner_drops = Rc::new(Cell::new(0));
    let published = assemble_fixture_with_owner_drop_probes_v2(&owner_drops);
    let mut wrong_identity = published.composite_provider;
    wrong_identity.composite_identity = fixture_digest_v2(0xb0, 0);
    let provider_drops = Rc::new(Cell::new(0));
    let failure = published
        .into_reader_handoff_v2(FixtureProviderV2 {
            identity: wrong_identity,
            drops: Rc::clone(&provider_drops),
            panic_on_identity: false,
        })
        .err()
        .expect("mismatched provider must fail");
    assert_eq!(
        failure.error_v2(),
        RnsNativePublicationAssemblyErrorV2::HandoffProviderMismatch
    );
    assert_eq!(owner_drops.get(), 0);
    assert_eq!(provider_drops.get(), 0);
    drop(failure);
    assert_eq!(owner_drops.get(), FIXTURE_CONSUMING_OWNER_COUNT_V2);
    assert_eq!(provider_drops.get(), 1);

    let owner_drops = Rc::new(Cell::new(0));
    let published = assemble_fixture_with_owner_drop_probes_v2(&owner_drops);
    let identity = published.composite_provider;
    let provider_drops = Rc::new(Cell::new(0));
    let handoff = published
        .into_reader_handoff_v2(FixtureProviderV2 {
            identity,
            drops: Rc::clone(&provider_drops),
            panic_on_identity: false,
        })
        .unwrap_or_else(|_| panic!("matching provider"));
    let failure = handoff
        .try_into_existing_reader_v2(RejectingReaderAdapterV2)
        .err()
        .expect("reader rejection must remain fail closed");
    assert_eq!(failure.error, "fixture rejection");
    assert_eq!(owner_drops.get(), 0);
    assert_eq!(provider_drops.get(), 0);
    drop(failure);
    assert_eq!(owner_drops.get(), FIXTURE_CONSUMING_OWNER_COUNT_V2);
    assert_eq!(provider_drops.get(), 1);
}

#[test]
fn unwind_drops_provider_and_never_returns_a_capability_v2() {
    let owner_drops = Rc::new(Cell::new(0));
    let published = assemble_fixture_with_owner_drop_probes_v2(&owner_drops);
    let identity = published.composite_provider;
    let provider_drops = Rc::new(Cell::new(0));
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let _ = published.into_reader_handoff_v2(FixtureProviderV2 {
            identity,
            drops: Rc::clone(&provider_drops),
            panic_on_identity: true,
        });
    }));
    assert!(outcome.is_err());
    assert_eq!(owner_drops.get(), FIXTURE_CONSUMING_OWNER_COUNT_V2);
    assert_eq!(provider_drops.get(), 1);

    let owner_drops = Rc::new(Cell::new(0));
    let published = assemble_fixture_with_owner_drop_probes_v2(&owner_drops);
    let identity = published.composite_provider;
    let provider_drops = Rc::new(Cell::new(0));
    let handoff = published
        .into_reader_handoff_v2(FixtureProviderV2 {
            identity,
            drops: Rc::clone(&provider_drops),
            panic_on_identity: false,
        })
        .unwrap_or_else(|_| panic!("matching provider"));
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let _ = handoff.try_into_existing_reader_v2(PanickingReaderAdapterV2);
    }));
    assert!(outcome.is_err());
    assert_eq!(owner_drops.get(), FIXTURE_CONSUMING_OWNER_COUNT_V2);
    assert_eq!(provider_drops.get(), 1);
}

#[test]
fn parent_declares_exactly_one_private_path_child_v2() {
    let parent_source = include_str!("incremental_source.rs");
    let path_attribute = "#[path = \"incremental_source_rns_native_publication_assembler_v2.rs\"]";
    let private_declaration = "mod incremental_source_rns_native_publication_assembler_v2;";

    assert_eq!(
        parent_source
            .lines()
            .filter(|line| line.trim() == path_attribute)
            .count(),
        1
    );
    assert_eq!(
        parent_source
            .lines()
            .filter(|line| line.trim() == private_declaration)
            .count(),
        1
    );
    assert!(!parent_source.lines().any(|line| {
        let line = line.trim();
        line.contains("incremental_source_rns_native_publication_assembler_v2")
            && (line.starts_with("pub mod ")
                || line.starts_with("pub(")
                || line.starts_with("pub "))
    }));
}

#[test]
fn integration_readiness_release_and_adapters_remain_closed_v2() {
    const {
        assert!(RNS_NATIVE_PUBLICATION_ASSEMBLER_CONTRACT_IMPLEMENTED_V2);
        assert!(!RNS_NATIVE_PUBLICATION_ASSEMBLER_LIVE_OWNER_INTEGRATED_V2);
        assert!(!RNS_NATIVE_PUBLICATION_ASSEMBLER_PRODUCTION_ADAPTER_AVAILABLE_V2);
        assert!(!RNS_NATIVE_PUBLICATION_ASSEMBLER_READER_INTEGRATED_V2);
        assert!(!RNS_NATIVE_PUBLICATION_ASSEMBLER_READINESS_V2);
        assert!(!RNS_NATIVE_PUBLICATION_ASSEMBLER_RELEASE_AUTHORIZED_V2);
    }
    let codes: BTreeSet<_> = RNS_NATIVE_PUBLICATION_ASSEMBLER_BLOCKERS_V2
        .iter()
        .map(|blocker| blocker.code)
        .collect();
    assert_eq!(
        codes.len(),
        RNS_NATIVE_PUBLICATION_ASSEMBLER_BLOCKERS_V2.len()
    );
    assert_eq!(codes.len(), 6);
    assert!(!codes.contains("DECLARE_PRIVATE_CHILDREN"));
    assert!(!codes.contains("DECLARE_PRIVATE_BASIS_EXTENSION"));
    let reader_blocker = RNS_NATIVE_PUBLICATION_ASSEMBLER_BLOCKERS_V2
        .iter()
        .find(|blocker| blocker.code == "EXISTING_READER_ADAPTER")
        .expect("existing private reader adapter remains an explicit blocker");
    assert!(
        reader_blocker
            .required_delta
            .contains("no reader visibility or public/untyped constructor change is required")
    );
}
