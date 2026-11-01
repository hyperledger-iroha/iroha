//! Admitted staging for fixed, private quotient and native replay transforms.
//!
//! Existing F matrices keep their allocations. Prepared staging and the complete
//! Metal allowance fit outside all arithmetic, source, replay/cache,
//! scratch and runtime reservations. Private coefficient folding precedes the
//! prepared forward adapter; native replay publishes its prepared inverse directly
//! into existing field slices before masking. Only forward publication may group
//! sixteen already allocated columns under its separate full payload charge.
//! Fixed transitions use the same prepared forward limit; their recovery inverses
//! and native replay retain the original eight-column policy.

use super::super::super::private_table::{PrivateTableV1, zeroize_words_v1};
use super::*;
use fastpq_prover::goldilocks_transform::{
    GoldilocksTransformBackendV1 as Backend, GoldilocksTransformDirectionV1 as Direction,
    GoldilocksTransformErrorV1 as TransformError, metal_goldilocks_transform_extra_payload_v1,
};

type Words = PrivateTableV1<Vec<u64>>;
type PublicPowers = crate::privacy_engines::transparent_stark::GoldilocksFftPowersV1;

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
    public_powers_reserved: bool,
}

// Two public max-stripe tables can coexist during a fixed inverse/forward
// transition. This fixed allowance is reserved before private transforms; it
// cannot be spent again by device staging or additional caller owners.
pub(super) const SHARED_POWERS_ALLOWANCE_V1: usize = 2
    * ((1usize << main_quotient_stripes::MAIN_QUOTIENT_STRIPE_LOG2_V1) / 2
        * core::mem::size_of::<F>()
        + core::mem::size_of::<PublicPowers>());
// The measured registered log5 domain regressed for both widths. Retain the
// original arithmetic there; the next registered native domain is public log8.
const SHARED_POWERS_MIN_ROWS_V1: usize = 1 << 8;
// Independent public row tiles expose parallelism beyond the eight-column cap.
// No second matrix, work queue of private values, or parallel reduction exists.
const HOST_TILE_ROWS_V1: usize = 1 << 14;
const HOST_PARALLEL_ROWS_V1: usize = 1 << 16;

// Only an isolated diagnostic can select the original serial scan/copy and
// column-parallel scaling. The ordinary test and production route is unchanged.
#[cfg(test)]
thread_local! {
    static HOST_TILES_OVERRIDE_V1: core::cell::Cell<Option<bool>> = const {
        core::cell::Cell::new(None)
    };
    static FORWARD_GROUP_LIMIT_OVERRIDE_V1: core::cell::Cell<Option<usize>> = const {
        core::cell::Cell::new(None)
    };
}

#[cfg(test)]
pub(super) fn with_forward_group_limit_for_testing_v1<T>(
    limit: usize,
    operation: impl FnOnce() -> T,
) -> T {
    assert!([8, 16].contains(&limit));
    struct Restore(Option<usize>);
    impl Drop for Restore {
        fn drop(&mut self) {
            FORWARD_GROUP_LIMIT_OVERRIDE_V1.with(|value| value.set(self.0));
        }
    }
    let _restore =
        Restore(FORWARD_GROUP_LIMIT_OVERRIDE_V1.with(|value| value.replace(Some(limit))));
    operation()
}

fn forward_group_limit_v1() -> usize {
    #[cfg(test)]
    if let Some(limit) = FORWARD_GROUP_LIMIT_OVERRIDE_V1.with(|value| value.get()) {
        return limit;
    }
    // Complete sixteen-column forward cycles passed parity and beat two
    // eight-column dispatches. Admission still charges their full shared
    // backing and metadata; source replay and native inverse widths stay eight.
    fastpq_prover::goldilocks_transform::MAX_GOLDILOCKS_PREPARED_FORWARD_COLUMNS_V1
}

