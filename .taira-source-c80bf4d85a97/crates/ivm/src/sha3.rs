pub fn keccak_f1600(state: &mut [u64; 25]) {
    #[cfg(all(target_os = "macos", feature = "metal"))]
    {
        if crate::vector::metal_keccak_f1600(state) {
            return;
        }
    }
    #[cfg(feature = "cuda")]
    if crate::cuda::keccak_f1600_cuda(state) {
        return;
    }
    keccak_f1600_impl(state);
}

pub fn keccak_f1600_impl(state: &mut [u64; 25]) {
    const RC: [u64; 24] = [
        0x0000000000000001,
        0x0000000000008082,
        0x800000000000808a,
        0x8000000080008000,
        0x000000000000808b,
        0x0000000080000001,
        0x8000000080008081,
        0x8000000000008009,
        0x000000000000008a,
        0x0000000000000088,
        0x0000000080008009,
        0x000000008000000a,
        0x000000008000808b,
        0x800000000000008b,
        0x8000000000008089,
        0x8000000000008003,
        0x8000000000008002,
        0x8000000000000080,
        0x000000000000800a,
        0x800000008000000a,
        0x8000000080008081,
        0x8000000000008080,
        0x0000000080000001,
        0x8000000080008008,
    ];
    const R: [[u32; 5]; 5] = [
        [0, 36, 3, 41, 18],
        [1, 44, 10, 45, 2],
        [62, 6, 43, 15, 61],
        [28, 55, 25, 21, 56],
        [27, 20, 39, 8, 14],
    ];

    for &rc in RC.iter() {
        // Theta
        let mut c = [0u64; 5];
        for x in 0..5 {
            c[x] = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20];
        }
        let mut d = [0u64; 5];
        for x in 0..5 {
            d[x] = c[(x + 4) % 5] ^ c[(x + 1) % 5].rotate_left(1);
        }
        for x in 0..5 {
            for y in 0..5 {
                state[x + 5 * y] ^= d[x];
            }
        }
        // Rho + Pi
        let mut b = [0u64; 25];
        for x in 0..5 {
            for y in 0..5 {
                let rot = R[x][y];
                let newx = y;
                let newy = (2 * x + 3 * y) % 5;
                b[newx + 5 * newy] = state[x + 5 * y].rotate_left(rot);
            }
        }
        // Chi
        for y in 0..5 {
            for x in 0..5 {
                state[x + 5 * y] =
                    b[x + 5 * y] ^ ((!b[(x + 1) % 5 + 5 * y]) & b[(x + 2) % 5 + 5 * y]);
            }
        }
        // Iota
        state[0] ^= rc;
    }
}

/// Absorb one 136-byte block and run the Keccak-f permutation.
pub fn sha3_absorb_block(state: &mut [u64; 25], block: &[u8; 136]) {
    for i in 0..17 {
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&block[i * 8..i * 8 + 8]);
        let lane = u64::from_le_bytes(bytes);
        state[i] ^= lane;
    }
    keccak_f1600(state);
}
