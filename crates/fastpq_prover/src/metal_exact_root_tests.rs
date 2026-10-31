//! Arithmetic, atomic publication and clearing controls for staged Metal FFTs.

use super::*;
use crate::cyclotomic::{self, Domain};
use crate::gpu_secret::ErasureObservation;

fn source(log: u32, columns: usize) -> Vec<Vec<u64>> {
    (0..columns)
        .map(|column| {
            (0..(1_usize << log))
                .map(|row| match row % 29 {
                    0 => 0,
                    1 => FIELD_MODULUS - 1,
                    _ => {
                        let index = (row + (column << log)) as u64;
                        (index.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ (index >> 3)) % FIELD_MODULUS
                    }
                })
                .collect()
        })
        .collect()
}

fn oracle_add(left: u64, right: u64) -> u64 {
    ((u128::from(left) + u128::from(right)) % u128::from(FIELD_MODULUS)) as u64
}

fn oracle_sub(left: u64, right: u64) -> u64 {
    ((u128::from(left) + u128::from(FIELD_MODULUS) - u128::from(right)) % u128::from(FIELD_MODULUS))
        as u64
}

fn reference_stage(values: &mut [u64], root: u64, stage: u32) {
    let half = 1_usize << stage;
    let step = goldilocks_pow(root, (values.len() / (2 * half)) as u64);
    for block in values.chunks_exact_mut(2 * half) {
        let (left, right) = block.split_at_mut(half);
        let mut twiddle = 1;
        for (left, right) in left.iter_mut().zip(right) {
            let original_left = *left;
            let scaled_right = goldilocks_mul(*right, twiddle);
            *left = oracle_add(original_left, scaled_right);
            *right = oracle_sub(original_left, scaled_right);
            twiddle = goldilocks_mul(twiddle, step);
        }
    }
}

fn fused_global_model(values: &mut [u64], log: u32, root: u64, stage: u32) -> Vec<u8> {
    let rows = values.len() as u64;
    let half = 1_u64 << stage;
    let paired = stage + 1 < log;
    let cells = if paired { rows / 4 } else { rows / 2 };
    let cells_per_group = if paired { 1024 } else { 2048 };
    let iterations = cells_per_group / LANES;
    let step = goldilocks_pow(root, rows >> (stage + if paired { 2 } else { 1 }));
    let stride = goldilocks_pow(step, LANES);
    let fourth_root = goldilocks_pow(root, rows / 4);
    let mut visits = vec![0_u8; values.len()];
    for group in 0..rows.div_ceil(WORDS_PER_GLOBAL_GROUP) {
        for lane in 0..LANES {
            let mut cell = group * cells_per_group + lane;
            let mut offset = cell & (half - 1);
            let first = goldilocks_pow(step, offset);
            let mut twiddle = first;
            for _ in 0..iterations {
                if cell >= cells {
                    break;
                }
                assert_eq!(twiddle, goldilocks_pow(step, offset));
                let low = ((cell - offset) * if paired { 4 } else { 2 } + offset) as usize;
                let half = half as usize;
                if paired {
                    let squared = goldilocks_mul(twiddle, twiddle);
                    let a = values[low];
                    let b = goldilocks_mul(values[low + half], squared);
                    let c = values[low + 2 * half];
                    let d = goldilocks_mul(values[low + 3 * half], squared);
                    let ab_sum = oracle_add(a, b);
                    let ab_difference = oracle_sub(a, b);
                    let cd_sum = goldilocks_mul(oracle_add(c, d), twiddle);
                    let cd_difference =
                        goldilocks_mul(oracle_sub(c, d), goldilocks_mul(twiddle, fourth_root));
                    values[low] = oracle_add(ab_sum, cd_sum);
                    values[low + half] = oracle_add(ab_difference, cd_difference);
                    values[low + 2 * half] = oracle_sub(ab_sum, cd_sum);
                    values[low + 3 * half] = oracle_sub(ab_difference, cd_difference);
                    for part in 0..4 {
                        visits[low + part * half] += 1;
                    }
                } else {
                    let left = values[low];
                    let right = goldilocks_mul(values[low + half], twiddle);
                    values[low] = oracle_add(left, right);
                    values[low + half] = oracle_sub(left, right);
                    visits[low] += 1;
                    visits[low + half] += 1;
                }
                twiddle = if offset + LANES >= half as u64 {
                    first
                } else {
                    goldilocks_mul(twiddle, stride)
                };
                cell += LANES;
                offset = cell & (half as u64 - 1);
            }
        }
    }
    visits
}

