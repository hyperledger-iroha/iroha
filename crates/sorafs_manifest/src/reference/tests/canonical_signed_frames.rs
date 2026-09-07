// Exact outer-frame admission for native signed-artifact reference validators.
fn reference_frame_layouts() -> [u8; 10] {
    use norito::core::header_flags::{COMPACT_LEN, FIELD_BITSET, PACKED_SEQ, PACKED_STRUCT};
    [
        0,
        COMPACT_LEN,
        PACKED_SEQ,
        PACKED_SEQ | COMPACT_LEN,
        PACKED_STRUCT,
        PACKED_STRUCT | COMPACT_LEN,
        PACKED_STRUCT | COMPACT_LEN | FIELD_BITSET,
        PACKED_SEQ | PACKED_STRUCT,
        PACKED_SEQ | PACKED_STRUCT | COMPACT_LEN,
        PACKED_SEQ | PACKED_STRUCT | COMPACT_LEN | FIELD_BITSET,
    ]
}

fn assert_reference_frame_boundary<T>(
    decode_code: &str,
    value: &T,
    validate: impl Fn(&[u8]) -> ValidationOutcomeV1,
) where
    T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
    T: PartialEq + std::fmt::Debug,
{
    let canonical = norito::encode_canonical(value).expect("independent canonical frame");
    let expected = validate(&canonical);
    assert_success(&expected);
    let assert_decode_failure = |outcome: &Outcome, canonical_error: bool| {
        assert_failure(outcome, decode_code, CATEGORY_NORITO);
        let message = if decode_code == "SFS-BND-001" {
            assert_context(outcome, "payload_code", "SFS-NORITO-001");
            field(&outcome.context, "payload_message")
                .expect("bundle preserves nested decode error")
        } else {
            outcome.message.as_str()
        };
        if canonical_error {
            assert!(message.contains("non-canonical encoding"), "{outcome:?}");
        }
        assert_eq!(outcome.inputs, expected.inputs);
        assert_eq!(outcome.generated_at, expected.generated_at);
    };
    // The default library has no compression codec. The explicit
    // `--features norito/compression` lane also qualifies genuine Zstd frames.
    let compressed =
        match norito::to_compressed_bytes(value, Some(norito::CompressionConfig::default())) {
            Ok(bytes) => {
                assert_eq!(norito::decode_from_bytes::<T>(&bytes).unwrap(), *value);
                Some(bytes)
            }
            Err(norito::Error::Io(error))
                if error.kind() == std::io::ErrorKind::Other
                    && error.to_string() == "compression support disabled" =>
            {
                None
            }
            Err(error) => panic!("failed to encode valid compressed fixture: {error}"),
        };
    let tagged = crate::canonical_test_support::with_compression_tag(value);
    let header = norito::core::Header::read(tagged.as_slice()).unwrap();
    let length_offset = header.magic.len() + 2 + header.schema.len() + 1;
    let mut oversized_header = tagged[..norito::core::Header::SIZE].to_vec();
    oversized_header[length_offset..length_offset + 8].copy_from_slice(&u64::MAX.to_le_bytes());
    let oversized = norito::core::Header::read(oversized_header.as_slice()).unwrap();
    assert_eq!(oversized.compression, norito::Compression::Zstd);
    assert_eq!(oversized.length, u64::MAX);
    let mut trailing = canonical.clone();
    trailing.push(0);
    let no_allocation = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 128);
    let mut alternate_frames = 0;
    for flags in reference_frame_layouts() {
        let _layout = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(validate(&canonical), expected, "layout {flags:#04x}");
        assert_eq!(decode_reference_frame::<T>(&canonical).unwrap(), *value);
        for bytes in [&tagged, &oversized_header]
            .into_iter()
            .chain(compressed.iter())
        {
            let outcome = validate(bytes);
            assert_decode_failure(&outcome, true);
            // Exercise the shared owner directly: a preceding valid frame in a
            // two-input validator must not mask the candidate's allocation order.
            assert!(matches!(
                norito::with_decode_limits_scope(no_allocation, || decode_reference_frame::<T>(
                    bytes
                )),
                Err(norito::Error::NonCanonicalEncoding)
            ));
        }
        // The outer scope still applies to valid frames and the general decoder.
        // These controls would fail if the canonical boundary erased resource limits.
        for bytes in std::iter::once(&canonical).chain(compressed.iter()) {
            assert!(matches!(
                norito::with_decode_limits_scope(no_allocation, || norito::decode_from_bytes::<T>(
                    bytes
                )),
                Err(norito::Error::TotalAllocationExceeded { limit: 0, .. })
            ));
        }
        assert!(matches!(
            norito::with_decode_limits_scope(no_allocation, || decode_reference_frame::<T>(
                &canonical
            )),
            Err(norito::Error::TotalAllocationExceeded { limit: 0, .. })
        ));
        assert_decode_failure(&validate(&trailing), false);
        let alternate = norito::core::to_bytes(value).expect("valid advertised alternate layout");
        if alternate != canonical {
            alternate_frames += 1;
            assert_eq!(norito::decode_from_bytes::<T>(&alternate).unwrap(), *value);
            let outcome = validate(&alternate);
            assert_decode_failure(&outcome, true);
        }
        assert_eq!(norito::core::get_decode_flags(), flags);
    }
    assert!(
        alternate_frames > 0,
        "fixture must distinguish actual supported layouts"
    );
}

