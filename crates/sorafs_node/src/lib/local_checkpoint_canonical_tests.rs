// Shared checkpoint admission has one fixed V1 frame under every caller layout.

#[test]
fn local_checkpoint_canonical_recovery_and_rejection_ignore_caller_layout() {
    let value = vec![11_u64, 22];
    let canonical = norito::encode_canonical(&value).expect("canonical checkpoint");
    let mut alternate_layouts = 0;
    for flags in (0..=u8::MAX).filter(|flags| norito::core::validate_header_flags(*flags).is_ok()) {
        let _caller = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(
            decode_local_checkpoint_canonical::<Vec<u64>>(&canonical, canonical.len() as u64, 2)
                .expect("canonical recovery under every caller layout"),
            value
        );
        let alternate = norito::to_bytes(&value).expect("advertised caller-layout checkpoint");
        assert_eq!(
            norito::decode_from_bytes::<Vec<u64>>(&alternate).unwrap(),
            value
        );
        if alternate != canonical {
            alternate_layouts += 1;
            assert_eq!(
                decode_local_checkpoint_canonical::<Vec<u64>>(&alternate, 4_096, 2),
                Err("checkpoint is not the exact canonical Norito encoding".to_owned())
            );
        }
        assert_eq!(norito::core::get_decode_flags(), flags);
    }
    assert!(
        alternate_layouts > 0,
        "exercise a real same-value alternate frame"
    );
}

#[test]
fn local_checkpoint_canonical_compression_is_rejected_before_allocation() {
    let value = vec![11_u64, 22];
    let canonical = norito::encode_canonical(&value).expect("canonical checkpoint");
    assert_eq!(
        decode_local_checkpoint_canonical::<Vec<u64>>(&canonical, 4_096, 2).unwrap(),
        value
    );
    let header = norito::core::Header::read(canonical.as_slice()).expect("canonical frame header");
    let compression_offset = header.magic.len() + 2 + header.schema.len();
    let mut forbidden = canonical;
    forbidden[compression_offset] = norito::Compression::Zstd as u8;
    forbidden[compression_offset + 1..compression_offset + 9]
        .copy_from_slice(&u64::MAX.to_le_bytes());
    let advertised =
        norito::core::Header::read(forbidden.as_slice()).expect("forbidden frame header");
    assert_eq!(advertised.compression, norito::Compression::Zstd);
    assert_eq!(advertised.length, u64::MAX);
    let zero_allocation = norito::DecodeLimits::new(0, 0, 0, 0, 0);
    for flags in (0..=u8::MAX).filter(|flags| norito::core::validate_header_flags(*flags).is_ok()) {
        let _caller = norito::core::DecodeFlagsGuard::enter(flags);
        for bytes in [
            forbidden.as_slice(),
            &forbidden[..norito::core::Header::SIZE],
        ] {
            let (result, usage) =
                norito::core::with_decode_limits_measured(zero_allocation, || {
                    decode_local_checkpoint_canonical::<Vec<u64>>(bytes, 4_096, 2)
                });
            assert_eq!(usage.total_allocated_bytes(), 0);
            assert_eq!(
                result,
                Err("checkpoint is not the exact canonical Norito encoding".to_owned())
            );
            assert_eq!(norito::core::get_decode_flags(), flags);
        }
    }
}

#[test]
fn local_checkpoint_canonical_exact_limits_and_suffix_are_enforced() {
    let value = vec![11_u64, 22];
    let canonical = norito::encode_canonical(&value).expect("canonical checkpoint");
    let max_bytes = canonical.len() as u64;
    let limits = local_checkpoint_decode_limits(canonical.len(), canonical.len(), 2).unwrap();
    let (result, usage) = norito::core::with_decode_limits_measured(limits, || {
        decode_local_checkpoint_canonical::<Vec<u64>>(&canonical, max_bytes, 2)
    });
    assert_eq!(result.unwrap(), value);
    assert!(usage.total_allocated_bytes() > 0);
    assert_eq!(
        decode_local_checkpoint_canonical::<Vec<u64>>(&canonical, max_bytes - 1, 2),
        Err(format!(
            "checkpoint is {} bytes, exceeding limit {}",
            canonical.len(),
            max_bytes - 1
        ))
    );
    let error = decode_local_checkpoint_canonical::<Vec<u64>>(&canonical, max_bytes, 1)
        .expect_err("one-under sequence bound must reject");
    assert!(
        error.starts_with("bounded checkpoint decode failed:"),
        "{error}"
    );
    let stricter = norito::DecodeLimits::new(
        limits.max_sequence_elements(),
        limits.max_field_bytes(),
        limits.max_total_elements(),
        usage.total_allocated_bytes() - 1,
        limits.max_nesting_depth(),
    );
    let error = norito::with_decode_limits_scope(stricter, || {
        decode_local_checkpoint_canonical::<Vec<u64>>(&canonical, max_bytes, 2)
    })
    .expect_err("stricter ambient allocation limit remains enforced");
    assert!(
        error.starts_with("bounded checkpoint decode failed:"),
        "{error}"
    );
    let mut trailing = canonical;
    trailing.push(0);
    assert!(decode_local_checkpoint_canonical::<Vec<u64>>(&trailing, max_bytes + 1, 2).is_err());
}

#[test]
fn local_checkpoint_decoder_rejects_trailing_bytes_and_sequence_bombs() {
    let value = vec![1_u64, 2];
    let canonical = norito::encode_canonical(&value).expect("encode canonical checkpoint fixture");
    assert_eq!(
        decode_local_checkpoint_canonical::<Vec<u64>>(&canonical, 4_096, 2)
            .expect("decode canonical checkpoint fixture"),
        value
    );
    let mut trailing = canonical;
    trailing.push(0);
    assert!(
        decode_local_checkpoint_canonical::<Vec<u64>>(&trailing, 4_096, 2).is_err(),
        "trailing bytes must not be accepted as an equivalent checkpoint"
    );
    let oversized_sequence =
        norito::encode_canonical(&vec![1_u64, 2, 3]).expect("encode sequence bomb fixture");
    assert!(
        decode_local_checkpoint_canonical::<Vec<u64>>(&oversized_sequence, 4_096, 2).is_err(),
        "declared sequence length must fail before allocation beyond the configured bound"
    );
}
#[test]
fn local_checkpoint_decode_limits_follow_actual_wire_size() {
    let limits = local_checkpoint_decode_limits(64, 4_096, usize::MAX)
        .expect("derive bounded checkpoint limits");
    assert_eq!(limits.max_sequence_elements(), 64 * 8);
    assert_eq!(limits.max_field_bytes(), 64);
    assert_eq!(limits.max_total_elements(), 64 * 8);
    assert_eq!(limits.max_total_allocated_bytes(), 4_096 * 4);
    assert_eq!(
        limits.max_nesting_depth(),
        norito::core::MAX_VALUE_NESTING_DEPTH
    );
    assert!(local_checkpoint_decode_limits(0, 4_096, 1).is_err());
    assert!(local_checkpoint_decode_limits(4_097, 4_096, 1).is_err());
}
