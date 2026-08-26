//! Shared BN254 canonical-limb helpers for FASTPQ GPU backends.
use core::convert::TryInto;
use halo2curves::{bn256::Fr as Bn254Fr, ff::PrimeField};
use iroha_zkp_halo2::{Bn254Scalar, IpaScalar};
/// Canonical BN254 scalars are represented as four little-endian `u64` limbs.
pub const BN254_LIMBS: usize = 4;
/// Maximum canonical-limb bytes staged for a single BN254 FFT twiddle table.
pub const MAX_STAGED_TWIDDLE_BYTES: u64 = 1 << 30;
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
/// Validate a dense slice of canonical BN254 scalar limbs.
pub fn validate_canonical_limbs(limbs: &[u64]) -> Result<(), &'static str> {
    if !limbs.len().is_multiple_of(BN254_LIMBS) {
        return Err("BN254 canonical input length must be a multiple of four limbs");
    }
    for chunk in limbs.chunks_exact(BN254_LIMBS) {
        let scalar_limbs: [u64; BN254_LIMBS] = chunk
            .try_into()
            .expect("chunks_exact yields four BN254 limbs");
        scalar_from_canonical_limbs(&scalar_limbs)?;
    }
    Ok(())
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
#[cfg(any(test, all(feature = "fastpq-gpu", target_os = "macos")))]
pub fn stage_twiddles_scalars(log_size: u32) -> Result<Vec<Bn254Scalar>, &'static str> {
    validate_staged_twiddle_resources(log_size)?;
    validate_log(log_size)?;
    let n = 1usize << log_size;
    let twiddle_count = n
        .checked_sub(1)
        .ok_or("BN254 staged twiddle count exceeds platform limits")?;
    let mut twiddles = Vec::new();
    twiddles
        .try_reserve_exact(twiddle_count)
        .map_err(|_| "BN254 staged twiddle scalars exceed available host memory")?;
    twiddles.resize(twiddle_count, Bn254Scalar::zero());
    let max_log = two_adicity();
    let mut omega = Bn254Scalar::from(Bn254Fr::ROOT_OF_UNITY);
    let exponent = 1u64 << (max_log - log_size);
    omega = omega.pow_u64(exponent);
    for stage in 0..log_size {
        let len = 1usize << (stage + 1);
        let half = len / 2;
        let stride = n / len;
        let stage_offset = half - 1;
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
    }
    Ok(twiddles)
}
/// Compute the staged BN254 twiddle factors as canonical limbs.
pub fn stage_twiddles_limbs(log_size: u32) -> Result<Vec<[u64; BN254_LIMBS]>, &'static str> {
    validate_staged_twiddle_resources(log_size)?;
    validate_log(log_size)?;
    let n = 1usize << log_size;
    let twiddle_count = n
        .checked_sub(1)
        .ok_or("BN254 staged twiddle count exceeds platform limits")?;
    let mut twiddles = Vec::new();
    twiddles
        .try_reserve_exact(twiddle_count)
        .map_err(|_| "BN254 staged twiddle limbs exceed available host memory")?;
    let max_log = two_adicity();
    let mut omega = Bn254Scalar::from(Bn254Fr::ROOT_OF_UNITY);
    omega = omega.pow_u64(1u64 << (max_log - log_size));
    for stage in 0..log_size {
        let len = 1usize << (stage + 1);
        let half = len / 2;
        let stride_twiddle = omega.pow_u64((n / len) as u64);
        let mut value = Bn254Scalar::one();
        for pair in 0..half {
            if pair != 0 {
                value = value.mul(stride_twiddle);
            }
            twiddles.push(scalar_to_canonical_limbs(&value));
        }
    }
    validate_twiddles_shape(log_size, &twiddles)?;
    Ok(twiddles)
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
/// Return the number of packed staged BN254 twiddles required for an FFT of `2^log_size`.
pub fn fft_twiddle_len(log_size: u32) -> Result<usize, &'static str> {
    validate_log(log_size)?;
    let n = 1usize << log_size;
    n.checked_sub(1)
        .ok_or("BN254 staged twiddle count exceeds platform limits")
}
/// Return the canonical-limb byte size of the staged BN254 twiddle table.
pub fn staged_twiddle_byte_len(log_size: u32) -> Result<u64, &'static str> {
    let bytes = fft_twiddle_len(log_size)?
        .checked_mul(core::mem::size_of::<[u64; BN254_LIMBS]>())
        .ok_or("BN254 staged twiddle byte length exceeds platform limits")?;
    u64::try_from(bytes).map_err(|_| "BN254 staged twiddle byte length exceeds 64-bit limits")
}
/// Reject staged BN254 twiddle tables whose host construction could exhaust memory.
pub fn validate_staged_twiddle_resources(log_size: u32) -> Result<(), &'static str> {
    if staged_twiddle_byte_len(log_size)? > MAX_STAGED_TWIDDLE_BYTES {
        return Err("BN254 staged twiddle table exceeds the 1 GiB safety limit");
    }
    Ok(())
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
        bit_reverse(column, log_size);
        for stage in 0..log_size {
            let len = 1usize << (stage + 1);
            let half = len / 2;
            let stage_offset = half - 1;
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

#[cfg(test)]
fn bit_reverse(values: &mut [Bn254Scalar], log_size: u32) {
    for index in 1..values.len().saturating_sub(1) {
        let reversed = index.reverse_bits() >> (usize::BITS - log_size);
        if index < reversed {
            values.swap(index, reversed);
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

    fn direct_coset_evaluations(
        coefficients: &[Bn254Scalar],
        log_size: u32,
        coset: Bn254Scalar,
    ) -> Vec<Bn254Scalar> {
        let max_log = two_adicity();
        let exponent = 1u64 << (max_log - log_size);
        let omega = Bn254Scalar::from(Bn254Fr::ROOT_OF_UNITY).pow_u64(exponent);
        let mut point = coset;
        let mut evaluations = Vec::with_capacity(1usize << log_size);
        for _ in 0..(1usize << log_size) {
            let value = coefficients
                .iter()
                .rev()
                .fold(Bn254Scalar::zero(), |acc, coefficient| {
                    acc.mul(point).add(*coefficient)
                });
            evaluations.push(value);
            point = point.mul(omega);
        }
        evaluations
    }

    #[test]
    fn roundtrip_canonical_limbs() {
        let value = Bn254Scalar::from(42u64);
        let limbs = scalar_to_canonical_limbs(&value);
        assert_eq!(scalar_from_canonical_limbs(&limbs).unwrap(), value);
    }
    #[test]
    fn canonical_limb_validation_rejects_modulus_and_all_ones() {
        const MODULUS: [u64; BN254_LIMBS] = [
            0x43e1_f593_f000_0001,
            0x2833_e848_79b9_7091,
            0xb850_45b6_8181_585d,
            0x3064_4e72_e131_a029,
        ];
        validate_canonical_limbs(&scalar_to_canonical_limbs(&Bn254Scalar::from(1u64)))
            .expect("one is canonical");
        assert!(validate_canonical_limbs(&MODULUS).is_err());
        assert!(validate_canonical_limbs(&[u64::MAX; BN254_LIMBS]).is_err());
        assert!(validate_canonical_limbs(&[0; BN254_LIMBS - 1]).is_err());
    }
    #[test]
    fn twiddle_len_matches_fft_shape() {
        assert_eq!(fft_twiddle_len(2).unwrap(), 3);
        assert_eq!(lde_twiddle_len(2, 1).unwrap(), 7);
    }
    #[test]
    fn packed_limb_twiddles_match_scalar_staging() {
        let scalars = stage_twiddles_scalars(4).expect("scalar twiddles");
        let limbs = stage_twiddles_limbs(4).expect("limb twiddles");
        let expected = scalars
            .iter()
            .map(scalar_to_canonical_limbs)
            .collect::<Vec<_>>();
        assert_eq!(limbs, expected);
        assert_eq!(limbs.len(), (1usize << 4) - 1);
    }
    #[test]
    fn staged_twiddle_resource_limit_rejects_oom_scale_domains() {
        validate_staged_twiddle_resources(25).expect("log-25 staging stays below one GiB");
        assert!(validate_staged_twiddle_resources(26).is_err());
        assert!(stage_twiddles_scalars(26).is_err());
        assert!(validate_staged_twiddle_resources(two_adicity()).is_err());
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

    #[test]
    fn cpu_fft_matches_direct_polynomial_evaluation() {
        let log_size = 2;
        let coefficients = vec![
            Bn254Scalar::zero(),
            Bn254Scalar::one(),
            Bn254Scalar::zero(),
            Bn254Scalar::zero(),
        ];
        let expected = direct_coset_evaluations(&coefficients, log_size, Bn254Scalar::one());
        let twiddles = stage_twiddles_scalars(log_size).expect("twiddles");
        let mut columns = vec![coefficients];

        cpu_fft(&mut columns, log_size, &twiddles);

        assert_eq!(columns[0], expected);
    }

    #[test]
    fn cpu_lde_matches_direct_coset_evaluation() {
        let trace_log = 2;
        let blowup_log = 1;
        let eval_log = trace_log + blowup_log;
        let coset = Bn254Scalar::from(5u64);
        let coefficients = vec![
            Bn254Scalar::zero(),
            Bn254Scalar::one(),
            Bn254Scalar::zero(),
            Bn254Scalar::zero(),
        ];
        let expected = direct_coset_evaluations(&coefficients, eval_log, coset);
        let twiddles = stage_twiddles_scalars(eval_log).expect("twiddles");

        let actual = cpu_lde(&[coefficients], trace_log, blowup_log, &twiddles, coset);

        assert_eq!(actual[0], expected);
    }
}
