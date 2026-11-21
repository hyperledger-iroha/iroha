//! Count the same symbolic sources used by the lazy kernels, without cloning
//! their coefficient/map arrays or allocating any reader/CSR/M-table plan.

use super::*;

#[derive(Clone, Copy)]
enum Source<'a> {
    One,
    Own(usize),
    Link(usize, usize, &'a [Option<usize>]),
    Identity(usize, usize, usize),
    All(usize, usize),
}
#[derive(Clone, Copy)]
enum Coefficients<'a, F> {
    Slice(&'a [F]),
    Ones(usize),
}
#[derive(Clone, Copy)]
struct TermRef<'a, F> {
    source: Source<'a>,
    coefficients: Coefficients<'a, F>,
}
impl<'a, F: Scalar> TermRef<'a, F> {
    fn borrowed(term: &'a Term<F>) -> Self {
        let source = match &term.src {
            Src::One => Source::One,
            Src::Own(k) => Source::Own(*k),
            Src::Link { array, cell, map } => Source::Link(*array, *cell, map),
            Src::All { array, cell } => Source::All(*array, *cell),
        };
        Self {
            source,
            coefficients: Coefficients::Slice(&term.coef),
        }
    }
    fn ones(source: Source<'a>, n: usize) -> Self {
        Self {
            source,
            coefficients: Coefficients::Ones(n),
        }
    }
    fn len(self) -> usize {
        match self.coefficients {
            Coefficients::Slice(v) => v.len(),
            Coefficients::Ones(n) => n,
        }
    }
    fn at(self, r: usize) -> F {
        match self.coefficients {
            Coefficients::Slice(v) => v[r],
            Coefficients::Ones(_) => F::ONE,
        }
    }
}

/// `push`/filtered `collect` vectors grow geometrically; unlike the former 4x
/// term estimate, this allowance applies only to actual entries of each vector.
fn growing<T>(n: u128) -> u128 {
    if n == 0 {
        0
    } else {
        n.saturating_mul(2).max(4) * size_of::<T>() as u128
    }
}

#[derive(Default)]
struct BlockCounts {
    readers: u128,
    all: u128,
    /// Reads of the block's readers (each over its whole map), the bound
    /// that groups the flat plans.
    reads: u128,
    #[cfg(test)]
    entries: u128,
    #[cfg(test)]
    coefficients: u128,
}

#[derive(Default)]
pub(super) struct PlanCounts {
    pub symbolic: u128,
    pub gather: u128,
    pub gather_scratch: u128,
    pub readers: u128,
    pub m_plans: u128,
    /// The largest group of flat `M(r_x, .)` plans alive at once (built and
    /// dropped per group while the M table is written).
    pub m_plan_group: u128,
    pub matrix_scratch: u128,
    pub helpers: u128,
    /// Pages of `M(r_x, .)`'s dense half its plans write ([`M_PAGE`] bytes
    /// each); the inner rounds never touch another.
    pub m_pages: u128,
    /// The streamed outer round's job index and sum-term values.
    pub gather_index: u128,
    #[cfg(test)]
    pub entry_count: u128,
    #[cfg(test)]
    pub coefficient_count: u128,
}

/// Page size of the resident-memory bound: the larger of the 4 KiB and
/// 16 KiB pages of the supported platforms.
pub(super) const M_PAGE: usize = 16 << 10;

#[derive(Clone, Copy)]
pub(super) enum PlanScope {
    Preparation,
    FullProof,
    #[cfg(test)]
    CountMatrixEntries,
}

enum Replacement {
    Lookup(usize, usize),
    Table(usize),
    Export(usize),
    Challenge(usize),
}

