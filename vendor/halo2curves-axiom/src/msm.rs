use std::{
    ops::Neg,
    sync::{Mutex, MutexGuard, OnceLock},
    thread,
};

use crate::CurveAffine;
use ff::Field;
use ff::PrimeField;
use group::Group;
use rayon::iter::{
    IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator,
};
use rayon::slice::ParallelSliceMut;
use rayon::{ThreadPool, ThreadPoolBuilder};

const BATCH_SIZE: usize = 64;
// Each large-MSM window owns two `2^(c - 1)` bucket tables. Running every
// window concurrently makes that working set scale with the host's core
// count, even though the final accumulator order is fixed. A process-wide pool
// of two long-lived workers retains useful parallelism while bounding both
// simultaneous bucket storage and per-thread allocator caches across every
// large MSM in the process.
const MAX_PARALLEL_WINDOW_SHARDS: usize = 2;
static LARGE_MSM_WINDOW_POOL: OnceLock<ThreadPool> = OnceLock::new();
static LARGE_MSM_ADMISSION: Mutex<()> = Mutex::new(());
// Cost model weights tuned from `benches/msm.rs` on representative x86_64 and
// aarch64 hosts. Bucket aggregation is slightly cheaper than per-scalar
// scheduling, while the doubling ladder is dominated by the other two terms.
const MAX_WINDOW: usize = 16;
const POINT_WEIGHT: f64 = 1.0;
const BUCKET_ACC_WEIGHT: f64 = 0.7;
const SCHEDULE_WEIGHT: f64 = 0.35;
const DOUBLING_WEIGHT: f64 = 0.01;

/// Pick an MSM Booth window size by minimising an empirical cost model.
///
/// The heuristic evaluates each window size in `[3, 16]` using the estimated
/// number of scalar bucket assignments, bucket additions, and doubling steps.
/// The weights were extracted from the Criterion benchmark in
/// `vendor/halo2curves-axiom/benches/msm.rs` (Intel Xeon + Apple M2 runs) and
/// capture that bucket aggregation is ~0.7× the cost of per-scalar scheduling,
/// while each staged affine batch adds another ~0.35×. The doubling ladder is
/// comparatively cheap, so it only receives a small penalty. The same heuristic
/// is shared between the serial and the parallel MSM implementations to keep
/// their scheduling decisions aligned.
fn optimal_window_size(num_points: usize, scalar_bits: usize) -> usize {
    if num_points == 0 {
        return 1;
    }
    if num_points < 4 {
        return 1;
    }
    if num_points < 32 {
        return 3;
    }

    let max_window = scalar_bits.max(3).min(MAX_WINDOW);
    if max_window < 3 {
        return 3;
    }

    let mut best_c = 3;
    let mut best_cost = f64::INFINITY;

    for c in 3..=max_window {
        let windows = (scalar_bits + c - 1) / c;
        let bucket_count = 1usize << (c - 1);
        let per_window_point_cost = POINT_WEIGHT * num_points as f64;
        let per_window_bucket_cost =
            (BUCKET_ACC_WEIGHT + SCHEDULE_WEIGHT) * bucket_count as f64;
        let doubling_cost =
            DOUBLING_WEIGHT * (c * windows * (windows - 1) / 2) as f64;
        let total_cost =
            windows as f64 * (per_window_point_cost + per_window_bucket_cost)
                + doubling_cost;
        if total_cost < best_cost {
            best_cost = total_cost;
            best_c = c;
        }
    }

    best_c
}

fn parallel_window_shard_len(number_of_windows: usize) -> usize {
    number_of_windows
        .div_ceil(MAX_PARALLEL_WINDOW_SHARDS)
        .max(1)
}

fn large_msm_window_pool() -> &'static ThreadPool {
    LARGE_MSM_WINDOW_POOL.get_or_init(|| {
        ThreadPoolBuilder::new()
            .num_threads(MAX_PARALLEL_WINDOW_SHARDS)
            .thread_name(|index| format!("halo2-msm-window-{index}"))
            .build()
            .expect("failed to construct bounded large-MSM window pool")
    })
}

