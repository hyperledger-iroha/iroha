//! Admitted clearing word staging for fixed, private quotient and native replay transforms.
//!
//! Existing F matrices keep their allocations. A single bounded word batch and
//! the complete Metal allowance fit outside all arithmetic, source, replay/cache,
//! scratch and runtime reservations. Private coefficient folding precedes the
//! forward-only adapter; native replay uses inverse-only before masking, and
//! the fixed path also supports inverse/shift recovery.

use super::super::super::private_table::{PrivateTableV1, zeroize_words_v1};
use super::*;
use fastpq_prover::goldilocks_transform::{
    GoldilocksTransformBackendV1 as Backend, GoldilocksTransformDirectionV1 as Direction,
    GoldilocksTransformErrorV1 as TransformError, metal_goldilocks_transform_extra_payload_v1,
};

type Words = PrivateTableV1<Vec<u64>>;

#[derive(Clone, Copy)]
enum TransformUseV1 {
    Fixed,
    PrivateQuotient,
    PrivateNativeReplay,
}

#[derive(Clone, Copy)]
pub(super) struct MainBoundedTransformPolicyV1 {
    available: usize,
    backend: Option<Backend>,
}

impl MainBoundedTransformPolicyV1 {
    pub(super) const fn cpu_v1() -> Self {
        Self {
            available: 0,
            backend: None,
        }
    }

    pub(super) fn for_assembly_v1(
        layout: &AggregateProofLayoutV1,
        assembly_payload: usize,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        let admitted = main_resources::MainProverBufferPlanV1::new_v1(layout)?
            .check_before_sources_v1(assembly_payload)?;
        let ceiling =
            usize::try_from(super::super::super::profile::ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1)
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
        Ok(Self {
            available: ceiling
                .checked_sub(admitted)
                .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?,
            backend: fastpq_prover::goldilocks_transform::available_goldilocks_transform_backend_v1(
            ),
        })
    }

    /// Keep both returned private stripe matrices and bounded replay headers
    /// charged while fixed or private word staging runs. Value payloads remain
    /// in the existing registration/replay/cache ledgers, with no double use of
    /// their slack. The private caller checks actual capacities before staging.
    pub(super) fn for_quotient_layout_v1(
        self,
        base_width: usize,
        aux_width: usize,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        let width = base_width
            .checked_add(aux_width)
            .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?;
        let metadata = width
            .checked_mul(core::mem::size_of::<Vec<F>>())
            .and_then(|n| n.checked_add(2 * core::mem::size_of::<PrivateTableV1<Vec<F>>>()))
            .and_then(|n| {
                n.checked_add(
                    aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1
                        * (core::mem::size_of::<PrivateTableV1<F>>()
                            + core::mem::size_of::<ZeroizingMainTraceColumnV1>()),
                )
            })
            .and_then(|n| {
                n.checked_add(2 * core::mem::size_of::<Vec<ZeroizingMainTraceColumnV1>>())
            })
            .and_then(|n| {
                n.checked_add(core::mem::size_of::<
                    [&[F]; aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1],
                >())
            })
            .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?;
        Ok(Self {
            available: self
                .available
                .checked_sub(metadata)
                .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?,
            ..self
        })
    }

    /// Reserve bounded replay/source/conversion header allocations separately
    /// from native values already covered by the original replay envelope.
    /// Five header arrays cover the native matrix, source iterator, masked
    /// output, prior exhausted iterator and joined coefficient receiver. This
    /// also bounds quotient replay, which uses fewer simultaneous arrays.
    pub(super) fn for_native_replay_v1(self) -> Result<Self, ZkX509StarkErrorV1> {
        let metadata =
            5 * aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1 * core::mem::size_of::<Vec<F>>()
                + core::mem::size_of::<PrivateTableV1<Vec<F>>>()
                + 2 * core::mem::size_of::<PrivateTableV1<F>>()
                + 4 * core::mem::size_of::<std::vec::IntoIter<ZeroizingMainTraceColumnV1>>()
                + 2 * core::mem::size_of::<Vec<ZeroizingMainTraceColumnV1>>()
                + core::mem::size_of::<Option<std::vec::IntoIter<ZeroizingMainTraceColumnV1>>>();
        Ok(Self {
            available: self
                .available
                .checked_sub(metadata)
                .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?,
            ..self
        })
    }

