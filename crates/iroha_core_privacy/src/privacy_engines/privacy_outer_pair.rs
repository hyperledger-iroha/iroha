//! Paired SHA3-384 owner for initial row commitments.
//!
//! Two rows framed after one cached public prefix share every Keccak-f\[1600\]
//! permutation through `fastpq_prover::keccak_permute_pair_v1`: the two-state
//! kernel where the CPU reports `FEAT_SHA3`, and the portable permutation of each
//! lane on every other CPU and build. Framing, SHA3 padding and the 48-byte
//! squeeze are byte-identical to the scalar owner on every backend.

use super::{
    PRIVACY_OUTER_DIGEST_BYTES_V1, PrivacyOuterDigestV1, PrivacyOuterDomainPrefixV1,
    PrivacyOuterLastFieldStreamErrorV1, PrivacyOuterLastFieldStreamV1, Sha3_384,
};
use sha3::digest::common::hazmat::{SerializableState, SerializedState};
use zeroize::Zeroize;

/// SHA3-384 sponge rate.
const RATE_BYTES: usize = 104;
/// Serialized SHA3 state: 25 little-endian lanes, then the absorbed rate position.
const SERIALIZED_BYTES: usize = 201;
/// SHA3 domain suffix with the first padding bit, XORed at the final position.
const SHA3_PAD_FIRST_V1: u64 = 0x06;
/// Closing padding bit, XORed into the last rate byte.
const SHA3_PAD_LAST_V1: u64 = 0x80;
pub(crate) type PairErrorV1 = (usize, PrivacyOuterLastFieldStreamErrorV1);

/// Clearing public serialized-state format, without a hybrid-array feature assumption.
struct SerializedSpongeV1(SerializedState<Sha3_384>);
impl Drop for SerializedSpongeV1 {
    fn drop(&mut self) {
        self.0.as_mut_slice().zeroize();
        #[cfg(test)]
        tests::observe_clear_v1(self.0.iter().all(|&byte| byte == 0));
    }
}

/// Cached public frame prefix in Keccak lanes, read once per commitment.
///
/// Every pair of the commitment starts from these lanes, so no pair clones,
/// serializes or reconstructs a scalar SHA3 owner.
pub(crate) struct PrivacyOuterPairPrefixV1 {
    lanes: [u64; 25],
    position: usize,
    prefix_bytes: usize,
}

/// Two identically framed row streams with no caller-supplied sponge state.
pub(crate) struct PrivacyOuterLastFieldPairV1 {
    state: [[u64; 25]; 2],
    position: usize,
    remaining: [usize; 2],
}

impl PrivacyOuterDomainPrefixV1 {
    /// Read the cached public prefix into Keccak lanes for every pair of one commitment.
    pub(crate) fn pair_prefix_v1(
        &self,
    ) -> Result<PrivacyOuterPairPrefixV1, PrivacyOuterLastFieldStreamErrorV1> {
        let invalid = PrivacyOuterLastFieldStreamErrorV1::FramingLimitExceeded;
        // Install clearing custody before the first lane is copied.
        let mut prefix = PrivacyOuterPairPrefixV1 {
            lanes: [0; 25],
            position: 0,
            prefix_bytes: self.prefix_bytes,
        };
        let encoded = SerializedSpongeV1(self.state.inner.serialize());
        let Some((&position, lanes)) = encoded.0.split_last() else {
            return Err(invalid);
        };
        let position = usize::from(position);
        if encoded.0.len() != SERIALIZED_BYTES || position >= RATE_BYTES {
            return Err(invalid);
        }
        for (word, bytes) in prefix.lanes.iter_mut().zip(lanes.as_chunks::<8>().0) {
            *word = u64::from_le_bytes(*bytes);
        }
        prefix.position = position;
        Ok(prefix)
    }

    /// Frame two rows with their original public row coordinates.
    ///
    /// Owners that build many pairs read [`Self::pair_prefix_v1`] once instead.
    pub(crate) fn last_field_pair_at_with_counter_v1(
        &self,
        indices: [u64; 2],
        counter: u64,
        prefix_fields: &[&[u8]],
        final_field_len: usize,
    ) -> Result<PrivacyOuterLastFieldPairV1, PairErrorV1> {
        self.pair_prefix_v1()
            .map_err(|error| (0, error))?
            .last_field_pair_at_with_counter_v1(indices, counter, prefix_fields, final_field_len)
    }
}

