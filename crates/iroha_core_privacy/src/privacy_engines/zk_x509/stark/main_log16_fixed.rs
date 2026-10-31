//! One-pass Log16 public fixed rows and bounded interpolation in final owners.

use super::*;
use fastpq_prover::goldilocks_transform::goldilocks_transform_completion_uncertain_v1;
use main_bounded_transform::{MainBoundedTransformPolicyV1, check_completion_v1};

fn validate_binding_v1(binding: MainP256RegistrationBindingV1) -> Result<(), ZkX509StarkErrorV1> {
    if binding.main.segment.trace_log2 != 16
        || !matches!(
            binding.p256.adapter_v1(),
            P256MainAdapterV1::WindowBatch | P256MainAdapterV1::BindingSink
        )
        || binding.p256.local_instance_v1() != 0
        || p256_main_registration_from_main_layout_v1(binding.main)? != binding.p256
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    Ok(())
}

pub(super) fn fixed_set_v1(
    source: &MainP256Log16ProverConstraintSourceV1<'_>,
    registration: RegisteredSegmentLayoutV1,
    policy: MainBoundedTransformPolicyV1,
    cache_plan: main_quotient_cache::MainQuotientCachePlanV1,
) -> Result<ZeroizingMainFixedPolynomialSetV1, ZkX509StarkErrorV1> {
    check_completion_v1(goldilocks_transform_completion_uncertain_v1())?;
    let binding = source.binding_v1(registration)?;
    validate_binding_v1(binding)?;
    let policy = policy.for_quotient_layout_v1(
        registration.segment.base_width,
        registration.segment.aux_width,
    )?;
    let root = goldilocks_primitive_root_v1(registration.segment.trace_log2)
        .map_err(map_transparent_error_v1)?;
    const {
        assert!(
            P256_WINDOW_AGGREGATE_FIXED_WIDTH_V1 <= main_resources::MAIN_FIXED_REPLAY_ROW_WIDTH_V1
        );
        assert!(P256_BINDING_SINK_FIXED_WIDTH_V1 <= main_resources::MAIN_FIXED_REPLAY_ROW_WIDTH_V1);
    }
    let columns = dense_columns_with_v1(
        registration.segment.trace_size(),
        registration.segment.fixed_width,
        root,
        policy,
        |columns| {
            source
                .source
                .fill_log16_fixed_matrix_v1(binding.p256, columns)
                .map_err(ZkX509StarkErrorV1::from)
        },
        &mut main_native_transform::ExactNativeTransformV1,
        goldilocks_transform_completion_uncertain_v1,
    )?;
    Ok(ZeroizingMainFixedPolynomialSetV1 {
        registration,
        cache_plan,
        owner: MainFixedCoefficientOwnerV1::Dense(columns),
    })
}

#[allow(clippy::too_many_arguments)]
fn dense_columns_with_v1(
    rows: usize,
    width: usize,
    root: F,
    policy: MainBoundedTransformPolicyV1,
    fill: impl FnOnce(&mut [Vec<F>]) -> Result<(), ZkX509StarkErrorV1>,
    transform: &mut impl main_native_transform::MainNativeTransformV1,
    mut uncertain: impl FnMut() -> bool,
) -> Result<ZeroizingBaseColumnsV1, ZkX509StarkErrorV1> {
    check_completion_v1(uncertain())?;
    if rows < 2
        || !rows.is_power_of_two()
        || rows > 1 << 16
        || width == 0
        || width > main_resources::MAIN_FIXED_REPLAY_ROW_WIDTH_V1
        || F::canonical(root.0).is_none()
        || root.pow(rows as u128) != F::ONE
        || root.pow((rows / 2) as u128) == F::ONE
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let mut columns = ZeroizingBaseColumnsV1(Vec::new());
    columns
        .0
        .try_reserve_exact(width)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    if columns.0.capacity() != width {
        return Err(ZkX509StarkErrorV1::AllocationFailure);
    }
    for _ in 0..width {
        check_completion_v1(uncertain())?;
        let values = zeroed_main_trace_column_v1(rows)?;
        if values.0.capacity() != rows {
            return Err(ZkX509StarkErrorV1::AllocationFailure);
        }
        columns.0.push(values.into_vec_v1());
    }
    fill(&mut columns.0)?;
    check_completion_v1(uncertain())?;
    // Preserve the original scalar IFFT's canonicality refusal before dispatch.
    if columns
        .0
        .iter()
        .flatten()
        .any(|value| F::canonical(value.0).is_none())
    {
        return Err(ZkX509StarkErrorV1::NonCanonicalField);
    }
    main_initial_fixed_coefficients_native_with_v1(
        &mut columns.0,
        root,
        policy,
        transform,
        &mut uncertain,
    )?;
    check_completion_v1(uncertain())?;
    Ok(columns)
}

#[cfg(test)]
#[path = "main_log16_fixed_tests.rs"]
mod tests;
