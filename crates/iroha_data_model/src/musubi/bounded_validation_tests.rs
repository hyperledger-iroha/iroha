//! Exact-byte and rejection controls for borrowed Musubi validation.

use super::*;

// This is a canonical encoding oracle used only by tests, not a second
// production digest path or a retired decoder.
#[derive(Encode)]
struct CanonicalAttestationSetFixture {
    archive_id: ArchiveId,
    replication_order: ReplicationOrderId,
    references: Vec<MusubiProviderBundleAttestationRefV1>,
}

fn attestation_references(count: usize) -> Vec<MusubiProviderBundleAttestationRefV1> {
    (1..=count)
        .map(|index| {
            let index = u8::try_from(index).expect("test provider count fits a byte");
            MusubiProviderBundleAttestationRefV1 {
                provider_id: ProviderId::new([index; 32]),
                digest: MusubiProviderBundleAttestationDigestV1::new([index; 32]),
            }
        })
        .collect()
}

#[test]
fn borrowed_attestation_set_matches_canonical_bytes_at_every_admitted_count() {
    assert_eq!(
        <MusubiProviderBundleAttestationSetPreimageV1<'_> as norito::NoritoSchema>::nominal_name(),
        "iroha_data_model::musubi::MusubiProviderBundleAttestationSetPreimageV1",
    );
    let archive_id = ArchiveId::new([0xa1; 32]);
    let replication_order = ReplicationOrderId::new([0xb2; 32]);
    for count in 1..=MUSUBI_MAX_LOCATION_PROVIDERS_V1 {
        let fixture = CanonicalAttestationSetFixture {
            archive_id,
            replication_order,
            references: attestation_references(count),
        };
        let preimage = MusubiProviderBundleAttestationSetPreimageV1 {
            archive_id,
            replication_order,
            references: &fixture.references,
        };
        let expected_bytes = fixture.encode();
        assert_eq!(preimage.encode(), expected_bytes, "provider count {count}");
        assert_eq!(
            musubi_provider_bundle_attestation_set_digest_v1(
                archive_id,
                replication_order,
                &fixture.references,
            )
            .expect("canonical provider set"),
            MusubiProviderBundleAttestationSetDigestV1(domain_hash(
                MUSUBI_PROVIDER_BUNDLE_ATTESTATION_SET_DIGEST_DOMAIN_V1,
                &expected_bytes,
            )),
            "provider count {count}",
        );
    }
}

#[test]
fn borrowed_attestation_set_preserves_bounds_ordering_and_duplicate_errors() {
    const SET_ERROR: &str = "Musubi provider bundle attestation set is invalid or noncanonical";
    const REFERENCE_ERROR: &str = "Musubi provider bundle attestation reference is invalid";
    let archive_id = ArchiveId::new([0xa1; 32]);
    let order = ReplicationOrderId::new([0xb2; 32]);
    let valid = attestation_references(2);
    let mut reversed = valid.clone();
    reversed.reverse();
    for references in [
        Vec::new(),
        attestation_references(MUSUBI_MAX_LOCATION_PROVIDERS_V1 + 1),
        reversed,
        vec![valid[0], valid[0]],
        vec![
            valid[0],
            MusubiProviderBundleAttestationRefV1 {
                digest: valid[1].digest,
                ..valid[0]
            },
        ],
    ] {
        assert_eq!(
            musubi_provider_bundle_attestation_set_digest_v1(archive_id, order, &references)
                .expect_err("invalid provider set")
                .reason(),
            SET_ERROR,
        );
    }
    for (archive, order) in [
        (ArchiveId::new([0; 32]), order),
        (archive_id, ReplicationOrderId::new([0; 32])),
    ] {
        assert_eq!(
            musubi_provider_bundle_attestation_set_digest_v1(archive, order, &valid)
                .expect_err("invalid set identity")
                .reason(),
            SET_ERROR,
        );
    }
    for reference in [
        MusubiProviderBundleAttestationRefV1 {
            provider_id: ProviderId::new([0; 32]),
            ..valid[0]
        },
        MusubiProviderBundleAttestationRefV1 {
            digest: MusubiProviderBundleAttestationDigestV1::new([0; 32]),
            ..valid[0]
        },
    ] {
        assert_eq!(
            musubi_provider_bundle_attestation_set_digest_v1(archive_id, order, &[reference])
                .expect_err("invalid provider reference")
                .reason(),
            REFERENCE_ERROR,
        );
    }
}