    /// Inject the bounded adapter in host tests without claiming device use.
    #[cfg(test)]
    pub(super) fn for_test_v1(rows: usize, columns: usize) -> Self {
        Self {
            available: required_v1(rows, columns).unwrap(),
            backend: Some(Backend::Metal),
        }
    }

    pub(super) fn columns_v1(self, rows: usize) -> usize {
        if self.backend != Some(Backend::Metal) {
            return 0;
        }
        [8, 4, 2, 1]
            .into_iter()
            .find(|&columns| required_v1(rows, columns).is_ok_and(|bytes| bytes <= self.available))
            .unwrap_or(0)
    }

    /// Apply the fixed-coset inverse/diagonal/forward transition with shared staging.
    pub(super) fn apply_with_v1(
        self,
        columns: &mut [Vec<F>],
        root: F,
        diagonal: F,
        recovery: bool,
        mut transform: impl FnMut(&mut [Vec<u64>], u64, Direction) -> Result<Backend, TransformError>,
        mut uncertain: impl FnMut() -> bool,
    ) -> Result<(), ZkX509StarkErrorV1> {
        self.apply_batch_with_v1(
            columns,
            root,
            diagonal,
            recovery,
            Direction::Forward,
            TransformUseV1::Fixed,
            &mut transform,
            &mut uncertain,
        )
    }

    /// Complete forward transforms of already shifted/folded private columns.
    /// CPU fallback is deterministic and may not bypass quarantined completion.
    pub(super) fn forward_with_v1(
        self,
        columns: &mut [Vec<F>],
        root: F,
        mut transform: impl FnMut(&mut [Vec<u64>], u64, Direction) -> Result<Backend, TransformError>,
        mut uncertain: impl FnMut() -> bool,
    ) -> Result<(), ZkX509StarkErrorV1> {
        self.private_with_v1(
            columns,
            root,
            Direction::Forward,
            TransformUseV1::PrivateQuotient,
            &mut transform,
            &mut uncertain,
        )
    }

