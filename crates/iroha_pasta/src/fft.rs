//! Number-theoretic transforms over the Pasta fields.
//!
//! [`FftDomain`] evaluates and interpolates polynomials on the multiplicative
//! subgroup of order `n = 2^k`, and on its cosets. It uses the generator
//! halo2 uses: `omega = ROOT_OF_UNITY^(2^(32 - k))`.
//!
//! # Algorithm
//!
//! Decimation in frequency (natural-order input, bit-reversed intermediate
//! order), iterative, with two radix-2 stages fused per pass ("radix-4"
//! memory traffic: each pass reads and writes the array once for two stages).
//! A trailing radix-2 stage runs when `k` is odd. The final pass restores
//! natural order. In a prime field a radix-4 butterfly needs the same number of
//! multiplications as two radix-2 stages, so the gain is memory traffic only.
//! Once the blocks of the remaining stages fit [`DEPTH_FIRST_ELEMENTS`], each
//! block finishes all of them before the next block starts, so a large
//! transform reads main memory for its first passes only.
//!
//! The twiddle factors of every stage are cached contiguously in the domain
//! ([`FftDomain::twiddle_bytes`] reports their size). The coset shift is fused
//! into the first pass of [`FftDomain::coset_fft`]; the inverse transforms fuse
//! the `1/n` scaling (and the inverse shift) into the final reordering pass.
//!
//! # Scheduling
//!
//! A transform offers each pass over the whole array as [`TASKS_PER_WORKER`]
//! tasks per worker, and a block that finishes depth first offers its quarters
//! once it is larger than that share, so a worker on a slow core never holds
//! more than a small part of a transform. A batch runs one claiming loop per
//! worker: a loop takes the next column and transforms it with the same
//! kernels. While every worker has a column, each column stays with its
//! worker; a worker whose loop has ended takes tasks of the columns still
//! running. A batch narrower than the pool, or one whose width is not a
//! multiple of it, therefore keeps every worker busy to the end.
//!
//! # Determinism
//!
//! The transforms are exact field arithmetic, so the output is a pure function
//! of the input regardless of the Rayon pool size or chunking.
#![allow(clippy::many_single_char_names)]

use std::sync::Mutex;

use rayon::prelude::*;

use crate::field::PastaField;
use crate::{CancellationToken, Cancelled};

/// Below this many elements per pass the transforms run on the calling
/// thread; above it they split work across the caller's Rayon pool.
const PARALLEL_MIN: usize = 1 << 12;

/// Tasks per worker that a pass over the whole array, and the depth-first
/// stage after it, offer the pool. A pass split into one or two tasks per
/// worker lasts as long as its slowest worker's share, and the cores of one
/// phone differ about six-fold in speed. Eight tasks bound that share at an
/// eighth of a fair one, at no measurable cost on equal cores.
const TASKS_PER_WORKER: usize = 8;

/// Fewest butterflies in one task of a parallel pass.
const PASS_TASK_MIN: usize = 64;

/// Fewest elements a depth-first block offers the pool as one task.
const LEAF_MIN: usize = 256;

/// At most this many elements per block finish their remaining stages
/// depth first. 2^12 elements of 32 bytes is 128 KiB, inside the L2 cache of
/// the smallest phone cores (Cortex-A55: 128 to 256 KiB), where a 2 MiB k16
/// column would otherwise stream through main memory for all eight passes.
/// Blocks are independent from that stage on, so this only reorders exact
/// butterflies: every output is unchanged.
const DEPTH_FIRST_ELEMENTS: usize = 1 << 12;

/// FFT domain construction failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FftError {
    /// The caller cancelled this transform; its in-place buffer is incomplete.
    Cancelled,
    /// `k` exceeds the 2-adicity of the field (32) or the platform limit.
    UnsupportedSize {
        /// The requested `log2(n)`.
        k: u64,
    },
    /// The input length is not `n`.
    WrongLength {
        /// The domain size `n`.
        expected: usize,
        /// The input length.
        actual: usize,
    },
    /// The source and destination batches have different column counts.
    WrongColumnCount {
        /// The number of destination columns.
        expected: usize,
        /// The number of source columns.
        actual: usize,
    },
    /// A coset shift of zero (not a coset; the inverse transform would need
    /// `0^-1`).
    ZeroShift,
}

impl core::fmt::Display for FftError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Cancelled => f.write_str("FFT: operation cancelled"),
            Self::UnsupportedSize { k } => write!(f, "unsupported FFT size 2^{k}"),
            Self::WrongLength { expected, actual } => {
                write!(f, "FFT input has {actual} elements, expected {expected}")
            }
            Self::WrongColumnCount { expected, actual } => {
                write!(f, "FFT has {actual} source columns, expected {expected}")
            }
            Self::ZeroShift => write!(f, "FFT coset shift is zero"),
        }
    }
}

impl std::error::Error for FftError {}
impl From<Cancelled> for FftError {
    fn from(_: Cancelled) -> Self {
        Self::Cancelled
    }
}

/// A multiplicative subgroup of order `2^k` with cached twiddle factors.
#[derive(Clone, Debug)]
pub struct FftDomain<F: PastaField> {
    k: u32,
    n: usize,
    omega: F,
    omega_inv: F,
    n_inv: F,
    /// `forward[s]` holds `w_{2h}^j` for `j < h`, `h = 2^s`.
    forward: Vec<Vec<F>>,
    /// The same for `omega_inv`.
    inverse: Vec<Vec<F>>,
}

/// A validated coset transform plan borrowing the exact domain and caller powers.
#[derive(Clone, Copy, Debug)]
pub struct CosetFftPlan<'domain, 'scratch, F: PastaField> {
    domain: &'domain FftDomain<F>,
    powers: &'scratch [F],
}

impl<F: PastaField> CosetFftPlan<'_, '_, F> {
    /// The exact coefficient and output length required by this plan.
    #[must_use]
    pub fn n(&self) -> usize {
        self.domain.n()
    }

    /// Evaluates columns in place using the immutable validated powers.
    ///
    /// # Errors
    ///
    /// Rejects any wrong column length before mutating any input.
    pub fn fft_many(&self, columns: &mut [&mut [F]]) -> Result<(), FftError> {
        self.fft_many_cancellable(columns, None)
    }

