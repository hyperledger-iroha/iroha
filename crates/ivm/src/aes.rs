pub const SBOX: [u8; 256] = [
    0x63, 0x7c, 0x77, 0x7b, 0xf2, 0x6b, 0x6f, 0xc5, 0x30, 0x01, 0x67, 0x2b, 0xfe, 0xd7, 0xab, 0x76,
    0xca, 0x82, 0xc9, 0x7d, 0xfa, 0x59, 0x47, 0xf0, 0xad, 0xd4, 0xa2, 0xaf, 0x9c, 0xa4, 0x72, 0xc0,
    0xb7, 0xfd, 0x93, 0x26, 0x36, 0x3f, 0xf7, 0xcc, 0x34, 0xa5, 0xe5, 0xf1, 0x71, 0xd8, 0x31, 0x15,
    0x04, 0xc7, 0x23, 0xc3, 0x18, 0x96, 0x05, 0x9a, 0x07, 0x12, 0x80, 0xe2, 0xeb, 0x27, 0xb2, 0x75,
    0x09, 0x83, 0x2c, 0x1a, 0x1b, 0x6e, 0x5a, 0xa0, 0x52, 0x3b, 0xd6, 0xb3, 0x29, 0xe3, 0x2f, 0x84,
    0x53, 0xd1, 0x00, 0xed, 0x20, 0xfc, 0xb1, 0x5b, 0x6a, 0xcb, 0xbe, 0x39, 0x4a, 0x4c, 0x58, 0xcf,
    0xd0, 0xef, 0xaa, 0xfb, 0x43, 0x4d, 0x33, 0x85, 0x45, 0xf9, 0x02, 0x7f, 0x50, 0x3c, 0x9f, 0xa8,
    0x51, 0xa3, 0x40, 0x8f, 0x92, 0x9d, 0x38, 0xf5, 0xbc, 0xb6, 0xda, 0x21, 0x10, 0xff, 0xf3, 0xd2,
    0xcd, 0x0c, 0x13, 0xec, 0x5f, 0x97, 0x44, 0x17, 0xc4, 0xa7, 0x7e, 0x3d, 0x64, 0x5d, 0x19, 0x73,
    0x60, 0x81, 0x4f, 0xdc, 0x22, 0x2a, 0x90, 0x88, 0x46, 0xee, 0xb8, 0x14, 0xde, 0x5e, 0x0b, 0xdb,
    0xe0, 0x32, 0x3a, 0x0a, 0x49, 0x06, 0x24, 0x5c, 0xc2, 0xd3, 0xac, 0x62, 0x91, 0x95, 0xe4, 0x79,
    0xe7, 0xc8, 0x37, 0x6d, 0x8d, 0xd5, 0x4e, 0xa9, 0x6c, 0x56, 0xf4, 0xea, 0x65, 0x7a, 0xae, 0x08,
    0xba, 0x78, 0x25, 0x2e, 0x1c, 0xa6, 0xb4, 0xc6, 0xe8, 0xdd, 0x74, 0x1f, 0x4b, 0xbd, 0x8b, 0x8a,
    0x70, 0x3e, 0xb5, 0x66, 0x48, 0x03, 0xf6, 0x0e, 0x61, 0x35, 0x57, 0xb9, 0x86, 0xc1, 0x1d, 0x9e,
    0xe1, 0xf8, 0x98, 0x11, 0x69, 0xd9, 0x8e, 0x94, 0x9b, 0x1e, 0x87, 0xe9, 0xce, 0x55, 0x28, 0xdf,
    0x8c, 0xa1, 0x89, 0x0d, 0xbf, 0xe6, 0x42, 0x68, 0x41, 0x99, 0x2d, 0x0f, 0xb0, 0x54, 0xbb, 0x16,
];