fn archive_commitment_with_chunker(chunker: ChunkerProfileHandle) -> MusubiArchiveCommitmentV1 {
    MusubiArchiveCommitmentV1 {
        root_cid: ManifestRootCid::from_blake3_digest([1; 32]).expect("root CID"),
        chunker,
        chunk_plan_digest: MusubiContentDigestV1::new([2; 32]),
        por_root: MusubiContentDigestV1::new([3; 32]),
        content_length: 1_024,
        car_digest: MusubiContentDigestV1::new([4; 32]),
        car_size: 2_048,
        bundle_digest: MusubiContentDigestV1::new([5; 32]),
        source_tree_digest: MusubiContentDigestV1::new([6; 32]),
        descriptor_digest: MusubiContentDigestV1::new([7; 32]),
        file_count: 2,
        chunk_count: 4,
    }
}

#[test]
fn archive_chunker_bound_counts_utf8_bytes_and_both_separators_without_formatting() {
    for (namespace, name, semver) in [
        (String::new(), String::new(), String::new()),
        ("n".repeat(124), "p".into(), String::new()),
        ("n".repeat(125), "p".into(), String::new()),
        ("n".repeat(126), "p".into(), String::new()),
        ("é".repeat(62), "p".into(), "v".into()),
        ("é".repeat(63), "p".into(), "v".into()),
        ("n".into(), "p".repeat(125), String::new()),
        ("n".into(), String::new(), "v".repeat(125)),
        ("n".into(), String::new(), "v".repeat(126)),
    ] {
        let commitment = archive_commitment_with_chunker(ChunkerProfileHandle {
            profile_id: 1,
            namespace,
            name,
            semver,
            multihash_code: 0x1f,
        });
        let expected_len = commitment.chunker.to_handle().len();
        let result = commitment.validate();
        assert_eq!(result.is_ok(), expected_len <= 128, "length {expected_len}");
        if let Err(error) = result {
            assert_eq!(
                error.reason(),
                "Musubi archive contains an invalid or inert commitment",
            );
        }
    }
}

fn comparator(op: MusubiComparatorOpV1, major: u64) -> MusubiVersionComparatorV1 {
    MusubiVersionComparatorV1 {
        op,
        version: MusubiVersionV1 {
            major,
            minor: 0,
            patch: 0,
            prerelease: Vec::new(),
        },
    }
}

#[test]
fn requirement_exact_scan_preserves_unique_and_conflicting_exact_versions() {
    use MusubiComparatorOpV1::{Equal, Greater, Less};
    for exact_count in 0..=3 {
        let mut comparisons = vec![comparator(Greater, 0), comparator(Less, 4)];
        comparisons.extend((1..=exact_count).map(|major| comparator(Equal, major)));
        comparisons.sort();
        let result = MusubiVersionReqV1::Comparators(comparisons).validate();
        if exact_count <= 1 {
            result.expect("zero or one exact version is consistent");
        } else {
            assert_eq!(
                result
                    .expect_err("distinct exact versions conflict")
                    .reason(),
                "Musubi comparator list contains contradictory exact versions",
            );
        }
    }
    // The comparison retains prerelease identity, rather than comparing only
    // major/minor/patch or formatted versions.
    let mut distinct_prereleases = vec![
        MusubiVersionComparatorV1 {
            op: Equal,
            version: "1.0.0-alpha".parse().expect("alpha version"),
        },
        MusubiVersionComparatorV1 {
            op: Equal,
            version: "1.0.0-beta".parse().expect("beta version"),
        },
    ];
    distinct_prereleases.sort();
    assert_eq!(
        MusubiVersionReqV1::Comparators(distinct_prereleases)
            .validate()
            .expect_err("prerelease exact versions conflict")
            .reason(),
        "Musubi comparator list contains contradictory exact versions",
    );
}

#[test]
fn requirement_exact_scan_preserves_comparator_bounds_ordering_and_duplicate_errors() {
    use MusubiComparatorOpV1::{Equal, Greater};
    const CANONICAL_ERROR: &str = "Musubi comparator list is empty, oversized, or noncanonical";
    let canonical = (0..MUSUBI_MAX_VERSION_COMPARATORS_V1)
        .map(|major| comparator(Greater, u64::try_from(major).expect("small major")))
        .collect::<Vec<_>>();
    MusubiVersionReqV1::Comparators(canonical.clone())
        .validate()
        .expect("maximum comparator count is admitted");
    let mut too_many = canonical.clone();
    too_many.push(comparator(Greater, 99));
    let mut unsorted = canonical;
    unsorted.reverse();
    for comparisons in [
        Vec::new(),
        vec![comparator(Equal, 1)],
        vec![comparator(Equal, 1), comparator(Equal, 1)],
        unsorted,
        too_many,
    ] {
        assert_eq!(
            MusubiVersionReqV1::Comparators(comparisons)
                .validate()
                .expect_err("noncanonical comparators")
                .reason(),
            CANONICAL_ERROR,
        );
    }
}