    /// Runs [`Self::fft_many`] with an explicit per-operation cancellation signal.
    /// All Rayon tasks join before returning. After cancellation the buffer is
    /// incomplete and must be discarded or reinitialized by its owner.
    ///
    /// # Errors
    /// As [`Self::fft_many`], or [`FftError::Cancelled`].
    pub fn fft_many_cancellable(
        &self,
        columns: &mut [&mut [F]],
        cancellation: Option<&CancellationToken>,
    ) -> Result<(), FftError> {
        CancellationToken::checkpoint(cancellation)?;
        for column in columns.iter() {
            self.domain.check(column)?;
        }
        let shift = Some(CosetShift::Prepared(self.powers));
        if let [column] = columns {
            dif::<_, true>(column, &self.domain.forward, shift, cancellation);
            bit_reverse_scale::<_, true>(column, self.domain.k, None, cancellation);
        } else {
            claim_each(columns.iter_mut(), |column| {
                dif::<_, true>(column, &self.domain.forward, shift, cancellation);
                bit_reverse_scale::<_, true>(column, self.domain.k, None, cancellation);
            });
        }
        CancellationToken::checkpoint(cancellation)?;
        Ok(())
    }

    /// Evaluates immutable coefficient columns into caller-owned output columns.
    ///
    /// The first butterfly pass reads the coefficients directly, avoiding a
    /// separate copy into the output. Later passes reuse that output in place.
    /// Each source is borrowed exactly once and retained as slice metadata before
    /// validation. Powers and coefficients remain unchanged; no field scratch
    /// is allocated.
    /// All Rayon tasks join before cancellation is reported. A cancelled output
    /// is incomplete and must be discarded or reinitialized by its owner.
    ///
    /// # Errors
    ///
    /// Checks cancellation, then column count, then every source/destination
    /// length before writing any output. A mismatch returns
    /// [`FftError::WrongColumnCount`] or [`FftError::WrongLength`].
    pub fn fft_many_from_cancellable(
        &self,
        coefficients: &[impl AsRef<[F]> + Sync],
        columns: &mut [&mut [F]],
        cancellation: Option<&CancellationToken>,
    ) -> Result<(), FftError> {
        CancellationToken::checkpoint(cancellation)?;
        if coefficients.len() != columns.len() {
            return Err(FftError::WrongColumnCount {
                expected: columns.len(),
                actual: coefficients.len(),
            });
        }
        // AsRef need not return the same slice on a later call. Freeze its first
        // result so the exact views checked below also supply the butterflies.
        let sources: Vec<&[F]> = coefficients.iter().map(AsRef::as_ref).collect();
        for (source, output) in sources.iter().zip(columns.iter()) {
            self.domain.check(source)?;
            self.domain.check(output)?;
        }
        CancellationToken::checkpoint(cancellation)?;
        if let [column] = columns {
            dif_from::<_, true>(
                sources[0],
                column,
                &self.domain.forward,
                self.powers,
                cancellation,
            );
            bit_reverse_scale::<_, true>(column, self.domain.k, None, cancellation);
        } else {
            claim_each(
                columns.iter_mut().zip(sources.iter()),
                |(column, source)| {
                    dif_from::<_, true>(
                        source,
                        column,
                        &self.domain.forward,
                        self.powers,
                        cancellation,
                    );
                    bit_reverse_scale::<_, true>(column, self.domain.k, None, cancellation);
                },
            );
        }
        CancellationToken::checkpoint(cancellation)?;
        Ok(())
    }
}

/// Exact first-pass coset scaling; all later passes have no shift.
#[derive(Clone, Copy)]
enum CosetShift<'a, F> {
    Geometric(F),
    Prepared(&'a [F]),
}

impl<F: PastaField> FftDomain<F> {
    /// Builds the domain of size `2^k` and its twiddle caches.
    ///
    /// # Errors
    ///
    /// [`FftError::UnsupportedSize`] when `k > 32` or `2^k` does not fit
    /// `usize`.
    pub fn new(k: u32) -> Result<Self, FftError> {
        if k > F::S || k >= usize::BITS {
            return Err(FftError::UnsupportedSize { k: u64::from(k) });
        }
        let n = 1usize << k;
        let mut omega = F::ROOT_OF_UNITY;
        let mut omega_inv = F::ROOT_OF_UNITY_INV;
        for _ in k..F::S {
            omega = omega.square();
            omega_inv = omega_inv.square();
        }
        // n^-1 = (1/2)^k.
        let n_inv = F::TWO_INV.pow_vartime([u64::from(k)]);
        let forward = stage_twiddles(omega, k);
        let inverse = stage_twiddles(omega_inv, k);
        Ok(Self {
            k,
            n,
            omega,
            omega_inv,
            n_inv,
            forward,
            inverse,
        })
    }

    /// `log2` of the domain size.
    pub fn k(&self) -> u32 {
        self.k
    }

    /// The domain size `n = 2^k`.
    pub fn n(&self) -> usize {
        self.n
    }

    /// The generator `omega` of the subgroup.
    pub fn omega(&self) -> F {
        self.omega
    }

    /// `omega^-1`.
    pub fn omega_inv(&self) -> F {
        self.omega_inv
    }

    /// `n^-1`.
    pub fn n_inv(&self) -> F {
        self.n_inv
    }

    /// Bytes held by the twiddle caches (both directions).
    pub fn twiddle_bytes(&self) -> usize {
        let count: usize = self
            .forward
            .iter()
            .chain(self.inverse.iter())
            .map(Vec::len)
            .sum();
        count.saturating_mul(core::mem::size_of::<F>())
    }

    fn check(&self, a: &[F]) -> Result<(), FftError> {
        if a.len() == self.n {
            Ok(())
        } else {
            Err(FftError::WrongLength {
                expected: self.n,
                actual: a.len(),
            })
        }
    }

    /// Evaluates the polynomial with coefficients `a` at `omega^i`, in place.
    ///
    /// # Errors
    ///
    /// [`FftError::WrongLength`] unless `a.len() == n`.
    pub fn fft(&self, a: &mut [F]) -> Result<(), FftError> {
        self.fft_cancellable(a, None)
    }

    /// Runs [`Self::fft`] with an explicit per-operation cancellation signal.
    /// All Rayon tasks join before returning. After cancellation the buffer is
    /// incomplete and must be discarded or reinitialized by its owner.
    ///
    /// # Errors
    /// As [`Self::fft`], or [`FftError::Cancelled`].
    pub fn fft_cancellable(
        &self,
        a: &mut [F],
        cancellation: Option<&CancellationToken>,
    ) -> Result<(), FftError> {
        CancellationToken::checkpoint(cancellation)?;
        self.check(a)?;
        dif::<_, true>(a, &self.forward, None, cancellation);
        bit_reverse_scale::<_, true>(a, self.k, None, cancellation);
        CancellationToken::checkpoint(cancellation)?;
        Ok(())
    }

    /// Interpolates evaluations at `omega^i` back to coefficients, in place.
    ///
    /// # Errors
    ///
    /// [`FftError::WrongLength`] unless `a.len() == n`.
    pub fn ifft(&self, a: &mut [F]) -> Result<(), FftError> {
        self.ifft_cancellable(a, None)
    }

