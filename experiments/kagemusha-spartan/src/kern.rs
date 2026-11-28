//! Prover kernels of the committed sumchecks (review lane ebb983).
//!
//! Same round values and folded tables as `spartan::round_values` and
//! `spartan::fold_top`, as exact field elements, with less work per row:
//!
//! - Round polynomials are accumulated on `{0, 1, inf}^k` (the value at
//!   infinity is the leading coefficient along that axis) and converted to the
//!   committed grid `{0, 1, 2}^k` once per round. Extending a multilinear
//!   table costs one subtraction per new point instead of two operations, and
//!   a product's value at infinity is the product of the two slopes.
//! - Outer rounds weigh `A` by `eq` once per corner (8 products for k = 3)
//!   instead of once per grid point (27). `C` is multilinear, so it has no
//!   leading coefficient: it only enters the Boolean points.
//! - In the first outer round of a satisfied relation every Boolean point is
//!   zero, so an honest prover skips those points and `C` altogether.
//! - Inner rounds read which corners of the round's variables may hold
//!   anything, per run of 256 rows ([`support`]), from public information
//!   only: the public half of `z` is zero beyond the public inputs, rows the
//!   witness build never wrote are zero, and `M(r_x, .)` is public. A variable
//!   whose upper (or lower) half is empty in both tables contributes the
//!   factor `(1 - Y)^2` (or `Y^2`), so the round runs on the remaining
//!   sub-cube (9 products instead of 27 for the public half alone, 3 where
//!   half the rows are empty too), and so does its fold. Later rounds carry
//!   the folded witness's empty runs forward ([`Support::folded_live`]).
//! - Tables fold by a tree (`2^k - 1` products per output instead of `2^k`).
//! - Every loop is cut into leaves of bounded length, so a slow core never
//!   holds a large share of a pass (unequal cores: a Pixel 6's 2 + 2 + 4).
//!
//! Private values never decide the schedule. Which rows, corners and
//! products a pass touches follows from the shape, the witness build's public
//! row account, public challenges and public tables; a private entry is never
//! tested for zero (a witness run that happens to be zero is evaluated like
//! any other), and the outer tables `Az, Bz, Cz` are always dense. The field
//! arithmetic itself is the Pasta implementation's constant-time arithmetic.
//! (The commitment MSMs are not constant time in the witness: TODO.md G8.)

use ff::Field;
use rayon::prelude::*;

use crate::grid::{boolean_positions, pow3};

/// Longest leaf of a parallel pass, in rows of the folded table.
const LEAF: usize = 1 << 10;

/// A table the inner rounds read: a slice of private values, a [`Public`]
/// slice, a [`Split`] `M(r_x, .)`, the prover's [`ZView`] of `z`, or a folded
/// private table with its public [`Live`] runs ([`Masked`]).
pub trait Table<F>: Sync {
    fn len(&self) -> usize;
    fn at(&self, i: usize) -> F;
    /// Whether every entry in `range` is zero by public information alone.
    ///
    /// A public table may read its values. A table of private values answers
    /// from its public layout only (rows the witness build never wrote, the
    /// public half beyond the io, runs an earlier fold emptied by layout) and
    /// never reads a private value to decide. `false` is always correct: the
    /// round then evaluates the run as live, with the same result.
    fn zero_run(&self, range: std::ops::Range<usize>) -> bool;
}

/// A plain slice is private: no entry is known to be zero, so a supported
/// round evaluates every run of it ([`Public`] for a public slice).
impl<F: Field> Table<F> for [F] {
    #[inline]
    fn len(&self) -> usize {
        <[F]>::len(self)
    }
    #[inline]
    fn at(&self, i: usize) -> F {
        self[i]
    }
    fn zero_run(&self, range: std::ops::Range<usize>) -> bool {
        range.is_empty()
    }
}

impl<F: Field> Table<F> for Vec<F> {
    #[inline]
    fn len(&self) -> usize {
        self.as_slice().len()
    }
    #[inline]
    fn at(&self, i: usize) -> F {
        self[i]
    }
    fn zero_run(&self, range: std::ops::Range<usize>) -> bool {
        self.as_slice().zero_run(range)
    }
}

/// A slice of public values (a folded `M(r_x, .)`): its zero runs are read
/// from its values.
pub struct Public<'a, F>(pub &'a [F]);

impl<F: Field> Table<F> for Public<'_, F> {
    #[inline]
    fn len(&self) -> usize {
        self.0.len()
    }
    #[inline]
    fn at(&self, i: usize) -> F {
        self.0[i]
    }
    fn zero_run(&self, range: std::ops::Range<usize>) -> bool {
        self.0[range].iter().all(|x| x.is_zero_vartime())
    }
}

/// Which runs of a private table may hold anything, by public layout alone:
/// run `s` covers entries `s * seg..(s + 1) * seg`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Live {
    seg: usize,
    runs: Vec<bool>,
}

impl Live {
    /// Every run of a table of `len` entries may hold anything.
    pub fn dense(len: usize) -> Self {
        let seg = SEGMENT.min(len).max(1);
        Live {
            seg,
            runs: vec![true; len.div_ceil(seg)],
        }
    }
    /// Whether no run meeting `range` may hold anything.
    fn zero_run(&self, range: std::ops::Range<usize>) -> bool {
        range.is_empty()
            || self.runs[range.start / self.seg..=(range.end - 1) / self.seg]
                .iter()
                .all(|live| !live)
    }
}

/// A table of private values with its public [`Live`] runs (`z` after an
/// inner fold): its zero runs come from the runs, never from its values.
pub struct Masked<'a, F> {
    values: &'a [F],
    live: &'a Live,
}

impl<'a, F> Masked<'a, F> {
    pub fn new(values: &'a [F], live: &'a Live) -> Self {
        assert_eq!(
            live.runs.len(),
            values.len().div_ceil(live.seg),
            "a live flag per run"
        );
        Masked { values, live }
    }
}

impl<F: Field> Table<F> for Masked<'_, F> {
    #[inline]
    fn len(&self) -> usize {
        self.values.len()
    }
    #[inline]
    fn at(&self, i: usize) -> F {
        self.values[i]
    }
    fn zero_run(&self, range: std::ops::Range<usize>) -> bool {
        self.live.zero_run(range)
    }
}

/// `(dense || tail || 0...)` of length `2 * dense.len()`: `M(r_x, .)` before
/// the first inner fold. Its upper (public) half is zero beyond the public
/// inputs, so it is never allocated. Its values are public (the verifier
/// evaluates the same `M(r_x, r_y)`), so its zero runs are read by value; a
/// private table never takes this form (the prover's `z` is a [`ZView`]).
pub struct Split<F> {
    pub dense: Vec<F>,
    pub tail: Vec<F>,
    /// Which runs of [`SEGMENT`] dense entries were written (bit `s % 64` of
    /// word `s / 64`); a run never written is zero and is never read, so a
    /// zero-on-demand dense half holds memory only where written. `None`:
    /// every run may hold anything.
    pub written: Option<Vec<u64>>,
}

#[inline]
fn run_written(written: Option<&[u64]>, run: usize) -> bool {
    written.is_none_or(|bits| bits[run / 64] >> (run % 64) & 1 == 1)
}

impl<F: Field> Split<F> {
    pub fn new(dense: Vec<F>, tail: Vec<F>) -> Self {
        assert!(tail.len() <= dense.len(), "the tail lies in the upper half");
        Split {
            dense,
            tail,
            written: None,
        }
    }
    /// A split table written only in the runs `written` marks.
    pub fn with_written(dense: Vec<F>, tail: Vec<F>, written: Vec<u64>) -> Self {
        assert!(written.len() * 64 * SEGMENT >= dense.len(), "a bit per run");
        Split {
            written: Some(written),
            ..Split::new(dense, tail)
        }
    }
    /// The upper half's nonzero prefix becomes the tail (found by value:
    /// `full` is a public `M(r_x, .)`).
    pub fn from_full(mut full: Vec<F>) -> Self {
        assert!(full.len().is_power_of_two() && full.len() >= 2);
        let half = full.len() / 2;
        let used = full[half..]
            .iter()
            .rposition(|x| !x.is_zero_vartime())
            .map_or(0, |i| i + 1);
        let tail = full[half..half + used].to_vec();
        full.truncate(half);
        full.shrink_to_fit();
        Split::new(full, tail)
    }
    /// Consume the public matrix and fold its first round into its lowest
    /// corner. Each output row only reads that row from every corner, so
    /// different rows can overwrite their first-corner values independently.
    pub fn into_folded(mut self, r: &[F], support: Option<&Support>) -> Vec<F> {
        let Some(support) = support.filter(|_| !r.is_empty() && !self.dense.is_empty()) else {
            return fold_top(&self, r, support);
        };
        let half = self.dense.len();
        let low = self.len() >> r.len();
        assert!(low > 0 && low <= half);
        let (reductions, runs) = fold_reductions(support, r);
        let (first, rest) = self.dense.split_at_mut(low);
        first
            .par_chunks_mut(support.seg)
            .zip_eq(runs.par_iter())
            .enumerate()
            .for_each_init(
                || vec![F::ZERO; 1 << r.len()],
                |scratch, (segment, (chunk, &run))| {
                    if run == u16::MAX {
                        chunk.fill(F::ZERO);
                        return;
                    }
                    let reduction = &reductions[usize::from(run)];
                    for (offset, output) in chunk.iter_mut().enumerate() {
                        let row = segment * support.seg + offset;
                        *output = reduction.evaluate(scratch, |corner| {
                            let index = corner * low + row;
                            if index >= half {
                                self.tail.get(index - half).copied().unwrap_or(F::ZERO)
                            } else if !run_written(self.written.as_deref(), index / SEGMENT) {
                                F::ZERO
                            } else if corner == 0 {
                                *output
                            } else {
                                rest[index - low]
                            }
                        });
                    }
                },
            );
        self.dense.truncate(low);
        self.dense.shrink_to_fit();
        self.dense
    }

    pub fn to_full(&self) -> Vec<F> {
        let half = self.dense.len();
        let mut full = zeroed(2 * half);
        full[..half].copy_from_slice(&self.dense);
        full[half..half + self.tail.len()].copy_from_slice(&self.tail);
        full
    }
}

impl<F: Field> Table<F> for Split<F> {
    #[inline]
    fn len(&self) -> usize {
        2 * self.dense.len()
    }
    #[inline]
    fn at(&self, i: usize) -> F {
        let half = self.dense.len();
        if i >= half {
            self.tail.get(i - half).copied().unwrap_or(F::ZERO)
        } else if run_written(self.written.as_deref(), i / SEGMENT) {
            self.dense[i]
        } else {
            // Never written: zero, and its page is never touched.
            F::ZERO
        }
    }
    fn zero_run(&self, range: std::ops::Range<usize>) -> bool {
        let half = self.dense.len();
        let low = range.start.min(half)..range.end.min(half);
        let tail_end = half + self.tail.len();
        let high = range.start.clamp(half, tail_end)..range.end.clamp(half, tail_end);
        // Runs never written are zero unread; written runs are public values.
        let mut at = low.start;
        while at < low.end {
            let s = at / SEGMENT;
            let end = ((s + 1) * SEGMENT).min(low.end);
            if run_written(self.written.as_deref(), s)
                && !self.dense[at..end].iter().all(|x| x.is_zero_vartime())
            {
                return false;
            }
            at = end;
        }
        self.tail[high.start - half..high.end - half]
            .iter()
            .all(|x| x.is_zero_vartime())
    }
}

