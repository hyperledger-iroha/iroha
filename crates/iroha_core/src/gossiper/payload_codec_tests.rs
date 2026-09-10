//! Gossip field codecs accept payload owners without inventing frame identities.

use super::*;

#[derive(Debug, PartialEq, Eq, norito::Encode, norito::Decode)]
struct PayloadOnly(u32);

#[test]
fn payload_only_fields_preserve_exact_boundaries_and_reject_unread_bytes() {
    for flags in [0, ncore::header_flags::COMPACT_LEN] {
        let _flags = ncore::DecodeFlagsGuard::enter(flags);
        let value = PayloadOnly(0x1234_5678);
        let mut fields = Vec::new();
        ncore::write_len_prefixed(&mut ncore::Encoder::new(&mut fields), &value).unwrap();
        let boundary = fields.len();
        fields.extend_from_slice(&[0xab, 0xcd]);
        assert_eq!(
            decode_len_prefixed_field::<PayloadOnly>(&fields, 0).unwrap(),
            (value, boundary),
        );
        assert_eq!(&fields[boundary..], &[0xab, 0xcd]);
        assert!(decode_len_prefixed_field::<PayloadOnly>(&fields[..boundary - 1], 0).is_err());

        let payload = PayloadOnly(7).encode();
        let mut unread = Vec::new();
        ncore::write_len(&mut unread, (payload.len() + 1) as u64).unwrap();
        unread.extend_from_slice(&payload);
        unread.push(0);
        assert!(decode_len_prefixed_field::<PayloadOnly>(&unread, 0).is_err());
    }
}

#[test]
fn payload_only_sequences_preserve_sizes_and_enforce_admission_before_decode() {
    for flags in [0, ncore::header_flags::COMPACT_LEN] {
        let _flags = ncore::DecodeFlagsGuard::enter(flags);
        let values = vec![PayloadOnly(1), PayloadOnly(u32::MAX)];
        let mut fields = Vec::new();
        ncore::write_len_prefixed(&mut ncore::Encoder::new(&mut fields), &values).unwrap();
        let boundary = fields.len();
        fields.push(0xee);
        let (payload, payload_end) = len_prefixed_field_payload(&fields, 0).unwrap();
        assert_eq!(payload_end, boundary);
        assert_eq!(
            gossip_encoded_vec_payload_len_exact(values.iter()),
            Some(payload.len())
        );
        assert_eq!(
            decode_bounded_len_prefixed_sequence::<PayloadOnly>(&fields, 0).unwrap(),
            (values, boundary),
        );
        assert_eq!(fields[boundary], 0xee);

        // The invalid count must be rejected even though no element body follows it.
        let count = transaction_gossip_sequence_limit() as u64 + 1;
        let mut payload = Vec::new();
        ncore::write_seq_len(&mut payload, count).unwrap();
        let mut oversized = Vec::new();
        ncore::write_len(&mut oversized, payload.len() as u64).unwrap();
        oversized.extend_from_slice(&payload);
        assert!(matches!(
            decode_bounded_len_prefixed_sequence::<PayloadOnly>(&oversized, 0),
            Err(ncore::Error::SequenceLengthExceeded { length, limit })
                if length == count && limit + 1 == count
        ));
    }
}
