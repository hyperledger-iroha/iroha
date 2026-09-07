//! SHAKE256 expansion into caller-owned buffers.
//!
//! Callers supply their own unambiguous protocol framing and output limits.
//! Splitting the same input bytes into several parts does not domain-separate
//! them, and different output lengths share the SHAKE prefix unless the caller
//! includes the requested length in its input frame.

use sha3::{
    Shake256,
    digest::{ExtendableOutput as _, Update as _, XofReader as _},
};

/// Expand the concatenation of `parts` with SHAKE256 into `output`.
///
/// This helper allocates no output buffer. Protocol callers must frame fields,
/// bind any required output length, and enforce their own resource limits.
pub fn shake256_into(parts: &[&[u8]], output: &mut [u8]) {
    let mut shake = Shake256::default();
    for part in parts {
        shake.update(part);
    }
    shake.finalize_xof().read(output);
}

/// Reusable, unfinished SHAKE256 state after absorbing a fixed input prefix.
///
/// Expansion clones the complete state, including a partially filled rate
/// block, appends the supplied suffix, and finalizes once. Its output is exactly
/// SHAKE256 of the concatenated prefix and suffix bytes. This is not a digest
/// of the prefix or an additional hash layer. Callers must still provide their
/// own unambiguous framing and bind output lengths when their protocol needs it.
#[derive(Clone)]
pub struct Shake256Prefix {
    state: Shake256,
}

impl std::fmt::Debug for Shake256Prefix {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Shake256Prefix")
            .finish_non_exhaustive()
    }
}

impl Shake256Prefix {
    /// Absorb the concatenation of `parts` without finalizing or squeezing.
    pub fn new(parts: &[&[u8]]) -> Self {
        let mut state = Shake256::default();
        for part in parts {
            state.update(part);
        }
        Self { state }
    }

    /// Append `suffix` to a private state clone and fill the caller's output.
    ///
    /// The stored prefix remains unchanged, so expansions are independent and
    /// can be shared across threads. No output buffer is allocated here.
    pub fn expand_into(&self, suffix: &[&[u8]], output: &mut [u8]) {
        let mut state = self.state.clone();
        for part in suffix {
            state.update(part);
        }
        state.finalize_xof().read(output);
    }
}

#[cfg(test)]
mod tests {
    use super::{Shake256Prefix, shake256_into};

    #[test]
    fn cached_prefix_matches_cold_hash_across_partial_rate_blocks() {
        let input: Vec<u8> = (0..=255).cycle().take(820).collect();
        for prefix_len in [0, 1, 7, 134, 135, 136, 137, 271, 272, 273, 409] {
            let midpoint = prefix_len / 2;
            let cached = Shake256Prefix::new(&[&input[..midpoint], &input[midpoint..prefix_len]]);
            for suffix_len in [0, 1, 135, 136, 137, 410] {
                let suffix = &input[prefix_len..prefix_len + suffix_len];
                let split = suffix_len / 2;
                for length in [0, 1, 135, 136, 137, 271, 272, 273, 409] {
                    let mut expected = vec![0; length];
                    shake256_into(&[&input[..prefix_len], suffix], &mut expected);
                    let mut guarded = vec![0xa5; length + 14];
                    cached.expand_into(
                        &[&suffix[..split], &[], &suffix[split..]],
                        &mut guarded[7..7 + length],
                    );
                    assert_eq!(
                        &guarded[7..7 + length],
                        expected,
                        "prefix={prefix_len}, suffix={suffix_len}, output={length}"
                    );
                    assert_eq!(&guarded[..7], &[0xa5; 7]);
                    assert_eq!(&guarded[7 + length..], &[0xa5; 7]);
                }
            }
        }
    }

    #[test]
    fn cached_prefix_reuse_and_clone_have_no_cross_call_state() {
        let prefix = Shake256Prefix::new(&[b"a"]);
        let mut first = [0; 140];
        prefix.expand_into(&[b"bc"], &mut first);
        let mut intervening = [0; 273];
        prefix.expand_into(&[b"different suffix"], &mut intervening);
        let mut again = [0; 140];
        prefix.clone().expand_into(&[b"b", b"c"], &mut again);
        assert_eq!(first, again);
        let mut cold = [0; 140];
        shake256_into(&[b"abc"], &mut cold);
        assert_eq!(first, cold);
        assert_ne!(first.as_slice(), &intervening[..140]);
    }

    #[test]
    fn prefix_debug_does_not_expose_absorbed_input_or_state() {
        let prefix = Shake256Prefix::new(&[b"private input"]);
        assert_eq!(format!("{prefix:?}"), "Shake256Prefix { .. }");
    }

    #[test]
    fn empty_message_known_answer_matches_fips_shake256() {
        let mut actual = [0_u8; 64];
        shake256_into(&[], &mut actual);
        assert_eq!(
            actual.as_slice(),
            hex::decode(concat!(
                "46b9dd2b0ba88d13233b3feb743eeb24",
                "3fcd52ea62b81b82b50c27646ed5762f",
                "d75dc4ddd8c0f200cb05019d67b592f6",
                "fc821c49479ab48640292eacb3b7c4be"
            ))
            .unwrap()
        );
    }

    #[test]
    fn partitioned_input_and_output_block_boundaries_preserve_exact_bytes() {
        let input: Vec<u8> = (0..=255).cycle().take(401).collect();
        let mut one = [0_u8; 409];
        let mut split = [0_u8; 409];
        shake256_into(&[&input], &mut one);
        shake256_into(
            &[
                &input[..135],
                &[],
                &input[135..136],
                &input[136..272],
                &input[272..],
            ],
            &mut split,
        );
        assert_eq!(one, split);
        for length in [0, 1, 135, 136, 137, 271, 272, 273, 409] {
            let mut output = vec![0x5a; length];
            shake256_into(&[&input], &mut output);
            assert_eq!(output, one[..length]);
        }
    }

    #[test]
    fn independent_block_crossing_known_answer_and_subslice_boundaries() {
        let mut guarded = [0xa5_u8; 154];
        shake256_into(&[b"abc"], &mut guarded[7..147]);
        assert_eq!(&guarded[..7], &[0xa5; 7]);
        assert_eq!(&guarded[147..], &[0xa5; 7]);
        assert_eq!(
            &guarded[7..147],
            hex::decode(
                "483366601360a8771c6863080cc4114d8db44530f8f1e1ee4f94ea37e78b5739d5a15bef186a5386c75744c0527e1faa9f8726e462a12a4feb06bd8801e751e41385141204f329979fd3047a13c5657724ada64d2470157b3cdc288620944d78dbcddbd912993f0913f164fb2ce95131a2d09a3e6d51cbfc622720d7a75c6334e8a2d7ec71a7cc29cf0ea610"
            )
            .unwrap()
        );
    }
}
