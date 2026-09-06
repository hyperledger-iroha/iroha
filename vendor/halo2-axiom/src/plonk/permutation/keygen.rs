use ff::{Field, PrimeField};
use group::Curve;

use super::{Argument, ProvingKey, VerifyingKey};
use crate::{
    arithmetic::{CurveAffine, parallelize},
    helpers::release_allocator_slack,
    plonk::{Any, Column, Error},
    poly::{
        EvaluationDomain,
        commitment::{Blind, Params},
    },
};

#[cfg(feature = "multicore")]
use crate::multicore::{IndexedParallelIterator, ParallelIterator};

#[cfg(feature = "thread-safe-region")]
use std::collections::{BTreeSet, HashMap};

#[cfg(not(feature = "thread-safe-region"))]
/// Struct that accumulates all the necessary data in order to construct the permutation argument.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assembly {
    /// Columns that participate on the copy permutation argument.
    columns: Vec<Column<Any>>,
    /// Copy-permutation state, kept implicit until the first nontrivial copy.
    mapping: MappingState,
    /// Number of rows in each permutation column.
    col_len: usize,
}

#[cfg(not(feature = "thread-safe-region"))]
#[derive(Clone, Debug, PartialEq, Eq)]
enum MappingState {
    /// Every cell maps to itself.
    Identity,
    /// Materialized union-find state after at least one nontrivial copy.
    Explicit {
        /// Mapping of the actual copies, encoded as flattened cell identifiers.
        mapping: Vec<usize>,
        /// Distinguished cycle identifier for each flattened cell.
        aux: Vec<usize>,
        /// Cycle size, meaningful only at distinguished cycle identifiers.
        sizes: Vec<usize>,
    },
}

#[cfg(not(feature = "thread-safe-region"))]
struct CompactMapping {
    cells: Option<Vec<usize>>,
    col_len: usize,
}

#[cfg(not(feature = "thread-safe-region"))]
impl CompactMapping {
    #[inline]
    fn get(&self, column: usize, row: usize) -> (usize, usize) {
        let cell = column * self.col_len + row;
        decode_cell_id(
            self.cells.as_ref().map_or(cell, |mapping| mapping[cell]),
            self.col_len,
        )
    }
}

#[cfg(not(feature = "thread-safe-region"))]
#[inline]
fn decode_cell_id(cell: usize, col_len: usize) -> (usize, usize) {
    debug_assert_ne!(col_len, 0);
    (cell / col_len, cell % col_len)
}

#[cfg(not(feature = "thread-safe-region"))]
impl Assembly {
    pub(crate) fn new(n: usize, p: &Argument) -> Self {
        p.columns
            .len()
            .checked_mul(n)
            .expect("permutation assembly exceeds the address space");

        Assembly {
            columns: p.columns.clone(),
            // Before any equality constraints are applied, every cell maps to
            // itself. Keep that identity mapping implicit: circuits without
            // copies do not need the three cell-wide union-find arrays.
            mapping: MappingState::Identity,
            col_len: n,
        }
    }

    pub(crate) fn copy(
        &mut self,
        left_column: Column<Any>,
        left_row: usize,
        right_column: Column<Any>,
        right_row: usize,
    ) -> Result<(), Error> {
        let left_column = self
            .columns
            .iter()
            .position(|c| c == &left_column)
            .ok_or(Error::ColumnNotInPermutation(left_column))?;
        let right_column = self
            .columns
            .iter()
            .position(|c| c == &right_column)
            .ok_or(Error::ColumnNotInPermutation(right_column))?;

        // Check bounds
        if left_row >= self.col_len || right_row >= self.col_len {
            return Err(Error::BoundsFailure);
        }

        // See book/src/design/permutation.md for a description of this algorithm.

        let left_cell = left_column * self.col_len + left_row;
        let right_cell = right_column * self.col_len + right_row;

        // A self-copy cannot change the identity mapping and should not force
        // materialization merely because the constraint was emitted.
        if left_cell == right_cell {
            return Ok(());
        }

        let (mapping, aux, sizes) = self.mapping.materialize(self.columns.len() * self.col_len);
        let mut left_cycle = aux[left_cell];
        let mut right_cycle = aux[right_cell];

        // If left and right are in the same cycle, do nothing.
        if left_cycle == right_cycle {
            return Ok(());
        }

        if sizes[left_cycle] < sizes[right_cycle] {
            std::mem::swap(&mut left_cycle, &mut right_cycle);
        }

        // Merge the right cycle into the left one.
        sizes[left_cycle] += sizes[right_cycle];
        let mut i = right_cycle;
        loop {
            aux[i] = left_cycle;
            i = mapping[i];
            if i == right_cycle {
                break;
            }
        }

        mapping.swap(left_cell, right_cell);

        Ok(())
    }