    /// Runs [`Self::ifft`] with an explicit per-operation cancellation signal.
    /// All Rayon tasks join before returning. After cancellation the buffer is
    /// incomplete and must be discarded or reinitialized by its owner.
    ///
    /// # Errors
    /// As [`Self::ifft`], or [`FftError::Cancelled`].
    pub fn ifft_cancellable(
        &self,
        a: &mut [F],
        cancellation: Option<&CancellationToken>,
    ) -> Result<(), FftError> {
        CancellationToken::checkpoint(cancellation)?;
        self.check(a)?;
        dif::<_, true>(a, &self.inverse, None, cancellation);
        bit_reverse_scale::<_, true>(a, self.k, Some((self.n_inv, F::ONE)), cancellation);
        CancellationToken::checkpoint(cancellation)?;
        Ok(())
    }

    /// Interpolates every column back to coefficients, in place: [`Self::ifft`]
    /// of each, as one batch on the caller's pool.
    ///
    /// # Errors
    ///
    /// [`FftError::WrongLength`] for any column that is not `n` long, before
    /// any column is touched.
    pub fn ifft_many(&self, columns: &mut [&mut [F]]) -> Result<(), FftError> {
        self.ifft_many_cancellable(columns, None)
    }

    /// Runs [`Self::ifft_many`] with an explicit per-operation cancellation signal.
    /// All Rayon tasks join before returning. After cancellation the buffers are
    /// incomplete and must be discarded or reinitialized by their owner.
    ///
    /// # Errors
    /// As [`Self::ifft_many`], or [`FftError::Cancelled`].
    pub fn ifft_many_cancellable(
        &self,
        columns: &mut [&mut [F]],
        cancellation: Option<&CancellationToken>,
    ) -> Result<(), FftError> {
        CancellationToken::checkpoint(cancellation)?;
        for column in columns.iter() {
            self.check(column)?;
        }
        let scale = Some((self.n_inv, F::ONE));
        if let [column] = columns {
            dif::<_, true>(column, &self.inverse, None, cancellation);
            bit_reverse_scale::<_, true>(column, self.k, scale, cancellation);
        } else {
            claim_each(columns.iter_mut(), |column| {
                dif::<_, true>(column, &self.inverse, None, cancellation);
                bit_reverse_scale::<_, true>(column, self.k, scale, cancellation);
            });
        }
        CancellationToken::checkpoint(cancellation)?;
        Ok(())
    }

    /// Evaluates the polynomial with coefficients `a` at `shift * omega^i`,
    /// in place. The multiplication of coefficient `i` by `shift^i` is fused
    /// into the first pass.
    ///
    /// # Errors
    ///
    /// [`FftError::WrongLength`] unless `a.len() == n`;
    /// [`FftError::ZeroShift`] for `shift = 0`, which is not a coset (checked
    /// before `a` is touched).
    pub fn coset_fft(&self, a: &mut [F], shift: F) -> Result<(), FftError> {
        self.coset_fft_cancellable(a, shift, None)
    }

    /// Runs [`Self::coset_fft`] with an explicit per-operation cancellation signal.
    /// All Rayon tasks join before returning. After cancellation the buffer is
    /// incomplete and must be discarded or reinitialized by its owner.
    ///
    /// # Errors
    /// As [`Self::coset_fft`], or [`FftError::Cancelled`].
    pub fn coset_fft_cancellable(
        &self,
        a: &mut [F],
        shift: F,
        cancellation: Option<&CancellationToken>,
    ) -> Result<(), FftError> {
        CancellationToken::checkpoint(cancellation)?;
        self.check(a)?;
        check_shift(&shift)?;
        dif::<_, true>(
            a,
            &self.forward,
            Some(CosetShift::Geometric(shift)),
            cancellation,
        );
        bit_reverse_scale::<_, true>(a, self.k, None, cancellation);
        CancellationToken::checkpoint(cancellation)?;
        Ok(())
    }

    /// Evaluates several coefficient columns on the same coset, in place.
    ///
    /// Multiple columns are claimed one at a time by the workers of the
    /// caller's Rayon pool (see the module's scheduling notes); each uses the
    /// same butterfly arithmetic and kernels as [`Self::coset_fft`]. One column
    /// retains the single-transform scheduling. The domain's immutable twiddles
    /// are shared and no field scratch buffers are allocated. Column order and
    /// worker count cannot affect the output.
    ///
    /// # Errors
    ///
    /// [`FftError::WrongLength`] if any column's length differs from `n`, then
    /// [`FftError::ZeroShift`] for `shift = 0`. All columns and the shift are
    /// checked before any input is touched, including for an empty batch.
    pub fn coset_fft_many(&self, columns: &mut [&mut [F]], shift: F) -> Result<(), FftError> {
        self.coset_fft_many_cancellable(columns, shift, None)
    }

    /// Runs [`Self::coset_fft_many`] with an explicit per-operation cancellation signal.
    /// All Rayon tasks join before returning. After cancellation the buffer is
    /// incomplete and must be discarded or reinitialized by its owner.
    ///
    /// # Errors
    /// As [`Self::coset_fft_many`], or [`FftError::Cancelled`].
    pub fn coset_fft_many_cancellable(
        &self,
        columns: &mut [&mut [F]],
        shift: F,
        cancellation: Option<&CancellationToken>,
    ) -> Result<(), FftError> {
        CancellationToken::checkpoint(cancellation)?;
        for column in columns.iter() {
            self.check(column)?;
        }
        check_shift(&shift)?;
        if let [column] = columns {
            return self.coset_fft_cancellable(column, shift, cancellation);
        }
        claim_each(columns.iter_mut(), |column| {
            dif::<_, true>(
                column,
                &self.forward,
                Some(CosetShift::Geometric(shift)),
                cancellation,
            );
            bit_reverse_scale::<_, true>(column, self.k, None, cancellation);
        });
        CancellationToken::checkpoint(cancellation)?;
        Ok(())
    }

    /// Fills caller-owned powers and binds them immutably to this domain.
    ///
    /// The plan and its transforms allocate no field scratch. The caller must
    /// retain and account for exactly `n` field elements for the plan lifetime.
    /// Powers use the same exact arithmetic for every caller worker count.
    ///
    /// # Errors
    ///
    /// Rejects a wrong scratch length or zero shift before touching scratch.
    pub fn coset_plan<'domain, 'scratch>(
        &'domain self,
        scratch: &'scratch mut [F],
        shift: F,
    ) -> Result<CosetFftPlan<'domain, 'scratch, F>, FftError> {
        self.coset_plan_cancellable(scratch, shift, None)
    }

