// Canonical codec allocation, framing, and error-classification regressions.

#[cfg(test)]
mod canonical_codec_tests {
    use super::*;

    #[test]
    fn canonical_scalar_and_unit_roundtrip() {
        let scalar = encode_canonical(&1_u8).expect("encode canonical scalar");
        assert_eq!(
            decode_canonical_with_limits::<u8>(&scalar, canonical_decode_limits(scalar.len()),)
                .expect("decode canonical scalar"),
            1
        );

        let unit = encode_canonical(&()).expect("encode canonical unit");
        decode_canonical_with_limits::<()>(&unit, canonical_decode_limits(unit.len()))
            .expect("decode canonical unit");
    }

    #[test]
    fn exact_slice_writer_tracks_mismatch_overrun_and_final_length() {
        let expected = b"canonical frame";
        let mut exact = ExactSliceWriter::new(expected);
        exact
            .write_all(b"canonical ")
            .expect("write matching prefix");
        assert!(!exact.mismatched());
        assert!(!exact.is_complete());
        exact.write_all(b"frame").expect("write matching suffix");
        assert!(exact.is_complete());

        let mut mismatch = ExactSliceWriter::new(expected);
        mismatch
            .write_all(b"canonical blame")
            .expect("comparison writer consumes a mismatching chunk");
        assert!(mismatch.mismatched());
        assert!(!mismatch.is_complete());

        let mut overrun = ExactSliceWriter::new(expected);
        overrun
            .write_all(b"canonical frame plus")
            .expect("comparison writer consumes an overrunning chunk");
        assert!(overrun.mismatched());
        assert!(!overrun.is_complete());
    }

    #[test]
    fn exact_frame_verifier_preserves_ambient_layout_and_rejects_byte_drift() {
        let value = vec!["first".to_owned(), "second".to_owned()];
        let alternate_flags = core::default_encode_flags() ^ core::header_flags::COMPACT_LEN;
        let _ambient = core::DecodeFlagsGuard::enter(alternate_flags);
        let frame = core::to_bytes(&value).expect("encode exact ambient frame");

        verify_exact_frame(&value, &frame).expect("exact ambient frame verifies");

        let mut mismatched = frame.clone();
        let last = mismatched.last_mut().expect("framed vector is non-empty");
        *last ^= 1;
        assert!(matches!(
            verify_exact_frame(&value, &mismatched),
            Err(Error::NonCanonicalEncoding)
        ));
        assert!(matches!(
            verify_exact_frame(&value, &frame[..frame.len() - 1]),
            Err(Error::NonCanonicalEncoding)
        ));
        let mut extended = frame.clone();
        extended.push(0);
        assert!(matches!(
            verify_exact_frame(&value, &extended),
            Err(Error::NonCanonicalEncoding)
        ));
    }

    #[test]
    fn canonical_allocation_budget_covers_large_signed_genesis() {
        // A production signed-genesis payload of this size accounts for slightly
        // more than the former 32x-plus-64-KiB allocation envelope while it is
        // reconstructed into owned containers.
        const PAYLOAD_BYTES: usize = 55_766;
        const ACCOUNTED_ALLOCATION_BYTES: usize = 1_850_832;

        let allocation_budget = canonical_decode_limits(PAYLOAD_BYTES).max_total_allocated_bytes();

        assert_eq!(allocation_budget, PAYLOAD_BYTES * 64 + 64 * 1024);
        assert!(allocation_budget >= ACCOUNTED_ALLOCATION_BYTES);
    }

    #[test]
    fn canonical_allocation_budget_covers_large_resultless_genesis_candidate() {
        // The canonical resultless projection has a smaller frame than its
        // result-bearing signed genesis, but reconstructing its owned graph
        // crosses the 33x-plus-64-KiB envelope used by the former policy.
        const PAYLOAD_BYTES: usize = 54_586;
        const FIRST_REJECTED_ALLOCATION_BYTES: usize = 1_867_001;

        let allocation_budget = canonical_decode_limits(PAYLOAD_BYTES).max_total_allocated_bytes();

        assert_eq!(allocation_budget, PAYLOAD_BYTES * 64 + 64 * 1024);
        assert!(allocation_budget >= FIRST_REJECTED_ALLOCATION_BYTES);
    }