/// Read and write access to a witness by flat index: a slice, or the
/// prover's [`Rows`].
pub trait Cells<F> {
    fn cell(&self, i: usize) -> F;
    fn set_cell(&mut self, i: usize, value: F);
}

impl<F: Copy> Cells<F> for [F] {
    #[inline]
    fn cell(&self, i: usize) -> F {
        self[i]
    }
    #[inline]
    fn set_cell(&mut self, i: usize, value: F) {
        self[i] = value;
    }
}

/// The prover's witness as its Hyrax rows. Writes to an immutable caller's
/// authored rows use copy on write. An owning guard lends its authored rows
/// exclusively for masks, the link and the second phase. Rows the
/// circuit says the caller never wrote ([`crate::r1cs::RowFill`]) are never
/// read from the caller's witness, so their pages are never allocated.
pub struct Rows<'a, F> {
    shift: usize,
    rows: Vec<Row<'a, F>>,
    /// A zero row and a padding row (1, 0, ...) for a reader that needs a
    /// never-written row as a slice; allocated on such a read only.
    zero: std::sync::OnceLock<Vec<F>>,
    unit: std::sync::OnceLock<Vec<F>>,
}

enum Row<'a, F> {
    Shared(&'a [F]),
    Exclusive(&'a mut [F]),
    Own(Vec<F>),
    /// All zero.
    Zero,
    /// A padding fixed row: 1 in column 0, zero elsewhere.
    Unit,
}

impl<'a, F: Field> Rows<'a, F> {
    /// `w` in rows of `cols` (a power of two) entries, every one read.
    pub fn new(w: &'a [F], cols: usize) -> Self {
        let rows = w.len() / cols.max(1);
        Self::with_fill(w, cols, &vec![crate::r1cs::RowFill::Values; rows])
    }
    /// `w` in rows of `cols` entries, of which those `fill` names zero or
    /// unit rows are known without reading `w`.
    pub fn with_fill(w: &'a [F], cols: usize, fill: &[crate::r1cs::RowFill]) -> Self {
        Self::from_rows(w.len(), cols, fill, w.chunks_exact(cols).map(Row::Shared))
    }

    /// An owning guard wipes the original authored rows on drop. Write those
    /// rows in place. Implicit rows still materialize from their public layout.
    pub(crate) fn with_mutable_values(
        w: &'a mut [F],
        cols: usize,
        fill: &[crate::r1cs::RowFill],
    ) -> Self {
        Self::from_rows(
            w.len(),
            cols,
            fill,
            w.chunks_exact_mut(cols).map(Row::Exclusive),
        )
    }

    fn from_rows(
        len: usize,
        cols: usize,
        fill: &[crate::r1cs::RowFill],
        rows: impl Iterator<Item = Row<'a, F>>,
    ) -> Self {
        use crate::r1cs::RowFill;
        assert!(cols.is_power_of_two() && len % cols == 0, "whole rows");
        assert_eq!(fill.len(), len / cols, "a fill per row");
        Rows {
            shift: cols.trailing_zeros() as usize,
            rows: rows
                .zip(fill)
                .map(|(row, f)| match f {
                    RowFill::Values => row,
                    RowFill::Zero => Row::Zero,
                    RowFill::Unit => Row::Unit,
                })
                .collect(),
            zero: std::sync::OnceLock::new(),
            unit: std::sync::OnceLock::new(),
        }
    }
    fn cols(&self) -> usize {
        1 << self.shift
    }
    pub fn len(&self) -> usize {
        self.rows.len() << self.shift
    }
    /// Whether row `r` is zero without reading it (a row the caller never
    /// wrote and the prover has not written).
    pub fn is_zero_row(&self, r: usize) -> bool {
        matches!(self.rows[r], Row::Zero)
    }
    /// Whether row `r` is a padding fixed row the caller never wrote beyond
    /// its 1 in column 0.
    pub fn is_unit_row(&self, r: usize) -> bool {
        matches!(self.rows[r], Row::Unit)
    }
    pub fn row(&self, r: usize) -> &[F] {
        match &self.rows[r] {
            Row::Shared(row) => row,
            Row::Exclusive(row) => row,
            Row::Own(row) => row,
            Row::Zero => self.zero.get_or_init(|| vec![F::ZERO; self.cols()]),
            Row::Unit => self.unit.get_or_init(|| {
                let mut row = vec![F::ZERO; self.cols()];
                row[0] = F::ONE;
                row
            }),
        }
    }
    /// Row `r`, copied first if it is still the caller's (a never-written
    /// row starts from fresh zero pages, without reading the caller's).
    pub fn row_mut(&mut self, r: usize) -> &mut [F] {
        let cols = self.cols();
        match self.rows[r] {
            Row::Shared(row) => self.rows[r] = Row::Own(row.to_vec()),
            Row::Zero => self.rows[r] = Row::Own(zeroed(cols)),
            Row::Unit => {
                let mut row = zeroed(cols);
                row[0] = F::ONE;
                self.rows[r] = Row::Own(row);
            }
            Row::Exclusive(_) | Row::Own(_) => {}
        }
        match &mut self.rows[r] {
            Row::Exclusive(row) => row,
            Row::Own(row) => row,
            _ => unreachable!("an owned row"),
        }
    }
    /// The rows as slices for the commitments: a never-written row is empty
    /// (zero, or a padding fixed row, whose commitment is the key's).
    pub fn slices(&self) -> Vec<&[F]> {
        self.rows
            .iter()
            .map(|row| match row {
                Row::Shared(row) => *row,
                Row::Exclusive(row) => &**row,
                Row::Own(row) => row.as_slice(),
                Row::Zero | Row::Unit => &[],
            })
            .collect()
    }
    /// How many rows the prover holds its own copy of.
    pub fn owned(&self) -> usize {
        self.rows
            .iter()
            .filter(|r| matches!(r, Row::Own(_)))
            .count()
    }
    /// Erases the rows the prover copied (its masked and filled values).
    pub fn zeroize_owned(&mut self)
    where
        F: zeroize::Zeroize,
    {
        for row in &mut self.rows {
            if let Row::Own(values) = row {
                zeroize::Zeroize::zeroize(values);
            }
        }
    }
    /// The whole witness, copied (fallbacks of circuits without row access).
    pub fn to_vec(&self) -> Vec<F> {
        (0..self.rows.len())
            .flat_map(|r| self.row(r).to_vec())
            .collect()
    }
    /// Takes `full`'s values for every row where they differ.
    pub fn write_back(&mut self, full: &[F]) {
        let cols = self.cols();
        for r in 0..self.rows.len() {
            let new = &full[r * cols..(r + 1) * cols];
            if self.row(r) != new {
                self.row_mut(r).copy_from_slice(new);
            }
        }
    }
}

impl<F: Field> Cells<F> for Rows<'_, F> {
    #[inline]
    fn cell(&self, i: usize) -> F {
        let (r, c) = (i >> self.shift, i & ((1 << self.shift) - 1));
        match &self.rows[r] {
            Row::Shared(row) => row[c],
            Row::Exclusive(row) => row[c],
            Row::Own(row) => row[c],
            Row::Zero => F::ZERO,
            Row::Unit if c == 0 => F::ONE,
            Row::Unit => F::ZERO,
        }
    }
    #[inline]
    fn set_cell(&mut self, i: usize, value: F) {
        let mask = (1 << self.shift) - 1;
        self.row_mut(i >> self.shift)[i & mask] = value;
    }
}

/// `z = (w || io)` read through the prover's [`Rows`]: the public half is
/// its tail (the io) and zero beyond. The witness half is private: its zero
/// runs are the rows the build never wrote and padding rows beyond column 0,
/// never a row's values.
pub struct ZView<'r, 'a, F> {
    pub rows: &'r Rows<'a, F>,
    pub tail: &'r [F],
}

impl<F: Field> ZView<'_, '_, F> {
    #[inline]
    pub fn get(&self, i: usize) -> F {
        let half = self.rows.len();
        if i < half {
            self.rows.cell(i)
        } else {
            self.tail.get(i - half).copied().unwrap_or(F::ZERO)
        }
    }
}

impl<F: Field> Table<F> for ZView<'_, '_, F> {
    #[inline]
    fn len(&self) -> usize {
        2 * self.rows.len()
    }
    #[inline]
    fn at(&self, i: usize) -> F {
        self.get(i)
    }
    fn zero_run(&self, range: std::ops::Range<usize>) -> bool {
        let half = self.rows.len();
        let cols = 1usize << self.rows.shift;
        let tail_end = half + self.tail.len();
        let mut at = range.start.min(half);
        let end = range.end.min(half);
        while at < end {
            let (r, c) = (at >> self.rows.shift, at & (cols - 1));
            let n = (cols - c).min(end - at);
            // The row's public account only: a row of values may hold
            // anything, whatever it holds, and is not read here.
            let zero = match &self.rows.rows[r] {
                Row::Shared(_) | Row::Exclusive(_) | Row::Own(_) => false,
                Row::Zero => true,
                Row::Unit => c != 0,
            };
            if !zero {
                return false;
            }
            at += n;
        }
        // The io is public: read by value.
        let high = range.start.clamp(half, tail_end)..range.end.clamp(half, tail_end);
        self.tail[high.start - half..high.end - half]
            .iter()
            .all(|x| x.is_zero_vartime())
    }
}

/// Allocate the native Pasta fields on zero-on-demand pages. For every other
/// field, initialize elements from its logical zero.
pub fn zeroed<F: Field>(n: usize) -> Vec<F> {
    use std::any::TypeId;

    let field = TypeId::of::<F>();
    let pasta =
        field == TypeId::of::<iroha_pasta::Fp>() || field == TypeId::of::<iroha_pasta::Fq>();
    if n == 0 || !pasta {
        return vec![F::ZERO; n];
    }
    let layout = std::alloc::Layout::array::<F>(n).expect("table size");
    // SAFETY: Fp and Fq store their Montgomery zero as four zero u64 limbs.
    // The type check above selects exactly those types and n is nonzero.
    // The returned Vec owns the allocation with its original layout.
    unsafe {
        let ptr = std::alloc::alloc_zeroed(layout).cast::<F>();
        if ptr.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        Vec::from_raw_parts(ptr, n, n)
    }
}

/// Grid bookkeeping for `k` variables: indices are base 3, coordinate 0 most
/// significant; digit 2 is "infinity" while accumulating and the node 2 after
/// [`Basis::to_grid`].
pub struct Basis {
    pub k: usize,
    cube: usize,
    grid: usize,
    /// Grid positions of the Boolean points, by corner index.
    inputs: Vec<usize>,
    /// Extension steps `(target, one, zero)`: `out[target] = out[one] - out[zero]`.
    steps: Vec<(usize, usize, usize)>,
    /// Grid positions with at least one coordinate at infinity.
    upper: Vec<usize>,
}

fn digits(mut index: usize, k: usize) -> Vec<usize> {
    let mut out = vec![0; k];
    for slot in out.iter_mut().rev() {
        *slot = index % 3;
        index /= 3;
    }
    out
}