#[test]
fn host_arguments_and_fused_global_partition_match_independent_radix_two_stages() {
    assert_eq!(mem::size_of::<ExactRootFftArgs>(), 32);
    assert_eq!(mem::offset_of!(ExactRootFftArgs, log_len), 16);
    for log in 9..=15 {
        let root = goldilocks_pow(7, (FIELD_MODULUS - 1) >> log);
        for root in [root, goldilocks_pow(root, 3), goldilocks_inv(root)] {
            for stage in (LOCAL_LOG..log).step_by(2) {
                let mut actual = source(log, 1).remove(0);
                let mut expected = actual.clone();
                reference_stage(&mut expected, root, stage);
                if stage + 1 < log {
                    reference_stage(&mut expected, root, stage + 1);
                }
                let visits = fused_global_model(&mut actual, log, root, stage);
                assert!(visits.iter().all(|visits| *visits == 1));
                assert_eq!(actual, expected, "log={log} stage={stage} root={root}");
            }
        }
    }
    for log in 9..=32 {
        let stages = (LOCAL_LOG..log).step_by(2).collect::<Vec<_>>();
        assert_eq!(stages.len(), ((log - LOCAL_LOG) as usize).div_ceil(2));
        let covered = stages
            .into_iter()
            .flat_map(|first| first..(first + 2).min(log))
            .collect::<Vec<_>>();
        assert_eq!(covered, (LOCAL_LOG..log).collect::<Vec<_>>());
    }
}

#[test]
fn fixed_bit_reverse_permutation_matches_every_small_domain_and_full_width_edges() {
    let reverse = |mut value: u32, log: u32| {
        value = ((value & 0x5555_5555) << 1) | ((value >> 1) & 0x5555_5555);
        value = ((value & 0x3333_3333) << 2) | ((value >> 2) & 0x3333_3333);
        value = ((value & 0x0f0f_0f0f) << 4) | ((value >> 4) & 0x0f0f_0f0f);
        value = ((value & 0x00ff_00ff) << 8) | ((value >> 8) & 0x00ff_00ff);
        value.rotate_right(16) >> (32 - log)
    };
    for log in 1..=32 {
        for value in (0_u32..(1 << log.min(16))).chain([u32::MAX, 0x0102_0304, 0x8000_0001]) {
            let value = value & (u32::MAX >> (32 - log));
            assert_eq!(reverse(value, log), value.reverse_bits() >> (32 - log));
            assert_eq!(reverse(reverse(value, log), log), value);
        }
    }
}

#[test]
fn fourth_root_shift_matches_independent_products_at_every_supported_order() {
    const POSITIVE: u64 = 1 << 48;
    const NEGATIVE: u64 = FIELD_MODULUS - POSITIVE;
    assert_eq!(NEGATIVE, 0xfffe_ffff_0000_0001);
    for log in 2..=32 {
        let root = goldilocks_pow(7, (FIELD_MODULUS - 1) >> log);
        for root in [root, goldilocks_pow(root, 3), goldilocks_inv(root)] {
            let fourth = goldilocks_pow(root, 1_u64 << (log - 2));
            assert!(fourth == POSITIVE || fourth == NEGATIVE);
            for value in [
                0,
                1,
                u64::from(u16::MAX),
                1 << 16,
                1 << 48,
                FIELD_MODULUS - 1,
            ]
            .into_iter()
            .chain(
                (0..1024_u64)
                    .map(|index| index.wrapping_mul(0x9e37_79b9_7f4a_7c15) % FIELD_MODULUS),
            ) {
                let actual = metal_fourth_root_model(value, fourth);
                let expected =
                    (u128::from(value) * u128::from(fourth) % u128::from(FIELD_MODULUS)) as u64;
                assert_eq!(actual, expected);
            }
        }
    }
}

// Mirrors the shader's unsigned fold, independently checked against u128 mod p.
fn metal_fourth_root_model(value: u64, fourth_root: u64) -> u64 {
    const POSITIVE: u64 = 1 << 48;
    const NEGATIVE: u64 = FIELD_MODULUS - POSITIVE;
    if fourth_root != POSITIVE && fourth_root != NEGATIVE {
        return goldilocks_mul(value, fourth_root);
    }
    let low_word = value << 48;
    let high_word = value >> 16;
    let epsilon = 0xffff_ffff_u64;
    let high_high = high_word >> 32;
    let mut low = low_word.wrapping_sub(high_high);
    if low_word < high_high {
        low = low.wrapping_sub(epsilon);
    }
    let mut folded = low.wrapping_add((high_word & epsilon) * epsilon);
    if folded < low {
        folded = folded.wrapping_add(epsilon);
    }
    if folded >= FIELD_MODULUS {
        folded -= FIELD_MODULUS;
    }
    if fourth_root == POSITIVE {
        folded
    } else {
        oracle_sub(0, folded)
    }
}

