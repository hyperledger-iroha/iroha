//! Exact child scopes exercised with the real Norito codec and destinations.

use super::*;
use std::cell::Cell;
use std::collections::VecDeque;
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};

struct Bytes<'a>(&'a [u8]);
impl SerializePayload for Bytes<'_> {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        writer.write_all(self.0)?;
        Ok(())
    }
}
enum Step {
    Accept(usize),
    Interrupt,
    Error,
    Zero,
    Overreport,
}
#[derive(Default)]
struct Destination {
    bytes: Vec<u8>,
    steps: VecDeque<Step>,
    calls: usize,
    flushes: usize,
}
impl Write for Destination {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.calls += 1;
        let amount = match self.steps.pop_front() {
            None => bytes.len(),
            Some(Step::Accept(amount)) => amount.min(bytes.len()),
            Some(Step::Interrupt) => return Err(io::ErrorKind::Interrupted.into()),
            Some(Step::Error) => return Err(io::ErrorKind::BrokenPipe.into()),
            Some(Step::Zero) => return Ok(0),
            Some(Step::Overreport) => return Ok(bytes.len() + 1),
        };
        self.bytes.extend_from_slice(&bytes[..amount]);
        Ok(amount)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.flushes += 1;
        Ok(())
    }
}

#[test]
fn buffer_append_scopes_use_relative_actual_offsets_and_zero_lengths() {
    let mut out = vec![99];
    let mut encoder = Encoder::for_buffer(&mut out);
    encoder.write_all(&[1]).unwrap();
    encoder
        .with_exact_length(3, |e| {
            e.with_exact_length(0, |e| {
                e.write_all(&[])?;
                Ok(())
            })?;
            e.with_exact_length(2, |e| {
                e.write_all(&[2, 3])?;
                Ok(())
            })?;
            e.write_all(&[4])?;
            Ok(())
        })
        .unwrap();
    encoder.write_all(&[5]).unwrap();
    assert_eq!(out, [99, 1, 2, 3, 4, 5]);
}

