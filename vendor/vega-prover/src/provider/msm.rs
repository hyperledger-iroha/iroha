// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT
// This file is part of the vega-prover project.
// See the LICENSE file in the project root for full license information.
// Source repository: https://github.com/Microsoft/vega-prover

//! This module provides a multi-scalar multiplication routine
//! The generic implementation is adapted from halo2; we add an optimization to commit to bits more efficiently
//! The specialized implementations are adapted from jolt, with additional optimizations and parallelization.
use crate::{
    errors::VegaError,
    provider::traits::{DlogGroup, DlogGroupExt},
    start_span,
    traits::Engine,
};
use ff::{Field, PrimeField};
use halo2curves::CurveExt;
use halo2curves::{CurveAffine, group::Group};
use num_integer::Integer;
use num_traits::{ToPrimitive, Zero};
use rayon::{current_num_threads, prelude::*};
use tracing::info;

#[derive(Clone, Copy)]
enum Bucket<C: CurveAffine> {
    None,
    Affine(C),
    Projective(C::Curve),
}

impl<C: CurveAffine> Bucket<C> {
    fn add_assign(&mut self, other: &C) {
        *self = match *self {
            Bucket::None => Bucket::Affine(*other),
            // Vartime mixed addition (7M+3S vs 11M complete) for bucket accumulation.
            // Safe because generators are never identity and distinct generators have different x-coords.
            Bucket::Affine(a) => Bucket::Projective(a.to_curve().add_mixed_vartime(other)),
            Bucket::Projective(a) => Bucket::Projective(a.add_mixed_vartime(other)),
        }
    }

    fn add(self, other: C::Curve) -> C::Curve {
        match self {
            Bucket::None => other,
            Bucket::Affine(a) => other.add_mixed_vartime(&a),
            Bucket::Projective(a) => other + a,
        }
    }

    fn add_ref(&self, other: C::Curve) -> C::Curve {
        match *self {
            Bucket::None => other,
            Bucket::Affine(a) => other.add_mixed_vartime(&a),
            Bucket::Projective(a) => other + a,
        }
    }
}

fn cpu_msm_serial<C: CurveAffine>(coeffs: &[C::Scalar], bases: &[C]) -> C::Curve {
    let c = if bases.len() < 4 {
        1
    } else if bases.len() < 32 {
        3
    } else {
        (f64::from(bases.len() as u32)).ln().ceil() as usize
    };

    fn get_at(segment: usize, c: usize, bytes: &[u8; 32]) -> usize {
        let skip_bits = segment * c;
        let skip_bytes = skip_bits / 8;

        if skip_bytes >= 32 {
            return 0;
        }

        let mut v = [0; 8];
        for (v, o) in v.iter_mut().zip(bytes[skip_bytes..].iter()) {
            *v = *o;
        }

        let mut tmp = u64::from_le_bytes(v);
        tmp >>= skip_bits - (skip_bytes * 8);
        tmp %= 1 << c;

        tmp as usize
    }

    // Separate boolean scalars, precompute representations once
    let mut boolean_sum = C::Curve::identity();
    let mut reprs: Vec<[u8; 32]> = Vec::with_capacity(coeffs.len());
    let mut nb_bases: Vec<C> = Vec::with_capacity(coeffs.len());

    for (s, b) in coeffs.iter().zip(bases) {
        if *s == C::Scalar::ONE {
            boolean_sum = boolean_sum.add_mixed_vartime(b);
        } else if *s != C::Scalar::ZERO {
            let repr = s.to_repr();
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(repr.as_ref());
            reprs.push(bytes);
            nb_bases.push(*b);
        }
    }

    if reprs.is_empty() {
        return boolean_sum;
    }

    let n = reprs.len();
    let segments = (256 / c) + 1;

    // Signed Pippenger: use signed digit decomposition to halve bucket count.
    // Digits in [-(2^(c-1)), 2^(c-1)-1] with carry propagation.
    let half = 1usize << (c - 1);
    let full = 1usize << c;
    let num_buckets = half;

    // Precompute signed digits low-to-high (carry flows upward)
    let max_segments = segments + 1;
    let mut signed_digits = vec![0i16; max_segments * n];
    let mut carry = vec![0u8; n];

    for seg in 0..segments {
        let offset = seg * n;
        for j in 0..n {
            let raw = get_at(seg, c, &reprs[j]) + carry[j] as usize;
            carry[j] = 0;
            if raw >= half {
                signed_digits[offset + j] = -((full - raw) as i16);
                carry[j] = 1;
            } else {
                signed_digits[offset + j] = raw as i16;
            }
        }
    }

    let total_segments = if carry.iter().any(|&c| c != 0) {
        let offset = segments * n;
        for j in 0..n {
            signed_digits[offset + j] = carry[j] as i16;
        }
        segments + 1
    } else {
        segments
    };

    // Process segments high-to-low with inline Horner evaluation
    let mut buckets = vec![Bucket::<C>::None; num_buckets];
    let mut acc = C::Curve::identity();

    for segment in (0..total_segments).rev() {
        for _ in 0..c {
            acc = acc.double();
        }
        for b in buckets.iter_mut() {
            *b = Bucket::None;
        }

        let offset = segment * n;
        for j in 0..n {
            let digit = signed_digits[offset + j];
            if digit > 0 {
                buckets[digit as usize - 1].add_assign(&nb_bases[j]);
            } else if digit < 0 {
                buckets[(-digit) as usize - 1].add_assign(&(-nb_bases[j]));
            }
        }

        // Summation by parts
        let mut running_sum = C::Curve::identity();
        for exp in buckets.iter().rev() {
            running_sum = exp.add_ref(running_sum);
            acc += &running_sum;
        }
    }

    boolean_sum + acc
}