fn enter_large_msm() -> MutexGuard<'static, ()> {
    // This mutex protects only admission, not mutable shared state. A panic
    // cannot leave an invariant-corrupt value behind, so poison recovery is
    // safe and avoids disabling every subsequent proof in the process.
    LARGE_MSM_ADMISSION
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn run_large_msm_admitted<R: Send>(operation: impl FnOnce() -> R + Send) -> R {
    // `ThreadPool::install` cooperatively executes work from the caller's
    // Rayon pool while it waits for a different pool. Holding the admission
    // mutex across such an install can therefore deadlock: a stolen outer
    // commitment may enter another large MSM and wait for the mutex held by
    // the suspended caller. Dispatch through a scoped OS thread so the source
    // Rayon worker performs an ordinary blocking join and cannot steal another
    // admitted MSM. The operation may still borrow its inputs because the
    // dispatcher is scoped.
    thread::scope(|scope| {
        let dispatcher = thread::Builder::new()
            .name("halo2-msm-dispatch".to_owned())
            .spawn_scoped(scope, move || {
                let _admission = enter_large_msm();
                large_msm_window_pool().install(operation)
            })
            .expect("failed to construct large-MSM dispatcher");

        match dispatcher.join() {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    })
}

fn get_booth_index(window_index: usize, window_size: usize, el: &[u8]) -> i32 {
    // Booth encoding:
    // * step by `window` size
    // * slice by size of `window + 1``
    // * each window overlap by 1 bit * append a zero bit to the least significant end
    // Indexing rule for example window size 3 where we slice by 4 bits:
    // `[0, +1, +1, +2, +2, +3, +3, +4, -4, -3, -3 -2, -2, -1, -1, 0]``
    // So we can reduce the bucket size without preprocessing scalars
    // and remembering them as in classic signed digit encoding

    let skip_bits = (window_index * window_size).saturating_sub(1);
    let skip_bytes = skip_bits / 8;

    // fill into a u32
    let mut v: [u8; 4] = [0; 4];
    for (dst, src) in v.iter_mut().zip(el.iter().skip(skip_bytes)) {
        *dst = *src
    }
    let mut tmp = u32::from_le_bytes(v);

    // pad with one 0 if slicing the least significant window
    if window_index == 0 {
        tmp <<= 1;
    }

    // remove further bits
    tmp >>= skip_bits - (skip_bytes * 8);
    // apply the booth window
    tmp &= (1 << (window_size + 1)) - 1;

    let sign = tmp & (1 << window_size) == 0;

    // div ceil by 2
    tmp = (tmp + 1) >> 1;

    // find the booth action index
    if sign {
        tmp as i32
    } else {
        ((!(tmp - 1) & ((1 << window_size) - 1)) as i32).neg()
    }
}

/// Batch addition.
fn batch_add<C: CurveAffine>(
    size: usize,
    buckets: &mut [BucketAffine<C>],
    points: &[SchedulePoint],
    bases: &[Affine<C>],
) {
    let mut t = vec![C::Base::ZERO; size]; // Stores x2 - x1
    let mut z = vec![C::Base::ZERO; size]; // Stores y2 - y1
    let mut acc = C::Base::ONE;

    for (
        (
            SchedulePoint {
                base_idx,
                buck_idx,
                sign,
            },
            t,
        ),
        z,
    ) in points.iter().zip(t.iter_mut()).zip(z.iter_mut())
    {
        if buckets[*buck_idx].is_inf() {
            // We assume bases[*base_idx] != infinity always.
            continue;
        }

        if buckets[*buck_idx].x() == bases[*base_idx].x {
            // y-coordinate matches:
            //  1. y1 == y2 and sign = false or
            //  2. y1 != y2 and sign = true
            //  => ( y1 == y2) xor !sign
            //  (This uses the fact that x1 == x2 and both points satisfy the curve eq.)
            if (buckets[*buck_idx].y() == bases[*base_idx].y) ^ !*sign {
                // Doubling
                let x_squared = bases[*base_idx].x.square();
                *z = buckets[*buck_idx].y() + buckets[*buck_idx].y(); // 2y
                *t = acc * (x_squared + x_squared + x_squared); // acc * 3x^2
                acc *= *z;
                continue;
            }
            // P + (-P)
            buckets[*buck_idx].set_inf();
            continue;
        }
        // Addition
        *z = buckets[*buck_idx].x() - bases[*base_idx].x; // x2 - x1
        if *sign {
            *t = acc * (buckets[*buck_idx].y() - bases[*base_idx].y);
        } else {
            *t = acc * (buckets[*buck_idx].y() + bases[*base_idx].y);
        } // y2 - y1
        acc *= *z;
    }

    acc = acc
        .invert()
        .expect("Some edge case has not been handled properly");

    for (
        (
            SchedulePoint {
                base_idx,
                buck_idx,
                sign,
            },
            t,
        ),
        z,
    ) in points.iter().zip(t.iter()).zip(z.iter()).rev()
    {
        if buckets[*buck_idx].is_inf() {
            // We assume bases[*base_idx] != infinity always.
            continue;
        }
        let lambda = acc * t;
        acc *= z; // update acc
        let x = lambda.square() - (buckets[*buck_idx].x() + bases[*base_idx].x); // x_result
        if *sign {
            buckets[*buck_idx].set_y(&((lambda * (bases[*base_idx].x - x)) - bases[*base_idx].y));
        } else {
            buckets[*buck_idx].set_y(&((lambda * (bases[*base_idx].x - x)) + bases[*base_idx].y));
        } // y_result = lambda * (x1 - x_result) - y1
        buckets[*buck_idx].set_x(&x);
    }
}

#[derive(Debug, Clone, Copy)]
struct Affine<C: CurveAffine> {
    x: C::Base,
    y: C::Base,
}

impl<C: CurveAffine> Affine<C> {
    fn from(point: &C) -> Self {
        let coords = point.coordinates().unwrap();

        Self {
            x: *coords.x(),
            y: *coords.y(),
        }
    }

    fn neg(&self) -> Self {
        Self {
            x: self.x,
            y: -self.y,
        }
    }

    fn eval(&self) -> C {
        C::from_xy(self.x, self.y).unwrap()
    }
}

#[derive(Debug, Clone)]
enum BucketAffine<C: CurveAffine> {
    None,
    Point(Affine<C>),
}

#[derive(Debug, Clone)]
enum Bucket<C: CurveAffine> {
    None,
    Point(C::Curve),
}

impl<C: CurveAffine> Bucket<C> {
    fn add_assign(&mut self, point: &C, sign: bool) {
        *self = match *self {
            Bucket::None => Bucket::Point({
                if sign {
                    point.to_curve()
                } else {
                    point.to_curve().neg()
                }
            }),
            Bucket::Point(a) => {
                if sign {
                    Self::Point(a + point)
                } else {
                    Self::Point(a - point)
                }
            }
        }
    }

    fn add(&self, other: &BucketAffine<C>) -> C::Curve {
        match (self, other) {
            (Self::Point(this), BucketAffine::Point(other)) => *this + other.eval(),
            (Self::Point(this), BucketAffine::None) => *this,
            (Self::None, BucketAffine::Point(other)) => other.eval().to_curve(),
            (Self::None, BucketAffine::None) => C::Curve::identity(),
        }
    }
}

impl<C: CurveAffine> BucketAffine<C> {
    fn assign(&mut self, point: &Affine<C>, sign: bool) -> bool {
        match *self {
            Self::None => {
                *self = Self::Point(if sign { *point } else { point.neg() });
                true
            }
            Self::Point(_) => false,
        }
    }

    fn x(&self) -> C::Base {
        match self {
            Self::None => panic!("::x None"),
            Self::Point(a) => a.x,
        }
    }

    fn y(&self) -> C::Base {
        match self {
            Self::None => panic!("::y None"),
            Self::Point(a) => a.y,
        }
    }

    fn is_inf(&self) -> bool {
        match self {
            Self::None => true,
            Self::Point(_) => false,
        }
    }

    fn set_x(&mut self, x: &C::Base) {
        match self {
            Self::None => panic!("::set_x None"),
            Self::Point(a) => a.x = *x,
        }
    }

    fn set_y(&mut self, y: &C::Base) {
        match self {
            Self::None => panic!("::set_y None"),
            Self::Point(a) => a.y = *y,
        }
    }

    fn set_inf(&mut self) {
        match self {
            Self::None => {}
            Self::Point(_) => *self = Self::None,
        }
    }
}

struct Schedule<C: CurveAffine> {
    buckets: Vec<BucketAffine<C>>,
    set: [SchedulePoint; BATCH_SIZE],
    ptr: usize,
}

#[derive(Debug, Clone, Default)]
struct SchedulePoint {
    base_idx: usize,
    buck_idx: usize,
    sign: bool,
}

impl SchedulePoint {
    fn new(base_idx: usize, buck_idx: usize, sign: bool) -> Self {
        Self {
            base_idx,
            buck_idx,
            sign,
        }
    }
}

impl<C: CurveAffine> Schedule<C> {
    fn new(c: usize) -> Self {
        let set = (0..BATCH_SIZE)
            .map(|_| SchedulePoint::default())
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        Self {
            buckets: vec![BucketAffine::None; 1 << (c - 1)],
            set,
            ptr: 0,
        }
    }

    fn contains(&self, buck_idx: usize) -> bool {
        self.set.iter().any(|sch| sch.buck_idx == buck_idx)
    }

    fn execute(&mut self, bases: &[Affine<C>]) {
        if self.ptr != 0 {
            batch_add(self.ptr, &mut self.buckets, &self.set, bases);
            self.ptr = 0;
            self.set
                .iter_mut()
                .for_each(|sch| *sch = SchedulePoint::default());
        }
    }

    fn add(&mut self, bases: &[Affine<C>], base_idx: usize, buck_idx: usize, sign: bool) {
        if !self.buckets[buck_idx].assign(&bases[base_idx], sign) {
            self.set[self.ptr] = SchedulePoint::new(base_idx, buck_idx, sign);
            self.ptr += 1;
        }

        if self.ptr == self.set.len() {
            self.execute(bases);
        }
    }
}

/// Performs a multi-scalar multiplication operation.
///
/// This function will panic if coeffs and bases have a different length.
pub fn msm_serial<C: CurveAffine>(coeffs: &[C::Scalar], bases: &[C], acc: &mut C::Curve) {
    let coeffs: Vec<_> = coeffs.iter().map(|a| a.to_repr()).collect();

    let scalar_bits = C::Scalar::NUM_BITS;
    let c = optimal_window_size(bases.len(), scalar_bits as usize);

    let field_byte_size = scalar_bits.div_ceil(8u32) as usize;
    // OR all coefficients in order to make a mask to figure out the maximum number of bytes used
    // among all coefficients.
    let mut acc_or = vec![0; field_byte_size];
    for coeff in &coeffs {
        for (acc_limb, limb) in acc_or.iter_mut().zip(coeff.as_ref().iter()) {
            *acc_limb |= *limb;
        }
    }
    let max_byte_size = field_byte_size
        - acc_or
            .iter()
            .rev()
            .position(|v| *v != 0)
            .unwrap_or(field_byte_size);
    if max_byte_size == 0 {
        return;
    }
    let number_of_windows = max_byte_size * 8_usize / c + 1;

    for current_window in (0..number_of_windows).rev() {
        for _ in 0..c {
            *acc = acc.double();
        }

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
                    Bucket::Affine(a) => Bucket::Projective(a + *other),
                    Bucket::Projective(mut a) => {
                        a += *other;
                        Bucket::Projective(a)
                    }
                }
            }

            fn add(self, mut other: C::Curve) -> C::Curve {
                match self {
                    Bucket::None => other,
                    Bucket::Affine(a) => {
                        other += a;
                        other
                    }
                    Bucket::Projective(a) => other + a,
                }
            }
        }

        let mut buckets: Vec<Bucket<C>> = vec![Bucket::None; 1 << (c - 1)];

        for (coeff, base) in coeffs.iter().zip(bases.iter()) {
            let coeff = get_booth_index(current_window, c, coeff.as_ref());
            if coeff.is_positive() {
                buckets[coeff as usize - 1].add_assign(base);
            }
            if coeff.is_negative() {
                buckets[coeff.unsigned_abs() as usize - 1].add_assign(&base.neg());
            }
        }

        // Summation by parts
        // e.g. 3a + 2b + 1c = a +
        //                    (a) + b +
        //                    ((a) + b) + c
        let mut running_sum = C::Curve::identity();
        for exp in buckets.into_iter().rev() {
            running_sum = exp.add(running_sum);
            *acc += &running_sum;
        }
    }
}

