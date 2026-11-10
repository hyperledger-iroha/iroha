//! Exact selected-query evaluation of compiled profiles' public fixed polynomials.
//!
//! The values are the same degree-<N interpolants used by the prover's IFFT/LDE.
//! All inputs come from the compiled adapter; proof bytes supply neither these
//! polynomials nor their evaluations. The verifier uses one path for every profile.
//! The same interpolants are evaluated at the transcript-derived out-of-domain
//! point, where the verifier checks the complete AIR.

use super::{
    E, F, ProofManagedNoteStarkErrorV1, checked_trace_size_v1, map_aggregate_error_v1,
    map_transparent_error_v1,
};
use crate::privacy_engines::{
    aggregate_stark::batch_invert_fp4_nonzero_v1,
    transparent_stark::{
        GOLDILOCKS_GENERATOR_V1, goldilocks_batch_invert_v1, goldilocks_primitive_root_v1,
    },
};
use rayon::prelude::*;
use std::collections::BTreeMap;

/// At most four queries retain weights and inversion-prefix scratch, regardless of Rayon width.
const QUERY_BATCH: usize = 4;

enum Column {
    Constant(F),
    Dense { offset: F, deltas: Vec<F> },
    Sparse { offset: F, deltas: Vec<(usize, F)> },
    Shift { source: usize, offset: F },
}
impl Column {
    fn delta_at(&self, row: usize) -> F {
        match self {
            Self::Dense { deltas, .. } => deltas[row],
            Self::Sparse { deltas, .. } => deltas
                .binary_search_by_key(&row, |(index, _)| *index)
                .map_or(F::ZERO, |index| deltas[index].1),
            Self::Constant(_) | Self::Shift { .. } => {
                unreachable!("only unique nonconstant columns are indexed")
            }
        }
    }
    fn offset(&self) -> F {
        match self {
            Self::Dense { offset, .. } | Self::Sparse { offset, .. } => *offset,
            Self::Constant(_) | Self::Shift { .. } => {
                unreachable!("only unique nonconstant columns are indexed")
            }
        }
    }
}

struct Oracle {
    columns: Vec<Column>,
    roots: Vec<F>,
    inverse_size: F,
}
impl Oracle {
    fn new(columns: Vec<Vec<F>>, trace_log2: u8) -> Result<Self, ProofManagedNoteStarkErrorV1> {
        let size = checked_trace_size_v1(trace_log2)?;
        if columns.is_empty()
            || columns.iter().any(|column| {
                column.len() != size || column.iter().any(|value| F::canonical(value.0).is_none())
            })
        {
            return Err(ProofManagedNoteStarkErrorV1::InvalidProfile);
        }
        let root = goldilocks_primitive_root_v1(trace_log2).map_err(map_transparent_error_v1)?;
        let mut roots = Vec::new();
        roots
            .try_reserve_exact(size)
            .map_err(|_| ProofManagedNoteStarkErrorV1::Resource)?;
        let mut power = F::ONE;
        for _ in 0..size {
            roots.push(power);
            power = power.mul(root);
        }
        let mut packed = Vec::<Column>::new();
        packed
            .try_reserve_exact(columns.len())
            .map_err(|_| ProofManagedNoteStarkErrorV1::Resource)?;
        let mut fingerprints = BTreeMap::<u64, Vec<usize>>::new();
        for mut column in columns {
            let offset = column[0];
            let mut nonzero = 0;
            let mut fingerprint = 0xcbf2_9ce4_8422_2325_u64;
            for value in &mut column {
                *value = value.sub(offset);
                nonzero += usize::from(*value != F::ZERO);
                fingerprint = (fingerprint ^ value.0).wrapping_mul(0x0000_0100_0000_01b3);
            }
            if nonzero == 0 {
                packed.push(Column::Constant(offset));
                continue;
            }
            // Fingerprints only select candidates. Exact equality is mandatory,
            // so a collision cannot change a polynomial or its evaluation.
            let source = fingerprints.get(&fingerprint).and_then(|candidates| {
                candidates.iter().copied().find(|index| {
                    column
                        .iter()
                        .enumerate()
                        .all(|(row, value)| *value == packed[*index].delta_at(row))
                })
            });
            if let Some(source) = source {
                packed.push(Column::Shift {
                    source,
                    offset: offset.sub(packed[source].offset()),
                });
                continue;
            }
            fingerprints
                .entry(fingerprint)
                .or_default()
                .push(packed.len());
            if nonzero <= size / 4 {
                let mut deltas = Vec::new();
                deltas
                    .try_reserve_exact(nonzero)
                    .map_err(|_| ProofManagedNoteStarkErrorV1::Resource)?;
                deltas.extend(
                    column
                        .into_iter()
                        .enumerate()
                        .filter(|(_, value)| *value != F::ZERO),
                );
                packed.push(Column::Sparse { offset, deltas });
            } else {
                packed.push(Column::Dense {
                    offset,
                    deltas: column,
                });
            }
        }
        Ok(Self {
            columns: packed,
            roots,
            inverse_size: F(size as u64)
                .inv()
                .ok_or(ProofManagedNoteStarkErrorV1::InvalidProfile)?,
        })
    }