/// Performs a multi-scalar-multiplication operation without GPU acceleration.
///
/// This will use multithreading if beneficial.
/// Adapted from zcash/halo2
///
/// # Errors
/// Returns `VegaError::InvalidInputLength` if coeffs and bases have different lengths.
pub fn msm<C: CurveAffine>(
    coeffs: &[C::Scalar],
    bases: &[C],
    use_parallelism_internally: bool,
) -> Result<C::Curve, VegaError> {
    let (_msm_span, msm_t) = start_span!("msm", size = coeffs.len());

    if coeffs.len() != bases.len() {
        return Err(VegaError::InvalidInputLength {
            reason: "MSM: Coefficients and bases must have the same length".to_string(),
        });
    }

    let num_threads = if use_parallelism_internally && coeffs.len() >= 1024 {
        // Large inputs benefit from parallel chunk-based MSM
        current_num_threads()
    } else {
        1
    };

    let result = if coeffs.len() > num_threads {
        let chunk = coeffs.len() / num_threads;
        coeffs
            .par_chunks(chunk)
            .zip(bases.par_chunks(chunk))
            .map(|(coeffs, bases)| cpu_msm_serial(coeffs, bases))
            .reduce(C::Curve::identity, |sum, evl| sum + evl)
    } else {
        cpu_msm_serial(coeffs, bases)
    };

    if msm_t.elapsed().as_millis() > 10 {
        info!(elapsed_ms = %msm_t.elapsed().as_millis(), size = coeffs.len(), "msm");
    }
    Ok(result)
}