fn local_general_stage_model(
    values: &mut [u64],
    log: u32,
    omega: u64,
    normalization: u64,
) -> (usize, usize) {
    let local_log = log.min(LOCAL_LOG);
    assert_eq!(values.len(), 1 << local_log);
    let mut products = 0;
    for stage in 0..local_log {
        let half = 1_usize << stage;
        let mut visits = vec![0_u8; values.len()];
        for lane in 0..values.len() / 2 {
            let offset = lane & (half - 1);
            let low = (lane - offset) * 2 + offset;
            let twiddle = goldilocks_pow(omega, (offset as u64) << (log - stage - 1));
            let left = values[low];
            // The shipping local stage uses one general product per butterfly.
            let right = goldilocks_mul(values[low + half], twiddle);
            products += 1;
            values[low] = oracle_add(left, right);
            values[low + half] = oracle_sub(left, right);
            visits[low] += 1;
            visits[low + half] += 1;
        }
        // Every word has exactly one writer before the original stage barrier.
        assert!(visits.iter().all(|visits| *visits == 1));
    }
    if log == local_log && normalization != 1 {
        for value in values {
            *value = goldilocks_mul(*value, normalization);
        }
    }
    (products, local_log as usize)
}

#[test]
fn local_general_tiles_match_independent_stages_at_every_order_root_and_public_position() {
    for log in 1..=32 {
        let local_log = log.min(LOCAL_LOG);
        let len = 1_usize << local_log;
        for odd in [1, 3, 5, 7, 255] {
            let root = goldilocks_pow(goldilocks_pow(7, (FIELD_MODULUS - 1) >> log), odd);
            for inverse in [false, true] {
                let omega = if inverse { goldilocks_inv(root) } else { root };
                let local_root = goldilocks_pow(omega, 1_u64 << (log - local_log));
                let normalization = if inverse {
                    goldilocks_inv(1_u64 << log)
                } else {
                    1
                };
                for pattern in 0..3 {
                    let mut actual = (0..len)
                        .map(|index| match pattern {
                            0 => [0, 1, FIELD_MODULUS - 1][index % 3],
                            1 => (index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) % FIELD_MODULUS,
                            _ => {
                                if index == len - 1 {
                                    FIELD_MODULUS - 1
                                } else {
                                    0
                                }
                            }
                        })
                        .collect::<Vec<_>>();
                    let mut expected = actual.clone();
                    for stage in 0..local_log {
                        reference_stage(&mut expected, local_root, stage);
                    }
                    if log == local_log && inverse {
                        for value in &mut expected {
                            *value = goldilocks_mul(*value, normalization);
                        }
                    }
                    let (products, passes) =
                        local_general_stage_model(&mut actual, log, omega, normalization);
                    assert_eq!(
                        actual, expected,
                        "log={log} odd={odd} inverse={inverse} pattern={pattern}"
                    );
                    assert_eq!(passes, local_log as usize);
                    assert_eq!(products, len / 2 * local_log as usize);
                }
            }
        }
    }
}

#[test]
fn malformed_shapes_fail_before_device_or_private_staging() {
    let observed = ErasureObservation::begin();
    for (mut columns, log) in [
        (Vec::new(), 3),
        (vec![vec![1]], 0),
        (vec![vec![1; 8]; 9], 3),
        (vec![vec![1; 8], vec![1; 4]], 3),
        (vec![vec![1]], 33),
    ] {
        let original = columns.clone();
        assert!(transform(&mut columns, log, 7, false).is_err());
        assert_eq!(columns, original);
    }
    assert_eq!(observed.counts(), (0, 0));
}

