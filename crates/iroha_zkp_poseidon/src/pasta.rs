//! Pinned upstream Pasta P128Pow5T3 native candidate, with no production callers.
//!
//! This is original Poseidon: width 3, rate 2, eight full and 56 partial rounds.
//! The fixed-length hash follows upstream `ConstantLength<L>` exactly for positive
//! lengths: capacity last, length times 2^64, zero padding and first-coordinate output.
//! See `pasta/README.md` for the pinned source, constants and test-vector provenance.
//! Parameter choices are fixed; inputs and outputs are canonical little-endian fields.
//!
//! Owned permutation and matrix scratch is cleared on success, error and unwind.
//! Borrowed inputs, returned bytes, caller buffers, arithmetic temporaries and compiler
//! copies remain outside that guarantee. These primitives do not supply application
//! domain separation, key generation or a hiding commitment by themselves.
//!
//! TODO: qualify the circuit adapters and protocol composition before adding production
//! callers. This candidate does not replace existing admitted 57-round constructions.

use halo2curves::{
    ff::PrimeField,
    pasta::{Fp, Fq},
};
use std::{fmt, sync::OnceLock};
use zeroize::{DefaultIsZeroes, Zeroize, Zeroizing};

const WIDTH: usize = 3;
const ROUNDS: usize = 64;
const PARAMETER_LENGTH: usize = (ROUNDS * WIDTH + WIDTH * WIDTH) * 32;

/// Invalid input to the fixed Pasta primitive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// An input was not a canonical field element strictly below the modulus.
    NonCanonicalField,
    /// The upstream fixed-length construction requires a positive input length.
    EmptyInput,
    /// The fixed length cannot be encoded in the u64 length domain.
    LengthOverflow,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NonCanonicalField => "noncanonical Pasta field element",
            Self::EmptyInput => "Pasta constant-length hashing requires a nonempty input",
            Self::LengthOverflow => "Pasta constant-length hash input exceeds the u64 domain",
        })
    }
}

impl std::error::Error for Error {}

struct Parameters<F> {
    rounds: [[F; WIDTH]; ROUNDS],
    mds: [[F; WIDTH]; WIDTH],
}

#[derive(Clone, Copy)]
struct ClearState<F: PrimeField>([F; WIDTH]);

impl<F: PrimeField> Default for ClearState<F> {
    fn default() -> Self {
        Self([F::ZERO; WIDTH])
    }
}

// Zeroize uses a volatile write of this exact default value plus its compiler fence.
impl<F: PrimeField> DefaultIsZeroes for ClearState<F> {}

struct OwnedState<F: PrimeField>(ClearState<F>);

impl<F: PrimeField> OwnedState<F> {
    fn zero() -> Self {
        Self(ClearState::default())
    }
}

impl<F: PrimeField> Drop for OwnedState<F> {
    fn drop(&mut self) {
        self.0.zeroize();
        #[cfg(test)]
        tests::observe_clear(self.0.0.iter().all(|value| bool::from(value.is_zero())));
    }
}

struct ClearRepr<R: AsMut<[u8]>>(R);

impl<R: AsMut<[u8]>> Zeroize for ClearRepr<R> {
    fn zeroize(&mut self) {
        self.0.as_mut().zeroize();
    }
}

fn decode<F: PrimeField>(bytes: &[u8; 32]) -> Result<F, Error> {
    let mut repr = Zeroizing::new(ClearRepr(F::Repr::default()));
    repr.0.as_mut().copy_from_slice(bytes);
    Option::from(F::from_repr(repr.0)).ok_or(Error::NonCanonicalField)
}

fn encode<F: PrimeField>(value: &F) -> [u8; 32] {
    let repr = Zeroizing::new(ClearRepr(value.to_repr()));
    repr.0.as_ref().try_into().expect("fixed Pasta field width")
}

fn parameters<F: PrimeField>(encoded: &[u8; PARAMETER_LENGTH]) -> Parameters<F> {
    let mut result = Parameters {
        rounds: [[F::ZERO; WIDTH]; ROUNDS],
        mds: [[F::ZERO; WIDTH]; WIDTH],
    };
    for (value, bytes) in result
        .rounds
        .iter_mut()
        .chain(result.mds.iter_mut())
        .flatten()
        .zip(encoded.chunks_exact(32))
    {
        *value = decode(bytes.try_into().expect("fixed parameter width"))
            .expect("pinned canonical public parameter");
    }
    result
}

