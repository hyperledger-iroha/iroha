//! Cached typed framing for explicitly specified fixed-size payloads.

use std::{io::Write, marker::PhantomData};

use super::{
    Compression, Error, Header, NoritoSerialize, crc64, payload_alignment_padding_for,
    payload_len_to_usize, payload_without_leading_padding_exact, validate_header_flags,
};

/// One typed fixed-payload frame layout retained by its original resource owner.
///
/// Construction resolves the existing schema identity once and may allocate its
/// name. The caller must fund construction and retain this layout with its I/O
/// resources. Subsequent framing and borrowed validation allocate no codec
/// storage, including on malformed-input errors. A supplied writer still owns
/// its allocations, errors and partial output.
///
/// The payload's fixed length and flags are explicit schema obligations, not
/// inferred from bytes. This boundary validates framing only: the schema's
/// consumer must validate field encodings, reserved bytes and logical content.
/// It performs no typed deserialization or alignment copy and grants no storage
/// lease, allocation credit, durability or publication authority.
pub struct FixedFrameLayout<T: NoritoSerialize> {
    schema: [u8; 16],
    payload_len: usize,
    payload_len_u64: u64,
    padding: usize,
    frame_len: usize,
    flags: u8,
    marker: PhantomData<fn() -> T>,
}

impl<T: NoritoSerialize> std::fmt::Debug for FixedFrameLayout<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FixedFrameLayout")
            .field("schema", &self.schema)
            .field("payload_len", &self.payload_len)
            .field("padding", &self.padding)
            .field("frame_len", &self.frame_len)
            .field("flags", &self.flags)
            .finish()
    }
}

impl<T: NoritoSerialize> FixedFrameLayout<T> {
    /// Resolve this type's identity for one fixed length and explicit flag set.
    ///
    /// The current archive-length ceiling is checked before schema resolution.
    /// Later operations use this admitted fixed length; they do not construct a
    /// decoder budget or consult ambient layout guards. Padding is derived from
    /// the same archived type alignment as other Norito frame writers.
    ///
    /// # Errors
    /// Returns a fixed error for invalid flags, archive-limit excess, or checked
    /// length overflow. Schema identity construction remains the caller's funded
    /// initialization obligation; no caller-supplied schema digest is accepted.
    pub fn new(payload_len: usize, flags: u8) -> Result<Self, Error> {
        validate_header_flags(flags)?;
        let padding = payload_alignment_padding_for::<T>();
        let frame_len = Header::SIZE
            .checked_add(padding)
            .and_then(|len| len.checked_add(payload_len))
            .ok_or(Error::LengthMismatch)?;
        let payload_len_u64 = u64::try_from(payload_len).map_err(|_| Error::LengthMismatch)?;
        payload_len_to_usize(payload_len_u64)?;
        Ok(Self {
            schema: crate::schema::identity::frame_hash::<T>(),
            payload_len,
            payload_len_u64,
            padding,
            frame_len,
            flags,
            marker: PhantomData,
        })
    }

    /// Exact complete frame length, including header and type-derived padding.
    pub fn frame_len(&self) -> usize {
        self.frame_len
    }

    /// Write one exact payload using the existing Norito header and CRC kernel.
    ///
    /// # Errors
    /// Rejects a different payload length before touching the writer. Writer
    /// failures are propagated unchanged and may leave partial output; the
    /// original I/O owner must retain or discard that output before publication.
    pub fn write<W: Write + ?Sized>(&self, writer: &mut W, payload: &[u8]) -> Result<(), Error> {
        if payload.len() != self.payload_len {
            return Err(Error::LengthMismatch);
        }
        write_bare_frame(writer, payload, self.schema, self.padding, self.flags)
    }

    /// Validate one complete fixed frame and borrow its exact payload bytes.
    ///
    /// Input may be unaligned: this method never casts bytes to archived values.
    /// No fallback layout, suffix, extra padding or compressed frame is accepted.
    /// Returned bytes remain borrowed from the caller's original buffer.
    ///
    /// # Errors
    /// Returns fixed header, schema, layout, length, padding or checksum errors.
    /// Field-level semantic validation remains the caller's responsibility.
    pub fn payload<'a>(&self, frame: &'a [u8]) -> Result<&'a [u8], Error> {
        if frame.len() != self.frame_len {
            return Err(Error::LengthMismatch);
        }
        let header = Header::read(std::io::Cursor::new(frame))?;
        if header.schema != self.schema {
            return Err(Error::SchemaMismatch);
        }
        if header.compression != Compression::None {
            return Err(Error::unsupported_compression_with(
                header.compression as u8,
                &[Compression::None],
            ));
        }
        if header.flags != self.flags {
            return Err(Error::NonCanonicalEncoding);
        }
        if header.length != self.payload_len_u64 {
            return Err(Error::LengthMismatch);
        }
        let payload = payload_without_leading_padding_exact(
            &frame[Header::SIZE..],
            self.payload_len,
            self.padding,
        )?;
        if crc64(payload) != header.checksum {
            return Err(Error::ChecksumMismatch);
        }
        Ok(payload)
    }
}

/// One physical writer for both cached fixed layouts and existing bare frames.
pub(super) fn write_bare_frame<W: Write + ?Sized>(
    writer: &mut W,
    payload: &[u8],
    schema: [u8; 16],
    mut padding: usize,
    flags: u8,
) -> Result<(), Error> {
    let length = u64::try_from(payload.len()).map_err(|_| Error::LengthMismatch)?;
    let mut header = Header::new(schema, length, crc64(payload));
    header.flags = flags;
    header.write(&mut *writer)?;
    const ZEROS: [u8; 64] = [0; 64];
    while padding != 0 {
        let chunk = padding.min(ZEROS.len());
        writer.write_all(&ZEROS[..chunk])?;
        padding -= chunk;
    }
    writer.write_all(payload)?;
    Ok(())
}