    /// Prepares coset powers with an explicit cancellation signal.
    ///
    /// # Errors
    /// As [`Self::coset_plan`], or [`FftError::Cancelled`]; scratch then holds
    /// incomplete powers and remains exclusively owned by the caller.
    pub fn coset_plan_cancellable<'domain, 'scratch>(
        &'domain self,
        scratch: &'scratch mut [F],
        shift: F,
        cancellation: Option<&CancellationToken>,
    ) -> Result<CosetFftPlan<'domain, 'scratch, F>, FftError> {
        CancellationToken::checkpoint(cancellation)?;
        self.check(scratch)?;
        check_shift(&shift)?;
        let fill = |(index, chunk): (usize, &mut [F])| {
            if CancellationToken::checkpoint(cancellation).is_err() {
                return;
            }
            let mut power = shift.pow_vartime([(index * PARALLEL_MIN) as u64]);
            for value in chunk {
                *value = power;
                power *= shift;
            }
        };
        if self.n >= PARALLEL_MIN {
            scratch
                .par_chunks_mut(PARALLEL_MIN)
                .enumerate()
                .for_each(fill);
        } else {
            fill((0, scratch));
        }
        CancellationToken::checkpoint(cancellation)?;
        Ok(CosetFftPlan {
            domain: self,
            powers: scratch,
        })
    }

    /// Interpolates evaluations at `shift * omega^i` back to coefficients, in
    /// place.
    ///
    /// # Errors
    ///
    /// [`FftError::WrongLength`] unless `a.len() == n`;
    /// [`FftError::ZeroShift`] for `shift = 0` (checked before `a` is
    /// touched).
    pub fn coset_ifft(&self, a: &mut [F], shift: F) -> Result<(), FftError> {
        self.coset_ifft_cancellable(a, shift, None)
    }

    /// Runs [`Self::coset_ifft`] with an explicit per-operation cancellation signal.
    /// All Rayon tasks join before returning. After cancellation the buffer is
    /// incomplete and must be discarded or reinitialized by its owner.
    ///
    /// # Errors
    /// As [`Self::coset_ifft`], or [`FftError::Cancelled`].
    pub fn coset_ifft_cancellable(
        &self,
        a: &mut [F],
        shift: F,
        cancellation: Option<&CancellationToken>,
    ) -> Result<(), FftError> {
        CancellationToken::checkpoint(cancellation)?;
        self.check(a)?;
        check_shift(&shift)?;
        // `shift` is nonzero, so the inverse exists.
        let shift_inv = shift.invert().unwrap_or(F::ZERO);
        dif::<_, true>(a, &self.inverse, None, cancellation);
        bit_reverse_scale::<_, true>(a, self.k, Some((self.n_inv, shift_inv)), cancellation);
        CancellationToken::checkpoint(cancellation)?;
        Ok(())
    }
}

/// Rejects a zero coset shift.
fn check_shift<F: PastaField>(shift: &F) -> Result<(), FftError> {
    if bool::from(shift.is_zero()) {
        Err(FftError::ZeroShift)
    } else {
        Ok(())
    }
}

/// Runs `transform` on every item with one claiming loop per worker of the
/// caller's pool.
///
/// The loops are not pool tasks: a worker that waits for a task of its own
/// column cannot start a whole other column in the meantime, which would
/// leave the first one waiting for the second. A loop that finds the queue
/// empty ends, and its worker then takes tasks of the columns still running.
/// Every worker runs its loop once and the call returns when all have, so a
/// worker held in a long task outside this batch delays the return until it
/// yields. The queue lock covers only the hand-over of the next item.
fn claim_each<T: Send>(items: impl Iterator<Item = T> + Send, transform: impl Fn(T) + Sync) {
    let mut items = items.peekable();
    if items.peek().is_none() {
        // An empty batch wakes no worker.
        return;
    }
    let queue = Mutex::new(items);
    rayon::broadcast(|_| {
        loop {
            // A poisoned queue ends the loop: a transform has panicked and the
            // broadcast reports it.
            let next = queue.lock().ok().and_then(|mut queue| queue.next());
            let Some(item) = next else {
                return;
            };
            transform(item);
        }
    });
}

/// Butterflies per task of a parallel pass over `n` elements.
fn pass_task(n: usize) -> usize {
    (n / 4 / (rayon::current_num_threads() * TASKS_PER_WORKER)).max(PASS_TASK_MIN)
}

/// `tw[s][j] = w^j` with `w` the primitive `2^(s+1)`-th root, for `j < 2^s`.
fn stage_twiddles<F: PastaField>(omega: F, k: u32) -> Vec<Vec<F>> {
    (0..k)
        .map(|s| {
            // w_{2h} = omega^(n / 2h) = omega^(2^(k - s - 1)).
            let w = omega.pow_vartime([1u64 << (k - s - 1)]);
            let h = 1usize << s;
            let mut v = Vec::with_capacity(h);
            let mut cur = F::ONE;
            for _ in 0..h {
                v.push(cur);
                cur *= w;
            }
            v
        })
        .collect()
}

/// Decimation-in-frequency transform; leaves the output in bit-reversed order.
///
/// With `shift`, coefficient `i` is multiplied by `shift^i` while the first
/// pass loads it.
fn dif<F: PastaField, const PARALLEL: bool>(
    a: &mut [F],
    tw: &[Vec<F>],
    shift: Option<CosetShift<'_, F>>,
    cancellation: Option<&CancellationToken>,
) {
    let n = a.len();
    if n <= 1 {
        // A single coefficient is its own evaluation; shift^0 = 1.
        return;
    }
    dif_stages::<_, PARALLEL>(a, tw, shift, tw.len(), cancellation);
}

/// Finish a transform after any already-completed outer stages.
fn dif_stages<F: PastaField, const PARALLEL: bool>(
    a: &mut [F],
    tw: &[Vec<F>],
    shift: Option<CosetShift<'_, F>>,
    mut s: usize,
    cancellation: Option<&CancellationToken>,
) {
    let n = a.len();
    let mut first = true;
    while s >= 2 {
        if CancellationToken::checkpoint(cancellation).is_err() {
            return;
        }
        let block = 1usize << s;
        // A coset shift belongs to the whole array's first pass; every later
        // stage keeps within blocks of 2^s, which finish while still in cache.
        // A block runs its own stages pass by pass: it is already in cache,
        // and splitting it again would only add calls.
        if n > DEPTH_FIRST_ELEMENTS
            && block < n
            && block <= DEPTH_FIRST_ELEMENTS
            && (!first || shift.is_none())
        {
            if PARALLEL && n >= PARALLEL_MIN {
                let leaf = (n / (rayon::current_num_threads() * TASKS_PER_WORKER))
                    .clamp(LEAF_MIN, DEPTH_FIRST_ELEMENTS);
                a.par_chunks_exact_mut(block)
                    .with_max_len(1)
                    .for_each(|chunk| dif_block_shared(chunk, tw, s, leaf, cancellation));
            } else {
                a.chunks_exact_mut(block)
                    .for_each(|chunk| dif_stages::<F, false>(chunk, tw, None, s, cancellation));
            }
            return;
        }
        // Fused stages with half sizes 2m (outer) and m (inner).
        let m = 1usize << (s - 2);
        let outer = &tw[s - 1]; // w_{4m}^j, j < 2m
        let inner = &tw[s - 2]; // w_{2m}^j, j < m
        let pass_shift = if first { shift } else { None };
        dif_pass4::<_, PARALLEL>(a, m, outer, inner, pass_shift, cancellation);
        first = false;
        s -= 2;
    }
    if s == 1 {
        // Final radix-2 stage with half size 1: twiddle 1.
        if let (true, Some(shift)) = (first, shift) {
            // k == 1: apply the shift to coefficient 1 before the stage.
            a[1] *= match shift {
                CosetShift::Geometric(value) => value,
                CosetShift::Prepared(powers) => powers[1],
            };
        }
        let body = |chunk: &mut [F]| {
            if CancellationToken::checkpoint(cancellation).is_err() {
                return;
            }
            for pair in chunk.as_chunks_mut::<2>().0 {
                let (x, y) = (pair[0], pair[1]);
                pair[0] = x + y;
                pair[1] = x - y;
            }
        };
        if PARALLEL && n >= PARALLEL_MIN {
            a.par_chunks_mut(PARALLEL_MIN).for_each(body);
        } else {
            body(a);
        }
    }
}

