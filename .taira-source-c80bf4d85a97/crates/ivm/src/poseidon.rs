//! Lightweight Poseidon hash adapters used by VM opcodes.
//!
//! The permutations operate on BN254 field elements via
//! [`bn254_vec`] which dispatches to SIMD backends when available.
//! This mirrors the circuit implementation used by Halo2 and allows
//! tests to exercise the same arithmetic on CPUs with SSE2, AVX2, AVX-512 or NEON.

use std::sync::OnceLock;

use ff::{Field, PrimeField};
use halo2curves::bn256::Fr;
use poseidon_primitives::poseidon::primitives::Spec;

use crate::bn254_vec::{self as field_vec, FieldElem};

#[derive(Debug)]
struct FrSpec;

impl Spec<Fr, 3, 2> for FrSpec {
    fn full_rounds() -> usize {
        8
    }
    fn partial_rounds() -> usize {
        56
    }
    fn sbox(val: Fr) -> Fr {
        val.pow_vartime([5])
    }
    fn secure_mds() -> usize {
        0
    }
}

impl Spec<Fr, 6, 5> for FrSpec {
    fn full_rounds() -> usize {
        8
    }
    fn partial_rounds() -> usize {
        56
    }
    fn sbox(val: Fr) -> Fr {
        val.pow_vartime([5])
    }
    fn secure_mds() -> usize {
        0
    }
}

fn to_u64(f: Fr) -> u64 {
    let repr = f.to_repr();
    let bytes = repr.as_ref();
    u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

fn poseidon2_params() -> (&'static Vec<[FieldElem; 3]>, &'static [[FieldElem; 3]; 3]) {
    static PARAMS: OnceLock<(Vec<[FieldElem; 3]>, [[FieldElem; 3]; 3])> = OnceLock::new();
    let params = PARAMS.get_or_init(|| {
        let (rc, m, _) = <FrSpec as Spec<Fr, 3, 2>>::constants();
        let rc_fe = rc.iter().map(|row| row.map(FieldElem::from_fr)).collect();
        let m_fe = m.map(|row| row.map(FieldElem::from_fr));
        (rc_fe, m_fe)
    });
    (&params.0, &params.1)
}

fn poseidon6_params() -> (&'static Vec<[FieldElem; 6]>, &'static [[FieldElem; 6]; 6]) {
    static PARAMS: OnceLock<(Vec<[FieldElem; 6]>, [[FieldElem; 6]; 6])> = OnceLock::new();
    let params = PARAMS.get_or_init(|| {
        let (rc, m, _) = <FrSpec as Spec<Fr, 6, 5>>::constants();
        let rc_fe = rc.iter().map(|row| row.map(FieldElem::from_fr)).collect();
        let m_fe = m.map(|row| row.map(FieldElem::from_fr));
        (rc_fe, m_fe)
    });
    (&params.0, &params.1)
}

/// Round constants for Poseidon2 expressed as BN254 limbs.
#[cfg(feature = "cuda")]
pub(crate) fn poseidon2_round_constants_words() -> &'static Vec<[[u64; 4]; 3]> {
    static WORDS: OnceLock<Vec<[[u64; 4]; 3]>> = OnceLock::new();
    WORDS.get_or_init(|| {
        poseidon2_params()
            .0
            .iter()
            .map(|row| row.map(|fe| fe.0))
            .collect()
    })
}

/// MDS matrix for Poseidon2 expressed as BN254 limbs.
#[cfg(feature = "cuda")]
pub(crate) fn poseidon2_mds_words() -> &'static [[[u64; 4]; 3]; 3] {
    static WORDS: OnceLock<[[[u64; 4]; 3]; 3]> = OnceLock::new();
    WORDS.get_or_init(|| {
        let (_, mds) = poseidon2_params();
        let mut out = [[[0u64; 4]; 3]; 3];
        for (row_idx, row) in mds.iter().enumerate() {
            for (col_idx, elem) in row.iter().enumerate() {
                out[row_idx][col_idx] = elem.0;
            }
        }
        out
    })
}

/// Round constants for Poseidon6 expressed as BN254 limbs.
#[cfg(feature = "cuda")]
pub(crate) fn poseidon6_round_constants_words() -> &'static Vec<[[u64; 4]; 6]> {
    static WORDS: OnceLock<Vec<[[u64; 4]; 6]>> = OnceLock::new();
    WORDS.get_or_init(|| {
        poseidon6_params()
            .0
            .iter()
            .map(|row| row.map(|fe| fe.0))
            .collect()
    })
}

/// MDS matrix for Poseidon6 expressed as BN254 limbs.
#[cfg(feature = "cuda")]
pub(crate) fn poseidon6_mds_words() -> &'static [[[u64; 4]; 6]; 6] {
    static WORDS: OnceLock<[[[u64; 4]; 6]; 6]> = OnceLock::new();
    WORDS.get_or_init(|| {
        let (_, mds) = poseidon6_params();
        let mut out = [[[0u64; 4]; 6]; 6];
        for (row_idx, row) in mds.iter().enumerate() {
            for (col_idx, elem) in row.iter().enumerate() {
                out[row_idx][col_idx] = elem.0;
            }
        }
        out
    })
}

pub fn poseidon2(a: u64, b: u64) -> u64 {
    #[cfg(feature = "cuda")]
    if let Some(res) = crate::cuda::poseidon2_cuda(a, b) {
        return res;
    }
    poseidon2_impl(a, b)
}