struct Changes<'a> {
    calls: &'a Cell<usize>,
    first: &'a [u8],
    second: &'a [u8],
    suppress: bool,
}
impl SerializePayload for Changes<'_> {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        let call = self.calls.get();
        self.calls.set(call + 1);
        let bytes = if call == 0 { self.first } else { self.second };
        for byte in bytes {
            let result = writer.write_all(&[*byte]);
            if !self.suppress {
                result?;
            }
        }
        Ok(())
    }
}
#[test]
fn second_pass_growth_is_rejected_before_forwarding_even_if_suppressed() {
    for suppress in [false, true] {
        let calls = Cell::new(0);
        let value = Changes {
            calls: &calls,
            first: &[1],
            second: &[1, 2],
            suppress,
        };
        let measured = encoded_payload_len(&value).unwrap();
        assert_eq!(measured, 1);
        let mut out = Vec::with_capacity(8);
        assert!(matches!(
            write_counted_payload(&value, &mut Encoder::for_buffer(&mut out), measured),
            Err(Error::LengthMismatch)
        ));
        assert_eq!(out, [1]);
        assert_eq!(calls.get(), 2);
    }
}
#[test]
fn second_pass_shrink_fails_exact_equality() {
    let calls = Cell::new(0);
    let value = Changes {
        calls: &calls,
        first: &[1, 2],
        second: &[1],
        suppress: false,
    };
    let measured = encoded_payload_len(&value).unwrap();
    let mut out = Vec::new();
    assert!(matches!(
        write_counted_payload(&value, &mut Encoder::for_buffer(&mut out), measured),
        Err(Error::LengthMismatch)
    ));
    assert_eq!(out, [1]);
}
#[test]
fn zero_length_scope_rejects_nonempty_slice_without_forwarding() {
    let mut out = Vec::new();
    let mut encoder = Encoder::for_buffer(&mut out);
    assert!(matches!(
        encoder.with_exact_length(0, |e| {
            let _ = e.write_all(&[1]);
            Ok(())
        }),
        Err(Error::LengthMismatch)
    ));
    assert!(encoder.write_all(&[]).is_err());
    assert!(out.is_empty());
}
#[test]
fn smallest_enclosing_bound_wins_and_overrun_stays_sticky() {
    let mut destination = Destination::default();
    let mut encoder = Encoder::new(&mut destination);
    assert!(matches!(
        encoder.with_exact_length(2, |e| {
            let error = e.with_exact_length(3, |e| {
                let _ = e.write_all(&[1, 2, 3]);
                Ok(())
            });
            assert!(matches!(error, Err(Error::LengthMismatch)));
            assert!(e.write_all(&[9]).is_err());
            Ok(())
        }),
        Err(Error::LengthMismatch)
    ));
    assert_eq!(destination.calls, 0);
    assert!(destination.bytes.is_empty());
}
#[test]
fn child_overrun_poison_survives_restored_larger_parent_bound() {
    let mut out = Vec::new();
    let mut encoder = Encoder::for_buffer(&mut out);
    assert!(matches!(
        encoder.with_exact_length(3, |e| {
            let _ = e.with_exact_length(1, |e| {
                e.write_all(&[1])?;
                let _ = e.write_all(&[2]);
                Ok(())
            });
            assert!(e.write_all(&[3, 4]).is_err());
            Ok(())
        }),
        Err(Error::LengthMismatch)
    ));
    assert_eq!(out, [1]);
}
#[test]
fn panic_restores_bound_and_keeps_successfully_written_prefix() {
    let mut out = Vec::new();
    Encoder::for_buffer(&mut out)
        .with_exact_length(3, |e| {
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _ = e.with_exact_length(1, |e| {
                        e.write_all(&[1])?;
                        panic!("scope panic")
                    });
                }))
                .is_err()
            );
            e.with_exact_length(2, |e| {
                e.write_all(&[2, 3])?;
                Ok(())
            })
        })
        .unwrap();
    assert_eq!(out, [1, 2, 3]);
}
#[test]
fn panic_does_not_clear_an_observed_overrun() {
    let mut out = Vec::new();
    let result = Encoder::for_buffer(&mut out).with_exact_length(3, |e| {
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _ = e.with_exact_length(1, |e| {
                    let _ = e.write_all(&[1, 2]);
                    panic!("after overrun")
                });
            }))
            .is_err()
        );
        assert!(e.write_all(&[3]).is_err());
        Ok(())
    });
    assert!(matches!(result, Err(Error::LengthMismatch)));
    assert!(out.is_empty());
}
#[test]
fn serializer_error_restores_bound_without_forgiving_underflow() {
    let mut out = Vec::new();
    let mut encoder = Encoder::for_buffer(&mut out);
    encoder
        .with_exact_length(3, |e| {
            let result = e.with_exact_length(1, |e| {
                e.write_all(&[1])?;
                Err(Error::NonCanonicalEncoding)
            });
            assert!(matches!(result, Err(Error::NonCanonicalEncoding)));
            e.write_all(&[2, 3])?;
            Ok(())
        })
        .unwrap();
    assert!(matches!(
        encoder.with_exact_length(1, |_| Ok(())),
        Err(Error::LengthMismatch)
    ));
    assert_eq!(out, [1, 2, 3]);
}
#[test]
fn partial_writes_and_interrupted_retry_count_only_actual_bytes() {
    let mut destination = Destination {
        steps: [Step::Interrupt, Step::Accept(1), Step::Accept(1)].into(),
        ..Default::default()
    };
    Encoder::new(&mut destination)
        .with_exact_length(3, |e| {
            e.write_all(&[1, 2, 3])?;
            e.flush()?;
            Ok(())
        })
        .unwrap();
    assert_eq!(destination.bytes, [1, 2, 3]);
    assert_eq!(destination.calls, 4);
    assert_eq!(destination.flushes, 1);
}
#[test]
fn partial_write_then_error_preserves_prefix_accounting_and_parent_bound() {
    let mut destination = Destination {
        steps: [Step::Accept(2), Step::Error].into(),
        ..Default::default()
    };
    Encoder::new(&mut destination).with_exact_length(4, |e| {
        let result = e.with_exact_length(3, |e| { e.write_all(b"abc")?; Ok(()) });
        assert!(matches!(result, Err(Error::Io(ref error)) if error.kind() == io::ErrorKind::BrokenPipe));
        e.with_exact_length(2, |e| { e.write_all(b"XY")?; Ok(()) })
    }).unwrap();
    assert_eq!(destination.bytes, b"abXY");
}
#[test]
fn suppressed_partial_error_cannot_fake_exact_completion() {
    let mut destination = Destination {
        steps: [Step::Accept(2), Step::Error].into(),
        ..Default::default()
    };
    assert!(matches!(
        Encoder::new(&mut destination).with_exact_length(3, |e| {
            let _ = e.write_all(b"abc");
            Ok(())
        }),
        Err(Error::LengthMismatch)
    ));
    assert_eq!(destination.bytes, b"ab");
}
#[test]
fn zero_write_is_error_and_contract_violating_overreport_is_sticky() {
    let mut zero = Destination {
        steps: [Step::Zero].into(),
        ..Default::default()
    };
    assert!(
        matches!(Encoder::new(&mut zero).with_exact_length(1, |e| { e.write_all(&[1])?; Ok(()) }), Err(Error::Io(ref error)) if error.kind() == io::ErrorKind::WriteZero)
    );
    let mut liar = Destination {
        steps: [Step::Overreport].into(),
        ..Default::default()
    };
    assert!(matches!(
        Encoder::new(&mut liar).with_exact_length(1, |e| {
            let _ = e.write_all(&[1]);
            Ok(())
        }),
        Err(Error::LengthMismatch)
    ));
    assert!(liar.bytes.is_empty());
}
#[test]
fn direct_field_remains_one_contiguous_destination_write() {
    let bytes = [0x5a; 1536];
    let mut destination = Destination::default();
    write_counted_payload(
        &Bytes(&bytes),
        &mut Encoder::new(&mut destination),
        bytes.len(),
    )
    .unwrap();
    assert_eq!(destination.bytes, bytes);
    assert_eq!(destination.calls, 1);
}
#[test]
fn total_offset_overflow_and_count_overflow_remain_sticky() {
    let mut counter = LengthCountingWriter::default();
    {
        let mut encoder = Encoder::for_counting(&mut counter);
        assert!(encoder.count_measured_bytes(usize::MAX).unwrap());
        assert!(encoder.count_measured_bytes(1).is_err());
        assert!(encoder.count_measured_bytes(0).is_err());
        assert!(encoder.write_all(&[]).is_err());
    }
    assert!(matches!(counter.finish(), Err(Error::LengthMismatch)));
    let mut counter = LengthCountingWriter::default();
    counter.add(usize::MAX - 1).unwrap();
    assert!(
        Encoder::for_counting(&mut counter)
            .count_measured_bytes(2)
            .is_err()
    );
    assert!(matches!(counter.finish(), Err(Error::LengthMismatch)));
}
#[test]
fn exact_scope_end_overflow_rejects_without_invoking_child() {
    let mut counter = LengthCountingWriter::default();
    let mut encoder = Encoder::for_counting(&mut counter);
    let result = encoder.with_exact_length(usize::MAX, |encoder| {
        encoder.count_measured_bytes(1).unwrap();
        assert!(matches!(
            encoder.with_exact_length(usize::MAX, |_| panic!("unrepresentable child called")),
            Err(Error::LengthMismatch)
        ));
        assert!(encoder.write_all(&[]).is_err());
        Ok(())
    });
    assert!(matches!(result, Err(Error::LengthMismatch)));
}
#[test]
fn trusted_counting_measured_bytes_still_respect_an_active_bound() {
    let mut counter = LengthCountingWriter::default();
    assert!(matches!(
        Encoder::for_counting(&mut counter).with_exact_length(1, |e| {
            let _ = e.count_measured_bytes(2);
            Ok(())
        }),
        Err(Error::LengthMismatch)
    ));
    assert_eq!(counter.len, 0);
}
#[test]
fn public_exact_api_validates_actual_bytes_even_over_counting_encoder() {
    let mut counter = LengthCountingWriter::default();
    {
        let mut encoder = Encoder::for_counting(&mut counter);
        assert!(matches!(
            serialize_to_writer_exact(&Bytes(&[1, 2]), &mut encoder, 1),
            Err(Error::LengthMismatch)
        ));
        serialize_to_writer_exact(&Bytes(&[3, 4]), &mut encoder, 2).unwrap();
    }
    assert_eq!(counter.finish().unwrap(), 2);
}
#[test]
fn public_exact_api_preserves_external_writer_and_comparison_bytes() {
    let mut destination = Destination::default();
    serialize_to_writer_exact(&Bytes(b"abc"), &mut destination, 3).unwrap();
    assert_eq!(destination.bytes, b"abc");
    let mut comparison = ExactSliceWriter::new(b"abc");
    serialize_to_writer_exact(&Bytes(b"abc"), &mut comparison, 3).unwrap();
    assert!(comparison.is_complete());
}