#[test]
fn borrowed_attestation_set_uses_canonical_flags_inside_another_layout_context() {
    let references = attestation_references(MUSUBI_MAX_LOCATION_PROVIDERS_V1);
    let archive_id = ArchiveId::new([0xa1; 32]);
    let replication_order = ReplicationOrderId::new([0xb2; 32]);
    let expected = musubi_provider_bundle_attestation_set_digest_v1(
        archive_id,
        replication_order,
        &references,
    )
    .expect("canonical digest");
    let _ambient = norito::core::DecodeFlagsGuard::enter(
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN,
    );
    assert_eq!(
        musubi_provider_bundle_attestation_set_digest_v1(
            archive_id,
            replication_order,
            &references,
        )
        .expect("digest sets its canonical flags"),
        expected,
    );
}

#[test]
fn requirement_exact_scan_validates_nested_versions_before_comparing_them() {
    let mut invalid = comparator(MusubiComparatorOpV1::Equal, 1);
    invalid.version.prerelease = vec![MusubiPrereleaseIdentifierV1::AlphaNumeric("01".to_owned())];
    let expected = invalid.version.validate().expect_err("invalid prerelease");
    let mut comparisons = vec![invalid, comparator(MusubiComparatorOpV1::Equal, 2)];
    comparisons.sort();
    assert_eq!(
        MusubiVersionReqV1::Comparators(comparisons)
            .validate()
            .expect_err("invalid version precedes contradictory exacts")
            .reason(),
        expected.reason(),
    );
}

// A serializer is deliberately invalid only in this test. Public Musubi
// signing inputs continue to use their sole canonical derived serializers.
struct FallibleSigningPayload {
    calls: std::cell::Cell<usize>,
    fail_at: usize,
    grow_at: usize,
}
impl norito::core::SerializePayload for FallibleSigningPayload {
    fn serialize(&self, encoder: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        let call = self.calls.get() + 1;
        self.calls.set(call);
        if call == self.fail_at {
            return Err(norito::Error::LengthMismatch);
        }
        std::io::Write::write_all(encoder, &[1])?;
        if call == self.grow_at {
            std::io::Write::write_all(encoder, &[2])?;
        }
        Ok(())
    }
}

#[test]
fn signing_hash_rejects_either_codec_pass_and_changed_lengths_without_partial_digest() {
    for (fail_at, grow_at, expected_calls, expected) in [
        (
            1,
            0,
            1,
            "Musubi signing hash has no canonical Norito encoding",
        ),
        (
            2,
            0,
            2,
            "Musubi signing hash has no canonical Norito encoding",
        ),
        (0, 2, 2, "Musubi signing-hash length changed between passes"),
    ] {
        let source = FallibleSigningPayload {
            calls: std::cell::Cell::new(0),
            fail_at,
            grow_at,
        };
        let error = try_domain_signing_hash(b"musubi.fallible.test", &source).unwrap_err();
        assert_eq!(error.reason(), expected);
        assert_eq!(source.calls.get(), expected_calls);
        assert!(!std::mem::needs_drop::<ParseError>());
    }
}

#[test]
fn replication_order_binding_streamed_length_matches_original_canonical_wire_and_rejections() {
    let commitment = archive_commitment_with_chunker(ChunkerProfileHandle {
        profile_id: 1,
        namespace: "sorafs".into(),
        name: "sf1".into(),
        semver: "1.0.0".into(),
        multihash_code: 0x1f,
    });
    let binding = MusubiReplicationOrderArchiveBindingV1::new(
        ReplicationOrderId::new([0x71; 32]),
        commitment.archive_id(),
        commitment,
    );
    assert_eq!(
        canonical_frame_len(&binding).unwrap(),
        binding.encode().len()
    );
    assert!(
        binding.encode().len() <= MUSUBI_MAX_REPLICATION_ORDER_ARCHIVE_BINDING_CANONICAL_BYTES_V1
    );
    binding.validate().unwrap();
    let encoded = binding.encode();
    let decoded = MusubiReplicationOrderArchiveBindingV1::decode(&mut encoded.as_slice()).unwrap();
    assert_eq!(decoded, binding);
    for mutate in 0..3 {
        let mut bad = binding.clone();
        match mutate {
            0 => bad.replication_order = ReplicationOrderId::new([0; 32]),
            1 => bad.archive_id = ArchiveId::new([0x72; 32]),
            _ => bad.commitment.content_length = 0,
        }
        assert!(bad.validate().is_err(), "mutation {mutate}");
    }
    // Wire measurement is explicit canonical encoding even under caller decode flags.
    let _ambient = norito::core::DecodeFlagsGuard::enter(0);
    assert_eq!(canonical_frame_len(&binding).unwrap(), encoded.len());
    binding.validate().unwrap();
}
