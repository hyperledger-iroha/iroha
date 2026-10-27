#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use ff::PrimeField;
use num_bigint::BigUint;

use crate::CurveAffine;

pub mod curve;
#[macro_use]
pub mod field;
pub mod pairing;

// SEED for random tests.
pub(crate) const SEED: [u8; 16] = [
    0x59, 0x62, 0xbe, 0x5d, 0x76, 0x3d, 0x31, 0x8d, 0x17, 0xdb, 0x37, 0x32, 0x54, 0x06, 0xbc, 0xe5,
];
// Helper functions for converting between
// Field <> BigInt <> Hex string
pub(crate) fn hex_to_bytes(hex: &str) -> Vec<u8> {
    let bytes = hex.as_bytes().to_vec();
    bytes
        .chunks(2)
        .map(|chunk| u8::from_str_radix(core::str::from_utf8(chunk).unwrap(), 16).unwrap())
        .collect()
}

pub(crate) fn hex_to_field<F: PrimeField>(hex: &str) -> F {
    let mut bytes = hex_to_bytes(hex);
    bytes.reverse();
    let mut repr = F::Repr::default();
    repr.as_mut()[..bytes.len()].copy_from_slice(&bytes);
    F::from_repr(repr).unwrap()
}

pub(crate) fn point_from_hex<C: CurveAffine>(x: &str, y: &str) -> C {
    let x = hex_to_field(x);
    let y = hex_to_field(y);
    C::from_xy(x, y).unwrap()
}

pub(crate) fn fe_to_big<F: PrimeField>(fe: &F) -> BigUint {
    BigUint::from_bytes_le(fe.to_repr().as_ref())
}

pub fn big_to_fe<F: PrimeField>(e: &BigUint) -> F {
    let e = e % modulus::<F>();
    let bytes = e.to_bytes_le();
    let mut repr = F::Repr::default();
    repr.as_mut()[..bytes.len()].copy_from_slice(&bytes[..]);
    F::from_repr(repr).unwrap()
}

pub(crate) fn modulus<F: PrimeField>() -> BigUint {
    fe_to_big(&-F::ONE) + 1usize
}