    #[test]
    fn canonical_allocation_budget_covers_live_consensus_peer_payload() {
        // Exact first rejected reserve observed for a legitimate high-stream
        // consensus frame. This proves the former 34x policy insufficient; it
        // is not a measurement of the allocation required to complete decode.
        const PAYLOAD_BYTES: usize = 42_241;
        const FIRST_REJECTED_ALLOCATION_BYTES: usize = 1_543_396;
        const LEGACY_ALLOCATION_BUDGET: usize = PAYLOAD_BYTES * 34 + 64 * 1024;

        let allocation_budget = canonical_decode_limits(PAYLOAD_BYTES).max_total_allocated_bytes();

        assert_eq!(LEGACY_ALLOCATION_BUDGET, 1_501_730);
        const { assert!(LEGACY_ALLOCATION_BUDGET < FIRST_REJECTED_ALLOCATION_BYTES) };
        assert_eq!(allocation_budget, PAYLOAD_BYTES * 64 + 64 * 1024);
        assert!(allocation_budget >= FIRST_REJECTED_ALLOCATION_BYTES);
    }

    #[test]
    fn canonical_allocation_policy_exceeds_rejected_live_consensus_reserve() {
        // Exact first rejected reserve from the legitimate consensus value
        // carried by the 42,716-byte P2P frame. This observation proves that the
        // former 35x policy was insufficient; it is not a measurement of the
        // allocation required to complete the decode.
        const PAYLOAD_BYTES: usize = 42_241;
        const FIRST_REJECTED_ALLOCATION_BYTES: usize = 1_584_958;
        const LEGACY_ALLOCATION_BUDGET: usize = PAYLOAD_BYTES * 35 + 64 * 1024;

        let allocation_budget = canonical_decode_limits(PAYLOAD_BYTES).max_total_allocated_bytes();

        assert_eq!(LEGACY_ALLOCATION_BUDGET, 1_543_971);
        const { assert!(LEGACY_ALLOCATION_BUDGET < FIRST_REJECTED_ALLOCATION_BYTES) };
        assert_eq!(allocation_budget, PAYLOAD_BYTES * 64 + 64 * 1024);
        assert_eq!(allocation_budget, 2_768_960);
        assert!(allocation_budget >= FIRST_REJECTED_ALLOCATION_BYTES);
    }

    #[test]
    fn canonical_allocation_policy_exceeds_rejected_queue_journal_reserve() {
        // Exact first rejected reserve from peer 2's legitimate queue-plan Put.
        // This independently proves the former 35x policy insufficient for the
        // durable journal path, but does not claim that the rejected reserve was
        // the decoder's final allocation.
        const PAYLOAD_BYTES: usize = 43_074;
        const FIRST_REJECTED_ALLOCATION_BYTES: usize = 1_614_849;
        const LEGACY_ALLOCATION_BUDGET: usize = PAYLOAD_BYTES * 35 + 64 * 1024;

        let allocation_budget = canonical_decode_limits(PAYLOAD_BYTES).max_total_allocated_bytes();

        assert_eq!(LEGACY_ALLOCATION_BUDGET, 1_573_126);
        const { assert!(LEGACY_ALLOCATION_BUDGET < FIRST_REJECTED_ALLOCATION_BYTES) };
        assert_eq!(allocation_budget, PAYLOAD_BYTES * 64 + 64 * 1024);
        assert_eq!(allocation_budget, 2_822_272);
        assert!(allocation_budget >= FIRST_REJECTED_ALLOCATION_BYTES);
    }

