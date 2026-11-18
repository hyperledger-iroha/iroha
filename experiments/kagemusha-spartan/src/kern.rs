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
//! - Inner rounds read which corners of the round's variables hold anything,
//!   per run of 256 rows ([`support`]): the public half of `z` and of
//!   `M(r_x, .)` is zero beyond the public inputs, and a padded witness has
//!   empty rows. A variable whose upper (or lower) half is empty in both
//!   tables contributes the factor `(1 - Y)^2` (or `Y^2`), so the round runs
//!   on the remaining sub-cube (9 products instead of 27 for the public half
//!   alone, 3 where half the rows are empty too), and so does its fold.
//! - Tables fold by a tree (`2^k - 1` products per output instead of `2^k`).
//! - Every loop is cut into leaves of bounded length, so a slow core never
//!   holds a large share of a pass (unequal cores: a Pixel 6's 2 + 2 + 4).

use ff::Field;
use rayon::prelude::*;

use crate::grid::{boolean_positions, pow3};

/// Longest leaf of a parallel pass, in rows of the folded table.
const LEAF: usize = 1 << 10;

/// A table the inner rounds read: a plain slice, or a [`Split`] table whose
/// upper half is zero beyond a short prefix.
pub trait Table<F>: Sync {
    fn len(&self) -> usize;
    fn at(&self, i: usize) -> F;
    /// Whether every entry in `range` is zero.
    fn zero_run(&self, range: std::ops::Range<usize>) -> bool;
}

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
        self[range].iter().all(|x| x.is_zero_vartime())
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

