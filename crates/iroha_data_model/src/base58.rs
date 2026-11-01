//! Bitcoin-alphabet Base58 for canonical asset definition addresses.
//!
//! Both directions work in caller-provided buffers and never allocate. Leading
//! zero bytes map to leading `1` digits and back, as in the Bitcoin encoding.

/// Bitcoin Base58 digits in value order.
const ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// Marks an ASCII byte that is not a Base58 digit in [`DIGIT_VALUES`].
const NOT_A_DIGIT: u8 = 0xFF;

/// Digit value of every ASCII byte, or [`NOT_A_DIGIT`].
const DIGIT_VALUES: [u8; 128] = {
    let mut values = [NOT_A_DIGIT; 128];
    let mut value = 0;
    while value < ALPHABET.len() {
        values[ALPHABET[value] as usize] = value as u8;
        value += 1;
    }
    values
};

/// Why a Base58 conversion failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Base58Error {
    /// The output buffer cannot hold the converted value.
    BufferTooSmall,
    /// The text contains a character outside the Base58 alphabet.
    InvalidCharacter,
}

/// Longest Base58 text produced for `len` input bytes.
///
/// One byte needs at most log(256)/log(58) < 1.5 digits, so `len + ceil(len / 2)`
/// digits always suffice.
pub(crate) const fn max_encoded_len(len: usize) -> usize {
    len + len.div_ceil(2)
}

/// Encode `input` as Base58 ASCII into the front of `output`.
///
/// Returns the number of bytes written.
///
/// # Errors
/// [`Base58Error::BufferTooSmall`] when `output` is shorter than the encoding;
/// [`max_encoded_len`] of the input length is always enough.
pub(crate) fn encode_into(input: &[u8], output: &mut [u8]) -> Result<usize, Base58Error> {
    // Accumulate base-58 digit values little-endian, then append one zero digit
    // per leading zero byte, map digit values to characters and reverse.
    let mut len = 0;
    for &byte in input {
        let mut carry = usize::from(byte);
        for digit in &mut output[..len] {
            carry += usize::from(*digit) << 8;
            *digit = (carry % 58) as u8;
            carry /= 58;
        }
        while carry > 0 {
            let digit = output.get_mut(len).ok_or(Base58Error::BufferTooSmall)?;
            *digit = (carry % 58) as u8;
            len += 1;
            carry /= 58;
        }
    }
    for _ in input.iter().take_while(|byte| **byte == 0) {
        let digit = output.get_mut(len).ok_or(Base58Error::BufferTooSmall)?;
        *digit = 0;
        len += 1;
    }
    let encoded = &mut output[..len];
    for digit in encoded.iter_mut() {
        *digit = ALPHABET[usize::from(*digit)];
    }
    encoded.reverse();
    Ok(len)
}

