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
    edges: u128,
    entries: u128,
    coefficients: u128,
}

#[derive(Default)]
pub(super) struct PlanCounts {
    pub symbolic: u128,
    pub symbolic_build_scratch: u128,
    pub gather: u128,
    pub gather_scratch: u128,
    pub readers: u128,
    pub readers_build_scratch: u128,
    pub m_plans: u128,
    pub matrix_scratch: u128,
    pub helpers: u128,
    #[cfg(test)]
    pub edge_count: u128,
    #[cfg(test)]
    pub entry_count: u128,
    #[cfg(test)]
    pub coefficient_count: u128,
}

enum Replacement {
    Lookup(usize, usize),
    Table(usize),
    Export(usize),
    Challenge(usize),
}

impl<F: Scalar> UniformCircuit<F> {
    pub(super) fn memory_plan_counts(&self) -> PlanCounts {
        let field = size_of::<F>() as u128;
        let workers = rayon::current_num_threads() as u128;
        let mut counts = PlanCounts::default();
        let mut blocks: Vec<Vec<BlockCounts>> = self
            .arrays
            .iter()
            .map(|a| (0..a.cells()).map(|_| BlockCounts::default()).collect())
            .collect();
        let mut one_readers = 0u128;
        let mut jobs = 0u128;
        let mut gather_factors = 0u128;
        let mut max_lin = 0u128;
        let mut symbolic_original = 0u128;
        let mut max_replacement = 0u128;
        let mut replacements = HashMap::new();
        if self.symbolic.get().is_none() {
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
        }
        counts.symbolic +=
            self.arrays.len() as u128 * size_of::<Vec<[Vec<(Term<F>, Factor)>; 3]>>() as u128;
        for (a, arr) in self.arrays.iter().enumerate() {
            let n = 1usize << arr.rho;
            let present = (0..n).filter(|r| self.r_present(a, *r)).count() as u128;
            counts.symbolic +=
                arr.rows.len() as u128 * size_of::<[Vec<(Term<F>, Factor)>; 3]>() as u128;
            for (j, original) in arr.rows.iter().enumerate() {
                let row: [Vec<TermRef<'_, F>>; 3] = if let Some(symbolic) = self.symbolic.get() {
                    std::array::from_fn(|mat| {
                        symbolic[a][j][mat]
                            .iter()
                            .map(|(t, _)| TermRef::borrowed(t))
                            .collect()
                    })
                } else {
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
                let mut row_storage = 0u128;
                for lin in &row {
                    let len = lin.len() as u128;
                    jobs += 1;
                    gather_factors += len;
                    max_lin = max_lin.max(len);
                    counts.gather += len
                        * (size_of::<Option<GatherTarget>>() + size_of::<Factor>()) as u128
                        + growing::<usize>(present);
                    row_storage += growing::<(Term<F>, Factor)>(len);
                    let mut sums = 0u128;
                    for &term in lin {
                        row_storage += term.len() as u128 * field;
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
                            Source::Link(a2, k2, map) => {
                                row_storage +=
                                    map.len() as u128 * size_of::<Option<usize>>() as u128;
                                Some((a2, k2))
                            }
                            Source::Identity(a2, k2, n) => {
                                row_storage += n as u128 * size_of::<Option<usize>>() as u128;
                                Some((a2, k2))
                            }
                        };
                        if let Some((a2, k2)) = target {
                            let block = &mut blocks[a2][k2];
                            block.readers += 1;
                            for r in 0..term.len() {
                                if matches!(term.source, Source::Link(_, _, map) if map[r].is_none())
                                {
                                    continue;
                                }
                                block.edges += 1;
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
                    counts.gather += growing::<usize>(sums);
                }
                counts.symbolic += row_storage;
                // symbolic_rows first clones originals, then replaces helper
                // rows. Only one replacement row is being constructed at once.
                if replacements.contains_key(&(a, j)) {
                    max_replacement = max_replacement.max(row_storage);
                    symbolic_original += original
                        .iter()
                        .map(|lin| {
                            lin.len() as u128 * size_of::<(Term<F>, Factor)>() as u128
                                + lin
                                    .iter()
                                    .map(|t| {
                                        t.coef.len() as u128 * field
                                            + match &t.src {
                                                Src::Link { map, .. } => {
                                                    map.len() as u128
                                                        * size_of::<Option<usize>>() as u128
                                                }
                                                _ => 0,
                                            }
                                    })
                                    .sum::<u128>()
                        })
                        .sum::<u128>();
                }
            }
        }
        counts.symbolic_build_scratch = symbolic_original + max_replacement;
        counts.gather += growing::<GatherJob>(jobs);
        // Nested Rayon work may keep one factor array for each suspended outer
        // gather job. Bases are per active worker and bounded by the largest row.
        counts.gather_scratch = gather_factors * size_of::<Option<F>>() as u128
            + workers * max_lin * size_of::<usize>() as u128;
        counts.readers += self.arrays.len() as u128
            * (size_of::<Vec<Vec<Reader>>>()
                + size_of::<Vec<Vec<(usize, usize, usize, usize)>>>()
                + size_of::<Vec<(Vec<u32>, Vec<(u32, u32)>)>>()) as u128
            + growing::<(usize, usize, usize, usize)>(one_readers);
        let mut plans = 0u128;
        let mut edge_scratch = 0u128;
        let mut max_csr_scratch = 0u128;
        let mut matrix_jobs = 0u128;
        let mut weight_count = 0u128;
        for (a, (arr, cells)) in self.arrays.iter().zip(&blocks).enumerate() {
            let n = 1u128 << arr.rho;
            counts.readers += cells.len() as u128
                * (size_of::<Vec<Reader>>()
                    + size_of::<Vec<(usize, usize, usize, usize)>>()
                    + size_of::<(Vec<u32>, Vec<(u32, u32)>)>()) as u128;
            for (k, b) in cells.iter().enumerate() {
                #[cfg(test)]
                {
                    counts.edge_count += b.edges;
                }
                counts.readers += growing::<Reader>(b.readers)
                    + growing::<(usize, usize, usize, usize)>(b.all)
                    + (n + 1) * size_of::<u32>() as u128
                    + b.edges * size_of::<(u32, u32)>() as u128;
                edge_scratch +=
                    growing::<(u32, u32, u32)>(b.edges) + size_of::<Vec<(u32, u32, u32)>>() as u128;
                max_csr_scratch = max_csr_scratch.max(n * size_of::<u32>() as u128);
                if self.moved[a][k].is_none() {
                    #[cfg(test)]
                    {
                        counts.entry_count += b.entries;
                        counts.coefficient_count += b.coefficients;
                    }
                    plans += 1;
                    counts.m_plans += (n + 1) * size_of::<u32>() as u128
                        + b.readers * size_of::<MReader>() as u128
                        + growing::<(u32, u8, u32, u32)>(b.entries)
                        + growing::<F>(b.coefficients);
                    matrix_jobs += (1u128 << self.m(a)).div_ceil(2048);
                    weight_count += b.readers;
                }
            }
        }
        counts.readers_build_scratch = edge_scratch
            + max_csr_scratch
            + self.arrays.len() as u128 * size_of::<Vec<Vec<(u32, u32, u32)>>>() as u128;
        counts.m_plans += growing::<MPlan<F>>(plans);
        counts.matrix_scratch = growing::<(usize, usize)>(matrix_jobs)
            + plans * size_of::<Vec<F>>() as u128
            + weight_count * field;
        let helper_slots = self
            .lookup_arrays
            .iter()
            .map(|(a, _, _)| self.instances(*a) as u128 * self.arrays[*a].lookups.len() as u128)
            .sum::<u128>()
            + self
                .tables
                .iter()
                .map(|(va, _, _)| self.instances(*va) as u128)
                .sum::<u128>()
            + self
                .exports
                .iter()
                .map(|(_, wa, _, _)| self.instances(*wa) as u128)
                .sum::<u128>();
        counts.helpers = growing::<(usize, F, F)>(helper_slots)
            + helper_slots * field
            + helper_slots.min(workers * 4096) * field;
        counts
    }
}