impl PrivacyOuterPairPrefixV1 {
    /// Frame two rows exactly as two scalar final-field streams; only the indices differ.
    pub(crate) fn last_field_pair_at_with_counter_v1(
        &self,
        indices: [u64; 2],
        counter: u64,
        prefix_fields: &[&[u8]],
        final_field_len: usize,
    ) -> Result<PrivacyOuterLastFieldPairV1, PairErrorV1> {
        // The scalar owner's checked framing arithmetic, before any lane is written.
        let invalid = (0, PrivacyOuterLastFieldStreamErrorV1::FramingLimitExceeded);
        let count = prefix_fields.len().checked_add(1).ok_or(invalid)?;
        let count = u32::try_from(count).map_err(|_| invalid)?;
        let final_length = u64::try_from(final_field_len).map_err(|_| invalid)?;
        let mut bytes = self.prefix_bytes.checked_add(4).ok_or(invalid)?;
        for field in prefix_fields {
            u64::try_from(field.len()).map_err(|_| invalid)?;
            bytes = bytes
                .checked_add(8)
                .and_then(|n| n.checked_add(field.len()))
                .ok_or(invalid)?;
        }
        bytes
            .checked_add(8)
            .and_then(|n| n.checked_add(final_field_len))
            .ok_or(invalid)?;
        // Install clearing custody before the prefix lanes are copied.
        let mut owner = PrivacyOuterLastFieldPairV1 {
            state: [[0; 25]; 2],
            position: 0,
            remaining: [0; 2],
        };
        for state in &mut owner.state {
            state.copy_from_slice(&self.lanes);
        }
        owner.position = self.position;
        owner.absorb_v1([&indices[0].to_be_bytes(), &indices[1].to_be_bytes()]);
        owner.absorb_both_v1(&counter.to_be_bytes());
        owner.absorb_both_v1(&count.to_be_bytes());
        for field in prefix_fields {
            owner.absorb_both_v1(&(field.len() as u64).to_be_bytes());
            owner.absorb_both_v1(field);
        }
        owner.absorb_both_v1(&final_length.to_be_bytes());
        owner.remaining = [final_field_len; 2];
        Ok(owner)
    }
}

impl PrivacyOuterLastFieldPairV1 {
    /// Absorb equal-length lane inputs, permuting both lanes together at each full rate.
    fn absorb_v1(&mut self, input: [&[u8]; 2]) {
        debug_assert_eq!(input[0].len(), input[1].len());
        let mut consumed = 0;
        while consumed < input[0].len() {
            let count = (RATE_BYTES - self.position).min(input[0].len() - consumed);
            for (state, bytes) in self.state.iter_mut().zip(input) {
                absorb_bytes_v1(state, self.position, &bytes[consumed..consumed + count]);
            }
            self.position += count;
            consumed += count;
            if self.position == RATE_BYTES {
                fastpq_prover::keccak_permute_pair_v1(&mut self.state);
                self.position = 0;
            }
        }
    }

    /// Absorb one public framing field into both lanes.
    fn absorb_both_v1(&mut self, bytes: &[u8]) {
        self.absorb_v1([bytes, bytes]);
    }

    /// Check both row refusals before mutating either original row state.
    pub(crate) fn update_v1(&mut self, input: [&[u8]; 2]) -> Result<(), PairErrorV1> {
        for lane in 0..2 {
            if self.remaining[lane] == usize::MAX {
                return Err((
                    lane,
                    PrivacyOuterLastFieldStreamErrorV1::FramingLimitExceeded,
                ));
            }
            if input[lane].len() > self.remaining[lane] {
                return Err((
                    lane,
                    PrivacyOuterLastFieldStreamErrorV1::InputOverrun {
                        remaining: self.remaining[lane],
                        supplied: input[lane].len(),
                    },
                ));
            }
        }
        if input[0].len() != input[1].len() {
            return Err((1, PrivacyOuterLastFieldStreamErrorV1::FramingLimitExceeded));
        }
        self.absorb_v1(input);
        for lane in 0..2 {
            self.remaining[lane] -= input[lane].len();
        }
        Ok(())
    }