fn index_of(digits: &[usize]) -> usize {
    digits.iter().fold(0, |acc, &d| acc * 3 + d)
}

impl Basis {
    pub fn new(k: usize) -> Self {
        let grid = pow3(k);
        let mut steps = Vec::new();
        for j in 0..k {
            for index in 0..grid {
                let point = digits(index, k);
                if point[j] != 2 || point[j + 1..].iter().any(|&d| d == 2) {
                    continue;
                }
                let mut one = point.clone();
                one[j] = 1;
                let mut zero = point;
                zero[j] = 0;
                steps.push((index, index_of(&one), index_of(&zero)));
            }
        }
        let upper = (0..grid).filter(|g| digits(*g, k).contains(&2)).collect();
        Self {
            k,
            cube: 1 << k,
            grid,
            inputs: boolean_positions(k),
            steps,
            upper,
        }
    }

    /// Extends corner values to `{0, 1, inf}^k`.
    #[inline]
    fn extend<F: Field>(&self, corners: &[F], out: &mut [F]) {
        for (value, &position) in corners.iter().zip(&self.inputs) {
            out[position] = *value;
        }
        for &(target, one, zero) in &self.steps {
            out[target] = out[one] - out[zero];
        }
    }

    /// Converts accumulated values on `{0, 1, inf}^k` to the grid
    /// `{0, 1, 2}^k`: along each axis `t(2) = 2 (t(1) + t(inf)) - t(0)` for a
    /// polynomial of degree at most 2 in that variable.
    pub fn to_grid<F: Field>(&self, acc: &mut [F]) {
        let mut stride = 1;
        for _ in 0..self.k {
            for base in 0..self.grid {
                if (base / stride) % 3 != 0 {
                    continue;
                }
                let (t0, t1, tinf) = (acc[base], acc[base + stride], acc[base + 2 * stride]);
                acc[base + 2 * stride] = (t1 + tinf).double() - t0;
            }
            stride *= 3;
        }
    }
}

fn add_into<F: Field>(mut a: Vec<F>, b: Vec<F>) -> Vec<F> {
    a.iter_mut().zip(b).for_each(|(x, y)| *x += y);
    a
}

/// Grid values of `t(Y) = sum_low e(low) (A B - C)(Y, low)`.
///
/// `satisfied`: the tables are `Az, Bz, Cz` of a satisfied relation before any
/// fold, so every Boolean point is zero and is not computed.
pub fn outer_round<F: Field>(
    basis: &Basis,
    a: &[F],
    b: &[F],
    c: &[F],
    e: &[F],
    satisfied: bool,
) -> Vec<F> {
    let (cube, grid) = (basis.cube, basis.grid);
    let low = a.len() >> basis.k;
    assert!(b.len() == a.len() && c.len() == a.len() && e.len() == low);
    let mut acc = (0..low)
        .into_par_iter()
        .with_min_len(64)
        .with_max_len(LEAF)
        .fold(
            || {
                (
                    vec![F::ZERO; grid],
                    vec![F::ZERO; 2 * cube],
                    vec![F::ZERO; 2 * grid],
                )
            },
            |(mut acc, mut corners, mut ext), j| {
                let weight = e[j];
                let (ca, cb) = corners.split_at_mut(cube);
                for hi in 0..cube {
                    ca[hi] = weight * a[hi * low + j];
                    cb[hi] = b[hi * low + j];
                }
                let (ga, gb) = ext.split_at_mut(grid);
                basis.extend(ca, ga);
                basis.extend(cb, gb);
                for &g in &basis.upper {
                    acc[g] += ga[g] * gb[g];
                }
                if !satisfied {
                    for (hi, &g) in basis.inputs.iter().enumerate() {
                        acc[g] += ga[g] * gb[g] - weight * c[hi * low + j];
                    }
                }
                (acc, corners, ext)
            },
        )
        .map(|(acc, _, _)| acc)
        .reduce(|| vec![F::ZERO; grid], add_into);
    basis.to_grid(&mut acc);
    acc
}

/// Low rows per chunk of a streamed first outer round.
pub const CHUNK: usize = 1 << 11;

/// The public prefix of constraint rows that can contribute to the first round.
/// The live high corners are rounded up to a Boolean cube so each omitted
/// leading variable contributes a factor of `1 - X` to each of A, B and C.
#[derive(Clone, Copy)]
pub struct OuterRows {
    low: usize,
    low_rows: usize,
    extent: usize,
}

impl OuterRows {
    pub fn new(low: usize, k: usize, extent: usize) -> Self {
        assert!(low > 0 && extent <= low << k);
        Self {
            low,
            low_rows: extent.min(low),
            extent,
        }
    }

    /// Number of nonempty high corners for the chunk starting at this low row.
    pub(crate) fn corners(self, start: usize) -> usize {
        self.extent.saturating_sub(start).div_ceil(self.low)
    }
}

/// The unfolded tables of a streamed first outer round, computed by the
/// caller a chunk at a time: `fill(js, mats, out)` writes, for each selected
/// matrix `m` (A, B, C), the corners `out[m][hi * js.len() + (j - js.start)]`
/// (rows `hi * low + j`) of the low rows `js`.
pub type Corners<'a, F> = dyn Fn(std::ops::Range<usize>, [bool; 3], &mut [Vec<F>; 3]) + Sync + 'a;

/// [`outer_round`] of a satisfied relation's unfolded `Az, Bz` (no Boolean
/// point, no `C`), which are never held whole: `fill` computes them a chunk
/// of low rows at a time.
pub fn outer_round_streamed<F: Field>(
    basis: &Basis,
    rows: OuterRows,
    e: &[F],
    fill: &Corners<'_, F>,
) -> Vec<F> {
    assert_eq!(e.len(), rows.low);
    let sub_cube = rows.corners(0).next_power_of_two();
    let reduced = (sub_cube < basis.cube).then(|| Basis::new(sub_cube.ilog2() as usize));
    let sub = reduced.as_ref().unwrap_or(basis);
    let (cube, grid) = (sub.cube, sub.grid);
    let points = reduced.as_ref().map(|_| (0..grid).collect::<Vec<_>>());
    let points = points.as_deref().unwrap_or(&basis.upper);
    let chunk = CHUNK.min(rows.low);
    let mut acc = (0..rows.low_rows.div_ceil(chunk))
        .into_par_iter()
        .with_max_len(1)
        .fold(
            || {
                (
                    vec![F::ZERO; grid],
                    vec![F::ZERO; 2 * cube],
                    vec![F::ZERO; 2 * grid],
                    [Vec::new(), Vec::new(), Vec::new()],
                )
            },
            |(mut acc, mut corners, mut ext, mut tables), ci| {
                let js = ci * chunk..((ci + 1) * chunk).min(rows.low_rows);
                let n = js.len();
                let live = rows.corners(js.start);
                for t in tables.iter_mut().take(2) {
                    t.clear();
                    t.resize(live * n, F::ZERO);
                }
                fill(js.clone(), [true, true, false], &mut tables);
                let (ca, cb) = corners.split_at_mut(cube);
                ca[live..].fill(F::ZERO);
                cb[live..].fill(F::ZERO);
                for (jj, j) in js.enumerate() {
                    let weight = e[j];
                    for hi in 0..live {
                        ca[hi] = weight * tables[0][hi * n + jj];
                        cb[hi] = tables[1][hi * n + jj];
                    }
                    let (ga, gb) = ext.split_at_mut(grid);
                    sub.extend(ca, ga);
                    sub.extend(cb, gb);
                    for &g in points {
                        acc[g] += ga[g] * gb[g];
                    }
                }
                (acc, corners, ext, tables)
            },
        )
        .map(|(acc, ..)| acc)
        .reduce(|| vec![F::ZERO; grid], add_into);
    if reduced.is_some() {
        // In the {0,1,inf} basis, each frozen axis multiplies AB by
        // (1-X)^2, with values [1,0,1]. C is multilinear and contributes
        // only at full Boolean points, where satisfaction cancels AB.
        let mut full = vec![F::ZERO; basis.grid];
        for (prefix, part) in full.chunks_exact_mut(grid).enumerate() {
            if !digits(prefix, basis.k - sub.k).contains(&1) {
                part.copy_from_slice(&acc);
            }
        }
        for &g in &basis.inputs {
            full[g] = F::ZERO;
        }
        acc = full;
    }
    basis.to_grid(&mut acc);
    acc
}

/// [`fold_top`] (without a support) of the unfolded `Az, Bz, Cz`, which
/// `fill` computes a chunk of low rows at a time.
pub fn fold_top_streamed<F: Field>(rows: OuterRows, r: &[F], fill: &Corners<'_, F>) -> [Vec<F>; 3] {
    let low = rows.low;
    // For a chunk with an empty high corner, the dot product takes at most
    // 2^k - 1 multiplications per row, and walks both tables contiguously.
    // A fully populated chunk still uses the 2^k - 1 interpolation tree.
    let weights = crate::grid::eq_table(r);
    let cube = weights.len();
    let chunk = CHUNK.min(low);
    let mut out = [vec![F::ZERO; low], vec![F::ZERO; low], vec![F::ZERO; low]];
    let [oa, ob, oc] = &mut out;
    oa[..rows.low_rows]
        .par_chunks_mut(chunk)
        .zip(ob[..rows.low_rows].par_chunks_mut(chunk))
        .zip(oc[..rows.low_rows].par_chunks_mut(chunk))
        .enumerate()
        .for_each(|(ci, ((a, b), c))| {
            let js = ci * chunk..ci * chunk + a.len();
            let n = js.len();
            let live = rows.corners(js.start);
            let mut tables = [
                vec![F::ZERO; live * n],
                vec![F::ZERO; live * n],
                vec![F::ZERO; live * n],
            ];
            fill(js.clone(), [true, true, true], &mut tables);
            if live < cube {
                for (table, folded) in tables.iter().zip([a, b, c]) {
                    for (hi, part) in table.chunks_exact(n).enumerate() {
                        let len = (rows.extent - (hi * low + js.start)).min(n);
                        for (value, &source) in folded[..len].iter_mut().zip(part) {
                            *value += weights[hi] * source;
                        }
                    }
                }
                return;
            }
            let mut corners = vec![F::ZERO; cube];
            for (table, folded) in tables.iter().zip([a, b, c]) {
                for (jj, value) in folded.iter_mut().enumerate() {
                    for (hi, corner) in corners.iter_mut().enumerate() {
                        *corner = table[hi * n + jj];
                    }
                    *value = fold_corners(&mut corners, r);
                }
            }
        });
    out
}

/// Rows per run of the corner support.
pub const SEGMENT: usize = 256;

/// Which corners of a round's `k` variables may hold anything: per run of
/// [`SEGMENT`] rows `j`, bit `hi` is set unless both `m` and `z` are zero at
/// corner `hi` throughout the run by public information ([`Table::zero_run`]).
/// A set bit over entries that are zero changes no output, so the masks are a
/// public over-approximation of the tables' true support: they never depend
/// on a private value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Support {
    k: usize,
    seg: usize,
    masks: Vec<u32>,
    /// The same per run for `z` alone (the private table).
    z_masks: Vec<u32>,
}