#[test]
fn reference_signed_governance_outer_frames_are_canonical() {
    let (blocks, head) = signed_governance_dag_chain();
    let node = &blocks[0].node;
    assert_reference_frame_boundary("SFS-NORITO-001", node, |bytes| {
        validate_governance_log_node_bytes(bytes, "node.to", Some(&node.node_cid), 51)
    });
    assert_reference_frame_boundary("SFS-NORITO-001", &blocks[0], |bytes| {
        validate_governance_dag_block_bytes(bytes, "block.to", Some(&blocks[0].block_cid), 52)
    });
    let head_bytes = norito::encode_canonical(&head).unwrap();
    let block_bytes = blocks
        .iter()
        .map(|block| norito::encode_canonical(block).unwrap())
        .collect::<Vec<_>>();
    let block_inputs = block_bytes
        .iter()
        .enumerate()
        .map(|(index, bytes)| (bytes.as_slice(), format!("block-{index}.to")))
        .collect::<Vec<_>>();
    assert_reference_frame_boundary("SFS-NORITO-001", &head, |bytes| {
        validate_governance_dag_head_chain_bytes(bytes, "head.to", &block_inputs, 53)
    });
    for (candidate_index, block) in blocks.iter().enumerate() {
        assert_reference_frame_boundary("SFS-NORITO-001", block, |candidate| {
            let inputs = block_bytes
                .iter()
                .enumerate()
                .map(|(index, bytes)| {
                    (
                        if index == candidate_index {
                            candidate
                        } else {
                            bytes.as_slice()
                        },
                        format!("block-{index}.to"),
                    )
                })
                .collect::<Vec<_>>();
            validate_governance_dag_head_chain_bytes(&head_bytes, "head.to", &inputs, 54)
        });
    }
    let mut invalid_signature = blocks[0].clone();
    invalid_signature.block_signature.signature[0] ^= 1;
    let outcome = validate_governance_dag_block_bytes(
        &norito::encode_canonical(&invalid_signature).unwrap(),
        "block.to",
        None,
        55,
    );
    assert_failure(&outcome, "SFS-SIG-006", CATEGORY_SIGNATURE);
}

