/// Derive default limits for a validated frame header.
///
/// Structural limits must cover the uncompressed payload, while the cumulative
/// allocation limit stays anchored to the bytes supplied by the caller. The
/// latter keeps a small compressed frame from authorizing arbitrary expansion;
/// [`deserialize_stream`] charges the complete declared payload before it
/// allocates or decompresses it.
fn framed_decode_limits(frame_len: usize, uncompressed_payload_len: usize) -> DecodeLimits {
    let frame_limits = canonical_decode_limits(frame_len);
    let structural_len = frame_len.max(uncompressed_payload_len);
    DecodeLimits::new(
        structural_len.saturating_mul(8),
        structural_len,
        structural_len.saturating_mul(8),
        frame_limits.max_total_allocated_bytes(),
        frame_limits.max_nesting_depth(),
    )
}

/// Decode an object from Norito-encoded bytes (compressed or not) under a
/// payload-derived resource budget.
///
/// Structural byte and element limits cover the validated header's declared
/// uncompressed payload length. The cumulative allocation budget remains
/// derived from the complete frame length, so a short input cannot force an
/// allocation proportional only to an attacker-declared uncompressed length.
/// Callers with a narrower schema limit, or trusted compressed data whose
/// legitimate expansion exceeds the default envelope, can use
/// [`decode_from_bytes_with_limits`] with an explicit budget.
pub fn decode_from_bytes<T>(bytes: &[u8]) -> Result<T, Error>
where
    for<'de> T: NoritoDeserialize<'de>,
{
    let header = core::Header::read(std::io::Cursor::new(bytes))?;
    if header.schema != T::schema_hash() {
        return Err(Error::SchemaMismatch);
    }
    let payload_len = core::payload_len_to_usize(header.length)?;
    with_decode_limits(framed_decode_limits(bytes.len(), payload_len), || {
        decode_from_bytes_inner(bytes)
    })
}
