//! Checked live-buffer accounting for the canonical mask-replay MAIN prover.
//!
//! This charges the actual transform/commitment owners and admits the separately
//! measured native source capacities and bounded replay scratch. An explicit
//! process reserve leaves space for allocator overhead and thread stacks; it is
//! deliberately not a measured RSS guarantee or a resource certificate.

pub(super) use super::super::super::allocation_payload::{
    MAIN_NATIVE_SOURCE_ALLOWANCE_BYTES_V1, MAIN_SOURCE_SCRATCH_ALLOWANCE_BYTES_V1,
};
use super::*;

/// Headroom beyond charged source and arithmetic allocations. This is a policy
/// reserve, not an estimate or a portable bound on the process allocator or RSS.
pub(super) const MAIN_PROVER_RUNTIME_RESERVE_BYTES_V1: usize = 1 << 30;

/// Dimensions inspected before any additional private source matrix is built.
/// All other source dimensions come from the exact registered MAIN layout.
#[derive(Clone, Copy)]
pub(super) struct MainSmallSourceShapeV1 {
    pub(super) der_active_rows: usize,
    pub(super) io_active_rows: usize,
    pub(super) io_declarations_payload: usize,
    pub(super) projection_rows: usize,
}

impl MainSmallSourceShapeV1 {
    pub(super) fn for_assembly_v1(
        assembly: &ZkX509MainTraceAssemblyV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        use super::super::super::allocation_payload::{declaration_v1, sum_v1, vector_v1};
        assembly.der_base.private_shape.validate()?;
        let der_active_rows = assembly.der_base.private_shape.active_rows()?;
        if assembly.der_base.rows.len() != der_active_rows
            || assembly.io.execution.len() != assembly.io.logical_active_rows
            || assembly.io.sorted.len() != assembly.io.logical_active_rows
            || assembly.io.witnesses.len() != assembly.io.declarations.len()
        {
            return Err(ZkX509StarkErrorV1::WitnessStatementMismatch);
        }
        Ok(Self {
            der_active_rows,
            io_active_rows: assembly.io.logical_active_rows,
            io_declarations_payload: sum_v1([
                vector_v1(&assembly.io.declarations),
                sum_v1(assembly.io.declarations.iter().map(declaration_v1)),
            ]),
            projection_rows: assembly.projection_trace.base.rows.len(),
        })
    }

    /// Retained DER/I/O/projection payload and their serial construction scratch.
    /// Full native widths are charged even when the actual I/O prefix is smaller.
    pub(super) fn allocation_forecast_v1(
        self,
        layout: &AggregateProofLayoutV1,
    ) -> Result<(usize, usize), ZkX509StarkErrorV1> {
        use super::super::super::io_air::IoPermutationRowV1;
        layout.validate_exact_full_profile_registration_v1()?;
        let io = layout
            .registered_segment(SegmentAdapterIdV1::ByteMemory, 0)?
            .segment;
        let projection = layout
            .registered_segment(SegmentAdapterIdV1::Projection, 0)?
            .segment;
        if self.der_active_rows == 0
            || self.der_active_rows > ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1
            || self.io_active_rows == 0
            || self.io_active_rows > io.trace_size()
            || self.projection_rows != projection.trace_size()
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let field = core::mem::size_of::<F>();
        let retained = sum(&[
            product(&[
                self.der_active_rows,
                ZK_X509_DER_STARK_BASE_WIDTH_V1 + ZK_X509_DER_STARK_AUX_WIDTH_V1,
                field,
            ])?,
            product(&[
                io.trace_size(),
                io.base_width + io.fixed_width + io.aux_width,
                field,
            ])?,
            product(&[self.projection_rows, projection.aux_width, field])?,
            self.io_declarations_payload,
            // Headers, cloned DER document lengths and canonical registration
            // metadata. P-256 heap ownership is charged by its separate forecast.
            core::mem::size_of::<MainLog19BoundTraceGroupSourceV1<'_>>(),
            core::mem::size_of::<MainIoTraceGroupSourceV1<'_>>(),
            core::mem::size_of::<MainProjectionTraceGroupSourceV1<'_>>(),
            product(&[
                layout.registered_segments.len(),
                core::mem::size_of::<RegisteredSegmentLayoutV1>()
                    + core::mem::size_of::<MainP256RegistrationBindingV1>(),
            ])?,
            product(&[
                super::super::super::der_stark::ZK_X509_DER_STARK_MAX_DOCUMENTS_V1,
                core::mem::size_of::<u16>(),
            ])?,
            product(&[
                io.base_width + io.fixed_width + io.aux_width,
                core::mem::size_of::<Vec<F>>(),
            ])?,
        ])?;
        // I/O builds execution/sorted tables, permutation rows and public fixed
        // topology before dropping them. Double their maximum capacities for
        // geometric Vec growth, and include sorting workspace and declarations.
        // Projection validates one additional full public fixed trace. DER/RFC
        // column extraction itself returns only the separately charged column.
        let scratch = sum(&[
            product(&[4, io.trace_size(), core::mem::size_of::<IoAccessV1>()])?,
            product(&[
                2,
                io.trace_size(),
                core::mem::size_of::<IoPermutationRowV1>(),
            ])?,
            product(&[
                4,
                io.trace_size(),
                core::mem::size_of::<MainIoAccessTopologyV1>(),
            ])?,
            product(&[2, io.trace_size(), core::mem::size_of::<Option<F>>()])?,
            product(&[2, self.projection_rows, projection.fixed_width, field])?,
            product(&[4, self.io_declarations_payload])?,
            // Row validators, continuation vectors and small metadata are
            // bounded by the fixed registration sizes and charged separately.
            32 << 20,
        ])?;
        Ok((retained, scratch))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MainProverBufferPlanV1 {
    pub(super) masks: usize,
    pub(super) joined_streams: usize,
    pub(super) replay_batch: usize,
    pub(super) composition: usize,
    pub(super) quotient_stage: usize,
    pub(super) fri_stage: usize,
    pub(super) openings: usize,
    pub(super) maximum_live_buffers: usize,
    pub(super) remaining_source_and_runtime_envelope: usize,
}

fn product(values: &[usize]) -> Result<usize, ZkX509StarkErrorV1> {
    values.iter().try_fold(1_usize, |total, value| {
        total
            .checked_mul(*value)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)
    })
}

fn sum(values: &[usize]) -> Result<usize, ZkX509StarkErrorV1> {
    values.iter().try_fold(0_usize, |total, value| {
        total
            .checked_add(*value)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)
    })
}