/// Shared-weight multi-MSM: compute multiple MSMs that all use the same scalar weights
/// but different base point sets. Precomputes scalar decomposition once.
/// `bases_rows[i]` is the i-th row of base points (length = num_weights).
/// Returns one result per row.
pub fn msm_shared_weights<C: CurveAffine>(
    weights: &[C::Scalar],
    bases_rows: &[&[C]],
) -> Result<Vec<C::Curve>, VegaError> {
    let n = weights.len();
    if n == 0 || bases_rows.is_empty() {
        return Ok(vec![C::Curve::identity(); bases_rows.len()]);
    }

    let c = if n < 4 {
        1
    } else if n < 32 {
        3
    } else {
        (f64::from(n as u32)).ln().ceil() as usize
    };

    fn get_at(segment: usize, c: usize, bytes: &[u8; 32]) -> usize {
        let skip_bits = segment * c;
        let skip_bytes = skip_bits / 8;
        if skip_bytes >= 32 {
            return 0;
        }
        let mut v = [0; 8];
        for (v, o) in v.iter_mut().zip(bytes[skip_bytes..].iter()) {
            *v = *o;
        }
        let mut tmp = u64::from_le_bytes(v);
        tmp >>= skip_bits - (skip_bytes * 8);
        tmp %= 1 << c;
        tmp as usize
    }

    // Precompute scalar classification: separate boolean (=1) from general, compute repr once
    let mut boolean_indices: Vec<usize> = Vec::new();
    let mut general_indices: Vec<usize> = Vec::new();
    let mut reprs: Vec<[u8; 32]> = Vec::new();

    for (i, s) in weights.iter().enumerate() {
        if *s == C::Scalar::ONE {
            boolean_indices.push(i);
        } else if *s != C::Scalar::ZERO {
            general_indices.push(i);
            let repr = s.to_repr();
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(repr.as_ref());
            reprs.push(bytes);
        }
    }

    let ng = general_indices.len();
    let segments = (256 / c) + 1;

    // Signed Pippenger: precompute signed digit decomposition once for all rows
    let half = 1usize << (c - 1);
    let full = 1usize << c;
    let num_buckets = half;

    let mut signed_windows: Vec<Vec<i16>> = Vec::with_capacity(segments + 1);
    let mut carry = vec![0u8; ng];

    for seg in 0..segments {
        let mut seg_digits = Vec::with_capacity(ng);
        for (j, repr) in reprs.iter().enumerate() {
            let raw = get_at(seg, c, repr) + carry[j] as usize;
            carry[j] = 0;
            if raw >= half {
                seg_digits.push(-((full - raw) as i16));
                carry[j] = 1;
            } else {
                seg_digits.push(raw as i16);
            }
        }
        signed_windows.push(seg_digits);
    }

    let total_segments = if carry.iter().any(|&c| c != 0) {
        signed_windows.push(carry.iter().map(|&c| c as i16).collect());
        segments + 1
    } else {
        segments
    };

    // Process each row using the precomputed signed digit decomposition
    let results: Vec<C::Curve> = bases_rows
        .par_iter()
        .map(|bases| {
            // Sum boolean scalars
            let mut boolean_sum = C::Curve::identity();
            for &idx in &boolean_indices {
                boolean_sum = boolean_sum.add_mixed_vartime(&bases[idx]);
            }

            if general_indices.is_empty() {
                return boolean_sum;
            }

            let mut buckets = vec![Bucket::<C>::None; num_buckets];
            let mut acc = C::Curve::identity();

            for segment in (0..total_segments).rev() {
                for _ in 0..c {
                    acc = acc.double();
                }
                for b in buckets.iter_mut() {
                    *b = Bucket::None;
                }
                let seg_digits = &signed_windows[segment];
                for j in 0..ng {
                    let digit = seg_digits[j];
                    if digit > 0 {
                        buckets[digit as usize - 1].add_assign(&bases[general_indices[j]]);
                    } else if digit < 0 {
                        buckets[(-digit) as usize - 1].add_assign(&(-bases[general_indices[j]]));
                    }
                }
                let mut running_sum = C::Curve::identity();
                for exp in buckets.iter().rev() {
                    running_sum = exp.add_ref(running_sum);
                    acc += &running_sum;
                }
            }

            boolean_sum + acc
        })
        .collect();

    Ok(results)
}

fn num_bits(n: usize) -> usize {
    if n == 0 { 0 } else { (n.ilog2() + 1) as usize }
}

