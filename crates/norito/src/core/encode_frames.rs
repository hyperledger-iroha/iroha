//! Streaming embedded frames without replaying payloads into length-only destinations.

use std::io::Write;

use super::{
    DecodeFlagsGuard, EncodeContextGuard, Encoder, Error, ExactLengthWriter, ExactSliceWriter,
    FramedPayloadWriter, Header, NoritoSerialize, SerializePayload, compact_len_used, crc64,
    current_decode_flags_effective, default_encode_flags, encode_writers::LengthCountingWriter,
    encoded_frame_len, finalized_encode_flags, payload_alignment_padding_for,
    validate_header_flags,
};

/// Write a measured prefix followed by one complete frame using the active layout.
///
/// The callback receives the frame length measured by this codec invocation; it
/// supplies no lengths back to the codec. A length-only destination visits the
/// payload once. A byte buffer also serializes the payload once: it appends the
/// payload, computes its checksum and layout flags from those bytes, and then
/// places the prefix, header and padding in front of it. A streaming destination
/// measures checksum and layout flags, then emits a checked second pass. No path
/// retains a second frame-sized buffer.
///
/// The prefix runs outside the embedded frame's encoding contexts, so its layout
/// markers belong to the enclosing value. A prefix must propagate writer errors,
/// and may write only bytes: a buffer destination runs it against a fresh
/// buffer encoder before splicing its output in place.
///
/// # Errors
///
/// Returns measurement, prefix, or output errors. Streaming output rejects
/// length, checksum, or layout drift between passes. Errors after any byte is
/// emitted leave partial output that the caller must discard.
#[doc(hidden)]
pub fn write_frame_with_prefix<T, F>(
    value: &T,
    writer: &mut Encoder<'_>,
    prefix: F,
) -> Result<(), Error>
where
    T: NoritoSerialize,
    F: for<'w> FnOnce(&mut Encoder<'w>, usize) -> Result<(), Error>,
{
    let flags = current_decode_flags_effective().unwrap_or_else(default_encode_flags);
    validate_header_flags(flags)?;
    if writer.in_place_buffer().is_some() {
        return append_frame_in_place(value, writer, flags, prefix);
    }
    if writer.compares_in_place() {
        return compare_frame_in_place(value, writer, flags, prefix);
    }
    if writer.is_counting() {
        let frame_len = encoded_frame_len(value)?;
        prefix(writer, frame_len)?;
        // This invocation measured this exact value. The private destination
        // operation cannot be reached with a caller-provided length.
        return writer
            .count_measured_bytes(frame_len)?
            .then_some(())
            .ok_or_else(|| Error::NonCanonicalEncoding);
    }
    write_frame_to_writer_with_prefix(value, writer, flags, prefix)
}

