//! Minimal Keccak-f[1600] sponge constructions required by canonical Vega.
//!
//! `tiny-keccak` already supplies the audited permutation under the crate's
//! existing `sha3` feature. Vega needs the original Keccak-256 delimiter and
//! SHAKE256 XOF, so this module applies their standardized sponge padding
//! directly without adding or changing dependencies.

use tiny_keccak::keccakf;

const KECCAK_256_RATE: usize = 136;

pub(super) struct Keccak256 {
    state: [u64; 25],
    pending: [u8; KECCAK_256_RATE],
    pending_len: usize,
}

impl Keccak256 {
    pub(super) const fn new() -> Self {
        Self {
            state: [0; 25],
            pending: [0; KECCAK_256_RATE],
            pending_len: 0,
        }
    }

    pub(super) fn update(&mut self, mut input: &[u8]) {
        if self.pending_len != 0 {
            let take = input
                .len()
                .min(KECCAK_256_RATE.saturating_sub(self.pending_len));
            self.pending[self.pending_len..self.pending_len + take].copy_from_slice(&input[..take]);
            self.pending_len += take;
            input = &input[take..];
            if self.pending_len == KECCAK_256_RATE {
                xor_rate_block(&mut self.state, &self.pending);
                keccakf(&mut self.state);
                self.pending.fill(0);
                self.pending_len = 0;
            } else {
                // `take` consumed all remaining input when it did not fill the
                // pending rate block. Preserve that partial block for the next
                // update instead of resetting `pending_len` below.
                return;
            }
        }

        let mut chunks = input.chunks_exact(KECCAK_256_RATE);
        for chunk in &mut chunks {
            xor_rate_block(&mut self.state, chunk);
            keccakf(&mut self.state);
        }
        let remainder = chunks.remainder();
        self.pending[..remainder.len()].copy_from_slice(remainder);
        self.pending_len = remainder.len();
    }

    pub(super) fn finalize(mut self) -> [u8; 32] {
        self.pending[self.pending_len] ^= 0x01;
        self.pending[KECCAK_256_RATE - 1] ^= 0x80;
        xor_rate_block(&mut self.state, &self.pending);
        keccakf(&mut self.state);

        let mut output = [0_u8; 32];
        for (destination, lane) in output.chunks_exact_mut(8).zip(self.state) {
            destination.copy_from_slice(&lane.to_le_bytes());
        }
        output
    }
}

fn sponge(input: &[u8], delimiter: u8, output_len: usize) -> Vec<u8> {
    let mut state = [0_u64; 25];
    let mut chunks = input.chunks_exact(KECCAK_256_RATE);
    for chunk in &mut chunks {
        xor_rate_block(&mut state, chunk);
        keccakf(&mut state);
    }

    let remainder = chunks.remainder();
    let mut final_block = [0_u8; KECCAK_256_RATE];
    final_block[..remainder.len()].copy_from_slice(remainder);
    final_block[remainder.len()] ^= delimiter;
    final_block[KECCAK_256_RATE - 1] ^= 0x80;
    xor_rate_block(&mut state, &final_block);
    keccakf(&mut state);

    let mut output = Vec::with_capacity(output_len);
    while output.len() < output_len {
        let take = (output_len - output.len()).min(KECCAK_256_RATE);
        for lane in state.iter().take(KECCAK_256_RATE / 8) {
            let bytes = lane.to_le_bytes();
            let remaining = take - output.len() % KECCAK_256_RATE;
            output.extend_from_slice(&bytes[..remaining.min(bytes.len())]);
            if output.len() == output_len || output.len() % KECCAK_256_RATE == 0 {
                break;
            }
        }
        if output.len() < output_len {
            keccakf(&mut state);
        }
    }
    output
}

fn xor_rate_block(state: &mut [u64; 25], block: &[u8]) {
    debug_assert_eq!(block.len(), KECCAK_256_RATE);
    for (lane, bytes) in state
        .iter_mut()
        .zip(block.chunks_exact(8))
        .take(KECCAK_256_RATE / 8)
    {
        *lane ^= u64::from_le_bytes(bytes.try_into().expect("lane has eight bytes"));
    }
}

pub(super) fn keccak256(input: &[u8]) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(input);
    hash.finalize()
}

pub(super) fn shake256(input: &[u8], output_len: usize) -> Vec<u8> {
    sponge(input, 0x1f, output_len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keccak256_and_shake256_match_independent_standard_vectors() {
        assert_eq!(
            hex::encode(keccak256(b"")),
            "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
        );
        assert_eq!(
            hex::encode(shake256(b"", 64)),
            concat!(
                "46b9dd2b0ba88d13233b3feb743eeb243fcd52ea62b81b82b50c27646ed5762f",
                "d75dc4ddd8c0f200cb05019d67b592f6fc821c49479ab48640292eacb3b7c4be"
            )
        );
        assert_eq!(
            hex::encode(shake256(b"abc", 64)),
            concat!(
                "483366601360a8771c6863080cc4114d8db44530f8f1e1ee4f94ea37e78b5739d",
                "5a15bef186a5386c75744c0527e1faa9f8726e462a12a4feb06bd8801e751e4"
            )
        );
    }

    #[test]
    fn sponge_handles_full_and_partial_rate_blocks() {
        for len in [
            0,
            1,
            KECCAK_256_RATE - 1,
            KECCAK_256_RATE,
            KECCAK_256_RATE + 1,
            2 * KECCAK_256_RATE,
        ] {
            let input = vec![0xa5; len];
            assert_eq!(keccak256(&input), keccak256(&input));
            assert_eq!(shake256(&input, 257).len(), 257);
        }
    }

    #[test]
    fn incremental_keccak_matches_one_shot_across_every_rate_boundary() {
        let input = (0..3 * KECCAK_256_RATE + 17)
            .map(|index| index as u8)
            .collect::<Vec<_>>();
        let expected = keccak256(&input);
        for chunk_size in [
            1,
            7,
            KECCAK_256_RATE - 1,
            KECCAK_256_RATE,
            KECCAK_256_RATE + 1,
            input.len(),
        ] {
            let mut hash = Keccak256::new();
            for chunk in input.chunks(chunk_size) {
                hash.update(chunk);
            }
            assert_eq!(hash.finalize(), expected, "chunk_size={chunk_size}");
        }
    }
}