    fn evaluate(&self, x: F) -> Result<Vec<F>, ProofManagedNoteStarkErrorV1> {
        let size = self.roots.len();
        let factor = x.pow(size as u128).sub(F::ONE).mul(self.inverse_size);
        if factor == F::ZERO {
            return Err(ProofManagedNoteStarkErrorV1::InvalidProfile);
        }
        let mut weights = Vec::new();
        weights
            .try_reserve_exact(size)
            .map_err(|_| ProofManagedNoteStarkErrorV1::Resource)?;
        weights.extend(self.roots.iter().map(|root| x.sub(*root)));
        goldilocks_batch_invert_v1(&mut weights).map_err(map_transparent_error_v1)?;
        // L_j(x) = (x^N-1) * omega^j / (N * (x-omega^j)).
        for (weight, root) in weights.iter_mut().zip(&self.roots) {
            *weight = weight.mul(*root).mul(factor);
        }
        let mut result = Vec::<F>::new();
        result
            .try_reserve_exact(self.columns.len())
            .map_err(|_| ProofManagedNoteStarkErrorV1::Resource)?;
        for column in &self.columns {
            result.push(match column {
                Column::Constant(value) => *value,
                Column::Shift { source, offset } => result[*source].add(*offset),
                Column::Dense { offset, deltas } => {
                    deltas
                        .iter()
                        .zip(&weights)
                        .fold(*offset, |sum, (delta, weight)| {
                            if *delta == F::ZERO {
                                sum
                            } else {
                                sum.add(delta.mul(*weight))
                            }
                        })
                }
                Column::Sparse { offset, deltas } => {
                    deltas.iter().fold(*offset, |sum, (index, delta)| {
                        sum.add(delta.mul(weights[*index]))
                    })
                }
            });
        }
        Ok(result)
    }