impl Support {
    /// The live runs of `z` folded under this support: a run whose corners
    /// are all zero in `z` by layout folds to zero, so the folded table's run
    /// `s` (entries `s * seg..`) is live only where `z` had a live corner.
    pub fn folded_live(&self) -> Live {
        Live {
            seg: self.seg,
            runs: self.z_masks.iter().map(|mask| *mask != 0).collect(),
        }
    }
}

/// Keep small private folds materialized. Above 32 MiB the next round can
/// recompute its inputs as it reads them, leaving the first array unallocated.
pub(crate) fn large_private_fold<F>(low: usize) -> bool {
    low > (32 << 20) / std::mem::size_of::<F>().max(1)
}

/// The first fold of a private table, evaluated while the second round reads
/// it. The public row layout determines every corner read. The weights are the
/// multilinear evaluation of the first round's transcript challenges.
pub(crate) struct FoldedWitness<'a, F, T: ?Sized> {
    witness: &'a T,
    weights: Vec<F>,
    masks: Vec<u32>,
    seg: usize,
    low: usize,
}

impl<'a, F: Field, T: Table<F> + ?Sized> FoldedWitness<'a, F, T> {
    pub(crate) fn new(witness: &'a T, r: &[F], support: &Support) -> Self {
        assert_eq!(r.len(), support.k);
        let low = witness.len() >> support.k;
        assert_eq!(support.z_masks.len(), low.div_ceil(support.seg));
        Self {
            witness,
            weights: crate::grid::eq_table(r),
            masks: support.z_masks.clone(),
            seg: support.seg,
            low,
        }
    }
}

impl<F: Field, T: Table<F> + ?Sized> Table<F> for FoldedWitness<'_, F, T> {
    fn len(&self) -> usize {
        self.low
    }

    #[inline]
    fn at(&self, j: usize) -> F {
        let mut mask = self.masks[j / self.seg];
        let mut sum = F::ZERO;
        while mask != 0 {
            let hi = mask.trailing_zeros() as usize;
            mask &= mask - 1;
            sum += self.weights[hi] * self.witness.at(hi * self.low + j);
        }
        sum
    }

    fn zero_run(&self, range: std::ops::Range<usize>) -> bool {
        range.is_empty()
            || self.masks[range.start / self.seg..=(range.end - 1) / self.seg]
                .iter()
                .all(|mask| *mask == 0)
    }
}

/// Reads both tables' public zero runs: `m` (public) may be scanned by
/// value, `z` (private) answers from its layout.
pub fn support<F: Field, M: Table<F> + ?Sized, Z: Table<F> + ?Sized>(
    k: usize,
    m: &M,
    z: &Z,
) -> Support {
    assert!(k <= 5 && z.len() == m.len());
    let low = m.len() >> k;
    let seg = SEGMENT.min(low).max(1);
    let pairs: Vec<(u32, u32)> = (0..low.div_ceil(seg))
        .into_par_iter()
        .with_max_len(16)
        .map(|s| {
            let (j0, j1) = (s * seg, ((s + 1) * seg).min(low));
            (0..1usize << k).fold((0u32, 0u32), |(mask, z_mask), hi| {
                let run = hi * low + j0..hi * low + j1;
                let z_any = !z.zero_run(run.clone());
                let any = z_any || !m.zero_run(run);
                (
                    mask | (u32::from(any) << hi),
                    z_mask | (u32::from(z_any) << hi),
                )
            })
        })
        .collect();
    let (masks, z_masks) = pairs.into_iter().unzip();
    Support {
        k,
        seg,
        masks,
        z_masks,
    }
}

/// How one corner mask reduces a round: the variables that stay free, the
/// corner of each sub-cube point, and the variables whose upper (`low`) or
/// lower (`high`) half is empty.
struct Reduction {
    free: Vec<usize>,
    corners: Vec<usize>,
    low: Vec<usize>,
    high: Vec<usize>,
}

impl Reduction {
    /// `mask` is nonzero.
    fn new(k: usize, mask: u32) -> Self {
        let (mut free, mut low, mut high) = (Vec::new(), Vec::new(), Vec::new());
        let mut base = 0usize;
        for t in 0..k {
            let bit = k - 1 - t;
            let upper = (0..1usize << k).any(|hi| (hi >> bit) & 1 == 1 && (mask >> hi) & 1 == 1);
            let lower = (0..1usize << k).any(|hi| (hi >> bit) & 1 == 0 && (mask >> hi) & 1 == 1);
            match (lower, upper) {
                (true, true) => free.push(t),
                (true, false) => low.push(t),
                _ => {
                    high.push(t);
                    base |= 1 << bit;
                }
            }
        }
        let f = free.len();
        let corners = (0..1usize << f)
            .map(|s| {
                free.iter().enumerate().fold(base, |hi, (i, t)| {
                    hi | (((s >> (f - 1 - i)) & 1) << (k - 1 - t))
                })
            })
            .collect();
        Reduction {
            free,
            corners,
            low,
            high,
        }
    }

    /// Adds the sub-cube's values on `{0, 1, inf}^f` into the round's: a
    /// variable with an empty upper half has `(1 - Y)^2 P` (P at 0 and as
    /// the leading coefficient, 0 at 1), one with an empty lower half
    /// `Y^2 P` (P at 1 and as the leading coefficient, 0 at 0).
    fn scatter<F: Field>(&self, k: usize, sub: &[F], acc: &mut [F]) {
        let f = self.free.len();
        let frozen = self.low.len() + self.high.len();
        for (g, value) in sub.iter().enumerate() {
            let d = digits(g, f);
            let mut point = vec![0usize; k];
            for (i, t) in self.free.iter().enumerate() {
                point[*t] = d[i];
            }
            for choice in 0..1usize << frozen {
                for (c, t) in self.low.iter().enumerate() {
                    point[*t] = if (choice >> c) & 1 == 1 { 2 } else { 0 };
                }
                for (c, t) in self.high.iter().enumerate() {
                    point[*t] = if (choice >> (self.low.len() + c)) & 1 == 1 {
                        2
                    } else {
                        1
                    };
                }
                acc[index_of(&point)] += value;
            }
        }
    }
}

/// The distinct masks of a support with their reductions, and each run's
/// index into them (`u16::MAX` for an empty run).
fn reductions(support: &Support) -> (Vec<(u32, Reduction)>, Vec<u16>) {
    let mut list: Vec<(u32, Reduction)> = Vec::new();
    let runs = support
        .masks
        .iter()
        .map(|&mask| {
            if mask == 0 {
                return u16::MAX;
            }
            match list.iter().position(|(m, _)| *m == mask) {
                Some(i) => i as u16,
                None => {
                    list.push((mask, Reduction::new(support.k, mask)));
                    (list.len() - 1) as u16
                }
            }
        })
        .collect();
    (list, runs)
}

/// Grid values of `q(Y) = sum_low (M Z)(Y, low)`.
///
/// With a [`Support`] every run of rows works on the sub-cube its corners
/// span; without one every row uses the whole cube.
pub fn inner_round<F: Field, M: Table<F> + ?Sized, Z: Table<F> + ?Sized>(
    basis: &Basis,
    m: &M,
    z: &Z,
    support: Option<&Support>,
) -> Vec<F> {
    let (k, cube, grid) = (basis.k, basis.cube, basis.grid);
    let low = m.len() >> k;
    assert_eq!(z.len(), m.len());
    let Some(support) = support else {
        let mut acc = (0..low)
            .into_par_iter()
            .with_min_len(64)
            .with_max_len(LEAF)
            .fold(
                || {
                    (
                        vec![F::ZERO; grid],
                        vec![F::ZERO; 2 * cube],
                        vec![F::ZERO; 2 * grid],
                    )
                },
                |(mut acc, mut corners, mut ext), j| {
                    let (cm, cz) = corners.split_at_mut(cube);
                    for hi in 0..cube {
                        cm[hi] = m.at(hi * low + j);
                        cz[hi] = z.at(hi * low + j);
                    }
                    let (gm, gz) = ext.split_at_mut(grid);
                    basis.extend(cm, gm);
                    basis.extend(cz, gz);
                    for g in 0..grid {
                        acc[g] += gm[g] * gz[g];
                    }
                    (acc, corners, ext)
                },
            )
            .map(|(acc, _, _)| acc)
            .reduce(|| vec![F::ZERO; grid], add_into);
        basis.to_grid(&mut acc);
        return acc;
    };
    assert_eq!(support.k, k);
    let (list, runs) = reductions(support);
    let subs: Vec<Basis> = (0..=k).map(Basis::new).collect();
    let fresh = || {
        (
            list.iter()
                .map(|(_, r)| vec![F::ZERO; pow3(r.free.len())])
                .collect::<Vec<_>>(),
            vec![F::ZERO; 2 * cube],
            vec![F::ZERO; 2 * grid],
        )
    };
    let (parts, _, _) = (0..runs.len())
        .into_par_iter()
        .with_max_len(4)
        .fold(fresh, |(mut parts, mut corners, mut ext), s| {
            if runs[s] == u16::MAX {
                return (parts, corners, ext);
            }
            let reduction = &list[runs[s] as usize].1;
            let sub = &subs[reduction.free.len()];
            let (n, sgrid) = (reduction.corners.len(), sub.grid);
            let part = &mut parts[runs[s] as usize];
            for j in s * support.seg..((s + 1) * support.seg).min(low) {
                let (cm, cz) = corners.split_at_mut(cube);
                for (c, hi) in reduction.corners.iter().enumerate() {
                    cm[c] = m.at(hi * low + j);
                    cz[c] = z.at(hi * low + j);
                }
                let (gm, gz) = ext.split_at_mut(grid);
                sub.extend(&cm[..n], &mut gm[..sgrid]);
                sub.extend(&cz[..n], &mut gz[..sgrid]);
                for g in 0..sgrid {
                    part[g] += gm[g] * gz[g];
                }
            }
            (parts, corners, ext)
        })
        .reduce(fresh, |(x, c, e), (y, _, _)| {
            (
                x.into_iter().zip(y).map(|(a, b)| add_into(a, b)).collect(),
                c,
                e,
            )
        });
    let mut acc = vec![F::ZERO; grid];
    for ((_, reduction), part) in list.iter().zip(&parts) {
        reduction.scatter(k, part, &mut acc);
    }
    basis.to_grid(&mut acc);
    acc
}

/// One output of the fold: the multilinear extension of `corners` (first
/// variable most significant) at `r`, by a tree from the last variable.
#[inline]
fn fold_corners<F: Field>(corners: &mut [F], r: &[F]) -> F {
    let mut len = corners.len();
    for t in (0..r.len()).rev() {
        len /= 2;
        for h in 0..len {
            let (v0, v1) = (corners[2 * h], corners[2 * h + 1]);
            corners[h] = v0 + r[t] * (v1 - v0);
        }
    }
    corners[0]
}

struct FoldReduction<F> {
    corners: Vec<usize>,
    challenges: Vec<F>,
    factor: Option<F>,
}

impl<F: Field> FoldReduction<F> {
    #[inline]
    fn evaluate(&self, scratch: &mut [F], at: impl Fn(usize) -> F) -> F {
        let corners = &mut scratch[..self.corners.len()];
        for (value, &corner) in corners.iter_mut().zip(&self.corners) {
            *value = at(corner);
        }
        let value = fold_corners(corners, &self.challenges);
        self.factor.map_or(value, |factor| factor * value)
    }
}