/// Performs a multi-scalar multiplication operation.
///
/// This function will panic if coeffs and bases have a different length.
///
/// This will use multithreading if beneficial.
pub fn msm_parallel<C: CurveAffine>(coeffs: &[C::Scalar], bases: &[C]) -> C::Curve {
    assert_eq!(coeffs.len(), bases.len());

    let num_threads = rayon::current_num_threads();
    if coeffs.len() > num_threads {
        let chunk = coeffs.len() / num_threads;
        let num_chunks = coeffs.chunks(chunk).len();
        let mut results = vec![C::Curve::identity(); num_chunks];
        rayon::scope(|scope| {
            let chunk = coeffs.len() / num_threads;

            for ((coeffs, bases), acc) in coeffs
                .chunks(chunk)
                .zip(bases.chunks(chunk))
                .zip(results.iter_mut())
            {
                scope.spawn(move |_| {
                    msm_serial(coeffs, bases, acc);
                });
            }
        });
        results.iter().fold(C::Curve::identity(), |a, b| a + b)
    } else {
        let mut acc = C::Curve::identity();
        msm_serial(coeffs, bases, &mut acc);
        acc
    }
}

/// This function will panic if coeffs and bases have a different length.
///
/// This will use multithreading if beneficial.
pub fn msm_best<C: CurveAffine>(coeffs: &[C::Scalar], bases: &[C]) -> C::Curve {
    assert_eq!(coeffs.len(), bases.len());

    let c = optimal_window_size(bases.len(), C::Scalar::NUM_BITS as usize);

    if c < 10 {
        return msm_parallel(coeffs, bases);
    }

    // One large MSM already consumes every worker in the bounded window pool.
    // Admit callers inside the non-cooperative dispatcher before allocating
    // preprocessing buffers, so parallel outer commitment loops cannot retain
    // one scalar/base copy per waiter or deadlock through Rayon work stealing.
    run_large_msm_admitted(|| {
        // coeffs to byte representation
        let coeffs: Vec<_> = coeffs.par_iter().map(|a| a.to_repr()).collect();
        // copy bases into `Affine` to skip in on curve check for every access
        let bases_local: Vec<_> = bases.par_iter().map(Affine::from).collect();

        // number of windows
        let number_of_windows = C::Scalar::NUM_BITS as usize / c + 1;
        // accumumator for each window
        let mut acc = vec![C::Curve::identity(); number_of_windows];
        let shard_len = parallel_window_shard_len(number_of_windows);
        acc.par_chunks_mut(shard_len)
            .enumerate()
            .rev()
            .for_each(|(shard_index, acc_shard)| {
                let window_offset = shard_index * shard_len;
                for (shard_window, acc) in acc_shard
                    .iter_mut()
                    .enumerate()
                    .rev()
                {
                    let w = window_offset + shard_window;

                    // jacobian buckets for already scheduled points
                    let mut j_bucks = vec![Bucket::<C>::None; 1 << (c - 1)];

                    // schedular for affine addition
                    let mut sched = Schedule::new(c);

                    for (base_idx, coeff) in coeffs.iter().enumerate() {
                        let buck_idx = get_booth_index(w, c, coeff.as_ref());

                        if buck_idx != 0 {
                            // parse bucket index
                            let sign = buck_idx.is_positive();
                            let buck_idx = buck_idx.unsigned_abs() as usize - 1;

                            if sched.contains(buck_idx) {
                                // greedy accumulation
                                // we use original bases here
                                j_bucks[buck_idx].add_assign(&bases[base_idx], sign);
                            } else {
                                // also flushes the schedule if full
                                sched.add(&bases_local, base_idx, buck_idx, sign);
                            }
                        }
                    }

                    // flush the schedule
                    sched.execute(&bases_local);

                    // summation by parts
                    // e.g. 3a + 2b + 1c = a +
                    //                    (a) + b +
                    //                    ((a) + b) + c
                    let mut running_sum = C::Curve::identity();
                    for (j_buck, a_buck) in j_bucks.iter().zip(sched.buckets.iter()).rev() {
                        running_sum += j_buck.add(a_buck);
                        *acc += running_sum;
                    }

                    // shift accumulator to the window position
                    for _ in 0..c * w {
                        *acc = acc.double();
                    }
                }
            });
        acc.into_iter().sum::<_>()
    })
}