/// Finishes one depth-first block of `2^s` elements. A block larger than
/// `leaf` runs its first two stages on the worker that holds it, still in that
/// worker's cache, and then offers its quarters to the pool, so an idle worker
/// can take over most of a block that a slower worker started. Quarters are
/// independent from that stage on: only the order of exact butterflies changes.
fn dif_block_shared<F: PastaField>(
    a: &mut [F],
    tw: &[Vec<F>],
    s: usize,
    leaf: usize,
    cancellation: Option<&CancellationToken>,
) {
    if a.len() <= leaf || s < 4 {
        dif_stages::<F, false>(a, tw, None, s, cancellation);
        return;
    }
    if CancellationToken::checkpoint(cancellation).is_err() {
        return;
    }
    let m = 1usize << (s - 2);
    dif_pass4::<F, false>(a, m, &tw[s - 1], &tw[s - 2], None, cancellation);
    a.par_chunks_exact_mut(m)
        .with_max_len(1)
        .for_each(|quarter| dif_block_shared(quarter, tw, s - 2, leaf, cancellation));
}

/// Shared two-stage butterfly; preserves the exact in-place operation order.
// Keep the field arrays in the caller's loop rather than passing them through
// an outlined helper's stack frame for every butterfly.
#[allow(
    clippy::inline_always,
    reason = "the measured butterfly kernel avoids per-call field-array copies"
)]
#[inline(always)]
fn dif_butterfly4<F: PastaField>(
    [a0, a1, a2, a3]: [F; 4],
    m: usize,
    outer: &[F],
    inner: &[F],
    j: usize,
) -> [F; 4] {
    let y0 = a0 + a2;
    let y1 = a1 + a3;
    let (y2, y3) = if j == 0 {
        (a0 - a2, (a1 - a3) * outer[m])
    } else {
        ((a0 - a2) * outer[j], (a1 - a3) * outer[j + m])
    };
    let r0 = y0 + y1;
    let r2 = y2 + y3;
    let (r1, r3) = if j == 0 {
        (y0 - y1, y2 - y3)
    } else {
        let w = inner[j];
        ((y0 - y1) * w, (y2 - y3) * w)
    };
    [r0, r1, r2, r3]
}

/// Reads the first pass directly from immutable coefficients into its output.
fn dif_from<F: PastaField, const PARALLEL: bool>(
    coefficients: &[F],
    output: &mut [F],
    tw: &[Vec<F>],
    powers: &[F],
    cancellation: Option<&CancellationToken>,
) {
    if CancellationToken::checkpoint(cancellation).is_err() {
        return;
    }
    let n = output.len();
    if n == 1 {
        output[0] = coefficients[0];
        return;
    }
    if n == 2 {
        let x = coefficients[0];
        let y = coefficients[1] * powers[1];
        output[0] = x + y;
        output[1] = x - y;
        return;
    }
    let k = tw.len();
    let m = n / 4;
    let outer = &tw[k - 1];
    let inner = &tw[k - 2];
    let butterfly = |q0: &mut [F], q1: &mut [F], q2: &mut [F], q3: &mut [F], j0: usize| {
        for (off, (((x0, x1), x2), x3)) in q0
            .iter_mut()
            .zip(q1.iter_mut())
            .zip(q2.iter_mut())
            .zip(q3.iter_mut())
            .enumerate()
        {
            if off % 1024 == 0 && CancellationToken::checkpoint(cancellation).is_err() {
                return;
            }
            let j = j0 + off;
            let a = [
                coefficients[j] * powers[j],
                coefficients[j + m] * powers[j + m],
                coefficients[j + 2 * m] * powers[j + 2 * m],
                coefficients[j + 3 * m] * powers[j + 3 * m],
            ];
            let [r0, r1, r2, r3] = dif_butterfly4(a, m, outer, inner, j);
            *x0 = r0;
            *x1 = r1;
            *x2 = r2;
            *x3 = r3;
        }
    };
    let (q0, rest) = output.split_at_mut(m);
    let (q1, rest) = rest.split_at_mut(m);
    let (q2, q3) = rest.split_at_mut(m);
    if PARALLEL && n >= PARALLEL_MIN {
        // Same scheduling as the single large block of the in-place first pass.
        let task = pass_task(n).min(m);
        q0.par_chunks_mut(task)
            .zip(q1.par_chunks_mut(task))
            .zip(q2.par_chunks_mut(task))
            .zip(q3.par_chunks_mut(task))
            .enumerate()
            .with_max_len(1)
            .for_each(|(c, (((c0, c1), c2), c3))| butterfly(c0, c1, c2, c3, c * task));
    } else {
        butterfly(q0, q1, q2, q3, 0);
    }
    dif_stages::<_, PARALLEL>(output, tw, None, k - 2, cancellation);
}

