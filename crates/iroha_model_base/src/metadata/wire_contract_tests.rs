// Metadata's tuple-sequence wire and checked-decoder error contracts.
mod wire_contract_tests {
    use super::*;
    use ncore::{DecodeFlagsGuard, DeserializePayload, PayloadCtxGuard};

    fn layouts() -> Vec<u8> {
        let flags: Vec<_> = (0..=u8::MAX)
            .filter(|flags| ncore::validate_header_flags(*flags).is_ok())
            .collect();
        assert_eq!(flags.len(), 10, "exercise every valid V1 layout");
        flags
    }

    fn entries() -> Vec<(Name, Json)> {
        vec![
            ("alpha".parse().unwrap(), Json::new("first value")),
            ("beta".parse().unwrap(), Json::new(vec![1_u64, 2, 3])),
        ]
    }

    fn metadata(entries: &[(Name, Json)]) -> Metadata {
        Metadata(entries.iter().cloned().collect())
    }

    fn tuple_frame(entries: &[(Name, Json)], flags: u8) -> Vec<u8> {
        let _layout = DecodeFlagsGuard::enter(flags);
        let (payload, actual_flags) = norito::codec::encode_with_header_flags(&entries.to_vec());
        ncore::frame_bare_with_header_flags::<Metadata>(&payload, actual_flags)
            .expect("frame actual tuple-sequence bytes with Metadata's schema")
    }

    fn direct_decode(frame: &[u8], infallible: bool) -> Result<Metadata, ncore::Error> {
        let view = ncore::from_bytes_view(frame)?;
        // The view validates Metadata's actual schema and padding before invoking
        // the decoder. The owning archive safely handles any required alignment.
        view.decode_exact_with::<Metadata, _>(|payload| {
            let archive = ncore::archived_from_slice::<Metadata>(payload)?;
            let _payload = PayloadCtxGuard::enter_with_flags(archive.bytes(), view.flags());
            let decoded = if infallible {
                Metadata::deserialize(archive.archived())
            } else {
                Metadata::try_deserialize(archive.archived())?
            };
            Ok((decoded, payload.len()))
        })
    }

    fn assert_message(result: Result<Metadata, ncore::Error>, expected: &str) {
        match result {
            Err(ncore::Error::Message(message)) => assert_eq!(message, expected),
            other => panic!("expected Message({expected:?}), got {other:?}"),
        }
    }

    #[test]
    fn metadata_preserves_tuple_wire_and_nested_frames_in_all_ten_layouts() {
        type Nested = (u64, Metadata, Vec<String>);
        for entries in [Vec::new(), entries()[..1].to_vec(), entries()] {
            let value = metadata(&entries);
            let canonical = norito::encode_canonical(&value).unwrap();
            let nested = (
                17_u64,
                value.clone(),
                vec!["left".to_owned(), "right".to_owned()],
            );
            let nested_reference = (17_u64, entries.clone(), nested.2.clone());
            let canonical_nested = norito::encode_canonical(&nested).unwrap();
            for flags in layouts() {
                let _layout = DecodeFlagsGuard::enter(flags);
                let (payload, actual_flags) = norito::codec::encode_with_header_flags(&value);
                assert_eq!(
                    (payload.clone(), actual_flags),
                    norito::codec::encode_with_header_flags(&entries),
                    "Metadata changed tuple-sequence bytes for {flags:#04x}"
                );
                let frame = ncore::frame_bare_with_header_flags::<Metadata>(&payload, actual_flags)
                    .unwrap();
                assert_eq!(frame, tuple_frame(&entries, flags));
                assert_eq!(frame, ncore::to_bytes(&value).unwrap());
                let decoded: Metadata = norito::decode_from_bytes(&frame).unwrap();
                assert_eq!(decoded, value);
                assert_eq!(ncore::to_bytes(&decoded).unwrap(), frame);
                assert_eq!(norito::encode_canonical(&decoded).unwrap(), canonical);
                assert_eq!(
                    norito::decode_canonical::<Metadata>(&canonical).unwrap(),
                    value
                );
                assert_eq!(
                    norito::canonical_frame_len(&value).unwrap(),
                    canonical.len()
                );

                let (nested_payload, nested_flags) =
                    norito::codec::encode_with_header_flags(&nested);
                assert_eq!(
                    (nested_payload.clone(), nested_flags),
                    norito::codec::encode_with_header_flags(&nested_reference),
                    "tuple siblings expose any Metadata layout change"
                );
                let nested_frame =
                    ncore::frame_bare_with_header_flags::<Nested>(&nested_payload, nested_flags)
                        .unwrap();
                assert_eq!(nested_frame, ncore::to_bytes(&nested).unwrap());
                let decoded_nested: Nested = norito::decode_from_bytes(&nested_frame).unwrap();
                assert_eq!(decoded_nested, nested);
                assert_eq!(ncore::to_bytes(&decoded_nested).unwrap(), nested_frame);
                assert_eq!(
                    norito::encode_canonical(&decoded_nested).unwrap(),
                    canonical_nested
                );
                assert_eq!(
                    norito::decode_canonical::<Nested>(&canonical_nested).unwrap(),
                    nested
                );
                assert_eq!(
                    ncore::to_bytes(&value).unwrap(),
                    frame,
                    "canonical operations must restore ambient layout {flags:#04x}"
                );
            }
        }
    }