/// Multi-scalar multiplication using the best algorithm for the given scalars.
///
/// # Errors
/// Returns `VegaError::InvalidInputLength` if bases and scalars have different lengths.
/// Returns `VegaError::InternalError` if scalars contain values that cannot be processed.
pub fn msm_small<C: CurveAffine, T: Integer + Into<u64> + Copy + Sync + ToPrimitive>(
    scalars: &[T],
    bases: &[C],
    use_parallelism_internally: bool,
) -> Result<C::Curve, VegaError> {
    let (_msm_small_span, msm_small_t) = start_span!("msm_small", size = scalars.len());

    if bases.len() != scalars.len() {
        return Err(VegaError::InvalidInputLength {
            reason: "MSM Small: Coefficients and bases must have the same length".to_string(),
        });
    }

    let max_scalar = scalars.iter().max().ok_or(VegaError::InternalError {
        reason: "Unable to find maximum value".to_string(),
    })?;
    let max_scalar_usize = max_scalar.to_usize().ok_or(VegaError::InternalError {
        reason: "Unable to convert maximum value to usize".to_string(),
    })?;
    let max_num_bits = num_bits(max_scalar_usize);
    let result = match max_num_bits {
        0 => C::identity().into(),
        1 => {
            let (_binary_span, binary_t) = start_span!("msm_binary");
            let result = msm_binary(scalars, bases, use_parallelism_internally);
            if binary_t.elapsed().as_millis() != 0 {
                info!(elapsed_ms = %binary_t.elapsed().as_millis(), size = scalars.len(), "msm_binary");
            }
            result
        }
        2..=10 => {
            let (_msm_10_span, msm_10_t) = start_span!("msm_10", max_bits = max_num_bits);
            let result = msm_10(scalars, bases, max_num_bits, use_parallelism_internally);
            info!(elapsed_ms = %msm_10_t.elapsed().as_millis(), max_bits = max_num_bits, "msm_10");
            result
        }
        _ => {
            let (_msm_rest_span, msm_rest_t) =
                start_span!("msm_small_rest", max_bits = max_num_bits);
            let result = msm_small_rest(scalars, bases, max_num_bits, use_parallelism_internally);
            info!(elapsed_ms = %msm_rest_t.elapsed().as_millis(), max_bits = max_num_bits, "msm_small_rest");
            result
        }
    };

    if msm_small_t.elapsed().as_millis() != 0 {
        info!(elapsed_ms = %msm_small_t.elapsed().as_millis(), size = scalars.len(), max_bits = max_num_bits, "msm_small");
    }
    Ok(result)
}

#[inline(always)]
fn msm_binary<C: CurveAffine, T: Integer + Sync>(
    scalars: &[T],
    bases: &[C],
    use_parallelism_internally: bool,
) -> C::Curve {
    assert_eq!(scalars.len(), bases.len());
    let num_threads = if use_parallelism_internally {
        current_num_threads()
    } else {
        1
    };
    let process_chunk = |scalars: &[T], bases: &[C]| {
        let mut acc = C::Curve::identity();
        scalars
            .iter()
            .zip(bases.iter())
            .filter(|(scalar, _)| !scalar.is_zero())
            .for_each(|(_, base)| {
                acc += *base;
            });
        acc
    };

    if scalars.len() > num_threads {
        let chunk = scalars.len() / num_threads;
        scalars
            .par_chunks(chunk)
            .zip(bases.par_chunks(chunk))
            .map(|(scalars, bases)| process_chunk(scalars, bases))
            .reduce(C::Curve::identity, |sum, evl| sum + evl)
    } else {
        process_chunk(scalars, bases)
    }
}