/// Append `value`'s frame, preceded by `prefix`, to the encoder's own buffer.
///
/// The payload is serialized once, straight into the buffer, so its nested
/// fields keep single-pass framing. Its length, checksum and layout flags are
/// then read from those bytes; the prefix, header and alignment padding are
/// appended after it and rotated in front of it, without scratch storage. The
/// bytes equal the streaming writer's output.
fn append_frame_in_place<T, F>(
    value: &T,
    writer: &mut Encoder<'_>,
    base_flags: u8,
    prefix: F,
) -> Result<(), Error>
where
    T: NoritoSerialize,
    F: for<'w> FnOnce(&mut Encoder<'w>, usize) -> Result<(), Error>,
{
    let _work = super::encode_work::ValueGuard::enter()?;
    let start = writer
        .in_place_buffer()
        .ok_or_else(|| Error::LengthMismatch)?
        .len();
    let payload_guard = EncodeContextGuard::enter();
    {
        let _flags = DecodeFlagsGuard::enter(base_flags);
        value.serialize(writer)?;
    }
    let flags = finalized_encode_flags(base_flags, compact_len_used());
    drop(payload_guard);
    // An overrun or a newly entered bound means the bytes cannot be trusted.
    let buffer = writer
        .in_place_buffer()
        .ok_or_else(|| Error::LengthMismatch)?;
    let payload = buffer.get(start..).ok_or_else(|| Error::LengthMismatch)?;
    let payload_len = payload.len();
    let checksum = crc64(payload);
    let padding = payload_alignment_padding_for::<T>();
    let frame_len = Header::SIZE
        .checked_add(padding)
        .and_then(|size| size.checked_add(payload_len))
        .ok_or_else(|| Error::LengthMismatch)?;
    let payload_end = buffer.len();
    // The prefix belongs to the enclosing value: it runs after the payload's
    // encoding context retired, as it does before the streaming second pass.
    prefix(writer, frame_len)?;
    let mut header = Header::new(
        crate::schema::identity::frame_hash::<T>(),
        u64::try_from(payload_len).map_err(|_| Error::LengthMismatch)?,
        checksum,
    );
    header.flags |= flags;
    // A prefix that swapped the destination or entered a bound leaves bytes
    // this frame cannot place; the caller discards the partial output.
    let buffer = writer
        .in_place_buffer()
        .ok_or_else(|| Error::NonCanonicalEncoding)?;
    header.write(&mut *buffer)?;
    buffer.resize(
        buffer
            .len()
            .checked_add(padding)
            .ok_or_else(|| Error::LengthMismatch)?,
        0,
    );
    let head_len = buffer.len() - payload_end;
    buffer[start..].rotate_right(head_len);
    Ok(())
}

/// Compare `value`'s frame, preceded by `prefix`, with the encoder's expected bytes.
///
/// One counting pass fixes the payload length and layout flags, and so the
/// prefix and header. A matching payload's checksum is the checksum of the
/// expected bytes it matches, so the header is compared without a second
/// measuring pass. The payload is then compared within its counted length.
fn compare_frame_in_place<T, F>(
    value: &T,
    writer: &mut Encoder<'_>,
    base_flags: u8,
    prefix: F,
) -> Result<(), Error>
where
    T: NoritoSerialize,
    F: for<'w> FnOnce(&mut Encoder<'w>, usize) -> Result<(), Error>,
{
    let _work = super::encode_work::ValueGuard::enter()?;
    let (payload_len, first_flags) = counted_payload(value, base_flags)?;
    let padding = payload_alignment_padding_for::<T>();
    let header_len = Header::SIZE
        .checked_add(padding)
        .ok_or_else(|| Error::LengthMismatch)?;
    let frame_len = header_len
        .checked_add(payload_len)
        .ok_or_else(|| Error::LengthMismatch)?;
    prefix(writer, frame_len)?;
    let Some(expected_payload) = writer.compare_upcoming(header_len, payload_len) else {
        // The expected bytes end before this frame's payload would.
        writer.mark_compare_mismatch();
        return Err(Error::NonCanonicalEncoding);
    };
    let mut header = Header::new(
        crate::schema::identity::frame_hash::<T>(),
        u64::try_from(payload_len).map_err(|_| Error::LengthMismatch)?,
        crc64(expected_payload),
    );
    header.flags |= first_flags;
    header.write(&mut *writer)?;
    const ZEROS: [u8; 64] = [0; 64];
    writer.write_all(&ZEROS[..padding])?;
    let second_guard = EncodeContextGuard::enter();
    let result = {
        let _flags = DecodeFlagsGuard::enter(base_flags);
        writer.with_exact_length(payload_len, |writer| value.serialize(writer))
    };
    let second_flags = finalized_encode_flags(base_flags, compact_len_used());
    drop(second_guard);
    result?;
    if second_flags != first_flags {
        return Err(Error::NonCanonicalEncoding);
    }
    Ok(())
}

