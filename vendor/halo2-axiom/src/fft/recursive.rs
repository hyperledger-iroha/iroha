//! This contains the recursive FFT.

use crate::{
    arithmetic::{self, parallelize, FftGroup},
    multicore,
};
use std::{
    any::{Any, TypeId},
    cell::RefCell,
    collections::HashMap,
};

pub use ff::Field;
pub use halo2curves::{CurveAffine, CurveExt};

thread_local! {
    static FFT_SCRATCH_POOL: RefCell<HashMap<TypeId, Box<dyn Any>>> =
        RefCell::new(HashMap::new());
}

fn take_scratch<G: Copy + 'static>(len: usize, filler: G) -> Vec<G> {
    if len == 0 {
        return Vec::new();
    }
    FFT_SCRATCH_POOL.with(|pool| {
        let mut pool = pool.borrow_mut();
        let type_id = TypeId::of::<G>();
        let mut scratch = pool
            .remove(&type_id)
            .and_then(|boxed| boxed.downcast::<Vec<G>>().ok())
            .map(|boxed_vec| *boxed_vec)
            .unwrap_or_default();
        scratch.resize(len, filler);
        scratch
    })
}

fn release_scratch<G: 'static>(mut scratch: Vec<G>) {
    FFT_SCRATCH_POOL.with(|pool| {
        scratch.clear();
        pool.borrow_mut()
            .insert(TypeId::of::<G>(), Box::new(scratch));
    });
}

/// FFTStage
#[derive(Clone, Debug)]
pub struct FFTStage {
    radix: usize,
    length: usize,
}

/// FFT stages
fn get_stages(size: usize, radixes: Vec<usize>) -> Vec<FFTStage> {
    let mut stages: Vec<FFTStage> = vec![];

    let mut n = size;

    // Use the specified radices
    for &radix in &radixes {
        n /= radix;
        stages.push(FFTStage { radix, length: n });
    }

    // Fill in the rest of the tree if needed
    let mut p = 2;
    while n > 1 {
        while n % p != 0 {
            if p == 4 {
                p = 2;
            }
        }
        n /= p;
        stages.push(FFTStage {
            radix: p,
            length: n,
        });
    }

    /*for i in 0..stages.len() {
        log_info(format!("Stage {}: {}, {}", i, stages[i].radix, stages[i].length));
    }*/

    stages
}

/// FFTData
#[derive(Clone, Debug)]
pub struct FFTData<F: arithmetic::Field> {
    n: usize,

    stages: Vec<FFTStage>,

    f_twiddles: Vec<Vec<F>>,
    inv_twiddles: Vec<Vec<F>>,
    //scratch: Vec<F>,
}

impl<F: arithmetic::Field> Default for FFTData<F> {
    fn default() -> Self {
        Self {
            n: Default::default(),
            stages: Default::default(),
            f_twiddles: Default::default(),
            inv_twiddles: Default::default(),
        }
    }
}

impl<F: arithmetic::Field> FFTData<F> {
    /// Create FFT data
    pub fn new(n: usize, omega: F, omega_inv: F) -> Self {
        let stages = get_stages(n, vec![]);
        let mut f_twiddles = vec![];
        let mut inv_twiddles = vec![];
        let mut scratch = vec![F::ZERO; n];

        // Generate stage twiddles
        for inv in 0..2 {
            let inverse = inv == 0;
            let o = if inverse { omega_inv } else { omega };
            let stage_twiddles = if inverse {
                &mut inv_twiddles
            } else {
                &mut f_twiddles
            };

            let twiddles = &mut scratch;

            // Twiddles
            parallelize(twiddles, |twiddles, start| {
                let w_m = o;
                let mut w = o.pow_vartime([start as u64]);
                for value in twiddles.iter_mut() {
                    *value = w;
                    w *= w_m;
                }
            });

            // Re-order twiddles for cache friendliness
            let num_stages = stages.len();
            stage_twiddles.resize(num_stages, vec![]);
            for l in 0..num_stages {
                let radix = stages[l].radix;
                let stage_length = stages[l].length;

                let num_twiddles = stage_length * (radix - 1);
                stage_twiddles[l].resize(num_twiddles + 1, F::ZERO);

                // Set j
                stage_twiddles[l][num_twiddles] = twiddles[(twiddles.len() * 3) / 4];

                let stride = n / (stage_length * radix);
                let mut tws = vec![0usize; radix - 1];
                for i in 0..stage_length {
                    for j in 0..radix - 1 {
                        stage_twiddles[l][i * (radix - 1) + j] = twiddles[tws[j]];
                        tws[j] += (j + 1) * stride;
                    }
                }
            }
        }

        Self {
            n,
            stages,
            f_twiddles,
            inv_twiddles,
            //scratch,
        }
    }