/// MSM optimized for up to 10-bit scalars
#[inline(always)]
fn msm_10<C: CurveAffine, T: Into<u64> + Zero + Copy + Sync>(
    scalars: &[T],
    bases: &[C],
    max_num_bits: usize,
    use_parallelism_internally: bool,
) -> C::Curve {
    fn msm_10_serial<C: CurveAffine, T: Into<u64> + Zero + Copy>(
        scalars: &[T],
        bases: &[C],
        max_num_bits: usize,
    ) -> C::Curve {
        let num_buckets: usize = 1 << max_num_bits;
        let mut buckets = vec![Bucket::None; num_buckets];

        scalars
            .iter()
            .zip(bases.iter())
            .filter(|(scalar, _base)| !scalar.is_zero())
            .for_each(|(scalar, base)| {
                let bucket_index: u64 = (*scalar).into();
                buckets[bucket_index as usize].add_assign(base);
            });

        let mut result = C::Curve::identity();
        let mut running_sum = C::Curve::identity();
        buckets.iter().skip(1).rev().for_each(|exp| {
            running_sum = exp.add(running_sum);
            result += &running_sum;
        });
        result
    }

    let num_threads = if use_parallelism_internally {
        current_num_threads()
    } else {
        1
    };
    if scalars.len() > num_threads {
        let chunk_size = scalars.len() / num_threads;
        scalars
            .par_chunks(chunk_size)
            .zip(bases.par_chunks(chunk_size))
            .map(|(scalars_chunk, bases_chunk)| {
                msm_10_serial(scalars_chunk, bases_chunk, max_num_bits)
            })
            .reduce(C::Curve::identity, |sum, evl| sum + evl)
    } else {
        msm_10_serial(scalars, bases, max_num_bits)
    }
}

#[inline(always)]
fn msm_small_rest<C: CurveAffine, T: Into<u64> + Zero + Copy + Sync>(
    scalars: &[T],
    bases: &[C],
    max_num_bits: usize,
    use_parallelism_internally: bool,
) -> C::Curve {
    fn msm_small_rest_serial<C: CurveAffine, T: Into<u64> + Zero + Copy>(
        scalars: &[T],
        bases: &[C],
        max_num_bits: usize,
    ) -> C::Curve {
        let c = if bases.len() < 32 {
            3
        } else {
            compute_ln(bases.len()) + 2
        };

        let zero = C::Curve::identity();

        let scalars_and_bases_iter = scalars.iter().zip(bases).filter(|(s, _base)| !s.is_zero());
        let window_starts = (0..max_num_bits).step_by(c);

        // Each window is of size `c`.
        // We divide up the bits 0..num_bits into windows of size `c`, and
        // in parallel process each such window.
        let window_sums: Vec<_> = window_starts
            .map(|w_start| {
                let mut res = zero;
                // We don't need the "zero" bucket, so we only have 2^c - 1 buckets.
                let mut buckets = vec![zero; (1 << c) - 1];
                // This clone is cheap, because the iterator contains just a
                // pointer and an index into the original vectors.
                scalars_and_bases_iter.clone().for_each(|(&scalar, base)| {
                    let scalar: u64 = scalar.into();
                    if scalar == 1 {
                        // We only process unit scalars once in the first window.
                        if w_start == 0 {
                            res += base;
                        }
                    } else {
                        let mut scalar = scalar;

                        // We right-shift by w_start, thus getting rid of the
                        // lower bits.
                        scalar >>= w_start;

                        // We mod the remaining bits by 2^{window size}, thus taking `c` bits.
                        scalar %= 1 << c;

                        // If the scalar is non-zero, we update the corresponding
                        // bucket.
                        // (Recall that `buckets` doesn't have a zero bucket.)
                        if scalar != 0 {
                            buckets[(scalar - 1) as usize] += base;
                        }
                    }
                });

                // Compute sum_{i in 0..num_buckets} (sum_{j in i..num_buckets} bucket[j])
                // This is computed below for b buckets, using 2b curve additions.
                //
                // We could first normalize `buckets` and then use mixed-addition
                // here, but that's slower for the kinds of groups we care about
                // (Short Weierstrass curves and Twisted Edwards curves).
                // In the case of Short Weierstrass curves,
                // mixed addition saves ~4 field multiplications per addition.
                // However normalization (with the inversion batched) takes ~6
                // field multiplications per element,
                // hence batch normalization is a slowdown.

                // `running_sum` = sum_{j in i..num_buckets} bucket[j],
                // where we iterate backward from i = num_buckets to 0.
                let mut running_sum = C::Curve::identity();
                buckets.into_iter().rev().for_each(|b| {
                    running_sum += &b;
                    res += &running_sum;
                });
                res
            })
            .collect();

        // We store the sum for the lowest window.
        let lowest = window_sums.first().copied().unwrap_or(zero);

        // We're traversing windows from high to low.
        lowest
            + window_sums[1..]
                .iter()
                .rev()
                .fold(zero, |mut total, sum_i| {
                    total += sum_i;
                    for _ in 0..c {
                        total = total.double();
                    }
                    total
                })
    }

    let num_threads = if use_parallelism_internally {
        current_num_threads()
    } else {
        1
    };
    if scalars.len() > num_threads {
        let chunk_size = scalars.len() / num_threads;
        scalars
            .par_chunks(chunk_size)
            .zip(bases.par_chunks(chunk_size))
            .map(|(scalars_chunk, bases_chunk)| {
                msm_small_rest_serial(scalars_chunk, bases_chunk, max_num_bits)
            })
            .reduce(C::Curve::identity, |sum, evl| sum + evl)
    } else {
        msm_small_rest_serial(scalars, bases, max_num_bits)
    }
}