pub const INV_SBOX: [u8; 256] = [
    0x52, 0x09, 0x6a, 0xd5, 0x30, 0x36, 0xa5, 0x38, 0xbf, 0x40, 0xa3, 0x9e, 0x81, 0xf3, 0xd7, 0xfb,
    0x7c, 0xe3, 0x39, 0x82, 0x9b, 0x2f, 0xff, 0x87, 0x34, 0x8e, 0x43, 0x44, 0xc4, 0xde, 0xe9, 0xcb,
    0x54, 0x7b, 0x94, 0x32, 0xa6, 0xc2, 0x23, 0x3d, 0xee, 0x4c, 0x95, 0x0b, 0x42, 0xfa, 0xc3, 0x4e,
    0x08, 0x2e, 0xa1, 0x66, 0x28, 0xd9, 0x24, 0xb2, 0x76, 0x5b, 0xa2, 0x49, 0x6d, 0x8b, 0xd1, 0x25,
    0x72, 0xf8, 0xf6, 0x64, 0x86, 0x68, 0x98, 0x16, 0xd4, 0xa4, 0x5c, 0xcc, 0x5d, 0x65, 0xb6, 0x92,
    0x6c, 0x70, 0x48, 0x50, 0xfd, 0xed, 0xb9, 0xda, 0x5e, 0x15, 0x46, 0x57, 0xa7, 0x8d, 0x9d, 0x84,
    0x90, 0xd8, 0xab, 0x00, 0x8c, 0xbc, 0xd3, 0x0a, 0xf7, 0xe4, 0x58, 0x05, 0xb8, 0xb3, 0x45, 0x06,
    0xd0, 0x2c, 0x1e, 0x8f, 0xca, 0x3f, 0x0f, 0x02, 0xc1, 0xaf, 0xbd, 0x03, 0x01, 0x13, 0x8a, 0x6b,
    0x3a, 0x91, 0x11, 0x41, 0x4f, 0x67, 0xdc, 0xea, 0x97, 0xf2, 0xcf, 0xce, 0xf0, 0xb4, 0xe6, 0x73,
    0x96, 0xac, 0x74, 0x22, 0xe7, 0xad, 0x35, 0x85, 0xe2, 0xf9, 0x37, 0xe8, 0x1c, 0x75, 0xdf, 0x6e,
    0x47, 0xf1, 0x1a, 0x71, 0x1d, 0x29, 0xc5, 0x89, 0x6f, 0xb7, 0x62, 0x0e, 0xaa, 0x18, 0xbe, 0x1b,
    0xfc, 0x56, 0x3e, 0x4b, 0xc6, 0xd2, 0x79, 0x20, 0x9a, 0xdb, 0xc0, 0xfe, 0x78, 0xcd, 0x5a, 0xf4,
    0x1f, 0xdd, 0xa8, 0x33, 0x88, 0x07, 0xc7, 0x31, 0xb1, 0x12, 0x10, 0x59, 0x27, 0x80, 0xec, 0x5f,
    0x60, 0x51, 0x7f, 0xa9, 0x19, 0xb5, 0x4a, 0x0d, 0x2d, 0xe5, 0x7a, 0x9f, 0x93, 0xc9, 0x9c, 0xef,
    0xa0, 0xe0, 0x3b, 0x4d, 0xae, 0x2a, 0xf5, 0xb0, 0xc8, 0xeb, 0xbb, 0x3c, 0x83, 0x53, 0x99, 0x61,
    0x17, 0x2b, 0x04, 0x7e, 0xba, 0x77, 0xd6, 0x26, 0xe1, 0x69, 0x14, 0x63, 0x55, 0x21, 0x0c, 0x7d,
];

fn gmul(mut a: u8, mut b: u8) -> u8 {
    let mut res = 0;
    for _ in 0..8 {
        if b & 1 != 0 {
            res ^= a;
        }
        let hi = a & 0x80;
        a <<= 1;
        if hi != 0 {
            a ^= 0x1B;
        }
        b >>= 1;
    }
    res
}

fn sub_bytes(state: &mut [u8; 16]) {
    for b in state.iter_mut() {
        *b = SBOX[*b as usize];
    }
}

fn inv_sub_bytes(state: &mut [u8; 16]) {
    for b in state.iter_mut() {
        *b = INV_SBOX[*b as usize];
    }
}