/// `(dense || tail || 0...)` of length `2 * dense.len()`: `z = (w || io)` and
/// `M(r_x, .)` before the first inner fold. Their upper (public) half is zero
/// beyond the public inputs, so it is never allocated.
pub struct Split<F> {
    pub dense: Vec<F>,
    pub tail: Vec<F>,
    /// Which runs of [`SEGMENT`] dense entries were written (bit `s % 64` of
    /// word `s / 64`); a run never written is zero and is never read, so a
    /// zero-on-demand dense half holds memory only where written. `None`:
    /// every run may hold anything.
    pub written: Option<Vec<u64>>,
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
    /// Whether dense run `s` (entries `s * SEGMENT..`) may hold anything.
    #[inline]
    fn run_written(&self, s: usize) -> bool {
        self.written
            .as_ref()
            .is_none_or(|bits| bits[s / 64] >> (s % 64) & 1 == 1)
    }
    /// The upper half's nonzero prefix becomes the tail.
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
        } else if self.run_written(i / SEGMENT) {
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
        // Runs never written are zero unread.
        let mut at = low.start;
        while at < low.end {
            let s = at / SEGMENT;
            let end = ((s + 1) * SEGMENT).min(low.end);
            if self.run_written(s) && !self.dense[at..end].iter().all(|x| x.is_zero_vartime()) {
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

/// The prover's witness as its Hyrax rows: every row is the caller's until
/// the prover writes into it (masks, the link phase, the second phase), and
/// only a written row is copied. The witness itself is never cloned. Rows the
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
        use crate::r1cs::RowFill;
        assert!(cols.is_power_of_two() && w.len() % cols == 0, "whole rows");
        assert_eq!(fill.len(), w.len() / cols, "a fill per row");
        Rows {
            shift: cols.trailing_zeros() as usize,
            rows: w
                .chunks_exact(cols)
                .zip(fill)
                .map(|(row, f)| match f {
                    RowFill::Values => Row::Shared(row),
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
            Row::Own(_) => {}
        }
        match &mut self.rows[r] {
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
/// its tail (the io) and zero beyond.
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
            let zero = match &self.rows.rows[r] {
                Row::Shared(row) => row[c..c + n].iter().all(|x| x.is_zero_vartime()),
                Row::Own(row) => row[c..c + n].iter().all(|x| x.is_zero_vartime()),
                Row::Zero => true,
                Row::Unit => c != 0,
            };
            if !zero {
                return false;
            }
            at += n;
        }
        let high = range.start.clamp(half, tail_end)..range.end.clamp(half, tail_end);
        self.tail[high.start - half..high.end - half]
            .iter()
            .all(|x| x.is_zero_vartime())
    }
}

/// A zero vector whose pages the operating system supplies on first write
/// (large allocations are fresh mappings), so a table written only in part
/// holds memory only where written. Falls back to writing zeros for a field
/// whose zero is not the all-zero bit pattern.
pub fn zeroed<F: Field>(n: usize) -> Vec<F> {
    let zero = F::ZERO;
    // SAFETY: reads the bytes of one field element (Pasta field elements are
    // plain arrays of limbs, without padding).
    let zero_bits =
        unsafe { std::slice::from_raw_parts((&zero as *const F).cast::<u8>(), size_of::<F>()) }
            .iter()
            .all(|b| *b == 0);
    if !zero_bits || n == 0 || size_of::<F>() == 0 {
        return vec![F::ZERO; n];
    }
    let layout = std::alloc::Layout::array::<F>(n).expect("table size");
    // SAFETY: the layout has nonzero size; the allocation is zeroed and the
    // all-zero bit pattern is `F::ZERO` (checked above), so all `n` elements
    // are initialized; `Vec` frees it with the same layout.
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
    low: usize,
    e: &[F],
    fill: &Corners<'_, F>,
) -> Vec<F> {
    let (cube, grid) = (basis.cube, basis.grid);
    assert_eq!(e.len(), low);
    let chunk = CHUNK.min(low);
    let mut acc = (0..low.div_ceil(chunk))
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
                let js = ci * chunk..((ci + 1) * chunk).min(low);
                let n = js.len();
                for t in tables.iter_mut().take(2) {
                    t.clear();
                    t.resize(cube * n, F::ZERO);
                }
                fill(js.clone(), [true, true, false], &mut tables);
                for (jj, j) in js.enumerate() {
                    let weight = e[j];
                    let (ca, cb) = corners.split_at_mut(cube);
                    for hi in 0..cube {
                        ca[hi] = weight * tables[0][hi * n + jj];
                        cb[hi] = tables[1][hi * n + jj];
                    }
                    let (ga, gb) = ext.split_at_mut(grid);
                    basis.extend(ca, ga);
                    basis.extend(cb, gb);
                    for &g in &basis.upper {
                        acc[g] += ga[g] * gb[g];
                    }
                }
                (acc, corners, ext, tables)
            },
        )
        .map(|(acc, ..)| acc)
        .reduce(|| vec![F::ZERO; grid], add_into);
    basis.to_grid(&mut acc);
    acc
}

/// [`fold_top`] (without a support) of the unfolded `Az, Bz, Cz`, which
/// `fill` computes a chunk of low rows at a time.
pub fn fold_top_streamed<F: Field>(
    k: usize,
    low: usize,
    r: &[F],
    fill: &Corners<'_, F>,
) -> [Vec<F>; 3] {
    assert_eq!(r.len(), k);
    let cube = 1usize << k;
    let chunk = CHUNK.min(low);
    let mut out = [vec![F::ZERO; low], vec![F::ZERO; low], vec![F::ZERO; low]];
    let [oa, ob, oc] = &mut out;
    oa.par_chunks_mut(chunk)
        .zip(ob.par_chunks_mut(chunk))
        .zip(oc.par_chunks_mut(chunk))
        .enumerate()
        .for_each(|(ci, ((a, b), c))| {
            let js = ci * chunk..ci * chunk + a.len();
            let n = js.len();
            let mut tables = [
                vec![F::ZERO; cube * n],
                vec![F::ZERO; cube * n],
                vec![F::ZERO; cube * n],
            ];
            fill(js, [true, true, true], &mut tables);
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

/// Which corners of a round's `k` variables hold anything: per run of
/// [`SEGMENT`] rows `j`, bit `hi` is set when `m` or `z` is nonzero at corner
/// `hi` somewhere in the run. Zero runs are public structure almost always
/// (the public half beyond the inputs, rows without cells); a run of cells
/// that are all zero by value is treated the same way, and the result is the
/// same either way.
pub struct Support {
    k: usize,
    seg: usize,
    masks: Vec<u32>,
}

/// Scans both tables (a run stops at its first nonzero entry).
pub fn support<F: Field, M: Table<F> + ?Sized, Z: Table<F> + ?Sized>(
    k: usize,
    m: &M,
    z: &Z,
) -> Support {
    assert!(k <= 5 && z.len() == m.len());
    let low = m.len() >> k;
    let seg = SEGMENT.min(low).max(1);
    let masks = (0..low.div_ceil(seg))
        .into_par_iter()
        .with_max_len(16)
        .map(|s| {
            let (j0, j1) = (s * seg, ((s + 1) * seg).min(low));
            (0..1usize << k).fold(0u32, |mask, hi| {
                let run = hi * low + j0..hi * low + j1;
                let any = !m.zero_run(run.clone()) || !z.zero_run(run);
                mask | (u32::from(any) << hi)
            })
        })
        .collect();
    Support { k, seg, masks }
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
    assert_eq!(support.k, k);
    let (list, runs) = reductions(support);
    // Per reduction: the challenges of its free variables and the factor of
    // its other variables.
    let bound: Vec<(Vec<F>, Option<F>)> = list
        .iter()
        .map(|(_, reduction)| {
            let free = reduction.free.iter().map(|t| r[*t]).collect();
            let factor = (reduction.low.len() + reduction.high.len() > 0).then(|| {
                reduction
                    .low
                    .iter()
                    .map(|t| F::ONE - r[*t])
                    .chain(reduction.high.iter().map(|t| r[*t]))
                    .product()
            });
            (free, factor)
        })
        .collect();
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
                let (reduction, (free, factor)) = (&list[run as usize].1, &bound[run as usize]);
                for (c, hi) in reduction.corners.iter().enumerate() {
                    corners[c] = table.at(hi * low + j);
                }
                let value = fold_corners(&mut corners[..reduction.corners.len()], free);
                factor.map_or(value, |f| f * value)
            },
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::SpikeRng;
    use crate::grid::{Extender, eq_table};
    use iroha_pasta::Fp;

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
                let sup = support(k, &m, &z);
                assert_eq!(
                    inner_round(&basis, &m, &z, Some(&sup)),
                    reference_round(k, &m, &z, None, None),
                    "padded inner k {k} log {log}"
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
}