    pub(crate) fn build_vk<'params, C: CurveAffine, P: Params<'params, C>>(
        self,
        params: &P,
        domain: &EvaluationDomain<C::Scalar>,
        p: &Argument,
    ) -> VerifyingKey<C> {
        let mapping = self.into_mapping();
        build_vk(params, domain, p, |i, j| mapping.get(i, j))
    }

    pub(crate) fn build_pk<'params, C: CurveAffine, P: Params<'params, C>>(
        self,
        params: &P,
        domain: &EvaluationDomain<C::Scalar>,
        p: &Argument,
    ) -> ProvingKey<C> {
        let mapping = self.into_mapping();
        build_pk(params, domain, p, |i, j| mapping.get(i, j))
    }

    pub(crate) fn build_pk_and_vk<'params, C: CurveAffine, P: Params<'params, C>>(
        self,
        params: &P,
        domain: &EvaluationDomain<C::Scalar>,
        p: &Argument,
    ) -> (ProvingKey<C>, VerifyingKey<C>) {
        let mapping = self.into_mapping();

        // Build the compact verifier key first so its temporary permutation
        // polynomials are gone before the proving-key polynomials are retained.
        let vk = build_vk(params, domain, p, |i, j| mapping.get(i, j));
        let pk = build_pk(params, domain, p, |i, j| mapping.get(i, j));
        (pk, vk)
    }

    fn into_mapping(self) -> CompactMapping {
        // Key construction only needs the finalized mapping. Move it through a
        // helper boundary so the union-find scratch (`aux` and `sizes`) is
        // dropped before allocating any permutation polynomials.
        CompactMapping {
            cells: match self.mapping {
                MappingState::Identity => None,
                MappingState::Explicit { mapping, .. } => Some(mapping),
            },
            col_len: self.col_len,
        }
    }

    #[inline]
    fn mapping_at_idx(&self, column: usize, row: usize) -> (usize, usize) {
        let cell = column * self.col_len + row;
        match &self.mapping {
            MappingState::Identity => (column, row),
            MappingState::Explicit { mapping, .. } => decode_cell_id(mapping[cell], self.col_len),
        }
    }

    /// Returns columns that participate in the permutation argument.
    pub fn columns(&self) -> &[Column<Any>] {
        &self.columns
    }

    #[cfg(feature = "multicore")]
    /// Returns mappings of the copies.
    pub fn mapping(
        &self,
    ) -> impl Iterator<Item = impl IndexedParallelIterator<Item = (usize, usize)> + '_> {
        use crate::multicore::IntoParallelIterator;

        (0..self.columns.len()).map(move |column| {
            (0..self.col_len)
                .into_par_iter()
                .map(move |row| self.mapping_at_idx(column, row))
        })
    }

    #[cfg(not(feature = "multicore"))]
    /// Returns mappings of the copies.
    pub fn mapping(&self) -> impl Iterator<Item = impl Iterator<Item = (usize, usize)> + '_> {
        (0..self.columns.len())
            .map(move |column| (0..self.col_len).map(move |row| self.mapping_at_idx(column, row)))
    }
}