/// Bind each distinct run mask to the free challenges and fixed-axis factor.
fn fold_reductions<F: Field>(support: &Support, r: &[F]) -> (Vec<FoldReduction<F>>, Vec<u16>) {
    assert_eq!(support.k, r.len());
    let (list, runs) = reductions(support);
    let bound = list
        .into_iter()
        .map(|(_, reduction)| {
            let challenges = reduction.free.iter().map(|t| r[*t]).collect();
            let factor = (!reduction.low.is_empty() || !reduction.high.is_empty()).then(|| {
                reduction
                    .low
                    .iter()
                    .map(|t| F::ONE - r[*t])
                    .chain(reduction.high.iter().map(|t| r[*t]))
                    .product()
            });
            FoldReduction {
                corners: reduction.corners,
                challenges,
                factor,
            }
        })
        .collect();
    (bound, runs)
}

/// Binds the top `r.len()` variables of `table` to `r`.
///
/// With a [`Support`] (of this table and its partner) a run folds its
/// sub-cube and multiplies by `1 - r` (or `r`) for every variable whose
/// upper (or lower) half is empty; an empty run folds to zero unread.
pub fn fold_top<F: Field, T: Table<F> + ?Sized>(
    table: &T,
    r: &[F],
    support: Option<&Support>,
) -> Vec<F> {
    let k = r.len();
    let cube = 1usize << k;
    let low = table.len() >> k;
    let Some(support) = support else {
        return (0..low)
            .into_par_iter()
            .with_min_len(64)
            .with_max_len(4 * LEAF)
            .map_init(
                || vec![F::ZERO; cube],
                |corners, j| {
                    for hi in 0..cube {
                        corners[hi] = table.at(hi * low + j);
                    }
                    fold_corners(corners, r)
                },
            )
            .collect();
    };
    let (reductions, runs) = fold_reductions(support, r);
    (0..low)
        .into_par_iter()
        .with_min_len(64)
        .with_max_len(4 * LEAF)
        .map_init(
            || vec![F::ZERO; cube],
            |corners, j| {
                let run = runs[j / support.seg];
                if run == u16::MAX {
                    return F::ZERO;
                }
                reductions[usize::from(run)].evaluate(corners, |hi| table.at(hi * low + j))
            },
        )
        .collect()
}

/// Receives the entries a relation computes for the dense half of a public
/// table (`M(r_x, .)` below `2^sw`), from parallel jobs.
pub trait Entries<F>: Sync {
    /// A worker's own state.
    type Local: Send;
    fn local(&self) -> Self::Local;
    /// The jobs run once per sweep, emitting in sweep `s` only the positions
    /// in `self.sweep(s)`.
    fn sweeps(&self) -> usize {
        1
    }
    fn sweep(&self, _s: usize) -> std::ops::Range<usize> {
        0..usize::MAX
    }
    fn put(&self, local: &mut Self::Local, pos: usize, value: F);
    /// A worker's finished state.
    fn absorb(&self, local: Self::Local);
}

/// A relation computing `M(r_x, .)`: its dense half into `sink`, and with
/// `tail` its public half up to the last io entry (`None` without).
///
/// # Safety
///
/// Every emitted position must be inside its sweep and emitted at most once
/// for that sweep across all workers. Puts from different sweeps must never
/// overlap in time. Distinct sweeps can map different positions to the same
/// mutable output cell when the sink folds a corner.
pub unsafe trait Generate<F> {
    fn run<S: Entries<F>>(&self, sink: &S, tail: bool) -> Option<Vec<F>>;
}

/// The first inner round: its values, its challenges, `M(r_x, .)` folded by
/// them and the round's support (which the caller's fold of `z` reuses).
pub struct FirstInner<F> {
    pub values: Vec<F>,
    pub r: Vec<F>,
    pub m: Vec<F>,
    pub support: Option<Support>,
}

/// The first inner round over a whole [`Split`] `M(r_x, .)`: `challenge`
/// commits the round's values and draws its challenges.
pub fn first_inner_split<F: Field, Z: Table<F> + ?Sized>(
    basis: &Basis,
    m: Split<F>,
    z: &Z,
    challenge: &mut dyn FnMut(&[F]) -> Vec<F>,
) -> FirstInner<F> {
    let k = basis.k;
    let support = (m.len() >> k >= SEGMENT)
        .then(|| crate::prof::time("i.support", || crate::kern::support(k, &m, z)));
    let values = crate::prof::time("i.round", || inner_round(basis, &m, z, support.as_ref()));
    let r = challenge(&values);
    let m = crate::prof::time("i.fold", || m.into_folded(&r, support.as_ref()));
    FirstInner {
        values,
        r,
        m,
        support,
    }
}

/// [`first_inner_split`] without the table: the relation computes the dense
/// half again for every corner but the lowest. The round's values are
/// bilinear in the tables, so for `P[h][h'] = sum_j M(h, j) z(h', j)` (linear
/// in M) each grid value is `sum_{h, h'} B[g][h] B[g][h'] P[h][h']`, where
/// `B` extends corners to `{0, 1, inf}^k`: the first pass adds every entry
/// into its worker's `P` in any order, marks the runs holding a nonzero
/// (public) entry, which are the zero runs [`support`] reads, and keeps the
/// lowest corner in the folded table's place. After the challenges the
/// second pass scales that corner by `eq(r, 0)` and adds `eq(r, h) M(h, j)`
/// of the other corners, which equals the fold under any support (a corner a
/// run's mask leaves out is zero in M). Field arithmetic is exact: the
/// values, masks and folded table equal the split table's.
pub fn first_inner_streamed<F: Field, Z: Table<F> + ?Sized, G: Generate<F>>(
    basis: &Basis,
    half: usize,
    z: &Z,
    generate: &G,
    challenge: &mut dyn FnMut(&[F]) -> Vec<F>,
) -> FirstInner<F> {
    let k = basis.k;
    assert!(half.is_power_of_two() && (1..=5).contains(&k) && (2 * half) >> k > 0);
    assert_eq!(z.len(), 2 * half, "z's two halves");
    let projection = Projection::new(k, half, z);
    let tail =
        crate::prof::time("i.mtable", || generate.run(&projection, true)).expect("the public half");
    assert!(tail.len() <= half, "the tail lies in the upper half");
    let (values, nonzero, lowest) =
        crate::prof::time("i.round", || projection.finish(basis, &tail));
    let zeros = Projected {
        half,
        nonzero: &nonzero,
        tail: &tail,
    };
    let support = ((2 * half) >> k >= SEGMENT)
        .then(|| crate::prof::time("i.support", || crate::kern::support(k, &zeros, z)));
    let r = challenge(&values);
    let folding = Folding::new(half, &r, lowest, &nonzero);
    if folding.sweeps() > 0 {
        crate::prof::time("i.mfold", || generate.run(&folding, false));
    }
    FirstInner {
        m: folding.finish(&tail),
        values,
        r,
        support,
    }
}

/// Pass one: `P[h][h'] += M(h, j) z(h', j)` per entry, z read at public
/// positions and never tested; the lowest corner's entries kept.
struct Projection<'z, F, Z: ?Sized> {
    z: &'z Z,
    cube: usize,
    low_bits: usize,
    seg_bits: usize,
    /// Per run of rows `j`: the corners where z may hold anything, by its
    /// public layout (the support's z masks).
    z_live: Vec<u32>,
    /// Per run of [`SEGMENT`] dense entries: whether one is nonzero.
    nonzero: Vec<std::sync::atomic::AtomicU64>,
    sums: std::sync::Mutex<Vec<F>>,
    /// The lowest corner `M(0, .)`, in the folded table's place.
    lowest: std::sync::Mutex<Vec<F>>,
    lowest_ptr: Shared<F>,
}

impl<'z, F: Field, Z: Table<F> + ?Sized> Projection<'z, F, Z> {
    fn new(k: usize, half: usize, z: &'z Z) -> Self {
        let (cube, low) = (1usize << k, (2 * half) >> k);
        let seg = SEGMENT.min(low);
        let z_live = (0..low / seg)
            .into_par_iter()
            .map(|s| {
                (0..cube).fold(0u32, |mask, h| {
                    let run = h * low + s * seg..h * low + (s + 1) * seg;
                    mask | (u32::from(!z.zero_run(run)) << h)
                })
            })
            .collect();
        let mut lowest = zeroed(low);
        let lowest_ptr = Shared(lowest.as_mut_ptr());
        Projection {
            z,
            cube,
            low_bits: low.trailing_zeros() as usize,
            seg_bits: seg.trailing_zeros() as usize,
            z_live,
            lowest: std::sync::Mutex::new(lowest),
            lowest_ptr,
            nonzero: (0..half.div_ceil(SEGMENT).div_ceil(64))
                .map(|_| std::sync::atomic::AtomicU64::new(0))
                .collect(),
            sums: std::sync::Mutex::new(vec![F::ZERO; cube * cube]),
        }
    }

    /// Adds `value z(h', j)` into `row` (`P[h]`) for every corner `h'` where
    /// z may hold anything by layout (a corner it empties adds zero).
    #[inline]
    fn add(&self, row: &mut [F], j: usize, value: F) {
        let mut live = self.z_live[j >> self.seg_bits];
        while live != 0 {
            let h = live.trailing_zeros() as usize;
            live &= live - 1;
            row[h] += value * self.z.at((h << self.low_bits) + j);
        }
    }

    /// The public half's entries joined: the round's grid values, the
    /// nonzero runs and the lowest corner.
    fn finish(&self, basis: &Basis, tail: &[F]) -> (Vec<F>, Vec<u64>, Vec<F>) {
        let (cube, grid) = (basis.cube, basis.grid);
        assert_eq!(cube, self.cube, "the round's variables");
        let half = (cube / 2) << self.low_bits;
        let mut sums = std::mem::take(&mut *self.sums.lock().unwrap());
        for (t, &value) in tail.iter().enumerate() {
            if !value.is_zero_vartime() {
                let (h, j) = (
                    (half + t) >> self.low_bits,
                    (half + t) & ((1 << self.low_bits) - 1),
                );
                self.add(&mut sums[h * cube..(h + 1) * cube], j, value);
            }
        }
        // B's columns: the corners' unit vectors extended.
        let mut columns = vec![F::ZERO; grid * cube];
        let (mut unit, mut ext) = (vec![F::ZERO; cube], vec![F::ZERO; grid]);
        for h in 0..cube {
            unit.fill(F::ZERO);
            unit[h] = F::ONE;
            basis.extend(&unit, &mut ext);
            for (g, value) in ext.iter().enumerate() {
                columns[g * cube + h] = *value;
            }
        }
        let mut acc: Vec<F> = (0..grid)
            .map(|g| {
                let b = &columns[g * cube..(g + 1) * cube];
                (0..cube)
                    .map(|h| b[h] * (0..cube).map(|h2| b[h2] * sums[h * cube + h2]).sum::<F>())
                    .sum()
            })
            .collect();
        basis.to_grid(&mut acc);
        let nonzero = self
            .nonzero
            .iter()
            .map(|w| w.load(std::sync::atomic::Ordering::Relaxed))
            .collect();
        let lowest = std::mem::take(&mut *self.lowest.lock().unwrap());
        (acc, nonzero, lowest)
    }
}

