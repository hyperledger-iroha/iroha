#![allow(clippy::missing_const_for_fn)]

//! NEON-accelerated helpers for the SM4 block cipher.
//!
//! The functions exposed here are safe wrappers around unsafe NEON intrinsics.
//! They return `None` when NEON is unavailable so upstream callers can fall
//! back to the scalar RustCrypto implementation without panicking.

#[cfg(target_arch = "aarch64")]
const CK: [u32; 32] = [
    0x0007_0E15,
    0x1C23_2A31,
    0x383F_464D,
    0x545B_6269,
    0x7077_7E85,
    0x8C93_9AA1,
    0xA8AF_B6BD,
    0xC4CB_D2D9,
    0xE0E7_EEF5,
    0xFC03_0A11,
    0x181F_262D,
    0x343B_4249,
    0x5057_5E65,
    0x6C73_7A81,
    0x888F_969D,
    0xA4AB_B2B9,
    0xC0C7_CED5,
    0xDCE3_EAF1,
    0xF8FF_060D,
    0x141B_2229,
    0x3037_3E45,
    0x4C53_5A61,
    0x686F_767D,
    0x848B_9299,
    0xA0A7_AEB5,
    0xBCC3_CAD1,
    0xD8DF_E6ED,
    0xF4FB_0209,
    0x1017_1E25,
    0x2C33_3A41,
    0x484F_565D,
    0x646B_7279,
];

#[cfg(target_arch = "aarch64")]
#[rustfmt::skip]
const SM4_SBOX: [u8; 256] = [
    0xD6, 0x90, 0xE9, 0xFE, 0xCC, 0xE1, 0x3D, 0xB7, 0x16, 0xB6, 0x14, 0xC2, 0x28, 0xFB, 0x2C, 0x05,
    0x2B, 0x67, 0x9A, 0x76, 0x2A, 0xBE, 0x04, 0xC3, 0xAA, 0x44, 0x13, 0x26, 0x49, 0x86, 0x06, 0x99,
    0x9C, 0x42, 0x50, 0xF4, 0x91, 0xEF, 0x98, 0x7A, 0x33, 0x54, 0x0B, 0x43, 0xED, 0xCF, 0xAC, 0x62,
    0xE4, 0xB3, 0x1C, 0xA9, 0xC9, 0x08, 0xE8, 0x95, 0x80, 0xDF, 0x94, 0xFA, 0x75, 0x8F, 0x3F, 0xA6,
    0x47, 0x07, 0xA7, 0xFC, 0xF3, 0x73, 0x17, 0xBA, 0x83, 0x59, 0x3C, 0x19, 0xE6, 0x85, 0x4F, 0xA8,
    0x68, 0x6B, 0x81, 0xB2, 0x71, 0x64, 0xDA, 0x8B, 0xF8, 0xEB, 0x0F, 0x4B, 0x70, 0x56, 0x9D, 0x35,
    0x1E, 0x24, 0x0E, 0x5E, 0x63, 0x58, 0xD1, 0xA2, 0x25, 0x22, 0x7C, 0x3B, 0x01, 0x21, 0x78, 0x87,
    0xD4, 0x00, 0x46, 0x57, 0x9F, 0xD3, 0x27, 0x52, 0x4C, 0x36, 0x02, 0xE7, 0xA0, 0xC4, 0xC8, 0x9E,
    0xEA, 0xBF, 0x8A, 0xD2, 0x40, 0xC7, 0x38, 0xB5, 0xA3, 0xF7, 0xF2, 0xCE, 0xF9, 0x61, 0x15, 0xA1,
    0xE0, 0xAE, 0x5D, 0xA4, 0x9B, 0x34, 0x1A, 0x55, 0xAD, 0x93, 0x32, 0x30, 0xF5, 0x8C, 0xB1, 0xE3,
    0x1D, 0xF6, 0xE2, 0x2E, 0x82, 0x66, 0xCA, 0x60, 0xC0, 0x29, 0x23, 0xAB, 0x0D, 0x53, 0x4E, 0x6F,
    0xD5, 0xDB, 0x37, 0x45, 0xDE, 0xFD, 0x8E, 0x2F, 0x03, 0xFF, 0x6A, 0x72, 0x6D, 0x6C, 0x5B, 0x51,
    0x8D, 0x1B, 0xAF, 0x92, 0xBB, 0xDD, 0xBC, 0x7F, 0x11, 0xD9, 0x5C, 0x41, 0x1F, 0x10, 0x5A, 0xD8,
    0x0A, 0xC1, 0x31, 0x88, 0xA5, 0xCD, 0x7B, 0xBD, 0x2D, 0x74, 0xD0, 0x12, 0xB8, 0xE5, 0xB4, 0xB0,
    0x89, 0x69, 0x97, 0x4A, 0x0C, 0x96, 0x77, 0x7E, 0x65, 0xB9, 0xF1, 0x09, 0xC5, 0x6E, 0xC6, 0x84,
    0x18, 0xF0, 0x7D, 0xEC, 0x3A, 0xDC, 0x4D, 0x20, 0x79, 0xEE, 0x5F, 0x3E, 0xD7, 0xCB, 0x39, 0x48,
];