#[cfg(not(feature = "thread-safe-region"))]
impl MappingState {
    fn materialize(&mut self, cells: usize) -> (&mut Vec<usize>, &mut Vec<usize>, &mut Vec<usize>) {
        if matches!(self, Self::Identity) {
            let mapping: Vec<_> = (0..cells).collect();
            *self = Self::Explicit {
                aux: mapping.clone(),
                mapping,
                sizes: vec![1; cells],
            };
        }
        match self {
            Self::Explicit {
                mapping,
                aux,
                sizes,
            } => (mapping, aux, sizes),
            Self::Identity => unreachable!("mapping was materialized above"),
        }
    }
}

#[cfg(feature = "thread-safe-region")]
/// Struct that accumulates all the necessary data in order to construct the permutation argument.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assembly {
    /// Columns that participate on the copy permutation argument.
    columns: Vec<Column<Any>>,
    /// Mapping of the actual copies done.
    cycles: Vec<Vec<(usize, usize)>>,
    /// Mapping of the actual copies done.
    ordered_cycles: Vec<BTreeSet<(usize, usize)>>,
    /// Mapping of the actual copies done.
    aux: HashMap<(usize, usize), usize>,
    /// total length of a column
    col_len: usize,
    /// number of columns
    num_cols: usize,
}

#[cfg(feature = "thread-safe-region")]
impl Assembly {
    pub(crate) fn new(n: usize, p: &Argument) -> Self {
        Assembly {
            columns: p.columns.clone(),
            cycles: Vec::with_capacity(n),
            ordered_cycles: Vec::with_capacity(n),
            aux: HashMap::new(),
            col_len: n,
            num_cols: p.columns.len(),
        }
    }

    pub(crate) fn copy(
        &mut self,
        left_column: Column<Any>,
        left_row: usize,
        right_column: Column<Any>,
        right_row: usize,
    ) -> Result<(), Error> {
        let left_column = self
            .columns
            .iter()
            .position(|c| c == &left_column)
            .ok_or(Error::ColumnNotInPermutation(left_column))?;
        let right_column = self
            .columns
            .iter()
            .position(|c| c == &right_column)
            .ok_or(Error::ColumnNotInPermutation(right_column))?;

        // Check bounds
        if left_row >= self.col_len || right_row >= self.col_len {
            return Err(Error::BoundsFailure);
        }

        let left_cycle = self.aux.get(&(left_column, left_row));
        let right_cycle = self.aux.get(&(right_column, right_row));

        // extract cycle elements
        let right_cycle_elems = match right_cycle {
            Some(i) => {
                let entry = self.cycles[*i].clone();
                self.cycles[*i] = vec![];
                entry
            }
            None => [(right_column, right_row)].into(),
        };

        assert!(right_cycle_elems.contains(&(right_column, right_row)));

        // merge cycles
        let cycle_idx = match left_cycle {
            Some(i) => {
                let entry = &mut self.cycles[*i];
                entry.extend(right_cycle_elems.clone());
                *i
            }
            // if they were singletons -- create a new cycle entry
            None => {
                let mut set: Vec<(usize, usize)> = right_cycle_elems.clone();
                set.push((left_column, left_row));
                self.cycles.push(set);
                let cycle_idx = self.cycles.len() - 1;
                self.aux.insert((left_column, left_row), cycle_idx);
                cycle_idx
            }
        };

        let index_updates = vec![cycle_idx; right_cycle_elems.len()].into_iter();
        let updates = right_cycle_elems.into_iter().zip(index_updates);

        self.aux.extend(updates);

        Ok(())
    }

    /// Builds the ordered mapping of the cycles.
    /// This will only get executed once.
    pub fn build_ordered_mapping(&mut self) {
        use crate::multicore::IntoParallelRefMutIterator;

        // will only get called once
        if self.ordered_cycles.is_empty() && !self.cycles.is_empty() {
            self.ordered_cycles = self
                .cycles
                .par_iter_mut()
                .map(|col| {
                    let mut set = BTreeSet::new();
                    set.extend(col.clone());
                    // free up memory
                    *col = vec![];
                    set
                })
                .collect();
        }
    }

