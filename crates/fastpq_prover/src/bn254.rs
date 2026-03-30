//! Shared BN254 canonical-limb helpers for FASTPQ GPU backends.

#[cfg(test)]
use core::convert::TryInto;

use halo2curves::{bn256::Fr as Bn254Fr, ff::PrimeField};
use iroha_zkp_halo2::{Bn254Scalar, IpaScalar};

/// Canonical BN254 scalars are represented as four little-endian `u64` limbs.
pub const BN254_LIMBS: usize = 4;

/// Return the supported 2-adicity of the BN254 scalar field.
pub fn two_adicity() -> u32 {
    Bn254Fr::S
}

/// Validate that a staged BN254 FFT/LDE log size is supported.
pub fn validate_log(log_size: u32) -> Result<(), &'static str> {
    if log_size == 0 {
        return Err("BN254 FFT requires log_size greater than zero");
    }
    if log_size > two_adicity() {
        return Err("BN254 FFT exceeds supported two-adicity");
    }
    Ok(())
}

/// Convert a BN254 scalar into canonical little-endian limbs.
pub fn scalar_to_canonical_limbs(value: &Bn254Scalar) -> [u64; BN254_LIMBS] {
    let bytes = (*value).to_bytes();
    let mut limbs = [0u64; BN254_LIMBS];
    for (index, limb) in limbs.iter_mut().enumerate() {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&bytes[index * 8..(index + 1) * 8]);
        *limb = u64::from_le_bytes(buf);
    }
    limbs
}

/// Decode canonical little-endian limbs into a BN254 scalar.
pub fn scalar_from_canonical_limbs(
    limbs: &[u64; BN254_LIMBS],
) -> Result<Bn254Scalar, &'static str> {
    let mut bytes = [0u8; 32];
    for (index, limb) in limbs.iter().enumerate() {
        bytes[index * 8..(index + 1) * 8].copy_from_slice(&limb.to_le_bytes());
    }
    Bn254Scalar::from_bytes(&bytes)
        .map_err(|_| "BN254 canonical limbs decode produced invalid field element")
}

/// Decode a four-limb slice into a BN254 scalar.
#[cfg(test)]
pub fn limbs_slice_to_scalar(slice: &[u64]) -> Result<Bn254Scalar, &'static str> {
    let limbs: [u64; BN254_LIMBS] = slice
        .try_into()
        .expect("slice length should equal BN254 limb count");
    scalar_from_canonical_limbs(&limbs)
}

/// Compute the staged BN254 twiddle factors in scalar form for a radix-2 FFT.
pub fn stage_twiddles_scalars(log_size: u32) -> Result<Vec<Bn254Scalar>, &'static str> {
    validate_log(log_size)?;
    let n = 1usize << log_size;
    let stage_span = n / 2;
    let mut twiddles = vec![Bn254Scalar::zero(); (log_size as usize) * stage_span];
    let max_log = two_adicity();
    let mut omega = Bn254Scalar::from(Bn254Fr::ROOT_OF_UNITY);
    let exponent = 1u64 << (max_log - log_size);
    omega = omega.pow_u64(exponent);
    for stage in 0..log_size {
        let len = 1usize << (stage + 1);
        let half = len / 2;
        let stride = n / len;
        let stage_offset = stage as usize * stage_span;
        let stride_twiddle = omega.pow_u64(stride as u64);
        let mut value = Bn254Scalar::one();
        for pair in 0..half {
            if pair == 0 {
                value = Bn254Scalar::one();
            } else {
                value = value.mul(stride_twiddle);
            }
            twiddles[stage_offset + pair] = value;
        }
        if half < stage_span {
            for idx in half..stage_span {
                twiddles[stage_offset + idx] = twiddles[stage_offset + idx % half];
            }
        }
    }
    Ok(twiddles)
}

/// Compute the staged BN254 twiddle factors as canonical limbs.
pub fn stage_twiddles_limbs(log_size: u32) -> Result<Vec<[u64; BN254_LIMBS]>, &'static str> {
    let scalars = stage_twiddles_scalars(log_size)?;
    let twiddles: Vec<[u64; BN254_LIMBS]> = scalars
        .into_iter()
        .map(|scalar| scalar_to_canonical_limbs(&scalar))
        .collect();
    validate_twiddles_shape(log_size, &twiddles)?;
    Ok(twiddles)
}

/// Flatten staged BN254 twiddles into a single limb buffer.
pub fn flatten_twiddles(twiddles: &[[u64; BN254_LIMBS]]) -> Vec<u64> {
    let mut flat = Vec::with_capacity(twiddles.len() * BN254_LIMBS);
    for limbs in twiddles {
        flat.extend_from_slice(limbs);
    }
    flat
}

/// Validate that the staged twiddle table matches the requested FFT log size.
pub fn validate_twiddles_shape(
    log_size: u32,
    twiddles: &[[u64; BN254_LIMBS]],
) -> Result<(), &'static str> {
    let expected = fft_twiddle_len(log_size)?;
    if twiddles.len() != expected {
        return Err("BN254 staged twiddle table length mismatch");
    }
    Ok(())
}

/// Return the number of staged BN254 twiddles required for an FFT of `2^log_size`.
pub fn fft_twiddle_len(log_size: u32) -> Result<usize, &'static str> {
    validate_log(log_size)?;
    let n = 1usize << log_size;
    Ok((log_size as usize) * (n / 2))
}

