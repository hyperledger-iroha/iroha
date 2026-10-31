//! Closed paired SHA3-384 owner for initial row commitments.
//!
//! Original framing and finalization remain owned by the SHA3 implementation.

use super::{
    PrivacyOuterDigestV1, PrivacyOuterDomainPrefixV1, PrivacyOuterLastFieldStreamErrorV1,
    PrivacyOuterLastFieldStreamV1, Sha3_384, WipingSha3V1,
};
use sha3::digest::common::hazmat::{SerializableState, SerializedState};
use zeroize::Zeroize;

const RATE_BYTES: usize = 104;
const SERIALIZED_BYTES: usize = 201;
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

/// Two identically framed row streams with no caller-supplied sponge state.
pub(crate) struct PrivacyOuterLastFieldPairV1 {
    state: [[u64; 25]; 2],
    position: usize,
    remaining: [usize; 2],
}

impl PrivacyOuterDomainPrefixV1 {
    /// Construct two original frames with their original public row coordinates.
    pub(crate) fn last_field_pair_at_with_counter_v1(
        &self,
        indices: [u64; 2],
        counter: u64,
        prefix_fields: &[&[u8]],
        final_field_len: usize,
    ) -> Result<PrivacyOuterLastFieldPairV1, PairErrorV1> {
        let streams = [
            self.last_field_stream_at_with_counter(
                indices[0],
                counter,
                prefix_fields,
                final_field_len,
            )
            .map_err(|error| (0, error))?,
            self.last_field_stream_at_with_counter(
                indices[1],
                counter,
                prefix_fields,
                final_field_len,
            )
            .map_err(|error| (1, error))?,
        ];
        PrivacyOuterLastFieldPairV1::from_original_streams_v1(&streams)
    }
}

impl PrivacyOuterLastFieldPairV1 {
    fn from_original_streams_v1(
        streams: &[PrivacyOuterLastFieldStreamV1; 2],
    ) -> Result<Self, PairErrorV1> {
        // Install clearing custody before the first private word is copied.
        let mut owner = Self {
            state: [[0; 25]; 2],
            position: 0,
            remaining: [0; 2],
        };
        for (lane, stream) in streams.iter().enumerate() {
            let encoded = SerializedSpongeV1(stream.state.inner.serialize());
            if encoded.0.len() != SERIALIZED_BYTES {
                return Err((
                    lane,
                    PrivacyOuterLastFieldStreamErrorV1::FramingLimitExceeded,
                ));
            }
            let position = usize::from(encoded.0[200]);
            if position >= RATE_BYTES || (lane != 0 && position != owner.position) {
                return Err((
                    lane,
                    PrivacyOuterLastFieldStreamErrorV1::FramingLimitExceeded,
                ));
            }
            owner.position = position;
            owner.remaining[lane] = stream.remaining;
            for (word, bytes) in owner.state[lane]
                .iter_mut()
                .zip(encoded.0[..200].as_chunks::<8>().0)
            {
                *word = u64::from_le_bytes(*bytes);
            }
        }
        Ok(owner)
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
        let mut consumed = 0;
        while consumed < input[0].len() {
            let count = (RATE_BYTES - self.position).min(input[0].len() - consumed);
            for lane in 0..2 {
                absorb_bytes_v1(
                    &mut self.state[lane],
                    self.position,
                    &input[lane][consumed..consumed + count],
                );
            }
            self.position += count;
            consumed += count;
            if self.position == RATE_BYTES {
                fastpq_prover::keccak_permute_pair_v1(&mut self.state);
                self.position = 0;
            }
        }
        for lane in 0..2 {
            self.remaining[lane] -= input[lane].len();
        }
        Ok(())
    }

    fn original_finalizer_v1(&self, lane: usize) -> Result<WipingSha3V1, PairErrorV1> {
        let mut encoded = SerializedSpongeV1(SerializedState::<Sha3_384>::default());
        if encoded.0.len() != SERIALIZED_BYTES || self.position >= RATE_BYTES {
            return Err((
                lane,
                PrivacyOuterLastFieldStreamErrorV1::FramingLimitExceeded,
            ));
        }
        for (bytes, word) in encoded.0[..200].chunks_exact_mut(8).zip(&self.state[lane]) {
            bytes.copy_from_slice(&word.to_le_bytes());
        }
        // The checked original rate is104, so this conversion is exact.
        encoded.0[200] = self.position as u8;
        Sha3_384::deserialize(&encoded.0)
            .map(|inner| WipingSha3V1 { inner })
            .map_err(|_| {
                (
                    lane,
                    PrivacyOuterLastFieldStreamErrorV1::FramingLimitExceeded,
                )
            })
    }

    /// Write to the caller's clearing digest tile only after both lane checks.
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
        // Both original SHA3 owners are established before either output write.
        // Finalize a clone and wipe each original slot in place, exactly as the
        // scalar owner does; never leave a consumed array slot vacated.
        let mut finalizers = [
            self.original_finalizer_v1(0)?,
            self.original_finalizer_v1(1)?,
        ];
        for (state, target) in finalizers.iter_mut().zip(output) {
            *target = state.finish_in_place_v1();
        }
        self.state.zeroize();
        self.position.zeroize();
        self.remaining = [usize::MAX; 2];
        Ok(())
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

/// Conservative constructor/finalizer overlap, including exact encoded scratch.
pub(crate) const fn paired_stream_workspace_bytes_v1() -> usize {
    2 * size_of::<PrivacyOuterLastFieldStreamV1>()
        + size_of::<PrivacyOuterLastFieldPairV1>()
        + 2 * size_of::<SerializedSpongeV1>()
        + 256
}
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
        let prefix = prefix_v1(false);
        let mut scalar = [0, 1].map(|row| {
            prefix
                .last_field_stream_at_with_counter(row, 0, &[], 8)
                .unwrap()
        });
        scalar[1].update(&[7]).unwrap();
        assert!(matches!(
            PrivacyOuterLastFieldPairV1::from_original_streams_v1(&scalar),
            Err((1, PrivacyOuterLastFieldStreamErrorV1::FramingLimitExceeded))
        ));
    }
}
