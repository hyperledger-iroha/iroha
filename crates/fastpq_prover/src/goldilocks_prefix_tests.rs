//! Test-only direct native-prefix preparation through the existing closed token.
//!
//! No production API or dispatch policy changes. Immutable coefficients include
//! the complete masking tail; only public dimensions determine staging work.

use super::*;
use GoldilocksTransformErrorV1 as Error;

/// Validate domain, complete input prefixes, and the complete public power table.
pub(crate) fn validate_prefixes_v1(
    prefixes: &[&[u64]],
    rows: usize,
    root: u64,
    powers: &[u64],
) -> Result<cyclotomic::Domain, Error> {
    if prefixes.is_empty()
        || prefixes.len() > MAX_GOLDILOCKS_TRANSFORM_COLUMNS_V1
        || rows < 2
        || !rows.is_power_of_two()
        || rows.trailing_zeros() > 32
        || powers.is_empty()
        || powers.len() > rows
        || prefixes
            .iter()
            .any(|column| column.is_empty() || column.len() > powers.len())
        || rows
            .checked_mul(prefixes.len())
            .and_then(|n| n.checked_mul(8))
            .is_none()
    {
        return Err(Error::InvalidShape);
    }
    if root == 0
        || root >= FIELD_MODULUS
        || power_v1(root, rows as u64) != 1
        || power_v1(root, rows as u64 / 2) == 1
    {
        return Err(Error::InvalidRoot);
    }
    let words = prefixes.iter().map(|column| column.len()).sum();
    if !canonical_word_columns_v1(prefixes, words) || !canonical_word_slice_v1(powers) {
        return Err(Error::NonCanonicalInput);
    }
    Ok(cyclotomic::Domain {
        log_size: rows.trailing_zeros(),
        generator: root,
    })
}

/// Test-only CPU diagonal, matching privacy's current unsigned Goldilocks fold.
/// Both benchmark arms use this exact helper; the oracle uses u128 remainder.
#[inline]
pub(crate) fn prefix_product_v1(left: u64, right: u64) -> u64 {
    const EPSILON: u64 = 0xffff_ffff;
    let value = u128::from(left) * u128::from(right);
    let low = value as u64;
    let high = (value >> 64) as u64;
    let (low, borrowed) = low.overflowing_sub(high >> 32);
    let low = if borrowed {
        low.wrapping_sub(EPSILON)
    } else {
        low
    };
    let (value, carried) = low.overflowing_add((high & EPSILON) * EPSILON);
    let value = if carried {
        value.wrapping_add(EPSILON)
    } else {
        value
    };
    if value >= FIELD_MODULUS {
        value - FIELD_MODULUS
    } else {
        value
    }
}

/// Borrow native prefixes and prepare common-domain output without a full host input.
/// The CPU outcome leaves all inputs unchanged for the original CPU fallback.
pub(crate) fn prepare_prefixes_v1(
    prefixes: &[&[u64]],
    rows: usize,
    root: u64,
    powers: &[u64],
    execution: ExecutionMode,
) -> Result<PreparedGoldilocksColumnsOutcomeV1, Error> {
    let domain = validate_prefixes_v1(prefixes, rows, root, powers)?;
    if matches!(execution, ExecutionMode::Cpu) {
        return Ok(PreparedGoldilocksColumnsOutcomeV1::Cpu);
    }
    if goldilocks_transform_completion_uncertain_v1() {
        return Err(Error::CompletionUncertain);
    }
    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    {
        use crate::backend::{self, GpuBackend};
        if matches!(execution.resolve(), ExecutionMode::Gpu)
            && backend::current_gpu_backend() == Some(GpuBackend::Metal)
            && let Some(lane) = transform_lane_v1(execution)
        {
            let result = crate::metal::exact_root::prefix_tests::prepare_validated_prefixes_v1(
                prefixes,
                domain.log_size,
                root,
                powers,
            );
            if let Some(inner) = classify_dispatch_result_v1(
                result,
                execution,
                goldilocks_transform_completion_uncertain_v1(),
            )? {
                return Ok(PreparedGoldilocksColumnsOutcomeV1::Metal(
                    PreparedGoldilocksColumnsV1 {
                        inner,
                        _lane: lane,
                        _sealed: core::cell::Cell::new(core::marker::PhantomData),
                    },
                ));
            }
        }
    }
    #[cfg(not(all(feature = "fastpq-gpu", target_os = "macos")))]
    let _ = domain;
    if matches!(execution, ExecutionMode::Gpu) {
        return Err(Error::DeviceUnavailable);
    }
    Ok(PreparedGoldilocksColumnsOutcomeV1::Cpu)
}

#[test]
fn native_prefix_validation_checks_all_tail_words_and_error_priority() {
    let root = power_v1(7, (FIELD_MODULUS - 1) >> 12);
    let source = vec![3; 2048 + 1816];
    let powers = vec![1; source.len()];
    assert!(validate_prefixes_v1(&[&source], 4096, root, &powers).is_ok());
    for position in [0, 2047, 2048, source.len() - 1] {
        let mut invalid = source.clone();
        invalid[position] = FIELD_MODULUS;
        assert_eq!(
            validate_prefixes_v1(&[&source, &invalid], 4096, root, &powers),
            Err(Error::NonCanonicalInput)
        );
        assert_eq!(
            validate_prefixes_v1(&[&invalid], 4096, 1, &powers),
            Err(Error::InvalidRoot)
        );
        assert_eq!(
            validate_prefixes_v1(&[&invalid], 4095, 1, &powers),
            Err(Error::InvalidShape)
        );
    }
    let mut invalid = powers.clone();
    *invalid.last_mut().unwrap() = u64::MAX;
    assert_eq!(
        validate_prefixes_v1(&[&source], 4096, root, &invalid),
        Err(Error::NonCanonicalInput)
    );
    for (columns, rows, powers) in [
        (vec![], 4096, powers.as_slice()),
        (vec![source.as_slice(); 9], 4096, powers.as_slice()),
        (vec![&[][..]], 4096, powers.as_slice()),
        (vec![source.as_slice()], 4096, &powers[..powers.len() - 1]),
        (vec![source.as_slice()], 2048, powers.as_slice()),
    ] {
        assert_eq!(
            validate_prefixes_v1(&columns, rows, root, powers),
            Err(Error::InvalidShape)
        );
    }
    assert!(matches!(
        prepare_prefixes_v1(&[&source], 4096, root, &powers, ExecutionMode::Cpu),
        Ok(PreparedGoldilocksColumnsOutcomeV1::Cpu)
    ));
    assert_eq!(source, vec![3; 2048 + 1816]);
}

#[test]
fn native_prefix_product_matches_independent_remainder_boundaries_and_samples() {
    for left in [
        0,
        1,
        7,
        u32::MAX as u64,
        1 << 32,
        FIELD_MODULUS - 2,
        FIELD_MODULUS - 1,
    ] {
        for right in [
            0,
            1,
            7,
            u32::MAX as u64,
            1 << 32,
            FIELD_MODULUS - 2,
            FIELD_MODULUS - 1,
        ] {
            assert_eq!(prefix_product_v1(left, right), mul_mod_v1(left, right));
        }
    }
    let mut state = 19_u64;
    for _ in 0..10000 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let left = state % FIELD_MODULUS;
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let right = state % FIELD_MODULUS;
        assert_eq!(prefix_product_v1(left, right), mul_mod_v1(left, right));
    }
}