#[cfg(test)]
mod test {
    use std::ops::Neg;

    use crate::bn256::{Fr, G1Affine, G1};
    use ark_std::{end_timer, start_timer};
    use ff::{Field, PrimeField};
    use group::{Curve, Group};
    use pasta_curves::arithmetic::CurveAffine;
    use rand_core::OsRng;

    #[test]
    fn window_size_heuristic_matches_expected_ranges() {
        let bits = Fr::NUM_BITS as usize;
        assert_eq!(super::optimal_window_size(0, bits), 1);
        assert_eq!(super::optimal_window_size(3, bits), 1);
        assert_eq!(super::optimal_window_size(16, bits), 3);
        assert_eq!(super::optimal_window_size(512, bits), 8);
        assert_eq!(super::optimal_window_size(4096, bits), 10);
        assert_eq!(super::optimal_window_size(65_536, bits), 13);
        assert_eq!(super::optimal_window_size(250_000, bits), 15);
    }

    #[test]
    fn bounded_window_shards_match_reference_msm() {
        use rand::{SeedableRng, rngs::StdRng};

        const POINTS: usize = 1 << 12;
        let window_size = super::optimal_window_size(POINTS, Fr::NUM_BITS as usize);
        assert!(window_size >= 10);
        let number_of_windows = Fr::NUM_BITS as usize / window_size + 1;
        let shard_len = super::parallel_window_shard_len(number_of_windows);
        assert!(number_of_windows > super::MAX_PARALLEL_WINDOW_SHARDS);
        assert!(
            number_of_windows.div_ceil(shard_len) <= super::MAX_PARALLEL_WINDOW_SHARDS
        );

        let mut rng = StdRng::seed_from_u64(0x4d53_4d42_4154_4348);
        let points = (0..POINTS)
            .map(|_| G1Affine::random(&mut rng))
            .collect::<Vec<_>>();
        let random_scalars = (0..POINTS)
            .map(|_| Fr::random(&mut rng))
            .collect::<Vec<_>>();
        let edge_scalars = (0..POINTS)
            .map(|index| match index % 4 {
                0 => Fr::ZERO,
                1 => Fr::ONE,
                2 => -Fr::ONE,
                _ => Fr::from(index as u64),
            })
            .collect::<Vec<_>>();

        for scalars in [&random_scalars, &edge_scalars] {
            assert_eq!(
                super::msm_best(scalars, &points),
                super::msm_parallel(scalars, &points)
            );
        }
    }