struct Nested<'a> {
    depth: usize,
    bytes: &'a [u8],
}
impl SerializePayload for Nested<'_> {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        if self.depth == 0 {
            return Bytes(self.bytes).serialize(writer);
        }
        let child = Nested {
            depth: self.depth - 1,
            bytes: self.bytes,
        };
        let measured = encoded_payload_len(&child)?;
        writer.write_all(&(measured as u64).to_le_bytes())?;
        write_counted_payload(&child, writer, measured)
    }
}
fn decode_nested(mut bytes: &[u8], depth: usize) -> &[u8] {
    for _ in 0..depth {
        let length = u64::from_le_bytes(bytes[..8].try_into().unwrap()) as usize;
        bytes = &bytes[8..];
        assert_eq!(bytes.len(), length);
    }
    bytes
}
#[test]
fn nested_bytes_match_golden_across_destinations_and_decode_component_fixture() {
    for depth in [0, 1, 2, 8, 32] {
        let payload = Nested {
            depth,
            bytes: &[0x12, 0x34, 0x56],
        };
        let mut golden = Vec::new();
        for remaining in (0..depth).rev() {
            golden.extend_from_slice(&(3_u64 + 8 * remaining as u64).to_le_bytes());
        }
        golden.extend_from_slice(payload.bytes);
        assert_eq!(encoded_payload_len(&payload).unwrap(), golden.len());
        let mut buffer = Vec::new();
        payload
            .serialize(&mut Encoder::for_buffer(&mut buffer))
            .unwrap();
        assert_eq!(buffer, golden);
        let mut byte_sink = ByteSink::with_headroom(0, 0);
        payload
            .serialize(&mut Encoder::for_byte_sink(&mut byte_sink))
            .unwrap();
        assert_eq!(byte_sink.checksum(), crc64(&golden));
        assert_eq!(byte_sink.into_inner(), golden);
        let mut destination = Destination::default();
        serialize_to_writer_exact(&payload, &mut destination, golden.len()).unwrap();
        assert_eq!(destination.bytes, golden);
        assert_eq!(decode_nested(&buffer, depth), payload.bytes);
    }
}
#[test]
fn all_nested_exact_scopes_use_the_same_encoder_object() {
    fn descend(encoder: &mut Encoder<'_>, depth: usize, identity: usize) -> Result<(), Error> {
        assert_eq!(encoder as *mut Encoder<'_> as usize, identity);
        if depth == 0 {
            encoder.write_all(&[7])?;
            return Ok(());
        }
        encoder.with_exact_length(1, |encoder| descend(encoder, depth - 1, identity))
    }
    let mut out = Vec::new();
    let mut encoder = Encoder::for_buffer(&mut out);
    let identity = &mut encoder as *mut Encoder<'_> as usize;
    descend(&mut encoder, 64, identity).unwrap();
    assert_eq!(out, [7]);
}

#[test]
fn unbounded_bytes_before_between_and_after_scopes_do_not_shift_limits() {
    let mut out = Vec::new();
    let mut encoder = Encoder::for_buffer(&mut out);
    encoder.write_all(b"before").unwrap();
    encoder
        .with_exact_length(2, |e| {
            e.write_all(b"AB")?;
            Ok(())
        })
        .unwrap();
    encoder.write_all(b"between").unwrap();
    encoder
        .with_exact_length(1, |e| {
            e.write_all(b"C")?;
            Ok(())
        })
        .unwrap();
    encoder.write_all(b"after").unwrap();
    encoder.with_exact_length(0, |_| Ok(())).unwrap();
    assert_eq!(out, b"beforeABbetweenCafter");
}

#[test]
fn unbounded_writer_delegation_resumes_after_scoped_partial_accounting() {
    #[derive(Default)]
    struct Specialized {
        bytes: Vec<u8>,
        all: usize,
        partial: usize,
    }
    impl Write for Specialized {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.partial += 1;
            let n = bytes.len().min(1);
            self.bytes.extend_from_slice(&bytes[..n]);
            Ok(n)
        }
        fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
            self.all += 1;
            self.bytes.extend_from_slice(bytes);
            Ok(())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut destination = Specialized::default();
    let mut encoder = Encoder::new(&mut destination);
    encoder.write_all(b"before").unwrap();
    encoder
        .with_exact_length(2, |e| {
            e.write_all(b"AB")?;
            Ok(())
        })
        .unwrap();
    encoder.write_all(b"between").unwrap();
    encoder
        .with_exact_length(2, |e| {
            e.write_all(b"CD")?;
            Ok(())
        })
        .unwrap();
    encoder.write_all(b"after").unwrap();
    assert_eq!(destination.bytes, b"beforeABbetweenCDafter");
    assert_eq!(destination.all, 3);
    assert_eq!(destination.partial, 4);
}

#[test]
fn inactive_counter_uses_cumulative_counter_without_stale_scope_offsets() {
    let mut counter = LengthCountingWriter::default();
    {
        let mut encoder = Encoder::for_counting(&mut counter);
        encoder.count_measured_bytes(7).unwrap();
        encoder
            .with_exact_length(3, |e| {
                e.count_measured_bytes(3)?;
                Ok(())
            })
            .unwrap();
        encoder.count_measured_bytes(11).unwrap();
        encoder
            .with_exact_length(2, |e| {
                e.count_measured_bytes(2)?;
                Ok(())
            })
            .unwrap();
        encoder.count_measured_bytes(5).unwrap();
    }
    assert_eq!(counter.finish().unwrap(), 28);
}

#[test]
fn outermost_panic_and_error_restore_inactive_destination_without_erasing_overrun() {
    let mut out = Vec::new();
    let mut encoder = Encoder::for_buffer(&mut out);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = encoder.with_exact_length(1, |e| {
                e.write_all(b"A")?;
                panic!("outermost scope")
            });
        }))
        .is_err()
    );
    encoder.write_all(b"between").unwrap();
    assert!(matches!(
        encoder.with_exact_length(1, |e| {
            e.write_all(b"B")?;
            Err(Error::NonCanonicalEncoding)
        }),
        Err(Error::NonCanonicalEncoding)
    ));
    encoder
        .with_exact_length(1, |e| {
            e.write_all(b"C")?;
            Ok(())
        })
        .unwrap();
    assert!(matches!(
        encoder.with_exact_length(0, |e| {
            let _ = e.write_all(b"D");
            Ok(())
        }),
        Err(Error::LengthMismatch)
    ));
    assert!(encoder.write_all(b"after").is_err());
    assert_eq!(out, b"AbetweenBC");
}