impl MainProverBufferPlanV1 {
    /// Inspect private source extents before constructing any corresponding
    /// matrix; forecast arithmetic must fit the admitted source allowances.
    pub(super) fn check_source_shapes_v1(
        &self,
        layout: &AggregateProofLayoutV1,
        assembly: &ZkX509MainTraceAssemblyV1,
    ) -> Result<(), ZkX509StarkErrorV1> {
        self.check_before_sources_v1(assembly.allocated_payload_bytes_v1())?;
        let (small_retained, small_scratch) =
            MainSmallSourceShapeV1::for_assembly_v1(assembly)?.allocation_forecast_v1(layout)?;
        let shape = assembly.sha_schedule.shape();
        let retained = sum(&[
            small_retained,
            P256MainBaseSourceV1::allocation_forecast_v1()?,
            ZkX509ShaBatchFixedProviderV1::allocation_forecast_v1(shape)
                .map_err(map_main_sha_source_error_v1)?,
        ])?;
        // Sources replay serially. The eight-way parallel stage starts only
        // after their native columns have become coefficient owners.
        let scratch = small_scratch
            .max(P256MainBaseSourceV1::replay_scratch_forecast_v1()?)
            .max(
                ZkX509ShaBatchFixedProviderV1::replay_scratch_forecast_v1(shape)
                    .map_err(map_main_sha_source_error_v1)?,
            );
        if retained > MAIN_NATIVE_SOURCE_ALLOWANCE_BYTES_V1
            || scratch > MAIN_SOURCE_SCRATCH_ALLOWANCE_BYTES_V1
        {
            return Err(ZkX509StarkErrorV1::ProofTooLarge);
        }
        Ok(())
    }

    /// Reserve the complete owned-source allowance before constructing any of
    /// those sources or consuming entropy. The caller already owns the assembly.
    pub(super) fn check_before_sources_v1(
        &self,
        assembly_payload: usize,
    ) -> Result<usize, ZkX509StarkErrorV1> {
        self.check_source_payloads_v1(
            &[assembly_payload, MAIN_NATIVE_SOURCE_ALLOWANCE_BYTES_V1],
            MAIN_SOURCE_SCRATCH_ALLOWANCE_BYTES_V1,
        )
    }

    /// Check actual source capacities before mask sampling, composition or FRI.
    /// Keeping the admitted allowance fixed prevents later phases from spending
    /// capacity reserved for source replay or runtime overhead.
    pub(super) fn check_native_sources_v1(
        &self,
        assembly_payload: usize,
        native_sources: &[usize],
    ) -> Result<usize, ZkX509StarkErrorV1> {
        let sources = sum(native_sources)?;
        if sources > MAIN_NATIVE_SOURCE_ALLOWANCE_BYTES_V1 {
            return Err(ZkX509StarkErrorV1::ProofTooLarge);
        }
        self.check_source_payloads_v1(
            &[assembly_payload, sources],
            MAIN_SOURCE_SCRATCH_ALLOWANCE_BYTES_V1,
        )
    }

    /// Admit reachable source capacities and their worst simultaneous transient
    /// payload before allocating the next proof phase. Overflow fails closed;
    /// capacity getters may conservatively report `usize::MAX` on overflow.
    pub(super) fn check_source_payloads_v1(
        &self,
        source_payloads: &[usize],
        source_scratch: usize,
    ) -> Result<usize, ZkX509StarkErrorV1> {
        let retained = sum(source_payloads)?;
        let required = sum(&[
            retained,
            source_scratch,
            MAIN_PROVER_RUNTIME_RESERVE_BYTES_V1,
        ])?;
        if required > self.remaining_source_and_runtime_envelope {
            return Err(ZkX509StarkErrorV1::ProofTooLarge);
        }
        sum(&[self.maximum_live_buffers, required])
    }