/// Count `value`'s payload once under `base_flags`, returning its length and
/// the frame flags that payload selects.
fn counted_payload(value: &dyn SerializePayload, base_flags: u8) -> Result<(usize, u8), Error> {
    let guard = EncodeContextGuard::enter();
    let mut counted = LengthCountingWriter::default();
    {
        let _flags = DecodeFlagsGuard::enter(base_flags);
        let mut encoder = Encoder::for_counting(&mut counted);
        value.serialize(&mut encoder)?;
    }
    let length = counted.finish()?;
    let flags = finalized_encode_flags(base_flags, compact_len_used());
    drop(guard);
    Ok((length, flags))
}

/// Compare `T`'s frame of `value` with `expected`, as writing it into an
/// [`ExactSliceWriter`] over `expected` does.
///
/// Outside a bounded work or count scope the payload is compared in one pass
/// (see `Encoder::compare_against`); its bytes then fix the header, which is
/// compared last. A serializer error is returned without a header verdict,
/// as the streaming writer returns it before writing a header.
pub(crate) fn compare_typed_frame<'e, T, P>(
    value: &P,
    expected: &'e [u8],
    base_flags: u8,
) -> (Result<(), Error>, ExactSliceWriter<'e>)
where
    T: NoritoSerialize,
    P: SerializePayload + ?Sized,
{
    let mut exact = ExactSliceWriter::new(expected);
    if super::encode_work::thread_accounting_active() {
        let result = write_typed_payload_frame::<T, P, _>(value, &mut exact, base_flags);
        return (result, exact);
    }
    if let Err(error) = validate_header_flags(base_flags) {
        return (Err(error), exact);
    }
    let padding = payload_alignment_padding_for::<T>();
    let header_len = Header::SIZE + padding;
    let expected_payload = expected.get(header_len..).unwrap_or_default();
    let payload_guard = EncodeContextGuard::enter();
    let (result, payload) = {
        let _flags = DecodeFlagsGuard::enter(base_flags);
        Encoder::compare_against(expected_payload, |encoder| value.serialize(encoder))
    };
    let flags = finalized_encode_flags(base_flags, compact_len_used());
    drop(payload_guard);
    if payload.mismatched() {
        exact.mark_mismatch();
        return (result, exact);
    }
    if result.is_err() {
        return (result, exact);
    }
    let payload_len = payload.matched_len();
    let Ok(header_payload_len) = u64::try_from(payload_len) else {
        return (Err(Error::LengthMismatch), exact);
    };
    let mut header = Header::new(
        crate::schema::identity::frame_hash::<T>(),
        header_payload_len,
        crc64(&expected_payload[..payload_len]),
    );
    header.flags |= flags;
    // Header and padding, on the stack: verification allocates nothing.
    let mut head = [0_u8; Header::SIZE + 64];
    if let Err(error) = header.write(&mut head[..Header::SIZE]) {
        return (Err(error), exact);
    }
    exact.compare(&head[..header_len]);
    exact.skip_matched(payload_len);
    (Ok(()), exact)
}

pub(super) fn write_frame_to_writer_with_flags<T, W>(
    value: &T,
    writer: &mut W,
    base_flags: u8,
) -> Result<(), Error>
where
    T: NoritoSerialize,
    W: Write + ?Sized,
{
    write_frame_to_writer_with_prefix(value, writer, base_flags, |_, _| Ok(()))
}

fn write_frame_to_writer_with_prefix<T, W, F>(
    value: &T,
    writer: &mut W,
    base_flags: u8,
    prefix: F,
) -> Result<(), Error>
where
    T: NoritoSerialize,
    W: Write + ?Sized,
    F: FnOnce(&mut W, usize) -> Result<(), Error>,
{
    write_typed_payload_frame_with_prefix::<T, T, _, _>(value, writer, base_flags, prefix)
}

/// The nominal type supplies only its original frame identity and alignment;
/// the payload still runs through the same checked two-pass frame writer.
pub(super) fn write_typed_payload_frame<T, P, W>(
    value: &P,
    writer: &mut W,
    base_flags: u8,
) -> Result<(), Error>
where
    T: NoritoSerialize,
    P: super::SerializePayload + ?Sized,
    W: Write + ?Sized,
{
    write_typed_payload_frame_with_prefix::<T, P, _, _>(value, writer, base_flags, |_, _| Ok(()))
}

pub(super) fn write_typed_payload_frame_with_prefix<T, P, W, F>(
    value: &P,
    writer: &mut W,
    base_flags: u8,
    prefix: F,
) -> Result<(), Error>
where
    T: NoritoSerialize,
    P: super::SerializePayload + ?Sized,
    W: Write + ?Sized,
    F: FnOnce(&mut W, usize) -> Result<(), Error>,
{
    write_typed_payload_frame_with_identity_and_prefix::<T, P, W, F>(
        value, writer, base_flags, None, prefix,
    )
}

/// Write an audited payload with a caller-supplied literal frame identity.
///
/// The caller must prove this literal equals `T::frame_name()`; this low-level
/// helper cannot authenticate that relationship for a generic identity. `T`
/// retains its original archive alignment. Payload bytes use the same checked
/// two-pass writer and the currently active bounded encoder allowance, without
/// entering a fresh work scope. No dynamic identity string is constructed.
/// Ordinary generic framed writers retain their bounded-source refusal.
///
/// # Errors
/// Requires an active bounded encoding scope; rejects an empty identity or a
/// contradiction with an available static identity before writing any bytes.
/// Preserves original work, serializer, length, checksum and layout refusals.
#[doc(hidden)]
pub fn write_canonical_payload_with_identity<T, P, W>(
    value: &P,
    writer: &mut W,
    identity: &'static str,
) -> Result<(), Error>
where
    T: NoritoSerialize,
    P: super::SerializePayload + ?Sized,
    W: Write + ?Sized,
{
    if !super::charge_encode_prewrite(0)? {
        return Err(Error::InvalidValue {
            context: "explicit identity requires bounded encoding",
        });
    }
    if identity.is_empty() || T::static_frame_name().is_some_and(|known| known != identity) {
        return Err(super::refuse_encode_source());
    }
    write_typed_payload_frame_with_identity_and_prefix::<T, P, W, _>(
        value,
        writer,
        default_encode_flags(),
        Some(identity),
        |_, _| Ok(()),
    )
}

fn write_typed_payload_frame_with_identity_and_prefix<T, P, W, F>(
    value: &P,
    writer: &mut W,
    base_flags: u8,
    identity: Option<&'static str>,
    prefix: F,
) -> Result<(), Error>
where
    T: NoritoSerialize,
    P: super::SerializePayload + ?Sized,
    W: Write + ?Sized,
    F: FnOnce(&mut W, usize) -> Result<(), Error>,
{
    let _work = super::encode_work::ValueGuard::enter()?;
    validate_header_flags(base_flags)?;
    // A bounded frame may not trigger dynamic identity construction, even when
    // an unrelated operation warmed the memo. Prepay literal hashing before
    // source serialization or output allocation; the digest uses fixed scratch.
    let bounded_schema = if super::charge_encode_prewrite(0)? {
        let name = identity
            .or_else(T::static_frame_name)
            .ok_or_else(super::refuse_encode_source)?;
        super::charge_encode_prewrite(
            name.len()
                .checked_add(128)
                .ok_or_else(super::refuse_encode_source)?,
        )?;
        Some(super::schema_hash_for_name(name))
    } else {
        None
    };
    let first_guard = EncodeContextGuard::enter();
    let mut discard = std::io::sink();
    let mut first_payload = FramedPayloadWriter::new(&mut discard);
    {
        let _flags = DecodeFlagsGuard::enter(base_flags);
        let mut encoder = Encoder::new(&mut first_payload);
        value.serialize(&mut encoder)?;
    }
    let payload_len = first_payload.len;
    let payload_len_u64 = u64::try_from(payload_len).map_err(|_| Error::LengthMismatch)?;
    let first_checksum = first_payload.digest.sum64();
    let first_flags = finalized_encode_flags(base_flags, compact_len_used());
    drop(first_guard);

    let padding = payload_alignment_padding_for::<T>();
    let frame_len = Header::SIZE
        .checked_add(padding)
        .and_then(|size| size.checked_add(payload_len))
        .ok_or_else(|| Error::LengthMismatch)?;
    prefix(writer, frame_len)?;

    let mut header = Header::new(
        bounded_schema.unwrap_or_else(crate::schema::identity::frame_hash::<T>),
        payload_len_u64,
        first_checksum,
    );
    header.flags |= first_flags;
    header.write(&mut *writer)?;
    let mut padding = padding;
    const ZEROS: [u8; 64] = [0; 64];
    while padding != 0 {
        let chunk = padding.min(ZEROS.len());
        writer.write_all(&ZEROS[..chunk])?;
        padding -= chunk;
    }

    let second_guard = EncodeContextGuard::enter();
    let mut second_payload = FramedPayloadWriter::new(writer);
    let (serialize_result, written_len, rejected_write) = {
        let mut exact = ExactLengthWriter::new(&mut second_payload, payload_len);
        let result = {
            let _flags = DecodeFlagsGuard::enter(base_flags);
            let mut encoder = Encoder::new(&mut exact);
            value.serialize(&mut encoder)
        };
        (result, exact.written_len(), exact.rejected_write())
    };
    let second_checksum = second_payload.digest.sum64();
    let second_flags = finalized_encode_flags(base_flags, compact_len_used());
    drop(second_guard);
    if rejected_write {
        return Err(Error::LengthMismatch);
    }
    serialize_result?;
    if written_len != payload_len || second_payload.len != payload_len {
        return Err(Error::LengthMismatch);
    }
    if second_checksum != first_checksum {
        return Err(Error::ChecksumMismatch);
    }
    if second_flags != first_flags {
        return Err(Error::NonCanonicalEncoding);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::SerializePayload;
    use std::cell::Cell;

    use super::*;
    use crate::core::{
        LengthCountingWriter, encoded_payload_len, frame_bare_with_header_flags, header_flags,
        note_compact_len_emitted, serialize_to_buffer, supported_header_flags, to_bytes,
        write_len_header,
    };

    #[derive(crate::NoritoSchema)]
    #[norito_schema(name = "norito.test.core.encode_frames.Leaf")]
    struct Leaf<'a>(&'a Cell<usize>);

    impl SerializePayload for Leaf<'_> {
        fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
            self.0.set(self.0.get() + 1);
            writer.write_all(&[0xab])?;
            Ok(())
        }

        fn encoded_len_exact(&self) -> Option<usize> {
            panic!("frame measurement must serialize the child")
        }
    }

    #[derive(crate::NoritoSchema)]
    #[norito_schema(name = "norito.test.core.encode_frames.Framed")]
    struct Framed<T>(T);

    impl<T: NoritoSerialize> SerializePayload for Framed<T> {
        fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
            write_frame_with_prefix(&self.0, writer, |writer, length| {
                writer.write_all(
                    &u64::try_from(length)
                        .map_err(|_| Error::LengthMismatch)?
                        .to_le_bytes(),
                )?;
                Ok(())
            })
        }
    }

    fn layouts() -> impl Iterator<Item = u8> {
        (0..=supported_header_flags()).filter(|flags| validate_header_flags(*flags).is_ok())
    }

    #[test]
    fn nested_prefixed_frames_measure_each_leaf_once_and_preserve_wire_bytes() {
        for flags in layouts() {
            let _flags = DecodeFlagsGuard::enter(flags);
            let visits = Cell::new(0);
            let value = Framed(Framed(Leaf(&visits)));
            let inner_frame = to_bytes(&value.0.0).expect("independent buffered leaf frame");
            let mut inner_record = u64::try_from(inner_frame.len())
                .unwrap()
                .to_le_bytes()
                .to_vec();
            inner_record.extend_from_slice(&inner_frame);
            let outer_frame = frame_bare_with_header_flags::<Framed<Leaf<'_>>>(
                &inner_record,
                finalized_encode_flags(flags, false),
            )
            .expect("frame independently assembled nested payload");
            let mut expected = u64::try_from(outer_frame.len())
                .unwrap()
                .to_le_bytes()
                .to_vec();
            expected.extend_from_slice(&outer_frame);

            visits.set(0);
            assert_eq!(encoded_payload_len(&value).unwrap(), expected.len());
            assert_eq!(
                visits.get(),
                1,
                "nested count replayed a leaf, flags {flags:#x}"
            );
            visits.set(0);
            let mut actual = Vec::new();
            serialize_to_buffer(&value, &mut actual).unwrap();
            assert_eq!(
                actual, expected,
                "nested frame bytes changed, flags {flags:#x}"
            );
            assert_eq!(
                visits.get(),
                1,
                "a buffer places each frame around its single payload pass"
            );
            visits.set(0);
            let mut streamed = Vec::new();
            crate::core::serialize_to_writer(&value, &mut streamed).unwrap();
            assert_eq!(streamed, expected, "streamed bytes, flags {flags:#x}");
            assert_eq!(
                visits.get(),
                4,
                "each streamed frame retains its two checked passes"
            );
        }
    }

    #[test]
    fn frame_prefix_flags_belong_to_the_outer_encoding_context() {
        let _flags = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);
        let expected_frame = to_bytes(&7_u8).unwrap();
        let outer = EncodeContextGuard::enter();
        let mut bytes = Vec::new();
        write_frame_with_prefix(
            &7_u8,
            &mut Encoder::for_buffer(&mut bytes),
            |writer, length| {
                // The compact length prefix is the only compact marker in this
                // encode: the one-byte frame payload writes no length itself.
                write_len_header(writer, u64::try_from(length).unwrap())?;
                Ok(())
            },
        )
        .unwrap();
        assert!(
            compact_len_used(),
            "frame guards discarded the compact prefix marker"
        );
        drop(outer);
        assert_eq!(&bytes[bytes.len() - expected_frame.len()..], expected_frame);
    }

    #[derive(crate::NoritoSchema)]
    #[norito_schema(name = "norito.test.core.encode_frames.Changes")]
    struct Changes {
        visits: Cell<usize>,
        first: &'static [u8],
        second: &'static [u8],
        flag_drift: bool,
    }

    impl SerializePayload for Changes {
        fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
            let first = self.visits.replace(self.visits.get() + 1) == 0;
            if self.flag_drift && first {
                // Only the first pass claims a compact length prefix, so the
                // advertised COMPACT_LEN bit drifts between the two passes.
                note_compact_len_emitted();
            }
            writer.write_all(if first { self.first } else { self.second })?;
            Ok(())
        }
    }

    #[test]
    fn prefixed_frame_rejects_length_checksum_and_flag_drift_with_bounded_output() {
        for (first, second, flag_drift) in [
            (&[1][..], &[1, 2][..], false),
            (&[1, 2][..], &[1][..], false),
            (&[1][..], &[2][..], false),
            (&[1][..], &[1][..], true),
        ] {
            let value = Changes {
                visits: Cell::new(0),
                first,
                second,
                flag_drift,
            };
            // A streaming destination needs the frame length before the
            // payload, so it measures first and must reject any drift.
            let mut declared = 0;
            let mut bytes = Vec::new();
            let error =
                write_frame_with_prefix(&value, &mut Encoder::new(&mut bytes), |writer, length| {
                    declared = length;
                    writer.write_all(&[0xcc])?;
                    Ok(())
                })
                .expect_err("changed frame must fail");
            if first.len() != second.len() {
                assert!(matches!(error, Error::LengthMismatch));
            } else if flag_drift {
                assert!(matches!(error, Error::NonCanonicalEncoding));
            } else {
                assert!(matches!(error, Error::ChecksumMismatch));
            }
            assert_eq!(
                value.visits.get(),
                2,
                "actual frame must not add a separate length pass"
            );
            assert_eq!(bytes[0], 0xcc);
            assert!(
                bytes.len() <= 1 + declared,
                "frame overrun reached the destination"
            );

            // A buffer has no second pass to drift from: it frames the bytes
            // and layout flags of its single pass.
            let value = Changes {
                visits: Cell::new(0),
                first,
                second,
                flag_drift,
            };
            let mut framed = Vec::new();
            write_frame_with_prefix(
                &value,
                &mut Encoder::for_buffer(&mut framed),
                |writer, length| {
                    writer.write_all(&[u8::try_from(length).unwrap()])?;
                    Ok(())
                },
            )
            .expect("single-pass frame");
            assert_eq!(value.visits.get(), 1);
            let header_flags = if flag_drift {
                header_flags::COMPACT_LEN
            } else {
                0
            };
            let frame = frame_bare_with_header_flags::<Changes>(first, header_flags)
                .expect("independent frame");
            assert_eq!(framed[0] as usize, frame.len());
            assert_eq!(&framed[1..], frame);
        }
    }

    #[test]
    fn prefixed_frame_propagates_measurement_and_prefix_errors_without_replay() {
        #[derive(crate::NoritoSchema)]
        #[norito_schema(name = "norito.test.core.encode_frames.Fails")]
        struct Fails;

        impl SerializePayload for Fails {
            fn serialize(&self, _writer: &mut Encoder<'_>) -> Result<(), Error> {
                Err(Error::NonCanonicalEncoding)
            }
        }
        let mut bytes = Vec::new();
        assert!(matches!(
            write_frame_with_prefix(&Fails, &mut Encoder::for_buffer(&mut bytes), |_, _| {
                panic!("failed measurement must not write a prefix")
            }),
            Err(Error::NonCanonicalEncoding)
        ));
        assert!(bytes.is_empty());
        assert!(matches!(
            encoded_payload_len(&Framed(Fails)),
            Err(Error::NonCanonicalEncoding)
        ));

        // A streaming destination measures, then the prefix fails before
        // the payload is written.
        let visits = Cell::new(0);
        let outer = EncodeContextGuard::enter();
        let error = write_frame_with_prefix(
            &Leaf(&visits),
            &mut Encoder::new(&mut bytes),
            |writer, _| {
                writer.write_all(&[0xdd])?;
                Err(Error::NonCanonicalEncoding)
            },
        )
        .expect_err("prefix error");
        assert!(matches!(error, Error::NonCanonicalEncoding));
        assert_eq!(bytes, [0xdd]);
        assert_eq!(visits.get(), 1);
        assert!(!compact_len_used());
        drop(outer);

        // A buffer has already written its single payload pass when the
        // prefix fails; the partial output is the caller's to discard.
        let visits = Cell::new(0);
        let outer = EncodeContextGuard::enter();
        let mut buffered = Vec::new();
        let error = write_frame_with_prefix(
            &Leaf(&visits),
            &mut Encoder::for_buffer(&mut buffered),
            |writer, _| {
                writer.write_all(&[0xdd])?;
                Err(Error::NonCanonicalEncoding)
            },
        )
        .expect_err("prefix error");
        assert!(matches!(error, Error::NonCanonicalEncoding));
        assert_eq!(buffered, [0xab, 0xdd]);
        assert_eq!(visits.get(), 1);
        assert!(!compact_len_used());
        drop(outer);
    }

    #[test]
    fn prefixed_frame_propagates_destination_failure_after_the_prefix() {
        struct PrefixOnly {
            bytes: Vec<u8>,
        }
        impl Write for PrefixOnly {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                if self.bytes.is_empty() {
                    self.bytes.extend_from_slice(bytes);
                    Ok(bytes.len())
                } else {
                    Err(std::io::Error::other("frame destination unavailable"))
                }
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let visits = Cell::new(0);
        let mut destination = PrefixOnly { bytes: Vec::new() };
        let error = write_frame_with_prefix(
            &Leaf(&visits),
            &mut Encoder::new(&mut destination),
            |writer, _| {
                writer.write_all(&[0xee])?;
                Ok(())
            },
        )
        .expect_err("header destination failure");
        assert!(matches!(error, Error::Io(_)));
        assert_eq!(destination.bytes, [0xee]);
        assert_eq!(
            visits.get(),
            1,
            "failed header must not start the payload pass"
        );
    }

    std::thread_local! {
        static ESCAPED_BYTES: Cell<usize> = const { Cell::new(0) };
    }

    /// A `'static` destination recording every byte that reaches it.
    struct Escaped;
    impl Write for Escaped {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            ESCAPED_BYTES.with(|escaped| escaped.set(escaped.get() + bytes.len()));
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Swap the prefix's destination for another one. The prefix bound is
    /// higher-ranked over the destination's lifetime, so only a `'static`
    /// destination can be swapped in; a borrowed one does not compile.
    fn redirect(writer: &mut Encoder<'_>) {
        *writer = Encoder::new(Box::leak(Box::new(Escaped)));
    }

    fn escaped_bytes() -> usize {
        ESCAPED_BYTES.with(Cell::get)
    }

    #[test]
    fn counting_frame_rejects_prefix_destination_replacement() {
        let visits = Cell::new(0);
        let mut counter = LengthCountingWriter::default();
        let escaped = escaped_bytes();
        let error = {
            let mut encoder = Encoder::for_counting(&mut counter);
            write_frame_with_prefix(&Leaf(&visits), &mut encoder, |writer, _| {
                writer.write_all(&[0xcc])?;
                redirect(writer);
                Ok(())
            })
            .expect_err("a prefix cannot redirect a measured frame into another destination")
        };
        assert!(matches!(error, Error::NonCanonicalEncoding));
        assert_eq!(counter.finish().unwrap(), 1, "only the prefix was counted");
        assert_eq!(
            escaped_bytes(),
            escaped,
            "rejected frame bytes escaped to the replacement"
        );
        assert_eq!(
            visits.get(),
            1,
            "replacement rejection must not replay the child"
        );
    }

    #[test]
    fn buffer_frame_rejects_prefix_destination_replacement() {
        let visits = Cell::new(0);
        let mut bytes = Vec::new();
        let escaped = escaped_bytes();
        let error = {
            let mut encoder = Encoder::for_buffer(&mut bytes);
            write_frame_with_prefix(&Leaf(&visits), &mut encoder, |writer, _| {
                writer.write_all(&[0xcc])?;
                redirect(writer);
                Ok(())
            })
            .expect_err("a prefix cannot redirect an in-place frame into another destination")
        };
        assert!(matches!(error, Error::NonCanonicalEncoding));
        assert_eq!(visits.get(), 1, "the payload was serialized once");
        assert_eq!(
            bytes,
            [0xab, 0xcc],
            "no header was placed around the payload"
        );
        assert_eq!(
            escaped_bytes(),
            escaped,
            "no frame byte reached the replacement"
        );
    }

    #[test]
    fn compared_frame_rejects_prefix_destination_replacement() {
        let visits = Cell::new(0);
        let mut expected = Vec::new();
        write_frame_with_prefix(
            &Leaf(&visits),
            &mut Encoder::for_buffer(&mut expected),
            |writer, length| {
                writer.write_all(&[length as u8])?;
                Ok(())
            },
        )
        .expect("reference frame");
        let (result, exact) = Encoder::compare_against(&expected, |encoder| {
            write_frame_with_prefix(&Leaf(&visits), encoder, |writer, length| {
                writer.write_all(&[length as u8])?;
                redirect(writer);
                Ok(())
            })
        });
        assert!(matches!(result, Err(Error::NonCanonicalEncoding)));
        assert!(exact.mismatched(), "a swapped comparison is never a match");
    }
}
