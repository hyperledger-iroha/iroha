// Canonical quarantine frames bind one immutable object and its actual AEAD context.

#[test]
fn quarantine_envelope_canonical_aead_and_rewrap_ignore_caller_layout() {
    let original = test_key_wrapper(0x74, "software://sorafs/moderation/key-v1");
    let replacement = test_key_wrapper(0x75, "software://sorafs/moderation/key-v2");
    let binding = test_key_provider_binding();
    let payload = vec![0xC3; 70_000];
    for seal_flags in
        (0..=u8::MAX).filter(|flags| norito::core::validate_header_flags(*flags).is_ok())
    {
        let _seal_layout = norito::core::DecodeFlagsGuard::enter(seal_flags);
        let (record, bytes) = seal_moderation_quarantine_object(
            ModerationQuarantineObjectInput {
                quarantine_id: [0x63; 16],
                payload: payload.clone(),
                captured_at_unix: 1_800_000_503,
                content_type: None,
                notes: None,
            },
            &binding,
            &original,
        )
        .expect("seal a real authenticated object under every caller layout");
        let envelope = decode_moderation_quarantine_object_envelope(&bytes, 8 * 1024 * 1024)
            .expect("recover canonical envelope");
        assert_eq!(norito::encode_canonical(&envelope).unwrap(), bytes);
        let expected_id = record.object_id;
        let expected_wrap_context = moderation_quarantine_wrap_context_digest(
            &quarantine_aad_header_from_envelope(&envelope).unwrap(),
        )
        .unwrap();
        let mut expected_replacement = None;
        for read_flags in
            (0..=u8::MAX).filter(|flags| norito::core::validate_header_flags(*flags).is_ok())
        {
            let _read_layout = norito::core::DecodeFlagsGuard::enter(read_flags);
            let restored =
                decode_moderation_quarantine_object_envelope(&bytes, 8 * 1024 * 1024).unwrap();
            assert_eq!(restored, envelope);
            assert_eq!(
                moderation_quarantine_object_id(
                    &quarantine_immutable_metadata_from_envelope(&restored).unwrap()
                )
                .unwrap(),
                expected_id
            );
            assert_eq!(
                moderation_quarantine_wrap_context_digest(
                    &quarantine_aad_header_from_envelope(&restored).unwrap()
                )
                .unwrap(),
                expected_wrap_context
            );
            assert_eq!(
                open_moderation_quarantine_object(&restored, &record, &binding, &original).unwrap(),
                payload
            );
            assert_eq!(
                open_moderation_quarantine_object_range(
                    &restored,
                    &record,
                    &binding,
                    &original,
                    65_000..66_000
                )
                .unwrap(),
                payload[65_000..66_000]
            );
            let (rebound_record, rebound_bytes) = rewrap_moderation_quarantine_object(
                &restored,
                &record,
                &binding,
                &original,
                &binding,
                &replacement,
            )
            .expect("authenticate original ciphertext and rewrap the exact DEK");
            assert_eq!(rebound_record, record);
            if let Some(expected) = expected_replacement.as_ref() {
                assert_eq!(&rebound_bytes, expected);
            } else {
                expected_replacement = Some(rebound_bytes.clone());
            }
            let rebound =
                decode_moderation_quarantine_object_envelope(&rebound_bytes, 8 * 1024 * 1024)
                    .unwrap();
            assert_eq!(rebound.object_id, restored.object_id);
            assert_eq!(rebound.chunks, restored.chunks);
            assert_eq!(rebound.ciphertext_digest, restored.ciphertext_digest);
            assert_ne!(rebound.wrapped_dek, restored.wrapped_dek);
            assert_eq!(
                open_moderation_quarantine_object(
                    &rebound,
                    &rebound_record,
                    &binding,
                    &replacement
                )
                .unwrap(),
                payload
            );
            assert_eq!(norito::core::get_decode_flags(), read_flags);
        }
        assert_eq!(norito::core::get_decode_flags(), seal_flags);
    }
}