/// Hash a batch of Poseidon2 inputs in order, using CUDA acceleration when available.
pub fn poseidon2_many(inputs: &[(u64, u64)]) -> Vec<u64> {
    if inputs.is_empty() {
        return Vec::new();
    }
    if let Some(outputs) = crate::cuda::poseidon2_cuda_many(inputs)
        && outputs.len() == inputs.len()
    {
        return outputs;
    }
    inputs.iter().map(|&(a, b)| poseidon2_impl(a, b)).collect()
}

#[doc(hidden)]
pub fn poseidon2_simd(a: u64, b: u64) -> u64 {
    poseidon2_impl(a, b)
}

fn poseidon2_impl(a: u64, b: u64) -> u64 {
    let (round_constants, mds) = poseidon2_params();

    let mut state = [
        FieldElem::from_fr(Fr::from(a)),
        FieldElem::from_fr(Fr::from(b)),
        FieldElem([0u64; 4]),
    ];

    let rf_half = <FrSpec as Spec<Fr, 3, 2>>::full_rounds() / 2;
    let rp = <FrSpec as Spec<Fr, 3, 2>>::partial_rounds();

    let sbox = |x: FieldElem| {
        let x2 = field_vec::mul(x, x);
        let x4 = field_vec::mul(x2, x2);
        field_vec::mul(x4, x)
    };

    let apply_mds = |st: &mut [FieldElem; 3]| {
        let mut new_state = [FieldElem([0u64; 4]); 3];
        for (i, row) in mds.iter().enumerate() {
            let mut acc = FieldElem([0u64; 4]);
            for (m, s) in row.iter().zip(st.iter()) {
                let prod = field_vec::mul(*m, *s);
                acc = field_vec::add(acc, prod);
            }
            new_state[i] = acc;
        }
        *st = new_state;
    };

    for rc in round_constants.iter().take(rf_half) {
        for (i, s) in state.iter_mut().enumerate() {
            *s = sbox(field_vec::add(*s, rc[i]));
        }
        apply_mds(&mut state);
    }

    for rc in round_constants.iter().skip(rf_half).take(rp) {
        for (i, s) in state.iter_mut().enumerate() {
            *s = field_vec::add(*s, rc[i]);
        }
        state[0] = sbox(state[0]);
        apply_mds(&mut state);
    }

    let start = rf_half + rp;
    for rc in round_constants.iter().skip(start).take(rf_half) {
        for (i, s) in state.iter_mut().enumerate() {
            *s = sbox(field_vec::add(*s, rc[i]));
        }
        apply_mds(&mut state);
    }

    to_u64(state[0].to_fr())
}

pub fn poseidon6(inputs: [u64; 6]) -> u64 {
    #[cfg(feature = "cuda")]
    if let Some(res) = crate::cuda::poseidon6_cuda(inputs) {
        return res;
    }
    poseidon6_impl(inputs)
}

/// Hash a batch of Poseidon6 inputs in order, using CUDA acceleration when available.
pub fn poseidon6_many(inputs: &[[u64; 6]]) -> Vec<u64> {
    if inputs.is_empty() {
        return Vec::new();
    }
    if let Some(outputs) = crate::cuda::poseidon6_cuda_many(inputs)
        && outputs.len() == inputs.len()
    {
        return outputs;
    }
    inputs.iter().map(|&value| poseidon6_impl(value)).collect()
}

#[doc(hidden)]
pub fn poseidon6_simd(inputs: [u64; 6]) -> u64 {
    poseidon6_impl(inputs)
}

fn poseidon6_impl(inputs: [u64; 6]) -> u64 {
    let (round_constants, mds) = poseidon6_params();

    let mut state = [
        FieldElem::from_fr(Fr::from(inputs[0])),
        FieldElem::from_fr(Fr::from(inputs[1])),
        FieldElem::from_fr(Fr::from(inputs[2])),
        FieldElem::from_fr(Fr::from(inputs[3])),
        FieldElem::from_fr(Fr::from(inputs[4])),
        FieldElem::from_fr(Fr::from(inputs[5])),
    ];

    let rf_half = <FrSpec as Spec<Fr, 6, 5>>::full_rounds() / 2;
    let rp = <FrSpec as Spec<Fr, 6, 5>>::partial_rounds();

    let sbox = |x: FieldElem| {
        let x2 = field_vec::mul(x, x);
        let x4 = field_vec::mul(x2, x2);
        field_vec::mul(x4, x)
    };

    let apply_mds = |st: &mut [FieldElem; 6]| {
        let mut new_state = [FieldElem([0u64; 4]); 6];
        for (i, row) in mds.iter().enumerate() {
            let mut acc = FieldElem([0u64; 4]);
            for (m, s) in row.iter().zip(st.iter()) {
                let prod = field_vec::mul(*m, *s);
                acc = field_vec::add(acc, prod);
            }
            new_state[i] = acc;
        }
        *st = new_state;
    };

    for rc in round_constants.iter().take(rf_half) {
        for (i, s) in state.iter_mut().enumerate() {
            *s = sbox(field_vec::add(*s, rc[i]));
        }
        apply_mds(&mut state);
    }

    for rc in round_constants.iter().skip(rf_half).take(rp) {
        for (i, s) in state.iter_mut().enumerate() {
            *s = field_vec::add(*s, rc[i]);
        }
        state[0] = sbox(state[0]);
        apply_mds(&mut state);
    }

    let start = rf_half + rp;
    for rc in round_constants.iter().skip(start).take(rf_half) {
        for (i, s) in state.iter_mut().enumerate() {
            *s = sbox(field_vec::add(*s, rc[i]));
        }
        apply_mds(&mut state);
    }

    to_u64(state[0].to_fr())
}