#[cfg(target_arch = "aarch64")]
mod neon {
    use std::sync::OnceLock;

    use super::{CK, SM4_SBOX};

    type NeonTable = core::arch::aarch64::uint8x16x4_t;

    static SBOX_TABLES: OnceLock<[NeonTable; 4]> = OnceLock::new();

    #[inline]
    pub fn is_supported() -> bool {
        cfg!(feature = "force-neon") || std::arch::is_aarch64_feature_detected!("neon")
    }

    #[allow(unsafe_code)]
    pub fn encrypt_block(key: &[u8; 16], block: &[u8; 16]) -> Option<[u8; 16]> {
        if !is_supported() {
            return None;
        }
        Some(unsafe { encrypt_block_neon(key, block) })
    }

    #[allow(unsafe_code)]
    pub fn decrypt_block(key: &[u8; 16], block: &[u8; 16]) -> Option<[u8; 16]> {
        if !is_supported() {
            return None;
        }
        Some(unsafe { decrypt_block_neon(key, block) })
    }

    #[allow(unsafe_code)]
    #[target_feature(enable = "neon")]
    unsafe fn encrypt_block_neon(key: &[u8; 16], block: &[u8; 16]) -> [u8; 16] {
        let round_keys = expand_round_keys(key);
        unsafe { apply_round_keys(block, &round_keys) }
    }

    #[allow(unsafe_code)]
    #[target_feature(enable = "neon")]
    unsafe fn decrypt_block_neon(key: &[u8; 16], block: &[u8; 16]) -> [u8; 16] {
        let mut round_keys = expand_round_keys(key);
        round_keys.reverse();
        unsafe { apply_round_keys(block, &round_keys) }
    }

    #[allow(unsafe_code)]
    #[target_feature(enable = "neon")]
    unsafe fn apply_round_keys(block: &[u8; 16], rk: &[u32; 32]) -> [u8; 16] {
        let mut x0 = u32::from_be_bytes(block[0..4].try_into().expect("SM4 block"));
        let mut x1 = u32::from_be_bytes(block[4..8].try_into().expect("SM4 block"));
        let mut x2 = u32::from_be_bytes(block[8..12].try_into().expect("SM4 block"));
        let mut x3 = u32::from_be_bytes(block[12..16].try_into().expect("SM4 block"));

        for &rk_i in rk {
            let tmp = x1 ^ x2 ^ x3 ^ rk_i;
            let transformed = unsafe { round_transform(tmp) };
            let new = x0 ^ transformed;
            x0 = x1;
            x1 = x2;
            x2 = x3;
            x3 = new;
        }

        let mut out = [0u8; 16];
        out[0..4].copy_from_slice(&x3.to_be_bytes());
        out[4..8].copy_from_slice(&x2.to_be_bytes());
        out[8..12].copy_from_slice(&x1.to_be_bytes());
        out[12..16].copy_from_slice(&x0.to_be_bytes());
        out
    }