/// Decode Base58 `input` into the front of `output`.
///
/// Returns the number of decoded bytes, including one zero byte per leading `1`.
///
/// # Errors
/// [`Base58Error::InvalidCharacter`] for a character outside the alphabet and
/// [`Base58Error::BufferTooSmall`] when the decoded value does not fit `output`.
/// Characters are examined in order and the first failure is reported.
pub(crate) fn decode_into(input: &str, output: &mut [u8]) -> Result<usize, Base58Error> {
    // Accumulate base-256 bytes little-endian, then append one zero byte per
    // leading `1` and reverse into big-endian order.
    let mut len = 0;
    for &character in input.as_bytes() {
        let value = DIGIT_VALUES
            .get(usize::from(character))
            .copied()
            .filter(|value| *value != NOT_A_DIGIT)
            .ok_or(Base58Error::InvalidCharacter)?;
        let mut carry = usize::from(value);
        for byte in &mut output[..len] {
            carry += usize::from(*byte) * 58;
            *byte = (carry & 0xFF) as u8;
            carry >>= 8;
        }
        while carry > 0 {
            let byte = output.get_mut(len).ok_or(Base58Error::BufferTooSmall)?;
            *byte = (carry & 0xFF) as u8;
            len += 1;
            carry >>= 8;
        }
    }
    for _ in input
        .bytes()
        .take_while(|character| *character == ALPHABET[0])
    {
        let byte = output.get_mut(len).ok_or(Base58Error::BufferTooSmall)?;
        *byte = 0;
        len += 1;
    }
    output[..len].reverse();
    Ok(len)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(input: &[u8]) -> String {
        let mut output = vec![0; max_encoded_len(input.len())];
        let len = encode_into(input, &mut output).expect("bound always fits");
        String::from_utf8(output[..len].to_vec()).expect("Base58 is ASCII")
    }

    fn decode(input: &str) -> Result<Vec<u8>, Base58Error> {
        let mut output = vec![0; input.len()];
        let len = decode_into(input, &mut output)?;
        Ok(output[..len].to_vec())
    }

    // Vectors from the Base58 test suite shared by Bitcoin Core and its ports.
    const VECTORS: &[(&[u8], &str)] = &[
        (b"", ""),
        (&[0x61], "2g"),
        (&[0x62, 0x62, 0x62], "a3gV"),
        (&[0x63, 0x63, 0x63], "aPEr"),
        (b"simply a long string", "2cFupjhnEsSn59qHXstmK2ffpLv2"),
        (
            &[
                0x00, 0xeb, 0x15, 0x23, 0x1d, 0xfc, 0xeb, 0x60, 0x92, 0x58, 0x86, 0xb6, 0x7d, 0x06,
                0x52, 0x99, 0x92, 0x59, 0x15, 0xae, 0xb1, 0x72, 0xc0, 0x66, 0x47,
            ],
            "1NS17iag9jJgTHD1VXjvLCEnZuQ3rJDE9L",
        ),
        (&[0x51, 0x6b, 0x6f, 0xcd, 0x0f], "ABnLTmg"),
        (
            &[0xbf, 0x4f, 0x89, 0x00, 0x1e, 0x67, 0x02, 0x74, 0xdd],
            "3SEo3LWLoPntC",
        ),
        (&[0x57, 0x2e, 0x47, 0x94], "3EFU7m"),
        (
            &[0xec, 0xac, 0x89, 0xca, 0xd9, 0x39, 0x23, 0xc0, 0x23, 0x21],
            "EJDM8drfXA6uyA",
        ),
        (&[0x10, 0xc8, 0x51, 0x1e], "Rt5zm"),
        (&[0x00; 10], "1111111111"),
        (
            &[
                0x00, 0x01, 0x11, 0xd3, 0x8e, 0x5f, 0xc9, 0x07, 0x1f, 0xfc, 0xd2, 0x0b, 0x4a, 0x76,
                0x3c, 0xc9, 0xae, 0x4f, 0x25, 0x2b, 0xb4, 0xe4, 0x8f, 0xd6, 0x6a, 0x83, 0x5e, 0x25,
                0x2a, 0xda, 0x93, 0xff, 0x48, 0x0d, 0x6d, 0xd4, 0x3d, 0xc6, 0x2a, 0x64, 0x11, 0x55,
                0xa5,
            ],
            "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz",
        ),
    ];

    #[test]
    fn known_vectors_round_trip() {
        for (bytes, text) in VECTORS {
            assert_eq!(encode(bytes), *text, "encode {bytes:02x?}");
            assert_eq!(decode(text).as_deref(), Ok(*bytes), "decode {text}");
        }
    }

    #[test]
    fn max_encoded_len_bounds_every_short_input() {
        assert_eq!(max_encoded_len(0), 0);
        assert_eq!(max_encoded_len(1), 2);
        assert_eq!(max_encoded_len(21), 32);
        for len in 0..=64 {
            let worst = vec![0xFF; len];
            let mut output = vec![0; max_encoded_len(len)];
            assert!(encode_into(&worst, &mut output).is_ok(), "len {len}");
        }
    }

    #[test]
    fn encode_reports_short_buffers() {
        let mut output = [0_u8; 3];
        assert_eq!(
            encode_into(&[0x63, 0x63, 0x63], &mut output),
            Err(Base58Error::BufferTooSmall)
        );
        assert_eq!(
            encode_into(&[0, 0, 0, 0], &mut output),
            Err(Base58Error::BufferTooSmall)
        );
        assert_eq!(encode_into(&[0x63, 0x63, 0x63], &mut [0; 4]), Ok(4));
    }

    #[test]
    fn decode_rejects_invalid_text_and_short_buffers() {
        for invalid in ["0", "O", "I", "l", "+", " a", "a ", "\u{e9}", "2g\0"] {
            assert_eq!(
                decode(invalid),
                Err(Base58Error::InvalidCharacter),
                "{invalid:?}"
            );
        }
        let mut output = [0_u8; 1];
        assert_eq!(
            decode_into("a3gV", &mut output),
            Err(Base58Error::BufferTooSmall)
        );
        assert_eq!(
            decode_into("11", &mut output),
            Err(Base58Error::BufferTooSmall)
        );
        assert_eq!(decode_into("1", &mut output), Ok(1));
        assert_eq!(output, [0]);
    }
}