fn permute<F: PrimeField>(state: &mut OwnedState<F>, params: &Parameters<F>) {
    let mut mixed = OwnedState::<F>::zero();
    for (round, constants) in params.rounds.iter().enumerate() {
        for (value, constant) in state.0.0.iter_mut().zip(constants) {
            *value += constant;
        }
        for column in 0..if !(4..60).contains(&round) { WIDTH } else { 1 } {
            let value = &mut state.0.0[column];
            *value *= value.square().square();
        }
        for (output, row) in mixed.0.0.iter_mut().zip(&params.mds) {
            *output = F::ZERO;
            for (coefficient, input) in row.iter().zip(&state.0.0) {
                *output += *coefficient * input;
            }
        }
        state.0.0.copy_from_slice(&mixed.0.0);
    }
}

fn permute_bytes<F: PrimeField>(
    state: &mut [[u8; 32]; WIDTH],
    params: &Parameters<F>,
) -> Result<(), Error> {
    let mut owned = OwnedState::zero();
    for (value, bytes) in owned.0.0.iter_mut().zip(state.iter()) {
        *value = decode(bytes)?;
    }
    permute(&mut owned, params);
    for (bytes, value) in state.iter_mut().zip(&owned.0.0) {
        *bytes = encode(value);
    }
    Ok(())
}

fn hash<F: PrimeField, const L: usize>(
    message: &[[u8; 32]; L],
    params: &Parameters<F>,
) -> Result<[u8; 32], Error> {
    if L == 0 {
        return Err(Error::EmptyInput);
    }
    let length = u64::try_from(L).map_err(|_| Error::LengthOverflow)?;
    let mut state = OwnedState::zero();
    state.0.0[2] = F::from_u128(u128::from(length) << 64);
    for pair in message.chunks(2) {
        for (cell, value) in state.0.0.iter_mut().zip(pair) {
            *cell += decode::<F>(value)?;
        }
        // An odd final pair is padded by adding zero to the second coordinate.
        permute(&mut state, params);
    }
    Ok(encode(&state.0.0[0]))
}

/// Pallas base/Vesta scalar field operations with the pinned Fp constants.
pub mod fp {
    use super::*;

    /// Round-major constants followed by row-major MDS, each canonical 32-byte LE.
    pub const PARAMETER_BYTES: &[u8; PARAMETER_LENGTH] = include_bytes!("pasta/fp.bin");

    fn specification() -> &'static Parameters<Fp> {
        static PARAMS: OnceLock<Parameters<Fp>> = OnceLock::new();
        PARAMS.get_or_init(|| parameters(PARAMETER_BYTES))
    }

    /// Apply the fixed permutation to three canonical Fp values in place.
    ///
    /// # Errors
    /// Rejects any noncanonical input without changing the caller's state.
    pub fn permute(state: &mut [[u8; 32]; 3]) -> Result<(), Error> {
        permute_bytes(state, specification())
    }

    /// Hash a fixed, positive-length list of canonical Fp values.
    ///
    /// # Errors
    /// Rejects empty input, an unrepresentable length or any noncanonical field.
    pub fn hash<const L: usize>(message: &[[u8; 32]; L]) -> Result<[u8; 32], Error> {
        super::hash(message, specification())
    }
}

/// Vesta base/Pallas scalar field operations with the pinned Fq constants.
pub mod fq {
    use super::*;

    /// Round-major constants followed by row-major MDS, each canonical 32-byte LE.
    pub const PARAMETER_BYTES: &[u8; PARAMETER_LENGTH] = include_bytes!("pasta/fq.bin");

    fn specification() -> &'static Parameters<Fq> {
        static PARAMS: OnceLock<Parameters<Fq>> = OnceLock::new();
        PARAMS.get_or_init(|| parameters(PARAMETER_BYTES))
    }

    /// Apply the fixed permutation to three canonical Fq values in place.
    ///
    /// # Errors
    /// Rejects any noncanonical input without changing the caller's state.
    pub fn permute(state: &mut [[u8; 32]; 3]) -> Result<(), Error> {
        permute_bytes(state, specification())
    }

    /// Hash a fixed, positive-length list of canonical Fq values.
    ///
    /// # Errors
    /// Rejects empty input, an unrepresentable length or any noncanonical field.
    pub fn hash<const L: usize>(message: &[[u8; 32]; L]) -> Result<[u8; 32], Error> {
        super::hash(message, specification())
    }
}

#[cfg(test)]
#[path = "pasta/tests.rs"]
mod tests;