    /// Evaluate every fixed interpolant at one extension-field point.
    ///
    /// These are the barycentric weights of [`Self::evaluate`] lifted to Fp4. A
    /// point in the native subgroup has no finite weight and is rejected; the
    /// transcript never derives one as an out-of-domain point.
    fn evaluate_extension(&self, point: E) -> Result<Vec<E>, ProofManagedNoteStarkErrorV1> {
        let size = self.roots.len();
        if !point.is_canonical() {
            return Err(ProofManagedNoteStarkErrorV1::InvalidProfile);
        }
        let factor = point
            .pow(size as u128)
            .sub(E::ONE)
            .mul_base(self.inverse_size);
        if factor == E::ZERO {
            return Err(ProofManagedNoteStarkErrorV1::InvalidProfile);
        }
        let mut weights = Vec::<E>::new();
        weights
            .try_reserve_exact(size)
            .map_err(|_| ProofManagedNoteStarkErrorV1::Resource)?;
        weights.extend(self.roots.iter().map(|root| point.sub(E::from_base(*root))));
        batch_invert_fp4_nonzero_v1(&mut weights).map_err(map_aggregate_error_v1)?;
        // L_j(z) = (z^N-1) * omega^j / (N * (z-omega^j)).
        for (weight, root) in weights.iter_mut().zip(&self.roots) {
            *weight = weight.mul_base(*root).mul(factor);
        }
        let mut result = Vec::<E>::new();
        result
            .try_reserve_exact(self.columns.len())
            .map_err(|_| ProofManagedNoteStarkErrorV1::Resource)?;
        for column in &self.columns {
            result.push(match column {
                Column::Constant(value) => E::from_base(*value),
                Column::Shift { source, offset } => result[*source].add(E::from_base(*offset)),
                Column::Dense { offset, deltas } => deltas.iter().zip(&weights).fold(
                    E::from_base(*offset),
                    |sum, (delta, weight)| {
                        if *delta == F::ZERO {
                            sum
                        } else {
                            sum.add(weight.mul_base(*delta))
                        }
                    },
                ),
                Column::Sparse { offset, deltas } => deltas
                    .iter()
                    .fold(E::from_base(*offset), |sum, (index, delta)| {
                        sum.add(weights[*index].mul_base(*delta))
                    }),
            });
        }
        Ok(result)
    }

    /// Evaluate precisely the authenticated query indices on the shifted LDE coset.
    fn query_rows(
        &self,
        lde_log2: u8,
        query_indices: &[usize],
    ) -> Result<BTreeMap<usize, Vec<F>>, ProofManagedNoteStarkErrorV1> {
        let lde_root = goldilocks_primitive_root_v1(lde_log2).map_err(map_transparent_error_v1)?;
        let mut rows = BTreeMap::new();
        for batch in query_indices.chunks(QUERY_BATCH) {
            let results = batch
                .par_iter()
                .map(|index| {
                    let x = F(GOLDILOCKS_GENERATOR_V1).mul(lde_root.pow(*index as u128));
                    self.evaluate(x)
                })
                .collect::<Vec<_>>();
            for (index, row) in batch.iter().copied().zip(results) {
                rows.insert(index, row?);
            }
        }
        Ok(rows)
    }
}

/// Reject a query set that is empty, oversized, out of range or repeated.
fn validate_query_geometry_v1(
    trace_log2: u8,
    lde_log2: u8,
    query_indices: &[usize],
) -> Result<(), ProofManagedNoteStarkErrorV1> {
    let lde_size = checked_trace_size_v1(lde_log2)?;
    if lde_log2 <= trace_log2
        || query_indices.is_empty()
        || query_indices.len() > 136
        || query_indices.iter().any(|index| *index >= lde_size)
    {
        return Err(ProofManagedNoteStarkErrorV1::InvalidProfile);
    }
    let mut sorted = query_indices.to_vec();
    sorted.sort_unstable();
    if sorted.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ProofManagedNoteStarkErrorV1::InvalidProfile);
    }
    Ok(())
}

/// Evaluate precisely the authenticated query indices; no LDE-sized matrix is allocated.
#[cfg(test)]
pub(super) fn query_rows_v1(
    columns: Vec<Vec<F>>,
    trace_log2: u8,
    lde_log2: u8,
    query_indices: &[usize],
) -> Result<BTreeMap<usize, Vec<F>>, ProofManagedNoteStarkErrorV1> {
    validate_query_geometry_v1(trace_log2, lde_log2, query_indices)?;
    Oracle::new(columns, trace_log2)?.query_rows(lde_log2, query_indices)
}