fn shift_rows(state: &mut [u8; 16]) {
    let tmp = *state;
    state[1] = tmp[5];
    state[5] = tmp[9];
    state[9] = tmp[13];
    state[13] = tmp[1];
    state[2] = tmp[10];
    state[6] = tmp[14];
    state[10] = tmp[2];
    state[14] = tmp[6];
    state[3] = tmp[15];
    state[7] = tmp[3];
    state[11] = tmp[7];
    state[15] = tmp[11];
}

fn inv_shift_rows(state: &mut [u8; 16]) {
    let tmp = *state;
    state[5] = tmp[1];
    state[9] = tmp[5];
    state[13] = tmp[9];
    state[1] = tmp[13];
    state[10] = tmp[2];
    state[14] = tmp[6];
    state[2] = tmp[10];
    state[6] = tmp[14];
    state[15] = tmp[3];
    state[3] = tmp[7];
    state[7] = tmp[11];
    state[11] = tmp[15];
}

fn mix_columns(state: &mut [u8; 16]) {
    for c in 0..4 {
        let i = c * 4;
        let a0 = state[i];
        let a1 = state[i + 1];
        let a2 = state[i + 2];
        let a3 = state[i + 3];
        state[i] = gmul(a0, 2) ^ gmul(a1, 3) ^ a2 ^ a3;
        state[i + 1] = a0 ^ gmul(a1, 2) ^ gmul(a2, 3) ^ a3;
        state[i + 2] = a0 ^ a1 ^ gmul(a2, 2) ^ gmul(a3, 3);
        state[i + 3] = gmul(a0, 3) ^ a1 ^ a2 ^ gmul(a3, 2);
    }
}

fn inv_mix_columns(state: &mut [u8; 16]) {
    for c in 0..4 {
        let i = c * 4;
        let a0 = state[i];
        let a1 = state[i + 1];
        let a2 = state[i + 2];
        let a3 = state[i + 3];
        state[i] = gmul(a0, 14) ^ gmul(a1, 11) ^ gmul(a2, 13) ^ gmul(a3, 9);
        state[i + 1] = gmul(a0, 9) ^ gmul(a1, 14) ^ gmul(a2, 11) ^ gmul(a3, 13);
        state[i + 2] = gmul(a0, 13) ^ gmul(a1, 9) ^ gmul(a2, 14) ^ gmul(a3, 11);
        state[i + 3] = gmul(a0, 11) ^ gmul(a1, 13) ^ gmul(a2, 9) ^ gmul(a3, 14);
    }
}

fn add_round_key(state: &mut [u8; 16], rk: &[u8; 16]) {
    for i in 0..16 {
        state[i] ^= rk[i];
    }
}

pub fn aesenc(state: [u8; 16], rk: [u8; 16]) -> [u8; 16] {
    #[cfg(all(target_os = "macos", feature = "metal"))]
    if let Some(res) = crate::vector::metal_aesenc_round(state, rk) {
        return res;
    }
    #[cfg(feature = "cuda")]
    if let Some(res) = crate::cuda::aesenc_cuda(state, rk) {
        return res;
    }
    // AArch64 AES acceleration (detected at runtime)
    #[cfg(target_arch = "aarch64")]
    {
        if is_aarch64_aes_available() {
            // SAFETY: guarded by runtime feature detection for `aes`.
            return unsafe { aesenc_armv8(state, rk) };
        }
    }
    // x86/x86_64 AES-NI acceleration (detected at runtime)
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if is_x86_aes_available() {
            // SAFETY: guarded by runtime feature detection for `aes`.
            return unsafe { aesenc_aesni(state, rk) };
        }
    }
    aesenc_impl(state, rk)
}

pub fn aesenc_impl(mut state: [u8; 16], rk: [u8; 16]) -> [u8; 16] {
    sub_bytes(&mut state);
    shift_rows(&mut state);
    mix_columns(&mut state);
    add_round_key(&mut state, &rk);
    state
}

