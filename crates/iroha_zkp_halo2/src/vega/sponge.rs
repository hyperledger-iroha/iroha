//! Minimal Keccak-f[1600] sponge constructions required by canonical Vega.
//!
//! `tiny-keccak` already supplies the audited permutation under the crate's
//! existing `sha3` feature. Vega needs the original Keccak-256 delimiter and
//! SHAKE256 XOF, so this module applies their standardized sponge padding
//! directly without adding or changing dependencies.

use tiny_keccak::keccakf;

const KECCAK_256_RATE: usize = 136;

fn clear_sensitive_bytes_v1(bytes: &mut [u8]) {
    let bytes = core::hint::black_box(bytes);
    bytes.fill(0);
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    let _ = core::hint::black_box(&mut *bytes);
}

fn clear_sensitive_lanes_v1(lanes: &mut [u64]) {
    let lanes = core::hint::black_box(lanes);
    lanes.fill(0);
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    let _ = core::hint::black_box(&mut *lanes);
}

fn clear_sensitive_usize_v1(value: &mut usize) {
    let value = core::hint::black_box(value);
    *value = 0;
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    let _ = core::hint::black_box(&mut *value);
}

#[cfg(test)]
std::thread_local! {
    static KECCAK_ZEROIZED_DROPS_V1: std::cell::Cell<usize> = const {
        std::cell::Cell::new(0)
    };
    static SHAKE256_READER_ZEROIZED_DROPS_V1: std::cell::Cell<usize> = const {
        std::cell::Cell::new(0)
    };
    static ONE_SHOT_SPONGE_ZEROIZED_DROPS_V1: std::cell::Cell<usize> = const {
        std::cell::Cell::new(0)
    };
}

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

    /// Fork the exact internal absorb state without exposing a general-purpose
    /// `Clone` capability. Both descendants retain independent zeroizing owners
    /// and produce the same digest until different suffixes are absorbed.
    pub(super) fn fork_v1(&self) -> Self {
        Self {
            state: self.state,
            pending: self.pending,
            pending_len: self.pending_len,
        }
    }

    pub(super) fn finalize(mut self) -> [u8; 32] {
        let mut output = [0_u8; 32];
        self.finalize_into(&mut output);
        output
    }

    /// Finalize directly into caller-owned storage.
    ///
    /// The caller retains the owner so secret-derived users can keep the
    /// sponge in a stable allocation from first absorption through explicit
    /// drop. `Drop` optimizer-resistantly erases the state and pending rate
    /// bytes on success, error, or unwind, while this method avoids an
    /// intermediate returned digest array.
    pub(super) fn finalize_into(&mut self, output: &mut [u8; 32]) {
        self.pending[self.pending_len] ^= 0x01;
        self.pending[KECCAK_256_RATE - 1] ^= 0x80;
        xor_rate_block(&mut self.state, &self.pending);
        keccakf(&mut self.state);

        for index in 0..output.len() {
            output[index] = (self.state[index / 8] >> (8 * (index % 8))) as u8;
        }
    }
}

impl Drop for Keccak256 {
    fn drop(&mut self) {
        clear_sensitive_lanes_v1(&mut self.state);
        clear_sensitive_bytes_v1(&mut self.pending);
        clear_sensitive_usize_v1(&mut self.pending_len);

        #[cfg(test)]
        if self.state == [0; 25] && self.pending == [0; KECCAK_256_RATE] && self.pending_len == 0 {
            let _ =
                KECCAK_ZEROIZED_DROPS_V1.try_with(|drops| drops.set(drops.get().saturating_add(1)));
        }
    }
}

/// One-shot permutation state erased on success, error, and unwind.
struct ZeroizingSpongeStateV1([u64; 25]);

impl ZeroizingSpongeStateV1 {
    const fn zeroed() -> Self {
        Self([0; 25])
    }

    fn as_slice(&self) -> &[u64; 25] {
        &self.0
    }

    fn as_mut_array(&mut self) -> &mut [u64; 25] {
        &mut self.0
    }
}

impl Drop for ZeroizingSpongeStateV1 {
    fn drop(&mut self) {
        clear_sensitive_lanes_v1(&mut self.0);

        #[cfg(test)]
        if self.0 == [0; 25] {
            let _ = ONE_SHOT_SPONGE_ZEROIZED_DROPS_V1
                .try_with(|drops| drops.set(drops.get().saturating_add(1)));
        }
    }
}

/// Final padded rate block erased after absorption or during unwind.
struct ZeroizingRateBlockV1([u8; KECCAK_256_RATE]);

impl ZeroizingRateBlockV1 {
    const fn zeroed() -> Self {
        Self([0; KECCAK_256_RATE])
    }