#[test]
fn quarantine_envelope_decoder_preserves_limits_and_rejects_compression_before_allocation() {
    let wrapper = test_key_wrapper(0x74, "software://sorafs/moderation/key-v1");
    let binding = test_key_provider_binding();
    let (_, bytes) = seal_moderation_quarantine_object(
        ModerationQuarantineObjectInput {
            quarantine_id: [0x64; 16],
            payload: vec![0xC3; 70_000],
            captured_at_unix: 1_800_000_504,
            content_type: None,
            notes: None,
        },
        &binding,
        &wrapper,
    )
    .unwrap();
    let envelope =
        decode_moderation_quarantine_object_envelope(&bytes, bytes.len() as u64).unwrap();
    assert_eq!(envelope.chunks.len(), 2);
    assert_quarantine_envelope_allocation_at_every_alignment(&bytes, &envelope);
    assert!(decode_moderation_quarantine_object_envelope(&bytes, bytes.len() as u64 - 1).is_err());
    let zero_allocation = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 128);
    assert!(
        norito::with_decode_limits_scope(zero_allocation, || {
            decode_moderation_quarantine_object_envelope(&bytes, bytes.len() as u64)
        })
        .expect_err("valid envelope still requires budget")
        .is_decode_resource_limit()
    );
    for (label, limits) in [
        (
            "sequence",
            norito::DecodeLimits::new(1, usize::MAX, usize::MAX, usize::MAX, 128),
        ),
        (
            "field",
            norito::DecodeLimits::new(usize::MAX, 0, usize::MAX, usize::MAX, 128),
        ),
        (
            "elements",
            norito::DecodeLimits::new(usize::MAX, usize::MAX, 0, usize::MAX, 128),
        ),
        (
            "depth",
            norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, 0),
        ),
    ] {
        let error = norito::with_decode_limits_scope(limits, || {
            decode_moderation_quarantine_object_envelope(&bytes, bytes.len() as u64)
        })
        .expect_err("schema allocation allowance cannot weaken any caller limit");
        assert!(
            matches!(
                (label, &error),
                (
                    "sequence",
                    norito::Error::SequenceLengthExceeded { limit: 1, .. }
                ) | ("field", norito::Error::FieldLengthExceeded { limit: 0, .. })
                    | (
                        "elements",
                        norito::Error::TotalElementsExceeded { limit: 0, .. }
                    )
                    | (
                        "depth",
                        norito::Error::NestingDepthExceeded { limit: 0, .. }
                    )
            ),
            "{label}: {error:?}"
        );
    }
    let mut suffix = bytes.clone();
    suffix.push(0);
    assert!(matches!(
        decode_moderation_quarantine_object_envelope(&suffix, bytes.len() as u64),
        Err(norito::Error::Message(message)) if message.contains("byte length exceeds its configured bound")
    ));
    assert!(decode_moderation_quarantine_object_envelope(&suffix, suffix.len() as u64).is_err());
    let header = norito::core::Header::read(bytes.as_slice()).unwrap();
    let offset = header.magic.len() + 2 + header.schema.len();
    let mut compressed = bytes;
    compressed[offset] = norito::Compression::Zstd as u8;
    compressed[offset + 1..offset + 9].copy_from_slice(&u64::MAX.to_le_bytes());
    for candidate in [
        compressed.as_slice(),
        &compressed[..norito::core::Header::SIZE],
    ] {
        let (result, usage) = norito::core::with_decode_limits_measured(zero_allocation, || {
            decode_moderation_quarantine_object_envelope(candidate, candidate.len() as u64)
        });
        assert_eq!(usage.total_allocated_bytes(), 0);
        assert!(matches!(result, Err(norito::Error::NonCanonicalEncoding)));
    }
}

fn assert_quarantine_envelope_allocation_at_every_alignment(
    bytes: &[u8],
    expected: &ModerationQuarantineObjectEnvelopeV1,
) {
    let alignment = align_of::<ModerationQuarantineObjectEnvelopeV1>();
    assert!(alignment >= align_of::<ModerationQuarantineCiphertextChunkV1>());
    let mut backing = vec![0; bytes.len() + alignment];
    for offset in 0..alignment {
        backing[offset..offset + bytes.len()].copy_from_slice(bytes);
        let candidate = &backing[offset..offset + bytes.len()];
        let defaults = norito::canonical_decode_limits(candidate.len());
        let (result, usage) = norito::core::with_decode_limits_measured(defaults, || {
            decode_moderation_quarantine_object_envelope(candidate, candidate.len() as u64)
        });
        assert_eq!(
            &result.expect("schema budget admits each input alignment"),
            expected
        );
        let measured = usage.total_allocated_bytes();
        assert!(measured > 0);
        assert!(
            measured < defaults.max_total_allocated_bytes(),
            "retain margin below the native general ceiling"
        );
        let exact = norito::DecodeLimits::new(
            defaults.max_sequence_elements(),
            defaults.max_field_bytes(),
            defaults.max_total_elements(),
            measured,
            defaults.max_nesting_depth(),
        );
        let recovered = norito::with_decode_limits_scope(exact, || {
            decode_moderation_quarantine_object_envelope(candidate, candidate.len() as u64)
        })
        .expect("exact measured caller allocation budget is sufficient");
        assert_eq!(&recovered, expected);
        drop(recovered);
        let one_under = norito::DecodeLimits::new(
            defaults.max_sequence_elements(),
            defaults.max_field_bytes(),
            defaults.max_total_elements(),
            measured - 1,
            defaults.max_nesting_depth(),
        );
        assert!(matches!(
            norito::with_decode_limits_scope(one_under, || {
                decode_moderation_quarantine_object_envelope(candidate, candidate.len() as u64)
            }),
            Err(norito::Error::TotalAllocationExceeded { limit, .. }) if limit == (measured - 1) as u64
        ));
    }
}