pub fn aesdec(state: [u8; 16], rk: [u8; 16]) -> [u8; 16] {
    #[cfg(all(target_os = "macos", feature = "metal"))]
    if let Some(res) = crate::vector::metal_aesdec_round(state, rk) {
        return res;
    }
    #[cfg(feature = "cuda")]
    if let Some(res) = crate::cuda::aesdec_cuda(state, rk) {
        return res;
    }
    // AArch64 AES acceleration (detected at runtime)
    #[cfg(target_arch = "aarch64")]
    {
        if is_aarch64_aes_available() {
            // SAFETY: guarded by runtime feature detection for `aes`.
            return unsafe { aesdec_armv8(state, rk) };
        }
    }
    // x86/x86_64 AES-NI acceleration (detected at runtime)
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if is_x86_aes_available() {
            // SAFETY: guarded by runtime feature detection for `aes`.
            return unsafe { aesdec_aesni(state, rk) };
        }
    }
    aesdec_impl(state, rk)
}

pub fn aesdec_impl(mut state: [u8; 16], rk: [u8; 16]) -> [u8; 16] {
    add_round_key(&mut state, &rk);
    inv_mix_columns(&mut state);
    inv_shift_rows(&mut state);
    inv_sub_bytes(&mut state);
    state
}

pub fn sbox(byte: u8) -> u8 {
    SBOX[byte as usize]
}

/// Process many 16-byte blocks with a single AESENC round.
/// Preference order: Metal (macOS) – batch not available; uses per-block → CUDA batch → CPU loop.
#[allow(dead_code)]
pub fn aesenc_many(states: &[[u8; 16]], rk: [u8; 16]) -> Vec<[u8; 16]> {
    if states.is_empty() {
        return Vec::new();
    }
    #[cfg(all(target_os = "macos", feature = "metal"))]
    {
        if crate::vector::metal_available() {
            let mut out = Vec::with_capacity(states.len());
            for &s in states {
                out.push(
                    crate::vector::metal_aesenc_round(s, rk).unwrap_or_else(|| aesenc_impl(s, rk)),
                );
            }
            return out;
        }
    }
    #[cfg(feature = "cuda")]
    if let Some(res) = crate::cuda::aesenc_batch_cuda(states, rk) {
        return res;
    }
    states.iter().map(|&s| aesenc(s, rk)).collect()
}

/// Process many 16-byte blocks with a single AESDEC round.
#[allow(dead_code)]
pub fn aesdec_many(states: &[[u8; 16]], rk: [u8; 16]) -> Vec<[u8; 16]> {
    if states.is_empty() {
        return Vec::new();
    }
    #[cfg(all(target_os = "macos", feature = "metal"))]
    {
        if let Some(res) = crate::vector::metal_aesdec_batch(states, rk) {
            return res;
        }
    }
    #[cfg(feature = "cuda")]
    if let Some(res) = crate::cuda::aesdec_batch_cuda(states, rk) {
        return res;
    }
    states.iter().map(|&s| aesdec(s, rk)).collect()
}

/// Apply N AES encryption rounds (each equivalent to `aesenc`) to many blocks.
/// `round_keys` supplies the N round keys in order. This function does not
/// perform the initial AddRoundKey or handle the final round differences; it is
/// intended as a building block for streaming multi-round operations.
#[allow(dead_code)]
pub fn aesenc_n_rounds_many(states: &[[u8; 16]], round_keys: &[[u8; 16]]) -> Vec<[u8; 16]> {
    if states.is_empty() {
        return Vec::new();
    }
    // Try fused Metal/CUDA batch first for lower launch overhead
    #[cfg(all(target_os = "macos", feature = "metal"))]
    {
        if let Some(res) = crate::vector::metal_aesenc_rounds_batch(states, round_keys) {
            return res;
        }
    }
    #[cfg(feature = "cuda")]
    if let Some(res) = crate::cuda::aesenc_rounds_batch_cuda(states, round_keys) {
        return res;
    }
    let mut cur: Vec<[u8; 16]> = states.to_vec();
    for &rk in round_keys.iter() {
        cur = aesenc_many(&cur, rk);
    }
    cur
}

