//! Dependency-free SHA-256 used by the Microsoft Vega compatibility boundary.
use thiserror::Error;
const INITIAL_STATE: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];
const ROUND_CONSTANTS: [u32; 64] = [
    0x428a_2f98,
    0x7137_4491,
    0xb5c0_fbcf,
    0xe9b5_dba5,
    0x3956_c25b,
    0x59f1_11f1,
    0x923f_82a4,
    0xab1c_5ed5,
    0xd807_aa98,
    0x1283_5b01,
    0x2431_85be,
    0x550c_7dc3,
    0x72be_5d74,
    0x80de_b1fe,
    0x9bdc_06a7,
    0xc19b_f174,
    0xe49b_69c1,
    0xefbe_4786,
    0x0fc1_9dc6,
    0x240c_a1cc,
    0x2de9_2c6f,
    0x4a74_84aa,
    0x5cb0_a9dc,
    0x76f9_88da,
    0x983e_5152,
    0xa831_c66d,
    0xb003_27c8,
    0xbf59_7fc7,
    0xc6e0_0bf3,
    0xd5a7_9147,
    0x06ca_6351,
    0x1429_2967,
    0x27b7_0a85,
    0x2e1b_2138,
    0x4d2c_6dfc,
    0x5338_0d13,
    0x650a_7354,
    0x766a_0abb,
    0x81c2_c92e,
    0x9272_2c85,
    0xa2bf_e8a1,
    0xa81a_664b,
    0xc24b_8b70,
    0xc76c_51a3,
    0xd192_e819,
    0xd699_0624,
    0xf40e_3585,
    0x106a_a070,
    0x19a4_c116,
    0x1e37_6c08,
    0x2748_774c,
    0x34b0_bcb5,
    0x391c_0cb3,
    0x4ed8_aa4a,
    0x5b9c_ca4f,
    0x682e_6ff3,
    0x748f_82ee,
    0x78a5_636f,
    0x84c8_7814,
    0x8cc7_0208,
    0x90be_fffa,
    0xa450_6ceb,
    0xbef9_a3f7,
    0xc671_78f2,
];
/// Input length exceeded SHA-256's 64-bit bit-length field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(super) enum Sha256Error {
    /// The concatenated input cannot be represented by SHA-256.
    #[error("SHA-256 input length exceeds the canonical 64-bit bit-length field")]
    LengthOverflow,
}
/// Streaming SHA-256 state with no dependency or allocation requirement.
#[derive(Clone)]
pub(super) struct Sha256 {
    state: [u32; 8],
    pending: [u8; 64],
    pending_len: usize,
    total_bytes: u64,
}
impl Sha256 {
    /// Construct an empty SHA-256 state.
    pub(super) const fn new() -> Self {
        Self {
            state: INITIAL_STATE,
            pending: [0; 64],
            pending_len: 0,
            total_bytes: 0,
        }
    }
    /// Absorb one fragment of the digest stream.
    pub(super) fn update(&mut self, mut input: &[u8]) -> Result<(), Sha256Error> {
        let added = u64::try_from(input.len()).map_err(|_| Sha256Error::LengthOverflow)?;
        self.total_bytes = self
            .total_bytes
            .checked_add(added)
            .filter(|bytes| *bytes <= u64::MAX / 8)
            .ok_or(Sha256Error::LengthOverflow)?;
        if self.pending_len != 0 {
            let take = input.len().min(64 - self.pending_len);
            self.pending[self.pending_len..self.pending_len + take].copy_from_slice(&input[..take]);
            self.pending_len += take;
            input = &input[take..];
            if self.pending_len < 64 {
                return Ok(());
            }
            compress(&mut self.state, &self.pending);
            self.pending = [0; 64];
            self.pending_len = 0;
        }
        let mut blocks = input.chunks_exact(64);
        for block in &mut blocks {
            compress(
                &mut self.state,
                block.try_into().expect("exact SHA-256 block"),
            );
        }
        let remainder = blocks.remainder();
        self.pending[..remainder.len()].copy_from_slice(remainder);
        self.pending_len = remainder.len();
        Ok(())
    }
    /// Finalize and return the standard 32-byte digest.
    pub(super) fn finalize(mut self) -> [u8; 32] {
        let bit_len = self.total_bytes * 8;
        self.pending[self.pending_len] = 0x80;
        self.pending_len += 1;
        if self.pending_len > 56 {
            self.pending[self.pending_len..].fill(0);
            compress(&mut self.state, &self.pending);
            self.pending = [0; 64];
        } else {
            self.pending[self.pending_len..56].fill(0);
        }
        self.pending[56..].copy_from_slice(&bit_len.to_be_bytes());
        compress(&mut self.state, &self.pending);
        let mut output = [0_u8; 32];
        for (destination, word) in output.chunks_exact_mut(4).zip(self.state) {
            destination.copy_from_slice(&word.to_be_bytes());
        }
        output
    }
}
/// Hash one complete byte string.
#[cfg(test)]
pub(super) fn sha256(input: &[u8]) -> Result<[u8; 32], Sha256Error> {
    let mut state = Sha256::new();
    state.update(input)?;
    Ok(state.finalize())
}
fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut schedule = [0_u32; 64];
    for (word, bytes) in schedule.iter_mut().zip(block.chunks_exact(4)) {
        *word = u32::from_be_bytes(bytes.try_into().expect("four-byte SHA-256 word"));
    }
    for index in 16..64 {
        let s0 = schedule[index - 15].rotate_right(7)
            ^ schedule[index - 15].rotate_right(18)
            ^ (schedule[index - 15] >> 3);
        let s1 = schedule[index - 2].rotate_right(17)
            ^ schedule[index - 2].rotate_right(19)
            ^ (schedule[index - 2] >> 10);
        schedule[index] = schedule[index - 16]
            .wrapping_add(s0)
            .wrapping_add(schedule[index - 7])
            .wrapping_add(s1);
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for index in 0..64 {
        let sigma1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let choice = (e & f) ^ (!e & g);
        let temporary1 = h
            .wrapping_add(sigma1)
            .wrapping_add(choice)
            .wrapping_add(ROUND_CONSTANTS[index])
            .wrapping_add(schedule[index]);
        let sigma0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let majority = (a & b) ^ (a & c) ^ (b & c);
        let temporary2 = sigma0.wrapping_add(majority);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(temporary1);
        d = c;
        c = b;
        b = a;
        a = temporary1.wrapping_add(temporary2);
    }
    for (destination, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *destination = destination.wrapping_add(value);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sha256_matches_fips_known_answers_and_fragmentation() {
        let cases = [
            (
                b"".as_slice(),
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                b"abc".as_slice(),
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ),
            (
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq".as_slice(),
                "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
            ),
        ];
        for (message, expected) in cases {
            assert_eq!(
                hex::encode(sha256(message).expect("bounded input")),
                expected
            );
            for fragment in 1..=message.len().max(1) {
                let mut state = Sha256::new();
                for chunk in message.chunks(fragment) {
                    state.update(chunk).expect("bounded input");
                }
                assert_eq!(hex::encode(state.finalize()), expected);
            }
        }
    }
    #[test]
    fn sha256_handles_padding_on_both_sides_of_one_block() {
        let fifty_five = [0xa5_u8; 55];
        let fifty_six = [0xa5_u8; 56];
        assert_eq!(
            hex::encode(sha256(&fifty_five).unwrap()),
            "26ee0116778740a66fe2ba10ea063748b27306acc99188ec812746d4e8d70083"
        );
        assert_eq!(
            hex::encode(sha256(&fifty_six).unwrap()),
            "4cf71e2b0aa0fcc0c271f68353026a77b8e50153632a8e4a73833cd64080e92e"
        );
    }
}