#[test]
fn reference_replication_outer_frames_are_canonical() {
    assert_reference_frame_boundary("SFS-NORITO-001", &replication_order(), |bytes| {
        validate_replication_order_bytes(bytes, "order.to", 61)
    });
    let signed = signed_replication_order();
    assert_reference_frame_boundary("SFS-NORITO-001", &signed, |bytes| {
        validate_signed_replication_order_bytes(bytes, "signed-order.to", 62)
    });
    let mut tampered = signed;
    tampered.order.deadline_at += 1;
    let outcome = validate_signed_replication_order_bytes(
        &norito::encode_canonical(&tampered).unwrap(),
        "signed-order.to",
        63,
    );
    assert_failure(&outcome, "SFS-SIG-006", CATEGORY_SIGNATURE);
}

#[test]
fn reference_admission_outer_frames_are_canonical() {
    let envelope = admission_envelope();
    let envelope_bytes = norito::encode_canonical(&envelope).unwrap();
    let renewal: ProviderAdmissionRenewalV1 =
        norito::decode_canonical(&admission_renewal_bytes()).unwrap();
    let renewal_bytes = norito::encode_canonical(&renewal).unwrap();
    let revocation: ProviderAdmissionRevocationV1 =
        norito::decode_canonical(&admission_revocation_bytes()).unwrap();
    let revocation_bytes = norito::encode_canonical(&revocation).unwrap();
    assert_reference_frame_boundary("SFS-NORITO-001", &envelope, |bytes| {
        validate_provider_admission_envelope_bytes(bytes, "envelope.to", 71)
    });
    assert_reference_frame_boundary("SFS-NORITO-001", &envelope, |bytes| {
        validate_provider_admission_renewal_bytes(
            bytes,
            &renewal_bytes,
            "envelope.to",
            "renewal.to",
            72,
        )
    });
    assert_reference_frame_boundary("SFS-NORITO-001", &renewal, |bytes| {
        validate_provider_admission_renewal_bytes(
            &envelope_bytes,
            bytes,
            "envelope.to",
            "renewal.to",
            72,
        )
    });
    assert_reference_frame_boundary("SFS-NORITO-001", &envelope, |bytes| {
        validate_provider_admission_revocation_bytes(
            bytes,
            &revocation_bytes,
            "envelope.to",
            "revocation.to",
            73,
        )
    });
    assert_reference_frame_boundary("SFS-NORITO-001", &revocation, |bytes| {
        validate_provider_admission_revocation_bytes(
            &envelope_bytes,
            bytes,
            "envelope.to",
            "revocation.to",
            73,
        )
    });
}

#[test]
fn reference_potr_outer_frame_is_canonical() {
    let receipt = potr_receipt();
    assert_reference_frame_boundary("SFS-NORITO-001", &receipt, |bytes| {
        validate_potr_receipt_bytes(bytes, "receipt.to", Some(ProofStreamTier::Hot), 81)
    });
}

#[test]
fn reference_fixture_bundle_outer_frames_are_canonical() {
    let advert = signed_advert(1_700_000_000);
    let order = replication_order();
    let advert_bytes = norito::encode_canonical(&advert).unwrap();
    let order_bytes = norito::encode_canonical(&order).unwrap();
    assert_reference_frame_boundary("SFS-BND-001", &advert, |bytes| {
        validate_fixture_bundle_payloads(
            &[
                FixtureBundlePayloadV1::new(BundleKind::ProviderAdvert, "advert.to", bytes),
                FixtureBundlePayloadV1::new(BundleKind::ReplicationOrder, "order.to", &order_bytes),
            ],
            1_700_000_001,
            91,
        )
    });
    assert_reference_frame_boundary("SFS-BND-001", &order, |bytes| {
        validate_fixture_bundle_payloads(
            &[
                FixtureBundlePayloadV1::new(BundleKind::ProviderAdvert, "advert.to", &advert_bytes),
                FixtureBundlePayloadV1::new(BundleKind::ReplicationOrder, "order.to", bytes),
            ],
            1_700_000_001,
            92,
        )
    });
}