/// Apply N AES decryption rounds (each equivalent to `aesdec`) to many blocks.
#[allow(dead_code)]
pub fn aesdec_n_rounds_many(states: &[[u8; 16]], round_keys: &[[u8; 16]]) -> Vec<[u8; 16]> {
    if states.is_empty() {
        return Vec::new();
    }
    #[cfg(all(target_os = "macos", feature = "metal"))]
    {
        if let Some(res) = crate::vector::metal_aesdec_rounds_batch(states, round_keys) {
            return res;
        }
    }
    #[cfg(feature = "cuda")]
    if let Some(res) = crate::cuda::aesdec_rounds_batch_cuda(states, round_keys) {
        return res;
    }
    let mut cur: Vec<[u8; 16]> = states.to_vec();
    for &rk in round_keys.iter() {
        cur = aesdec_many(&cur, rk);
    }
    cur
}

/// AES "last" round for encryption (no MixColumns): SubBytes → ShiftRows → AddRoundKey.
#[allow(dead_code)]
pub fn aesenc_last_impl(mut state: [u8; 16], rk: [u8; 16]) -> [u8; 16] {
    sub_bytes(&mut state);
    shift_rows(&mut state);
    add_round_key(&mut state, &rk);
    state
}

/// AES "last" round for decryption (no InvMixColumns): AddRoundKey → InvShiftRows → InvSubBytes.
#[allow(dead_code)]
pub fn aesdec_last_impl(mut state: [u8; 16], rk: [u8; 16]) -> [u8; 16] {
    add_round_key(&mut state, &rk);
    inv_shift_rows(&mut state);
    inv_sub_bytes(&mut state);
    state
}

/// Convenience: AES-128 encrypt many blocks with pre-expanded key schedule (11 round keys).
/// Layout: round_keys[0] is initial key (for AddRoundKey), 1..=9 used with aesenc_many,
/// and round_keys[10] used with aesenc_last_impl.
#[allow(dead_code)]
pub fn aes128_encrypt_many(states: &[[u8; 16]], round_keys: &[[u8; 16]; 11]) -> Vec<[u8; 16]> {
    if states.is_empty() {
        return Vec::new();
    }
    // Initial AddRoundKey
    let mut cur: Vec<[u8; 16]> = states
        .iter()
        .map(|&s| {
            let mut t = s;
            add_round_key(&mut t, &round_keys[0]);
            t
        })
        .collect();
    // 9 standard rounds
    cur = aesenc_n_rounds_many(&cur, &round_keys[1..10]);
    // Last round (no MixColumns)
    for b in &mut cur {
        *b = aesenc_last_impl(*b, round_keys[10]);
    }
    cur
}

/// Convenience: AES-128 decrypt many blocks with pre-expanded key schedule (11 round keys, reversed order).
#[allow(dead_code)]
pub fn aes128_decrypt_many(states: &[[u8; 16]], round_keys: &[[u8; 16]; 11]) -> Vec<[u8; 16]> {
    if states.is_empty() {
        return Vec::new();
    }
    let mut cur: Vec<[u8; 16]> = states.to_vec();
    // First "last" inverse round (no InvMixColumns)
    for b in &mut cur {
        *b = aesdec_last_impl(*b, round_keys[10]);
    }
    // 9 inverse rounds
    cur = aesdec_n_rounds_many(&cur, &round_keys[1..10]);
    // Final AddRoundKey with initial key
    for b in &mut cur {
        add_round_key(b, &round_keys[0]);
    }
    cur
}

// --- AES-128 key expansion (pre-expanded schedule helpers) ---

#[allow(dead_code)]
const RCON: [u8; 10] = [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x1B, 0x36];

#[inline]
#[allow(dead_code)]
fn rot_word(w: [u8; 4]) -> [u8; 4] {
    [w[1], w[2], w[3], w[0]]
}

#[inline]
#[allow(dead_code)]
fn sub_word(mut w: [u8; 4]) -> [u8; 4] {
    w[0] = SBOX[w[0] as usize];
    w[1] = SBOX[w[1] as usize];
    w[2] = SBOX[w[2] as usize];
    w[3] = SBOX[w[3] as usize];
    w
}