impl<F: Field, Z: Table<F> + ?Sized> Entries<F> for Projection<'_, F, Z> {
    /// The worker's `P` and the run it marked last.
    type Local = (Vec<F>, usize);
    fn local(&self) -> Self::Local {
        (vec![F::ZERO; self.cube * self.cube], usize::MAX)
    }
    fn put(&self, (sums, last): &mut Self::Local, pos: usize, value: F) {
        let h = pos >> self.low_bits;
        assert!(h < self.cube / 2, "a dense entry");
        // M is public: a zero entry adds nothing.
        if value.is_zero_vartime() {
            return;
        }
        let s = pos / SEGMENT;
        if s != *last {
            self.nonzero[s / 64].fetch_or(1 << (s % 64), std::sync::atomic::Ordering::Relaxed);
            *last = s;
        }
        let j = pos & ((1 << self.low_bits) - 1);
        if h == 0 {
            // SAFETY: `j` is below the lowest corner's `low` entries, which
            // outlive the pass, and a generator emits a position once.
            unsafe {
                self.lowest_ptr.0.add(j).write(value);
            }
        }
        self.add(&mut sums[h * self.cube..(h + 1) * self.cube], j, value);
    }
    fn absorb(&self, (sums, _): Self::Local) {
        let mut total = self.sums.lock().unwrap();
        total.iter_mut().zip(sums).for_each(|(t, s)| *t += s);
    }
}

/// `M(r_x, .)`'s zero runs after pass one: a dense run is zero unless marked
/// (the [`SEGMENT`] runs [`support`] reads are whole; a part of a run answers
/// for the whole, which may only be `false`), the public half by value.
struct Projected<'a, F> {
    half: usize,
    nonzero: &'a [u64],
    tail: &'a [F],
}

impl<F: Field> Table<F> for Projected<'_, F> {
    fn len(&self) -> usize {
        2 * self.half
    }
    fn at(&self, _i: usize) -> F {
        unreachable!("a projected table holds its zero runs only")
    }
    fn zero_run(&self, range: std::ops::Range<usize>) -> bool {
        let dense = range.start.min(self.half)..range.end.min(self.half);
        if !dense.is_empty()
            && (dense.start / SEGMENT..=(dense.end - 1) / SEGMENT)
                .any(|s| self.nonzero[s / 64] >> (s % 64) & 1 == 1)
        {
            return false;
        }
        let tail_end = self.half + self.tail.len();
        let high = range.start.clamp(self.half, tail_end)..range.end.clamp(self.half, tail_end);
        self.tail[high.start - self.half..high.end - self.half]
            .iter()
            .all(|x| x.is_zero_vartime())
    }
}

/// The folded table's entries, written by one job at a time each.
struct Shared<F>(*mut F);
unsafe impl<F: Send> Send for Shared<F> {}
unsafe impl<F: Sync> Sync for Shared<F> {}

/// Pass two: the lowest corner scaled by `eq(r, 0)`, then
/// `folded[j] += eq(r, h) M(h, j)` for the other dense corners, one sweep
/// each: in a sweep distinct positions lie in one corner, so they add into
/// distinct outputs.
struct Folding<F> {
    low_bits: usize,
    half: usize,
    weights: Vec<F>,
    out: Vec<F>,
    ptr: Shared<F>,
}

impl<F: Field> Folding<F> {
    /// `lowest` holds `M(0, .)`; only its runs `nonzero` marks hold anything.
    fn new(half: usize, r: &[F], mut out: Vec<F>, nonzero: &[u64]) -> Self {
        let k = r.len();
        let low = (2 * half) >> k;
        assert_eq!(out.len(), low, "the lowest corner");
        // Corner h's weight eq(r, h), the first variable most significant.
        let weights: Vec<F> = (0..1usize << k)
            .map(|h| {
                r.iter()
                    .enumerate()
                    .map(|(t, &rt)| {
                        if (h >> (k - 1 - t)) & 1 == 1 {
                            rt
                        } else {
                            F::ONE - rt
                        }
                    })
                    .product()
            })
            .collect();
        let w0 = weights[0];
        out.par_chunks_mut(SEGMENT)
            .enumerate()
            .for_each(|(s, run)| {
                if nonzero[s / 64] >> (s % 64) & 1 == 1 {
                    run.iter_mut().for_each(|x| *x *= w0);
                }
            });
        let ptr = Shared(out.as_mut_ptr());
        Folding {
            low_bits: low.trailing_zeros() as usize,
            half,
            weights,
            out,
            ptr,
        }
    }

    /// The public half's entries added: the folded table.
    fn finish(self, tail: &[F]) -> Vec<F> {
        let Folding {
            low_bits,
            half,
            weights,
            mut out,
            ..
        } = self;
        for (t, &value) in tail.iter().enumerate() {
            let pos = half + t;
            out[pos & ((1 << low_bits) - 1)] += weights[pos >> low_bits] * value;
        }
        out
    }
}

impl<F: Field> Entries<F> for Folding<F> {
    type Local = ();
    fn local(&self) {}
    fn sweeps(&self) -> usize {
        (self.half >> self.low_bits) - 1
    }
    fn sweep(&self, s: usize) -> std::ops::Range<usize> {
        (s + 1) << self.low_bits..(s + 2) << self.low_bits
    }
    fn put(&self, _: &mut (), pos: usize, value: F) {
        assert!(
            pos >> self.low_bits >= 1 && pos < self.half,
            "a dense entry above the lowest corner"
        );
        let j = pos & ((1 << self.low_bits) - 1);
        // SAFETY: `j` is below the table's `low` entries, which outlive the
        // passes; a generator emits a position once per sweep and a sweep's
        // positions lie in one corner (`Generate`), so no two jobs add into
        // the same entry at once.
        unsafe {
            *self.ptr.0.add(j) += self.weights[pos >> self.low_bits] * value;
        }
    }
    fn absorb(&self, _: ()) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::SpikeRng;
    use crate::grid::{Extender, eq_table};
    use iroha_pasta::Fp;

    #[test]
    fn mutable_rows_reuse_authored_values_and_preserve_implicit_layout() {
        use super::{Cells, Rows};
        use crate::r1cs::RowFill;
        let mut storage = (1_u64..=12).map(Fp::from).collect::<Vec<_>>();
        let initial = storage.clone();
        let fill = [RowFill::Values, RowFill::Zero, RowFill::Unit];
        let mut shared = Rows::with_fill(&initial, 4, &fill);
        let mut exclusive = Rows::with_mutable_values(&mut storage, 4, &fill);
        for i in 0..12 {
            assert_eq!(shared.cell(i), exclusive.cell(i));
        }
        for i in [2, 7, 8] {
            shared.set_cell(i, Fp::from(29));
            exclusive.set_cell(i, Fp::from(29));
        }
        assert_eq!(shared.slices(), exclusive.slices());
        assert_eq!(shared.owned(), 3);
        assert_eq!(exclusive.owned(), 2);
        exclusive.zeroize_owned();
        drop(exclusive);
        assert_eq!(storage[2], Fp::from(29));
        assert_eq!(storage[4..], initial[4..]);
    }

    #[test]
    fn folded_witness_matches_the_materialized_table_on_both_curves() {
        fn check<F: ff::PrimeField>() {
            let n = 1 << 16;
            let seg = super::SEGMENT;
            let values: Vec<_> = (0..n)
                .map(|i| {
                    if i / seg % 7 < 3 {
                        F::from((i % 251) as u64)
                    } else {
                        F::ZERO
                    }
                })
                .collect();
            let live = super::Live {
                seg,
                runs: (0..n / seg).map(|i| i % 7 < 3).collect(),
            };
            let z = super::Masked::new(&values, &live);
            let r = [F::from(3), F::from(5), F::from(7)];
            let matrix: Vec<_> = (0..n).map(|i| F::from((i % 149) as u64)).collect();
            let support = super::support(3, &super::Public(&matrix), &z);
            let dense = super::fold_top(&z, &r, Some(&support));
            let folded = super::FoldedWitness::new(&z, &r, &support);
            assert_eq!(super::Table::len(&folded), dense.len());
            let live = support.folded_live();
            for (i, expected) in dense.iter().enumerate() {
                assert_eq!(super::Table::at(&folded, i), *expected);
            }
            for i in (0..dense.len()).step_by(seg) {
                let end = (i + seg).min(dense.len());
                assert_eq!(
                    super::Table::zero_run(&folded, i..end),
                    live.zero_run(i..end)
                );
            }
            let matrix = super::Public(&matrix[..dense.len()]);
            let second = super::support(3, &matrix, &folded);
            let eager = super::Masked::new(&dense, &live);
            let basis = super::Basis::new(3);
            assert_eq!(
                super::inner_round(&basis, &matrix, &eager, Some(&second)),
                super::inner_round(&basis, &matrix, &folded, Some(&second))
            );
            assert_eq!(
                super::fold_top(&eager, &r, Some(&second)),
                super::fold_top(&folded, &r, Some(&second))
            );
        }
        check::<iroha_pasta::Fp>();
        check::<iroha_pasta::Fq>();
    }

    #[test]
    fn consumed_matrix_matches_logical_dense_fold_on_both_fields() {
        fn check<F: ff::PrimeField>() {
            for half in [16, 128 * SEGMENT] {
                // Unwritten segments contain junk to catch any read that bypasses the bitmap.
                let dense: Vec<F> = (0..half).map(|i| F::from((i % 251 + 1) as u64)).collect();
                let zero = vec![F::ZERO; 2 * half];
                let challenges = [F::ZERO, F::ONE, F::from(11), F::from(19), F::from(23)];
                for (bits, tail_len) in [(None, half), (Some([0, 0]), 0), (Some([1 << 63, 1]), 3)] {
                    let table = || Split {
                        dense: dense.clone(),
                        tail: dense[..tail_len].to_vec(),
                        written: bits.map(Vec::from),
                    };
                    let matrix = table();
                    let logical: Vec<F> = (0..matrix.len()).map(|i| matrix.at(i)).collect();
                    for k in 1..=5 {
                        let r = &challenges[..k];
                        let expected = fold_top(&logical, r, None);
                        let runs = support(k, &matrix, &Public(&zero));
                        assert_eq!(table().into_folded(r, Some(&runs)), expected);
                        assert_eq!(table().into_folded(r, None), expected);
                    }
                }
            }
        }
        check::<iroha_pasta::Fp>();
        check::<iroha_pasta::Fq>();
    }

    /// The round values the spike's first kernels compute.
    fn reference_round(
        k: usize,
        a: &[Fp],
        b: &[Fp],
        c: Option<&[Fp]>,
        e: Option<&[Fp]>,
    ) -> Vec<Fp> {
        let extender = Extender::new(k);
        let (cube, grid) = (1 << k, pow3(k));
        let low = a.len() >> k;
        let mut acc = vec![Fp::ZERO; grid];
        for j in 0..low {
            let pick = |t: &[Fp]| -> Vec<Fp> { (0..cube).map(|hi| t[hi * low + j]).collect() };
            let (mut ga, mut gb, mut gc) = (
                vec![Fp::ZERO; grid],
                vec![Fp::ZERO; grid],
                vec![Fp::ZERO; grid],
            );
            extender.extend(&pick(a), &mut ga);
            extender.extend(&pick(b), &mut gb);
            match (c, e) {
                (Some(c), Some(e)) => {
                    extender.extend(&pick(c), &mut gc);
                    for g in 0..grid {
                        acc[g] += e[j] * (ga[g] * gb[g] - gc[g]);
                    }
                }
                _ => {
                    for g in 0..grid {
                        acc[g] += ga[g] * gb[g];
                    }
                }
            }
        }
        acc
    }