#[inline(always)]
fn compute_ln(a: usize) -> usize {
    // log2(a) * ln(2)
    if a == 0 {
        0 // Handle edge case where log2 is undefined
    } else {
        a.ilog2() as usize * 69 / 100
    }
}

pub(crate) type AffineGroupElement<E> = <<E as Engine>::GE as DlogGroup>::AffineGroupElement;

/// Precomputed table for fast fixed-base scalar multiplication of a single point.
/// Uses windowed method: precomputes multiples [1*P, 2*P, ..., (2^w-1)*P] for each window.
#[derive(Clone, Debug)]
pub struct FixedBaseMul<E: Engine>
where
    E::GE: DlogGroup,
{
    /// tables[j][d-1] = d * (2^(j*w)) * P for d in 1..2^w, j in 0..num_windows
    tables: Vec<Vec<AffineGroupElement<E>>>,
    window_bits: usize,
}

impl<E: Engine> FixedBaseMul<E>
where
    E::GE: DlogGroupExt,
{
    /// Precompute window table for point P using batch affine conversion.
    pub fn precompute(p: &E::GE, window_bits: usize) -> Self {
        let num_windows = 256_usize.div_ceil(window_bits);
        let entries_per_window = (1usize << window_bits) - 1;

        // Collect all projective points, then batch-convert to affine (single field inversion)
        let total_entries = num_windows * entries_per_window;
        let mut all_proj = Vec::with_capacity(total_entries);

        let mut base = *p; // base = 2^(j*w) * P
        for _ in 0..num_windows {
            let mut acc = base;
            all_proj.push(acc); // 1 * base
            for _ in 1..entries_per_window {
                acc += base;
                all_proj.push(acc); // d * base
            }
            // Advance base by 2^w
            for _ in 0..window_bits {
                base = base + base;
            }
        }

        let all_affine = E::GE::batch_affine(&all_proj);

        // Split flat affine array into per-window tables
        let mut tables = Vec::with_capacity(num_windows);
        for w in 0..num_windows {
            let start = w * entries_per_window;
            let end = start + entries_per_window;
            tables.push(all_affine[start..end].to_vec());
        }

        Self {
            tables,
            window_bits,
        }
    }

    /// Variable-time scalar multiplication using the precomputed table.
    #[inline(always)]
    pub fn mul(&self, scalar: &E::Scalar) -> E::GE {
        let repr = scalar.to_repr();
        let bytes = repr.as_ref();
        let w = self.window_bits;
        let mask = (1u64 << w) - 1;
        let mut acc = E::GE::zero();

        for (j, table) in self.tables.iter().enumerate() {
            let bit_offset = j * w;
            let byte_idx = bit_offset / 8;
            let bit_idx = bit_offset % 8;

            if byte_idx >= bytes.len() {
                break;
            }

            // Extract w bits starting at bit_offset
            let mut val = bytes[byte_idx] as u64 >> bit_idx;
            if bit_idx + w > 8 && byte_idx + 1 < bytes.len() {
                val |= (bytes[byte_idx + 1] as u64) << (8 - bit_idx);
            }
            if bit_idx + w > 16 && byte_idx + 2 < bytes.len() {
                val |= (bytes[byte_idx + 2] as u64) << (16 - bit_idx);
            }
            let digit = (val & mask) as usize;

            if digit != 0 {
                acc = acc.add_affine_vartime(&table[digit - 1]);
            }
        }

        acc
    }

    /// Multi-scalar multiplication: sum tables[i].mul(scalars[i])
    /// Uses a single accumulator to avoid intermediate projective additions.
    #[inline(always)]
    pub fn multi_mul(tables: &[Self], scalars: &[E::Scalar]) -> E::GE {
        debug_assert_eq!(tables.len(), scalars.len());
        let w = if tables.is_empty() {
            8
        } else {
            debug_assert!(
                tables
                    .iter()
                    .all(|t| t.window_bits == tables[0].window_bits),
                "multi_mul: all tables must share the same window_bits"
            );
            tables[0].window_bits
        };
        let mask = (1u64 << w) - 1;

        // Pre-convert all scalars to bytes
        let reprs: Vec<_> = scalars.iter().map(|s| s.to_repr()).collect();

        let mut acc = E::GE::zero();
        for (repr, table) in reprs.iter().zip(tables.iter()) {
            let bytes = repr.as_ref();
            for (j, table_j) in table.tables.iter().enumerate() {
                let bit_offset = j * w;
                let byte_idx = bit_offset / 8;
                let bit_idx = bit_offset % 8;

                if byte_idx >= bytes.len() {
                    break;
                }

                let mut val = bytes[byte_idx] as u64 >> bit_idx;
                if bit_idx + w > 8 && byte_idx + 1 < bytes.len() {
                    val |= (bytes[byte_idx + 1] as u64) << (8 - bit_idx);
                }
                if bit_idx + w > 16 && byte_idx + 2 < bytes.len() {
                    val |= (bytes[byte_idx + 2] as u64) << (16 - bit_idx);
                }
                let digit = (val & mask) as usize;

                if digit != 0 {
                    acc = acc.add_affine_vartime(&table_j[digit - 1]);
                }
            }
        }
        acc
    }
}