// Test-only provider format pads a real context-authenticated wrapping result to
// the admitted maximum; its decoder checks every padding byte before unwrapping.
#[derive(Debug)]
struct MaximumSizeQuarantineTestWrapper(TestQuarantineKeyWrapper);

impl ModerationQuarantineKeyWrapper for MaximumSizeQuarantineTestWrapper {
    fn provider_handle(&self) -> &str {
        self.0.provider_handle()
    }
    fn qualification(
        &self,
    ) -> Result<
        ModerationQuarantineKeyProviderQualificationV1,
        ModerationQuarantineKeyProviderReadinessErrorV1,
    > {
        self.0.qualification()
    }
    fn active_key_id(&self) -> &str {
        self.0.active_key_id()
    }
    fn wrap_dek(
        &self,
        context: [u8; 32],
        dek: &[u8; 32],
    ) -> Result<Vec<u8>, ModerationQuarantineKeyOperationErrorV1> {
        let mut wrapped = self.0.wrap_dek(context, dek)?;
        assert_eq!(
            wrapped.len(),
            32 + MODERATION_QUARANTINE_OBJECT_AEAD_TAG_BYTES_V1
        );
        wrapped.resize(MODERATION_QUARANTINE_OBJECT_MAX_WRAPPED_DEK_BYTES_V1, 0);
        Ok(wrapped)
    }
    fn unwrap_dek(
        &self,
        key_id: &str,
        context: [u8; 32],
        wrapped: &[u8],
    ) -> Result<[u8; 32], ModerationQuarantineKeyOperationErrorV1> {
        let inner_len = 32 + MODERATION_QUARANTINE_OBJECT_AEAD_TAG_BYTES_V1;
        if wrapped.len() != MODERATION_QUARANTINE_OBJECT_MAX_WRAPPED_DEK_BYTES_V1
            || wrapped[inner_len..].iter().any(|byte| *byte != 0)
        {
            return Err(ModerationQuarantineKeyOperationErrorV1::Rejected);
        }
        self.0.unwrap_dek(key_id, context, &wrapped[..inner_len])
    }
}

#[test]
fn quarantine_envelope_maximum_valid_schema_fits_derived_allocation_budget() {
    let prefix = "software://sorafs/moderation/";
    let maximum_key_id = format!(
        "{prefix}{}",
        "k".repeat(iroha_config::parameters::PRODUCTION_RUNTIME_HANDLE_MAX_BYTES - prefix.len())
    );
    validate_wrapping_key_id_text(&maximum_key_id).unwrap();
    let wrapper = MaximumSizeQuarantineTestWrapper(test_key_wrapper(0x76, &maximum_key_id));
    let binding = test_key_provider_binding();
    let payload = vec![0xD3; MODERATION_QUARANTINE_OBJECT_MAX_PAYLOAD_BYTES_V1 as usize];
    let (record, bytes) = seal_moderation_quarantine_object(
        ModerationQuarantineObjectInput {
            quarantine_id: [0x65; 16],
            payload: payload.clone(),
            captured_at_unix: 1_800_000_507,
            // This is the longest actually allowlisted label; arbitrary
            // 256-byte text is invalid even though the structural cap is 256.
            content_type: Some("application/octet-stream".to_owned()),
            notes: None,
        },
        &binding,
        &wrapper,
    )
    .expect("seal maximum payload with a real context-bound wrapped DEK");
    let envelope =
        decode_moderation_quarantine_object_envelope(&bytes, bytes.len() as u64).unwrap();
    assert_eq!(
        envelope.chunks.len(),
        MODERATION_QUARANTINE_OBJECT_MAX_CHUNKS_V1
    );
    assert_eq!(
        envelope.wrapped_dek.len(),
        MODERATION_QUARANTINE_OBJECT_MAX_WRAPPED_DEK_BYTES_V1
    );
    assert_eq!(envelope.wrapping_key_id, maximum_key_id);
    validate_quarantine_object_envelope(&envelope).unwrap();
    assert_eq!(
        open_moderation_quarantine_object(&envelope, &record, &binding, &wrapper).unwrap(),
        payload
    );
    drop(payload);
    assert_quarantine_envelope_allocation_at_every_alignment(&bytes, &envelope);
    assert!(decode_moderation_quarantine_object_envelope(&bytes, bytes.len() as u64 - 1).is_err());
    let mut oversized_key = envelope.clone();
    oversized_key.wrapped_dek.push(0);
    assert!(
        matches!(validate_quarantine_object_envelope(&oversized_key), Err(ModerationQuarantineObjectError::InvalidSnapshot { message }) if message.contains("wrapped DEK length"))
    );
}