    fn mapping_at_idx(&self, col: usize, row: usize) -> (usize, usize) {
        assert!(
            !self.ordered_cycles.is_empty() || self.cycles.is_empty(),
            "cycles have not been ordered"
        );

        if let Some(cycle_idx) = self.aux.get(&(col, row)) {
            let cycle = &self.ordered_cycles[*cycle_idx];
            let mut cycle_iter = cycle.range((
                std::ops::Bound::Excluded((col, row)),
                std::ops::Bound::Unbounded,
            ));
            // point to the next node in the cycle
            match cycle_iter.next() {
                Some((i, j)) => (*i, *j),
                // wrap back around to the first element which SHOULD exist
                None => *(cycle.iter().next().unwrap()),
            }
        // is a singleton
        } else {
            (col, row)
        }
    }

    pub(crate) fn build_vk<'params, C: CurveAffine, P: Params<'params, C>>(
        &mut self,
        params: &P,
        domain: &EvaluationDomain<C::Scalar>,
        p: &Argument,
    ) -> VerifyingKey<C> {
        self.build_ordered_mapping();
        build_vk(params, domain, p, |i, j| self.mapping_at_idx(i, j))
    }

    pub(crate) fn build_pk<'params, C: CurveAffine, P: Params<'params, C>>(
        &mut self,
        params: &P,
        domain: &EvaluationDomain<C::Scalar>,
        p: &Argument,
    ) -> ProvingKey<C> {
        self.build_ordered_mapping();
        build_pk(params, domain, p, |i, j| self.mapping_at_idx(i, j))
    }

    /// Returns columns that participate in the permutation argument.
    pub fn columns(&self) -> &[Column<Any>] {
        &self.columns
    }

    #[cfg(feature = "multicore")]
    /// Returns mappings of the copies.
    pub fn mapping(
        &self,
    ) -> impl Iterator<Item = impl IndexedParallelIterator<Item = (usize, usize)> + '_> {
        use crate::multicore::IntoParallelIterator;

        (0..self.num_cols).map(move |i| {
            (0..self.col_len)
                .into_par_iter()
                .map(move |j| self.mapping_at_idx(i, j))
        })
    }

    #[cfg(not(feature = "multicore"))]
    /// Returns mappings of the copies.
    pub fn mapping(&self) -> impl Iterator<Item = impl Iterator<Item = (usize, usize)> + '_> {
        (0..self.num_cols).map(move |i| (0..self.col_len).map(move |j| self.mapping_at_idx(i, j)))
    }
}

pub(crate) fn build_pk<'params, C: CurveAffine, P: Params<'params, C>>(
    params: &P,
    domain: &EvaluationDomain<C::Scalar>,
    p: &Argument,
    mapping: impl Fn(usize, usize) -> (usize, usize) + Sync,
) -> ProvingKey<C> {
    // Compute [omega^0, omega^1, ..., omega^{params.n - 1}]
    let mut omega_powers = vec![C::Scalar::ZERO; params.n() as usize];
    {
        let omega = domain.get_omega();
        parallelize(&mut omega_powers, |o, start| {
            let mut cur = omega.pow_vartime([start as u64]);
            for v in o.iter_mut() {
                *v = cur;
                cur *= &omega;
            }
        })
    }

    // Compute [delta^0, delta^1, ..., delta^(m - 1)]. Keeping the omega and
    // delta factors separate avoids materializing an n-by-m delta-omega grid.
    let mut delta_powers = Vec::with_capacity(p.columns.len());
    let mut delta = C::Scalar::ONE;
    for _ in 0..p.columns.len() {
        delta_powers.push(delta);
        delta *= &C::Scalar::DELTA;
    }

    // Compute permutation polynomials, convert to coset form.
    let mut permutations = vec![domain.empty_lagrange(); p.columns.len()];
    {
        parallelize(&mut permutations, |o, start| {
            for (x, permutation_poly) in o.iter_mut().enumerate() {
                let i = start + x;
                for (j, p) in permutation_poly.iter_mut().enumerate() {
                    let (permuted_i, permuted_j) = mapping(i, j);
                    *p = omega_powers[permuted_j] * delta_powers[permuted_i];
                }
            }
        });
    }
    drop(omega_powers);
    drop(delta_powers);

    // Each inverse FFT is internally parallel. Convert columns serially so at
    // most one cloned input polynomial is live while retaining all outputs.
    let polys = permutations
        .iter()
        .cloned()
        .map(|poly| domain.lagrange_to_coeff(poly))
        .collect();

    ProvingKey {
        permutations,
        polys,
    }
}

