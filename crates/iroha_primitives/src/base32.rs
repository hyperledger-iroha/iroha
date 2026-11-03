//! Lowercase, unpadded RFC 4648 base32 for content identifiers and DNS labels.
//!
//! Only the alphabet and encoding used by Iroha are implemented. The encoder
//! emits lowercase directly, without an intermediate uppercase allocation.

const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";

/// Encode bytes using the lowercase RFC 4648 base32 alphabet without padding.
///
/// Five input bytes produce eight characters. An incomplete final group has
/// zero trailing bits and emits only the characters needed for its input.
/// The output uses one allocation; empty input does not allocate.
#[must_use]
pub fn encode_lower(data: &[u8]) -> String {
    let mut output = vec![0; encoded_len(data.len()).expect("base32 output too large")];
    let (blocks, tail) = data.as_chunks::<5>();
    let (encoded, rest) = output.as_chunks_mut::<8>();
    for (&block, destination) in blocks.iter().zip(encoded) {
        *destination = encode_block(block);
    }
    if !tail.is_empty() {
        let mut block = [0; 5];
        block[..tail.len()].copy_from_slice(tail);
        rest.copy_from_slice(&encode_block(block)[..rest.len()]);
    }
    String::from_utf8(output).expect("base32 alphabet is ASCII")
}

fn encoded_len(length: usize) -> Option<usize> {
    (length / 5)
        .checked_mul(8)?
        .checked_add(((length % 5) * 8).div_ceil(5))
}

// A 2 KiB table emits two symbols per lookup, halving lookups for full blocks.
// Entries are byte arrays so their output is independent of native endianness.
const PAIRS: [[u8; 2]; 1024] = {
    let mut pairs = [[0; 2]; 1024];
    let mut index = 0;
    while index < 1024 {
        pairs[index] = [ALPHABET[index >> 5], ALPHABET[index & 31]];
        index += 1;
    }
    pairs
};

fn encode_block(block: [u8; 5]) -> [u8; 8] {
    let word = u64::from_be_bytes([0, 0, 0, block[0], block[1], block[2], block[3], block[4]]);
    let first = PAIRS[(word >> 30) as usize];
    let second = PAIRS[((word >> 20) & 1023) as usize];
    let third = PAIRS[((word >> 10) & 1023) as usize];
    let fourth = PAIRS[(word & 1023) as usize];
    [
        first[0], first[1], second[0], second[1], third[0], third[1], fourth[0], fourth[1],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    // A deliberately independent, bit-by-bit oracle exercises group boundaries.
    fn reference(data: &[u8]) -> String {
        let mut output = String::new();
        for start in (0..data.len() * 8).step_by(5) {
            let mut symbol = 0;
            for bit in start..start + 5 {
                symbol <<= 1;
                if let Some(byte) = data.get(bit / 8) {
                    symbol |= (byte >> (7 - bit % 8)) & 1;
                }
            }
            output.push(char::from(ALPHABET[usize::from(symbol)]));
        }
        output
    }

    #[test]
    fn rfc4648_vectors_and_binary_content_id() {
        for (input, expected) in [
            ("", ""),
            ("f", "my"),
            ("fo", "mzxq"),
            ("foo", "mzxw6"),
            ("foob", "mzxw6yq"),
            ("fooba", "mzxw6ytb"),
            ("foobar", "mzxw6ytboi"),
        ] {
            assert_eq!(encode_lower(input.as_bytes()), expected);
        }
        assert_eq!(
            encode_lower(&[0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]),
            "aerukz4jvpg66"
        );
    }

    #[test]
    fn exhaustive_short_inputs_match_bitwise_reference() {
        for byte in u8::MIN..=u8::MAX {
            assert_eq!(encode_lower(&[byte]), reference(&[byte]));
            for second in u8::MIN..=u8::MAX {
                assert_eq!(encode_lower(&[byte, second]), reference(&[byte, second]));
            }
        }
    }

    #[test]
    fn blocks_and_every_tail_length_match_bitwise_reference() {
        let input: Vec<_> = (0..1024).map(|i| (i * 173 + 41) as u8).collect();
        for length in 0..=input.len() {
            let encoded = encode_lower(&input[..length]);
            assert_eq!(encoded, reference(&input[..length]));
            assert_eq!(Some(encoded.len()), encoded_len(length));
        }
        for position in 0..40 {
            let mut block = [0; 5];
            block[position / 8] = 1 << (position % 8);
            assert_eq!(encode_block(block).as_slice(), reference(&block).as_bytes());
        }
        assert_eq!(encode_block([0xff; 5]), *b"77777777");
    }

    #[test]
    fn length_calculation_rejects_overflow() {
        assert_eq!(encoded_len(0), Some(0));
        assert_eq!(encoded_len(5), Some(8));
        assert_eq!(encoded_len(usize::MAX), None);
        assert_eq!(encoded_len(usize::MAX / 8 * 5), Some(usize::MAX / 8 * 8));
    }
}