#[test]
fn actual_nested_codec_preserves_layout_checksum_streaming_and_roundtrip() {
    let value = vec![vec![0x1234_u16, 0xabcd], Vec::new(), vec![0x9876]];
    for flags in [0, header_flags::COMPACT_LEN] {
        let _flags = DecodeFlagsGuard::enter(flags);
        let measured = encoded_payload_len(&value).expect("measure actual nested value");
        let mut payload = Vec::new();
        serialize_to_buffer(&value, &mut payload).expect("buffer actual nested value");
        assert_eq!(payload.len(), measured);
        let mut sink = ByteSink::with_headroom(0, 0);
        value
            .serialize(&mut Encoder::for_byte_sink(&mut sink))
            .unwrap();
        assert_eq!(sink.checksum(), crc64(&payload));
        assert_eq!(sink.into_inner(), payload);
        let canonical = to_bytes(&value).expect("canonical frame");
        let mut streamed = Vec::new();
        write_frame_to_writer(&value, &mut streamed).expect("stream actual frame");
        assert_eq!(
            streamed, canonical,
            "streaming parity for layout {flags:#x}"
        );
        let header = Header::read(&mut &canonical[..]).unwrap();
        let start = Header::SIZE + payload_alignment_padding_for::<Vec<Vec<u16>>>();
        assert_eq!(header.checksum, crc64(&canonical[start..]));
        let view = from_bytes_view(&canonical).expect("validate actual frame");
        assert_eq!(view.decode_exact::<Vec<Vec<u16>>>().unwrap(), value);
    }
}

