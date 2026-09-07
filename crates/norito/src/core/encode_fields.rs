//! Measurement and emission of packed struct fields.

use super::{
    Encoder, Error, NoritoSerialize, encoded_payload_len, mark_field_bitset_used_if_encoding,
    write_counted_payload, write_len_header, write_packed_offset_table,
};

/// A packed field whose payload is measured by the encoder that emits its header.
#[doc(hidden)]
#[derive(Clone, Copy)]
pub enum PackedField<'a> {
    /// A value encoded with its own canonical serializer.
    Value(&'a dyn NoritoSerialize),
    /// A byte array encoded directly, without the generic array codec's framing.
    Bytes(&'a [u8]),
}

/// Measure packed fields, emit their headers, and stream their validated payloads.
///
/// A bitset selects fields with an explicit size header. Without a bitset, all
/// fields are described by the canonical cumulative offset table. Measurements
/// come from serialization, never from caller-supplied lengths or length hints.
///
/// # Errors
///
/// Rejects a bitset with an incorrect byte count or nonzero unused bits. Returns
/// allocation, serialization, or I/O errors, including a payload that changes
/// its length between measurement and emission.
#[doc(hidden)]
#[inline(never)]
pub fn write_packed_fields(
    writer: &mut Encoder<'_>,
    fields: &[PackedField<'_>],
    bitset: Option<&[u8]>,
) -> Result<(), Error> {
    if let Some(bits) = bitset {
        if bits.len() != fields.len().div_ceil(8) {
            return Err(Error::LengthMismatch);
        }
        let tail = fields.len() % 8;
        if tail != 0 && bits.last().is_some_and(|byte| byte >> tail != 0) {
            return Err(Error::NonCanonicalEncoding);
        }
    }

    let allocation = fields
        .len()
        .checked_mul(core::mem::size_of::<usize>())
        .ok_or(Error::LengthMismatch)?;
    let allocation = u64::try_from(allocation).map_err(|_| Error::LengthMismatch)?;
    let mut lengths = Vec::new();
    lengths
        .try_reserve_exact(fields.len())
        .map_err(|_| Error::AllocationFailed { bytes: allocation })?;
    for field in fields {
        lengths.push(match field {
            PackedField::Value(value) => encoded_payload_len(*value)?,
            PackedField::Bytes(bytes) => bytes.len(),
        });
    }

    if let Some(bits) = bitset {
        mark_field_bitset_used_if_encoding();
        writer.write_all(bits)?;
        for (index, &length) in lengths.iter().enumerate() {
            if bits[index / 8] & (1 << (index % 8)) != 0 {
                write_len_header(
                    writer,
                    u64::try_from(length).map_err(|_| Error::LengthMismatch)?,
                )?;
            }
        }
    } else {
        write_packed_offset_table(writer, &lengths)?;
    }
    for (field, length) in fields.iter().zip(lengths) {
        match field {
            PackedField::Value(value) => write_counted_payload(*value, writer, length)?,
            PackedField::Bytes(bytes) => writer.write_all(bytes)?,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::core::{
        DecodeFlagsGuard, NoritoDeserialize, frame_bare_with_header_flags, from_bytes,
        header_flags, serialize_to_buffer,
    };

    struct CountedValue<'a> {
        visits: &'a Cell<usize>,
        bytes: &'a [u8],
    }

    impl NoritoSerialize for CountedValue<'_> {
        fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
            self.visits.set(self.visits.get() + 1);
            writer.write_all(self.bytes)?;
            Ok(())
        }

        fn encoded_len_hint(&self) -> Option<usize> {
            panic!("packed measurement must not consult length hints")
        }

        fn encoded_len_exact(&self) -> Option<usize> {
            panic!("packed measurement must not consult exact-length hints")
        }
    }

    struct Record<'a> {
        fields: &'a [PackedField<'a>],
        bitset: Option<&'a [u8]>,
    }

    impl NoritoSerialize for Record<'_> {
        fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
            write_packed_fields(writer, self.fields, self.bitset)
        }
    }

    fn layouts() -> [u8; 4] {
        [
            header_flags::PACKED_STRUCT,
            header_flags::PACKED_STRUCT | header_flags::COMPACT_LEN,
            header_flags::PACKED_STRUCT | header_flags::FIELD_BITSET | header_flags::COMPACT_LEN,
            header_flags::PACKED_STRUCT
                | header_flags::FIELD_BITSET
                | header_flags::COMPACT_LEN
                | header_flags::PACKED_SEQ,
        ]
    }

    fn bitset_for(flags: u8) -> Option<&'static [u8]> {
        (flags & header_flags::FIELD_BITSET != 0).then_some(&[0x05])
    }

    fn expected_header(flags: u8) -> Vec<u8> {
        if flags & header_flags::FIELD_BITSET != 0 {
            if flags & header_flags::COMPACT_LEN != 0 {
                vec![0x05, 2, 1]
            } else {
                [
                    vec![0x05],
                    2_u64.to_le_bytes().to_vec(),
                    1_u64.to_le_bytes().to_vec(),
                ]
                .concat()
            }
        } else {
            [0_u64, 2, 5, 6]
                .into_iter()
                .flat_map(u64::to_le_bytes)
                .collect()
        }
    }

    #[test]
    fn packed_fields_measure_each_child_once_and_preserve_headers() {
        for flags in layouts() {
            let _guard = DecodeFlagsGuard::enter(flags);
            let visits = Cell::new(0);
            let first = CountedValue {
                visits: &visits,
                bytes: &[0x11, 0x22],
            };
            let last = CountedValue {
                visits: &visits,
                bytes: &[0x33],
            };
            let fields = [
                PackedField::Value(&first),
                PackedField::Bytes(&[0xa0, 0xb0, 0xc0]),
                PackedField::Value(&last),
            ];
            let record = Record {
                fields: &fields,
                bitset: bitset_for(flags),
            };
            let mut expected = expected_header(flags);
            expected.extend_from_slice(&[0x11, 0x22, 0xa0, 0xb0, 0xc0, 0x33]);
            assert_eq!(encoded_payload_len(&record).unwrap(), expected.len());
            assert_eq!(
                visits.get(),
                2,
                "one measurement visit per value, flags {flags:#x}"
            );
            visits.set(0);
            let mut actual = Vec::new();
            serialize_to_buffer(&record, &mut actual).unwrap();
            assert_eq!(actual, expected, "flags {flags:#x}");
            assert_eq!(
                visits.get(),
                4,
                "real output still validates each measured value"
            );
        }
    }

    #[test]
    fn packed_fields_reject_invalid_bitsets_before_visiting_or_writing() {
        let visits = Cell::new(0);
        let value = CountedValue {
            visits: &visits,
            bytes: &[1],
        };
        let fields = [PackedField::Value(&value)];
        for bits in [&[][..], &[1, 0], &[2], &[0x81]] {
            let mut bytes = Vec::new();
            let error =
                write_packed_fields(&mut Encoder::for_buffer(&mut bytes), &fields, Some(bits))
                    .expect_err("invalid bitset");
            assert!(matches!(
                error,
                Error::LengthMismatch | Error::NonCanonicalEncoding
            ));
            assert!(bytes.is_empty());
            assert_eq!(visits.get(), 0);
        }
        let mut bytes = Vec::new();
        write_packed_fields(&mut Encoder::for_buffer(&mut bytes), &[], Some(&[])).unwrap();
        assert!(bytes.is_empty());
    }

    struct FailingValue;

    impl NoritoSerialize for FailingValue {
        fn serialize(&self, _writer: &mut Encoder<'_>) -> Result<(), Error> {
            Err(Error::NonCanonicalEncoding)
        }
    }

    #[test]
    fn packed_fields_propagate_measurement_errors_before_headers() {
        for bitset in [None, Some(&[1][..])] {
            let record = Record {
                fields: &[PackedField::Value(&FailingValue)],
                bitset,
            };
            assert!(matches!(
                encoded_payload_len(&record),
                Err(Error::NonCanonicalEncoding)
            ));
            let mut bytes = Vec::new();
            assert!(matches!(
                serialize_to_buffer(&record, &mut bytes),
                Err(Error::NonCanonicalEncoding)
            ));
            assert!(bytes.is_empty());
        }
    }

    struct ChangingValue {
        visits: Cell<usize>,
        grows: bool,
    }

    impl NoritoSerialize for ChangingValue {
        fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
            let first = self.visits.replace(self.visits.get() + 1) == 0;
            writer.write_all(if first == self.grows { &[1] } else { &[1, 2] })?;
            Ok(())
        }
    }

    #[test]
    fn packed_fields_reject_changed_lengths_on_real_output() {
        for flags in layouts() {
            let _guard = DecodeFlagsGuard::enter(flags);
            for grows in [false, true] {
                let value = ChangingValue {
                    visits: Cell::new(0),
                    grows,
                };
                let bitset = (flags & header_flags::FIELD_BITSET != 0).then_some(&[1][..]);
                let record = Record {
                    fields: &[PackedField::Value(&value)],
                    bitset,
                };
                let mut bytes = Vec::new();
                assert!(matches!(
                    serialize_to_buffer(&record, &mut bytes),
                    Err(Error::LengthMismatch)
                ));
                assert_eq!(value.visits.get(), 2);
                let header_len = if bitset.is_none() {
                    16
                } else if flags & header_flags::COMPACT_LEN != 0 {
                    2
                } else {
                    9
                };
                assert!(
                    bytes.len() <= header_len + usize::from(!grows),
                    "overrun bytes reached output"
                );
            }
        }
    }

    #[derive(crate::Encode, crate::Decode, Debug, PartialEq)]
    struct Wrapped {
        value: u16,
    }

    #[derive(crate::Encode, crate::Decode, Debug, PartialEq)]
    struct WithRaw {
        wrapped: Wrapped,
        raw: [u8; 3],
    }

    #[test]
    fn packed_derive_raw_arrays_keep_canonical_bytes_and_roundtrip() {
        let value = WithRaw {
            wrapped: Wrapped { value: 0x0102 },
            raw: [0xa0, 0xb0, 0xc0],
        };
        for flags in layouts() {
            let _guard = DecodeFlagsGuard::enter(flags);
            let mut expected = if flags & header_flags::FIELD_BITSET != 0 {
                let mut bytes = vec![1];
                if flags & header_flags::COMPACT_LEN != 0 {
                    bytes.push(3);
                } else {
                    bytes.extend_from_slice(&3_u64.to_le_bytes());
                }
                bytes.extend_from_slice(&[0, 2, 1]);
                bytes
            } else {
                [0_u64, 18, 21, 0, 2]
                    .into_iter()
                    .flat_map(u64::to_le_bytes)
                    .chain([2, 1])
                    .collect()
            };
            expected.extend_from_slice(&value.raw);
            let mut actual = Vec::new();
            serialize_to_buffer(&value, &mut actual).unwrap();
            assert_eq!(
                actual, expected,
                "raw array framing changed, flags {flags:#x}"
            );
            let frame = frame_bare_with_header_flags::<WithRaw>(&actual, flags).unwrap();
            let archived = from_bytes::<WithRaw>(&frame).unwrap();
            assert_eq!(WithRaw::try_deserialize(archived).unwrap(), value);
        }
    }
}
