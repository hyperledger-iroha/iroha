//! Bounded evaluation of the unchanged MAIN coset polynomials.
//!
//! Eight evaluation columns remain resident. The measured exact-root Metal
//! kernel is selected only when its complete staging allowance fits beyond all
//! admitted source, replay scratch and runtime reserves. CPU fallback preserves
//! the same polynomial and does not relax uncertain-completion admission.
//! Measured small selected sets use CPU Horner or interleaved eight-column pruning
//! before any full-domain device staging is allocated.

use super::super::super::private_table::{PrivateTableV1, zeroize_fields_v1, zeroize_words_v1};
#[cfg(test)]
use super::super::super::prover_observation;
use super::*;
use crate::privacy_engines::transparent_stark::masked_trace_coefficients_on_coset_v1;
use fastpq_prover::goldilocks_transform::{
    GoldilocksTransformBackendV1 as Backend, GoldilocksTransformDirectionV1 as Direction,
    transform_goldilocks_columns_v1,
};

type Column = aggregate::ZeroizingFieldColumnV1;

#[path = "main_selected_coset.rs"]
mod selected_coset;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(super) struct MainTransformReceiptV1 {
    cpu_columns: usize,
    metal_columns: usize,
    #[cfg(test)]
    observe_full_cpu_routes: bool,
    #[cfg(test)]
    full_dense_cpu_columns: usize,
    #[cfg(test)]
    full_prefix_cpu_columns: usize,
}

// The full MAIN layout's largest masked polynomial is native log19 plus its
// original hiding coefficients. This public allocation can coexist with source
// callbacks, so the common source envelope reserves it before any source build.
const MAX_PUBLIC_COSET_POWERS_V1: usize = (1 << 19) + MASK_DEGREE + 1;
pub(super) const PUBLIC_COSET_POWERS_ALLOWANCE_V1: usize = MAX_PUBLIC_COSET_POWERS_V1
    * core::mem::size_of::<F>()
    + core::mem::size_of::<MainTraceCosetEvaluatorV1>();

/// Verifier-public generator powers; no witness or transcript material is stored.
struct PublicCosetPowersV1 {
    values: Vec<F>,
}

impl PublicCosetPowersV1 {
    fn new_v1(count: usize) -> Result<Self, AggregateStarkErrorV1> {
        if count == 0 || count > MAX_PUBLIC_COSET_POWERS_V1 {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
        if values.capacity() != count {
            return Err(AggregateStarkErrorV1::AllocationFailure);
        }
        let shift = F(GOLDILOCKS_GENERATOR_V1);
        let mut power = F::ONE;
        for _ in 0..count {
            values.push(power);
            power = power.mul(shift);
        }
        Ok(Self { values })
    }
}

pub(super) struct MainTraceCosetEvaluatorV1 {
    evaluation_rows: usize,
    device_columns: usize,
    receipt: MainTransformReceiptV1,
    // One owner for the entire joined commitment/replay; released with self.
    coset_powers: Option<PublicCosetPowersV1>,
    // Field payload replaces the existing eight-output replay allowance. Its
    // inline owner is included by PUBLIC_COSET_POWERS_ALLOWANCE_V1 above.
    selected_scratch: selected_coset::SelectedScratchV1,
}

/// Outer taken owner, previous owner, fallback match binding and active matrix
/// headers, conservatively simultaneous inside the unchanged runtime reserve.
pub(super) const fn selected_scratch_metadata_v1() -> usize {
    4 * core::mem::size_of::<selected_coset::SelectedScratchV1>()
}

impl MainTraceCosetEvaluatorV1 {
    pub(super) fn new_v1(
        layout: &AggregateProofLayoutV1,
        assembly_payload: usize,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        let admitted = main_resources::MainProverBufferPlanV1::new_v1(layout)?
            .joined_transform_admission_v1(assembly_payload)?;
        let ceiling =
            usize::try_from(super::super::super::profile::ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1)
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
        let available = ceiling
            .checked_sub(admitted)
            .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?;
        // Native19/common22 full-output parity and bounded dispatch timing
        // qualify this kernel selection, not the complete MAIN time budget.
        // CUDA has no matching staging contract and remains on the CPU path.
        let device_columns = select_device_columns_v1(
            layout.common_lde_size(),
            available,
            fastpq_prover::goldilocks_transform::available_goldilocks_transform_backend_v1(),
        );
        // check_before_sources_v1 already reserved this whole public owner;
        // do not spend that allowance again or subtract it twice here. CPU-only
        // evaluation retains its existing implementation and allocates no table.
        let coset_powers = if device_columns == 0 {
            None
        } else {
            Some(
                PublicCosetPowersV1::new_v1(MAX_PUBLIC_COSET_POWERS_V1)
                    .map_err(map_aggregate_error_v1)?,
            )
        };
        #[cfg(test)]
        prover_observation::policy_v1(device_columns);
        Ok(Self {
            evaluation_rows: layout.common_lde_size(),
            device_columns,
            receipt: MainTransformReceiptV1::default(),
            coset_powers,
            selected_scratch: selected_coset::SelectedScratchV1::default(),
        })
    }

    pub(super) fn receipt_v1(&self) -> &MainTransformReceiptV1 {
        &self.receipt
    }

    /// Keep completed initial outputs in their original clearing GPU backing.
    /// Selected replay retains its existing field outputs and pruning policy.
    pub(super) fn evaluate_initial_v1(
        &mut self,
        columns: &[Column],
        native_log: u8,
        common_log: u8,
    ) -> Result<
        Vec<aggregate::initial_evaluated_column::InitialEvaluatedColumnV1>,
        AggregateStarkErrorV1,
    > {
        use aggregate::initial_evaluated_column::InitialEvaluatedColumnV1 as Evaluated;
        if self.device_columns == 0 {
            return self
                .evaluate_v1(columns, native_log, common_log)
                .map(|columns| columns.into_iter().map(Evaluated::from_field_v1).collect());
        }
        // End a previous query matrix before every validation/dispatch/fallback.
        self.selected_scratch.release_v1();
        let result = self.evaluate_initial_inner_v1(columns, native_log, common_log);
        #[cfg(test)]
        if result.is_err() {
            prover_observation::failed_transform_v1();
        }
        result
    }

