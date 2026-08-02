//! Table-free AES-256 block encryption for holder-secret sampling.
//!
//! This deliberately does not use IVM's accelerator-dispatch AES helpers:
//! holder randomness must never be copied to Metal/CUDA devices, and a
//! secret-indexed S-box table is not an acceptable software fallback.  The
//! implementation follows the FIPS 197 AES-256 schedule and round function
//! with fixed-control-flow GF(2^8) arithmetic only.

use zeroize::Zeroizing;

const AES_BLOCK_BYTES: usize = 16;
const AES_256_KEY_BYTES: usize = 32;
const AES_256_ROUND_KEYS: usize = 15;
const AES_256_SCHEDULE_WORDS: usize = 60;
const RCON: [u8; 7] = [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40];

/// Owning AES-256 key schedule for one holder-randomness stream.
///
/// The schedule is neither cloneable nor printable and is wiped on drop.
pub(super) struct ConstantTimeAes256KeyV1 {
    round_keys: Zeroizing<[[u8; AES_BLOCK_BYTES]; AES_256_ROUND_KEYS]>,
}

impl core::fmt::Debug for ConstantTimeAes256KeyV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ConstantTimeAes256KeyV1(<redacted>)")
    }
}

impl ConstantTimeAes256KeyV1 {
    /// Expand one exact 256-bit key into its 15 round keys.
    #[must_use]
    pub(super) fn new(key: &[u8; AES_256_KEY_BYTES]) -> Self {
        let mut round_keys = Zeroizing::new([[0_u8; AES_BLOCK_BYTES]; AES_256_ROUND_KEYS]);
        round_keys[0].copy_from_slice(&key[..AES_BLOCK_BYTES]);
        round_keys[1].copy_from_slice(&key[AES_BLOCK_BYTES..]);

        for index in 8..AES_256_SCHEDULE_WORDS {
            let mut temporary = Zeroizing::new(schedule_word(&round_keys, index - 1));
            if index % 8 == 0 {
                temporary.rotate_left(1);
                sub_word(&mut temporary);
                temporary[0] ^= RCON[index / 8 - 1];
            } else if index % 8 == 4 {
                sub_word(&mut temporary);
            }
            let previous = Zeroizing::new(schedule_word(&round_keys, index - 8));
            let mut next = Zeroizing::new([0_u8; 4]);
            for byte in 0..4 {
                next[byte] = previous[byte] ^ temporary[byte];
            }
            set_schedule_word(&mut round_keys, index, &next);
        }

        Self { round_keys }
    }

    /// Encrypt one block without accelerator or device dispatch.
    #[must_use]
    pub(super) fn encrypt_block(&self, block: [u8; AES_BLOCK_BYTES]) -> [u8; AES_BLOCK_BYTES] {
        let mut state = Zeroizing::new(block);
        add_round_key(&mut state, &self.round_keys[0]);
        for round_key in &self.round_keys[1..AES_256_ROUND_KEYS - 1] {
            sub_bytes(&mut state);
            shift_rows(&mut state);
            mix_columns(&mut state);
            add_round_key(&mut state, round_key);
        }
        sub_bytes(&mut state);
        shift_rows(&mut state);
        add_round_key(&mut state, &self.round_keys[AES_256_ROUND_KEYS - 1]);
        *state
    }
}

fn schedule_word(
    round_keys: &[[u8; AES_BLOCK_BYTES]; AES_256_ROUND_KEYS],
    index: usize,
) -> [u8; 4] {
    let round = index / 4;
    let offset = (index % 4) * 4;
    core::array::from_fn(|byte| round_keys[round][offset + byte])
}

fn set_schedule_word(
    round_keys: &mut [[u8; AES_BLOCK_BYTES]; AES_256_ROUND_KEYS],
    index: usize,
    word: &[u8; 4],
) {
    let round = index / 4;
    let offset = (index % 4) * 4;
    for byte in 0..4 {
        round_keys[round][offset + byte] = word[byte];
    }
}

fn sub_word(word: &mut [u8; 4]) {
    for byte in word {
        *byte = aes_sbox(*byte);
    }
}

fn add_round_key(state: &mut [u8; AES_BLOCK_BYTES], round_key: &[u8; AES_BLOCK_BYTES]) {
    for (byte, key_byte) in state.iter_mut().zip(round_key) {
        *byte ^= key_byte;
    }
}