#[test]
fn actual_nested_sequence_matches_explicit_v1_wire_vectors() {
    let value = vec![vec![0x1234_u16, 0xabcd], Vec::new(), vec![0x9876]];
    for flags in [0, header_flags::COMPACT_LEN] {
        // This oracle writes the V1 specification directly, independent of the
        // codec's length helpers, scopes, counting pass and generic writers.
        let prefix = |out: &mut Vec<u8>, length: u8| {
            if flags == 0 {
                out.extend_from_slice(&u64::from(length).to_le_bytes());
            } else {
                out.push(length);
            }
        };
        let mut expected = 3_u64.to_le_bytes().to_vec();
        for words in [&[0x1234_u16, 0xabcd][..], &[][..], &[0x9876][..]] {
            let mut child = (words.len() as u64).to_le_bytes().to_vec();
            for word in words {
                prefix(&mut child, 2);
                child.extend_from_slice(&word.to_le_bytes());
            }
            prefix(&mut expected, u8::try_from(child.len()).unwrap());
            expected.extend_from_slice(&child);
        }
        let _layout = DecodeFlagsGuard::enter(flags);
        let mut actual = Vec::new();
        serialize_to_buffer(&value, &mut actual).unwrap();
        assert_eq!(actual, expected, "layout {flags:#x}");
        let frame = to_bytes(&value).unwrap();
        let start = Header::SIZE + payload_alignment_padding_for::<Vec<Vec<u16>>>();
        assert_eq!(&frame[start..], expected);
    }
}

