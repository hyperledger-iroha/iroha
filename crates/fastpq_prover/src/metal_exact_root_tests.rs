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

#[test]
fn host_arguments_and_global_butterfly_partition_match_kernel_contract() {
    assert_eq!(mem::size_of::<ExactRootFftArgs>(), 32);
    assert_eq!(mem::offset_of!(ExactRootFftArgs, log_len), 16);
    for log in 9..=15 {
        let rows = 1_u64 << log;
        let root = goldilocks_pow(7, (FIELD_MODULUS - 1) >> log);
        for stage in LOCAL_LOG..log {
            let half = 1_u64 << stage;
            let step = goldilocks_pow(root, rows >> (stage + 1));
            let stride = goldilocks_pow(step, LANES);
            let mut visits = vec![0_u8; rows as usize];
            for group in 0..(rows / 2).div_ceil(BUTTERFLIES_PER_GROUP) {
                for lane in 0..LANES {
                    let mut butterfly = group * BUTTERFLIES_PER_GROUP + lane;
                    let first = goldilocks_pow(step, butterfly & (half - 1));
                    let mut twiddle = first;
                    for _ in 0..8 {
                        if butterfly >= rows / 2 {
                            break;
                        }
                        let offset = butterfly & (half - 1);
                        let low = (butterfly - offset) * 2 + offset;
                        assert!(low + half < rows);
                        visits[low as usize] += 1;
                        visits[(low + half) as usize] += 1;
                        assert_eq!(twiddle, goldilocks_pow(step, offset));
                        twiddle = if offset + LANES >= half {
                            first
                        } else {
                            goldilocks_mul(twiddle, stride)
                        };
                        butterfly += LANES;
                    }
                }
            }
            assert!(visits.iter().all(|visits| *visits == 1));
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
fn staged_dense_forward_and_inverse_match_cpu_across_tile_and_group_boundaries() {
    if select_metal_device().is_none() {
        return;
    }
    let _lane = crate::backend::acquire_gpu_lane();
    for log in [1, 2, 7, 8, 9, 10, 11, 12, 15] {
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
                        factorized_lookup(&table, exponent as u32),
                        goldilocks_pow(stages[stage as usize], offset)
                    );
                }
                if stage >= LOCAL_LOG {
                    let exponent = LANES << shift;
                    assert!(exponent <= 1_u64 << 31);
                    assert_eq!(
                        factorized_lookup(&table, exponent as u32),
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
