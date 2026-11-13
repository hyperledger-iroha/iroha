//! Checked uppercase-hex JSON strings containing exact Norito frames.

use super::{BoundedJsonError, JsonWriteSink};
use crate::core::NoritoSerialize;
use std::io::{self, Write};

struct HexWriter<'a> {
    output: &'a mut dyn JsonWriteSink,
    refusal: Option<BoundedJsonError>,
}

impl Write for HexWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.refusal.is_some() {
            return Err(io::Error::other("JSON sink already refused frame hex"));
        }
        const HEX: &[u8; 16] = b"0123456789ABCDEF";
        for byte in bytes {
            for digit in [HEX[usize::from(byte >> 4)], HEX[usize::from(byte & 15)]] {
                if let Err(error) = self.output.push(char::from(digit)) {
                    self.refusal = Some(error);
                    return Err(io::Error::other("JSON sink refused frame hex"));
                }
            }
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn write_hex_frame(
    output: &mut dyn JsonWriteSink,
    write: impl FnOnce(&mut HexWriter<'_>) -> Result<(), crate::core::Error>,
) -> Result<(), BoundedJsonError> {
    output.push('"')?;
    let mut writer = HexWriter {
        output,
        refusal: None,
    };
    if write(&mut writer).is_err() {
        return Err(writer.refusal.unwrap_or(BoundedJsonError::LengthMismatch));
    }
    if let Some(error) = writer.refusal {
        return Err(error);
    }
    writer.output.push('"')
}

/// Stream a canonical Norito frame as one uppercase-hex JSON string.
///
/// No output-sized frame or text buffer is allocated. Serializer-internal scratch still
/// requires auditing. The emitted frame matches [`crate::encode_canonical`] independently
/// of the ambient layout flags.
///
/// # Errors
/// Returns the original sink refusal, or [`BoundedJsonError::LengthMismatch`] if framing
/// fails. The caller must discard partially written output after an error.
pub fn write_canonical_json_hex_to<T: NoritoSerialize>(
    value: &T,
    output: &mut dyn JsonWriteSink,
) -> Result<(), BoundedJsonError> {
    write_hex_frame(output, |writer| {
        crate::core::write_canonical_to_writer(value, writer)
    })
}

/// Stream a Norito frame with the active layout flags as an uppercase-hex JSON string.
///
/// This preserves the exact framing of [`crate::to_bytes`] without staging its output.
/// Use [`write_canonical_json_hex_to`] when the caller requires a canonical layout
/// independent of the ambient flags.
///
/// # Errors
/// Returns the original sink refusal, or [`BoundedJsonError::LengthMismatch`] if framing
/// fails. The caller must discard partially written output after an error.
pub fn write_frame_json_hex_to<T: NoritoSerialize>(
    value: &T,
    output: &mut dyn JsonWriteSink,
) -> Result<(), BoundedJsonError> {
    write_hex_frame(output, |writer| {
        crate::core::write_frame_to_writer(value, writer)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::{self, JsonSerialize};

    struct FrameJson<'a> {
        value: &'a Vec<u64>,
        canonical: bool,
    }

    impl JsonSerialize for FrameJson<'_> {
        fn json_serialize(&self, output: &mut String) {
            let bytes = if self.canonical {
                crate::encode_canonical(self.value)
            } else {
                crate::to_bytes(self.value)
            }
            .unwrap();
            json::write_json_string(
                &bytes
                    .iter()
                    .map(|byte| format!("{byte:02X}"))
                    .collect::<String>(),
                output,
            );
        }

        fn json_serialize_to(
            &self,
            output: &mut dyn JsonWriteSink,
        ) -> Result<(), BoundedJsonError> {
            if self.canonical {
                write_canonical_json_hex_to(self.value, output)
            } else {
                write_frame_json_hex_to(self.value, output)
            }
        }
    }

    #[test]
    fn bounded_frame_hex_preserves_frames_and_refuses_every_short_capacity() {
        for flags in [0, crate::core::default_encode_flags()] {
            let _guard = crate::core::DecodeFlagsGuard::enter(flags);
            for canonical in [false, true] {
                let value = vec![0, 255, u64::MAX];
                let json = FrameJson {
                    value: &value,
                    canonical,
                };
                let expected = json::to_json(&json).unwrap();
                assert_eq!(
                    json::to_json_bounded(&json, expected.len()),
                    Ok(expected.clone())
                );
                for cap in 0..expected.len() {
                    assert_eq!(
                        json::to_json_bounded(&json, cap),
                        Err(BoundedJsonError::BodyTooLarge)
                    );
                }
            }
        }
    }

    #[test]
    fn bounded_frame_hex_preserves_the_original_sink_error_and_stops_writing() {
        struct RefusingSink(usize);
        impl JsonWriteSink for RefusingSink {
            fn push(&mut self, _: char) -> Result<(), BoundedJsonError> {
                self.0 += 1;
                Err(BoundedJsonError::Unsupported)
            }
            fn push_str(&mut self, _: &str) -> Result<(), BoundedJsonError> {
                self.0 += 1;
                Err(BoundedJsonError::Unsupported)
            }
        }
        let mut output = RefusingSink(0);
        let mut writer = HexWriter {
            output: &mut output,
            refusal: None,
        };
        assert!(writer.write_all(&[1, 2]).is_err());
        assert!(writer.write_all(&[3]).is_err());
        assert_eq!(writer.refusal, Some(BoundedJsonError::Unsupported));
        assert_eq!(output.0, 1);
    }
}