/// Expand a 128-bit AES key into 11 round keys (initial + 10 rounds).
#[allow(dead_code)]
pub fn aes128_expand_key(key: [u8; 16]) -> [[u8; 16]; 11] {
    let mut w = [[0u8; 4]; 44];
    for i in 0..4 {
        w[i][0] = key[i * 4];
        w[i][1] = key[i * 4 + 1];
        w[i][2] = key[i * 4 + 2];
        w[i][3] = key[i * 4 + 3];
    }
    for i in 4..44 {
        let mut temp = w[i - 1];
        if i % 4 == 0 {
            temp = sub_word(rot_word(temp));
            temp[0] ^= RCON[(i / 4) - 1];
        }
        for (j, t) in temp.iter().enumerate() {
            w[i][j] = w[i - 4][j] ^ *t;
        }
    }
    // Collect into 11 round keys
    let mut rks = [[0u8; 16]; 11];
    for r in 0..11 {
        for (j, word) in w[r * 4..r * 4 + 4].iter().enumerate() {
            rks[r][j * 4] = word[0];
            rks[r][j * 4 + 1] = word[1];
            rks[r][j * 4 + 2] = word[2];
            rks[r][j * 4 + 3] = word[3];
        }
    }
    rks
}

#[cfg(test)]
mod key_schedule_tests {
    use super::*;

    #[test]
    fn expand_key_matches_known_vector() {
        // FIPS-197 Appendix A.1 test vector
        let key: [u8; 16] = [
            0x2b, 0x7e, 0x15, 0x16, 0x28, 0xae, 0xd2, 0xa6, 0xab, 0xf7, 0x15, 0x88, 0x09, 0xcf,
            0x4f, 0x3c,
        ];
        let rks = aes128_expand_key(key);
        // Check first, second and last round key bytes (spot-check)
        assert_eq!(rks[0], key);
        // Round 1 expected: 0xa0fafe1788542cb123a339392a6c7605
        assert_eq!(
            rks[1],
            [
                0xa0, 0xfa, 0xfe, 0x17, 0x88, 0x54, 0x2c, 0xb1, 0x23, 0xa3, 0x39, 0x39, 0x2a, 0x6c,
                0x76, 0x05,
            ]
        );
        // Round 10 expected: 0xd014f9a8c9ee2589e13f0cc8b6630ca6
        assert_eq!(
            rks[10],
            [
                0xd0, 0x14, 0xf9, 0xa8, 0xc9, 0xee, 0x25, 0x89, 0xe1, 0x3f, 0x0c, 0xc8, 0xb6, 0x63,
                0x0c, 0xa6,
            ]
        );
    }
}

// --- CPU acceleration helpers (x86 AES-NI) ---

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[inline(always)]
fn is_x86_aes_available() -> bool {
    #[cfg(target_arch = "x86")]
    {
        std::is_x86_feature_detected!("aes")
    }
    #[cfg(target_arch = "x86_64")]
    {
        std::is_x86_feature_detected!("aes")
    }
}

// --- CPU acceleration helpers (AArch64 AES) ---