/// One fused DIF pass over blocks of `4m`.
fn dif_pass4<F: PastaField, const PARALLEL: bool>(
    a: &mut [F],
    m: usize,
    outer: &[F],
    inner: &[F],
    shift: Option<CosetShift<'_, F>>,
    cancellation: Option<&CancellationToken>,
) {
    let n = a.len();
    let block = 4 * m;
    // The shift only occurs in the first pass, where the block is the whole
    // array, so element j + q*m sits at original index j + q*m.
    let quarter_shift = match shift {
        Some(CosetShift::Geometric(sh)) => {
            let sm = sh.pow_vartime([m as u64]);
            let s2m = sm.square();
            Some((sh, [F::ONE, sm, s2m, s2m * sm]))
        }
        _ => None,
    };
    let powers = match shift {
        Some(CosetShift::Prepared(powers)) => Some(powers),
        _ => None,
    };
    let butterfly = |q0: &mut [F], q1: &mut [F], q2: &mut [F], q3: &mut [F], j0: usize| {
        let mut sj = quarter_shift.map(|(sh, _)| sh.pow_vartime([j0 as u64]));
        for (off, (((x0, x1), x2), x3)) in q0
            .iter_mut()
            .zip(q1.iter_mut())
            .zip(q2.iter_mut())
            .zip(q3.iter_mut())
            .enumerate()
        {
            if off % 1024 == 0 && CancellationToken::checkpoint(cancellation).is_err() {
                return;
            }
            let j = j0 + off;
            let (mut a0, mut a1, mut a2, mut a3) = (*x0, *x1, *x2, *x3);
            if let (Some(s), Some((sh, c))) = (sj.as_mut(), quarter_shift.as_ref()) {
                a0 *= *s;
                a1 *= *s * c[1];
                a2 *= *s * c[2];
                a3 *= *s * c[3];
                *s *= sh;
            }
            if let Some(powers) = powers {
                a0 *= powers[j];
                a1 *= powers[j + m];
                a2 *= powers[j + 2 * m];
                a3 *= powers[j + 3 * m];
            }
            let [r0, r1, r2, r3] = dif_butterfly4([a0, a1, a2, a3], m, outer, inner, j);
            *x0 = r0;
            *x1 = r1;
            *x2 = r2;
            *x3 = r3;
        }
    };
    let run = |blk: &mut [F]| {
        let (q0, rest) = blk.split_at_mut(m);
        let (q1, rest) = rest.split_at_mut(m);
        let (q2, q3) = rest.split_at_mut(m);
        butterfly(q0, q1, q2, q3, 0);
    };
    if !PARALLEL || n < PARALLEL_MIN {
        a.chunks_exact_mut(block).for_each(run);
        return;
    }
    // One region per pass, in tasks of `task` butterflies each.
    let task = pass_task(n);
    if m <= task {
        // A quarter no larger than a task cannot split: whole blocks share one.
        a.par_chunks_mut(block * (task / m))
            .with_max_len(1)
            .for_each(|blocks| blocks.chunks_exact_mut(block).for_each(&run));
    } else {
        // Larger blocks each offer their j range; no block waits for another.
        a.par_chunks_exact_mut(block)
            .with_max_len(1)
            .for_each(|blk| {
                let (q0, rest) = blk.split_at_mut(m);
                let (q1, rest) = rest.split_at_mut(m);
                let (q2, q3) = rest.split_at_mut(m);
                q0.par_chunks_mut(task)
                    .zip(q1.par_chunks_mut(task))
                    .zip(q2.par_chunks_mut(task))
                    .zip(q3.par_chunks_mut(task))
                    .enumerate()
                    .with_max_len(1)
                    .for_each(|(c, (((c0, c1), c2), c3))| butterfly(c0, c1, c2, c3, c * task));
            });
    }
}

/// Bit-reverses the order of `a` (length `2^k`) and, with `scale = (c, s)`,
/// multiplies the element at natural index `i` by `c * s^i`.
fn bit_reverse_scale<F: PastaField, const PARALLEL: bool>(
    a: &mut [F],
    k: u32,
    scale: Option<(F, F)>,
    cancellation: Option<&CancellationToken>,
) {
    let n = a.len();
    if k > 0 {
        let shift = usize::BITS - k;
        for i in 0..n {
            if i % 4096 == 0 && CancellationToken::checkpoint(cancellation).is_err() {
                return;
            }
            let r = i.reverse_bits() >> shift;
            if i < r {
                a.swap(i, r);
            }
        }
    }
    if let Some((c, s)) = scale {
        let unit_shift = s == F::ONE;
        let body = |(ci, chunk): (usize, &mut [F])| {
            if CancellationToken::checkpoint(cancellation).is_err() {
                return;
            }
            if unit_shift {
                // Plain inverse transform: one multiplication per element.
                for x in chunk {
                    *x *= c;
                }
                return;
            }
            let start = ci * PARALLEL_MIN;
            let mut f = c * s.pow_vartime([start as u64]);
            for x in chunk {
                *x *= f;
                f *= s;
            }
        };
        if PARALLEL && n >= PARALLEL_MIN {
            a.par_chunks_mut(PARALLEL_MIN).enumerate().for_each(body);
        } else {
            a.chunks_mut(PARALLEL_MIN).enumerate().for_each(body);
        }
    }
}