/// Evaluate the authenticated query indices and the one transcript-derived
/// out-of-domain point from a single compiled oracle.
///
/// The second value holds every fixed column at `deep_point`, in column order.
pub(super) fn query_rows_and_deep_point_v1(
    columns: Vec<Vec<F>>,
    trace_log2: u8,
    lde_log2: u8,
    query_indices: &[usize],
    deep_point: E,
) -> Result<(BTreeMap<usize, Vec<F>>, Vec<E>), ProofManagedNoteStarkErrorV1> {
    validate_query_geometry_v1(trace_log2, lde_log2, query_indices)?;
    let oracle = Oracle::new(columns, trace_log2)?;
    let rows = oracle.query_rows(lde_log2, query_indices)?;
    let deep_row = oracle.evaluate_extension(deep_point)?;
    Ok((rows, deep_row))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_queries_equal_full_native_ifft_lde_for_every_column_shape() {
        for trace_log2 in [3, 6, 9] {
            let size = 1_usize << trace_log2;
            let dense = (0..size)
                .map(|row| F((row * row * 17 + row * 29 + 3) as u64))
                .collect::<Vec<_>>();
            let columns = vec![
                vec![F::ZERO; size],
                vec![F(83); size],
                dense.clone(),
                dense.iter().map(|value| value.add(F(47))).collect(),
                (0..size)
                    .map(|row| if row % 17 == 0 { F(7) } else { F::ZERO })
                    .collect(),
                (0..size)
                    .map(|row| if row + 1 == size { F(11) } else { F(13) })
                    .collect(),
            ];
            let lde_log2 = trace_log2 + 3;
            let reference = super::super::fixed_lde_columns_v1(&columns, trace_log2, lde_log2)
                .expect("native full LDE");
            let indices = [0, 1, 7, 19, (1_usize << lde_log2) - 1];
            let actual =
                query_rows_v1(columns, trace_log2, lde_log2, &indices).expect("selected rows");
            for index in indices {
                assert_eq!(
                    actual[&index],
                    reference
                        .iter()
                        .map(|column| column[index])
                        .collect::<Vec<_>>()
                );
            }
        }
    }

    /// One column of every packed shape: constant, dense, shifted, sparse and near-constant.
    fn every_column_shape(size: usize) -> Vec<Vec<F>> {
        let dense = (0..size)
            .map(|row| F((row * row * 17 + row * 29 + 3) as u64))
            .collect::<Vec<_>>();
        vec![
            vec![F::ZERO; size],
            vec![F(83); size],
            dense.clone(),
            dense.iter().map(|value| value.add(F(47))).collect(),
            (0..size)
                .map(|row| if row % 17 == 0 { F(7) } else { F::ZERO })
                .collect(),
            (0..size)
                .map(|row| if row + 1 == size { F(11) } else { F(13) })
                .collect(),
        ]
    }

    #[test]
    fn deep_point_row_equals_coefficient_horner_for_every_column_shape() {
        let point = E::canonical([5, 11, 17, 23]).expect("canonical extension point");
        for trace_log2 in [3, 6, 9] {
            let columns = every_column_shape(1_usize << trace_log2);
            let root = goldilocks_primitive_root_v1(trace_log2).expect("native root");
            let reference = columns
                .iter()
                .map(|column| {
                    let mut coefficients = column.clone();
                    crate::privacy_engines::transparent_stark::goldilocks_ifft_v1(
                        &mut coefficients,
                        root,
                    )
                    .expect("native interpolant");
                    coefficients
                        .iter()
                        .rev()
                        .fold(E::ZERO, |value, coefficient| {
                            value.mul(point).add(E::from_base(*coefficient))
                        })
                })
                .collect::<Vec<_>>();
            let (_, actual) =
                query_rows_and_deep_point_v1(columns, trace_log2, trace_log2 + 3, &[0], point)
                    .expect("selected rows and deep point");
            assert_eq!(actual, reference);
        }
    }

    #[test]
    fn deep_point_row_at_an_embedded_base_point_equals_the_base_query_row() {
        let (trace_log2, lde_log2) = (6, 9);
        let lde_root = goldilocks_primitive_root_v1(lde_log2).expect("LDE root");
        for index in [0, 5, (1_usize << lde_log2) - 1] {
            let x = F(GOLDILOCKS_GENERATOR_V1).mul(lde_root.pow(index as u128));
            let (rows, deep_row) = query_rows_and_deep_point_v1(
                every_column_shape(1_usize << trace_log2),
                trace_log2,
                lde_log2,
                &[index],
                E::from_base(x),
            )
            .expect("selected rows and deep point");
            assert_eq!(
                deep_row,
                rows[&index]
                    .iter()
                    .map(|value| E::from_base(*value))
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn deep_point_in_the_native_subgroup_or_noncanonical_is_rejected() {
        let oracle = Oracle::new(every_column_shape(8), 3).expect("small native oracle");
        for root in &oracle.roots {
            assert_eq!(
                oracle.evaluate_extension(E::from_base(*root)),
                Err(ProofManagedNoteStarkErrorV1::InvalidProfile),
                "a zero denominator must never produce an interpolated value"
            );
        }
        assert_eq!(
            oracle.evaluate_extension(E::noncanonical_fixture_v1()),
            Err(ProofManagedNoteStarkErrorV1::InvalidProfile)
        );
    }

    #[test]
    fn rejects_noncanonical_columns_and_inexact_query_geometry() {
        let columns = vec![vec![F::ONE; 8]];
        for indices in [vec![], vec![0, 0], vec![64], vec![0; 137]] {
            assert!(query_rows_v1(columns.clone(), 3, 6, &indices).is_err());
        }
        assert!(query_rows_v1(columns.clone(), 3, 3, &[0]).is_err());
        let mut noncanonical = columns;
        noncanonical[0][1] = F(u64::MAX);
        assert!(query_rows_v1(noncanonical, 3, 6, &[0]).is_err());
        assert!(query_rows_v1(vec![vec![F::ONE; 7]], 3, 6, &[0]).is_err());
    }

    #[test]
    fn native_subgroup_is_rejected_and_privacy_coset_is_disjoint() {
        let oracle = Oracle::new(vec![vec![F::ONE; 8]], 3).expect("small native oracle");
        for root in &oracle.roots {
            assert_eq!(
                oracle.evaluate(*root),
                Err(ProofManagedNoteStarkErrorV1::InvalidProfile),
                "a zero denominator must never produce an interpolated row"
            );
        }
        let native_log2 = super::super::PROOF_MANAGED_NOTE_MAX_NATIVE_TRACE_LOG2_V1;
        let lde_log2 = native_log2 + super::super::PROOF_MANAGED_NOTE_BLOWUP_LOG2_V1;
        let native_size = checked_trace_size_v1(native_log2).expect("native size");
        let lde_size = checked_trace_size_v1(lde_log2).expect("LDE size");
        let shift = F(GOLDILOCKS_GENERATOR_V1);
        // If any shifted LDE point belonged to the native subgroup, its LDE-size
        // power would be one too. This proves disjointness for the entire coset.
        assert_ne!(shift.pow(lde_size as u128), F::ONE);
        let root = goldilocks_primitive_root_v1(lde_log2).expect("LDE root");
        for index in [0, 1, lde_size - 1] {
            let x = shift.mul(root.pow(index as u128));
            assert_ne!(x.pow(native_size as u128), F::ONE);
        }
    }

    #[test]
    fn selected_evaluations_are_identical_across_rayon_widths() {
        let columns = vec![
            (0..512)
                .map(|row| F(u64::try_from(row * row + 7).expect("fixture field element")))
                .collect::<Vec<_>>(),
            (0..512).map(|row| F(u64::from(row % 7 == 0))).collect(),
        ];
        let indices = (0..136).map(|index| index * 29).collect::<Vec<_>>();
        let run = |threads| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .expect("private deterministic query pool")
                .install(|| query_rows_v1(columns.clone(), 9, 12, &indices).expect("queries"))
        };
        let serial = run(1);
        assert_eq!(serial, run(4));
        assert_eq!(serial, run(8));
    }
}