#[test]
fn exact_root_payload_forecast_covers_actual_single_shared_backing() {
    let Some(device) = select_metal_device() else {
        return;
    };
    let _lane = crate::backend::acquire_gpu_lane();
    for log in [1, 8, 12, 16] {
        for columns in [1, 3, 8] {
            let original = source(log, columns);
            let mut buffer = PooledBuffer::from_columns(&original).unwrap();
            let staging_capacity = buffer.backing.pages.capacity() * METAL_BUFFER_PAGE_BYTES;
            let forecast =
                crate::goldilocks_transform::metal_goldilocks_transform_extra_payload_v1(
                    1 << log,
                    columns,
                )
                .unwrap();
            let private_and_pool =
                forecast - crate::gpu_memory::METAL_TWIDDLE_PAYLOAD_ALLOWANCE - (1 << 20);
            let idle_capacity = buffer_pool()
                .lock()
                .unwrap()
                .spare
                .iter()
                .map(|pages| pages.capacity() * METAL_BUFFER_PAGE_BYTES)
                .sum::<usize>();
            assert!(staging_capacity + idle_capacity <= private_and_pool);
            let (pointer, length) = buffer.metal_region();
            let metal = shared_pooled_buffer(&device, &mut buffer).unwrap();
            assert_eq!(metal.contents().cast_const(), pointer);
            assert_eq!(metal.length(), length);
            assert_eq!(buffer.len(), columns * (1 << log));
            assert!(buffer.backing.sensitive);
            for (column, expected) in original.iter().enumerate() {
                for row in [0, expected.len() - 1] {
                    assert_eq!(buffer.word(column * expected.len() + row), expected[row]);
                }
            }
        }
    }
}

#[test]
fn staged_dense_forward_and_inverse_match_cpu_across_tile_and_group_boundaries() {
    if select_metal_device().is_none() {
        return;
    }
    let _lane = crate::backend::acquire_gpu_lane();
    for log in [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 15] {
        for odd in [1, 3, 5] {
            let root = goldilocks_pow(goldilocks_pow(7, (FIELD_MODULUS - 1) >> log), odd);
            for inverse in [false, true] {
                let mut actual = source(log, 3);
                let mut expected = actual.clone();
                for column in &mut expected {
                    let domain = Domain {
                        log_size: log,
                        generator: root,
                    };
                    if inverse {
                        cyclotomic::ifft(column, domain);
                    } else {
                        cyclotomic::fft(column, domain);
                    }
                }
                transform(&mut actual, log, root, inverse).unwrap();
                assert_eq!(
                    actual, expected,
                    "log={log}, root_power={odd}, inverse={inverse}"
                );
            }
        }
    }
}

#[test]
fn success_failure_and_unwind_clear_real_staging_without_partial_publication() {
    if select_metal_device().is_none() {
        return;
    }
    let _lane = crate::backend::acquire_gpu_lane();
    let observed = ErasureObservation::begin();
    let log = 12;
    let root = goldilocks_pow(7, (FIELD_MODULUS - 1) >> log);
    let original = source(log, 2);
    let mut actual = original.clone();
    autoreleasepool(|| transform(&mut actual, log, root, false).unwrap());
    let after_success = observed.counts().0;
    assert!(after_success >= 8192);
    assert_eq!(observed.counts().1, 0);
    actual = original.clone();
    autoreleasepool(|| {
        let _failure = fail_column_batch_wait_after(0);
        assert!(transform(&mut actual, log, root, false).is_err());
    });
    assert_eq!(actual, original);
    assert!(observed.counts().0 >= after_success + 8192);
    assert_eq!(observed.counts().1, 0);
    assert!(!backend_quarantined());

    let weak = autoreleasepool(|| {
        let _scope = DrainScope::enter(METAL_COMMAND_TIMEOUT);
        let context = metal_context().unwrap();
        let twiddles = context
            .factorized_root_twiddle_buffer(log, root, false)
            .unwrap();
        let mut buffer = PooledBuffer::from_columns(&original).unwrap();
        let metal_buffer = shared_pooled_buffer(&context.device, &mut buffer).unwrap();
        let weak = buffer.weak_backing_for_tests();
        let ticket = submit(
            context,
            &metal_buffer,
            &twiddles,
            ExactRootFftArgs {
                column_len: 1 << log,
                normalization: 1,
                log_len: log,
                column_count: 2,
                stage: 0,
                padding: 0,
            },
        )
        .unwrap();
        let command = ticket.command.clone();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _pending = ColumnBatchTicket {
                range: 0..2,
                buffer,
                metal_buffer,
                tickets: smallvec::smallvec![ticket],
            };
            panic!("exercise staged exact-root unwind after actual submission");
        }));
        assert!(result.is_err());
        assert_eq!(command.status(), MTLCommandBufferStatus::Completed);
        drop(command);
        weak
    });
    assert!(weak.upgrade().is_none());
    assert_eq!(actual, original);
    assert_eq!(observed.counts().1, 0);
}

