//! Fiat-Shamir transcript and spike-only deterministic randomness.

use ff::{Field, FromUniformBytes, PrimeField, WithSmallOrderMulGroup};
use iroha_pasta::poseidon::{PoseidonField, RATE, WIDTH, permute};
use iroha_pasta::{PastaAffine, PastaCurve};
use zeroize::{Zeroize, Zeroizing};

/// The scalar fields used here: 32-byte canonical encodings, wide reduction.
pub trait Scalar: PrimeField<Repr = [u8; 32]> + FromUniformBytes<64> {}
impl<T: PrimeField<Repr = [u8; 32]> + FromUniformBytes<64>> Scalar for T {}

/// Bits per challenge: one squeezed base-field element gives two.
pub const CHALLENGE_BITS: usize = 126;

/// Recursion-friendly transcript: an RP57 duplex sponge (rate 2) over the base
/// field of the proof's points, so the circuit where those points are native
/// hashes it directly.
/// - Points are absorbed as (x, y); the identity is absorbed as (0, 0), which
///   no circuit can allocate, so in-circuit verification rejects it.
/// - Scalars are embedded when the scalar modulus is below the base modulus
///   (Vesta proofs: Fp into Fq), else absorbed as 128-bit low and high halves.
/// - Each squeeze permutes and yields two 126-bit challenges from the
///   canonical bits of the first state word. Challenges that multiply points
///   in the verifier are Halo endoscalars of those bits (`endo_challenge`);
///   the others are the 126-bit integers themselves (`challenge`).
/// - Tags are ignored: the absorption order is fixed by the shape.
#[derive(Clone)]
pub struct Transcript<C: PastaCurve> {
    state: [C::Base; WIDTH],
    pending: usize,
    spare: Option<Vec<bool>>,
    pub absorbed: usize,
    pub squeezes: usize,
}

impl<C: PastaCurve> Transcript<C> {
    pub fn new(label: &[u8]) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"kagemusha-zks-spike/v2");
        hasher.update(label);
        let mut wide = [0_u8; 64];
        hasher.finalize_xof().fill(&mut wide);
        let state = [
            C::Base::ZERO,
            C::Base::ZERO,
            C::Base::from_uniform_bytes(&wide),
        ];
        Self {
            state,
            pending: 0,
            spare: None,
            absorbed: 0,
            squeezes: 0,
        }
    }

    /// The sponge state and the number of rate words used since the last
    /// permutation (the circuit starts from the state after the constant header).
    pub fn snapshot(&self) -> ([C::Base; WIDTH], usize) {
        (self.state, self.pending)
    }

    pub fn absorb(&mut self, x: C::Base) {
        if self.pending == RATE {
            permute(&mut self.state);
            self.pending = 0;
        }
        self.state[self.pending] += x;
        self.pending += 1;
        self.spare = None;
        self.absorbed += 1;
    }

    /// Constants (the shape header), eight bytes per element.
    pub fn bytes(&mut self, _tag: &[u8], data: &[u8]) {
        for chunk in data.chunks(8) {
            let mut word = [0_u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.absorb(C::Base::from(u64::from_le_bytes(word)));
        }
    }

    pub fn point(&mut self, _tag: &[u8], p: &C::AffineExt) {
        let (x, y) = Option::from(p.coordinates()).unwrap_or((C::Base::ZERO, C::Base::ZERO));
        self.absorb(x);
        self.absorb(y);
    }

    /// Whether scalars embed directly (scalar modulus below base modulus).
    pub fn scalars_embed() -> bool {
        Option::<C::Base>::from(C::Base::from_repr((-C::ScalarExt::ONE).to_repr())).is_some()
    }

    /// A word of the other curve's field (32-byte encoding) as two 128-bit
    /// limbs: how the pairing digest, rho and R enter each transcript.
    pub fn word_limbs(&mut self, _tag: &[u8], repr: &[u8; 32]) {
        self.absorb(C::Base::from_u128(u128::from_le_bytes(
            repr[..16].try_into().unwrap(),
        )));
        self.absorb(C::Base::from_u128(u128::from_le_bytes(
            repr[16..].try_into().unwrap(),
        )));
    }

    pub fn scalar(&mut self, _tag: &[u8], value: &C::ScalarExt) {
        for word in scalar_words::<C>(value) {
            self.absorb(word);
        }
    }

    fn squeeze(&mut self) -> Vec<bool> {
        permute(&mut self.state);
        self.pending = 0;
        self.squeezes += 1;
        let repr = self.state[0].to_repr();
        (0..255)
            .map(|i| (repr[i / 8] >> (i % 8)) & 1 == 1)
            .collect()
    }

    /// The next 126 challenge bits (little-endian).
    pub fn challenge_bits(&mut self) -> Vec<bool> {
        if let Some(bits) = self.spare.take() {
            return bits;
        }
        let bits = self.squeeze();
        self.spare = Some(bits[CHALLENGE_BITS..2 * CHALLENGE_BITS].to_vec());
        bits[..CHALLENGE_BITS].to_vec()
    }

    /// Whether the next challenge comes from the current squeeze's spare half.
    pub fn has_spare(&self) -> bool {
        self.spare.is_some()
    }

    /// A full-width challenge: the whole squeezed word, recombined from its two
    /// 128-bit halves in the scalar field (the halves are what crosses circuits).
    pub fn challenge_full(&mut self, _tag: &[u8]) -> C::ScalarExt {
        permute(&mut self.state);
        self.pending = 0;
        self.squeezes += 1;
        self.spare = None;
        full_scalar(&self.state[0].to_repr())
    }

    /// The word the last squeeze produced (for the circuit replay).
    pub fn last_word(&self) -> C::Base {
        self.state[0]
    }

    /// A raw challenge: the 126-bit integer.
    pub fn challenge(&mut self, _tag: &[u8]) -> C::ScalarExt {
        raw_scalar(&self.challenge_bits())
    }

    /// A point-multiplying challenge: the Halo endoscalar of 126 bits.
    pub fn endo_challenge(&mut self, _tag: &[u8]) -> C::ScalarExt {
        endo_scalar(&self.challenge_bits())
    }

    pub fn challenges(&mut self, tag: &[u8], count: usize) -> Vec<C::ScalarExt> {
        (0..count).map(|_| self.challenge(tag)).collect()
    }
}