/// Variable-time wNAF-5 scalar multiplication (width-5 non-adjacent form).
/// ~40% faster than the group's default constant-time scalar mul for 256-bit scalars.
#[inline(always)]
pub(crate) fn vartime_scalar_mul<E: Engine>(base: E::GE, scalar: &E::Scalar) -> E::GE
where
    E::GE: DlogGroup,
{
    const W: usize = 5;
    const TABLE_SIZE: usize = 1 << (W - 1); // 16 entries

    // Build table of odd multiples: [P, 3P, 5P, 7P, ..., 31P]
    let double = base + base;
    let mut table = [E::GE::zero(); TABLE_SIZE];
    table[0] = base;
    for i in 1..TABLE_SIZE {
        table[i] = table[i - 1] + double;
    }

    // Convert scalar to wNAF-5 form
    let repr = scalar.to_repr();
    let bytes = repr.as_ref();
    let mut wnaf = [0i8; 257]; // wNAF digits (at most 257 for 256-bit scalar)
    let mut wnaf_len = 0;

    // Convert to a working big-integer (u64 limbs)
    let mut limbs = [0u64; 4];
    for (i, chunk) in bytes.chunks(8).enumerate() {
        if i < 4 {
            let mut buf = [0u8; 8];
            buf[..chunk.len()].copy_from_slice(chunk);
            limbs[i] = u64::from_le_bytes(buf);
        }
    }

    // Generate wNAF digits
    let half = 1i16 << W; // 32
    let mask = half - 1; // 31
    while limbs[0] != 0 || limbs[1] != 0 || limbs[2] != 0 || limbs[3] != 0 {
        if limbs[0] & 1 == 1 {
            // Odd: extract a signed digit
            let digit = (limbs[0] & mask as u64) as i16;
            let signed = if digit >= half / 2 {
                // Borrow from higher bits
                let d = digit - half;
                // Subtract d (which is negative, so add |d|)
                let borrow = (-(d as i64)) as u64;
                let (v, carry) = limbs[0].overflowing_add(borrow);
                limbs[0] = v;
                if carry {
                    for limb in limbs.iter_mut().skip(1) {
                        let (v2, c2) = limb.overflowing_add(1);
                        *limb = v2;
                        if !c2 {
                            break;
                        }
                    }
                }
                d as i8
            } else {
                limbs[0] -= digit as u64;
                digit as i8
            };
            wnaf[wnaf_len] = signed;
        } else {
            wnaf[wnaf_len] = 0;
        }
        wnaf_len += 1;
        // Right shift by 1
        for i in 0..3 {
            limbs[i] = (limbs[i] >> 1) | (limbs[i + 1] << 63);
        }
        limbs[3] >>= 1;
    }

    // Process wNAF from most significant digit
    let mut acc = E::GE::zero();
    let mut started = false;
    for i in (0..wnaf_len).rev() {
        if started {
            acc = acc + acc;
        }
        let d = wnaf[i];
        if d > 0 {
            started = true;
            acc += table[(d as usize - 1) / 2];
        } else if d < 0 {
            started = true;
            acc -= table[((-d) as usize - 1) / 2];
        }
    }
    acc
}