    #[test]
    fn large_msm_window_pool_has_bounded_width() {
        assert_eq!(
            super::large_msm_window_pool().current_num_threads(),
            super::MAX_PARALLEL_WINDOW_SHARDS
        );
    }

    #[test]
    fn large_msm_admission_serializes_concurrent_callers() {
        use std::{
            sync::{
                Arc, Barrier,
                atomic::{AtomicUsize, Ordering},
            },
            time::Duration,
        };

        const CALLERS: usize = 8;
        let start = Arc::new(Barrier::new(CALLERS));
        let active = AtomicUsize::new(0);
        let max_active = AtomicUsize::new(0);

        std::thread::scope(|scope| {
            for _ in 0..CALLERS {
                let start = Arc::clone(&start);
                let active = &active;
                let max_active = &max_active;
                scope.spawn(move || {
                    start.wait();
                    let _admission = super::enter_large_msm();
                    let active_now = active.fetch_add(1, Ordering::SeqCst) + 1;
                    max_active.fetch_max(active_now, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(5));
                    assert_eq!(active.fetch_sub(1, Ordering::SeqCst), 1);
                });
            }
        });

        assert_eq!(max_active.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn large_msm_dispatch_does_not_steal_source_pool_work() {
        use std::{
            sync::atomic::{AtomicBool, Ordering},
            time::Duration,
        };

        let source_work_ran = AtomicBool::new(false);
        let source_work_ran_during_dispatch = AtomicBool::new(false);
        let source_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .expect("source pool should build");

        source_pool.install(|| {
            rayon::join(
                || {
                    super::run_large_msm_admitted(|| {
                        std::thread::sleep(Duration::from_millis(20));
                        source_work_ran_during_dispatch.store(
                            source_work_ran.load(Ordering::SeqCst),
                            Ordering::SeqCst,
                        );
                    });
                },
                || source_work_ran.store(true, Ordering::SeqCst),
            );
        });

        assert!(source_work_ran.load(Ordering::SeqCst));
        assert!(!source_work_ran_during_dispatch.load(Ordering::SeqCst));
    }

    #[test]
    fn test_booth_encoding() {
        fn mul(scalar: &Fr, point: &G1Affine, window: usize) -> G1Affine {
            let u = scalar.to_repr();
            let n = Fr::NUM_BITS as usize / window + 1;

            let table = (0..=1 << (window - 1))
                .map(|i| point * Fr::from(i as u64))
                .collect::<Vec<_>>();

            let mut acc = G1::identity();
            for i in (0..n).rev() {
                for _ in 0..window {
                    acc = acc.double();
                }

                let idx = super::get_booth_index(i, window, u.as_ref());

                if idx.is_negative() {
                    acc += table[idx.unsigned_abs() as usize].neg();
                }
                if idx.is_positive() {
                    acc += table[idx.unsigned_abs() as usize];
                }
            }

            acc.to_affine()
        }

        let (scalars, points): (Vec<_>, Vec<_>) = (0..10)
            .map(|_| {
                let scalar = Fr::random(OsRng);
                let point = G1Affine::random(OsRng);
                (scalar, point)
            })
            .unzip();

        for window in 1..10 {
            for (scalar, point) in scalars.iter().zip(points.iter()) {
                let c0 = mul(scalar, point, window);
                let c1 = point * scalar;
                assert_eq!(c0, c1.to_affine());
            }
        }
    }

    fn run_msm_cross<C: CurveAffine>(min_k: usize, max_k: usize) {
        use rayon::iter::{IntoParallelIterator, ParallelIterator};

        let points = (0..1 << max_k)
            .into_par_iter()
            .map(|_| C::Curve::random(OsRng))
            .collect::<Vec<_>>();
        let mut affine_points = vec![C::identity(); 1 << max_k];
        C::Curve::batch_normalize(&points[..], &mut affine_points[..]);
        let points = affine_points;

        let scalars = (0..1 << max_k)
            .into_par_iter()
            .map(|_| C::Scalar::random(OsRng))
            .collect::<Vec<_>>();

        for k in min_k..=max_k {
            let points = &points[..1 << k];
            let scalars = &scalars[..1 << k];

            let t0 = start_timer!(|| format!("cyclone indep k={}", k));
            let e0 = super::msm_best(scalars, points);
            end_timer!(t0);

            let t1 = start_timer!(|| format!("older k={}", k));
            let e1 = super::msm_parallel(scalars, points);
            end_timer!(t1);
            assert_eq!(e0, e1);
        }
    }

    #[test]
    fn test_msm_cross() {
        run_msm_cross::<G1Affine>(14, 22);
    }
}