    #[test]
    fn generic_canonical_policy_is_linear_and_rejects_forged_resources() {
        const POLICY_PROBE_BYTES: usize = 1024;
        let limits = canonical_decode_limits(POLICY_PROBE_BYTES);
        assert_eq!(
            limits.max_total_allocated_bytes(),
            POLICY_PROBE_BYTES * 64 + 64 * 1024
        );

        const FORGED_LENGTH: u64 = 1 << 40;
        let bare = FORGED_LENGTH.to_le_bytes();
        let frame =
            core::frame_bare_with_header_flags::<Vec<u64>>(&bare, core::default_encode_flags())
                .expect("frame forged vector with a valid checksum");

        assert!(matches!(
            decode_canonical::<Vec<u64>>(&frame),
            Err(Error::SequenceLengthExceeded { .. }) | Err(Error::TotalElementsExceeded { .. })
        ));

        let forged_allocation = limits
            .max_total_allocated_bytes()
            .checked_add(1)
            .expect("bounded policy probe fits usize");
        let forged_allocation_u64 =
            u64::try_from(forged_allocation).expect("bounded policy probe fits u64");
        let allocation_limit_u64 = u64::try_from(limits.max_total_allocated_bytes())
            .expect("bounded policy limit fits u64");
        let error = with_decode_limits(limits, || {
            core::reserve_decode_allocation(forged_allocation)
        })
        .expect_err("one byte beyond the linear allocation policy must fail before allocation");
        assert!(matches!(
            error,
            Error::TotalAllocationExceeded { attempted, limit }
                if attempted == forged_allocation_u64 && limit == allocation_limit_u64
        ));
    }

    #[test]
    fn canonical_allocation_policy_caps_amplified_extra() {
        const LAST_UNCAPPED_PAYLOAD_BYTES: usize = CANONICAL_DECODE_MAX_EXTRA_ALLOCATION_BYTES
            / CANONICAL_DECODE_ALLOCATION_EXTRA_MULTIPLIER;
        const FIRST_CAPPED_PAYLOAD_BYTES: usize = LAST_UNCAPPED_PAYLOAD_BYTES + 1;
        const ONE_GIB: usize = 1024 * 1024 * 1024;

        let last_uncapped_extra = LAST_UNCAPPED_PAYLOAD_BYTES
            .checked_mul(CANONICAL_DECODE_ALLOCATION_EXTRA_MULTIPLIER)
            .expect("cap-transition fixture fits usize");
        let first_uncapped_extra = FIRST_CAPPED_PAYLOAD_BYTES
            .checked_mul(CANONICAL_DECODE_ALLOCATION_EXTRA_MULTIPLIER)
            .expect("cap-transition fixture fits usize");
        assert!(last_uncapped_extra <= CANONICAL_DECODE_MAX_EXTRA_ALLOCATION_BYTES);
        assert!(first_uncapped_extra > CANONICAL_DECODE_MAX_EXTRA_ALLOCATION_BYTES);
        assert_eq!(
            canonical_decode_limits(LAST_UNCAPPED_PAYLOAD_BYTES).max_total_allocated_bytes(),
            LAST_UNCAPPED_PAYLOAD_BYTES
                + last_uncapped_extra
                + CANONICAL_DECODE_FIXED_ALLOCATION_BYTES
        );
        assert_eq!(
            canonical_decode_limits(FIRST_CAPPED_PAYLOAD_BYTES).max_total_allocated_bytes(),
            FIRST_CAPPED_PAYLOAD_BYTES
                + CANONICAL_DECODE_MAX_EXTRA_ALLOCATION_BYTES
                + CANONICAL_DECODE_FIXED_ALLOCATION_BYTES
        );
        assert_eq!(
            canonical_decode_limits(ONE_GIB).max_total_allocated_bytes(),
            ONE_GIB
                + CANONICAL_DECODE_MAX_EXTRA_ALLOCATION_BYTES
                + CANONICAL_DECODE_FIXED_ALLOCATION_BYTES
        );
    }