    fn evaluate_initial_inner_v1(
        &mut self,
        columns: &[Column],
        native_log: u8,
        common_log: u8,
    ) -> Result<
        Vec<aggregate::initial_evaluated_column::InitialEvaluatedColumnV1>,
        AggregateStarkErrorV1,
    > {
        use crate::privacy_engines::transparent_stark::goldilocks_words_v1;
        use aggregate::initial_evaluated_column::InitialEvaluatedColumnV1 as Evaluated;
        use fastpq_prover::goldilocks_transform::{
            PreparedGoldilocksColumnsOutcomeV1 as Prepared,
            goldilocks_transform_completion_uncertain_v1, prepare_goldilocks_prefix_columns_v1,
            prepared_goldilocks_columns_payload_bound_v1,
        };
        if goldilocks_transform_completion_uncertain_v1() {
            return Err(AggregateStarkErrorV1::InternalInvariant);
        }
        let rows = 1_usize
            .checked_shl(u32::from(common_log))
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
        let native_rows = 1_usize
            .checked_shl(u32::from(native_log))
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
        let root = goldilocks_primitive_root_v1(common_log)
            .map_err(aggregate::map_transparent_error_v1)?;
        let shift = F(GOLDILOCKS_GENERATOR_V1);
        if rows != self.evaluation_rows
            || columns.is_empty()
            || columns.len() > aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1
            || rows <= native_rows
            || shift.pow(rows as u128) == F::ONE
            || shift.pow(native_rows as u128) == F::ONE
            || columns
                .iter()
                .any(|column| column.is_empty() || column.len() > rows)
        {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        if columns
            .iter()
            .flat_map(|column| column.iter())
            .any(|value| F::canonical(value.0).is_none())
        {
            return Err(AggregateStarkErrorV1::NonCanonicalField);
        }
        // Retained pages replace existing field cells only when each column
        // fills whole shared pages. Tiny domains use the original CPU owner;
        // no uncharged page padding can survive into a source callback.
        if rows < fastpq_prover::goldilocks_transform::MIN_UNPADDED_GOLDILOCKS_COLUMN_ROWS_V1 {
            let outputs = columns
                .par_iter()
                .map(|column| {
                    masked_trace_coefficients_on_coset_v1(column, native_log, common_log)
                        .map(Column::from_vec_v1)
                        .map(Evaluated::from_field_v1)
                        .map_err(aggregate::map_transparent_error_v1)
                })
                .collect::<Result<Vec<_>, _>>()?;
            if goldilocks_transform_completion_uncertain_v1() {
                return Err(AggregateStarkErrorV1::InternalInvariant);
            }
            self.receipt.cpu_columns += columns.len();
            #[cfg(test)]
            prover_observation::completed_transform_v1(false, columns.len());
            return Ok(outputs);
        }
        let powers = &self
            .coset_powers
            .as_ref()
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?
            .values;
        if columns.iter().any(|column| column.len() > powers.len()) {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        let mut outputs = Vec::new();
        outputs
            .try_reserve_exact(columns.len())
            .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
        if outputs.capacity() != columns.len() {
            return Err(AggregateStarkErrorV1::AllocationFailure);
        }
        for batch in columns.chunks(self.device_columns) {
            let views: [&[u64]; 8] = core::array::from_fn(|index| {
                batch
                    .get(index)
                    .map_or::<&[u64], _>(&[], |column| goldilocks_words_v1(column))
            });
            match prepare_goldilocks_prefix_columns_v1(
                &views[..batch.len()],
                rows,
                root.0,
                goldilocks_words_v1(powers),
                fastpq_prover::ExecutionMode::Auto,
            )
            .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?
            {
                Prepared::Metal(prepared) => {
                    let allowed = prepared_goldilocks_columns_payload_bound_v1(rows, batch.len())
                        .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?;
                    if prepared
                        .allocated_payload_bytes_v1()
                        .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?
                        > allowed
                    {
                        return Err(AggregateStarkErrorV1::AllocationFailure);
                    }
                    let completed = prepared
                        .into_readonly_v1()
                        .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?;
                    outputs.extend(Evaluated::from_completed_v1(completed)?);
                    self.receipt.metal_columns += batch.len();
                    #[cfg(test)]
                    prover_observation::completed_transform_v1(true, batch.len());
                }
                Prepared::Cpu => {
                    if goldilocks_transform_completion_uncertain_v1() {
                        return Err(AggregateStarkErrorV1::InternalInvariant);
                    }
                    // Only this still-borrowed native batch falls back. Earlier
                    // completed chunks remain outputs; no second full matrix exists.
                    let evaluated = batch
                        .par_iter()
                        .map(|column| {
                            masked_trace_coefficients_on_coset_v1(column, native_log, common_log)
                                .map(Column::from_vec_v1)
                                .map(Evaluated::from_field_v1)
                                .map_err(aggregate::map_transparent_error_v1)
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    outputs.extend(evaluated);
                    self.receipt.cpu_columns += batch.len();
                    #[cfg(test)]
                    prover_observation::completed_transform_v1(false, batch.len());
                }
            }
        }
        // A concurrent device owner may become uncertain while CPU fallback,
        // canonical validation or output-handle allocation was completing.
        // Refuse before the accumulated columns reach any hashing callback.
        if goldilocks_transform_completion_uncertain_v1() {
            return Err(AggregateStarkErrorV1::InternalInvariant);
        }
        Ok(outputs)
    }

    pub(super) fn evaluate_v1(
        &mut self,
        columns: &[Column],
        native_log: u8,
        common_log: u8,
    ) -> Result<Vec<Column>, AggregateStarkErrorV1> {
        self.evaluate_rows_v1(columns, native_log, common_log, None)
    }

    /// Return only the exact canonical selected rows from the original coset.
    pub(super) fn evaluate_selected_v1(
        &mut self,
        columns: &[Column],
        native_log: u8,
        common_log: u8,
        selected: &[usize],
    ) -> Result<Vec<Column>, AggregateStarkErrorV1> {
        self.evaluate_rows_v1(columns, native_log, common_log, Some(selected))
    }

    fn evaluate_rows_v1(
        &mut self,
        columns: &[Column],
        native_log: u8,
        common_log: u8,
        selected: Option<&[usize]>,
    ) -> Result<Vec<Column>, AggregateStarkErrorV1> {
        // An already admitted CPU phase must also stop if another device call
        // has since left a live private allocation with uncertain completion.
        if fastpq_prover::goldilocks_transform::goldilocks_transform_completion_uncertain_v1() {
            self.selected_scratch.release_v1();
            #[cfg(test)]
            prover_observation::failed_transform_v1();
            return Err(AggregateStarkErrorV1::InternalInvariant);
        }
        let result = self.evaluate_selected_with_v1(
            columns,
            native_log,
            common_log,
            selected,
            |values, root| {
                transform_goldilocks_columns_v1(
                    values,
                    root,
                    Direction::Forward,
                    fastpq_prover::ExecutionMode::Auto,
                )
                .map_err(|_| AggregateStarkErrorV1::InvalidLayout)
            },
        );
        #[cfg(test)]
        if result.is_err() {
            prover_observation::failed_transform_v1();
        }
        result
    }

    #[cfg(test)]
    fn evaluate_with_v1(
        &mut self,
        columns: &[Column],
        native_log: u8,
        common_log: u8,
        transform: impl FnMut(&mut [Vec<u64>], u64) -> Result<Backend, AggregateStarkErrorV1>,
    ) -> Result<Vec<Column>, AggregateStarkErrorV1> {
        self.evaluate_selected_with_v1(columns, native_log, common_log, None, transform)
    }

    fn evaluate_selected_with_v1(
        &mut self,
        columns: &[Column],
        native_log: u8,
        common_log: u8,
        selected: Option<&[usize]>,
        transform: impl FnMut(&mut [Vec<u64>], u64) -> Result<Backend, AggregateStarkErrorV1>,
    ) -> Result<Vec<Column>, AggregateStarkErrorV1> {
        self.evaluate_selected_policy_with_v1::<true>(
            columns, native_log, common_log, selected, transform,
        )
    }

    // The false instantiation is used only by the native cost diagnostic to
    // compare the original device route with the selected-coordinate policy.
    fn evaluate_selected_policy_with_v1<const SELECT_CPU: bool>(
        &mut self,
        columns: &[Column],
        native_log: u8,
        common_log: u8,
        selected: Option<&[usize]>,
        mut transform: impl FnMut(&mut [Vec<u64>], u64) -> Result<Backend, AggregateStarkErrorV1>,
    ) -> Result<Vec<Column>, AggregateStarkErrorV1> {
        // Local custody covers validation failures and unwinds as well as
        // arithmetic errors. Only a complete selected evaluation restores it.
        let mut selected_scratch = core::mem::take(&mut self.selected_scratch);
        let rows = 1_usize
            .checked_shl(u32::from(common_log))
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
        let native_rows = 1_usize
            .checked_shl(u32::from(native_log))
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
        let root = goldilocks_primitive_root_v1(common_log)
            .map_err(aggregate::map_transparent_error_v1)?;
        let shift = F(GOLDILOCKS_GENERATOR_V1);
        if rows != self.evaluation_rows
            || columns.is_empty()
            || columns.len() > aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1
            || rows <= native_rows
            || shift.pow(rows as u128) == F::ONE
            || shift.pow(native_rows as u128) == F::ONE
            || columns
                .iter()
                .any(|column| column.is_empty() || column.len() > rows)
        {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        if selected.is_some_and(|indices| {
            indices.is_empty()
                || indices.last().is_none_or(|&row| row >= rows)
                || indices.windows(2).any(|pair| pair[0] >= pair[1])
        }) {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        if columns
            .iter()
            .flat_map(|column| column.iter())
            .any(|value| F::canonical(value.0).is_none())
        {
            return Err(AggregateStarkErrorV1::NonCanonicalField);
        }
        let use_horner = SELECT_CPU
            && selected.is_some_and(|indices| {
                columns.iter().all(|column| {
                    selected_coset::prefer_horner_v1(
                        native_log,
                        common_log,
                        column.len(),
                        indices.len(),
                    )
                })
            });
        let use_pruned_fft = SELECT_CPU
            && selected.is_some_and(|indices| {
                columns.iter().all(|column| {
                    selected_coset::prefer_pruned_fft_v1(
                        native_log,
                        common_log,
                        column.len(),
                        indices.len(),
                        columns.len(),
                    )
                })
            });
        if !use_pruned_fft {
            // Never overlap a retained selected matrix with scalar/device FFT
            // allocations. This also releases it before partial-column batches.
            selected_scratch.release_v1();
        }
        let prefix_full_batch = selected.is_none() && columns.len() == 8;
        // Selection follows complete original validation and the outer
        // uncertain-completion check, before allocating any device words.
        if self.device_columns == 0 || use_horner || use_pruned_fft {
            let result = if use_pruned_fft {
                selected_coset::evaluate_batched_v1(
                    &mut selected_scratch,
                    columns,
                    native_log,
                    common_log,
                    selected.ok_or(AggregateStarkErrorV1::InvalidLayout)?,
                )?
            } else {
                columns
                    .par_iter()
                    .map(|column| {
                        selected.map_or_else(
                            || {
                                if prefix_full_batch {
                                    return selected_coset::evaluate_full_v1(
                                        column, native_log, common_log,
                                    );
                                }
                                masked_trace_coefficients_on_coset_v1(
                                    column, native_log, common_log,
                                )
                                .map(Column::from_vec_v1)
                                .map_err(aggregate::map_transparent_error_v1)
                            },
                            |indices| {
                                if use_horner {
                                    selected_coset::evaluate_horner_v1(
                                        column, native_log, common_log, indices,
                                    )
                                } else {
                                    selected_coset::evaluate_v1(
                                        column, native_log, common_log, indices,
                                    )
                                }
                            },
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?
            };
            #[cfg(test)]
            if self.receipt.observe_full_cpu_routes && selected.is_none() {
                if prefix_full_batch {
                    self.receipt.full_prefix_cpu_columns += columns.len();
                } else {
                    self.receipt.full_dense_cpu_columns += columns.len();
                }
            }
            self.receipt.cpu_columns += columns.len();
            #[cfg(test)]
            prover_observation::completed_transform_v1(false, columns.len());
            self.selected_scratch = selected_scratch;
            return Ok(result);
        }
        let powers = &self
            .coset_powers
            .as_ref()
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?
            .values;
        if columns.iter().any(|column| column.len() > powers.len()) {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        // Guard both allocation levels before any private write. Each FFT
        // operates on at most the admitted batch, although eight outputs live.
        let mut words = PrivateTableV1::new(Vec::new(), erase_word_rows_v1);
        words
            .try_reserve_exact(columns.len())
            .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
        for _ in columns {
            let mut column = PrivateTableV1::new(Vec::new(), zeroize_words_v1);
            column
                .try_reserve_exact(rows)
                .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
            column.resize(rows, 0);
            words.push(column.into_vec());
        }
        // Public powers are constructed once, not once per private column.
        // Each output has one writer; elementwise products introduce no reduction
        // or scheduling-dependent arithmetic and preserve the original FFT order.
        populate_coset_words_v1(&mut words, columns, powers)?;
        for batch in words.chunks_mut(self.device_columns) {
            match transform(batch, root.0)? {
                Backend::Cpu => {
                    self.receipt.cpu_columns += batch.len();
                    #[cfg(test)]
                    prover_observation::completed_transform_v1(false, batch.len());
                }
                Backend::Metal => {
                    self.receipt.metal_columns += batch.len();
                    #[cfg(test)]
                    prover_observation::completed_transform_v1(true, batch.len());
                }
                // Detection is process-immutable, and this evaluator is only
                // admitted for Metal. Never reinterpret another backend receipt.
                Backend::Cuda => return Err(AggregateStarkErrorV1::InvalidLayout),
            }
        }
        let mut output = Vec::new();
        output
            .try_reserve_exact(words.len())
            .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
        for words in words.iter_mut() {
            if selected.is_none() {
                // The completed words become the final clearing field owner.
                // Validation covers every output before the new owner returns;
                // no duplicate full column or early wipe is needed.
                output.push(Column::from_canonical_words_v1(core::mem::take(words))?);
                continue;
            }
            let mut column = PrivateTableV1::new(Vec::new(), zeroize_fields_v1);
            let output_rows = selected.map_or(rows, <[usize]>::len);
            column
                .try_reserve_exact(output_rows)
                .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
            if column.capacity() != output_rows {
                return Err(AggregateStarkErrorV1::AllocationFailure);
            }
            // Preserve full device-output canonical checking, including
            // unselected rows; gather only after original completion succeeds.
            if words.iter().any(|&word| F::canonical(word).is_none()) {
                return Err(AggregateStarkErrorV1::NonCanonicalField);
            }
            match selected {
                Some(indices) => column.extend(indices.iter().map(|&row| F(words[row]))),
                None => column.extend(words.iter().copied().map(F)),
            }
            output.push(Column::from_vec_v1(column.into_vec()));
            // Only one field conversion column overlaps the word outputs.
            zeroize_words_v1(words);
            *words = Vec::new();
        }
        Ok(output)
    }
}

/// Populate caller-owned, zero-padded columns from already validated canonical
/// coefficients. The public table is borrowed; no duplicate table is allocated.
fn populate_coset_words_v1(
    words: &mut [Vec<u64>],
    columns: &[Column],
    powers: &[F],
) -> Result<(), AggregateStarkErrorV1> {
    if words.len() != columns.len()
        || columns.is_empty()
        || words.iter().zip(columns).any(|(output, coefficients)| {
            coefficients.len() > output.len() || coefficients.len() > powers.len()
        })
    {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    words
        .par_iter_mut()
        .zip(columns.par_iter())
        .for_each(|(output, coefficients)| {
            for ((output, coefficient), power) in
                output.iter_mut().zip(coefficients.iter()).zip(powers)
            {
                *output = coefficient.mul(*power).0;
            }
        });
    Ok(())
}

fn erase_word_rows_v1(rows: &mut [Vec<u64>]) {
    for row in rows {
        zeroize_words_v1(row);
    }
}

fn select_device_columns_v1(rows: usize, available: usize, backend: Option<Backend>) -> usize {
    if backend == Some(Backend::Metal) {
        select_metal_columns_v1(rows, available)
    } else {
        0
    }
}

fn select_metal_columns_v1(rows: usize, available: usize) -> usize {
    use fastpq_prover::goldilocks_transform::metal_goldilocks_transform_extra_payload_v1;
    // Conversion overlaps only one completed word column. Its allowance is
    // bounded by every admitted Metal dispatch allowance (which includes cache).
    [8, 4, 2, 1]
        .into_iter()
        .find(|&columns| {
            metal_goldilocks_transform_extra_payload_v1(rows, columns)
                .is_ok_and(|required| required <= available)
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::super::super::super::private_table::inspection;
    use super::*;
    use fastpq_prover::goldilocks_transform::metal_goldilocks_transform_extra_payload_v1;

    #[test]
    fn production_transform_selects_only_admitted_metal_backend() {
        let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
        let plan = main_resources::MainProverBufferPlanV1::new_v1(&layout).unwrap();
        let cap = super::super::super::super::profile::ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1 as usize;
        let available = cap - plan.joined_transform_admission_v1(288_345_698).unwrap();
        let evaluator = MainTraceCosetEvaluatorV1::new_v1(&layout, 288_345_698).unwrap();
        assert_eq!(
            evaluator.device_columns,
            select_device_columns_v1(
                layout.common_lde_size(),
                available,
                fastpq_prover::goldilocks_transform::available_goldilocks_transform_backend_v1(),
            )
        );
        for backend in [None, Some(Backend::Cpu), Some(Backend::Cuda)] {
            assert_eq!(
                select_device_columns_v1(layout.common_lde_size(), usize::MAX, backend),
                0
            );
        }
        assert_eq!(
            select_device_columns_v1(layout.common_lde_size(), 0, Some(Backend::Metal)),
            0
        );
        assert_eq!(
            select_device_columns_v1(layout.common_lde_size(), available, Some(Backend::Metal),),
            8
        );
        assert_eq!(evaluator.receipt_v1(), &MainTransformReceiptV1::default());
    }

    fn columns_v1(count: usize) -> Vec<Column> {
        (0..count)
            .map(|column| {
                Column::from_vec_v1(
                    (0..21)
                        .map(|degree| F((column * 19 + degree * 7 + 1) as u64))
                        .collect(),
                )
            })
            .collect()
    }

    #[test]
    fn device_allowance_preserves_cpu_admission_and_all_reserves() {
        let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
        let plan = main_resources::MainProverBufferPlanV1::new_v1(&layout).unwrap();
        let cap = usize::try_from(
            super::super::super::super::profile::ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1,
        )
        .expect("fixture value fits usize");
        let baseline = plan.check_before_sources_v1(0).unwrap();
        let assembly_limit = cap - baseline;
        // The 12 GiB ceiling still reserves 6 GiB of native sources, 1 GiB
        // of source scratch and 1 GiB of runtime beyond the live transforms.
        assert_eq!(
            assembly_limit,
            (12_usize << 30)
                - main_resources::CURRENT_FULL_PROFILE_PEAK_V1
                - (8 << 30)
                - main_bounded_transform::SHARED_POWERS_ALLOWANCE_V1
                - PUBLIC_COSET_POWERS_ALLOWANCE_V1
        );
        assert_eq!(
            assembly_limit,
            (4_usize << 30)
                - main_resources::CURRENT_FULL_PROFILE_PEAK_V1
                - main_bounded_transform::SHARED_POWERS_ALLOWANCE_V1
                - PUBLIC_COSET_POWERS_ALLOWANCE_V1
        );
        let joined_available = cap - plan.joined_transform_admission_v1(288_345_698).unwrap();
        assert!(joined_available > assembly_limit - 288_345_698);
        assert_eq!(
            select_metal_columns_v1(layout.common_lde_size(), joined_available),
            8
        );
        for count in [1, 2, 4, 8] {
            let required =
                metal_goldilocks_transform_extra_payload_v1(layout.common_lde_size(), count)
                    .unwrap();
            assert_eq!(
                select_metal_columns_v1(layout.common_lde_size(), required),
                count
            );
            assert!(select_metal_columns_v1(layout.common_lde_size(), required - 1) < count);
        }
        assert_eq!(select_metal_columns_v1(layout.common_lde_size(), 0), 0);
        assert!(plan.check_before_sources_v1(assembly_limit).is_ok());
        assert!(plan.check_before_sources_v1(assembly_limit + 1).is_err());
    }

    #[test]
    fn bounded_packing_subdispatch_and_conversion_match_independent_coset_values() {
        for count in [1, 3, 8] {
            for device_columns in [1, 2, 4, 8] {
                let columns = columns_v1(count);
                let mut evaluator = MainTraceCosetEvaluatorV1 {
                    evaluation_rows: 64,
                    device_columns,
                    receipt: MainTransformReceiptV1::default(),
                    selected_scratch: selected_coset::SelectedScratchV1::default(),
                    coset_powers: Some(PublicCosetPowersV1::new_v1(64).unwrap()),
                };
                let mut original_allocations = Vec::new();
                let (output, erased) = inspection::observe_v1(|| {
                    evaluator
                        .evaluate_with_v1(&columns, 4, 6, |batch, root| {
                            assert!(batch.len() <= device_columns);
                            original_allocations
                                .extend(batch.iter().map(|column| column.as_ptr().cast::<F>()));
                            transform_goldilocks_columns_v1(
                                batch,
                                root,
                                Direction::Forward,
                                fastpq_prover::ExecutionMode::Cpu,
                            )
                            .map_err(|_| AggregateStarkErrorV1::InvalidLayout)
                        })
                        .unwrap()
                });
                let root = goldilocks_primitive_root_v1(6).unwrap();
                assert_eq!(
                    output
                        .iter()
                        .map(|column| column.as_ptr())
                        .collect::<Vec<_>>(),
                    original_allocations
                );
                let ((), transferred_erased) = aggregate::field_column_erasure::observe_v1(|| {
                    for (coefficients, values) in columns.iter().zip(output) {
                        for (index, value) in values.iter().enumerate() {
                            let point = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(index as u128));
                            let expected = coefficients
                                .iter()
                                .rev()
                                .fold(F::ZERO, |sum, value| sum.mul(point).add(*value));
                            assert_eq!(*value, expected);
                        }
                    }
                });
                assert_eq!(evaluator.receipt.cpu_columns, count);
                assert_eq!(evaluator.receipt.metal_columns, 0);
                assert_eq!(
                    transferred_erased
                        .iter()
                        .map(|entry| entry.cells)
                        .sum::<usize>(),
                    count * 64
                );
                assert_eq!(erased.iter().map(|entry| entry.cells).sum::<usize>(), 0);
                assert!(erased.iter().all(|entry| entry.nonzero_after == 0));
                assert!(
                    transferred_erased
                        .iter()
                        .all(|entry| entry.nonzero_after == 0)
                );
            }
        }
    }

    #[test]
    fn failed_and_unwinding_dispatch_erase_the_actual_word_allocations() {
        let columns = columns_v1(3);
        for unwind in [false, true] {
            let ((), observations) = inspection::observe_v1(|| {
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut evaluator = MainTraceCosetEvaluatorV1 {
                        evaluation_rows: 64,
                        device_columns: 2,
                        receipt: MainTransformReceiptV1::default(),
                        selected_scratch: selected_coset::SelectedScratchV1::default(),
                        coset_powers: Some(PublicCosetPowersV1::new_v1(64).unwrap()),
                    };
                    evaluator.evaluate_with_v1(&columns, 4, 6, |batch, _| {
                        batch[0][0] = 999;
                        assert!(!unwind, "injected dispatch unwind");
                        Err(AggregateStarkErrorV1::AllocationFailure)
                    })
                }));
                assert!(if unwind {
                    outcome.is_err()
                } else {
                    outcome.unwrap().is_err()
                });
            });
            assert_eq!(
                observations.iter().map(|entry| entry.cells).sum::<usize>(),
                3 * 64
            );
            assert!(
                observations
                    .iter()
                    .map(|entry| entry.nonzero_before)
                    .sum::<usize>()
                    > 0
            );
            assert!(observations.iter().all(|entry| entry.nonzero_after == 0));
        }
    }
    #[test]
    fn malformed_inputs_and_device_outputs_fail_before_publication() {
        use crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1;
        let mut evaluator = MainTraceCosetEvaluatorV1 {
            evaluation_rows: 64,
            device_columns: 2,
            receipt: MainTransformReceiptV1::default(),
            selected_scratch: selected_coset::SelectedScratchV1::default(),
            coset_powers: Some(PublicCosetPowersV1::new_v1(64).unwrap()),
        };
        let mut columns = columns_v1(3);
        let mut calls = 0;
        assert!(
            evaluator
                .evaluate_with_v1(&columns, 4, 7, |_, _| {
                    calls += 1;
                    unreachable!()
                })
                .is_err()
        );
        columns[2] = Column::from_vec_v1(vec![F(GOLDILOCKS_MODULUS_V1)]);
        assert!(
            evaluator
                .evaluate_with_v1(&columns, 4, 6, |_, _| {
                    calls += 1;
                    unreachable!()
                })
                .is_err()
        );
        assert_eq!(calls, 0);
        let columns = columns_v1(3);
        for invalid_column in 0..columns.len() {
            let mut first_column = 0;
            let ((result, cleared), transferred_cleared) =
                aggregate::field_column_erasure::observe_v1(|| {
                    inspection::observe_v1(|| {
                        evaluator.evaluate_with_v1(&columns, 4, 6, |batch, _| {
                            if (first_column..first_column + batch.len()).contains(&invalid_column)
                            {
                                batch[invalid_column - first_column][1] = GOLDILOCKS_MODULUS_V1;
                            }
                            first_column += batch.len();
                            Ok(Backend::Metal)
                        })
                    })
                });
            assert!(matches!(
                result,
                Err(AggregateStarkErrorV1::NonCanonicalField)
            ));
            // All three raw output columns are cleared at their actual ownership
            // boundaries; canonical validation precedes field-value publication.
            assert_eq!(
                cleared.iter().map(|entry| entry.cells).sum::<usize>()
                    + transferred_cleared
                        .iter()
                        .map(|entry| entry.cells)
                        .sum::<usize>(),
                3 * 64
            );
            assert!(cleared.iter().all(|entry| entry.nonzero_after == 0));
            assert!(
                transferred_cleared
                    .iter()
                    .all(|entry| entry.nonzero_after == 0)
            );
        }
    }
    #[test]
    fn selected_cpu_and_device_adapter_keep_original_row_coordinates_and_fault_checks() {
        use crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1;
        for width in [1, 3, 8] {
            let columns = columns_v1(width);
            for selected in [vec![0], vec![63], vec![0, 7, 16, 63], (0..64).collect()] {
                let expected = columns
                    .iter()
                    .map(|column| masked_trace_coefficients_on_coset_v1(column, 4, 6).unwrap())
                    .collect::<Vec<_>>();
                for device_columns in [0, 1, 2, 4] {
                    let mut evaluator = MainTraceCosetEvaluatorV1 {
                        evaluation_rows: 64,
                        device_columns,
                        receipt: MainTransformReceiptV1::default(),
                        selected_scratch: selected_coset::SelectedScratchV1::default(),
                        coset_powers: (device_columns != 0)
                            .then(|| PublicCosetPowersV1::new_v1(64).unwrap()),
                    };
                    let actual = evaluator
                        .evaluate_selected_with_v1(
                            &columns,
                            4,
                            6,
                            Some(&selected),
                            |batch, root| {
                                transform_goldilocks_columns_v1(
                                    batch,
                                    root,
                                    Direction::Forward,
                                    fastpq_prover::ExecutionMode::Cpu,
                                )
                                .map_err(|_| AggregateStarkErrorV1::InvalidLayout)
                            },
                        )
                        .unwrap();
                    for (column, full) in actual.iter().zip(&expected) {
                        assert_eq!(column.len(), selected.len());
                        for (&row, &value) in selected.iter().zip(column.iter()) {
                            assert_eq!(value, full[row]);
                        }
                    }
                    assert_eq!(evaluator.receipt.cpu_columns, width);
                }
            }
        }
        let columns = columns_v1(1);
        let mut evaluator = MainTraceCosetEvaluatorV1 {
            evaluation_rows: 64,
            device_columns: 1,
            receipt: MainTransformReceiptV1::default(),
            selected_scratch: selected_coset::SelectedScratchV1::default(),
            coset_powers: Some(PublicCosetPowersV1::new_v1(64).unwrap()),
        };
        let result = evaluator.evaluate_selected_with_v1(&columns, 4, 6, Some(&[0]), |batch, _| {
            batch[0][63] = GOLDILOCKS_MODULUS_V1;
            Ok(Backend::Metal)
        });
        assert!(
            matches!(result, Err(AggregateStarkErrorV1::NonCanonicalField)),
            "unselected device corruption must remain visible"
        );
        for selected in [vec![], vec![64], vec![2, 1], vec![1, 1]] {
            assert!(
                evaluator
                    .evaluate_selected_with_v1(&columns, 4, 6, Some(&selected), |_, _| panic!(
                        "invalid selected geometry dispatched"
                    ))
                    .is_err()
            );
        }
    }

    #[test]
    fn selected_low_degree_dispatch_avoids_device_staging_and_counts_cpu_columns() {
        let rows = 1 << 22;
        let selected = [0, rows / 3, rows - 1];
        for native in [5, 8] {
            let mut input = vec![F::ZERO; (1usize << native) + MASK_DEGREE + 1];
            input[0] = F(19);
            *input.last_mut().unwrap() = F::ONE;
            let expected = selected_coset::evaluate_v1(&input, native, 22, &selected).unwrap();
            for workers in [1, 4] {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(workers)
                    .build()
                    .unwrap();
                pool.install(|| {
                    for width in [1, 3, 8] {
                        let columns = (0..width)
                            .map(|_| Column::from_vec_v1(input.clone()))
                            .collect::<Vec<_>>();
                        for device_columns in [0, 4] {
                            let mut evaluator = MainTraceCosetEvaluatorV1 {
                                evaluation_rows: rows,
                                device_columns,
                                receipt: MainTransformReceiptV1::default(),
                                // Any attempt to enter device staging fails even
                                // before the injected dispatch can be reached.
                                selected_scratch: selected_coset::SelectedScratchV1::default(),
                                coset_powers: None,
                            };
                            let actual = evaluator
                                .evaluate_selected_with_v1(
                                    &columns,
                                    native,
                                    22,
                                    Some(&selected),
                                    |_, _| {
                                        panic!("low-degree selected replay reached device staging")
                                    },
                                )
                                .unwrap();
                            assert_eq!(actual.len(), width);
                            for output in actual {
                                assert_eq!(output.as_ref(), expected.as_ref());
                            }
                            assert_eq!(evaluator.receipt.cpu_columns, width);
                            assert_eq!(evaluator.receipt.metal_columns, 0);
                        }
                    }
                });
            }
        }
    }

    #[test]
    fn selected_eight_column_dispatch_keeps_complete_mask_tail_and_erases_fft_scratch() {
        let rows = 1usize << 22;
        let selected = [0, rows / 3, rows - 1];
        let root = goldilocks_primitive_root_v1(22).unwrap();
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap();
        for native in [15, 16, 18, 19] {
            let length = (1usize << native) + MASK_DEGREE + 1;
            let columns = (0..8)
                .map(|column| {
                    let mut input = vec![F::ZERO; length];
                    input[0] = F(19 + column);
                    input[length - 1] = F::ONE;
                    Column::from_vec_v1(input)
                })
                .collect::<Vec<_>>();
            pool.install(|| {
                let mut evaluator = MainTraceCosetEvaluatorV1 {
                    evaluation_rows: rows,
                    device_columns: 4,
                    receipt: MainTransformReceiptV1::default(),
                    selected_scratch: selected_coset::SelectedScratchV1::default(),
                    coset_powers: None,
                };
                let ((actual, erased), batched_erased) =
                    selected_coset::observe_batch_clear_v1(|| {
                        inspection::observe_v1(|| {
                            let actual = evaluator
                                .evaluate_selected_with_v1(
                                    &columns,
                                    native,
                                    22,
                                    Some(&selected),
                                    |_, _| panic!("measured selected batch reached device staging"),
                                )
                                .unwrap();
                            assert_eq!(evaluator.receipt.cpu_columns, 8);
                            assert_eq!(evaluator.receipt.metal_columns, 0);
                            assert!(evaluator.selected_scratch.retained_identity_v1().is_some());
                            drop(evaluator);
                            actual
                        })
                    });
                for (column, output) in actual.iter().enumerate() {
                    for (&row, &value) in selected.iter().zip(output.iter()) {
                        let point = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(row as u128));
                        assert_eq!(
                            value,
                            F(19 + column as u64).add(point.pow((length - 1) as u128))
                        );
                    }
                }
                assert_eq!(
                    batched_erased
                        .iter()
                        .map(|entry| entry.cells)
                        .sum::<usize>(),
                    8 * rows
                );
                assert_eq!(batched_erased.len(), 1, "one interleaved scratch owner");
                assert!(batched_erased.iter().all(|entry| entry.nonzero_after == 0));
                assert_eq!(erased.iter().map(|entry| entry.cells).sum::<usize>(), 0);
                assert!(erased.iter().all(|entry| entry.nonzero_after == 0));
                let ((), compact_erased) =
                    aggregate::field_column_erasure::observe_v1(|| drop(actual));
                assert_eq!(compact_erased.len(), 8, "all returned column owners clear");
                assert_eq!(
                    compact_erased
                        .iter()
                        .map(|entry| entry.cells)
                        .sum::<usize>(),
                    8 * selected.len()
                );
                assert!(compact_erased.iter().all(|entry| entry.nonzero_before != 0));
                assert!(compact_erased.iter().all(|entry| entry.nonzero_after == 0));
            });
        }
    }

    #[test]
    fn selected_eight_column_dispatch_validates_and_refuses_unmeasured_shapes_before_allocation() {
        use crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1;
        let rows = 1usize << 22;
        let mut evaluator = MainTraceCosetEvaluatorV1 {
            evaluation_rows: rows,
            device_columns: 4,
            receipt: MainTransformReceiptV1::default(),
            selected_scratch: selected_coset::SelectedScratchV1::default(),
            coset_powers: None,
        };
        for failure in 0..8 {
            let mut columns = (0..8)
                .map(|_| Column::from_vec_v1(vec![F::ONE]))
                .collect::<Vec<_>>();
            let mut selected = vec![0];
            let mut selection = true;
            match failure {
                0 => {
                    columns[7] = Column::from_vec_v1(vec![F(GOLDILOCKS_MODULUS_V1)]);
                }
                1 => {
                    selected.clear();
                }
                2 => {
                    selected = vec![1, 1];
                }
                3 => {
                    selected = vec![rows];
                }
                4 => {
                    columns.pop();
                }
                5 => {
                    selected = (0..273).collect();
                }
                6 => {
                    selection = false;
                }
                _ => {
                    columns[7] = Column::from_vec_v1(vec![F::ONE; (1 << 19) + MASK_DEGREE + 2]);
                }
            }
            let ((result, erased), batched_erased) = selected_coset::observe_batch_clear_v1(|| {
                inspection::observe_v1(|| {
                    evaluator.evaluate_selected_with_v1(
                        &columns,
                        19,
                        22,
                        selection.then_some(selected.as_slice()),
                        |_, _| panic!("invalid or missing power owner dispatched"),
                    )
                })
            });
            assert!(matches!(result, Err(error) if error == if failure == 0 {
                AggregateStarkErrorV1::NonCanonicalField
            } else {
                AggregateStarkErrorV1::InvalidLayout
            }));
            assert!(erased.is_empty());
            assert!(batched_erased.is_empty());
            assert_eq!(evaluator.receipt, MainTransformReceiptV1::default());
        }
    }

    #[test]
    fn selected_horner_dispatch_retains_validation_before_allocation_and_counters() {
        use crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1;
        let rows = 1 << 22;
        let mut evaluator = MainTraceCosetEvaluatorV1 {
            evaluation_rows: rows,
            device_columns: 4,
            receipt: MainTransformReceiptV1::default(),
            selected_scratch: selected_coset::SelectedScratchV1::default(),
            coset_powers: None,
        };
        let valid = vec![F::ONE; (1 << 5) + MASK_DEGREE + 1];
        for (input, selected, expected) in [
            (valid.clone(), vec![], AggregateStarkErrorV1::InvalidLayout),
            (
                valid.clone(),
                vec![rows],
                AggregateStarkErrorV1::InvalidLayout,
            ),
            (
                valid.clone(),
                vec![2, 1],
                AggregateStarkErrorV1::InvalidLayout,
            ),
            (
                valid.clone(),
                vec![1, 1],
                AggregateStarkErrorV1::InvalidLayout,
            ),
            (vec![], vec![0], AggregateStarkErrorV1::InvalidLayout),
            (
                {
                    let mut invalid = valid.clone();
                    *invalid.last_mut().unwrap() = F(GOLDILOCKS_MODULUS_V1);
                    invalid
                },
                vec![0],
                AggregateStarkErrorV1::NonCanonicalField,
            ),
        ] {
            let columns = [Column::from_vec_v1(input)];
            let (result, erased) = inspection::observe_v1(|| {
                evaluator.evaluate_selected_with_v1(&columns, 5, 22, Some(&selected), |_, _| {
                    panic!("invalid selected input reached device dispatch")
                })
            });
            assert!(matches!(result, Err(error) if error == expected));
            assert!(erased.is_empty());
            assert_eq!(evaluator.receipt, MainTransformReceiptV1::default());
        }
        // Public shapes just outside the measured envelope retain the original
        // device route, which requires its public power owner before staging.
        for (input, selected) in [
            (vec![F::ONE; valid.len() + 1], Some(vec![0])),
            (valid.clone(), Some((0..2177).collect())),
            (valid, None),
        ] {
            let columns = [Column::from_vec_v1(input)];
            assert!(matches!(
                evaluator.evaluate_selected_with_v1(
                    &columns,
                    5,
                    22,
                    selected.as_deref(),
                    |_, _| { panic!("missing device power owner must be refused") }
                ),
                Err(AggregateStarkErrorV1::InvalidLayout)
            ));
        }
    }

    #[test]
    #[ignore = "requires actual Metal common22 selected-row parity; component evidence only"]
    fn selected_required_metal_matches_pruned_cpu_and_original_full_coset() {
        use crate::privacy_engines::transparent_stark::masked_trace_coefficients_with_mask_v1;
        assert_eq!(
            fastpq_prover::goldilocks_transform::available_goldilocks_transform_backend_v1(),
            Some(Backend::Metal)
        );
        let common = 22;
        let rows = 1usize << common;
        let selected = (0..136)
            .flat_map(|query| {
                let block = (query * 1_729 + 17) % (rows / 16);
                block * 16..block * 16 + 16
            })
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        for native in [5, 19] {
            for width in [1, 8] {
                let columns = (0..width)
                    .map(|column| {
                        let values = (0..1usize << native)
                            .map(|row| F((row * 17 + column * 13 + 1) as u64))
                            .collect::<Vec<_>>();
                        let mask = (0..1816)
                            .map(|degree| F((degree * 29 + column * 7 + 3) as u64))
                            .collect::<Vec<_>>();
                        Column::from_vec_v1(
                            masked_trace_coefficients_with_mask_v1(&values, native, &mask).unwrap(),
                        )
                    })
                    .collect::<Vec<_>>();
                let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
                let mut metal = MainTraceCosetEvaluatorV1::new_v1(&layout, 0).unwrap();
                assert!(metal.device_columns > 0);
                let actual = metal
                    .evaluate_selected_v1(&columns, native, common, &selected)
                    .unwrap();
                assert_eq!(
                    metal.receipt.metal_columns,
                    if native == 19 { width } else { 0 }
                );
                assert_eq!(
                    metal.receipt.cpu_columns,
                    if native == 5 { width } else { 0 }
                );
                let mut cpu = MainTraceCosetEvaluatorV1 {
                    evaluation_rows: rows,
                    device_columns: 0,
                    receipt: MainTransformReceiptV1::default(),
                    selected_scratch: selected_coset::SelectedScratchV1::default(),
                    coset_powers: None,
                };
                let pruned = cpu
                    .evaluate_selected_v1(&columns, native, common, &selected)
                    .unwrap();
                for ((column, actual), pruned) in
                    columns.iter().zip(actual.iter()).zip(pruned.iter())
                {
                    let full = Column::from_vec_v1(
                        masked_trace_coefficients_on_coset_v1(column, native, common).unwrap(),
                    );
                    for ((&row, &actual), &pruned) in
                        selected.iter().zip(actual.iter()).zip(pruned.iter())
                    {
                        assert_eq!(actual, full[row]);
                        assert_eq!(pruned, full[row]);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "main_common22_coarse_diagnostic_tests.rs"]
mod common22_coarse_diagnostic_tests;

#[cfg(test)]
#[path = "main_full_dispatch_tests.rs"]
mod full_dispatch_tests;

#[cfg(test)]
#[path = "main_public_coset_powers_tests.rs"]
mod public_coset_powers_tests;

#[cfg(test)]
#[path = "main_selected_owner_tests.rs"]
mod selected_owner_tests;

#[cfg(test)]
#[path = "main_initial_readonly_tests.rs"]
mod initial_readonly_tests;
