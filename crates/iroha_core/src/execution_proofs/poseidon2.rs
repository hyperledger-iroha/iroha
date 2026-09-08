//! Execution-only Poseidon2 Goldilocks sponge, width 16 / rate 8 / capacity 8.
//!
//! This is a single established sponge construction, not a concatenation of narrow sponges.
//! The six canonical output coordinates retain the native 48-byte digest encoding.
//! Parameters and the permutation KAT are pinned to Plonky3; see POSEIDON2_PROVENANCE.md.
//! Whole-proof qualification remains separate from this primitive's parameter target.

#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) use fastpq_isi::GoldilocksDigest384LastFieldStreamErrorV1 as StreamError;
use fastpq_isi::{GoldilocksDigest384V1, GoldilocksDigestDomainV1};
#[path = "poseidon2_constants.rs"]
mod constants;
use constants::{EXTERNAL_FINAL, EXTERNAL_INITIAL, INTERNAL, INTERNAL_DIAGONAL};

const MODULUS: u64 = 0xffff_ffff_0000_0001;
const WIDTH: usize = 16;
const RATE: usize = 8;
const FRAME: &[u8] = b"iroha:execution:poseidon2-goldilocks-w16-r8-c8:frame:v1";

#[inline]
fn reduce(value: u128) -> u64 {
    const EPSILON: u64 = 0xffff_ffff;
    let low = value as u64;
    let high = (value >> 64) as u64;
    let (low, borrowed) = low.overflowing_sub(high >> 32);
    let low = if borrowed {
        low.wrapping_sub(EPSILON)
    } else {
        low
    };
    let (value, carried) = low.overflowing_add((high & EPSILON) * EPSILON);
    let value = if carried {
        value.wrapping_add(EPSILON)
    } else {
        value
    };
    if value >= MODULUS {
        value - MODULUS
    } else {
        value
    }
}

#[inline]
fn add(a: u64, b: u64) -> u64 {
    let (value, carry) = a.overflowing_add(b);
    let value = if carry {
        value.wrapping_sub(MODULUS)
    } else {
        value
    };
    if value >= MODULUS {
        value - MODULUS
    } else {
        value
    }
}

#[inline]
fn mul(a: u64, b: u64) -> u64 {
    reduce(u128::from(a) * u128::from(b))
}

#[inline]
fn pow7(a: u64) -> u64 {
    let square = mul(a, a);
    mul(mul(mul(square, square), square), a)
}

fn external(state: &mut [u64; WIDTH]) {
    // Plonky3 MDSMat4: circulant(2,3,1,1), followed by the inter-block layer.
    for block in state.chunks_exact_mut(4) {
        let before: [u64; 4] = block.try_into().expect("four-word block");
        for row in 0..4 {
            block[row] = [2, 3, 1, 1]
                .into_iter()
                .enumerate()
                .fold(0, |sum, (col, weight)| {
                    add(sum, mul(weight, before[(row + col) % 4]))
                });
        }
    }
    let sums: [u64; 4] = core::array::from_fn(|column| {
        (column..WIDTH)
            .step_by(4)
            .fold(0, |sum, index| add(sum, state[index]))
    });
    for (index, word) in state.iter_mut().enumerate() {
        *word = add(*word, sums[index % 4]);
    }
}

fn permute(state: &mut [u64; WIDTH]) {
    external(state);
    for constants in EXTERNAL_INITIAL {
        for (word, constant) in state.iter_mut().zip(constants) {
            *word = pow7(add(*word, constant));
        }
        external(state);
    }
    for constant in INTERNAL {
        state[0] = pow7(add(state[0], constant));
        let sum = state.iter().copied().fold(0, add);
        for (word, diagonal) in state.iter_mut().zip(INTERNAL_DIAGONAL) {
            *word = add(mul(*word, diagonal), sum);
        }
    }
    for constants in EXTERNAL_FINAL {
        for (word, constant) in state.iter_mut().zip(constants) {
            *word = pow7(add(*word, constant));
        }
        external(state);
    }
}

#[derive(Clone)]
struct Sponge {
    state: [u64; WIDTH],
    position: usize,
}