    pub(super) fn new_v1(layout: &AggregateProofLayoutV1) -> Result<Self, ZkX509StarkErrorV1> {
        layout.validate_exact_full_profile_registration_v1()?;
        let shared = layout.as_shared()?;
        let field = core::mem::size_of::<F>();
        let extension = core::mem::size_of::<E>();
        let digest = core::mem::size_of::<PrivacyOuterDigestV1>();
        let rows = layout.common_lde_size();
        let degree_cap = shared
            .fri_degree_cap(AGGREGATE_PARAMETERS_V1)
            .map_err(map_aggregate_error_v1)?;
        let total_width = layout
            .trace_groups
            .iter()
            .try_fold(0_usize, |total, group| {
                sum(&[total, group.base_width, group.aux_width])
            })?;
        let maximum_native = layout
            .trace_groups
            .iter()
            .map(|group| 1_usize << group.native_trace_log2)
            .max()
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let masks = product(&[total_width, MASK_DEGREE + 1, field])?;
        let joined_streams = product(&[
            rows,
            core::mem::size_of::<
                crate::privacy_engines::privacy_outer_hash::PrivacyOuterLastFieldStreamV1,
            >(),
        ])?;
        // Eight original coefficient owners and eight common-domain FFT outputs;
        // one additional native/IFFT column covers serial reconstruction overlap.
        let replay_batch = sum(&[
            product(&[
                aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1,
                sum(&[maximum_native, MASK_DEGREE + 1])?,
                field,
            ])?,
            product(&[aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1, rows, field])?,
            product(&[2, maximum_native, field])?,
        ])?;
        let composition = product(&[
            SECURITY_LANES,
            COMPOSITION_DEGREE_CHUNKS,
            sum(&[rows, degree_cap])?,
            extension,
        ])?;
        let mut quotient_stage = 0;
        for registration in &layout.registered_segments {
            let segment = registration.segment;
            let plan = registered_retained_prover_plan_v1(segment, layout.common_lde_log2)?;
            let stripe = main_quotient_stripes::MainQuotientStripeV1::new_v1(
                segment.trace_log2,
                plan.quotient_coset_log2,
                0,
            )?;
            // One complete registration stripe; its fixed native coefficients;
            // quotient evaluations plus IFFT copy; accumulated and incoming chunks.
            let candidate = sum(&[
                product(&[
                    sum(&[segment.base_width, segment.aux_width, segment.fixed_width])?,
                    stripe.rows,
                    field,
                ])?,
                product(&[segment.fixed_width, segment.trace_size(), field])?,
                product(&[2, SECURITY_LANES, plan.quotient_coset_rows, extension])?,
                product(&[
                    2,
                    SECURITY_LANES,
                    COMPOSITION_DEGREE_CHUNKS,
                    degree_cap,
                    extension,
                ])?,
            ])?;
            quotient_stage = quotient_stage.max(candidate);
        }
        // Mask coefficients/evaluations and all binary digest-tree levels, plus
        // current/successor FRI layers and a DEEP coefficient accumulator.
        let fri_stage = sum(&[
            product(&[SECURITY_LANES, 4, rows, extension])?,
            product(&[SECURITY_LANES, 2, rows, digest])?,
            product(&[SECURITY_LANES, degree_cap, extension])?,
        ])?;
        // Authenticated rows coexist with copied query rows and canonical wire.
        let openings = sum(&[
            product(&[2, AGGREGATE_PARAMETERS_V1.query_count, total_width, field])?,
            product(&[
                4,
                usize::try_from(ZK_X509_MAX_PROOF_BYTES_V1)
                    .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
            ])?,
        ])?;
        // Composition material is returned only after registration quotient
        // temporaries drop. The FRI phase then retains composition material,
        // mask oracles and the replayed FRI input while rebuilding joined roots.
        // These are real caller lifetime boundaries, not optimistic allocator
        // reuse. Within each stage the accounting remains conservative.
        let mut plan = Self {
            masks,
            joined_streams,
            replay_batch,
            composition,
            quotient_stage,
            fri_stage,
            openings,
            maximum_live_buffers: 0,
            remaining_source_and_runtime_envelope: 0,
        };
        plan.maximum_live_buffers = sum(&[
            plan.masks,
            sum(&[plan.quotient_stage, plan.replay_batch])?.max(sum(&[
                plan.joined_streams,
                plan.replay_batch,
                plan.composition,
                plan.fri_stage,
                plan.openings,
            ])?),
        ])?;
        let ceiling =
            usize::try_from(super::super::super::profile::ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1)
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
        plan.remaining_source_and_runtime_envelope = ceiling
            .checked_sub(plan.maximum_live_buffers)
            .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?;
        Ok(plan)
    }
}