    #[allow(unsafe_code)]
    #[target_feature(enable = "neon")]
    unsafe fn round_transform(value: u32) -> u32 {
        let broadcast = core::arch::aarch64::vdupq_n_u32(value);
        let bytes = core::arch::aarch64::vreinterpretq_u8_u32(broadcast);
        let substituted = unsafe { sub_bytes(bytes) };
        let substituted_u32 = core::arch::aarch64::vreinterpretq_u32_u8(substituted);
        let scalar = core::arch::aarch64::vgetq_lane_u32(substituted_u32, 0);
        unsafe { linear_transform(scalar) }
    }

    #[allow(unsafe_code)]
    #[target_feature(enable = "neon")]
    unsafe fn sub_bytes(bytes: core::arch::aarch64::uint8x16_t) -> core::arch::aarch64::uint8x16_t {
        let tables = sbox_tables();
        let selector = core::arch::aarch64::vshrq_n_u8(bytes, 6);
        let indices = core::arch::aarch64::vandq_u8(bytes, core::arch::aarch64::vdupq_n_u8(0x3F));
        let mut result = core::arch::aarch64::vdupq_n_u8(0);
        for (idx, table) in tables.iter().enumerate() {
            let mask =
                core::arch::aarch64::vceqq_u8(selector, core::arch::aarch64::vdupq_n_u8(idx as u8));
            let values = core::arch::aarch64::vqtbl4q_u8(*table, indices);
            result = core::arch::aarch64::vbslq_u8(mask, values, result);
        }
        result
    }

    #[allow(unsafe_code)]
    #[target_feature(enable = "neon")]
    unsafe fn linear_transform(value: u32) -> u32 {
        let v = core::arch::aarch64::vdupq_n_u32(value);
        let rotl2 = unsafe { rotl32(v, 2) };
        let rotl10 = unsafe { rotl32(v, 10) };
        let rotl18 = unsafe { rotl32(v, 18) };
        let rotl24 = unsafe { rotl32(v, 24) };
        let combined = core::arch::aarch64::veorq_u32(
            core::arch::aarch64::veorq_u32(v, rotl2),
            core::arch::aarch64::veorq_u32(core::arch::aarch64::veorq_u32(rotl10, rotl18), rotl24),
        );
        core::arch::aarch64::vgetq_lane_u32(combined, 0)
    }

    #[allow(unsafe_code)]
    #[target_feature(enable = "neon")]
    unsafe fn rotl32(
        value: core::arch::aarch64::uint32x4_t,
        n: i32,
    ) -> core::arch::aarch64::uint32x4_t {
        let left = core::arch::aarch64::vshlq_u32(value, core::arch::aarch64::vdupq_n_s32(n));
        let right = core::arch::aarch64::vshlq_u32(value, core::arch::aarch64::vdupq_n_s32(n - 32));
        core::arch::aarch64::vorrq_u32(left, right)
    }

    #[inline]
    fn expand_round_keys(key: &[u8; 16]) -> [u32; 32] {
        const FK: [u32; 4] = [0xA3B1_BAC6, 0x56AA_3350, 0x677D_9197, 0xB270_22DC];
        let mut k = [0u32; 4];
        for (slot, chunk) in k.iter_mut().zip(key.chunks_exact(4)) {
            *slot = u32::from_be_bytes(chunk.try_into().expect("SM4 key chunk"));
        }
        for (slot, fk) in k.iter_mut().zip(FK) {
            *slot ^= fk;
        }

        let mut rk = [0u32; 32];
        for (i, entry) in rk.iter_mut().enumerate() {
            let temp = k[1] ^ k[2] ^ k[3] ^ CK[i];
            let transformed = key_schedule_transform(temp);
            k[0] ^= transformed;
            *entry = k[0];
            k.rotate_left(1);
        }
        rk
    }