#[cfg(test)]
mod tests {
    use ff::Field;
    use halo2curves::{CurveAffine, group::Group};
    use rand_core::OsRng;

    use super::*;
    use crate::provider::pasta::{pallas, vesta};

    fn test_general_msm_with<F: Field, A: CurveAffine<ScalarExt = F>>() {
        let n = 8;
        let coeffs = (0..n).map(|_| F::random(OsRng)).collect::<Vec<_>>();
        let bases = (0..n)
            .map(|_| A::from(A::generator() * F::random(OsRng)))
            .collect::<Vec<_>>();

        assert_eq!(coeffs.len(), bases.len());
        let naive = coeffs
            .iter()
            .zip(bases.iter())
            .fold(A::CurveExt::identity(), |acc, (coeff, base)| {
                acc + *base * coeff
            });
        let msm = msm(&coeffs, &bases, true);

        assert_eq!(naive, msm.unwrap())
    }

    #[test]
    fn test_general_msm() {
        test_general_msm_with::<pallas::Scalar, pallas::Affine>();
        test_general_msm_with::<vesta::Scalar, vesta::Affine>();
    }

    fn test_msm_ux_with<F: PrimeField, A: CurveAffine<ScalarExt = F>>() {
        let n = 8;
        let bases = (0..n)
            .map(|_| A::from(A::generator() * F::random(OsRng)))
            .collect::<Vec<_>>();

        for bit_width in [1, 4, 8, 10, 16, 20, 32, 40, 64] {
            println!("bit_width: {bit_width}");
            assert!(bit_width <= 64); // Ensure we don't overflow F::from
            let coeffs: Vec<u64> = (0..n)
                .map(|_| {
                    let r = rand::random::<u64>();
                    if bit_width == 64 {
                        r
                    } else {
                        r % (1 << bit_width)
                    }
                })
                .collect::<Vec<_>>();
            let coeffs_scalar: Vec<F> = coeffs.iter().map(|b| F::from(*b)).collect::<Vec<_>>();
            let general = msm(&coeffs_scalar, &bases, true);
            let integer = msm_small(&coeffs, &bases, true);

            assert_eq!(general.unwrap(), integer.unwrap());
        }
    }

    #[test]
    fn test_msm_ux() {
        test_msm_ux_with::<pallas::Scalar, pallas::Affine>();
        test_msm_ux_with::<vesta::Scalar, vesta::Affine>();
    }
}
