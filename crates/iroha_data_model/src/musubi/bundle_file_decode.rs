#[derive(Clone, Copy)]
struct CanonicalBundleFileWriterV1<'bytes> {
    expected: &'bytes [u8],
    offset: usize,
    matches: bool,
}

impl<'bytes> CanonicalBundleFileWriterV1<'bytes> {
    const fn new(expected: &'bytes [u8]) -> Self {
        Self {
            expected,
            offset: 0,
            matches: true,
        }
    }

    const fn finish(self) -> bool {
        self.matches && self.offset == self.expected.len()
    }
}

impl io::Write for CanonicalBundleFileWriterV1<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let Some(end) = self.offset.checked_add(bytes.len()) else {
            self.matches = false;
            self.offset = usize::MAX;
            return Ok(bytes.len());
        };
        if self.expected.get(self.offset..end) != Some(bytes) {
            self.matches = false;
        }
        self.offset = end;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn decode_canonical_bundle_file_v1<T>(
    bytes: &[u8],
    maximum_bytes: u64,
    limits: norito::DecodeLimits,
    validate: impl FnOnce(&T) -> Result<(), ParseError>,
    failure_reason: &'static str,
) -> Result<T, ParseError>
where
    T: Encode,
    for<'de> T: norito::core::NoritoDeserialize<'de> + norito::core::DecodeFromSlice<'de>,
{
    let byte_len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if bytes.is_empty() || byte_len > maximum_bytes {
        return Err(ParseError::new(failure_reason));
    }
    let value = norito::with_decode_limits(norito::canonical_decode_limits(bytes.len()), || {
        norito::with_decode_limits(limits, || {
            // Bundle files always use the fixed V1 bare layout. Scope that layout instead of
            // resetting Norito's thread-local decode state so callers retain any ambient flags
            // and payload context after this boundary returns.
            let _canonical_flags =
                norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
            norito::core::decode_field_canonical_from_slice::<T>(bytes).map(|(value, _used)| value)
        })
    })
    .map_err(|_| ParseError::new(failure_reason))?;
    validate(&value).map_err(|_| ParseError::new(failure_reason))?;

    // Compare the canonical re-encoding directly against the caller's slice instead of first
    // materializing a second complete top-level metadata `Vec`. Norito's derived encoder can still
    // transiently buffer one length-delimited field, which is bounded by the file-size gate above.
    let mut writer = CanonicalBundleFileWriterV1::new(bytes);
    value.encode_to(&mut writer);
    if !writer.finish() {
        return Err(ParseError::new(failure_reason));
    }
    Ok(value)
}
