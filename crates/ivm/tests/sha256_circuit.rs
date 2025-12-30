#![cfg(feature = "ivm_zk_tests")]
use ivm::halo2::Sha256BlockCircuit;
use rand_core::{OsRng, RngCore};

fn sha256_compress_ref(state: [u32; 8], block: &[u8; 64]) -> [u32; 8] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut w = [0u32; 64];
    for (t, chunk) in block.chunks(4).enumerate().take(16) {
        w[t] = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }
    for t in 16..64 {
        let s0 = w[t - 15].rotate_right(7) ^ w[t - 15].rotate_right(18) ^ (w[t - 15] >> 3);
        let s1 = w[t - 2].rotate_right(17) ^ w[t - 2].rotate_right(19) ^ (w[t - 2] >> 10);
        w[t] = w[t - 16]
            .wrapping_add(s0)
            .wrapping_add(w[t - 7])
            .wrapping_add(s1);
    }
    let mut a = state[0];
    let mut b = state[1];
    let mut c = state[2];
    let mut d = state[3];
    let mut e = state[4];
    let mut f = state[5];
    let mut g = state[6];
    let mut h = state[7];
    for t in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ ((!e) & g);
        let temp1 = h
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(K[t])
            .wrapping_add(w[t]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let temp2 = s0.wrapping_add(maj);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(temp1);
        d = c;
        c = b;
        b = a;
        a = temp1.wrapping_add(temp2);
    }
    let mut out = [0u32; 8];
    out[0] = state[0].wrapping_add(a);
    out[1] = state[1].wrapping_add(b);
    out[2] = state[2].wrapping_add(c);
    out[3] = state[3].wrapping_add(d);
    out[4] = state[4].wrapping_add(e);
    out[5] = state[5].wrapping_add(f);
    out[6] = state[6].wrapping_add(g);
    out[7] = state[7].wrapping_add(h);
    out
}

#[test]
fn test_sha256block_circuit_known_vector() {
    let block = [0u8; 64];
    let state = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let expected = sha256_compress_ref(state, &block);
    let circuit = Sha256BlockCircuit {
        state,
        block,
        result: expected,
    };
    assert!(circuit.verify().is_ok());
}

#[test]
fn test_sha256block_circuit_random() {
    let mut rng = OsRng;
    let mut block = [0u8; 64];
    rng.fill_bytes(&mut block);
    let mut state = [0u32; 8];
    for s in &mut state {
        *s = rng.next_u32();
    }
    let expected = sha256_compress_ref(state, &block);
    let circuit = Sha256BlockCircuit {
        state,
        block,
        result: expected,
    };
    assert!(circuit.verify().is_ok());
}

#[test]
fn test_sha256block_circuit_bad_witness() {
    let block = [0u8; 64];
    let state = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut expected = sha256_compress_ref(state, &block);
    expected[0] ^= 1; // flip a bit
    let circuit = Sha256BlockCircuit {
        state,
        block,
        result: expected,
    };
    assert!(circuit.verify().is_err());
}

#[test]
fn test_sha256_ch_maj() {
    fn ch(x: u32, y: u32, z: u32) -> u32 {
        (x & y) ^ ((!x) & z)
    }
    fn maj(x: u32, y: u32, z: u32) -> u32 {
        (x & y) ^ (x & z) ^ (y & z)
    }
    assert_eq!(ch(0xffff_ffff, 0x1234_5678, 0x9abc_def0), 0x1234_5678);
    assert_eq!(ch(0, 0x1234_5678, 0x9abc_def0), 0x9abc_def0);
    assert_eq!(maj(0xffff_ffff, 0, 0), 0);
    assert_eq!(maj(0xffff_ffff, 0xffff_ffff, 0), 0xffff_ffff);
}

#[test]
fn test_addition_wraparound() {
    let a: u32 = 0xffff_ffff;
    let b: u32 = 1;
    let sum = a.wrapping_add(b);
    assert_eq!(sum, 0);
}