    #[test]
    fn schema_limits_tighten_the_payload_derived_default() {
        let value = vec![1_u64, 2, 3, 4];
        let frame = encode_canonical(&value).expect("encode canonical vector");
        let limits = DecodeLimits::new(3, frame.len(), 3, frame.len() * 32, 16);
        assert!(matches!(
            decode_canonical_with_limits::<Vec<u64>>(&frame, limits),
            Err(Error::SequenceLengthExceeded {
                length: 4,
                limit: 3
            }) | Err(Error::TotalElementsExceeded {
                attempted: 4,
                limit: 3
            })
        ));
    }

    #[test]
    fn canonical_decode_restores_ambient_flags_and_payload_context() {
        let value = vec!["first".to_owned(), "second".to_owned()];
        let canonical = encode_canonical(&value).expect("encode canonical fixture");
        let alternate_flags = core::default_encode_flags() ^ core::header_flags::COMPACT_LEN;
        let alternate = {
            let _alternate = core::DecodeFlagsGuard::enter(alternate_flags);
            core::to_bytes(&value).expect("encode alternate fixture")
        };
        assert_ne!(alternate, canonical);

        let _ambient = core::DecodeFlagsGuard::enter(alternate_flags);
        let ambient_payload = b"ambient outer payload";
        let _ambient_payload = core::PayloadCtxGuard::enter(ambient_payload);
        let payload_context_before = core::payload_ctx();
        let encoding_before = core::to_bytes(&value).expect("encode ambient fixture");
        assert_eq!(
            decode_canonical::<Vec<String>>(&canonical).expect("decode canonical fixture"),
            value
        );
        assert!(matches!(
            decode_canonical::<Vec<String>>(&alternate),
            Err(Error::NonCanonicalEncoding)
        ));
        assert_eq!(core::payload_ctx(), payload_context_before);
        assert_eq!(
            core::to_bytes(&vec!["first".to_owned(), "second".to_owned()])
                .expect("encode after canonical decode"),
            encoding_before
        );
    }

    #[cfg(all(feature = "compression", not(target_arch = "wasm32")))]
    #[test]
    fn canonical_decode_classifies_valid_compression_as_noncanonical() {
        let value = vec!["compressed".to_owned(); 64];
        let compressed = to_compressed_bytes(&value, Some(CompressionConfig::default()))
            .expect("encode compressed fixture");

        assert!(matches!(
            decode_canonical::<Vec<String>>(&compressed),
            Err(Error::NonCanonicalEncoding)
        ));
    }

    #[cfg(all(feature = "compression", not(target_arch = "wasm32")))]
    #[test]
    fn canonical_decode_rejects_compression_before_decode_allocation_budget() {
        let value = vec!["compressed expansion".to_owned(); 4096];
        let compressed = to_compressed_bytes(&value, Some(CompressionConfig::default()))
            .expect("encode compressed expansion fixture");
        let no_decode_resources = DecodeLimits::new(0, 0, 0, 0, 0);

        assert!(matches!(
            decode_canonical_with_limits::<Vec<String>>(&compressed, no_decode_resources),
            Err(Error::NonCanonicalEncoding)
        ));
    }

    #[test]
    fn canonical_decode_preserves_schema_and_malformed_errors() {
        let wrong_schema = encode_canonical(&42_u64).expect("encode wrong-schema fixture");
        assert!(matches!(
            decode_canonical::<Vec<u64>>(&wrong_schema),
            Err(Error::SchemaMismatch)
        ));

        let mut bad_checksum = encode_canonical(&vec![42_u64]).expect("encode checksum fixture");
        let last = bad_checksum
            .last_mut()
            .expect("canonical frame has a payload byte");
        *last ^= 0x80;
        assert!(matches!(
            decode_canonical::<Vec<u64>>(&bad_checksum),
            Err(Error::ChecksumMismatch)
        ));
    }
}