/// Return the number of staged BN254 twiddles required for an LDE evaluation.
#[cfg(test)]
pub fn lde_twiddle_len(trace_log: u32, blowup_log: u32) -> Result<usize, &'static str> {
    let eval_log = trace_log
        .checked_add(blowup_log)
        .ok_or("BN254 LDE log size exceeds 32-bit representation")?;
    fft_twiddle_len(eval_log)
}

/// Validate a canonical BN254 column batch and return the element extent per column.
#[cfg(test)]
pub fn column_extent(columns: &[Vec<u64>]) -> Result<usize, &'static str> {
    if columns.is_empty() {
        return Ok(0);
    }
    let limb_len = columns[0].len();
    if !limb_len.is_multiple_of(BN254_LIMBS) {
        return Err("BN254 column length must be a multiple of four limbs");
    }
    if columns.iter().any(|column| column.len() != limb_len) {
        return Err("BN254 columns must share the same limb length");
    }
    Ok(limb_len / BN254_LIMBS)
}

/// Convert a canonical BN254 column into scalar values.
#[cfg(test)]
pub fn canonical_to_scalars(column: &[u64]) -> Vec<Bn254Scalar> {
    column
        .chunks_exact(BN254_LIMBS)
        .map(|chunk| limbs_slice_to_scalar(chunk).expect("valid scalar"))
        .collect()
}

/// Convert scalar BN254 columns back into canonical limbs.
#[cfg(test)]
pub fn scalars_to_canonical(columns: &[Vec<Bn254Scalar>]) -> Vec<Vec<u64>> {
    columns
        .iter()
        .map(|column| {
            let mut out = Vec::with_capacity(column.len() * BN254_LIMBS);
            for value in column {
                out.extend_from_slice(&scalar_to_canonical_limbs(value));
            }
            out
        })
        .collect()
}

/// Execute the staged BN254 radix-2 FFT on the provided scalar columns.
#[cfg(test)]
pub fn cpu_fft(columns: &mut [Vec<Bn254Scalar>], log_size: u32, twiddles: &[Bn254Scalar]) {
    let n = 1usize << log_size;
    for column in columns {
        for stage in 0..log_size {
            let len = 1usize << (stage + 1);
            let half = len / 2;
            let stage_offset = stage as usize * (n / 2);
            for block in (0..n).step_by(len) {
                for pair in 0..half {
                    let idx = block + pair;
                    let twiddle = twiddles[stage_offset + pair];
                    let u = column[idx];
                    let v = column[idx + half].mul(twiddle);
                    column[idx] = u.add(v);
                    column[idx + half] = u.sub(v);
                }
            }
        }
    }
}

/// Execute the staged BN254 coset LDE on the provided scalar columns.
#[cfg(test)]
pub fn cpu_lde(
    coeffs: &[Vec<Bn254Scalar>],
    trace_log: u32,
    blowup_log: u32,
    twiddles: &[Bn254Scalar],
    coset: Bn254Scalar,
) -> Vec<Vec<Bn254Scalar>> {
    let eval_log = trace_log + blowup_log;
    let trace_len = 1usize << trace_log;
    let eval_len = 1usize << eval_log;
    let mut outputs = Vec::with_capacity(coeffs.len());
    for column in coeffs {
        let mut data = vec![Bn254Scalar::zero(); eval_len];
        data[..trace_len].copy_from_slice(column);
        let mut coset_power = Bn254Scalar::one();
        for coeff in data.iter_mut().take(trace_len) {
            *coeff = (*coeff).mul(coset_power);
            coset_power = coset_power.mul(coset);
        }
        let mut column_fft = vec![data];
        cpu_fft(&mut column_fft, eval_log, twiddles);
        outputs.push(column_fft.pop().expect("single column present"));
    }
    outputs
}

/// Build deterministic BN254 sample columns for parity tests.
#[cfg(test)]
pub fn sample_columns(log_size: u32, column_count: usize) -> Vec<Vec<u64>> {
    let len = 1usize << log_size;
    let mut columns = Vec::with_capacity(column_count);
    for column in 0..column_count {
        let mut data = Vec::with_capacity(len * BN254_LIMBS);
        for row in 0..len {
            let value = Bn254Scalar::from(((column as u64 + 1) * 31).wrapping_add(row as u64));
            data.extend_from_slice(&scalar_to_canonical_limbs(&value));
        }
        columns.push(data);
    }
    columns
}

/// Build a deterministic BN254 test coset.
#[cfg(test)]
pub fn sample_coset() -> [u64; BN254_LIMBS] {
    scalar_to_canonical_limbs(&Bn254Scalar::from(5u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_canonical_limbs() {
        let value = Bn254Scalar::from(42u64);
        let limbs = scalar_to_canonical_limbs(&value);
        assert_eq!(scalar_from_canonical_limbs(&limbs).unwrap(), value);
    }

    #[test]
    fn twiddle_len_matches_fft_shape() {
        assert_eq!(fft_twiddle_len(2).unwrap(), 4);
        assert_eq!(lde_twiddle_len(2, 1).unwrap(), 12);
    }

    #[test]
    fn validate_log_rejects_zero() {
        assert_eq!(
            validate_log(0).expect_err("zero log rejected"),
            "BN254 FFT requires log_size greater than zero"
        );
    }

    #[test]
    fn column_extent_requires_limb_multiples() {
        let columns = vec![vec![1u64, 2, 3]];
        assert_eq!(
            column_extent(&columns).expect_err("invalid limb count rejected"),
            "BN254 column length must be a multiple of four limbs"
        );
    }
}