#[test]
fn actual_generated_containers_never_consult_length_hints() {
    struct PoisonHint;
    impl SerializePayload for PoisonHint {
        fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
            writer.write_all(b"abc")?;
            Ok(())
        }
        fn encoded_len_hint(&self) -> Option<usize> {
            panic!("estimated length must not be trusted")
        }
        fn encoded_len_exact(&self) -> Option<usize> {
            panic!("claimed exact length must not be trusted")
        }
    }
    #[derive(crate::SerializePayload)]
    struct Generated<T> {
        body: T,
    }
    for flags in [0, header_flags::COMPACT_LEN] {
        let _layout = DecodeFlagsGuard::enter(flags);
        let value = Generated {
            body: vec![Some(Box::new((PoisonHint, 7_u16)))],
        };
        let measured = encoded_payload_len(&value).unwrap();
        let mut actual = Vec::new();
        serialize_to_buffer(&value, &mut actual).unwrap();
        assert_eq!(actual.len(), measured);
        let mut compared = ExactSliceWriter::new(&actual);
        serialize_to_writer(&value, &mut compared).unwrap();
        assert!(compared.is_complete());
    }
}

#[test]
fn actual_sequence_rejects_second_pass_growth_even_when_leaf_suppresses_error() {
    for flags in [0, header_flags::COMPACT_LEN] {
        for suppress in [false, true] {
            let _layout = DecodeFlagsGuard::enter(flags);
            let calls = Cell::new(0);
            let value = vec![Changes {
                calls: &calls,
                first: &[1],
                second: &[1, 2],
                suppress,
            }];
            let mut actual = Vec::new();
            assert!(matches!(
                serialize_to_buffer(&value, &mut actual),
                Err(Error::LengthMismatch)
            ));
            let mut expected = 1_u64.to_le_bytes().to_vec();
            if flags == 0 {
                expected.extend_from_slice(&1_u64.to_le_bytes());
            } else {
                expected.push(1);
            }
            expected.push(1);
            assert_eq!(actual, expected, "the second leaf byte was never emitted");
            assert_eq!(calls.get(), 2);
        }
    }
}

#[test]
fn actual_sequence_rejects_second_pass_shrink() {
    for flags in [0, header_flags::COMPACT_LEN] {
        let _layout = DecodeFlagsGuard::enter(flags);
        let calls = Cell::new(0);
        let value = vec![Changes {
            calls: &calls,
            first: &[1, 2],
            second: &[1],
            suppress: false,
        }];
        assert!(matches!(
            serialize_to_buffer(&value, &mut Vec::new()),
            Err(Error::LengthMismatch)
        ));
        assert_eq!(calls.get(), 2);
    }
}

#[derive(crate::NoritoSchema)]
#[norito_schema(name = "norito.test.core.encoder_tests.NestedChecksumDrift")]
struct NestedChecksumDrift {
    calls: Cell<usize>,
}
impl SerializePayload for NestedChecksumDrift {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        struct Leaf<'a>(&'a Cell<usize>);
        impl SerializePayload for Leaf<'_> {
            fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
                let call = self.0.get();
                self.0.set(call + 1);
                writer.write_all(if call < 2 { b"A" } else { b"B" })?;
                Ok(())
            }
        }
        // The first frame pass counts and writes A. The second pass counts and
        // writes B; all child lengths are valid, so only real checksum coverage
        // can reject this mutation.
        write_len_prefixed(writer, &Leaf(&self.calls))
    }
}

#[test]
fn actual_nested_frame_still_rejects_same_length_checksum_drift() {
    for flags in [0, header_flags::COMPACT_LEN] {
        let _layout = DecodeFlagsGuard::enter(flags);
        let value = NestedChecksumDrift {
            calls: Cell::new(0),
        };
        assert!(matches!(
            write_frame_to_writer(&value, &mut Vec::new()),
            Err(Error::ChecksumMismatch)
        ));
        assert_eq!(value.calls.get(), 4);
    }
}