    /// Return private field `n`
    pub fn get_n(&self) -> usize {
        self.n
    }
}

/// Radix 2 butterfly
fn butterfly_2<Scalar: Field, G: FftGroup<Scalar>>(
    out: &mut [G],
    twiddles: &[Scalar],
    stage_length: usize,
) {
    let mut out_offset = 0;
    let mut out_offset2 = stage_length;

    let t = out[out_offset2];
    out[out_offset2] = out[out_offset] - &t;
    out[out_offset] += &t;
    out_offset2 += 1;
    out_offset += 1;

    for twiddle in twiddles[1..stage_length].iter() {
        let t = out[out_offset2] * twiddle;
        out[out_offset2] = out[out_offset] - &t;
        out[out_offset] += &t;
        out_offset2 += 1;
        out_offset += 1;
    }
}

/// Radix 2 butterfly
fn butterfly_2_parallel<Scalar: Field, G: FftGroup<Scalar>>(
    out: &mut [G],
    twiddles: &[Scalar],
    _stage_length: usize,
    num_threads: usize,
) {
    let n = out.len();
    let mut chunk = n / num_threads;
    if chunk < num_threads {
        chunk = n;
    }

    multicore::scope(|scope| {
        let (part_a, part_b) = out.split_at_mut(n / 2);
        for (i, (part0, part1)) in part_a
            .chunks_mut(chunk)
            .zip(part_b.chunks_mut(chunk))
            .enumerate()
        {
            scope.spawn(move |_| {
                let offset = i * chunk;
                for k in 0..part0.len() {
                    let t = part1[k] * &twiddles[offset + k];
                    part1[k] = part0[k] - &t;
                    part0[k] += &t;
                }
            });
        }
    });
}

/// Radix 4 butterfly
fn butterfly_4<Scalar: Field, G: FftGroup<Scalar>>(
    out: &mut [G],
    twiddles: &[Scalar],
    stage_length: usize,
) {
    let j = twiddles[twiddles.len() - 1];
    let mut tw = 0;

    /* Case twiddle == one */
    {
        let i0 = 0;
        let i1 = stage_length;
        let i2 = stage_length * 2;
        let i3 = stage_length * 3;

        let z0 = out[i0];
        let z1 = out[i1];
        let z2 = out[i2];
        let z3 = out[i3];

        let t1 = z0 + &z2;
        let t2 = z1 + &z3;
        let t3 = z0 - &z2;
        let t4j = (z1 - &z3) * &j;

        out[i0] = t1 + &t2;
        out[i1] = t3 - &t4j;
        out[i2] = t1 - &t2;
        out[i3] = t3 + &t4j;

        tw += 3;
    }

    for k in 1..stage_length {
        let i0 = k;
        let i1 = k + stage_length;
        let i2 = k + stage_length * 2;
        let i3 = k + stage_length * 3;

        let z0 = out[i0];
        let z1 = out[i1] * &twiddles[tw];
        let z2 = out[i2] * &twiddles[tw + 1];
        let z3 = out[i3] * &twiddles[tw + 2];

        let t1 = z0 + &z2;
        let t2 = z1 + &z3;
        let t3 = z0 - &z2;
        let t4j = (z1 - &z3) * &j;

        out[i0] = t1 + &t2;
        out[i1] = t3 - &t4j;
        out[i2] = t1 - &t2;
        out[i3] = t3 + &t4j;

        tw += 3;
    }
}

