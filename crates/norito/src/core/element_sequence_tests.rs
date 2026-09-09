//! Projected sequence items, iterator contracts, and checked output regressions.

use super::*;

struct CountedScalar<'a>(&'a u16, &'a Cell<usize>);

impl SerializePayload for CountedScalar<'_> {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        self.1.set(self.1.get() + 1);
        self.0.serialize(writer)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        panic!("projected sequence measurement must not trust a length hint")
    }
}

struct ProjectedEntry<'a>(&'a (u16, u16), &'a Cell<usize>);

impl SerializePayload for ProjectedEntry<'_> {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        write_len_prefixed(writer, &CountedScalar(&self.0.0, self.1))?;
        write_len_prefixed(writer, &CountedScalar(&self.0.1, self.1))
    }
}

struct ProjectedSequence<'a>(&'a [(u16, u16)], &'a Cell<usize>);

impl SerializePayload for ProjectedSequence<'_> {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        write_element_sequence::<ProjectedEntry<'_>, _>(
            writer,
            self.0.iter().map(|entry| ProjectedEntry(entry, self.1)),
            u64::MAX,
        )
    }
}

fn layouts() -> impl Iterator<Item = u8> {
    (0..=supported_header_flags()).filter(|flags| validate_header_flags(*flags).is_ok())
}

#[test]
fn projected_sequences_measure_each_leaf_once_and_preserve_tuple_wire() {
    let entries = [(0x1234, 0x5678), (0x9abc, 0xdef0)];
    for flags in layouts() {
        let _flags = DecodeFlagsGuard::enter(flags);
        let visits = Cell::new(0);
        let projected = ProjectedSequence(&entries, &visits);
        let measured = encoded_payload_len(&projected).unwrap();
        assert_eq!(visits.get(), entries.len() * 2, "flags {flags:#x}");
        visits.set(0);
        let mut actual = Vec::new();
        serialize_to_buffer(&projected, &mut actual).unwrap();
        assert_eq!(measured, actual.len());
        assert_eq!(visits.get(), entries.len() * 6, "flags {flags:#x}");
        let mut expected = Vec::new();
        serialize_to_buffer(&entries.to_vec(), &mut expected).unwrap();
        assert_eq!(actual, expected, "tuple sequence flags {flags:#x}");

        // An enclosing field must preserve the counting destination all the
        // way through ephemeral entry views and their length-framed children.
        let parent = Some(Box::new(projected));
        visits.set(0);
        let parent_len = encoded_payload_len(&parent).unwrap();
        assert_eq!(visits.get(), entries.len() * 2, "nested flags {flags:#x}");
        let mut parent_bytes = Vec::new();
        serialize_to_buffer(&parent, &mut parent_bytes).unwrap();
        assert_eq!(parent_len, parent_bytes.len());
        let mut reference = Vec::new();
        serialize_to_buffer(&Some(Box::new(entries.to_vec())), &mut reference).unwrap();
        assert_eq!(parent_bytes, reference, "nested tuple flags {flags:#x}");
    }
}

#[derive(Clone)]
struct WrongCount<'a> {
    remaining: &'a [u16],
    reported: usize,
}

impl<'a> Iterator for WrongCount<'a> {
    type Item = &'a u16;

    fn next(&mut self) -> Option<Self::Item> {
        let (first, rest) = self.remaining.split_first()?;
        self.remaining = rest;
        self.reported = self.reported.saturating_sub(1);
        Some(first)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.reported, Some(self.reported))
    }
}

impl ExactSizeIterator for WrongCount<'_> {}

#[test]
fn element_sequences_reject_wrong_reported_cardinality_in_both_destinations() {
    let items = [0x1234_u16, 0x5678];
    for flags in layouts() {
        let _flags = DecodeFlagsGuard::enter(flags);
        for (actual, reported) in [(0, 1), (1, 0), (1, 2), (2, 1)] {
            let iter = WrongCount {
                remaining: &items[..actual],
                reported,
            };
            let mut counter = LengthCountingWriter::default();
            assert!(matches!(
                write_element_sequence::<u16, _>(
                    &mut Encoder::for_counting(&mut counter),
                    iter.clone(),
                    u64::MAX,
                ),
                Err(Error::LengthMismatch)
            ));
            let mut bytes = Vec::new();
            assert!(matches!(
                write_element_sequence::<u16, _>(
                    &mut Encoder::for_buffer(&mut bytes),
                    iter,
                    u64::MAX,
                ),
                Err(Error::LengthMismatch)
            ));
            let mut expected = u64::try_from(reported).unwrap().to_le_bytes().to_vec();
            if !use_packed_seq() {
                for item in items.iter().take(actual.min(reported)) {
                    write_len_with_flags(&mut expected, 2, flags).unwrap();
                    expected.extend_from_slice(&item.to_le_bytes());
                }
            }
            assert_eq!(
                bytes, expected,
                "actual {actual}, reported {reported}, {flags:#x}"
            );
        }
    }
}

