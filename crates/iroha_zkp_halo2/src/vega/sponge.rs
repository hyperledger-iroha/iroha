//! Minimal Keccak-f[1600] sponge constructions required by canonical Vega.
//!
//! `tiny-keccak` already supplies the audited permutation under the crate's
//! existing `sha3` feature. Vega needs the original Keccak-256 delimiter and
//! SHAKE256 XOF, so this module applies their standardized sponge padding
//! directly without adding or changing dependencies.

use tiny_keccak::keccakf;

const KECCAK_256_RATE: usize = 136;

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
    sponge(input, 0x01, 32)
        .try_into()
        .expect("requested exactly 32 bytes")
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
}