#[cfg(test)]
fn with_host_tiles_for_testing_v1<T>(enabled: bool, operation: impl FnOnce() -> T) -> T {
    struct Restore(Option<bool>);
    impl Drop for Restore {
        fn drop(&mut self) {
            HOST_TILES_OVERRIDE_V1.with(|value| value.set(self.0));
        }
    }
    let _restore = Restore(HOST_TILES_OVERRIDE_V1.with(|value| value.replace(Some(enabled))));
    operation()
}

fn use_host_tiles_v1(rows: usize) -> bool {
    #[cfg(test)]
    if HOST_TILES_OVERRIDE_V1.with(|value| value.get()) == Some(false) {
        return false;
    }
    rows >= HOST_PARALLEL_ROWS_V1 && rayon::current_num_threads() > 1
}

fn canonical_field_columns_v1(columns: &[Vec<F>]) -> bool {
    if use_host_tiles_v1(columns.first().map_or(0, Vec::len)) {
        columns.par_iter().all(|column| {
            column
                .par_chunks(HOST_TILE_ROWS_V1)
                .all(|tile| tile.iter().all(|value| F::canonical(value.0).is_some()))
        })
    } else {
        columns
            .iter()
            .flatten()
            .all(|value| F::canonical(value.0).is_some())
    }
}

fn canonical_word_columns_v1(columns: &[Vec<u64>]) -> bool {
    if use_host_tiles_v1(columns.first().map_or(0, Vec::len)) {
        columns.par_iter().all(|column| {
            column
                .par_chunks(HOST_TILE_ROWS_V1)
                .all(|tile| tile.iter().all(|&word| F::canonical(word).is_some()))
        })
    } else {
        columns
            .iter()
            .flatten()
            .all(|&word| F::canonical(word).is_some())
    }
}

fn copy_fields_to_words_v1(words: &mut [Vec<u64>], fields: &[Vec<F>]) {
    let copy = |target: &mut [u64], source: &[F]| {
        for (target, value) in target.iter_mut().zip(source) {
            *target = value.0;
        }
    };
    if use_host_tiles_v1(words.first().map_or(0, Vec::len)) {
        words
            .par_iter_mut()
            .zip(fields.par_iter())
            .for_each(|(target, source)| {
                target
                    .par_chunks_mut(HOST_TILE_ROWS_V1)
                    .zip(source.par_chunks(HOST_TILE_ROWS_V1))
                    .for_each(|(target, source)| copy(target, source));
            });
    } else {
        for (target, source) in words.iter_mut().zip(fields) {
            copy(target, source);
        }
    }
}

fn copy_words_to_fields_v1(fields: &mut [Vec<F>], words: &[Vec<u64>]) {
    let copy = |target: &mut [F], source: &[u64]| {
        for (target, &word) in target.iter_mut().zip(source) {
            *target = F(word);
        }
    };
    if use_host_tiles_v1(words.first().map_or(0, Vec::len)) {
        fields
            .par_iter_mut()
            .zip(words.par_iter())
            .for_each(|(target, source)| {
                target
                    .par_chunks_mut(HOST_TILE_ROWS_V1)
                    .zip(source.par_chunks(HOST_TILE_ROWS_V1))
                    .for_each(|(target, source)| copy(target, source));
            });
    } else {
        for (target, source) in fields.iter_mut().zip(words) {
            copy(target, source);
        }
    }
}

fn scale_word_columns_v1(words: &mut [Vec<u64>], diagonal: F) {
    if diagonal == F::ONE {
        return;
    }
    let scale = |first: usize, values: &mut [u64]| {
        let mut power = diagonal.pow(first as u128);
        for word in values {
            *word = F(*word).mul(power).0;
            power = power.mul(diagonal);
        }
    };
    if use_host_tiles_v1(words.first().map_or(0, Vec::len)) {
        words.par_iter_mut().for_each(|column| {
            column
                .par_chunks_mut(HOST_TILE_ROWS_V1)
                .enumerate()
                .for_each(|(tile, values)| scale(tile * HOST_TILE_ROWS_V1, values));
        });
    } else {
        words.par_iter_mut().for_each(|column| scale(0, column));
    }
}