    /// Pad, permute and squeeze both lanes into the caller's tile after both lane checks.
    ///
    /// SHA3 padding closes the final block, one paired permutation follows, and each
    /// 48-byte digest is the first six little-endian lanes; the state is then wiped
    /// and the owner permanently sealed.
    pub(crate) fn finalize_into_v1(
        &mut self,
        output: &mut [PrivacyOuterDigestV1; 2],
    ) -> Result<(), PairErrorV1> {
        for lane in 0..2 {
            if self.remaining[lane] != 0 {
                return Err((
                    lane,
                    PrivacyOuterLastFieldStreamErrorV1::InputUnderrun {
                        remaining: self.remaining[lane],
                    },
                ));
            }
        }
        if self.position >= RATE_BYTES {
            return Err((0, PrivacyOuterLastFieldStreamErrorV1::FramingLimitExceeded));
        }
        let position = self.position;
        for state in &mut self.state {
            state[position / 8] ^= SHA3_PAD_FIRST_V1 << (8 * (position % 8));
            state[(RATE_BYTES - 1) / 8] ^= SHA3_PAD_LAST_V1 << (8 * ((RATE_BYTES - 1) % 8));
        }
        fastpq_prover::keccak_permute_pair_v1(&mut self.state);
        for (state, target) in self.state.iter().zip(output.iter_mut()) {
            let mut digest = [0_u8; PRIVACY_OUTER_DIGEST_BYTES_V1];
            for (bytes, word) in digest.as_chunks_mut::<8>().0.iter_mut().zip(state) {
                *bytes = word.to_le_bytes();
            }
            *target = PrivacyOuterDigestV1::from_bytes(digest);
        }
        self.state.zeroize();
        self.position.zeroize();
        self.remaining = [usize::MAX; 2];
        Ok(())
    }
}

impl Drop for PrivacyOuterPairPrefixV1 {
    fn drop(&mut self) {
        self.lanes.zeroize();
        self.position.zeroize();
        #[cfg(test)]
        tests::observe_clear_v1(self.lanes.iter().all(|&word| word == 0) && self.position == 0);
    }
}

impl Drop for PrivacyOuterLastFieldPairV1 {
    fn drop(&mut self) {
        self.state.zeroize();
        self.position.zeroize();
        self.remaining.zeroize();
        #[cfg(test)]
        tests::observe_clear_v1(
            self.state.iter().flatten().all(|&word| word == 0)
                && self.position == 0
                && self.remaining == [0; 2],
        );
    }
}

/// Explicit byte encoding; all callers supply one bounded portion of the rate.
fn absorb_bytes_v1(state: &mut [u64; 25], mut position: usize, mut bytes: &[u8]) {
    while position % 8 != 0 && !bytes.is_empty() {
        state[position / 8] ^= u64::from(bytes[0]) << (8 * (position % 8));
        position += 1;
        bytes = &bytes[1..];
    }
    let (words, tail) = bytes.as_chunks::<8>();
    for word in words {
        state[position / 8] ^= u64::from_le_bytes(*word);
        position += 8;
    }
    for &byte in tail {
        state[position / 8] ^= u64::from(byte) << (8 * (position % 8));
        position += 1;
    }
}