impl Sponge {
    fn new() -> Self {
        Self {
            state: [0; WIDTH],
            position: 0,
        }
    }
    fn absorb(&mut self, value: u64) {
        debug_assert!(value < MODULUS);
        self.state[self.position] = add(self.state[self.position], value);
        self.position += 1;
        if self.position == RATE {
            permute(&mut self.state);
            self.position = 0;
        }
    }
    fn bytes(&mut self, tag: u64, bytes: &[u8]) {
        self.absorb(tag);
        self.absorb(bytes.len() as u64);
        let mut chunks = bytes.chunks_exact(7);
        for chunk in &mut chunks {
            let mut word = [0; 8];
            word[..7].copy_from_slice(chunk);
            self.absorb(u64::from_le_bytes(word));
        }
        let rest = chunks.remainder();
        let mut word = [0; 8];
        word[..rest.len()].copy_from_slice(rest);
        word[rest.len()] = 1;
        self.absorb(u64::from_le_bytes(word));
    }
    fn finish(mut self) -> GoldilocksDigest384V1 {
        // pad10*: a final nonzero field delimiter, zero fill, then one permutation.
        self.absorb(1);
        if self.position != 0 {
            permute(&mut self.state);
        }
        GoldilocksDigest384V1::new(core::array::from_fn(|i| self.state[i]))
            .expect("permutation outputs canonical residues")
    }
}

/// Hash a canonically framed execution message under the execution sponge suite.
pub(crate) fn hash_bytes_384_v1(
    domain: GoldilocksDigestDomainV1<'_>,
    fields: &[&[u8]],
) -> Option<GoldilocksDigest384V1> {
    let domain_fields = [
        domain.catalog,
        domain.protocol,
        domain.profile,
        domain.role,
        domain.phase,
    ];
    if u32::try_from(fields.len()).is_err()
        || domain_fields
            .iter()
            .chain(fields.iter())
            .any(|field| u32::try_from(field.len()).is_err())
    {
        return None;
    }
    let mut sponge = Sponge::new();
    sponge.bytes(1, FRAME);
    for (index, bytes) in domain_fields.into_iter().enumerate() {
        sponge.bytes(2 + index as u64, bytes);
    }
    sponge.bytes(7, &domain.level.to_le_bytes());
    sponge.bytes(8, &domain.index.to_le_bytes());
    sponge.bytes(9, &domain.counter.to_le_bytes());
    sponge.absorb(10);
    sponge.absorb(fields.len() as u64);
    for (index, bytes) in fields.iter().enumerate() {
        sponge.bytes(11 + index as u64, bytes);
    }
    Some(sponge.finish())
}