/// Scale disjoint field tiles in their existing allocations. Each tile starts
/// at the exact public exponent, so worker scheduling cannot change a value.
fn scale_field_columns_v1(columns: &mut [Vec<F>], diagonal: F) {
    if diagonal == F::ONE {
        return;
    }
    let scale = |first: usize, values: &mut [F]| {
        let mut power = diagonal.pow(first as u128);
        for value in values {
            *value = value.mul(power);
            power = power.mul(diagonal);
        }
    };
    if use_host_tiles_v1(columns.first().map_or(0, Vec::len)) {
        columns.par_iter_mut().for_each(|column| {
            column
                .par_chunks_mut(HOST_TILE_ROWS_V1)
                .enumerate()
                .for_each(|(tile, values)| scale(tile * HOST_TILE_ROWS_V1, values));
        });
    } else {
        columns.par_iter_mut().for_each(|column| scale(0, column));
    }
}

impl MainBoundedTransformPolicyV1 {
    pub(super) const fn cpu_v1() -> Self {
        Self {
            available: 0,
            backend: None,
            public_powers_reserved: false,
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
                .and_then(|available| {
                    available.checked_sub(
                        super::main_terminal_links::MainTerminalLinkPlanV1::public_owner_charge_v1(
                        ),
                    )
                })
                .and_then(|available| {
                    available.checked_sub(
                        super::main_key_joins::MainKeyJoinPlanV1::public_owner_charge_v1(),
                    )
                })
                .and_then(|available| {
                    available.checked_sub(
                        super::main_sha_union::MainShaUnionPlanV1::public_owner_charge_v1(),
                    )
                })
                .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?,
            backend: fastpq_prover::goldilocks_transform::available_goldilocks_transform_backend_v1(
            ),
            // check_before_sources_v1 already included the complete public
            // allowance; do not subtract it a second time from device slack.
            public_powers_reserved: true,
        })
    }

    /// Reserve additional caller-owned private buffers before any allocation or dispatch.
    pub(super) fn reserve_additional_v1(self, payload: usize) -> Result<Self, ZkX509StarkErrorV1> {
        Ok(Self {
            available: self
                .available
                .checked_sub(payload)
                .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?,
            ..self
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
            .and_then(|n| n.checked_add(super::main_quotient_cache::GROUPING_METADATA_BYTES_V1))
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
            public_powers_reserved: false,
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

    /// Wider forward staging spends only the existing public residual allowance.
    /// Native source batches and ordinary/inverse transform limits remain eight.
    pub(super) fn forward_columns_v1(self, rows: usize) -> usize {
        if self.backend != Some(Backend::Metal) {
            return 0;
        }
        [16, 8, 4, 2, 1]
            .into_iter()
            .filter(|&columns| columns <= forward_group_limit_v1())
            .find(|&columns| {
                super::main_native_transform::required_forward_payload_v1(rows, columns)
                    .is_ok_and(|bytes| bytes <= self.available)
            })
            .unwrap_or(0)
    }

    #[cfg(test)]
    pub(super) fn for_forward_test_v1(rows: usize, columns: usize) -> Self {
        Self {
            available: super::main_native_transform::required_forward_payload_v1(rows, columns)
                .unwrap(),
            backend: Some(Backend::Metal),
            public_powers_reserved: false,
        }
    }

    /// Use the separately reserved public allowance, never private residual slack.
    /// Explicit reference policies and tiny public domains use the original FFT.
    pub(super) fn cpu_powers_v1(
        self,
        rows: usize,
        root: F,
    ) -> Result<Option<PublicPowers>, ZkX509StarkErrorV1> {
        PublicPowers::required_payload_bytes_v1(rows).map_err(map_transparent_error_v1)?;
        if rows > 1usize << main_quotient_stripes::MAIN_QUOTIENT_STRIPE_LOG2_V1 {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        if !self.public_powers_reserved || rows < SHARED_POWERS_MIN_ROWS_V1 {
            return Ok(None);
        }
        PublicPowers::new_v1(rows, root, SHARED_POWERS_ALLOWANCE_V1)
            .map(Some)
            .map_err(map_transparent_error_v1)
    }

    /// One fixed public allowance owns the complete simultaneous pair. Actual
    /// forward capacity is deducted before inverse allocation; excess is an
    /// error, never a witness-dependent fallback or a second use of the budget.
    pub(super) fn cpu_power_pair_v1(
        self,
        rows: usize,
        root: F,
        inverse: bool,
    ) -> Result<(Option<PublicPowers>, Option<PublicPowers>), ZkX509StarkErrorV1> {
        if !inverse {
            return Ok((self.cpu_powers_v1(rows, root)?, None));
        }
        let minimum =
            PublicPowers::required_payload_bytes_v1(rows).map_err(map_transparent_error_v1)?;
        if rows > 1usize << main_quotient_stripes::MAIN_QUOTIENT_STRIPE_LOG2_V1 {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        if !self.public_powers_reserved || rows < SHARED_POWERS_MIN_ROWS_V1 {
            return Ok((None, None));
        }
        let forward_budget = SHARED_POWERS_ALLOWANCE_V1
            .checked_sub(minimum)
            .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?;
        let forward =
            PublicPowers::new_v1(rows, root, forward_budget).map_err(map_transparent_error_v1)?;
        let inverse_budget = SHARED_POWERS_ALLOWANCE_V1
            .checked_sub(
                forward
                    .allocated_payload_bytes_v1()
                    .map_err(map_transparent_error_v1)?,
            )
            .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?;
        let inverse = PublicPowers::new_v1(
            rows,
            root.inv().ok_or(ZkX509StarkErrorV1::ProfileMismatch)?,
            inverse_budget,
        )
        .map_err(map_transparent_error_v1)?;
        Ok((Some(forward), Some(inverse)))
    }

    /// Preserve the original word transition only as a test oracle/fault seam.
    #[cfg(test)]
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

    /// Transform the unpublished fixed owner without a second word matrix.
    /// Recovery retains its eight-column limit; the final forward group may
    /// contain sixteen already charged output columns. Each prepared token is
    /// consumed and erased before the next stage, so staging allowances do not
    /// overlap. A failed stage is terminal for the enclosing fixed owner.
    pub(super) fn apply_exact_v1(
        self,
        columns: &mut [Vec<F>],
        root: F,
        diagonal: F,
        recovery: bool,
        mut uncertain: impl FnMut() -> bool,
    ) -> Result<(), ZkX509StarkErrorV1> {
        check_completion_v1(uncertain())?;
        validate_private_columns_with_limit_v1(
            columns,
            root,
            fastpq_prover::goldilocks_transform::MAX_GOLDILOCKS_PREPARED_FORWARD_COLUMNS_V1,
        )?;
        if F::canonical(diagonal.0).is_none() || diagonal == F::ZERO {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        if recovery {
            for batch in columns.chunks_mut(aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1) {
                self.exact_for_use_v1(
                    batch,
                    root,
                    Direction::Inverse,
                    TransformUseV1::Fixed,
                    &mut uncertain,
                )?;
            }
        }
        check_completion_v1(uncertain())?;
        scale_field_columns_v1(columns, diagonal);
        self.exact_for_use_v1(
            columns,
            root,
            Direction::Forward,
            TransformUseV1::Fixed,
            &mut uncertain,
        )?;
        check_completion_v1(uncertain())
    }

    /// Complete forward transforms of already shifted/folded private columns.
    /// CPU fallback is deterministic and may not bypass quarantined completion.
    #[cfg(test)]
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
    #[cfg(test)]
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

    /// Native interpolation uses one closed production transaction. Existing
    /// word callbacks are available only through the test injection adapter.
    pub(super) fn inverse_native_with_v1(
        self,
        columns: &mut [Vec<F>],
        root: F,
        transform: &mut impl super::main_native_transform::MainNativeTransformV1,
        mut uncertain: impl FnMut() -> bool,
    ) -> Result<(), ZkX509StarkErrorV1> {
        transform.inverse_v1(self, columns, root, &mut uncertain)
    }

    pub(super) fn inverse_native_exact_v1(
        self,
        columns: &mut [Vec<F>],
        root: F,
        uncertain: impl FnMut() -> bool,
    ) -> Result<(), ZkX509StarkErrorV1> {
        self.private_exact_v1(columns, root, Direction::Inverse, uncertain)
    }

    /// Publish a complete forward group directly into its final field allocations.
    pub(super) fn forward_exact_v1(
        self,
        columns: &mut [Vec<F>],
        root: F,
        mut uncertain: impl FnMut() -> bool,
    ) -> Result<(), ZkX509StarkErrorV1> {
        self.private_exact_v1(columns, root, Direction::Forward, &mut uncertain)?;
        // Preserve the old forward adapter's final completion refusal. Its
        // enclosing quotient owner clears the entire matrix on any error.
        check_completion_v1(uncertain())
    }

    fn private_exact_v1(
        self,
        columns: &mut [Vec<F>],
        root: F,
        direction: Direction,
        uncertain: impl FnMut() -> bool,
    ) -> Result<(), ZkX509StarkErrorV1> {
        let usage = if direction == Direction::Forward {
            TransformUseV1::PrivateQuotient
        } else {
            TransformUseV1::PrivateNativeReplay
        };
        self.exact_for_use_v1(columns, root, direction, usage, uncertain)
    }

    fn exact_for_use_v1(
        self,
        columns: &mut [Vec<F>],
        root: F,
        direction: Direction,
        usage: TransformUseV1,
        mut uncertain: impl FnMut() -> bool,
    ) -> Result<(), ZkX509StarkErrorV1> {
        check_completion_v1(uncertain())?;
        let forward = direction == Direction::Forward;
        let maximum = if forward {
            fastpq_prover::goldilocks_transform::MAX_GOLDILOCKS_PREPARED_FORWARD_COLUMNS_V1
        } else {
            aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1
        };
        let rows = validate_private_columns_with_limit_v1(columns, root, maximum)?;
        let width = if forward {
            self.forward_columns_v1(rows)
        } else {
            self.columns_v1(rows)
        };
        if width == 0 {
            return self.private_cpu_v1(columns, root, direction, usage, &mut uncertain);
        }
        for batch in columns.chunks_mut(width) {
            check_completion_v1(uncertain())?;
            let prepared = if forward {
                super::main_native_transform::prepare_forward_and_publish_v1(
                    batch,
                    root,
                    self.available,
                    &mut uncertain,
                )
            } else {
                super::main_native_transform::prepare_and_publish_v1(
                    batch,
                    root,
                    self.available,
                    &mut uncertain,
                )
            }?;
            match prepared {
                super::main_native_transform::MainNativePreparedV1::Published =>
                {
                    #[cfg(test)]
                    match usage {
                        TransformUseV1::Fixed => {
                            super::super::super::prover_observation::completed_fixed_backend_v1(
                                true,
                                direction == Direction::Inverse,
                                batch.len(),
                            );
                        }
                        TransformUseV1::PrivateQuotient => {
                            super::super::super::prover_observation::completed_quotient_backend_v1(
                                true,
                                batch.len(),
                            );
                        }
                        TransformUseV1::PrivateNativeReplay => {
                            super::super::super::prover_observation::completed_native_replay_backend_v1(
                                true,
                                batch.len(),
                            );
                        }
                    }
                }
                super::main_native_transform::MainNativePreparedV1::Cpu => {
                    self.private_cpu_v1(batch, root, direction, usage, &mut uncertain)?;
                }
            }
        }
        Ok(())
    }

    fn private_cpu_v1(
        self,
        columns: &mut [Vec<F>],
        root: F,
        direction: Direction,
        usage: TransformUseV1,
        uncertain: &mut impl FnMut() -> bool,
    ) -> Result<(), ZkX509StarkErrorV1> {
        let cpu = Self {
            backend: None,
            ..self
        };
        // CPU arithmetic keeps its original bounded eight-column work groups.
        for batch in columns.chunks_mut(aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1) {
            cpu.private_with_v1(
                batch,
                root,
                direction,
                usage,
                |_, _, _| Err(TransformError::DeviceUnavailable),
                &mut *uncertain,
            )?;
        }
        Ok(())
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
        let rows = validate_private_columns_v1(columns, root)?;
        let width = self.columns_v1(rows);
        if width == 0 {
            // Table selection uses only the public domain and pre-reserved
            // capability. Bounded columns retain the shared FFT windows.
            let table_root = match direction {
                Direction::Forward => root,
                Direction::Inverse => root.inv().ok_or(ZkX509StarkErrorV1::ProfileMismatch)?,
            };
            let powers = self.cpu_powers_v1(rows, table_root)?;
            columns.par_iter_mut().try_for_each(|column| {
                if let Some(powers) = &powers {
                    return match direction {
                        Direction::Forward => crate::privacy_engines::transparent_stark::goldilocks_fft_coarse_with_powers_v1(column, root, powers),
                        Direction::Inverse => crate::privacy_engines::transparent_stark::goldilocks_ifft_coarse_with_powers_v1(column, root, powers),
                    }.map_err(map_transparent_error_v1);
                }
                match direction {
                    Direction::Forward => {
                        crate::privacy_engines::transparent_stark::goldilocks_fft_coarse_v1(
                            column, root,
                        )
                    }
                    Direction::Inverse => {
                        crate::privacy_engines::transparent_stark::goldilocks_ifft_coarse_v1(
                            column, root,
                        )
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
                TransformUseV1::Fixed => {
                    super::super::super::prover_observation::completed_fixed_backend_v1(
                        false,
                        direction == Direction::Inverse,
                        columns.len(),
                    )
                }
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
            || !canonical_field_columns_v1(columns)
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
        copy_fields_to_words_v1(&mut words, columns);
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
        // Every public tile starts at its exact power. Input copy and every
        // completed transform validate all words before this arithmetic.
        scale_word_columns_v1(&mut words, diagonal);
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
        copy_words_to_fields_v1(columns, &words);
        Ok(())
    }
}

fn validate_private_columns_v1(columns: &[Vec<F>], root: F) -> Result<usize, ZkX509StarkErrorV1> {
    validate_private_columns_with_limit_v1(
        columns,
        root,
        aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1,
    )
}

fn validate_private_columns_with_limit_v1(
    columns: &[Vec<F>],
    root: F,
    maximum: usize,
) -> Result<usize, ZkX509StarkErrorV1> {
    let rows = columns.first().map_or(0, Vec::len);
    if columns.is_empty()
        || columns.len() > maximum
        || rows < 2
        || !rows.is_power_of_two()
        || rows > 1 << main_quotient_stripes::MAIN_QUOTIENT_STRIPE_LOG2_V1
        || columns.iter().any(|column| column.len() != rows)
        || !canonical_field_columns_v1(columns)
        || F::canonical(root.0).is_none()
        || root.pow(rows as u128) != F::ONE
        || root.pow((rows / 2) as u128) == F::ONE
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    Ok(rows)
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
        TransformError::NonCanonicalInput | TransformError::NonCanonicalOutput => {
            ZkX509StarkErrorV1::NonCanonicalField
        }
        _ => ZkX509StarkErrorV1::ProfileMismatch,
    })?;
    if backend == Backend::Cuda
        || words.iter().enumerate().any(|(index, column)| {
            column.len() != rows || (column.as_ptr(), column.capacity()) != allocations[index]
        })
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    if !canonical_word_columns_v1(words) {
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

#[cfg(test)]
#[path = "main_fft_scheduler_tests.rs"]
mod scheduler_tests;

#[cfg(test)]
#[path = "main_shared_fft_powers_tests.rs"]
mod shared_power_tests;

#[cfg(all(test, target_os = "macos"))]
#[path = "main_cpu_metal_geometry_tests.rs"]
mod cpu_metal_geometry_tests;