impl<F: Scalar> UniformCircuit<F> {
    pub(super) fn memory_plan_counts(&self, scope: PlanScope) -> PlanCounts {
        let field = size_of::<F>() as u128;
        let workers = rayon::current_num_threads() as u128;
        let mut counts = PlanCounts::default();
        let mut matrix_blocks: Option<Vec<Vec<BlockCounts>>> =
            (!matches!(scope, PlanScope::Preparation)).then(|| {
                self.arrays
                    .iter()
                    .map(|a| (0..a.cells()).map(|_| BlockCounts::default()).collect())
                    .collect()
            });
        let mut one_readers = 0u128;
        let mut jobs = 0u128;
        // The longest rows: at most two jobs per worker hold their resolved
        // terms at once (a running one and a suspended outer one).
        let resident_jobs = 2 * workers as usize;
        let mut longest: std::collections::BinaryHeap<std::cmp::Reverse<u128>> =
            std::collections::BinaryHeap::new();
        let mut max_present = 0u128;
        let mut max_sums = 0u128;
        let mut replacements = HashMap::new();
        for &(a, first_cell, first_row) in &self.lookup_arrays {
            for j in 0..self.arrays[a].lookups.len() {
                replacements.insert((a, first_row + j), Replacement::Lookup(j, first_cell));
            }
        }
        for &(va, _, wa) in &self.tables {
            replacements.insert((wa, 0), Replacement::Table(va));
        }
        for (index, (_, wa, _, _)) in self.exports.iter().enumerate() {
            replacements.insert((*wa, 0), Replacement::Export(index));
        }
        for (index, (a, j, _)) in self.ch_rows.iter().enumerate() {
            replacements.insert((*a, *j), Replacement::Challenge(index));
        }
        // The symbolic rows are one kind per template row; their terms are
        // the circuit's own, borrowed.
        counts.symbolic += self.arrays.len() as u128 * size_of::<Vec<RowKind>>() as u128;
        for (a, arr) in self.arrays.iter().enumerate() {
            let n = 1usize << arr.rho;
            let present = (0..n).filter(|r| self.r_present(a, *r)).count() as u128;
            max_present = max_present.max(present);
            counts.symbolic += arr.rows.len() as u128 * size_of::<RowKind>() as u128;
            for (j, original) in arr.rows.iter().enumerate() {
                let row: [Vec<TermRef<'_, F>>; 3] = {
                    match replacements.get(&(a, j)) {
                        Some(Replacement::Challenge(index)) => std::array::from_fn(|mat| {
                            self.ch_rows[*index].2[mat]
                                .iter()
                                .map(|(t, _)| TermRef::borrowed(t))
                                .collect()
                        }),
                        Some(Replacement::Lookup(index, first_cell)) => {
                            let tuple = &arr.lookups[*index].1;
                            let mut b = vec![TermRef::ones(Source::One, n)];
                            b.extend(tuple.iter().flatten().map(TermRef::borrowed));
                            let rhs = if let Some(&(x, k)) = self.lookup_mask_col.get(&(a, *index))
                            {
                                TermRef::ones(Source::Identity(x, k, n), n)
                            } else if let Some(on) = self.lookup_mask.get(&(a, *index)) {
                                TermRef {
                                    source: Source::One,
                                    coefficients: Coefficients::Slice(on),
                                }
                            } else {
                                TermRef::ones(Source::One, n)
                            };
                            [
                                vec![TermRef::ones(Source::Own(first_cell + index), n)],
                                b,
                                vec![rhs],
                            ]
                        }
                        Some(Replacement::Table(va)) => {
                            let mut b = vec![TermRef::ones(Source::One, n)];
                            b.extend(
                                (0..self.arrays[*va].cells())
                                    .map(|k| TermRef::ones(Source::Identity(*va, k, n), n)),
                            );
                            [
                                vec![TermRef::ones(Source::Own(1), n)],
                                b,
                                vec![TermRef::ones(Source::Own(0), n)],
                            ]
                        }
                        Some(Replacement::Export(index)) => {
                            let mut b = vec![TermRef::ones(Source::One, n)];
                            b.extend(
                                self.exports[*index]
                                    .3
                                    .iter()
                                    .flatten()
                                    .map(TermRef::borrowed),
                            );
                            [
                                vec![TermRef::ones(Source::Own(1), n)],
                                b,
                                vec![TermRef::ones(Source::Own(0), n)],
                            ]
                        }
                        None => std::array::from_fn(|mat| {
                            original[mat].iter().map(TermRef::borrowed).collect()
                        }),
                    }
                };
                for lin in &row {
                    let len = lin.len() as u128;
                    jobs += 1;
                    longest.push(std::cmp::Reverse(len));
                    if longest.len() > resident_jobs {
                        longest.pop();
                    }
                    let Some(blocks) = matrix_blocks.as_mut() else {
                        let sums = lin
                            .iter()
                            .filter(|term| matches!(term.source, Source::All(..)))
                            .count();
                        max_sums = max_sums.max(sums as u128);
                        continue;
                    };
                    let mut sums = 0u128;
                    for &term in lin {
                        let target = match term.source {
                            Source::One => {
                                one_readers += 1;
                                None
                            }
                            Source::All(a2, k2) => {
                                sums += 1;
                                blocks[a2][k2].all += 1;
                                None
                            }
                            Source::Own(k) => Some((a, k)),
                            Source::Link(a2, k2, _) | Source::Identity(a2, k2, _) => Some((a2, k2)),
                        };
                        if let Some((a2, k2)) = target {
                            let block = &mut blocks[a2][k2];
                            block.readers += 1;
                            block.reads += term.len() as u128;
                            #[cfg(test)]
                            if matches!(scope, PlanScope::CountMatrixEntries) {
                                for r in 0..term.len() {
                                    if matches!(term.source, Source::Link(_, _, map) if map[r].is_none())
                                    {
                                        continue;
                                    }
                                    if !self.r_present(a, r) {
                                        continue;
                                    }
                                    let value = term.at(r);
                                    if bool::from(value.is_zero()) {
                                        continue;
                                    }
                                    block.entries += 1;
                                    if value != F::ONE && value != -F::ONE {
                                        block.coefficients += 1;
                                    }
                                }
                            }
                        }
                    }
                    max_sums = max_sums.max(sums);
                }
            }
        }
        counts.gather += growing::<GatherJob>(jobs);
        // A job's terms, targets, factor values and bases are resolved when
        // it runs: the longest rows' at once, for as many jobs as can be
        // resident; present r and sum terms are per active worker.
        let per_term = (size_of::<(TermView<'_, F>, Factor)>()
            + size_of::<Option<GatherTarget>>()
            + size_of::<Option<F>>()
            + size_of::<usize>()) as u128;
        counts.gather_scratch = longest.iter().map(|len| len.0).sum::<u128>() * per_term
            + workers * (growing::<usize>(max_sums) + max_present * size_of::<u32>() as u128);
        // The job index: each job in every chunk of rows its block spans.
        let bits = crate::kern::CHUNK.trailing_zeros();
        let chunks = (1u128 << self.shape.sx).div_ceil(1 << bits);
        let mut spans = 0u128;
        let mut summed = 0u128;
        for (a, arr) in self.arrays.iter().enumerate() {
            let len = (self.counts[arr.family] as u128) << arr.rho;
            for j in 0..arr.rows.len() {
                let base = self.row_base[a][j] as u128;
                if len > 0 {
                    spans += 3 * (((base + len - 1) >> bits) - (base >> bits) + 1);
                }
                let sums = match replacements.get(&(a, j)) {
                    None => arr.rows[j]
                        .iter()
                        .filter(|lin| lin.iter().any(|t| matches!(t.src, Src::All { .. })))
                        .count(),
                    Some(Replacement::Challenge(index)) => self.ch_rows[*index]
                        .2
                        .iter()
                        .filter(|lin| lin.iter().any(|(t, _)| matches!(t.src, Src::All { .. })))
                        .count(),
                    Some(_) => 0,
                };
                summed += sums as u128;
            }
        }
        counts.gather_index = chunks * (size_of::<Vec<u32>>() as u128 + 16)
            + 2 * spans * size_of::<u32>() as u128
            + 2 * summed * (size_of::<u32>() + size_of::<F>() + size_of::<u64>()) as u128;
        counts.helpers = self.helper_batch_capacity() as u128 * size_of::<HelperSlot<F>>() as u128;
        let Some(blocks) = matrix_blocks else {
            return counts;
        };
        counts.readers += self.arrays.len() as u128 * 2 * size_of::<Vec<Vec<Reader>>>() as u128
            + growing::<Reader>(one_readers);
        let mut plans = 0u128;
        let mut matrix_jobs = 0u128;
        let mut weight_count = 0u128;
        // The flat plans are built per group of consecutive placed blocks,
        // as `m_table_split` groups them (by the same per-block bound).
        let (mut group, mut group_max) = (0u128, 0u128);
        let budget = M_PLAN_GROUP as u128;
        for (a, (arr, cells)) in self.arrays.iter().zip(&blocks).enumerate() {
            let n = 1u128 << arr.rho;
            counts.readers += cells.len() as u128 * 2 * size_of::<Vec<Reader>>() as u128;
            for (k, b) in cells.iter().enumerate() {
                counts.readers += growing::<Reader>(b.readers) + growing::<Reader>(b.all);
                if self.moved[a][k].is_none() {
                    #[cfg(test)]
                    {
                        counts.entry_count += b.entries;
                        counts.coefficient_count += b.coefficients;
                    }
                    plans += 1;
                    let bound =
                        MPlan::<F>::bound(n as usize, b.reads as usize, b.readers as usize) as u128;
                    if group > 0 && group + bound > budget {
                        group_max = group_max.max(group);
                        group = 0;
                    }
                    group += bound;
                    matrix_jobs += (1u128 << self.m(a)).div_ceil(2048);
                    weight_count += b.readers;
                }
            }
        }
        counts.m_plan_group = group_max.max(group);
        counts.m_plans += growing::<MBlock>(plans);
        // Per group: its plan list, jobs, weights and sum-row constants.
        counts.matrix_scratch = growing::<(usize, usize)>(matrix_jobs)
            + plans * size_of::<Vec<F>>() as u128
            + weight_count * field
            + plans * field;
        // Pages of M(r_x, .)'s dense half that the flat plans write.
        let half = 1usize << self.shape.sw();
        let per_page = (M_PAGE / size_of::<F>()).max(1);
        let mut pages = vec![0u64; half.div_ceil(per_page).div_ceil(64)];
        for (a2, arr2) in self.arrays.iter().enumerate() {
            if self.is_io(a2) {
                continue;
            }
            for k2 in (0..arr2.cells()).filter(|k| self.moved[a2][*k].is_none()) {
                let block = &self.blocks[a2][k2];
                for i2 in (0..1usize << self.m(a2)).filter(|i2| self.present(a2, *i2)) {
                    let pos = block.index(i2);
                    if pos < half {
                        let page = pos / per_page;
                        pages[page / 64] |= 1 << (page % 64);
                    }
                }
            }
        }
        counts.m_pages = pages.iter().map(|w| w.count_ones() as u128).sum();
        counts
    }
}