    fn reference_fold(table: &[Fp], r: &[Fp]) -> Vec<Fp> {
        let low = table.len() >> r.len();
        let eq_r = eq_table(r);
        (0..low)
            .map(|j| {
                eq_r.iter()
                    .enumerate()
                    .map(|(hi, e)| *e * table[hi * low + j])
                    .sum()
            })
            .collect()
    }

    #[test]
    fn streamed_constraint_prefix_matches_dense_round_and_fold_on_both_fields() {
        fn check<F: ff::PrimeField>() {
            let cases = [0, 1, 119, 128, 129, 256, 383, 1023, 1024].map(|extent| (3, 128, extent));
            // In the last case the final high corner ends inside one chunk
            // and is wholly absent from the next chunk.
            for (k, low, extent) in cases
                .into_iter()
                .chain([(4, 16, 33), (2, 2 * CHUNK, 3 * CHUNK - 17)])
            {
                let n = low << k;
                let r: Vec<_> = (0..k).map(|i| F::from((19 + 4 * i) as u64)).collect();
                let e: Vec<_> = (0..low).map(|i| F::from((37 * i + 11) as u64)).collect();
                let basis = Basis::new(k);
                let rows = OuterRows::new(low, k, extent);
                let mut tables = [vec![F::ZERO; n], vec![F::ZERO; n], vec![F::ZERO; n]];
                for i in 0..extent {
                    tables[0][i] = F::from((i + 57) as u64);
                    tables[1][i] = F::from((i * i + 3) as u64);
                    tables[2][i] = tables[0][i] * tables[1][i];
                }
                let fill = |js: std::ops::Range<usize>, mats: [bool; 3], out: &mut [Vec<F>; 3]| {
                    assert!(js.end <= extent.min(low));
                    for m in 0..3 {
                        if mats[m] {
                            for (hi, chunk) in out[m].chunks_exact_mut(js.len()).enumerate() {
                                assert!(hi < rows.corners(js.start));
                                chunk.copy_from_slice(
                                    &tables[m][hi * low + js.start..hi * low + js.end],
                                );
                            }
                        }
                    }
                };
                assert_eq!(
                    outer_round_streamed(&basis, rows, &e, &fill),
                    outer_round(&basis, &tables[0], &tables[1], &tables[2], &e, false),
                    "outer grid extent {extent}"
                );
                let folded = fold_top_streamed(rows, &r, &fill);
                for (actual, dense) in folded.iter().zip(&tables) {
                    assert_eq!(*actual, fold_top(dense, &r, None), "fold extent {extent}");
                }
            }
        }
        check::<iroha_pasta::Fp>();
        check::<iroha_pasta::Fq>();
    }

    #[test]
    fn rounds_and_folds_equal_the_reference_kernels() {
        let mut rng = SpikeRng::new(b"kern");
        for k in 1..=4 {
            for log in [k, k + 1, k + 5, k + 10] {
                let n = 1usize << log;
                let low = n >> k;
                let random =
                    |rng: &mut SpikeRng| -> Vec<Fp> { (0..n).map(|_| rng.scalar()).collect() };
                let (a, b, c) = (random(&mut rng), random(&mut rng), random(&mut rng));
                let e: Vec<Fp> = (0..low).map(|_| rng.scalar()).collect();
                let basis = Basis::new(k);
                assert_eq!(
                    outer_round(&basis, &a, &b, &c, &e, false),
                    reference_round(k, &a, &b, Some(&c), Some(&e)),
                    "outer k {k} log {log}"
                );
                // A satisfied relation: c = a b on every row.
                let ab: Vec<Fp> = a.iter().zip(&b).map(|(x, y)| *x * y).collect();
                let expected = reference_round(k, &a, &b, Some(&ab), Some(&e));
                assert_eq!(
                    outer_round(&basis, &a, &b, &ab, &e, true),
                    expected,
                    "satisfied outer k {k} log {log}"
                );
                assert_eq!(
                    outer_round(&basis, &a, &b, &ab, &e, false),
                    expected,
                    "exact outer k {k} log {log}"
                );
                assert_eq!(
                    inner_round(&basis, &a, &b, None),
                    reference_round(k, &a, &b, None, None),
                    "inner k {k} log {log}"
                );
                let r: Vec<Fp> = (0..k).map(|_| rng.scalar()).collect();
                assert_eq!(
                    fold_top(&a, &r, None),
                    reference_fold(&a, &r),
                    "fold k {k} log {log}"
                );
                // Padded tables: per run of rows, corners emptied in both
                // tables by a pattern (the public half beyond a few rows,
                // empty quarters, single corners, everything), with one
                // stray value in m to keep a corner's run alive.
                let (mut m, mut z) = (a.clone(), b.clone());
                let seg = SEGMENT.min(low);
                for run in 0..low.div_ceil(seg) {
                    let pattern = (run * 7 + log) % 6;
                    for hi in 0..1usize << k {
                        let top = hi >> (k - 1) == 1;
                        let empty = match pattern {
                            0 => top,
                            1 => top || (k >= 2 && (hi >> (k - 2)) & 1 == 1),
                            2 => !top,
                            3 => hi != (1 << k) - 1 && hi != 0,
                            4 => true,
                            _ => false,
                        };
                        if empty {
                            for j in run * seg..((run + 1) * seg).min(low) {
                                m[hi * low + j] = Fp::ZERO;
                                z[hi * low + j] = Fp::ZERO;
                            }
                        }
                    }
                }
                if n >= 4 {
                    m[n - 1] = Fp::from(5u64);
                }
                // Read as public tables, so the emptied corners are seen.
                let sup = support(k, &Public(&m), &Public(&z));
                assert_eq!(
                    inner_round(&basis, &m, &z, Some(&sup)),
                    reference_round(k, &m, &z, None, None),
                    "padded inner k {k} log {log}"
                );
                // As private tables nothing is known to be empty; the
                // round evaluates every run, with the same values.
                let dense = support(k, &m, &z);
                assert!(
                    dense
                        .masks
                        .iter()
                        .all(|mask| *mask == (1u32 << (1 << k)) - 1)
                );
                assert_eq!(
                    inner_round(&basis, &m, &z, Some(&dense)),
                    reference_round(k, &m, &z, None, None),
                    "private padded inner k {k} log {log}"
                );
                assert_eq!(
                    inner_round(&basis, &m, &z, None),
                    reference_round(k, &m, &z, None, None),
                    "dense inner on padded k {k} log {log}"
                );
                assert_eq!(
                    fold_top(&m, &r, Some(&sup)),
                    reference_fold(&m, &r),
                    "padded fold of m k {k} log {log}"
                );
                assert_eq!(
                    fold_top(&z, &r, Some(&sup)),
                    reference_fold(&z, &r),
                    "padded fold of z k {k} log {log}"
                );
            }
        }
    }

    /// Witness rows of 64 columns, 32 rows: rows 8..12 and 24..28 never
    /// written (their caller pages hold garbage the prover must not read),
    /// row 5 written with zeros only, row 30 a padding row.
    const COLS: usize = 64;
    const ROWS: usize = 32;

    fn layout() -> Vec<crate::r1cs::RowFill> {
        use crate::r1cs::RowFill;
        (0..ROWS)
            .map(|r| match r {
                8..12 | 24..28 => RowFill::Zero,
                30 => RowFill::Unit,
                _ => RowFill::Values,
            })
            .collect()
    }

    fn witness(rng: &mut SpikeRng, zero_row_5: bool) -> Vec<Fp> {
        let mut w: Vec<Fp> = (0..ROWS * COLS).map(|_| rng.scalar()).collect();
        if zero_row_5 {
            w[5 * COLS..6 * COLS].fill(Fp::ZERO);
        }
        w
    }

    /// `M(r_x, .)` zero where no constraint reads (the rows never written),
    /// with three public entries beyond the witness half.
    fn m_table(rng: &mut SpikeRng) -> Split<Fp> {
        let mut dense: Vec<Fp> = (0..ROWS * COLS).map(|_| rng.scalar()).collect();
        let mut written = vec![0u64; 1];
        for s in 0..ROWS * COLS / SEGMENT {
            // Runs 2 and 6 are rows 8..12 and 24..28.
            if s == 2 || s == 6 {
                dense[s * SEGMENT..(s + 1) * SEGMENT].fill(Fp::ZERO);
            } else {
                written[0] |= 1 << s;
            }
        }
        let tail = (0..3).map(|_| rng.scalar()).collect();
        Split::with_written(dense, tail, written)
    }

    /// Zero runs of private tables come from public layout only: a slice
    /// knows none, a row of values is live whatever it holds, the io is read
    /// by value, and the support of two witnesses that differ only in their
    /// values is the same.
    #[test]
    fn private_tables_answer_from_public_layout_only() {
        let mut rng = SpikeRng::new(b"kern public layout");
        let zeros = vec![Fp::ZERO; ROWS * COLS];
        assert!(!<[Fp] as Table<Fp>>::zero_run(&zeros, 0..COLS));
        assert!(<[Fp] as Table<Fp>>::zero_run(&zeros, 3..3));
        assert!(Public(&zeros).zero_run(0..COLS));
        let fill = layout();
        let w = witness(&mut rng, true);
        let rows = Rows::with_fill(&w, COLS, &fill);
        let io = [Fp::ONE, Fp::ZERO, Fp::from(3u64)];
        let zv = ZView {
            rows: &rows,
            tail: &io,
        };
        let half = ROWS * COLS;
        assert!(!zv.zero_run(5 * COLS..6 * COLS), "zeros by value stay live");
        assert!(!zv.zero_run(0..COLS));
        assert!(zv.zero_run(8 * COLS..12 * COLS), "rows never written");
        assert!(!zv.zero_run(30 * COLS..30 * COLS + 1), "a padding row's 1");
        assert!(
            zv.zero_run(30 * COLS + 1..31 * COLS),
            "padding beyond column 0"
        );
        assert!(!zv.zero_run(half..half + 1), "public io by value");
        assert!(zv.zero_run(half + 1..half + 2), "a public zero in the io");
        assert!(
            zv.zero_run(half + 3..2 * half),
            "the public half beyond the io"
        );
        let live = Live {
            seg: 4,
            runs: vec![true, false, false, true],
        };
        let masked = Masked::new(&zeros[..16], &live);
        assert!(!masked.zero_run(0..1), "a live run of zeros");
        assert!(masked.zero_run(4..12));
        assert!(!masked.zero_run(5..13));

        let m = m_table(&mut rng);
        let basis = Basis::new(2);
        let mut seen = None;
        for zero_row_5 in [false, true] {
            let w = witness(&mut rng, zero_row_5);
            let rows = Rows::with_fill(&w, COLS, &fill);
            let zv = ZView {
                rows: &rows,
                tail: &io,
            };
            let sup = support(2, &m, &zv);
            let z_full: Vec<Fp> = (0..2 * half).map(|i| zv.get(i)).collect();
            assert_eq!(
                inner_round(&basis, &m, &zv, Some(&sup)),
                reference_round(2, &m.to_full(), &z_full, None, None)
            );
            match &seen {
                None => seen = Some(sup),
                Some(first) => assert_eq!(*first, sup, "a public schedule"),
            }
        }
    }