/// The base-field words a scalar is absorbed as.
pub fn scalar_words<C: PastaCurve>(value: &C::ScalarExt) -> Vec<C::Base> {
    let repr = value.to_repr();
    if Transcript::<C>::scalars_embed() {
        vec![Option::from(C::Base::from_repr(repr)).expect("embeds")]
    } else {
        let lo = u128::from_le_bytes(repr[..16].try_into().unwrap());
        let hi = u128::from_le_bytes(repr[16..].try_into().unwrap());
        vec![C::Base::from_u128(lo), C::Base::from_u128(hi)]
    }
}

/// A base-field word as a scalar: lo + 2^128 hi, reduced in the scalar field.
pub fn full_scalar<S: PrimeField>(repr: &[u8; 32]) -> S {
    let lo = u128::from_le_bytes(repr[..16].try_into().unwrap());
    let hi = u128::from_le_bytes(repr[16..].try_into().unwrap());
    S::from_u128(lo) + S::from_u128(hi) * S::from(2).pow_vartime([128])
}

/// The fixed challenge schedule of this backend:
/// - tau_i = t^(2^i) for one full-width challenge t: F(tau) is multilinear, so
///   distinct monomials map to distinct powers (binary), degree < 2^sx;
/// - a committed round's k challenges are r^(3^j) for one 126-bit challenge:
///   round polynomials have degree at most 2 per variable, so base 3 keeps
///   monomials distinct, total degree at most 3^k - 1;
/// - r_A, r_B, r_C = rho, rho^2, rho^3;
/// - the inner rounds holding r_sel and the row bits keep per-variable
///   challenges (the row bits are endoscalars for the Hyrax fold).
pub fn lean_schedule() -> bool {
    true
}

pub fn powers2<S: Field>(t: S, n: usize) -> Vec<S> {
    let mut out = vec![t];
    for _ in 1..n {
        let last = *out.last().unwrap();
        out.push(last.square());
    }
    out
}

pub fn kron3<S: Field>(r: S, k: usize) -> Vec<S> {
    let mut out = vec![r];
    for _ in 1..k {
        let last = *out.last().unwrap();
        out.push(last.square() * last);
    }
    out
}