/// Reference `O(n^2)` evaluation of `a` at `shift * omega^i` (tests and
/// differential checks only).
pub fn naive_coset_dft<F: PastaField>(a: &[F], omega: F, shift: F) -> Vec<F> {
    let n = a.len();
    let mut out = Vec::with_capacity(n);
    let mut x = shift;
    for _ in 0..n {
        // Horner evaluation at x.
        let v = a.iter().rev().fold(F::ZERO, |acc, c| acc * x + c);
        out.push(v);
        x *= omega;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::{Fp, Fq};
    use ff::Field;
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;

    fn random_vec<F: PastaField>(n: usize, seed: u64) -> Vec<F> {
        let mut rng = ChaCha20Rng::seed_from_u64(seed);
        (0..n).map(|_| F::random(&mut rng)).collect()
    }

    #[test]
    fn small_sizes_match_naive() {
        for k in 0..=9u32 {
            let d = FftDomain::<Fp>::new(k).unwrap();
            let a = random_vec::<Fp>(1 << k, u64::from(k));
            let mut b = a.clone();
            d.fft(&mut b).unwrap();
            assert_eq!(b, naive_coset_dft(&a, d.omega(), Fp::ONE), "k = {k}");
            d.ifft(&mut b).unwrap();
            assert_eq!(b, a);
            let shift = <Fp as ff::WithSmallOrderMulGroup<3>>::ZETA;
            let mut c = a.clone();
            d.coset_fft(&mut c, shift).unwrap();
            assert_eq!(c, naive_coset_dft(&a, d.omega(), shift), "coset k = {k}");
            d.coset_ifft(&mut c, shift).unwrap();
            assert_eq!(c, a);
        }
    }

    /// Every output of a monomial `x^r` is exact: `(shift * omega^i)^r`. A
    /// transform is linear, so monomials of several degrees pin full-size
    /// transforms, through the passes over the whole array and the blocks
    /// that finish depth first, in O(n) per check.
    #[test]
    fn full_size_transforms_evaluate_monomials_exactly() {
        fn check<F: PastaField>() {
            for workers in [1, 4] {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(workers)
                    .build()
                    .unwrap();
                pool.install(|| {
                    for k in [12u32, 13, 14, 16, 17] {
                        let domain = FftDomain::<F>::new(k).unwrap();
                        let n = domain.n();
                        let degrees = [1usize, 3, 4095, 4097, n / 2 + 5, n - 1];
                        for degree in degrees.into_iter().filter(|&degree| degree < n) {
                            for shift in [F::ONE, F::from(7)] {
                                let mut monomial = vec![F::ZERO; n];
                                monomial[degree] = F::ONE;
                                let mut single = monomial.clone();
                                domain.coset_fft(&mut single, shift).unwrap();
                                let mut batch = vec![monomial.clone(), monomial.clone()];
                                let mut columns: Vec<_> =
                                    batch.iter_mut().map(Vec::as_mut_slice).collect();
                                domain.coset_fft_many(&mut columns, shift).unwrap();
                                // (shift * omega^i)^r = shift^r * (omega^r)^i.
                                let step = domain.omega().pow_vartime([degree as u64]);
                                let mut expected = shift.pow_vartime([degree as u64]);
                                for (i, value) in single.iter().enumerate() {
                                    assert_eq!(*value, expected, "k={k} degree={degree} i={i}");
                                    expected *= step;
                                }
                                assert_eq!(batch[0], single);
                                assert_eq!(batch[1], single);
                                domain.coset_ifft(&mut single, shift).unwrap();
                                assert_eq!(single, monomial);
                            }
                        }
                    }
                });
            }
        }
        check::<Fp>();
        check::<Fq>();
    }

    #[test]
    fn batched_cosets_match_single_transforms_and_naive_dft() {
        fn check<F: PastaField>() {
            for workers in [1, 4] {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(workers)
                    .build()
                    .unwrap();
                pool.install(|| {
                    for k in [0, 1, 2, 3, 4, 7, 12, 13] {
                        let domain = FftDomain::<F>::new(k).unwrap();
                        for count in [0u64, 1, 3, 17] {
                            let original: Vec<Vec<F>> = (0..count)
                                .map(|index| random_vec(domain.n(), index + 97))
                                .collect();
                            for shift in [F::ONE, F::from(7)] {
                                let mut batch = original.clone();
                                let addresses: Vec<_> = batch.iter().map(Vec::as_ptr).collect();
                                let mut columns: Vec<_> =
                                    batch.iter_mut().map(Vec::as_mut_slice).collect();
                                domain.coset_fft_many(&mut columns, shift).unwrap();
                                for (index, (values, source)) in
                                    batch.iter_mut().zip(&original).enumerate()
                                {
                                    let mut single = source.clone();
                                    domain.coset_fft(&mut single, shift).unwrap();
                                    assert_eq!(*values, single, "k={k} count={count}");
                                    assert_eq!(values.as_ptr(), addresses[index]);
                                    if k <= 4 {
                                        assert_eq!(
                                            *values,
                                            naive_coset_dft(source, domain.omega(), shift)
                                        );
                                    }
                                    domain.coset_ifft(values, shift).unwrap();
                                    assert_eq!(values, source);
                                }
                            }
                        }
                    }
                });
            }
        }
        check::<Fp>();
        check::<Fq>();
    }

    /// A batch gives every worker a claiming loop and every transform splits
    /// its own passes. Batches narrower than the pool, wider than it by one, of
    /// exactly its width, and batches started by tasks of the same pool must
    /// all produce the outputs of single transforms.
    #[test]
    fn batches_of_every_width_match_single_transforms() {
        fn batch_of<F: PastaField>(source: &[Vec<F>]) -> Vec<Vec<F>> {
            source.to_vec()
        }
        fn check<F: PastaField>() {
            for (workers, k) in [(1usize, 13u32), (3, 13), (3, 16), (8, 13)] {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(workers)
                    .build()
                    .unwrap();
                pool.install(|| {
                    let domain = FftDomain::<F>::new(k).unwrap();
                    let shift = F::from(7);
                    let widest = 2 * workers + 1;
                    let source: Vec<Vec<F>> = (0..widest as u64)
                        .map(|index| random_vec(domain.n(), 31 * u64::from(k) + index))
                        .collect();
                    let expected: Vec<Vec<F>> = source
                        .iter()
                        .map(|column| {
                            let mut single = column.clone();
                            domain.coset_fft(&mut single, shift).unwrap();
                            single
                        })
                        .collect();
                    let mut powers = vec![F::ZERO; domain.n()];
                    let plan = domain.coset_plan(&mut powers, shift).unwrap();
                    for width in [2, workers.max(2), workers + 1, widest] {
                        let what = format!("k={k} workers={workers} width={width}");
                        let mut outputs = vec![vec![F::ZERO; domain.n()]; width];
                        let mut columns: Vec<_> =
                            outputs.iter_mut().map(Vec::as_mut_slice).collect();
                        plan.fft_many_from_cancellable(&source[..width], &mut columns, None)
                            .unwrap();
                        assert!(
                            outputs[..] == expected[..width],
                            "from coefficients, {what}"
                        );
                        let mut batch = batch_of(&source[..width]);
                        let mut columns: Vec<_> = batch.iter_mut().map(Vec::as_mut_slice).collect();
                        plan.fft_many(&mut columns).unwrap();
                        assert!(batch[..] == expected[..width], "prepared in place, {what}");
                        let mut batch = batch_of(&source[..width]);
                        let mut columns: Vec<_> = batch.iter_mut().map(Vec::as_mut_slice).collect();
                        domain.coset_fft_many(&mut columns, shift).unwrap();
                        assert!(batch[..] == expected[..width], "in place, {what}");
                    }
                    // Two batches started by two tasks of this pool at once.
                    let (mut left, mut right) = (batch_of(&source[..3]), batch_of(&source[..3]));
                    rayon::join(
                        || {
                            let mut columns: Vec<_> =
                                left.iter_mut().map(Vec::as_mut_slice).collect();
                            domain.coset_fft_many(&mut columns, shift).unwrap();
                        },
                        || {
                            let mut columns: Vec<_> =
                                right.iter_mut().map(Vec::as_mut_slice).collect();
                            domain.coset_fft_many(&mut columns, shift).unwrap();
                        },
                    );
                    assert!(left[..] == expected[..3], "left of a join, k={k}");
                    assert!(right[..] == expected[..3], "right of a join, k={k}");
                });
            }
        }
        check::<Fp>();
        check::<Fq>();
    }

    /// A batch of inverse transforms writes, at every width and on every pool
    /// size, what one inverse transform per column writes, and refuses a wrong
    /// length before it changes any column.
    #[test]
    fn inverse_batches_match_single_inverse_transforms() {
        fn check<F: PastaField>() {
            for (workers, k) in [(1usize, 13u32), (3, 13), (3, 16), (8, 13)] {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(workers)
                    .build()
                    .unwrap();
                pool.install(|| {
                    let domain = FftDomain::<F>::new(k).unwrap();
                    let widest = 2 * workers + 1;
                    let source: Vec<Vec<F>> = (0..widest as u64)
                        .map(|index| random_vec(domain.n(), 53 * u64::from(k) + index))
                        .collect();
                    let expected: Vec<Vec<F>> = source
                        .iter()
                        .map(|column| {
                            let mut single = column.clone();
                            domain.ifft(&mut single).unwrap();
                            single
                        })
                        .collect();
                    for width in [0, 1, 2, workers.max(2), workers + 1, widest] {
                        let mut batch = source[..width].to_vec();
                        let mut columns: Vec<_> = batch.iter_mut().map(Vec::as_mut_slice).collect();
                        domain.ifft_many(&mut columns).unwrap();
                        assert!(
                            batch[..] == expected[..width],
                            "k={k} workers={workers} width={width}"
                        );
                    }
                    let mut batch = source[..3].to_vec();
                    batch[1].pop();
                    let before = batch.clone();
                    let mut columns: Vec<_> = batch.iter_mut().map(Vec::as_mut_slice).collect();
                    assert_eq!(
                        domain.ifft_many(&mut columns),
                        Err(FftError::WrongLength {
                            expected: domain.n(),
                            actual: domain.n() - 1
                        })
                    );
                    assert!(batch == before, "a refused batch changed a column");
                });
            }
        }
        check::<Fp>();
        check::<Fq>();
    }

    #[test]
    fn batched_coset_errors_leave_every_column_untouched() {
        fn check<F: PastaField>() {
            let domain = FftDomain::<F>::new(3).unwrap();
            let original = vec![random_vec::<F>(8, 1), random_vec(8, 2), random_vec(8, 3)];
            for bad_index in 0..original.len() {
                let mut batch = original.clone();
                batch[bad_index].pop();
                let before = batch.clone();
                let mut columns: Vec<_> = batch.iter_mut().map(Vec::as_mut_slice).collect();
                // Validate the entire batch before starting even the first transform.
                assert_eq!(
                    domain.coset_fft_many(&mut columns, F::ZERO),
                    Err(FftError::WrongLength {
                        expected: 8,
                        actual: 7
                    })
                );
                assert_eq!(batch, before);
            }
            let mut batch = original.clone();
            let mut columns: Vec<_> = batch.iter_mut().map(Vec::as_mut_slice).collect();
            assert_eq!(
                domain.coset_fft_many(&mut columns, F::ZERO),
                Err(FftError::ZeroShift)
            );
            assert_eq!(batch, original);
            assert_eq!(
                domain.coset_fft_many(&mut [], F::ZERO),
                Err(FftError::ZeroShift)
            );
            assert_eq!(domain.coset_fft_many(&mut [], F::ONE), Ok(()));
        }
        check::<Fp>();
        check::<Fq>();
    }

    #[test]
    fn coset_plan_reports_exact_domain_length() {
        fn check<F: PastaField>() {
            for k in [0, 1, 4, 13] {
                let domain = FftDomain::<F>::new(k).unwrap();
                let mut powers = vec![F::ZERO; domain.n()];
                let plan = domain.coset_plan(&mut powers, F::from(7)).unwrap();
                assert_eq!(plan.n(), 1 << k);
            }
        }
        check::<Fp>();
        check::<Fq>();
    }

    #[test]
    fn domain_parameters() {
        let d = FftDomain::<Fq>::new(10).unwrap();
        assert_eq!(d.n(), 1024);
        assert_eq!(d.k(), 10);
        assert_eq!(d.omega().pow_vartime([1024u64]), Fq::ONE);
        assert_ne!(d.omega().pow_vartime([512u64]), Fq::ONE);
        assert_eq!(d.omega() * d.omega_inv(), Fq::ONE);
        assert_eq!(d.n_inv() * Fq::from(1024u64), Fq::ONE);
        assert!(d.twiddle_bytes() > 0);
        assert_eq!(
            FftDomain::<Fq>::new(33).unwrap_err(),
            FftError::UnsupportedSize { k: 33 }
        );
        let mut short = vec![Fq::ZERO; 3];
        assert_eq!(
            d.fft(&mut short),
            Err(FftError::WrongLength {
                expected: 1024,
                actual: 3
            })
        );
        assert_eq!(
            FftError::UnsupportedSize { k: 40 }.to_string(),
            "unsupported FFT size 2^40"
        );
    }

    #[test]
    fn zero_coset_shift_is_rejected_without_touching_the_input() {
        let d = FftDomain::<Fp>::new(3).unwrap();
        let a = random_vec::<Fp>(8, 21);
        let mut b = a.clone();
        assert_eq!(d.coset_ifft(&mut b, Fp::ZERO), Err(FftError::ZeroShift));
        assert_eq!(b, a);
        assert_eq!(d.coset_fft(&mut b, Fp::ZERO), Err(FftError::ZeroShift));
        assert_eq!(b, a);
        assert_eq!(FftError::ZeroShift.to_string(), "FFT coset shift is zero");
        assert_eq!(check_shift(&Fp::ONE), Ok(()));
        // Length errors take precedence, as before.
        assert!(matches!(
            d.coset_ifft(&mut b[..3], Fp::ZERO),
            Err(FftError::WrongLength { .. })
        ));
    }

    #[test]
    fn stage_twiddles_are_powers() {
        let w = FftDomain::<Fp>::new(4).unwrap().omega();
        let tw = stage_twiddles(w, 4);
        assert_eq!(tw.len(), 4);
        assert_eq!(tw[3][1], w);
        assert_eq!(tw[0], vec![Fp::ONE]);
    }

    #[test]
    fn bit_reverse_scale_permutes() {
        let mut a: Vec<Fp> = (0..8u64).map(Fp::from).collect();
        bit_reverse_scale::<_, true>(&mut a, 3, None, None);
        let expected: Vec<Fp> = [0u64, 4, 2, 6, 1, 5, 3, 7]
            .into_iter()
            .map(Fp::from)
            .collect();
        assert_eq!(a, expected);
        let mut b = vec![Fp::ONE; 4];
        bit_reverse_scale::<_, true>(&mut b, 2, Some((Fp::from(3u64), Fp::from(2u64))), None);
        assert_eq!(
            b,
            vec![
                Fp::from(3u64),
                Fp::from(6u64),
                Fp::from(12u64),
                Fp::from(24u64)
            ]
        );
    }

    #[test]
    fn dif_pass_and_naive_dft_agree_on_impulse() {
        let d = FftDomain::<Fp>::new(3).unwrap();
        let mut a = vec![Fp::ZERO; 8];
        a[1] = Fp::ONE;
        d.fft(&mut a).unwrap();
        let mut x = Fp::ONE;
        for v in a {
            assert_eq!(v, x);
            x *= d.omega();
        }
        let mut b = vec![Fp::ONE; 8];
        dif::<_, true>(&mut b, &stage_twiddles(d.omega(), 3), None, None);
        assert_eq!(b[0], Fp::from(8u64));
    }
}

#[cfg(test)]
#[path = "fft_plan_tests.rs"]
mod plan_tests;
