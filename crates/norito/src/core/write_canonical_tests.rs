//! Tests for allocation-free canonical frame streaming.

use std::{cell::Cell, io::Write};

use super::*;

#[test]
fn streamed_canonical_frame_matches_buffered_encoding() {
    let value = vec![1_u64, 2, 3, 5, 8, 13];
    let expected = {
        let _flags = DecodeFlagsGuard::enter(default_encode_flags());
        to_bytes(&value).expect("encode buffered canonical frame")
    };
    let mut actual = Vec::new();
    let alternate_flags = default_encode_flags() ^ header_flags::COMPACT_LEN;
    {
        let _ambient = DecodeFlagsGuard::enter(alternate_flags);
        write_canonical_to_writer(&value, &mut actual).expect("stream canonical frame");
    }
    assert_eq!(actual, expected);
}

struct ChangingPayload {
    calls: Cell<usize>,
    first: &'static [u8],
    second: &'static [u8],
}

impl NoritoSerialize for ChangingPayload {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        let call = self.calls.get();
        self.calls.set(call + 1);
        writer.write_all(if call == 0 { self.first } else { self.second })?;
        Ok(())
    }
}

#[test]
fn streamed_canonical_frame_rejects_second_pass_length_drift() {
    let value = ChangingPayload {
        calls: Cell::new(0),
        first: &[0x11],
        second: &[0x11, 0x22],
    };
    let error = write_canonical_to_writer(&value, &mut Vec::new())
        .expect_err("second-pass growth must be rejected");
    assert!(matches!(error, Error::LengthMismatch));
    assert_eq!(value.calls.get(), 2);
}

#[test]
fn streamed_canonical_frame_rejects_second_pass_checksum_drift() {
    let value = ChangingPayload {
        calls: Cell::new(0),
        first: &[0x11],
        second: &[0x22],
    };
    let error = write_canonical_to_writer(&value, &mut Vec::new())
        .expect_err("same-length payload drift must be rejected");
    assert!(matches!(error, Error::ChecksumMismatch));
    assert_eq!(value.calls.get(), 2);
}

#[test]
fn streamed_canonical_frame_rejects_second_pass_flag_drift() {
    struct ChangingFlags(Cell<usize>);

    impl NoritoSerialize for ChangingFlags {
        fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
            let call = self.0.get();
            self.0.set(call + 1);
            if call == 0 {
                note_fixed_offsets_emitted();
            }
            writer.write_all(&[0x11])?;
            Ok(())
        }
    }

    let value = ChangingFlags(Cell::new(0));
    let error = write_canonical_to_writer(&value, &mut Vec::new())
        .expect_err("layout-flag drift must be rejected");
    assert!(matches!(error, Error::NonCanonicalEncoding));
    assert_eq!(value.0.get(), 2);
}

#[test]
fn streamed_canonical_frame_propagates_writer_failure() {
    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _bytes: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other(
                "intentional canonical writer failure",
            ))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let error = write_canonical_to_writer(&7_u64, &mut FailingWriter)
        .expect_err("writer failure must propagate");
    assert!(matches!(error, Error::Io(_)));
}