fn fill_permutation_column<F: Field>(
    values: &mut [F],
    column: usize,
    omega_powers: &[F],
    delta_powers: &[F],
    mapping: &(impl Fn(usize, usize) -> (usize, usize) + Sync),
) {
    parallelize(values, |values, start| {
        for (offset, value) in values.iter_mut().enumerate() {
            let row = start + offset;
            let (permuted_column, permuted_row) = mapping(column, row);
            *value = omega_powers[permuted_row] * delta_powers[permuted_column];
        }
    });
}

pub(crate) fn build_vk<'params, C: CurveAffine, P: Params<'params, C>>(
    params: &P,
    domain: &EvaluationDomain<C::Scalar>,
    p: &Argument,
    mapping: impl Fn(usize, usize) -> (usize, usize) + Sync,
) -> VerifyingKey<C> {
    // Compute [omega^0, omega^1, ..., omega^{params.n - 1}]
    let mut omega_powers = vec![C::Scalar::ZERO; params.n() as usize];
    {
        let omega = domain.get_omega();
        parallelize(&mut omega_powers, |o, start| {
            let mut cur = omega.pow_vartime([start as u64]);
            for v in o.iter_mut() {
                *v = cur;
                cur *= &omega;
            }
        })
    }

    // Compute [delta^0, delta^1, ..., delta^(m - 1)]. Keeping the omega and
    // delta factors separate avoids materializing an n-by-m delta-omega grid.
    let mut delta_powers = Vec::with_capacity(p.columns.len());
    let mut delta = C::Scalar::ONE;
    for _ in 0..p.columns.len() {
        delta_powers.push(delta);
        delta *= &C::Scalar::DELTA;
    }

    // Build and commit one permutation polynomial at a time. The verifier key
    // retains only commitments, so retaining every n-element permutation
    // column until the first MSM needlessly retains `(columns - 1) * n` extra
    // permutation field elements. The streamed path keeps the n-element omega
    // table through every column while avoiding the otherwise duplicated
    // full-column residency in large recursive provers.
    let mut commitments = Vec::with_capacity(p.columns.len());
    for column in 0..p.columns.len() {
        let mut permutation = domain.empty_lagrange();
        fill_permutation_column(
            &mut permutation,
            column,
            &omega_powers,
            &delta_powers,
            &mapping,
        );
        commitments.push(
            params
                .commit_lagrange(&permutation, Blind::default())
                .to_affine(),
        );
        release_allocator_slack();
    }

    VerifyingKey { commitments }
}

#[cfg(all(test, not(feature = "thread-safe-region")))]
mod tests {
    use super::*;

    struct ReferenceAssembly {
        mapping: Vec<Vec<(usize, usize)>>,
        aux: Vec<Vec<(usize, usize)>>,
        sizes: Vec<Vec<usize>>,
    }

    impl ReferenceAssembly {
        fn new(columns: usize, rows: usize) -> Self {
            let mapping: Vec<Vec<_>> = (0..columns)
                .map(|column| (0..rows).map(|row| (column, row)).collect())
                .collect();
            Self {
                aux: mapping.clone(),
                mapping,
                sizes: vec![vec![1; rows]; columns],
            }
        }