pub fn raw_scalar<S: PrimeField>(bits: &[bool]) -> S {
    let v = bits
        .iter()
        .enumerate()
        .fold(0_u128, |acc, (i, b)| acc | ((*b as u128) << i));
    S::from_u128(v)
}

/// Halo's endoscalar of bit pairs (sign, endo), most significant pair first:
/// a = b = 2; per pair (a, b) = (2a, 2b), then a or b (endo or not) gains the
/// sign; the scalar is a zeta + b, with zeta the scalar acting as the
/// endomorphism. Matches `gadgets::endoscale` (and `endo_point` below).
pub fn endo_coefficients(bits: &[bool]) -> (u128, u128, i128, i128) {
    assert_eq!(bits.len() % 2, 0);
    let (mut a, mut b): (i128, i128) = (2, 2);
    for i in (0..bits.len() / 2).rev() {
        a *= 2;
        b *= 2;
        let s = if bits[2 * i] { 1 } else { -1 };
        if bits[2 * i + 1] {
            a += s;
        } else {
            b += s;
        }
    }
    (a.unsigned_abs(), b.unsigned_abs(), a.signum(), b.signum())
}

pub fn endo_scalar<S: PrimeField + WithSmallOrderMulGroup<3>>(bits: &[bool]) -> S {
    let (a, b, sa, sb) = endo_coefficients(bits);
    let signed = |m: u128, s: i128| {
        if s < 0 {
            -S::from_u128(m)
        } else {
            S::from_u128(m)
        }
    };
    signed(a, sa) * S::ZETA + signed(b, sb)
}

/// Rate-2 sponge digest of public inputs in their own field.
pub fn io_digest<S: PoseidonField>(io: &[S]) -> S {
    let mut state = [S::ZERO, S::ZERO, S::from(io.len() as u64)];
    for pair in io.chunks(RATE) {
        for (slot, value) in state.iter_mut().zip(pair) {
            *slot += value;
        }
        permute(&mut state);
    }
    state[0]
}

/// Deterministic randomness for the spike only (witness, blinds, masks).
/// A wallet draws these from its CSPRNG.
#[derive(Clone)]
pub struct SpikeRng(blake3::OutputReader);

impl Drop for SpikeRng {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// Fresh secret randomness for one complete native proof operation. The seed
/// is never exported or serialized; distinct operations require fresh OS
/// entropy. Recreating a named stream is only for the same operation's private
/// first-phase replay while constructing its linked proof pair.
pub struct ProofEntropy(Zeroizing<[u8; 32]>);

impl ProofEntropy {
    /// Fail closed if the operating system cannot supply cryptographic entropy.
    pub fn fresh() -> Result<Self, getrandom::Error> {
        let mut key = Zeroizing::new([0u8; 32]);
        getrandom::fill(&mut *key)?;
        Ok(Self(key))
    }

    /// Domain-separated blinding stream within this single proof operation.
    pub(crate) fn stream(&self, purpose: &[u8]) -> SpikeRng {
        let mut hasher = blake3::Hasher::new_keyed(&self.0);
        hasher.update(b"kagemusha-spartan/prover-entropy/v1");
        hasher.update(&(purpose.len() as u64).to_le_bytes());
        hasher.update(purpose);
        let reader = hasher.finalize_xof();
        hasher.zeroize();
        SpikeRng(reader)
    }
}

impl SpikeRng {
    pub fn new(seed: &[u8]) -> Self {
        let mut state = blake3::Hasher::new();
        state.update(b"kagemusha-zks-spike/rng");
        state.update(seed);
        Self(state.finalize_xof())
    }

    /// Uniform field element.
    pub fn scalar<F: Scalar>(&mut self) -> F {
        let mut wide = [0_u8; 64];
        self.0.fill(&mut wide);
        F::from_uniform_bytes(&wide)
    }

    /// Uniform integer below `2^bits`, `bits <= 128`.
    pub fn below<F: Scalar>(&mut self, bits: u32) -> F {
        let mut bytes = [0_u8; 16];
        self.0.fill(&mut bytes);
        let value = u128::from_le_bytes(bytes);
        let value = if bits >= 128 {
            value
        } else {
            value & ((1_u128 << bits) - 1)
        };
        F::from_u128(value)
    }
}