    fn as_slice(&self) -> &[u8; KECCAK_256_RATE] {
        &self.0
    }

    fn as_mut_array(&mut self) -> &mut [u8; KECCAK_256_RATE] {
        &mut self.0
    }
}

impl Drop for ZeroizingRateBlockV1 {
    fn drop(&mut self) {
        clear_sensitive_bytes_v1(&mut self.0);
    }
}

fn absorb_and_finalize_into(input: &[u8], delimiter: u8, state: &mut [u64; 25]) {
    state.fill(0);
    let mut chunks = input.chunks_exact(KECCAK_256_RATE);
    for chunk in &mut chunks {
        xor_rate_block(state, chunk);
        keccakf(state);
    }

    let remainder = chunks.remainder();
    let mut final_block = ZeroizingRateBlockV1::zeroed();
    final_block.as_mut_array()[..remainder.len()].copy_from_slice(remainder);
    final_block.as_mut_array()[remainder.len()] ^= delimiter;
    final_block.as_mut_array()[KECCAK_256_RATE - 1] ^= 0x80;
    xor_rate_block(state, final_block.as_slice());
    keccakf(state);
}

fn sponge(input: &[u8], delimiter: u8, output_len: usize) -> Vec<u8> {
    let mut state = ZeroizingSpongeStateV1::zeroed();
    absorb_and_finalize_into(input, delimiter, state.as_mut_array());

    let mut output = Vec::with_capacity(output_len);
    while output.len() < output_len {
        let take = (output_len - output.len()).min(KECCAK_256_RATE);
        for index in 0..take {
            output.push((state.as_slice()[index / 8] >> (8 * (index % 8))) as u8);
        }
        if output.len() < output_len {
            keccakf(state.as_mut_array());
        }
    }
    output
}

/// Incremental SHAKE256 output reader.
///
/// This keeps only one Keccak rate block in memory, which is required by the
/// release-size deterministic RNS samplers.  Reading the same total number of
/// bytes in any chunking produces the exact one-shot `shake256` stream.
pub(super) struct Shake256Reader {
    state: [u64; 25],
    block: [u8; KECCAK_256_RATE],
    cursor: usize,
}

impl Shake256Reader {
    pub(super) fn new(input: &[u8]) -> Self {
        let mut reader = Self {
            state: [0; 25],
            block: [0; KECCAK_256_RATE],
            cursor: 0,
        };
        absorb_and_finalize_into(input, 0x1f, &mut reader.state);
        reader.materialize_block();
        reader
    }

    pub(super) fn read(&mut self, mut output: &mut [u8]) {
        while !output.is_empty() {
            if self.cursor == KECCAK_256_RATE {
                keccakf(&mut self.state);
                self.materialize_block();
                self.cursor = 0;
            }
            let take = output
                .len()
                .min(KECCAK_256_RATE.saturating_sub(self.cursor));
            output[..take].copy_from_slice(&self.block[self.cursor..self.cursor + take]);
            self.cursor += take;
            output = &mut output[take..];
        }
    }

    fn materialize_block(&mut self) {
        for index in 0..self.block.len() {
            self.block[index] = (self.state[index / 8] >> (8 * (index % 8))) as u8;
        }
    }
}

impl Drop for Shake256Reader {
    fn drop(&mut self) {
        clear_sensitive_lanes_v1(&mut self.state);
        clear_sensitive_bytes_v1(&mut self.block);
        clear_sensitive_usize_v1(&mut self.cursor);

        #[cfg(test)]
        if self.state == [0; 25] && self.block == [0; KECCAK_256_RATE] && self.cursor == 0 {
            let _ = SHAKE256_READER_ZEROIZED_DROPS_V1
                .try_with(|drops| drops.set(drops.get().saturating_add(1)));
        }
    }
}