    #[test]
    fn unordered_distinct_metadata_keys_decode_but_fail_exact_canonical_comparison() {
        let sorted = entries();
        let value = metadata(&sorted);
        let canonical = norito::encode_canonical(&value).unwrap();
        assert_eq!(
            tuple_frame(&sorted, ncore::default_encode_flags()),
            canonical,
            "the sorted control uses Metadata's exact canonical frame"
        );
        assert_eq!(
            norito::decode_canonical::<Metadata>(&canonical).unwrap(),
            value
        );
        let mut unordered = sorted;
        unordered.reverse();
        for flags in layouts() {
            let frame = tuple_frame(&unordered, flags);
            assert_eq!(direct_decode(&frame, false).unwrap(), value);
            assert_eq!(
                norito::decode_from_bytes::<Metadata>(&frame).unwrap(),
                value
            );
            assert_eq!(direct_decode(&frame, true).unwrap(), value);
        }
        // Use the canonical flags to isolate key order from alternate layouts.
        let unordered_frame = tuple_frame(&unordered, ncore::default_encode_flags());
        assert_ne!(unordered_frame, canonical);
        assert!(matches!(
            norito::decode_canonical::<Metadata>(&unordered_frame),
            Err(ncore::Error::NonCanonicalEncoding)
        ));
    }

    #[test]
    fn duplicate_metadata_keys_keep_the_exact_fallible_and_canonical_error() {
        let positive = entries();
        let value = metadata(&positive);
        let mut duplicate = positive.clone();
        duplicate[1].0 = duplicate[0].0.clone();
        assert_ne!(duplicate[0].1, duplicate[1].1);
        for flags in layouts() {
            let valid = tuple_frame(&positive, flags);
            assert_eq!(direct_decode(&valid, false).unwrap(), value);
            assert_eq!(
                norito::decode_from_bytes::<Metadata>(&valid).unwrap(),
                value
            );
            let frame = tuple_frame(&duplicate, flags);
            assert_message(direct_decode(&frame, false), "duplicate metadata key");
            assert_message(
                norito::decode_from_bytes::<Metadata>(&frame),
                "duplicate metadata key",
            );
        }
        let canonical_control = norito::encode_canonical(&value).unwrap();
        assert_eq!(
            norito::decode_canonical::<Metadata>(&canonical_control).unwrap(),
            value
        );
        let frame = tuple_frame(&duplicate, ncore::default_encode_flags());
        assert_message(
            norito::decode_canonical::<Metadata>(&frame),
            "duplicate metadata key",
        );
    }

    #[test]
    fn infallible_metadata_decode_rejects_duplicate_keys_in_all_ten_layouts() {
        let positive = entries();
        let value = metadata(&positive);
        let mut duplicate = positive.clone();
        duplicate[1].0 = duplicate[0].0.clone();
        for flags in layouts() {
            let valid = tuple_frame(&positive, flags);
            assert_eq!(direct_decode(&valid, true).unwrap(), value);
            let frame = tuple_frame(&duplicate, flags);
            assert_message(direct_decode(&frame, false), "duplicate metadata key");
            let panic = std::panic::catch_unwind(|| direct_decode(&frame, true))
                .expect_err("infallible Metadata decode must reject duplicate entries");
            let message = panic
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| panic.downcast_ref::<&str>().copied())
                .expect("Metadata's checked decoder emits a textual panic");
            assert!(message.contains("Metadata decode"), "{message}");
            assert!(message.contains("duplicate metadata key"), "{message}");
            assert_eq!(
                direct_decode(&valid, true).unwrap(),
                value,
                "unwinding must restore the decode context"
            );
        }
    }

    #[test]
    fn malformed_later_metadata_entry_is_rejected_before_an_earlier_duplicate() {
        const LATER_NAME: &str = "z_later_key";
        const NAME_ERROR: &str = "White space not allowed in `Name` constructs";
        let mut positive = entries();
        positive.push((LATER_NAME.parse().unwrap(), Json::new("last value")));
        let value = metadata(&positive);
        for flags in layouts() {
            let valid = tuple_frame(&positive, flags);
            assert_eq!(direct_decode(&valid, false).unwrap(), value);
            assert_eq!(
                norito::decode_from_bytes::<Metadata>(&valid).unwrap(),
                value
            );
            let mut duplicate = positive.clone();
            duplicate[1].0 = duplicate[0].0.clone();
            let duplicate_frame = tuple_frame(&duplicate, flags);
            assert_message(
                direct_decode(&duplicate_frame, false),
                "duplicate metadata key",
            );

            let _layout = DecodeFlagsGuard::enter(flags);
            let (mut payload, actual_flags) = norito::codec::encode_with_header_flags(&duplicate);
            let occurrences: Vec<_> = payload
                .windows(LATER_NAME.len())
                .enumerate()
                .filter_map(|(offset, bytes)| (bytes == LATER_NAME.as_bytes()).then_some(offset))
                .collect();
            assert_eq!(
                occurrences.len(),
                1,
                "mutate only the final actual Name field"
            );
            let start = occurrences[0];
            payload[start..start + LATER_NAME.len()].fill(b' ');
            // Keep every real tuple/field length and recompute the real frame's
            // checksum; the later Name validator must own the rejection.
            let frame =
                ncore::frame_bare_with_header_flags::<Metadata>(&payload, actual_flags).unwrap();
            assert_message(direct_decode(&frame, false), NAME_ERROR);
            assert_message(norito::decode_from_bytes::<Metadata>(&frame), NAME_ERROR);
            if flags == ncore::default_encode_flags() {
                assert_eq!(norito::decode_canonical::<Metadata>(&valid).unwrap(), value);
                assert_message(
                    norito::decode_canonical::<Metadata>(&duplicate_frame),
                    "duplicate metadata key",
                );
                assert_message(norito::decode_canonical::<Metadata>(&frame), NAME_ERROR);
            }
        }
    }
}
