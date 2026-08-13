//! Allocation-bounded canonical-Norito base64 JSON strings.
use std::io::{self, Write};
use super::{BoundedJsonError, JsonWriteSink, bounded::UnboundedJsonSink};
use crate::core::{NoritoSerialize, write_canonical_to_writer};
const STANDARD_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
/// Run a checked JSON writer against an ordinary string destination.
///
/// Manual serializers use this to share one direct implementation between
/// bounded and legacy output without materializing an intermediate [`super::Value`].
#[doc(hidden)]
pub fn write_with_unbounded_sink(
    output: &mut String,
    write: impl FnOnce(&mut dyn JsonWriteSink) -> Result<(), BoundedJsonError>,
) {
    let mut sink = UnboundedJsonSink::new(output);
    write(&mut sink).expect("checked JSON serialization must accept an unbounded sink");
}
struct Base64Writer<'a> {
    output: &'a mut dyn JsonWriteSink,
    carry: [u8; 3],
    carry_len: usize,
    sink_error: Option<BoundedJsonError>,
}
impl<'a> Base64Writer<'a> {
    fn new(output: &'a mut dyn JsonWriteSink) -> Self {
        Self {
            output,
            carry: [0; 3],
            carry_len: 0,
            sink_error: None,
        }
    }
    fn sink_failure(&mut self, error: BoundedJsonError) -> io::Error {
        self.sink_error = Some(error);
        io::Error::other("bounded JSON sink rejected streamed base64")
    }
    fn push(&mut self, byte: u8) -> io::Result<()> {
        if self.sink_error.is_some() {
            return Err(io::Error::other(
                "bounded JSON sink already rejected streamed base64",
            ));
        }
        match self.output.push(char::from(byte)) {
            Ok(()) => Ok(()),
            Err(error) => Err(self.sink_failure(error)),
        }
    }
    fn emit_full_block(&mut self, block: [u8; 3]) -> io::Result<()> {
        self.push(STANDARD_ALPHABET[usize::from(block[0] >> 2)])?;
        self.push(STANDARD_ALPHABET[usize::from(((block[0] & 0x03) << 4) | (block[1] >> 4))])?;
        self.push(STANDARD_ALPHABET[usize::from(((block[1] & 0x0f) << 2) | (block[2] >> 6))])?;
        self.push(STANDARD_ALPHABET[usize::from(block[2] & 0x3f)])
    }
    fn write_bytes(&mut self, mut bytes: &[u8]) -> io::Result<()> {
        if self.carry_len != 0 {
            let take = (3 - self.carry_len).min(bytes.len());
            self.carry[self.carry_len..self.carry_len + take].copy_from_slice(&bytes[..take]);
            self.carry_len += take;
            bytes = &bytes[take..];
            if self.carry_len != 3 {
                return Ok(());
            }
            self.emit_full_block(self.carry)?;
            self.carry_len = 0;
        }
        let mut chunks = bytes.chunks_exact(3);
        for chunk in &mut chunks {
            self.emit_full_block([chunk[0], chunk[1], chunk[2]])?;
        }
        let remainder = chunks.remainder();
        self.carry[..remainder.len()].copy_from_slice(remainder);
        self.carry_len = remainder.len();
        Ok(())
    }
    fn finish(&mut self) -> Result<(), BoundedJsonError> {
        let result = match self.carry_len {
            0 => Ok(()),
            1 => {
                let first = self.carry[0];
                self.push(STANDARD_ALPHABET[usize::from(first >> 2)])
                    .and_then(|()| self.push(STANDARD_ALPHABET[usize::from((first & 0x03) << 4)]))
                    .and_then(|()| self.push(b'='))
                    .and_then(|()| self.push(b'='))
            }
            2 => {
                let first = self.carry[0];
                let second = self.carry[1];
                self.push(STANDARD_ALPHABET[usize::from(first >> 2)])
                    .and_then(|()| {
                        self.push(
                            STANDARD_ALPHABET[usize::from(((first & 0x03) << 4) | (second >> 4))],
                        )
                    })
                    .and_then(|()| self.push(STANDARD_ALPHABET[usize::from((second & 0x0f) << 2)]))
                    .and_then(|()| self.push(b'='))
            }
            _ => unreachable!("base64 carry is always shorter than one block"),
        };
        self.carry_len = 0;
        result.map_err(|_| self.sink_error.unwrap_or(BoundedJsonError::LengthMismatch))
    }
    fn sink_error(&self) -> Option<BoundedJsonError> {
        self.sink_error
    }
}
impl Write for Base64Writer<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.write_bytes(bytes)?;
        Ok(bytes.len())
    }
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.write_bytes(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
/// Stream bytes as one padded standard-base64 JSON string.
pub fn write_base64_json_to(
    bytes: &[u8],
    output: &mut dyn JsonWriteSink,
) -> Result<(), BoundedJsonError> {
    output.push('"')?;
    {
        let mut base64 = Base64Writer::new(output);
        if base64.write_all(bytes).is_err() {
            return Err(base64
                .sink_error()
                .unwrap_or(BoundedJsonError::LengthMismatch));
        }
        base64.finish()?;
    }
    output.push('"')
}
/// Unbounded-destination counterpart of [`write_base64_json_to`].
pub fn write_base64_json(bytes: &[u8], output: &mut String) {
    let mut sink = UnboundedJsonSink::new(output);
    write_base64_json_to(bytes, &mut sink).expect("base64 JSON serialization must succeed");
}
/// Stream a value's fixed-layout bare Norito bytes as padded standard-base64 JSON.
///
/// This preserves [`crate::codec::Encode::encode`] bytes without retaining the
/// complete bare payload or an encoded string.
pub fn write_bare_norito_base64_json_to<T: NoritoSerialize>(
    value: &T,
    output: &mut dyn JsonWriteSink,
) -> Result<(), BoundedJsonError> {
    output.push('"')?;
    {
        let mut base64 = Base64Writer::new(output);
        if crate::codec::encode_adaptive_into(value, &mut base64).is_err() {
            return Err(base64
                .sink_error()
                .unwrap_or(BoundedJsonError::LengthMismatch));
        }
        base64.finish()?;
    }
    output.push('"')
}
/// Unbounded-destination counterpart of [`write_bare_norito_base64_json_to`].
pub fn write_bare_norito_base64_json<T: NoritoSerialize>(value: &T, output: &mut String) {
    let mut sink = UnboundedJsonSink::new(output);
    write_bare_norito_base64_json_to(value, &mut sink)
        .expect("bare Norito base64 JSON serialization must succeed");
}
/// Stream a canonical Norito frame as one padded standard-base64 JSON string.
///
/// The encoder retains only a three-byte base64 carry. Canonical framing is
/// count-first and rejects length, checksum, or layout-flag drift before this
/// function reports success. An error may leave a partial string in `output`;
/// callers must discard the enclosing response.
pub fn write_canonical_base64_json_to<T: NoritoSerialize>(
    value: &T,
    output: &mut dyn JsonWriteSink,
) -> Result<(), BoundedJsonError> {
    output.push('"')?;
    {
        let mut base64 = Base64Writer::new(output);
        if write_canonical_to_writer(value, &mut base64).is_err() {
            return Err(base64
                .sink_error()
                .unwrap_or(BoundedJsonError::LengthMismatch));
        }
        base64.finish()?;
    }
    output.push('"')
}
/// Unbounded-destination counterpart of [`write_canonical_base64_json_to`].
///
/// This keeps ordinary JSON bytes identical while avoiding a second
/// frame-sized byte vector and base64 string. It panics only if the audited
/// canonical serializer fails, matching legacy `JsonSerialize` contracts.
pub fn write_canonical_base64_json<T: NoritoSerialize>(value: &T, output: &mut String) {
    let mut sink = UnboundedJsonSink::new(output);
    write_canonical_base64_json_to(value, &mut sink)
        .expect("canonical Norito base64 JSON serialization must succeed");
}
#[cfg(test)]
mod tests {
    use super::*;
    fn encode_chunks(bytes: &[u8], chunk: usize) -> String {
        let mut output = String::new();
        let mut sink = UnboundedJsonSink::new(&mut output);
        let mut writer = Base64Writer::new(&mut sink);
        for part in bytes.chunks(chunk) {
            writer.write_all(part).expect("encode base64 chunk");
            writer.write_all(&[]).expect("empty writes retain carry");
        }
        writer.finish().expect("finish base64");
        output
    }
    #[test]
    fn base64_writer_matches_padded_standard_vectors_across_chunk_boundaries() {
        for chunk in 1..=5 {
            assert_eq!(encode_chunks(b"", chunk), "");
            assert_eq!(encode_chunks(b"f", chunk), "Zg==");
            assert_eq!(encode_chunks(b"fo", chunk), "Zm8=");
            assert_eq!(encode_chunks(b"foo", chunk), "Zm9v");
            assert_eq!(encode_chunks(b"foobar", chunk), "Zm9vYmFy");
        }
    }
    #[test]
    fn canonical_base64_json_has_closed_output_bound() {
        let value = vec![1_u64, 2, 3];
        let mut expected = String::new();
        write_canonical_base64_json(&value, &mut expected);
        assert_eq!(
            super::super::to_json_bounded(&CanonicalBase64(&value), expected.len())
                .expect("exact output bound"),
            expected
        );
        assert_eq!(
            super::super::to_json_bounded(&CanonicalBase64(&value), expected.len() - 1),
            Err(BoundedJsonError::BodyTooLarge)
        );
    }
    #[test]
    fn bare_norito_base64_json_matches_buffered_encode() {
        use crate::codec::Encode as _;
        let value = vec![1_u64, 2, 3];
        let mut expected = String::new();
        write_base64_json(&value.encode(), &mut expected);
        let mut actual = String::new();
        write_bare_norito_base64_json(&value, &mut actual);
        assert_eq!(actual, expected);
    }
    struct CanonicalBase64<'a, T>(&'a T);
    impl<T: NoritoSerialize> super::super::JsonSerialize for CanonicalBase64<'_, T> {
        fn json_serialize(&self, output: &mut String) {
            write_canonical_base64_json(self.0, output);
        }
        fn json_serialize_to(
            &self,
            output: &mut dyn JsonWriteSink,
        ) -> Result<(), BoundedJsonError> {
            write_canonical_base64_json_to(self.0, output)
        }
    }
}