fn xor_rate_block(state: &mut [u64; 25], block: &[u8]) {
    debug_assert_eq!(block.len(), KECCAK_256_RATE);
    for (lane, bytes) in state
        .iter_mut()
        .zip(block.chunks_exact(8))
        .take(KECCAK_256_RATE / 8)
    {
        *lane ^= u64::from(bytes[0])
            | (u64::from(bytes[1]) << 8)
            | (u64::from(bytes[2]) << 16)
            | (u64::from(bytes[3]) << 24)
            | (u64::from(bytes[4]) << 32)
            | (u64::from(bytes[5]) << 40)
            | (u64::from(bytes[6]) << 48)
            | (u64::from(bytes[7]) << 56);
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

    fn production_source_v1() -> &'static str {
        include_str!("sponge.rs")
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .expect("production sponge source")
    }

    fn reset_keccak_zeroized_drops_v1() {
        let _ = KECCAK_ZEROIZED_DROPS_V1.try_with(|drops| drops.set(0));
    }

    fn keccak_zeroized_drops_v1() -> usize {
        KECCAK_ZEROIZED_DROPS_V1
            .try_with(std::cell::Cell::get)
            .unwrap_or(usize::MAX)
    }

    fn reset_shake256_reader_zeroized_drops_v1() {
        let _ = SHAKE256_READER_ZEROIZED_DROPS_V1.try_with(|drops| drops.set(0));
    }

    fn shake256_reader_zeroized_drops_v1() -> usize {
        SHAKE256_READER_ZEROIZED_DROPS_V1
            .try_with(std::cell::Cell::get)
            .unwrap_or(usize::MAX)
    }

    fn reset_one_shot_sponge_zeroized_drops_v1() {
        let _ = ONE_SHOT_SPONGE_ZEROIZED_DROPS_V1.try_with(|drops| drops.set(0));
    }

    fn one_shot_sponge_zeroized_drops_v1() -> usize {
        ONE_SHOT_SPONGE_ZEROIZED_DROPS_V1
            .try_with(std::cell::Cell::get)
            .unwrap_or(usize::MAX)
    }

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
    fn one_shot_sponge_state_zeroizes_on_success_error_and_unwind() {
        reset_one_shot_sponge_zeroized_drops_v1();
        assert_eq!(
            shake256(b"one-shot secret sampler input", 73),
            sponge(b"one-shot secret sampler input", 0x1f, 73)
        );
        assert_eq!(one_shot_sponge_zeroized_drops_v1(), 2);

        reset_one_shot_sponge_zeroized_drops_v1();
        let failed = (|| -> Result<(), ()> {
            let mut state = ZeroizingSpongeStateV1::zeroed();
            absorb_and_finalize_into(b"abandoned one-shot state", 0x1f, state.as_mut_array());
            Err(())
        })();
        assert_eq!(failed, Err(()));
        assert_eq!(one_shot_sponge_zeroized_drops_v1(), 1);

        reset_one_shot_sponge_zeroized_drops_v1();
        let unwind = std::panic::catch_unwind(|| {
            let mut state = ZeroizingSpongeStateV1::zeroed();
            absorb_and_finalize_into(b"panicking one-shot state", 0x1f, state.as_mut_array());
            panic!("exercise one-shot sponge zeroization during unwind");
        });
        assert!(unwind.is_err());
        assert_eq!(one_shot_sponge_zeroized_drops_v1(), 1);

        let production = production_source_v1();
        let one_shot = production
            .split("fn sponge(")
            .nth(1)
            .expect("one-shot sponge")
            .split("/// Incremental SHAKE256 output reader")
            .next()
            .expect("one-shot sponge boundary");
        assert!(one_shot.contains("let mut state = ZeroizingSpongeStateV1::zeroed()"));
        assert!(one_shot.contains("absorb_and_finalize_into("));
        assert!(!one_shot.contains("let mut state = absorb_and_finalize("));
        assert!(!one_shot.contains("to_le_bytes"));
        let state_drop = production
            .split("impl Drop for ZeroizingSpongeStateV1")
            .nth(1)
            .expect("one-shot state drop implementation");
        assert!(state_drop.contains("clear_sensitive_lanes_v1(&mut self.0)"));
    }

    #[test]
    fn keccak_fork_preserves_exact_prefix_state_and_independent_suffixes() {
        assert_eq!(core::mem::size_of::<Keccak256>(), 344);
        for prefix_len in [
            0,
            1,
            KECCAK_256_RATE - 1,
            KECCAK_256_RATE,
            KECCAK_256_RATE + 19,
        ] {
            let prefix = vec![0x5a; prefix_len];
            let mut original = Keccak256::new();
            original.update(&prefix);
            let mut fork = original.fork_v1();
            original.update(b"first suffix");
            fork.update(b"second suffix");

            let mut expected_first = prefix.clone();
            expected_first.extend_from_slice(b"first suffix");
            let mut expected_second = prefix;
            expected_second.extend_from_slice(b"second suffix");
            assert_eq!(original.finalize(), keccak256(&expected_first));
            assert_eq!(fork.finalize(), keccak256(&expected_second));
        }
    }

    #[test]
    fn streaming_shake_matches_one_shot_under_adversarial_chunkings() {
        let input = (0..2 * KECCAK_256_RATE + 19)
            .map(|index| index as u8)
            .collect::<Vec<_>>();
        let expected = shake256(&input, 3 * KECCAK_256_RATE + 23);
        for chunk_size in [1, 7, 8, 31, KECCAK_256_RATE - 1, KECCAK_256_RATE, 509] {
            let mut reader = Shake256Reader::new(&input);
            let mut actual = vec![0_u8; expected.len()];
            for chunk in actual.chunks_mut(chunk_size) {
                reader.read(chunk);
            }
            assert_eq!(actual, expected, "chunk_size={chunk_size}");
        }
    }

    #[test]
    fn streaming_shake_reader_zeroizes_on_success_error_and_unwind() {
        assert_eq!(core::mem::size_of::<Shake256Reader>(), 344);

        reset_shake256_reader_zeroized_drops_v1();
        {
            let mut reader = Shake256Reader::new(b"secret common-a sampler prefix");
            reader.read(&mut [0_u8; 31]);
        }
        assert_eq!(shake256_reader_zeroized_drops_v1(), 1);

        reset_shake256_reader_zeroized_drops_v1();
        let failed = (|| -> Result<(), ()> {
            let mut reader = Shake256Reader::new(b"secret abandoned sampler prefix");
            reader.read(&mut [0_u8; 17]);
            Err(())
        })();
        assert_eq!(failed, Err(()));
        assert_eq!(shake256_reader_zeroized_drops_v1(), 1);

        reset_shake256_reader_zeroized_drops_v1();
        let unwind = std::panic::catch_unwind(|| {
            let mut reader = Shake256Reader::new(b"secret panicking sampler prefix");
            reader.read(&mut [0_u8; 9]);
            panic!("exercise streaming SHAKE zeroization during unwind");
        });
        assert!(unwind.is_err());
        assert_eq!(shake256_reader_zeroized_drops_v1(), 1);

        let production = production_source_v1();
        let reader_impl = production
            .split("impl Shake256Reader")
            .nth(1)
            .expect("streaming SHAKE implementation")
            .split("impl Drop for Shake256Reader")
            .next()
            .expect("streaming SHAKE implementation boundary");
        assert!(!reader_impl.contains("to_le_bytes"));
        let drop_impl = production
            .split("impl Drop for Shake256Reader")
            .nth(1)
            .expect("streaming SHAKE drop implementation");
        assert!(drop_impl.contains("clear_sensitive_lanes_v1(&mut self.state)"));
        assert!(drop_impl.contains("clear_sensitive_bytes_v1(&mut self.block)"));
        assert!(drop_impl.contains("clear_sensitive_usize_v1(&mut self.cursor)"));
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

    #[test]
    fn finalize_into_matches_finalize_and_zeroizes_on_success_and_unwind() {
        let input = b"secret-derived nonce material crossing a rate boundary";
        let expected = keccak256(input);

        reset_keccak_zeroized_drops_v1();
        let mut hash = Keccak256::new();
        hash.update(input);
        let mut output = [0_u8; 32];
        hash.finalize_into(&mut output);
        assert_eq!(output, expected);
        assert_eq!(keccak_zeroized_drops_v1(), 0);
        drop(hash);
        assert_eq!(keccak_zeroized_drops_v1(), 1);

        reset_keccak_zeroized_drops_v1();
        {
            let mut abandoned = Keccak256::new();
            abandoned.update(input);
        }
        assert_eq!(keccak_zeroized_drops_v1(), 1);

        reset_keccak_zeroized_drops_v1();
        let unwind = std::panic::catch_unwind(|| {
            let mut hash = Keccak256::new();
            hash.update(input);
            panic!("exercise Keccak zeroization during unwind");
        });
        assert!(unwind.is_err());
        assert_eq!(keccak_zeroized_drops_v1(), 1);

        let source = production_source_v1();
        assert!(source.contains("pub(super) fn finalize_into(&mut self"));
        assert!(source.contains("impl Drop for Keccak256"));
        assert!(source.contains("clear_sensitive_lanes_v1(&mut self.state)"));
        assert!(source.contains("clear_sensitive_bytes_v1(&mut self.pending)"));
        assert!(source.contains("clear_sensitive_usize_v1(&mut self.pending_len)"));
        let finalize_into = source
            .split("pub(super) fn finalize_into")
            .nth(1)
            .expect("direct finalizer")
            .split("impl Drop for Keccak256")
            .next()
            .expect("finalizer source slice");
        assert!(finalize_into.contains("self.state[index / 8] >> (8 * (index % 8))"));
        assert!(!finalize_into.contains(".copied()"));
        assert!(!finalize_into.contains("to_le_bytes"));
        let xor_rate = source
            .split("fn xor_rate_block")
            .nth(1)
            .expect("rate-block decoder")
            .split("pub(super) fn keccak256")
            .next()
            .expect("rate-block source slice");
        assert!(xor_rate.contains("u64::from(bytes[7]) << 56"));
        assert!(!xor_rate.contains("from_le_bytes"));
        assert!(!xor_rate.contains("try_into"));
    }
}