/// Incremental final field under exactly the same execution framing as the one-shot function.
#[cfg(any(test, feature = "privacy-release-evidence"))]
#[derive(Clone)]
pub(crate) struct LastFieldStream {
    sponge: Sponge,
    expected: usize,
    received: usize,
    pending: [u8; 7],
    pending_len: usize,
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
impl LastFieldStream {
    pub(crate) fn new(
        domain: GoldilocksDigestDomainV1<'_>,
        prefix: &[&[u8]],
        expected: usize,
    ) -> Result<Self, StreamError> {
        let domain_fields = [
            domain.catalog,
            domain.protocol,
            domain.profile,
            domain.role,
            domain.phase,
        ];
        let total = prefix
            .len()
            .checked_add(1)
            .ok_or(StreamError::FramingLimitExceeded)?;
        if u32::try_from(total).is_err()
            || u32::try_from(expected).is_err()
            || domain_fields
                .iter()
                .chain(prefix.iter())
                .any(|field| u32::try_from(field.len()).is_err())
        {
            return Err(StreamError::FramingLimitExceeded);
        }
        let mut sponge = Sponge::new();
        sponge.bytes(1, FRAME);
        for (index, bytes) in domain_fields.into_iter().enumerate() {
            sponge.bytes(2 + index as u64, bytes);
        }
        sponge.bytes(7, &domain.level.to_le_bytes());
        sponge.bytes(8, &domain.index.to_le_bytes());
        sponge.bytes(9, &domain.counter.to_le_bytes());
        sponge.absorb(10);
        sponge.absorb(total as u64);
        for (index, bytes) in prefix.iter().enumerate() {
            sponge.bytes(11 + index as u64, bytes);
        }
        sponge.absorb(11 + prefix.len() as u64);
        sponge.absorb(expected as u64);
        Ok(Self {
            sponge,
            expected,
            received: 0,
            pending: [0; 7],
            pending_len: 0,
        })
    }
    pub(crate) fn update(&mut self, bytes: &[u8]) -> Result<(), StreamError> {
        if bytes.len() > self.expected - self.received {
            return Err(StreamError::InputOverrun {
                expected: self.expected,
                received: self.received,
                additional: bytes.len(),
            });
        }
        for byte in bytes {
            self.pending[self.pending_len] = *byte;
            self.pending_len += 1;
            if self.pending_len == 7 {
                let mut word = [0; 8];
                word[..7].copy_from_slice(&self.pending);
                self.sponge.absorb(u64::from_le_bytes(word));
                self.pending = [0; 7];
                self.pending_len = 0;
            }
        }
        self.received += bytes.len();
        Ok(())
    }
    pub(crate) fn finalize(mut self) -> Result<GoldilocksDigest384V1, StreamError> {
        if self.received != self.expected {
            return Err(StreamError::InputUnderrun {
                expected: self.expected,
                received: self.received,
            });
        }
        let mut word = [0; 8];
        word[..self.pending_len].copy_from_slice(&self.pending[..self.pending_len]);
        word[self.pending_len] = 1;
        self.sponge.absorb(u64::from_le_bytes(word));
        Ok(self.sponge.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn published_plonky3_width16_permutation_vector() {
        let mut state = constants::UPSTREAM_INPUT;
        permute(&mut state);
        assert_eq!(state, constants::UPSTREAM_OUTPUT);
    }
    #[test]
    fn reduction_matches_full_width_modulo_reference() {
        let mut rng = 0x6a09_e667_f3bc_c908_u64;
        for _ in 0..100_000 {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            let high = rng;
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            let value = u128::from(high) << 64 | u128::from(rng);
            assert_eq!(reduce(value), (value % u128::from(MODULUS)) as u64);
        }
    }
    #[test]
    fn frame_separates_fields_domains_and_padding() {
        let domain = GoldilocksDigestDomainV1 {
            catalog: b"catalog",
            protocol: b"execution-v1",
            profile: b"race-v1",
            role: b"leaf",
            phase: b"row",
            level: 0,
            index: 0,
            counter: 0,
        };
        let empty = hash_bytes_384_v1(domain, &[]).unwrap();
        assert_ne!(empty, hash_bytes_384_v1(domain, &[b""]).unwrap());
        for size in 0..130 {
            let bytes = vec![0; size];
            let zero_extended = vec![0; size + 1];
            let digest = hash_bytes_384_v1(domain, &[&bytes]).unwrap();
            assert_ne!(
                digest,
                hash_bytes_384_v1(domain, &[&zero_extended]).unwrap()
            );
            assert_ne!(digest, hash_bytes_384_v1(domain, &[&bytes, b""]).unwrap());
            assert_ne!(
                digest,
                hash_bytes_384_v1(GoldilocksDigestDomainV1 { index: 1, ..domain }, &[&bytes])
                    .unwrap()
            );
        }
    }
    #[test]
    fn incremental_field_matches_every_split_and_fails_atomically() {
        let domain = GoldilocksDigestDomainV1 {
            catalog: b"catalog",
            protocol: b"execution-v1",
            profile: b"race-v1",
            role: b"leaf",
            phase: b"row",
            level: 0,
            index: u64::MAX,
            counter: u64::MAX,
        };
        for size in 0..130 {
            let bytes: Vec<_> = (0..size).map(|i| i as u8).collect();
            let expected = hash_bytes_384_v1(domain, &[b"prefix", &bytes]).unwrap();
            for split in 0..=size {
                let mut stream = LastFieldStream::new(domain, &[b"prefix"], size).unwrap();
                assert!(stream.update(&vec![0; size + 1]).is_err());
                stream.update(&bytes[..split]).unwrap();
                if split != size {
                    assert!(stream.clone().finalize().is_err());
                }
                stream.update(&bytes[split..]).unwrap();
                assert_eq!(stream.finalize().unwrap(), expected);
            }
        }
    }
}