    #[inline]
    fn key_schedule_transform(value: u32) -> u32 {
        let substituted = substitute_word(value);
        substituted ^ substituted.rotate_left(13) ^ substituted.rotate_left(23)
    }

    #[inline]
    fn substitute_word(value: u32) -> u32 {
        let b0 = u32::from(SM4_SBOX[(value >> 24) as usize]);
        let b1 = u32::from(SM4_SBOX[((value >> 16) & 0xFF) as usize]);
        let b2 = u32::from(SM4_SBOX[((value >> 8) & 0xFF) as usize]);
        let b3 = u32::from(SM4_SBOX[(value & 0xFF) as usize]);
        (b0 << 24) | (b1 << 16) | (b2 << 8) | b3
    }

    fn sbox_tables() -> &'static [NeonTable; 4] {
        SBOX_TABLES.get_or_init(|| {
            let mut chunks = [[0u8; 64]; 4];
            for (i, &value) in SM4_SBOX.iter().enumerate() {
                chunks[i >> 6][i & 0x3F] = value;
            }
            chunks.map(|chunk| unsafe { core::arch::aarch64::vld1q_u8_x4(chunk.as_ptr()) })
        })
    }
}

#[cfg(target_arch = "aarch64")]
pub use neon::{decrypt_block, encrypt_block, is_supported};

#[cfg(not(target_arch = "aarch64"))]
#[inline]
pub fn is_supported() -> bool {
    false
}

#[cfg(not(target_arch = "aarch64"))]
#[allow(clippy::unused_unit)]
pub fn encrypt_block(_: &[u8; 16], _: &[u8; 16]) -> Option<[u8; 16]> {
    None
}

#[cfg(not(target_arch = "aarch64"))]
#[allow(clippy::unused_unit)]
pub fn decrypt_block(_: &[u8; 16], _: &[u8; 16]) -> Option<[u8; 16]> {
    None
}

#[cfg(all(test, target_arch = "aarch64"))]
mod tests {
    use super::{CK, SM4_SBOX, decrypt_block, encrypt_block, is_supported};

    #[test]
    fn neon_round_trip_matches_scalar() {
        if !is_supported() {
            eprintln!("skipping NEON SM4 test: NEON not available");
            return;
        }

        for seed in render_seeds() {
            let (key, block) = render_case(seed);
            let scalar_enc = sm4_scalar_encrypt(&key, &block);
            let neon_enc = encrypt_block(&key, &block).expect("NEON encrypt");
            assert_eq!(
                scalar_enc, neon_enc,
                "SM4 NEON encrypt mismatch for seed {seed:#04x}"
            );

            let neon_dec =
                decrypt_block(&key, &scalar_enc).expect("NEON decrypt must succeed when enabled");
            assert_eq!(
                block, neon_dec,
                "SM4 NEON decrypt mismatch for seed {seed:#04x}"
            );

            let scalar_dec = sm4_scalar_decrypt(&key, &neon_enc);
            assert_eq!(
                block, scalar_dec,
                "SM4 scalar decrypt mismatch for seed {seed:#04x}"
            );
        }
    }

    fn render_seeds() -> [u8; 12] {
        [
            0x00, 0x01, 0x02, 0x10, 0x22, 0x45, 0x7F, 0x80, 0xA5, 0xC3, 0xEE, 0xFF,
        ]
    }