fn factorized_lookup(table: &[u64], exponent: u32) -> u64 {
    assert_eq!(table.len(), 4 * 256);
    let mut result = table[(exponent & 255) as usize];
    for digit in 1..4 {
        result = goldilocks_mul(
            result,
            table[digit * 256 + ((exponent >> (8 * digit)) & 255) as usize],
        );
    }
    result
}

// Independent sparse-factor model: the shader is exercised by the required
// device test below; this model also counts the eliminated identity products.
fn sparse_factorized_lookup(table: &[u64], exponent: u32) -> (u64, usize) {
    assert_eq!(table.len(), 4 * 256);
    let mut value = None;
    let mut multiplications = 0;
    for digit in 0..4 {
        let byte = ((exponent >> (8 * digit)) & 255) as usize;
        if byte != 0 {
            let factor = table[digit * 256 + byte];
            value = Some(value.map_or(factor, |previous| {
                multiplications += 1;
                goldilocks_mul(previous, factor)
            }));
        }
    }
    (value.unwrap_or(1), multiplications)
}

#[test]
fn sparse_public_twiddle_factors_match_dense_lookup_and_full_width_exponents() {
    for log in 1..=32 {
        for odd in [1, 3, 5] {
            let root = goldilocks_pow(goldilocks_pow(7, (FIELD_MODULUS - 1) >> log), odd);
            for inverse in [false, true] {
                let table = compute_factorized_root_twiddles(root, inverse);
                let omega = if inverse { goldilocks_inv(root) } else { root };
                let single_digits =
                    (0..4).flat_map(|digit| (0..256_u32).map(move |byte| byte << (8 * digit)));
                let mixed_digits = (0..1024_u32).map(|i| i.wrapping_mul(0x9e37_79b9));
                for exponent in
                    single_digits
                        .chain(mixed_digits)
                        .chain([u32::MAX, 0x0102_0304, 0x7fff_ffff])
                {
                    let (actual, multiplications) = sparse_factorized_lookup(&table, exponent);
                    assert_eq!(actual, factorized_lookup(&table, exponent));
                    assert_eq!(actual, goldilocks_pow(omega, u64::from(exponent)));
                    let nonzero_digits = exponent
                        .to_le_bytes()
                        .into_iter()
                        .filter(|byte| *byte != 0)
                        .count();
                    assert_eq!(multiplications, nonzero_digits.saturating_sub(1));
                }
            }
        }
    }
}

#[test]
fn factorized_public_powers_match_independent_exponentiation_for_all_orders_and_digits() {
    for log in 1..=32 {
        for odd in [1, 3, 5] {
            let root = goldilocks_pow(goldilocks_pow(7, (FIELD_MODULUS - 1) >> log), odd);
            for inverse in [false, true] {
                let table = compute_factorized_root_twiddles(root, inverse);
                assert_eq!(
                    table.len(),
                    crate::gpu_memory::METAL_FACTORIZED_TWIDDLE_WORDS
                );
                assert!(table.iter().all(|&word| word < FIELD_MODULUS));
                let omega = if inverse { goldilocks_inv(root) } else { root };
                for digit in 0..4 {
                    for byte in 0..256_u32 {
                        let exponent = byte << (8 * digit);
                        assert_eq!(
                            factorized_lookup(&table, exponent),
                            goldilocks_pow(omega, u64::from(exponent)),
                            "log={log} odd={odd} inverse={inverse} digit={digit} byte={byte}"
                        );
                    }
                }
                for exponent in [
                    0,
                    1,
                    255,
                    256,
                    65535,
                    65536,
                    0x0102_0304,
                    0x7fff_ffff,
                    u32::MAX,
                ] {
                    assert_eq!(
                        factorized_lookup(&table, exponent),
                        goldilocks_pow(omega, u64::from(exponent))
                    );
                }
            }
        }
    }
}