struct DifferentClone<'a> {
    current: WrongCount<'a>,
    cloned: &'a [u16],
}

impl Clone for DifferentClone<'_> {
    fn clone(&self) -> Self {
        Self {
            current: WrongCount {
                remaining: self.cloned,
                reported: self.current.reported,
            },
            cloned: self.cloned,
        }
    }
}

impl<'a> Iterator for DifferentClone<'a> {
    type Item = &'a u16;

    fn next(&mut self) -> Option<Self::Item> {
        self.current.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.current.size_hint()
    }
}

impl ExactSizeIterator for DifferentClone<'_> {}

#[test]
fn packed_sequences_reject_clone_cardinality_changes_in_measurement_and_emission() {
    let items = [0x1234_u16, 0x5678];
    for flags in layouts().filter(|flags| flags & header_flags::PACKED_SEQ != 0) {
        let _flags = DecodeFlagsGuard::enter(flags);
        for (original, cloned, reported) in [(2, 1, 2), (1, 2, 2), (2, 1, 1), (1, 2, 1)] {
            let make_iter = || DifferentClone {
                current: WrongCount {
                    remaining: &items[..original],
                    reported,
                },
                cloned: &items[..cloned],
            };
            let mut counter = LengthCountingWriter::default();
            assert!(matches!(
                write_element_sequence::<u16, _>(
                    &mut Encoder::for_counting(&mut counter),
                    make_iter(),
                    u64::MAX,
                ),
                Err(Error::LengthMismatch)
            ));
            let mut bytes = Vec::new();
            assert!(matches!(
                write_element_sequence::<u16, _>(
                    &mut Encoder::for_buffer(&mut bytes),
                    make_iter(),
                    u64::MAX,
                ),
                Err(Error::LengthMismatch)
            ));
            let mut expected = u64::try_from(reported).unwrap().to_le_bytes().to_vec();
            if cloned == reported {
                for offset in 0..=reported {
                    expected.extend_from_slice(&u64::try_from(offset * 2).unwrap().to_le_bytes());
                }
                for item in items.iter().take(original.min(reported)) {
                    expected.extend_from_slice(&item.to_le_bytes());
                }
            }
            assert_eq!(
                bytes, expected,
                "original {original}, clone {cloned}, reported {reported}"
            );
        }
    }
}

struct ChangingPayload<'a> {
    first: &'a [u8],
    second: &'a [u8],
    visits: Cell<usize>,
    fail_on: Option<usize>,
}

impl SerializePayload for ChangingPayload<'_> {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        let visit = self.visits.get() + 1;
        self.visits.set(visit);
        if self.fail_on == Some(visit) {
            return Err(Error::NonCanonicalEncoding);
        }
        for byte in if visit == 1 { self.first } else { self.second } {
            writer.write_all(std::slice::from_ref(byte))?;
        }
        Ok(())
    }
}

struct PayloadView<'a>(&'a ChangingPayload<'a>);

impl SerializePayload for PayloadView<'_> {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        self.0.serialize(writer)
    }
}

fn element_prefix(length: usize, flags: u8) -> Vec<u8> {
    let mut prefix = 1_u64.to_le_bytes().to_vec();
    if flags & header_flags::PACKED_SEQ != 0 {
        prefix.extend_from_slice(&0_u64.to_le_bytes());
        prefix.extend_from_slice(&u64::try_from(length).unwrap().to_le_bytes());
    } else {
        write_len_with_flags(&mut prefix, u64::try_from(length).unwrap(), flags).unwrap();
    }
    prefix
}

#[test]
fn projected_sequences_reject_real_output_growth_and_shrinkage() {
    let payload = [0x11, 0x22];
    for flags in layouts() {
        let _flags = DecodeFlagsGuard::enter(flags);
        for (first, second) in [(1, 2), (2, 1)] {
            let item = ChangingPayload {
                first: &payload[..first],
                second: &payload[..second],
                visits: Cell::new(0),
                fail_on: None,
            };
            let mut bytes = Vec::with_capacity(32);
            let capacity = bytes.capacity();
            assert!(matches!(
                write_element_sequence::<PayloadView<'_>, _>(
                    &mut Encoder::for_buffer(&mut bytes),
                    std::iter::once(&item).map(PayloadView),
                    u64::MAX,
                ),
                Err(Error::LengthMismatch)
            ));
            assert_eq!(item.visits.get(), 2);
            let mut expected = element_prefix(first, flags);
            expected.extend_from_slice(&payload[..first.min(second)]);
            assert_eq!(
                bytes, expected,
                "growth or shrinkage escaped bounds, {flags:#x}"
            );
            assert_eq!(bytes.capacity(), capacity);
        }
    }
}