    fn render_case(seed: u8) -> ([u8; 16], [u8; 16]) {
        let mut key = [0u8; 16];
        let mut block = [0u8; 16];
        for (idx, slot) in key.iter_mut().enumerate() {
            *slot = seed
                .wrapping_mul(0x11)
                .wrapping_add((idx as u8).wrapping_mul(0x07));
        }
        for (idx, slot) in block.iter_mut().enumerate() {
            *slot = seed
                .wrapping_mul(0x31)
                .wrapping_add((idx as u8).wrapping_mul(0x0B));
        }
        (key, block)
    }

    fn sm4_scalar_encrypt(key: &[u8; 16], block: &[u8; 16]) -> [u8; 16] {
        let round_keys = scalar_round_keys(key);
        scalar_apply_rounds(block, &round_keys)
    }

    fn sm4_scalar_decrypt(key: &[u8; 16], block: &[u8; 16]) -> [u8; 16] {
        let mut round_keys = scalar_round_keys(key);
        round_keys.reverse();
        scalar_apply_rounds(block, &round_keys)
    }

    fn scalar_round_keys(key: &[u8; 16]) -> [u32; 32] {
        const FK: [u32; 4] = [0xA3B1_BAC6, 0x56AA_3350, 0x677D_9197, 0xB270_22DC];
        let mut k = [0u32; 4];
        for (slot, chunk) in k.iter_mut().zip(key.chunks_exact(4)) {
            *slot = u32::from_be_bytes(chunk.try_into().expect("SM4 key chunk"));
        }
        for (slot, fk) in k.iter_mut().zip(FK) {
            *slot ^= fk;
        }

        let mut rk = [0u32; 32];
        for (i, entry) in rk.iter_mut().enumerate() {
            let temp = k[1] ^ k[2] ^ k[3] ^ CK[i];
            let transformed = scalar_key_schedule_transform(temp);
            k[0] ^= transformed;
            *entry = k[0];
            k.rotate_left(1);
        }
        rk
    }

    fn scalar_apply_rounds(block: &[u8; 16], rk: &[u32; 32]) -> [u8; 16] {
        let mut x0 = u32::from_be_bytes(block[0..4].try_into().expect("SM4 block"));
        let mut x1 = u32::from_be_bytes(block[4..8].try_into().expect("SM4 block"));
        let mut x2 = u32::from_be_bytes(block[8..12].try_into().expect("SM4 block"));
        let mut x3 = u32::from_be_bytes(block[12..16].try_into().expect("SM4 block"));

        for &rk_i in rk {
            let tmp = x1 ^ x2 ^ x3 ^ rk_i;
            let transformed = scalar_round_transform(tmp);
            let new = x0 ^ transformed;
            x0 = x1;
            x1 = x2;
            x2 = x3;
            x3 = new;
        }

        let mut out = [0u8; 16];
        out[0..4].copy_from_slice(&x3.to_be_bytes());
        out[4..8].copy_from_slice(&x2.to_be_bytes());
        out[8..12].copy_from_slice(&x1.to_be_bytes());
        out[12..16].copy_from_slice(&x0.to_be_bytes());
        out
    }

    fn scalar_key_schedule_transform(value: u32) -> u32 {
        let substituted = scalar_substitute_word(value);
        substituted ^ substituted.rotate_left(13) ^ substituted.rotate_left(23)
    }

    fn scalar_round_transform(value: u32) -> u32 {
        let substituted = scalar_substitute_word(value);
        substituted
            ^ substituted.rotate_left(2)
            ^ substituted.rotate_left(10)
            ^ substituted.rotate_left(18)
            ^ substituted.rotate_left(24)
    }

    fn scalar_substitute_word(value: u32) -> u32 {
        let b0 = u32::from(SM4_SBOX[(value >> 24) as usize]);
        let b1 = u32::from(SM4_SBOX[((value >> 16) & 0xFF) as usize]);
        let b2 = u32::from(SM4_SBOX[((value >> 8) & 0xFF) as usize]);
        let b3 = u32::from(SM4_SBOX[(value & 0xFF) as usize]);
        (b0 << 24) | (b1 << 16) | (b2 << 8) | b3
    }
}