        fn copy(
            &mut self,
            left_column: usize,
            left_row: usize,
            right_column: usize,
            right_row: usize,
        ) {
            let mut left_cycle = self.aux[left_column][left_row];
            let mut right_cycle = self.aux[right_column][right_row];
            if left_cycle == right_cycle {
                return;
            }

            if self.sizes[left_cycle.0][left_cycle.1] < self.sizes[right_cycle.0][right_cycle.1] {
                std::mem::swap(&mut left_cycle, &mut right_cycle);
            }

            self.sizes[left_cycle.0][left_cycle.1] += self.sizes[right_cycle.0][right_cycle.1];
            let mut current = right_cycle;
            loop {
                self.aux[current.0][current.1] = left_cycle;
                current = self.mapping[current.0][current.1];
                if current == right_cycle {
                    break;
                }
            }

            let tmp = self.mapping[left_column][left_row];
            self.mapping[left_column][left_row] = self.mapping[right_column][right_row];
            self.mapping[right_column][right_row] = tmp;
        }
    }

    fn argument(column_count: usize) -> (Argument, Vec<Column<Any>>) {
        let mut argument = Argument::new();
        let columns: Vec<_> = (0..column_count)
            .map(|index| Column::new(index, Any::advice()))
            .collect();
        for column in &columns {
            argument.add_column(*column);
        }
        (argument, columns)
    }

    fn assert_matches_reference(actual: &Assembly, expected: &ReferenceAssembly) {
        assert_eq!(actual.columns.len(), expected.mapping.len());
        for column in 0..actual.columns.len() {
            assert_eq!(actual.col_len, expected.mapping[column].len());
            for row in 0..actual.col_len {
                assert_eq!(
                    actual.mapping_at_idx(column, row),
                    expected.mapping[column][row]
                );
            }
        }

        match &actual.mapping {
            MappingState::Identity => {
                for column in 0..actual.columns.len() {
                    for row in 0..actual.col_len {
                        assert_eq!(expected.aux[column][row], (column, row));
                        assert_eq!(expected.sizes[column][row], 1);
                    }
                }
            }
            MappingState::Explicit { aux, sizes, .. } => {
                for column in 0..actual.columns.len() {
                    for row in 0..actual.col_len {
                        let cell = column * actual.col_len + row;
                        assert_eq!(
                            decode_cell_id(aux[cell], actual.col_len),
                            expected.aux[column][row]
                        );
                        assert_eq!(sizes[cell], expected.sizes[column][row]);
                    }
                }
            }
        }
    }

    #[test]
    fn identity_mapping_stays_lazy_without_nontrivial_copies() {
        let (argument, columns) = argument(4);
        let mut assembly = Assembly::new(13, &argument);

        assert!(matches!(&assembly.mapping, MappingState::Identity));
        for column in 0..4 {
            for row in 0..13 {
                assert_eq!(assembly.mapping_at_idx(column, row), (column, row));
            }
        }
        assert_eq!(
            assembly
                .mapping()
                .map(|column| column.collect::<Vec<_>>())
                .collect::<Vec<_>>(),
            (0..4)
                .map(|column| (0..13).map(|row| (column, row)).collect())
                .collect::<Vec<Vec<_>>>()
        );

        // A no-op constraint and rejected constraints leave the implicit
        // identity representation untouched.
        assembly.copy(columns[2], 7, columns[2], 7).unwrap();
        assert!(matches!(&assembly.mapping, MappingState::Identity));
        assert!(matches!(
            assembly.copy(columns[0], 13, columns[1], 0),
            Err(Error::BoundsFailure)
        ));
        assert!(matches!(&assembly.mapping, MappingState::Identity));

        let compact = assembly.into_mapping();
        assert!(compact.cells.is_none());
        assert_eq!(compact.get(0, 0), (0, 0));
        assert_eq!(compact.get(3, 12), (3, 12));

        let empty = Assembly::new(0, &argument);
        assert!(matches!(&empty.mapping, MappingState::Identity));
        assert_eq!(empty.mapping().count(), 4);
    }