#[test]
fn projected_sequences_preserve_measurement_and_emission_errors() {
    for flags in layouts() {
        let _flags = DecodeFlagsGuard::enter(flags);
        for fail_on in [1, 2] {
            let item = ChangingPayload {
                first: &[0x11],
                second: &[0x11],
                visits: Cell::new(0),
                fail_on: Some(fail_on),
            };
            let mut bytes = Vec::new();
            assert!(matches!(
                write_element_sequence::<PayloadView<'_>, _>(
                    &mut Encoder::for_buffer(&mut bytes),
                    std::iter::once(&item).map(PayloadView),
                    u64::MAX,
                ),
                Err(Error::NonCanonicalEncoding)
            ));
            assert_eq!(item.visits.get(), fail_on);
            assert_eq!(
                bytes,
                if fail_on == 1 {
                    1_u64.to_le_bytes().to_vec()
                } else {
                    element_prefix(1, flags)
                },
                "child error changed partial output, flags {flags:#x}"
            );
        }
    }
}

#[test]
fn projected_sequences_apply_packed_limits_before_visiting_or_writing_payloads() {
    let items = [0x1234_u16, 0x5678];
    for flags in layouts().filter(|flags| flags & header_flags::PACKED_SEQ != 0) {
        let _flags = DecodeFlagsGuard::enter(flags);
        for limit in [23, 27, 28] {
            let projected = Cell::new(0);
            let visits = Cell::new(0);
            let mut bytes = Vec::new();
            let result = write_element_sequence::<CountedScalar<'_>, _>(
                &mut Encoder::for_buffer(&mut bytes),
                items.iter().map(|value| {
                    projected.set(projected.get() + 1);
                    CountedScalar(value, &visits)
                }),
                limit,
            );
            if limit == 28 {
                result.unwrap();
                assert_eq!(bytes.len(), 8 + 28);
                assert_eq!(projected.get(), 4);
                assert_eq!(visits.get(), 4);
            } else {
                assert!(matches!(
                    result,
                    Err(Error::ArchiveLengthExceeded { length, limit: actual })
                        if actual == limit && length == if limit == 23 { 24 } else { 28 }
                ));
                assert_eq!(bytes, 2_u64.to_le_bytes());
                let expected_visits = if limit == 23 { 0 } else { 2 };
                assert_eq!(projected.get(), expected_visits);
                assert_eq!(visits.get(), expected_visits);
            }
        }
    }
}

#[test]
fn generalized_payload_helpers_preserve_map_columns_and_counting() {
    let entries = [(0x1234_u16, 0x5678_u16), (0x9abc, 0xdef0)];
    for flags in layouts() {
        let _flags = DecodeFlagsGuard::enter(flags);
        let visits = Cell::new(0);
        let map = entries
            .iter()
            .map(|(key, value)| (*key, CountedScalar(value, &visits)))
            .collect::<BTreeMap<_, _>>();
        let length = encoded_payload_len(&map).unwrap();
        assert_eq!(visits.get(), entries.len());
        visits.set(0);
        let mut actual = Vec::new();
        serialize_to_buffer(&map, &mut actual).unwrap();
        assert_eq!(visits.get(), entries.len() * 2);
        assert_eq!(length, actual.len());
        let mut expected = 2_u64.to_le_bytes().to_vec();
        if use_packed_seq() {
            for _ in 0..2 {
                for offset in [0_u64, 2, 4] {
                    expected.extend_from_slice(&offset.to_le_bytes());
                }
            }
            for (key, _) in entries {
                expected.extend_from_slice(&key.to_le_bytes());
            }
            for (_, value) in entries {
                expected.extend_from_slice(&value.to_le_bytes());
            }
        } else {
            for (key, value) in entries {
                for field in [key, value] {
                    write_len_with_flags(&mut expected, 2, flags).unwrap();
                    expected.extend_from_slice(&field.to_le_bytes());
                }
            }
        }
        assert_eq!(actual, expected, "map columns changed for flags {flags:#x}");
    }
}