/// Radix 4 butterfly
fn butterfly_4_parallel<Scalar: Field, G: FftGroup<Scalar>>(
    out: &mut [G],
    twiddles: &[Scalar],
    _stage_length: usize,
    num_threads: usize,
) {
    let j = twiddles[twiddles.len() - 1];

    let n = out.len();
    let mut chunk = n / num_threads;
    if chunk < num_threads {
        chunk = n;
    }
    multicore::scope(|scope| {
        //let mut parts: Vec<&mut [F]> = out.chunks_mut(4).collect();
        //out.chunks_mut(4).map(|c| c.chunks_mut(chunk)).fold(predicate)
        let (part_a, part_b) = out.split_at_mut(n / 2);
        let (part_aa, part_ab) = part_a.split_at_mut(n / 4);
        let (part_ba, part_bb) = part_b.split_at_mut(n / 4);
        for (i, (((part0, part1), part2), part3)) in part_aa
            .chunks_mut(chunk)
            .zip(part_ab.chunks_mut(chunk))
            .zip(part_ba.chunks_mut(chunk))
            .zip(part_bb.chunks_mut(chunk))
            .enumerate()
        {
            scope.spawn(move |_| {
                let offset = i * chunk;
                let mut tw = offset * 3;
                for k in 0..part1.len() {
                    let z0 = part0[k];
                    let z1 = part1[k] * &twiddles[tw];
                    let z2 = part2[k] * &twiddles[tw + 1];
                    let z3 = part3[k] * &twiddles[tw + 2];

                    let t1 = z0 + &z2;
                    let t2 = z1 + &z3;
                    let t3 = z0 - &z2;
                    let t4j = (z1 - &z3) * &j;

                    part0[k] = t1 + &t2;
                    part1[k] = t3 - &t4j;
                    part2[k] = t1 - &t2;
                    part3[k] = t3 + &t4j;

                    tw += 3;
                }
            });
        }
    });
}

/// Inner recursion
#[allow(clippy::too_many_arguments)]
fn recursive_fft_inner<Scalar: Field, G: FftGroup<Scalar>>(
    data_in: &[G],
    data_out: &mut [G],
    twiddles: &Vec<Vec<Scalar>>,
    stages: &Vec<FFTStage>,
    in_offset: usize,
    stride: usize,
    level: usize,
    num_threads: usize,
) {
    let radix = stages[level].radix;
    let stage_length = stages[level].length;

    if num_threads > 1 {
        if stage_length == 1 {
            for i in 0..radix {
                data_out[i] = data_in[in_offset + i * stride];
            }
        } else {
            let num_threads_recursive = if num_threads >= radix {
                radix
            } else {
                num_threads
            };
            parallelize_count(data_out, num_threads_recursive, |data_out, i| {
                let num_threads_in_recursion = if num_threads < radix {
                    1
                } else {
                    (num_threads + i) / radix
                };
                recursive_fft_inner(
                    data_in,
                    data_out,
                    twiddles,
                    stages,
                    in_offset + i * stride,
                    stride * radix,
                    level + 1,
                    num_threads_in_recursion,
                )
            });
        }
        match radix {
            2 => butterfly_2_parallel(data_out, &twiddles[level], stage_length, num_threads),
            4 => butterfly_4_parallel(data_out, &twiddles[level], stage_length, num_threads),
            _ => unimplemented!("radix unsupported"),
        }
    } else {
        if stage_length == 1 {
            for i in 0..radix {
                data_out[i] = data_in[in_offset + i * stride];
            }
        } else {
            for i in 0..radix {
                recursive_fft_inner(
                    data_in,
                    &mut data_out[i * stage_length..(i + 1) * stage_length],
                    twiddles,
                    stages,
                    in_offset + i * stride,
                    stride * radix,
                    level + 1,
                    num_threads,
                );
            }
        }
        match radix {
            2 => butterfly_2(data_out, &twiddles[level], stage_length),
            4 => butterfly_4(data_out, &twiddles[level], stage_length),
            _ => unimplemented!("radix unsupported"),
        }
    }
}

