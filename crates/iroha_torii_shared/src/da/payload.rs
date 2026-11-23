//! Bounded application DA decompression shared by the node and producer clients.

use flate2::read::{GzDecoder, ZlibDecoder};
use iroha_data_model::da::types::Compression;
use std::{
    borrow::Cow,
    io::{self, Read},
};

use iroha_data_model::da::MAX_PAYLOAD_BYTES;

/// A transmitted payload cannot produce its declared canonical extent.
#[derive(Debug)]
pub enum DecodeError {
    /// The declared or transmitted payload cannot fit the V1 transport.
    TooLarge,
    /// Compressed bytes are malformed or disagree with the declared payload size.
    Invalid(io::Error),
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge => f.write_str("payload exceeds the 64 MiB DA ingest limit"),
            Self::Invalid(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for DecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::TooLarge => None,
            Self::Invalid(error) => Some(error),
        }
    }
}
impl From<io::Error> for DecodeError {
    fn from(error: io::Error) -> Self {
        Self::Invalid(error)
    }
}

/// Decode exactly the declared bounded payload. Identity payloads borrow the input.
///
/// # Errors
/// Refuses oversized blobs, malformed compression and incorrect canonical sizes.
pub fn decode(
    compression: Compression,
    bytes: &[u8],
    total_size: u64,
) -> Result<Cow<'_, [u8]>, DecodeError> {
    if total_size > MAX_PAYLOAD_BYTES || bytes.len() as u64 > MAX_PAYLOAD_BYTES {
        return Err(DecodeError::TooLarge);
    }
    let expected = total_size as usize;
    match compression {
        Compression::Identity if bytes.len() == expected => Ok(Cow::Borrowed(bytes)),
        Compression::Identity => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "payload length does not match total_size",
        )
        .into()),
        Compression::Gzip => Ok(Cow::Owned(decompress_reader(
            GzDecoder::new(bytes),
            expected,
            "gzip",
        )?)),
        Compression::Deflate => Ok(Cow::Owned(decompress_reader(
            ZlibDecoder::new(bytes),
            expected,
            "deflate",
        )?)),
        Compression::Zstd => {
            let mut decoder = zstd::stream::read::Decoder::new(bytes)?;
            decoder.window_log_max(26)?;
            Ok(Cow::Owned(decompress_reader(decoder, expected, "zstd")?))
        }
    }
}

fn decompress_reader(reader: impl Read, expected: usize, algorithm: &str) -> io::Result<Vec<u8>> {
    let limit = expected
        .checked_add(1)
        .and_then(|v| u64::try_from(v).ok())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{algorithm} total_size exceeds supported decompression boundary"),
            )
        })?;
    let mut buffer = Vec::with_capacity(expected.min(16 * 1024));
    reader.take(limit).read_to_end(&mut buffer).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!("failed to decompress {algorithm} payload: {err}"),
        )
    })?;
    if buffer.len() != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{algorithm} payload decompressed to {} bytes but total_size advertises {} bytes",
                buffer.len(),
                expected
            ),
        ));
    }
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    #[test]
    fn every_wire_codec_yields_identical_bounded_canonical_bytes() {
        let payload = b"application availability bytes";
        let n = payload.len() as u64;
        let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gzip.write_all(payload).unwrap();
        let mut deflate =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        deflate.write_all(payload).unwrap();
        for (compression, wire) in [
            (Compression::Identity, payload.to_vec()),
            (Compression::Gzip, gzip.finish().unwrap()),
            (Compression::Deflate, deflate.finish().unwrap()),
            (
                Compression::Zstd,
                zstd::stream::encode_all(&payload[..], 1).unwrap(),
            ),
        ] {
            assert_eq!(decode(compression, &wire, n).unwrap().as_ref(), payload);
            assert!(decode(compression, &wire, n - 1).is_err());
            assert!(matches!(
                decode(compression, &wire, MAX_PAYLOAD_BYTES + 1),
                Err(DecodeError::TooLarge)
            ));
        }
        let mut raw =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        raw.write_all(payload).unwrap();
        assert!(decode(Compression::Deflate, &raw.finish().unwrap(), n).is_err());
    }
    struct CountingReader {
        remaining: usize,
        emitted: usize,
    }
    impl CountingReader {
        fn new(remaining: usize) -> Self {
            Self {
                remaining,
                emitted: 0,
            }
        }
    }
    impl Read for CountingReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.remaining == 0 {
                return Ok(0);
            }
            let n = self.remaining.min(buf.len());
            buf[..n].fill(0xA5);
            self.remaining -= n;
            self.emitted += n;
            Ok(n)
        }
    }
    #[test]
    fn decompress_reader_stops_after_advertised_len_plus_one() {
        let mut reader = CountingReader::new(64);
        let err = decompress_reader(&mut reader, 8, "test")
            .expect_err("overlong decompressed stream should reject");
        assert_eq!(
            reader.emitted, 9,
            "decompressor should read only one byte beyond advertised length"
        );
        assert!(
            err.to_string()
                .contains("test payload decompressed to 9 bytes but total_size advertises 8 bytes"),
            "unexpected error message: {}",
            err.to_string()
        );
    }
    #[test]
    fn decompress_reader_rejects_unbounded_expected_len_without_reading() {
        let mut reader = CountingReader::new(1);
        let err = decompress_reader(&mut reader, usize::MAX, "test")
            .expect_err("unbounded expected length should reject");
        assert_eq!(reader.emitted, 0);
        assert!(
            err.to_string().contains("supported decompression boundary"),
            "unexpected error message: {}",
            err.to_string()
        );
    }
}