/// Fixed per-pair construction and finalization workspace charged by row-state owners.
///
/// Construction from a domain prefix holds one pair prefix and its serialized scratch
/// next to the pair, and finalization works in place; the charge below covers that
/// peak with margin.
pub(crate) const fn paired_stream_workspace_bytes_v1() -> usize {
    2 * size_of::<PrivacyOuterLastFieldStreamV1>()
        + size_of::<PrivacyOuterLastFieldPairV1>()
        + 2 * size_of::<SerializedSpongeV1>()
        + 256
}
const _: () = assert!(
    size_of::<PrivacyOuterPairPrefixV1>()
        + size_of::<SerializedSpongeV1>()
        + size_of::<PrivacyOuterLastFieldPairV1>()
        <= paired_stream_workspace_bytes_v1()
);
#[cfg(test)]
impl PrivacyOuterLastFieldPairV1 {
    pub(crate) fn corrupt_remaining_v1(&mut self, lane: usize, bytes: usize) {
        self.remaining[lane] = self.remaining[lane].checked_sub(bytes).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::super::{PrivacyOuterDomainV1, PrivacyOuterProofFamilyV1, PrivacyOuterProofScopeV1};
    use super::*;
    use sha3::Digest as _;
    use zeroize::Zeroizing;

    thread_local! { static CLEARS: std::cell::RefCell<Option<Vec<bool>>> = const { std::cell::RefCell::new(None) }; }
    pub(super) fn observe_clear_v1(clean: bool) {
        CLEARS.with_borrow_mut(|slot| {
            if let Some(values) = slot {
                values.push(clean);
            }
        });
    }
    fn prefix_v1(instance: bool) -> PrivacyOuterDomainPrefixV1 {
        PrivacyOuterDomainPrefixV1::new(PrivacyOuterDomainV1 {
            catalog: &[17; 32],
            protocol: b"pair-test-protocol",
            profile: b"pair-test-profile",
            proof_scope: instance.then_some(PrivacyOuterProofScopeV1 {
                nonce: [29; 32],
                family: PrivacyOuterProofFamilyV1::Main,
            }),
            role: b"pair-test-role",
            phase: b"vector-row-leaf",
            level: 0,
            index: 0,
            counter: 0,
        })
        .unwrap()
    }
    /// The complete frame of `prefix_v1`, encoded independently of every production writer.
    fn frame_bytes_v1(instance: bool, index: u64, counter: u64, fields: &[&[u8]]) -> Vec<u8> {
        let mut profile = b"pair-test-profile".to_vec();
        if instance {
            profile.extend_from_slice(b"\0X5I1");
            profile.push(PrivacyOuterProofFamilyV1::Main as u8);
            profile.extend_from_slice(&[29; 32]);
        }
        let mut frame = b"iroha:privacy:stark:sha3-384:frame:v1\0".to_vec();
        frame.extend_from_slice(&[17; 32]);
        for label in [
            b"pair-test-protocol".as_slice(),
            &profile,
            b"pair-test-role",
            b"vector-row-leaf",
        ] {
            frame.extend_from_slice(&u16::try_from(label.len()).unwrap().to_be_bytes());
            frame.extend_from_slice(label);
        }
        for coordinate in [0, index, counter] {
            frame.extend_from_slice(&coordinate.to_be_bytes());
        }
        frame.extend_from_slice(&u32::try_from(fields.len()).unwrap().to_be_bytes());
        for field in fields {
            frame.extend_from_slice(&u64::try_from(field.len()).unwrap().to_be_bytes());
            frame.extend_from_slice(field);
        }
        frame
    }

    #[test]
    fn paired_sponge_matches_sha3_384_frames_around_block_boundaries() {
        for instance in [false, true] {
            let prefix = prefix_v1(instance);
            let pair_prefix = prefix.pair_prefix_v1().unwrap();
            // The public prefix field moves the final field's first byte through every
            // rate offset; the final lengths straddle word and block boundaries.
            for extra in 0..RATE_BYTES {
                let field = vec![0xa5_u8; extra];
                for bytes in [
                    0, 1, 7, 8, 9, 55, 56, 57, 103, 104, 105, 207, 208, 209, 311, 312, 313, 1000,
                ] {
                    let indices = [2 * extra as u64, 2 * extra as u64 + 1];
                    let payloads = [0_usize, 1].map(|lane| {
                        (0..bytes)
                            .map(|index| (index * 13 + lane * 101 + extra) as u8)
                            .collect::<Vec<_>>()
                    });
                    let expected = [0, 1].map(|lane| {
                        PrivacyOuterDigestV1::from_bytes(
                            Sha3_384::digest(frame_bytes_v1(
                                instance,
                                indices[lane],
                                65_535,
                                &[&field, &payloads[lane]],
                            ))
                            .into(),
                        )
                    });
                    for owner in [
                        pair_prefix.last_field_pair_at_with_counter_v1(
                            indices,
                            65_535,
                            &[&field],
                            bytes,
                        ),
                        prefix.last_field_pair_at_with_counter_v1(
                            indices,
                            65_535,
                            &[&field],
                            bytes,
                        ),
                    ] {
                        let mut owner = owner.unwrap();
                        owner
                            .update_v1([payloads[0].as_slice(), payloads[1].as_slice()])
                            .unwrap();
                        let mut output = [PrivacyOuterDigestV1::default(); 2];
                        owner.finalize_into_v1(&mut output).unwrap();
                        assert_eq!(output, expected, "extra={extra} bytes={bytes}");
                    }
                }
            }
        }
    }

    #[test]
    fn paired_stream_complete_original_framing_rate_split_and_terminal_parity() {
        for instance in [false, true] {
            let prefix = prefix_v1(instance);
            for extra in 0..RATE_BYTES {
                let field = Zeroizing::new(vec![37_u8; extra]);
                for bytes in [0, 1, 7, 8, 63, 64, 103, 104, 105, 207, 208, 209, 1024] {
                    for chunk in [1, 7, 64, 137] {
                        let mut paired = prefix
                            .last_field_pair_at_with_counter_v1([11, 29], 65535, &[&field], bytes)
                            .unwrap();
                        let mut scalar = [11, 29].map(|row| {
                            prefix
                                .last_field_stream_at_with_counter(row, 65535, &[&field], bytes)
                                .unwrap()
                        });
                        let input = [0, 1].map(|lane| {
                            Zeroizing::new(
                                (0..bytes)
                                    .map(|index| (index * 31 + lane * 47) as u8)
                                    .collect::<Vec<_>>(),
                            )
                        });
                        for first in (0..bytes).step_by(chunk) {
                            let end = bytes.min(first + chunk);
                            let slices = [&input[0][first..end], &input[1][first..end]];
                            for (stream, data) in scalar.iter_mut().zip(slices) {
                                stream.update(data).unwrap();
                            }
                            paired.update_v1(slices).unwrap();
                        }
                        let mut output = [PrivacyOuterDigestV1::default(); 2];
                        paired.finalize_into_v1(&mut output).unwrap();
                        assert_eq!(
                            output,
                            scalar.each_mut().map(|s| s.finalize_in_place_v1().unwrap())
                        );
                        assert!(paired.state.iter().flatten().all(|&v| v == 0));
                        assert_eq!(paired.position, 0);
                        assert_eq!(
                            paired.update_v1([&[], &[]]),
                            Err((0, PrivacyOuterLastFieldStreamErrorV1::FramingLimitExceeded))
                        );
                        assert_eq!(
                            paired.finalize_into_v1(&mut output),
                            Err((
                                0,
                                PrivacyOuterLastFieldStreamErrorV1::InputUnderrun {
                                    remaining: usize::MAX
                                }
                            ))
                        );
                        for value in &mut output {
                            value.zeroize_v1();
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn paired_stream_refusal_precedence_and_private_custody_on_error_and_unwind() {
        for unwind in [false, true] {
            CLEARS.set(Some(Vec::new()));
            let result = std::panic::catch_unwind(|| {
                let prefix = prefix_v1(true);
                let mut paired = prefix
                    .last_field_pair_at_with_counter_v1([0, 1], 7, &[], 8)
                    .unwrap();
                let before = Zeroizing::new(paired.state);
                let mut output = [PrivacyOuterDigestV1::from_bytes([23; 48]); 2];
                for (input, lane) in [
                    ([&[1_u8; 9][..], &[2_u8; 9][..]], 0),
                    ([&[1_u8; 8][..], &[2_u8; 9][..]], 1),
                ] {
                    assert_eq!(
                        paired.update_v1(input),
                        Err((
                            lane,
                            PrivacyOuterLastFieldStreamErrorV1::InputOverrun {
                                remaining: 8,
                                supplied: 9
                            }
                        ))
                    );
                    assert_eq!(paired.state, *before);
                    assert_eq!(paired.remaining, [8; 2]);
                }
                assert_eq!(
                    paired.update_v1([&[1; 7], &[2; 8]]),
                    Err((1, PrivacyOuterLastFieldStreamErrorV1::FramingLimitExceeded))
                );
                assert_eq!(paired.state, *before);
                assert_eq!(
                    paired.finalize_into_v1(&mut output),
                    Err((
                        0,
                        PrivacyOuterLastFieldStreamErrorV1::InputUnderrun { remaining: 8 }
                    ))
                );
                assert_eq!(output, [PrivacyOuterDigestV1::from_bytes([23; 48]); 2]);
                paired.update_v1([&[1; 8], &[2; 8]]).unwrap();
                paired.position = RATE_BYTES;
                assert_eq!(
                    paired.finalize_into_v1(&mut output),
                    Err((0, PrivacyOuterLastFieldStreamErrorV1::FramingLimitExceeded))
                );
                assert_eq!(output, [PrivacyOuterDigestV1::from_bytes([23; 48]); 2]);
                assert!(!unwind, "injected paired stream unwind");
            });
            assert_eq!(result.is_err(), unwind);
            let values = CLEARS.take().unwrap();
            assert!(!values.is_empty());
            assert!(values.into_iter().all(|clean| clean));
        }
        // Framing refusals match the scalar owner and precede any lane write.
        let prefix = prefix_v1(false);
        let pair_prefix = prefix.pair_prefix_v1().unwrap();
        for final_field_len in [usize::MAX, usize::MAX - 8] {
            assert!(matches!(
                prefix.last_field_stream_at_with_counter(0, 0, &[b"field"], final_field_len),
                Err(PrivacyOuterLastFieldStreamErrorV1::FramingLimitExceeded)
            ));
            assert!(matches!(
                pair_prefix.last_field_pair_at_with_counter_v1(
                    [0, 1],
                    0,
                    &[b"field"],
                    final_field_len
                ),
                Err((0, PrivacyOuterLastFieldStreamErrorV1::FramingLimitExceeded))
            ));
        }
        // The cached prefix lanes are exactly the scalar prefix's absorbed state.
        let encoded = SerializedSpongeV1(prefix.state.inner.serialize());
        assert_eq!(
            usize::from(encoded.0[SERIALIZED_BYTES - 1]),
            pair_prefix.position
        );
        for (word, bytes) in pair_prefix.lanes.iter().zip(encoded.0.as_chunks::<8>().0) {
            assert_eq!(*word, u64::from_le_bytes(*bytes));
        }
    }
}