/// Todo: Brechts impl starts here
fn recursive_fft<Scalar: Field, G: FftGroup<Scalar>>(
    data: &FFTData<Scalar>,
    data_in: &mut Vec<G>,
    inverse: bool,
) {
    if data_in.is_empty() {
        return;
    }
    let num_threads = multicore::current_num_threads();
    //let start = start_measure(format!("recursive fft {} ({})", data_in.len(), num_threads), false);
    //let start_mem = start_measure(format!("alloc"), false);
    let filler = data_in[0];
    let mut scratch = take_scratch::<G>(data_in.len(), filler);
    //stop_measure(start_mem);

    recursive_fft_inner(
        data_in,
        &mut /*data.*/scratch,
        if inverse {
            &data.inv_twiddles
        } else {
            &data.f_twiddles
        },
        &data.stages,
        0,
        1,
        0,
        num_threads,
    );
    //let duration = stop_measure(start);

    //let start = start_measure(format!("copy"), false);
    // Will simply swap the vector's buffer, no data is actually copied
    std::mem::swap(data_in, &mut /*data.*/scratch);
    release_scratch(scratch);
    //stop_measure(start);
}

/// This simple utility function will parallelize an operation that is to be
/// performed over a mutable slice.
fn parallelize_count<T: Send, F: Fn(&mut [T], usize) + Send + Sync + Clone>(
    v: &mut [T],
    num_threads: usize,
    f: F,
) {
    let n = v.len();
    let mut chunk = n / num_threads;
    if chunk < num_threads {
        chunk = n;
    }

    multicore::scope(|scope| {
        for (chunk_num, v) in v.chunks_mut(chunk).enumerate() {
            let f = f.clone();
            scope.spawn(move |_| {
                f(v, chunk_num);
            });
        }
    });
}

/// Generic adaptor
pub fn fft<Scalar: Field, G: FftGroup<Scalar>>(
    data_in: &mut [G],
    _omega: Scalar,
    _log_n: u32,
    data: &FFTData<Scalar>,
    inverse: bool,
) {
    let len = data_in.len();
    if len == 0 {
        return;
    }
    let filler = data_in[0];
    let mut buffer = take_scratch::<G>(len, filler);
    buffer.copy_from_slice(data_in);
    recursive_fft(data, &mut buffer, inverse);
    data_in.copy_from_slice(&buffer);
    release_scratch(buffer);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scratch_pool_reuses_allocation() {
        let mut first = take_scratch::<u64>(8, 1);
        assert!(first.iter().all(|&v| v == 1));
        let ptr = first.as_ptr();
        release_scratch(first);

        let second = take_scratch::<u64>(8, 2);
        assert!(second.iter().all(|&v| v == 2));
        assert_eq!(ptr, second.as_ptr());
        release_scratch(second);
    }

    #[test]
    fn scratch_pool_expands_capacity() {
        let mut small = take_scratch::<u32>(4, 7);
        let base_capacity = small.capacity();
        release_scratch(small);

        let larger_len = base_capacity + 8;
        let larger = take_scratch::<u32>(larger_len, 9);
        assert!(larger.capacity() >= larger_len);
        assert!(larger.iter().all(|&v| v == 9));
        release_scratch(larger);
    }
}