    /// Every inner round of a padded witness: the first on `M`'s split and
    /// the prover's view of `z`, later ones on the folded public `M` and the
    /// folded private `z` with the runs its layout emptied. Each round and
    /// fold equals the dense reference, a run the layout empties is zero,
    /// and the schedule reaches a reduced sub-cube after the first fold.
    #[test]
    fn masked_rounds_carry_the_witness_layout_forward() {
        let mut rng = SpikeRng::new(b"kern masked rounds");
        let k = 2;
        let basis = Basis::new(k);
        let fill = layout();
        let w = witness(&mut rng, true);
        let rows = Rows::with_fill(&w, COLS, &fill);
        let io: Vec<Fp> = (0..5).map(|_| rng.scalar()).collect();
        let zv = ZView {
            rows: &rows,
            tail: &io,
        };
        let m_split = m_table(&mut rng);
        let half = ROWS * COLS;
        let mut m_ref = m_split.to_full();
        let mut z_ref: Vec<Fp> = (0..2 * half).map(|i| zv.get(i)).collect();
        let sup = support(k, &m_split, &zv);
        assert_eq!(
            inner_round(&basis, &m_split, &zv, Some(&sup)),
            reference_round(k, &m_ref, &z_ref, None, None),
            "first round"
        );
        let r: Vec<Fp> = (0..k).map(|_| rng.scalar()).collect();
        let mut m = fold_top(&m_split, &r, Some(&sup));
        let mut z = fold_top(&zv, &r, Some(&sup));
        m_ref = reference_fold(&m_ref, &r);
        z_ref = reference_fold(&z_ref, &r);
        assert_eq!((&m, &z), (&m_ref, &z_ref), "first fold");
        let mut live = sup.folded_live();
        // Run 2 of the fold is rows 8..12 and 24..28 at every corner.
        assert_eq!(live.runs, [true, true, false, true]);
        let mut reduced = false;
        while m.len() > 1 {
            let k = k.min(m.len().trailing_zeros() as usize);
            let basis = Basis::new(k);
            for (s, run) in live.runs.iter().enumerate() {
                let run_values = &z[s * live.seg..((s + 1) * live.seg).min(z.len())];
                assert!(*run || run_values.iter().all(|x| x.is_zero_vartime()));
            }
            let (mt, zt) = (Public(&m), Masked::new(&z, &live));
            let sup = support(k, &mt, &zt);
            reduced |= sup.masks.iter().any(|mask| *mask != (1u32 << (1 << k)) - 1);
            assert_eq!(
                inner_round(&basis, &mt, &zt, Some(&sup)),
                reference_round(k, &m, &z, None, None),
                "round over {} entries",
                m.len()
            );
            let r: Vec<Fp> = (0..k).map(|_| rng.scalar()).collect();
            let (m2, z2) = (fold_top(&mt, &r, Some(&sup)), fold_top(&zt, &r, Some(&sup)));
            assert_eq!(m2, reference_fold(&m, &r), "fold of M over {}", m.len());
            assert_eq!(z2, reference_fold(&z, &r), "fold of z over {}", z.len());
            live = sup.folded_live();
            (m, z) = (m2, z2);
        }
        assert!(reduced, "the carried layout reduces a later round");
    }

    /// `M(r_x, .)` as a relation computes it: dense entries at distinct
    /// positions (zero values among them) in any order, emitted in parallel
    /// jobs of 7, and the public half.
    struct Listed<F> {
        dense: Vec<(usize, F)>,
        tail: Vec<F>,
    }

    // SAFETY: the positions are distinct and a sweep emits only its own.
    unsafe impl<F: Field> Generate<F> for Listed<F> {
        fn run<S: Entries<F>>(&self, sink: &S, tail: bool) -> Option<Vec<F>> {
            for s in 0..sink.sweeps() {
                let range = sink.sweep(s);
                self.dense
                    .par_chunks(7)
                    .fold(
                        || sink.local(),
                        |mut local, jobs| {
                            for &(pos, value) in jobs {
                                if range.contains(&pos) {
                                    sink.put(&mut local, pos, value);
                                }
                            }
                            local
                        },
                    )
                    .for_each(|local| sink.absorb(local));
            }
            tail.then(|| self.tail.clone())
        }
    }

    /// The split table holding the listed entries, their runs written.
    fn listed_split<F: Field>(half: usize, listed: &Listed<F>) -> Split<F> {
        let mut dense = vec![F::ZERO; half];
        let mut written = vec![0u64; half.div_ceil(SEGMENT).div_ceil(64)];
        for &(pos, value) in &listed.dense {
            dense[pos] = value;
            written[pos / SEGMENT / 64] |= 1 << (pos / SEGMENT % 64);
        }
        Split::with_written(dense, listed.tail.clone(), written)
    }

    fn below(rng: &mut SpikeRng, n: usize) -> usize {
        use ff::PrimeField;
        let x: Fp = rng.scalar();
        u64::from_le_bytes(x.to_repr()[..8].try_into().unwrap()) as usize % n
    }

    /// The streamed first round equals the split table's (values, the
    /// challenges `challenge` draws once from them, the support's union and z
    /// masks, the folded M) and the dense kernels, for empty, zero, sparse,
    /// dense, boundary and tail-only matrices, M entries where z is zero by
    /// layout, io spanning several public corners, with and without support.
    #[test]
    fn streamed_first_round_equals_the_split_table_on_both_fields() {
        fn check<F: crate::fs::Scalar>() {
            use crate::r1cs::RowFill;
            let mut rng = SpikeRng::new(b"kern streamed first round");
            let mut cases = 0;
            for (rows, cols) in [(8usize, 512usize), (4, 16), (2, 256), (16, 64)] {
                let half = rows * cols;
                let fill: Vec<RowFill> = (0..rows)
                    .map(|r| match r % 4 {
                        1 => RowFill::Zero,
                        3 if r + 1 == rows => RowFill::Unit,
                        _ => RowFill::Values,
                    })
                    .collect();
                let w: Vec<F> = (0..half).map(|_| rng.scalar()).collect();
                let rows_view = Rows::with_fill(&w, cols, &fill);
                // Rows a witness never wrote, and padding beyond column 0.
                let z_zero = |pos: usize| match fill[pos / cols] {
                    RowFill::Zero => true,
                    RowFill::Unit => pos % cols != 0,
                    RowFill::Values => false,
                };
                for k in 1..=5usize {
                    let low = (2 * half) >> k;
                    if low == 0 {
                        continue;
                    }
                    for pattern in [
                        "empty", "zero", "sparse", "dense", "boundary", "z-zero", "tail",
                    ] {
                        let random = |rng: &mut SpikeRng| -> F {
                            if below(rng, 5) == 0 {
                                F::ZERO
                            } else {
                                rng.scalar()
                            }
                        };
                        let mut positions: Vec<usize> = match pattern {
                            "empty" | "tail" => Vec::new(),
                            "zero" | "dense" => (0..half).collect(),
                            "sparse" => (0..half)
                                .filter(|p| (p / SEGMENT.min(half) + p / 3) % 5 == 0)
                                .collect(),
                            "boundary" => {
                                let mut b: Vec<usize> = (0..=2 * (half / low))
                                    .flat_map(|c| [c * low / 2, (c * low / 2).wrapping_sub(1)])
                                    .chain([0, half - 1, SEGMENT - 1, SEGMENT])
                                    .filter(|p| *p < half)
                                    .collect();
                                b.sort_unstable();
                                b.dedup();
                                b
                            }
                            _ => (0..half).filter(|p| z_zero(*p)).collect(),
                        };
                        // Any emission order.
                        for i in (1..positions.len()).rev() {
                            positions.swap(i, below(&mut rng, i + 1));
                        }
                        let zero = pattern == "zero";
                        let dense = positions
                            .into_iter()
                            .map(|p| (p, if zero { F::ZERO } else { random(&mut rng) }))
                            .collect();
                        let m_tail = [1, 3, low + 2][below(&mut rng, 3)].min(half);
                        let tail = (0..m_tail)
                            .map(|_| {
                                if pattern == "zero" {
                                    F::ZERO
                                } else {
                                    random(&mut rng)
                                }
                            })
                            .collect();
                        let listed = Listed { dense, tail };
                        let io_len = [0, 2, low + 3, 2 * low + 1][cases % 4].min(half);
                        let mut io: Vec<F> = (0..io_len).map(|_| random(&mut rng)).collect();
                        if let Some(last) = io.last_mut() {
                            *last = F::ONE;
                        }
                        let zv = ZView {
                            rows: &rows_view,
                            tail: &io,
                        };
                        let basis = Basis::new(k);
                        let challenge_of = |values: &[F]| -> Vec<F> {
                            let s: F = values
                                .iter()
                                .enumerate()
                                .map(|(g, v)| *v * F::from(g as u64 + 3))
                                .sum();
                            (0..k).map(|t| s + F::from(7919 * t as u64 + 1)).collect()
                        };
                        let mut seen = Vec::new();
                        let table = listed_split(half, &listed);
                        let split = first_inner_split(&basis, table, &zv, &mut |v| {
                            seen.push(v.to_vec());
                            challenge_of(v)
                        });
                        let streamed = first_inner_streamed(&basis, half, &zv, &listed, &mut |v| {
                            seen.push(v.to_vec());
                            challenge_of(v)
                        });
                        let label = format!(
                            "rows {rows} cols {cols} k {k} {pattern} io {io_len} tail {m_tail}"
                        );
                        assert_eq!(seen.len(), 2, "{label}: one commitment each");
                        assert_eq!(seen[0], seen[1], "{label}: committed values");
                        assert_eq!(streamed.values, split.values, "{label}: values");
                        assert_eq!(streamed.r, split.r, "{label}: challenges");
                        assert_eq!(streamed.support, split.support, "{label}: masks");
                        assert_eq!(streamed.m, split.m, "{label}: folded M");
                        assert_eq!(split.support.is_some(), low >= SEGMENT, "{label}");
                        // The dense kernels on the whole tables.
                        let m_full = listed_split(half, &listed).to_full();
                        let z_full: Vec<F> = (0..2 * half).map(|i| zv.get(i)).collect();
                        let dense_round = inner_round(&basis, &m_full, &z_full, None);
                        assert_eq!(streamed.values, dense_round, "{label}");
                        assert_eq!(streamed.m, fold_top(&m_full, &streamed.r, None), "{label}");
                        let live = listed.dense.iter().any(|(_, v)| !v.is_zero_vartime());
                        if pattern == "z-zero" && live {
                            assert!(
                                streamed.m.iter().any(|x| !x.is_zero_vartime()),
                                "{label}: M kept where z is zero by layout"
                            );
                        }
                        cases += 1;
                    }
                }
            }
            assert!(cases >= 100, "{cases} cases");
        }
        check::<iroha_pasta::Fp>();
        check::<iroha_pasta::Fq>();
    }
}