fn sub_bytes(state: &mut [u8; AES_BLOCK_BYTES]) {
    for byte in state {
        *byte = aes_sbox(*byte);
    }
}

fn shift_rows(state: &mut [u8; AES_BLOCK_BYTES]) {
    let previous = Zeroizing::new(*state);
    state[1] = previous[5];
    state[5] = previous[9];
    state[9] = previous[13];
    state[13] = previous[1];
    state[2] = previous[10];
    state[6] = previous[14];
    state[10] = previous[2];
    state[14] = previous[6];
    state[3] = previous[15];
    state[7] = previous[3];
    state[11] = previous[7];
    state[15] = previous[11];
}

fn mix_columns(state: &mut [u8; AES_BLOCK_BYTES]) {
    for column in 0..4 {
        let offset = column * 4;
        let a0 = state[offset];
        let a1 = state[offset + 1];
        let a2 = state[offset + 2];
        let a3 = state[offset + 3];
        let sum = a0 ^ a1 ^ a2 ^ a3;
        state[offset] = a0 ^ sum ^ aes_xtime(a0 ^ a1);
        state[offset + 1] = a1 ^ sum ^ aes_xtime(a1 ^ a2);
        state[offset + 2] = a2 ^ sum ^ aes_xtime(a2 ^ a3);
        state[offset + 3] = a3 ^ sum ^ aes_xtime(a3 ^ a0);
    }
}

#[inline]
fn aes_xtime(value: u8) -> u8 {
    let reduction_mask = 0_u8.wrapping_sub(value >> 7);
    (value << 1) ^ (0x1b & reduction_mask)
}

#[inline]
fn gf256_multiply(mut left: u8, mut right: u8) -> u8 {
    let mut product = 0_u8;
    for _ in 0..8 {
        let selection_mask = 0_u8.wrapping_sub(right & 1);
        product ^= left & selection_mask;
        left = aes_xtime(left);
        right >>= 1;
    }
    product
}

