//! Native execution proof driver; privacy proofs use their separate implementation.
//! Exact selected-query evaluation of execution profiles' public fixed polynomials.
//!
//! The values are the same degree-<N interpolants used by the prover's IFFT/LDE.
//! All inputs come from the compiled adapter; proof bytes supply neither these
//! polynomials nor their evaluations. Privacy profiles retain their original path.

use super::super::transparent_stark::{
    GOLDILOCKS_GENERATOR_V1, goldilocks_batch_invert_v1, goldilocks_primitive_root_v1,
};
use super::{F, ProofManagedNoteStarkErrorV1, checked_trace_size_v1, map_transparent_error_v1};
use rayon::prelude::*;
use std::collections::BTreeMap;

/// At most four independent inversion buffers are resident, regardless of Rayon width.
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
}

/// Evaluate precisely the authenticated query indices; no LDE-sized matrix is allocated.
pub(super) fn query_rows_v1(
    columns: Vec<Vec<F>>,
    trace_log2: u8,
    lde_log2: u8,
    query_indices: &[usize],
) -> Result<BTreeMap<usize, Vec<F>>, ProofManagedNoteStarkErrorV1> {
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
    let oracle = Oracle::new(columns, trace_log2)?;
    let lde_root = goldilocks_primitive_root_v1(lde_log2).map_err(map_transparent_error_v1)?;
    let mut rows = BTreeMap::new();
    for batch in query_indices.chunks(QUERY_BATCH) {
        let results = batch
            .par_iter()
            .map(|index| {
                let x = F(GOLDILOCKS_GENERATOR_V1).mul(lde_root.pow(*index as u128));
                oracle.evaluate(x)
            })
            .collect::<Vec<_>>();
        for (index, row) in batch.iter().copied().zip(results) {
            rows.insert(index, row?);
        }
    }
    Ok(rows)
}