#[test]
fn factorized_stage_exponents_and_stride_preserve_every_public_schedule_boundary() {
    for log in 1..=32 {
        let root = goldilocks_pow(7, (FIELD_MODULUS - 1) >> log);
        for inverse in [false, true] {
            let table = compute_factorized_root_twiddles(root, inverse);
            let stages = compute_stage_twiddles(log, root, inverse);
            for stage in 0..log {
                let shift = log - stage - 1;
                let half = 1_u64 << stage;
                for offset in [0, 1, 127, 128, 255, 256, 2047, 2048, half / 2, half - 1]
                    .into_iter()
                    .filter(|&offset| offset < half)
                {
                    let exponent = offset << shift;
                    assert!(exponent < 1_u64 << 31);
                    assert_eq!(
                        factorized_lookup(&table, u32::try_from(exponent).unwrap()),
                        goldilocks_pow(stages[stage as usize], offset)
                    );
                }
                if stage >= LOCAL_LOG {
                    let exponent = LANES << shift;
                    assert!(exponent <= 1_u64 << 31);
                    assert_eq!(
                        factorized_lookup(&table, u32::try_from(exponent).unwrap()),
                        goldilocks_pow(stages[stage as usize], LANES)
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "requires an actual Metal device for bounded cache layout qualification"]
fn required_metal_stage_and_factorized_layouts_share_one_bounded_cache_without_aliasing() {
    let device = select_metal_device()
        .expect("required Metal cache qualification needs an actual Metal device");
    let _lane = crate::backend::acquire_gpu_lane();
    let mut cache = TwiddleCache::new();
    let log = 32;
    let root = goldilocks_pow(7, (FIELD_MODULUS - 1) >> log);
    let stage = cache.resolve(&device, log, root, false, false).unwrap();
    assert_eq!(stage.length(), 32 * 8);
    let powers = cache.resolve(&device, log, root, false, true).unwrap();
    assert_eq!(powers.length(), 4 * 256 * 8);
    assert_eq!(cache.buffers.len(), 2);
    drop(stage);
    drop(powers);
    // Distinct layouts and inverse directions must never reuse one another.
    cache.resolve(&device, log, root, true, true).unwrap();
    assert_eq!(cache.buffers.len(), 3);
    for odd in (3..).step_by(2).take(61) {
        cache
            .resolve(&device, log, goldilocks_pow(root, odd), false, true)
            .unwrap();
    }
    assert_eq!(cache.buffers.len(), GOLDILOCKS_TWIDDLE_CACHE_MAX_ENTRIES);
    let bytes: u64 = cache
        .buffers
        .values()
        .map(|entry| entry.buffer.length())
        .sum();
    assert!(bytes <= (64 * 4 * 256 * 8) as u64);
    cache
        .resolve(&device, log, goldilocks_pow(root, 127), false, true)
        .unwrap();
    assert_eq!(cache.buffers.len(), 1);
    assert_eq!(
        crate::gpu_memory::METAL_TWIDDLE_PAYLOAD_ALLOWANCE,
        66 * 4 * 256 * 8
    );
}

#[test]
#[ignore = "requires rebuilt authenticated Metal artifact and actual native19/common22 device parity"]
fn required_metal_sparse_public_twiddles_match_cpu_at_native_and_common_domains() {
    select_metal_device().expect("required twiddle parity needs an actual Metal device");
    let _lane = crate::backend::acquire_gpu_lane();
    for log in [19, 22] {
        for odd in [1, 3] {
            let root = goldilocks_pow(goldilocks_pow(7, (FIELD_MODULUS - 1) >> log), odd);
            for inverse in [false, true] {
                let mut actual = source(log, 1);
                let mut expected = actual.clone();
                let domain = Domain {
                    log_size: log,
                    generator: root,
                };
                if inverse {
                    cyclotomic::ifft(&mut expected[0], domain);
                } else {
                    cyclotomic::fft(&mut expected[0], domain);
                }
                let allocation = (actual[0].as_ptr(), actual[0].len(), actual[0].capacity());
                let started = std::time::Instant::now();
                transform(&mut actual, log, root, inverse).unwrap();
                let elapsed = started.elapsed();
                assert_eq!(
                    allocation,
                    (actual[0].as_ptr(), actual[0].len(), actual[0].capacity())
                );
                assert_eq!(
                    actual, expected,
                    "log={log}, root_power={odd}, inverse={inverse}"
                );
                assert!(!backend_quarantined());
                eprintln!(
                    "exact-root sparse public twiddles: log={log}, columns=1, root_power={odd}, inverse={inverse}, device_seconds={:.6}; byte parity only, no full-proof timing qualification",
                    elapsed.as_secs_f64()
                );
            }
        }
    }
}

#[test]
#[ignore = "actual Metal native19 repeated inverse cost; run alone in release mode"]
fn required_metal_native19_pool_retention_cost_matches_original_values() {
    // This benchmark compares the historical 16MiB cache ceiling with retained
    // 32MiB entries. It is not a production policy for oversized live loans.
    const HISTORICAL_16MIB_CACHE_BASELINE_PAGES: usize = (16 << 20) / METAL_BUFFER_PAGE_BYTES;

    assert!(!cfg!(debug_assertions), "run in release mode");
    select_metal_device().expect("native pool diagnostic requires actual Metal");
    let _lane = crate::backend::acquire_gpu_lane();
    let log = 19;
    let root = goldilocks_pow(7, (FIELD_MODULUS - 1) >> log);
    let original = source(log, 8);
    let mut expected = original.clone();
    for column in &mut expected {
        cyclotomic::ifft(
            column,
            Domain {
                log_size: log,
                generator: root,
            },
        );
    }
    let mut actual = original.clone();
    let mut elapsed = [Vec::new(), Vec::new()];
    let discard_new_entries = || {
        buffer_pool()
            .lock()
            .unwrap()
            .spare
            .retain(|pages| pages.capacity() <= HISTORICAL_16MIB_CACHE_BASELINE_PAGES);
    };
    for round in 0..6 {
        for retain in if round % 2 == 0 {
            [false, true]
        } else {
            [true, false]
        } {
            // Warm the same kernel and allocation before either policy. The old
            // policy then discards precisely the newly cacheable capacities.
            transform(&mut actual, log, root, true).unwrap();
            for (column, original) in actual.iter_mut().zip(&original) {
                column.copy_from_slice(original);
            }
            if !retain {
                discard_new_entries();
            }
            let started = std::time::Instant::now();
            transform(&mut actual, log, root, true).unwrap();
            if !retain {
                // The original policy freed this 32MiB staging on return. Keep
                // its equivalent final deallocation inside the measured span.
                discard_new_entries();
            }
            elapsed[usize::from(retain)].push(started.elapsed().as_secs_f64());
            assert_eq!(actual, expected);
            let pool = buffer_pool().lock().unwrap();
            assert!(
                pool.spare.iter().map(Vec::capacity).sum::<usize>() <= MAX_BUFFER_POOL_CACHED_PAGES
            );
        }
    }
    for timings in &mut elapsed {
        timings.sort_by(f64::total_cmp);
    }
    eprintln!(
        "native19 inverse eight-column pool diagnostic: old16MiB_median_seconds={:.6}, retained32MiB_median_seconds={:.6}, old={:?}, retained={:?}; exact CPU parity; includes staging, dispatch, wait, copyback and sensitive clearing; excludes source replay and warmup; component timing only",
        elapsed[0][elapsed[0].len() / 2],
        elapsed[1][elapsed[1].len() / 2],
        elapsed[0],
        elapsed[1]
    );
}

#[test]
fn phase_diagnostic_scope_is_opt_in_and_clears_after_unwind() {
    assert!(diagnostic_start().is_none());
    let result = std::panic::catch_unwind(|| {
        let _scope = PhaseDiagnosticScope::begin();
        assert!(diagnostic_start().is_some());
        panic!("exercise diagnostic scope unwind");
    });
    assert!(result.is_err());
    assert!(diagnostic_start().is_none());
    let scope = PhaseDiagnosticScope::begin();
    diagnostic_copy_back(diagnostic_start());
    let _record = scope.finish();
    assert!(diagnostic_start().is_none());
}

#[test]
#[ignore = "actual Metal phase timing with CPU parity; run alone in release mode"]
fn required_metal_exact_root_phase_latency_matches_cpu_with_original_owners() {
    exact_root_phase_latency_with_wait_strategies(
        &[ticket_lifetime::TestWaitStrategy::YieldThenNotify],
        12,
    );
}

#[test]
#[ignore = "actual Metal wait-strategy comparison with full CPU parity; run alone in release mode"]
fn required_metal_exact_root_active_wait_strategies_match_cpu_with_original_owners() {
    use ticket_lifetime::TestWaitStrategy;
    exact_root_phase_latency_with_wait_strategies(
        &[
            TestWaitStrategy::YieldThenNotify,
            TestWaitStrategy::ActiveOneMillisecond,
            TestWaitStrategy::ActiveTwoMilliseconds,
            TestWaitStrategy::ActiveSixteenMilliseconds,
        ],
        8,
    );
}

fn exact_root_phase_latency_with_wait_strategies(
    strategies: &[ticket_lifetime::TestWaitStrategy],
    rounds: usize,
) {
    use rayon::prelude::*;

    assert!(!cfg!(debug_assertions), "run in release mode");
    select_metal_device().expect("phase diagnostic requires actual Metal");
    let _lane = crate::backend::acquire_gpu_lane();
    struct Case {
        log: u32,
        columns: usize,
        root: u64,
        inverse: bool,
        original: Vec<Vec<u64>>,
        expected: Vec<Vec<u64>>,
        actual: Vec<Vec<u64>>,
    }
    let mut cases = [(19, 4), (19, 8), (22, 4), (22, 8)]
        .into_iter()
        .map(|(log, columns)| {
            let root = goldilocks_pow(7, (FIELD_MODULUS - 1) >> log);
            let inverse = log == 19;
            let original = source(log, columns);
            let mut expected = original.clone();
            expected.par_iter_mut().for_each(|column| {
                let domain = Domain {
                    log_size: log,
                    generator: root,
                };
                if inverse {
                    cyclotomic::ifft(column, domain);
                } else {
                    cyclotomic::fft(column, domain);
                }
            });
            let actual = original.clone();
            Case {
                log,
                columns,
                root,
                inverse,
                original,
                expected,
                actual,
            }
        })
        .collect::<Vec<_>>();
    if strategies.len() > 1 {
        // The controlled comparison excludes one-time pipeline/allocation
        // warmup for every public shape before any policy receives a sample.
        for case in &mut cases {
            transform(&mut case.actual, case.log, case.root, case.inverse).unwrap();
            assert_eq!(case.actual, case.expected);
        }
    }
    for round in 0..rounds {
        for index in if round % 2 == 0 {
            [0, 1, 2, 3]
        } else {
            [3, 2, 1, 0]
        } {
            let case = &mut cases[index];
            for strategy_offset in 0..strategies.len() {
                // Rotate and reverse so each policy occupies every position
                // twice across two rounds per policy.
                let strategy_index = if round % 2 == 0 {
                    (round / 2 + strategy_offset) % strategies.len()
                } else {
                    (round / 2 + strategies.len() - 1 - strategy_offset) % strategies.len()
                };
                let strategy = strategies[strategy_index];
                for (actual, original) in case.actual.iter_mut().zip(&case.original) {
                    actual.copy_from_slice(original);
                }
                let _strategy = ticket_lifetime::TestWaitStrategyScope::enter(strategy);
                let scope = PhaseDiagnosticScope::begin();
                let started = Instant::now();
                transform(&mut case.actual, case.log, case.root, case.inverse).unwrap();
                let outer = started.elapsed();
                let record = scope.finish();
                assert_eq!(case.actual, case.expected);
                assert!(record.gpu_seconds > 0.0);
                let cleanup = record
                    .wait_and_cleanup
                    .saturating_sub(record.ticket_wait + record.copy_back);
                let final_return = outer.saturating_sub(record.total);
                let ms = |duration: Duration| duration.as_secs_f64() * 1000.0;
                eprintln!(
                    "exact_root_phase log={} columns={} inverse={} round={} context_twiddle_ms={:.6} flatten_ms={:.6} mapping_ms={:.6} submit_ms={:.6} wait_cleanup_ms={:.6} ticket_wait_ms={:.6} copyback_ms={:.6} wait_owner_cleanup_ms={:.6} backing_drop_ms={:.6} final_return_ms={:.6} inner_total_ms={:.6} outer_total_ms={:.6} gpu_ms={:.6} kernel_ms={:.6} parity=exact wait_strategy={strategy:?} strategy_position={strategy_offset}",
                    case.log,
                    case.columns,
                    case.inverse,
                    round,
                    ms(record.context_twiddles),
                    ms(record.flatten),
                    ms(record.mapping),
                    ms(record.submit),
                    ms(record.wait_and_cleanup),
                    ms(record.ticket_wait),
                    ms(record.copy_back),
                    ms(cleanup),
                    ms(record.backing_drop),
                    ms(final_return),
                    ms(record.total),
                    ms(outer),
                    record.gpu_seconds * 1000.0,
                    record.kernel_seconds * 1000.0,
                );
            }
        }
    }
    eprintln!(
        "exact_root_phase_notes no_command_clone=true original_owner_drop_order=true gpu_timestamps_after_success_only=true private_input=false backing_drop_records_synchronous_thread_only=true final_return_includes_public_twiddle_and_drain_scope_drop=true wait_override_test_only=true shared_deadline_unchanged=true active_wait_replaces_yields=true active_wait_then_original_notification_fallback=true"
    );
}