    /// Interpolate native evaluations before original masks are applied.
    /// This is inverse-only: no diagonal or subsequent forward pass occurs.
    pub(super) fn inverse_with_v1(
        self,
        columns: &mut [Vec<F>],
        root: F,
        mut transform: impl FnMut(&mut [Vec<u64>], u64, Direction) -> Result<Backend, TransformError>,
        mut uncertain: impl FnMut() -> bool,
    ) -> Result<(), ZkX509StarkErrorV1> {
        self.private_with_v1(
            columns,
            root,
            Direction::Inverse,
            TransformUseV1::PrivateNativeReplay,
            &mut transform,
            &mut uncertain,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn private_with_v1(
        self,
        columns: &mut [Vec<F>],
        root: F,
        direction: Direction,
        usage: TransformUseV1,
        mut transform: impl FnMut(&mut [Vec<u64>], u64, Direction) -> Result<Backend, TransformError>,
        mut uncertain: impl FnMut() -> bool,
    ) -> Result<(), ZkX509StarkErrorV1> {
        check_completion_v1(uncertain())?;
        let rows = columns.first().map_or(0, Vec::len);
        if columns.is_empty()
            || columns.len() > aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1
            || rows < 2
            || !rows.is_power_of_two()
            || rows > 1 << main_quotient_stripes::MAIN_QUOTIENT_STRIPE_LOG2_V1
            || columns.iter().any(|column| column.len() != rows)
            || columns
                .iter()
                .flatten()
                .any(|value| F::canonical(value.0).is_none())
            || F::canonical(root.0).is_none()
            || root.pow(rows as u128) != F::ONE
            || root.pow((rows / 2) as u128) == F::ONE
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let width = self.columns_v1(rows);
        if width == 0 {
            columns.par_iter_mut().try_for_each(|column| {
                match direction {
                    Direction::Forward => {
                        crate::privacy_engines::transparent_stark::goldilocks_fft_v1(column, root)
                    }
                    Direction::Inverse => {
                        crate::privacy_engines::transparent_stark::goldilocks_ifft_v1(column, root)
                    }
                }
                .map_err(map_transparent_error_v1)
            })?;
            check_completion_v1(uncertain())?;
            #[cfg(test)]
            match usage {
                TransformUseV1::PrivateQuotient => {
                    super::super::super::prover_observation::completed_quotient_backend_v1(
                        false,
                        columns.len(),
                    )
                }
                TransformUseV1::PrivateNativeReplay => {
                    super::super::super::prover_observation::completed_native_replay_backend_v1(
                        false,
                        columns.len(),
                    )
                }
                TransformUseV1::Fixed => unreachable!("fixed transitions use the fixed adapter"),
            }
        } else {
            for batch in columns.chunks_mut(width) {
                self.apply_batch_with_v1(
                    batch,
                    root,
                    F::ONE,
                    false,
                    direction,
                    usage,
                    &mut transform,
                    &mut uncertain,
                )?;
            }
        }
        check_completion_v1(uncertain())
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_batch_with_v1(
        self,
        columns: &mut [Vec<F>],
        root: F,
        diagonal: F,
        recovery: bool,
        direction: Direction,
        usage: TransformUseV1,
        mut transform: impl FnMut(&mut [Vec<u64>], u64, Direction) -> Result<Backend, TransformError>,
        mut uncertain: impl FnMut() -> bool,
    ) -> Result<(), ZkX509StarkErrorV1> {
        check_completion_v1(uncertain())?;
        let rows = columns.first().map_or(0, Vec::len);
        if columns.is_empty()
            || columns.len() > self.columns_v1(rows)
            || columns.iter().any(|column| column.len() != rows)
            || columns
                .iter()
                .flatten()
                .any(|value| F::canonical(value.0).is_none())
            || F::canonical(diagonal.0).is_none()
            || diagonal == F::ZERO
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let mut words =
            allocate_words_with_v1(rows, columns.len(), self.available, |column, count| {
                column
                    .try_reserve_exact(count)
                    .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)
            })?;
        for (target, source) in words.iter_mut().zip(columns.iter()) {
            for (target, value) in target.iter_mut().zip(source) {
                *target = value.0;
            }
        }
        if recovery {
            checked_transform_v1(
                &mut words,
                root,
                Direction::Inverse,
                usage,
                &mut transform,
                &mut uncertain,
            )?;
        }
        // Independent columns retain the original CPU path's parallelism;
        // no reduction order or additional field matrix is introduced.
        if diagonal != F::ONE {
            words.par_iter_mut().for_each(|column| {
                let mut power = F::ONE;
                for word in column {
                    // Input copy and every completed transform validate all words.
                    *word = F(*word).mul(power).0;
                    power = power.mul(diagonal);
                }
            });
        }
        checked_transform_v1(
            &mut words,
            root,
            direction,
            usage,
            &mut transform,
            &mut uncertain,
        )?;
        // checked_transform_v1 validated the entire returned batch before
        // this copy into existing F allocations; no second field batch lives.
        for (target, source) in columns.iter_mut().zip(words.iter()) {
            for (target, word) in target.iter_mut().zip(source) {
                *target = F(*word);
            }
        }
        Ok(())
    }
}

pub(super) fn check_completion_v1(uncertain: bool) -> Result<(), ZkX509StarkErrorV1> {
    if uncertain {
        Err(ZkX509StarkErrorV1::AcceleratorCompletionUncertain)
    } else {
        Ok(())
    }
}

fn checked_transform_v1(
    words: &mut [Vec<u64>],
    root: F,
    direction: Direction,
    usage: TransformUseV1,
    transform: &mut impl FnMut(&mut [Vec<u64>], u64, Direction) -> Result<Backend, TransformError>,
    uncertain: &mut impl FnMut() -> bool,
) -> Result<(), ZkX509StarkErrorV1> {
    check_completion_v1(uncertain())?;
    let rows = words.first().map_or(0, Vec::len);
    // This admission relies on the exact-root facade preserving caller Vec
    // allocations: CPU uses slices and Metal copies back only after its wait.
    // Any future reallocating facade must update this resource contract first.
    let allocations: [(*const u64, usize); 8] = core::array::from_fn(|index| {
        words.get(index).map_or((core::ptr::null(), 0), |column| {
            (column.as_ptr(), column.capacity())
        })
    });
    let result = transform(words, root.0, direction);
    check_completion_v1(uncertain())?;
    let backend = result.map_err(|error| match error {
        TransformError::CompletionUncertain => ZkX509StarkErrorV1::AcceleratorCompletionUncertain,
        TransformError::NonCanonicalInput => ZkX509StarkErrorV1::NonCanonicalField,
        _ => ZkX509StarkErrorV1::ProfileMismatch,
    })?;
    if backend == Backend::Cuda
        || words.iter().enumerate().any(|(index, column)| {
            column.len() != rows || (column.as_ptr(), column.capacity()) != allocations[index]
        })
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    if words
        .iter()
        .flatten()
        .any(|word| F::canonical(*word).is_none())
    {
        return Err(ZkX509StarkErrorV1::NonCanonicalField);
    }
    #[cfg(test)]
    match usage {
        TransformUseV1::Fixed => {
            super::super::super::prover_observation::completed_fixed_backend_v1(
                backend == Backend::Metal,
                direction == Direction::Inverse,
                words.len(),
            )
        }
        TransformUseV1::PrivateQuotient => {
            super::super::super::prover_observation::completed_quotient_backend_v1(
                backend == Backend::Metal,
                words.len(),
            )
        }
        TransformUseV1::PrivateNativeReplay => {
            super::super::super::prover_observation::completed_native_replay_backend_v1(
                backend == Backend::Metal,
                words.len(),
            )
        }
    }
    #[cfg(not(test))]
    let _ = usage;
    Ok(())
}

fn erase_words_v1(rows: &mut [Vec<u64>]) {
    for row in rows {
        // Extending only to existing capacity cannot allocate. Clear the whole
        // admitted allocation even if a malformed result shortened its length.
        row.resize(row.capacity(), 0);
        zeroize_words_v1(row);
    }
}

fn required_v1(rows: usize, columns: usize) -> Result<usize, ZkX509StarkErrorV1> {
    if rows > 1 << main_quotient_stripes::MAIN_QUOTIENT_STRIPE_LOG2_V1 {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let extra = metal_goldilocks_transform_extra_payload_v1(rows, columns)
        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
    rows.checked_mul(columns)
        .and_then(|n| n.checked_mul(core::mem::size_of::<u64>()))
        .and_then(|n| n.checked_add(columns.checked_mul(core::mem::size_of::<Vec<u64>>())?))
        .and_then(|n| n.checked_add(core::mem::size_of::<Words>()))
        .and_then(|n| n.checked_add(extra))
        .ok_or(ZkX509StarkErrorV1::ProofTooLarge)
}

fn allocate_words_with_v1(
    rows: usize,
    columns: usize,
    available: usize,
    mut reserve: impl FnMut(&mut Vec<u64>, usize) -> Result<(), ZkX509StarkErrorV1>,
) -> Result<Words, ZkX509StarkErrorV1> {
    if required_v1(rows, columns)? > available {
        return Err(ZkX509StarkErrorV1::ProofTooLarge);
    }
    let extra = metal_goldilocks_transform_extra_payload_v1(rows, columns)
        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
    let mut words = Words::new(Vec::new(), erase_words_v1);
    words
        .try_reserve_exact(columns)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    let mut actual = words
        .capacity()
        .checked_mul(core::mem::size_of::<Vec<u64>>())
        .and_then(|n| n.checked_add(core::mem::size_of::<Words>()))
        .and_then(|n| n.checked_add(extra))
        .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?;
    if actual > available {
        return Err(ZkX509StarkErrorV1::ProofTooLarge);
    }
    for _ in 0..columns {
        let mut column = PrivateTableV1::new(Vec::new(), zeroize_words_v1);
        reserve(&mut column, rows)?;
        actual = column
            .capacity()
            .checked_mul(core::mem::size_of::<u64>())
            .and_then(|n| actual.checked_add(n))
            .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?;
        if actual > available || column.capacity() < rows {
            return Err(ZkX509StarkErrorV1::ProofTooLarge);
        }
        column.resize(rows, 0);
        words.push(column.into_vec());
    }
    Ok(words)
}

#[cfg(test)]
#[path = "main_bounded_transform_tests.rs"]
mod tests;