    #[test]
    fn permutation_column_stream_preserves_mapped_values() {
        use halo2curves::pasta::Fp;

        let omega_powers = [Fp::from(1), Fp::from(2), Fp::from(4), Fp::from(8)];
        let delta_powers = [Fp::from(1), Fp::from(3), Fp::from(9)];
        let mapping = |column: usize, row: usize| {
            (
                (column + row) % delta_powers.len(),
                (row + 1) % omega_powers.len(),
            )
        };
        let mut actual = vec![Fp::ZERO; omega_powers.len()];

        fill_permutation_column(&mut actual, 1, &omega_powers, &delta_powers, &mapping);

        let expected = (0..omega_powers.len())
            .map(|row| {
                let (permuted_column, permuted_row) = mapping(1, row);
                omega_powers[permuted_row] * delta_powers[permuted_column]
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }

    #[test]
    fn streamed_vk_commitments_match_proving_key_permutations() {
        use crate::poly::{
            commitment::{Params, ParamsProver},
            ipa::commitment::ParamsIPA,
        };
        use halo2curves::pasta::{EqAffine, Fp};

        const K: u32 = 3;
        const COLUMNS: usize = 3;
        const ROWS: usize = 1 << K;

        let params = ParamsIPA::<EqAffine>::new(K);
        let domain = EvaluationDomain::<Fp>::new(3, K);
        let (argument, _) = argument(COLUMNS);
        let mapping = |column: usize, row: usize| ((column + 1) % COLUMNS, (row + 1) % ROWS);

        let proving_key = build_pk(&params, &domain, &argument, &mapping);
        let streamed_vk = build_vk(&params, &domain, &argument, &mapping);
        let recomputed_commitments = proving_key
            .permutations
            .iter()
            .map(|permutation| {
                params
                    .commit_lagrange(permutation, Blind::default())
                    .to_affine()
            })
            .collect::<Vec<_>>();

        assert_eq!(
            streamed_vk.commitments().as_slice(),
            recomputed_commitments.as_slice()
        );
    }

    #[test]
    fn flattened_union_find_matches_coordinate_reference() {
        const COLUMNS: usize = 5;
        const ROWS: usize = 17;
        let (argument, columns) = argument(COLUMNS);
        let mut actual = Assembly::new(ROWS, &argument);
        let mut expected = ReferenceAssembly::new(COLUMNS, ROWS);

        // Exercise singleton insertion, equal-size ties, union-by-size swaps,
        // cycle splicing, and redundant copies using a deterministic sequence.
        let operations = [
            (0, 0, 1, 0),
            (0, 1, 1, 1),
            (2, 5, 3, 7),
            (0, 0, 0, 1),
            (2, 5, 0, 0),
            (4, 16, 2, 5),
            (1, 0, 3, 7),
            (4, 16, 4, 16),
        ];
        for (left_column, left_row, right_column, right_row) in operations {
            actual
                .copy(
                    columns[left_column],
                    left_row,
                    columns[right_column],
                    right_row,
                )
                .unwrap();
            expected.copy(left_column, left_row, right_column, right_row);
            assert_matches_reference(&actual, &expected);
        }
        assert!(matches!(&actual.mapping, MappingState::Explicit { .. }));

        let mut state = 0xd1b5_4a32_d192_ed03_u64;
        for _ in 0..1_000 {
            let mut next = || {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                state
            };
            let left_column = next() as usize % COLUMNS;
            let left_row = next() as usize % ROWS;
            let right_column = next() as usize % COLUMNS;
            let right_row = next() as usize % ROWS;
            actual
                .copy(
                    columns[left_column],
                    left_row,
                    columns[right_column],
                    right_row,
                )
                .unwrap();
            expected.copy(left_column, left_row, right_column, right_row);
            assert_matches_reference(&actual, &expected);
        }

        let before = actual.clone();
        assert!(matches!(
            actual.copy(columns[0], ROWS, columns[1], 0),
            Err(Error::BoundsFailure)
        ));
        assert_eq!(actual, before);
    }
}