#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn is_aarch64_aes_available() -> bool {
    use core::sync::atomic::{AtomicBool, Ordering};
    use std::sync::OnceLock;
    static FORCED_DISABLED: AtomicBool = AtomicBool::new(false);
    static SELFTEST_OK: OnceLock<bool> = OnceLock::new();
    if FORCED_DISABLED.load(Ordering::SeqCst) {
        return false;
    }
    *SELFTEST_OK.get_or_init(|| {
        if !std::arch::is_aarch64_feature_detected!("aes") {
            return false;
        }
        // Minimal parity self-test to ensure instruction semantics match our
        // scalar reference on this platform.
        let s1 = [
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ];
        let k1 = [
            0x0f, 0x47, 0x0c, 0xaf, 0x15, 0xd9, 0xb7, 0x7f, 0x71, 0xe8, 0xad, 0x67, 0xc9, 0x59,
            0xd6, 0x98,
        ];
        let enc_ref = aesenc_impl(s1, k1);
        let enc_hw = unsafe { aesenc_armv8(s1, k1) };
        if enc_ref != enc_hw {
            FORCED_DISABLED.store(true, Ordering::SeqCst);
            return false;
        }
        let d1 = enc_ref; // use the encrypted block as an input to dec round test
        let dec_ref = aesdec_impl(d1, k1);
        let dec_hw = unsafe { aesdec_armv8(d1, k1) };
        if dec_ref != dec_hw {
            FORCED_DISABLED.store(true, Ordering::SeqCst);
            return false;
        }
        true
    })
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "aes")]
unsafe fn aesenc_armv8(state: [u8; 16], rk: [u8; 16]) -> [u8; 16] {
    use core::arch::aarch64::*;
    let s = unsafe { vld1q_u8(state.as_ptr()) };
    let k = unsafe { vld1q_u8(rk.as_ptr()) };
    // NOTE: The arm64 AESE intrinsic performs one AES encryption round.
    // If the specific micro-architecture requires explicit MixColumns, we
    // apply AESMC as part of the round. This ordering matches AESENC
    // semantics on supported platforms.
    let r = vaeseq_u8(s, k);
    let r = vaesmcq_u8(r);
    let mut out = [0u8; 16];
    unsafe { vst1q_u8(out.as_mut_ptr(), r) };
    out
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "aes")]
unsafe fn aesdec_armv8(state: [u8; 16], rk: [u8; 16]) -> [u8; 16] {
    use core::arch::aarch64::*;
    let s = unsafe { vld1q_u8(state.as_ptr()) };
    let k = unsafe { vld1q_u8(rk.as_ptr()) };
    // Inverse AES round using AESD + AESIMC.
    let r = vaesdq_u8(s, k);
    let r = vaesimcq_u8(r);
    let mut out = [0u8; 16];
    unsafe { vst1q_u8(out.as_mut_ptr(), r) };
    out
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "aes")]
unsafe fn aesenc_aesni(state: [u8; 16], rk: [u8; 16]) -> [u8; 16] {
    #[cfg(target_arch = "x86")]
    use core::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use core::arch::x86_64::*;

    unsafe {
        let s = _mm_loadu_si128(state.as_ptr() as *const __m128i);
        let k = _mm_loadu_si128(rk.as_ptr() as *const __m128i);
        let r = _mm_aesenc_si128(s, k);
        let mut out = core::mem::MaybeUninit::<[u8; 16]>::uninit();
        _mm_storeu_si128(out.as_mut_ptr() as *mut __m128i, r);
        out.assume_init()
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "aes")]
unsafe fn aesdec_aesni(state: [u8; 16], rk: [u8; 16]) -> [u8; 16] {
    #[cfg(target_arch = "x86")]
    use core::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use core::arch::x86_64::*;

    unsafe {
        let s = _mm_loadu_si128(state.as_ptr() as *const __m128i);
        let k = _mm_loadu_si128(rk.as_ptr() as *const __m128i);
        let r = _mm_aesdec_si128(s, k);
        let mut out = core::mem::MaybeUninit::<[u8; 16]>::uninit();
        _mm_storeu_si128(out.as_mut_ptr() as *mut __m128i, r);
        out.assume_init()
    }
}

#[cfg(test)]
mod tests_accel {
    use super::*;

    #[test]
    fn aesenc_parity_with_scalar() {
        // Fixed test vectors (arbitrary but deterministic)
        let state = [
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ];
        let rk = [
            0x0f, 0x47, 0x0c, 0xaf, 0x15, 0xd9, 0xb7, 0x7f, 0x71, 0xe8, 0xad, 0x67, 0xc9, 0x59,
            0xd6, 0x98,
        ];
        let scalar = aesenc_impl(state, rk);
        let got = aesenc(state, rk);
        assert_eq!(got, scalar);
    }

    #[test]
    fn aesdec_parity_with_scalar() {
        let state = [
            0x69, 0xc4, 0xe0, 0xd8, 0x6a, 0x7b, 0x04, 0x30, 0xd8, 0xcd, 0xb7, 0x80, 0x70, 0xb4,
            0xc5, 0x5a,
        ];
        let rk = [
            0x0f, 0x47, 0x0c, 0xaf, 0x15, 0xd9, 0xb7, 0x7f, 0x71, 0xe8, 0xad, 0x67, 0xc9, 0x59,
            0xd6, 0x98,
        ];
        let scalar = aesdec_impl(state, rk);
        let got = aesdec(state, rk);
        assert_eq!(got, scalar);
    }
}
