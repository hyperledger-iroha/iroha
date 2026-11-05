//! Fixed-size hexadecimal text for telemetry hashes and signatures.
//!
//! Decoding rejects the length before inspecting any digits and never allocates.

/// Failure to decode the exact 32-byte hash text accepted by telemetry state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DecodeError {
    /// The input does not contain exactly 64 bytes.
    Length(usize),
    /// The byte at this index is not an ASCII hexadecimal digit.
    Digit(usize),
}

/// Encode a hash as lowercase hexadecimal with one allocation for the result.
pub(super) fn encode(hash: &[u8; 32]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = [0_u8; 64];
    for (byte, pair) in hash.iter().zip(text.chunks_exact_mut(2)) {
        pair[0] = DIGITS[usize::from(byte >> 4)];
        pair[1] = DIGITS[usize::from(byte & 0xf)];
    }
    String::from_utf8(text.to_vec()).expect("hexadecimal digits are ASCII")
}

/// Decode exactly one hash, accepting both ASCII letter cases.
pub(super) fn decode(text: &str) -> Result<[u8; 32], DecodeError> {
    if text.len() != 64 {
        return Err(DecodeError::Length(text.len()));
    }
    let mut hash = [0_u8; 32];
    for (index, (out, pair)) in hash
        .iter_mut()
        .zip(text.as_bytes().chunks_exact(2))
        .enumerate()
    {
        let high = digit(pair[0]).ok_or(DecodeError::Digit(index * 2))?;
        let low = digit(pair[1]).ok_or(DecodeError::Digit(index * 2 + 1))?;
        *out = (high << 4) | low;
    }
    Ok(hash)
}

fn digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_every_byte_to_canonical_lowercase() {
        for byte in 0_u8..=u8::MAX {
            let expected = format!("{byte:02x}").repeat(32);
            assert_eq!(encode(&[byte; 32]), expected);
            assert_eq!(decode(&expected), Ok([byte; 32]));
        }
    }

    #[test]
    fn preserves_byte_order_and_accepts_mixed_case() {
        let bytes = std::array::from_fn(|index| index as u8 * 7);
        let text = "00070e151c232a31383f464d545b626970777e858c939aa1a8afb6bdc4cbd2d9";
        assert_eq!(encode(&bytes), text);
        assert_eq!(decode(text), Ok(bytes));
        assert_eq!(decode(&text.to_ascii_uppercase()), Ok(bytes));
        let mixed = text
            .chars()
            .enumerate()
            .map(|(index, ch)| {
                if index % 2 == 0 {
                    ch.to_ascii_uppercase()
                } else {
                    ch
                }
            })
            .collect::<String>();
        assert_eq!(decode(&mixed), Ok(bytes));
    }

    #[test]
    fn validates_every_digit_value() {
        for byte in 0_u8..=u8::MAX {
            let expected = char::from(byte).to_digit(16).map(|digit| digit as u8);
            assert_eq!(digit(byte), expected, "byte {byte}");
        }
    }

    #[test]
    fn rejects_invalid_ascii_at_every_position() {
        for byte in 0_u8..=127 {
            if byte.is_ascii_hexdigit() {
                continue;
            }
            for index in 0..64 {
                let mut bytes = [b'0'; 64];
                bytes[index] = byte;
                let text = std::str::from_utf8(&bytes).unwrap();
                assert_eq!(decode(text), Err(DecodeError::Digit(index)));
            }
        }
    }

    #[test]
    fn rejects_non_ascii_and_checks_length_first() {
        for invalid in ["é", "Ａ", "🦀"] {
            let text = format!("{invalid}{}", "0".repeat(64 - invalid.len()));
            assert_eq!(decode(&text), Err(DecodeError::Digit(0)));
        }
        for length in [0, 1, 2, 62, 63, 65, 66, 1024 * 1024] {
            // Invalid digits must never be examined for a wrong-size input.
            assert_eq!(
                decode(&"!".repeat(length)),
                Err(DecodeError::Length(length))
            );
        }
    }
}