fn aes_sbox(value: u8) -> u8 {
    // Inversion as value^254; zero maps to zero.  The fixed addition chain
    // has no data-dependent branch or memory access.
    let power_2 = gf256_multiply(value, value);
    let power_4 = gf256_multiply(power_2, power_2);
    let power_8 = gf256_multiply(power_4, power_4);
    let power_16 = gf256_multiply(power_8, power_8);
    let power_32 = gf256_multiply(power_16, power_16);
    let power_64 = gf256_multiply(power_32, power_32);
    let power_128 = gf256_multiply(power_64, power_64);
    let mut inverse = gf256_multiply(power_2, power_4);
    inverse = gf256_multiply(inverse, power_8);
    inverse = gf256_multiply(inverse, power_16);
    inverse = gf256_multiply(inverse, power_32);
    inverse = gf256_multiply(inverse, power_64);
    inverse = gf256_multiply(inverse, power_128);
    inverse
        ^ inverse.rotate_left(1)
        ^ inverse.rotate_left(2)
        ^ inverse.rotate_left(3)
        ^ inverse.rotate_left(4)
        ^ 0x63
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIPS_SBOX: [u8; 256] = [
        0x63, 0x7c, 0x77, 0x7b, 0xf2, 0x6b, 0x6f, 0xc5, 0x30, 0x01, 0x67, 0x2b, 0xfe, 0xd7, 0xab,
        0x76, 0xca, 0x82, 0xc9, 0x7d, 0xfa, 0x59, 0x47, 0xf0, 0xad, 0xd4, 0xa2, 0xaf, 0x9c, 0xa4,
        0x72, 0xc0, 0xb7, 0xfd, 0x93, 0x26, 0x36, 0x3f, 0xf7, 0xcc, 0x34, 0xa5, 0xe5, 0xf1, 0x71,
        0xd8, 0x31, 0x15, 0x04, 0xc7, 0x23, 0xc3, 0x18, 0x96, 0x05, 0x9a, 0x07, 0x12, 0x80, 0xe2,
        0xeb, 0x27, 0xb2, 0x75, 0x09, 0x83, 0x2c, 0x1a, 0x1b, 0x6e, 0x5a, 0xa0, 0x52, 0x3b, 0xd6,
        0xb3, 0x29, 0xe3, 0x2f, 0x84, 0x53, 0xd1, 0x00, 0xed, 0x20, 0xfc, 0xb1, 0x5b, 0x6a, 0xcb,
        0xbe, 0x39, 0x4a, 0x4c, 0x58, 0xcf, 0xd0, 0xef, 0xaa, 0xfb, 0x43, 0x4d, 0x33, 0x85, 0x45,
        0xf9, 0x02, 0x7f, 0x50, 0x3c, 0x9f, 0xa8, 0x51, 0xa3, 0x40, 0x8f, 0x92, 0x9d, 0x38, 0xf5,
        0xbc, 0xb6, 0xda, 0x21, 0x10, 0xff, 0xf3, 0xd2, 0xcd, 0x0c, 0x13, 0xec, 0x5f, 0x97, 0x44,
        0x17, 0xc4, 0xa7, 0x7e, 0x3d, 0x64, 0x5d, 0x19, 0x73, 0x60, 0x81, 0x4f, 0xdc, 0x22, 0x2a,
        0x90, 0x88, 0x46, 0xee, 0xb8, 0x14, 0xde, 0x5e, 0x0b, 0xdb, 0xe0, 0x32, 0x3a, 0x0a, 0x49,
        0x06, 0x24, 0x5c, 0xc2, 0xd3, 0xac, 0x62, 0x91, 0x95, 0xe4, 0x79, 0xe7, 0xc8, 0x37, 0x6d,
        0x8d, 0xd5, 0x4e, 0xa9, 0x6c, 0x56, 0xf4, 0xea, 0x65, 0x7a, 0xae, 0x08, 0xba, 0x78, 0x25,
        0x2e, 0x1c, 0xa6, 0xb4, 0xc6, 0xe8, 0xdd, 0x74, 0x1f, 0x4b, 0xbd, 0x8b, 0x8a, 0x70, 0x3e,
        0xb5, 0x66, 0x48, 0x03, 0xf6, 0x0e, 0x61, 0x35, 0x57, 0xb9, 0x86, 0xc1, 0x1d, 0x9e, 0xe1,
        0xf8, 0x98, 0x11, 0x69, 0xd9, 0x8e, 0x94, 0x9b, 0x1e, 0x87, 0xe9, 0xce, 0x55, 0x28, 0xdf,
        0x8c, 0xa1, 0x89, 0x0d, 0xbf, 0xe6, 0x42, 0x68, 0x41, 0x99, 0x2d, 0x0f, 0xb0, 0x54, 0xbb,
        0x16,
    ];

    #[test]
    fn table_free_sbox_matches_every_fips_value() {
        for (input, expected) in FIPS_SBOX.iter().copied().enumerate() {
            assert_eq!(
                aes_sbox(u8::try_from(input).expect("S-box input fits u8")),
                expected,
                "AES S-box mismatch at {input}"
            );
        }
    }

    #[test]
    fn aes256_matches_fips_197_appendix_c3() {
        let key = [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b,
            0x1c, 0x1d, 0x1e, 0x1f,
        ];
        let plaintext = [
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ];
        let expected = [
            0x8e, 0xa2, 0xb7, 0xca, 0x51, 0x67, 0x45, 0xbf, 0xea, 0xfc, 0x49, 0x90, 0x4b, 0x49,
            0x60, 0x89,
        ];
        assert_eq!(
            ConstantTimeAes256KeyV1::new(&key).encrypt_block(plaintext),
            expected
        );
    }

    #[test]
    fn aes256_uses_every_key_and_plaintext_byte() {
        let key = core::array::from_fn(|index| {
            u8::try_from(index * 7 + 3).expect("fixed key byte fits u8")
        });
        let plaintext = core::array::from_fn(|index| {
            u8::try_from(255 - index * 11).expect("fixed block byte fits u8")
        });
        let baseline = ConstantTimeAes256KeyV1::new(&key).encrypt_block(plaintext);
        for index in 0..key.len() {
            let mut changed = key;
            changed[index] ^= 1;
            assert_ne!(
                ConstantTimeAes256KeyV1::new(&changed).encrypt_block(plaintext),
                baseline,
                "AES-256 ignored key byte {index}"
            );
        }
        for index in 0..plaintext.len() {
            let mut changed = plaintext;
            changed[index] ^= 1;
            assert_ne!(
                ConstantTimeAes256KeyV1::new(&key).encrypt_block(changed),
                baseline,
                "AES-256 ignored plaintext byte {index}"
            );
        }
    }
}
