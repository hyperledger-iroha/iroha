//! Public-fixture native-prefix staging candidate; production preparation is unchanged.

use super::*;
use crate::goldilocks_transform::{
    GoldilocksTransformBackendV1 as Backend, GoldilocksTransformDirectionV1 as Direction,
    GoldilocksTransformErrorV1 as Error, PreparedGoldilocksColumnsOutcomeV1 as Outcome,
    prefix_tests::{prefix_product_v1, prepare_prefixes_v1, validate_prefixes_v1},
    prepared_goldilocks_columns_payload_bound_v1, transform_goldilocks_columns_v1,
};
use crate::gpu_secret::ErasureObservation;
use zeroize::Zeroize as _;

/// One sensitive, fully zeroed backing; multiplication writes only public prefixes.
/// Inputs have already passed complete shape and canonicality validation.
fn stage_validated_prefixes_v1(
    prefixes: &[&[u64]],
    rows: usize,
    powers: &[u64],
) -> MetalResult<PooledBuffer> {
    let words = rows
        .checked_mul(prefixes.len())
        .ok_or(GpuError::InvalidInput("prefix extent overflow"))?;
    let mut buffer = PooledBuffer::sensitive_zeroed(words)?;
    let backing = Arc::get_mut(&mut buffer.backing).expect("new prefix backing is unique");
    if rows >= METAL_BUFFER_PAGE_WORDS {
        // rows is a power of two, so independent full columns have disjoint pages.
        backing
            .pages
            .par_chunks_mut(rows / METAL_BUFFER_PAGE_WORDS)
            .zip(prefixes.par_iter())
            .for_each(|(pages, prefix)| {
                for (index, page) in pages
                    .iter_mut()
                    .take(prefix.len().div_ceil(METAL_BUFFER_PAGE_WORDS))
                    .enumerate()
                {
                    let first = index * METAL_BUFFER_PAGE_WORDS;
                    let count = (prefix.len() - first).min(METAL_BUFFER_PAGE_WORDS);
                    for ((destination, coefficient), power) in page.words[..count]
                        .iter_mut()
                        .zip(&prefix[first..first + count])
                        .zip(&powers[first..first + count])
                    {
                        *destination = prefix_product_v1(*coefficient, *power);
                    }
                }
            });
    } else {
        // Tiny test domains may share pages; each word still has exactly one writer.
        for (column, prefix) in prefixes.iter().enumerate() {
            for (row, (&coefficient, &power)) in prefix.iter().zip(powers).enumerate() {
                let index = column * rows + row;
                backing.pages[index / METAL_BUFFER_PAGE_WORDS].words
                    [index % METAL_BUFFER_PAGE_WORDS] = prefix_product_v1(coefficient, power);
            }
        }
    }
    Ok(buffer)
}

/// Copy of the original bounded preparation around a different staging constructor.
/// The facade validated all prefixes and powers before entering this trusted seam.
pub(crate) fn prepare_validated_prefixes_v1(
    prefixes: &[&[u64]],
    log_size: u32,
    root: u64,
    powers: &[u64],
) -> MetalResult<PreparedColumns> {
    let _drain_scope = DrainScope::enter(METAL_COMMAND_TIMEOUT);
    ensure_backend_available()?;
    let extent = goldilocks_domain_len(log_size)?;
    if log_size == 0
        || prefixes.is_empty()
        || prefixes.len() > 8
        || prefixes
            .iter()
            .any(|column| column.is_empty() || column.len() > extent || column.len() > powers.len())
    {
        return Err(GpuError::InvalidInput("invalid bounded prefix FFT shape"));
    }
    let column_count = u32::try_from(prefixes.len())
        .map_err(|_| GpuError::InvalidInput("prefix count overflow"))?;
    let words = extent
        .checked_mul(prefixes.len())
        .ok_or(GpuError::InvalidInput("prefix staging extent overflow"))?;
    let diagnostic_context = diagnostic_start();
    let context = metal_context()?;
    for pipeline in [
        &context.exact_root_bit_reverse,
        &context.exact_root_local_tiles,
        &context.exact_root_global_stage,
    ] {
        if pipeline.max_total_threads_per_threadgroup() < LANES {
            return Err(GpuError::Unsupported(GpuBackend::Metal));
        }
    }
    validate_metal_pooled_word_len(&context.device, words)?;
    let twiddles = context.factorized_root_twiddle_buffer(log_size, root, false)?;
    diagnostic_record(diagnostic_context, |record| &mut record.context_twiddles);
    let args = ExactRootFftArgs {
        column_len: extent as u64,
        normalization: 1,
        log_len: log_size,
        column_count,
        stage: 0,
        padding: 0,
    };
    let diagnostic_flatten = diagnostic_start();
    let mut buffer = stage_validated_prefixes_v1(prefixes, extent, powers)?;
    diagnostic_record(diagnostic_flatten, |record| &mut record.flatten);
    let diagnostic_mapping = diagnostic_start();
    let metal_buffer = shared_pooled_buffer(&context.device, &mut buffer)?;
    diagnostic_record(diagnostic_mapping, |record| &mut record.mapping);
    let diagnostic_submit = diagnostic_start();
    let ticket = submit(context, &metal_buffer, &twiddles, args)?;
    diagnostic_record(diagnostic_submit, |record| &mut record.submit);
    let diagnostic_wait = diagnostic_start();
    let completed = ColumnBatchTicket {
        range: 0..prefixes.len(),
        buffer,
        metal_buffer,
        tickets: smallvec::smallvec![ticket],
    }
    .complete(true)?;
    diagnostic_record(diagnostic_wait, |record| &mut record.wait_and_cleanup);
    let CompletedColumnBatch {
        range,
        buffer,
        _metal_buffer,
    } = completed;
    debug_assert_eq!(range, 0..prefixes.len());
    Ok(PreparedColumns {
        output: PreparedOutput {
            buffer,
            rows: extent,
            columns: prefixes.len(),
        },
        _metal_buffer,
    })
}

struct ClearingWords(Vec<Vec<u64>>);
impl Drop for ClearingWords {
    fn drop(&mut self) {
        for column in &mut self.0 {
            // Full initialized capacity is cleared, including unused allocation tail.
            column.resize(column.capacity(), 0);
            column.as_mut_slice().zeroize();
        }
    }
}

fn public_fixture(lengths: &[usize]) -> ClearingWords {
    ClearingWords(
        lengths
            .iter()
            .enumerate()
            .map(|(column, &len)| {
                (0..len)
                    .map(|row| {
                        if row % 7 == 0 {
                            FIELD_MODULUS - 1 - column as u64
                        } else {
                            (row as u64 * 31 + column as u64 * 17 + 1) % FIELD_MODULUS
                        }
                    })
                    .collect()
            })
            .collect(),
    )
}

fn public_powers(len: usize) -> Vec<u64> {
    let mut value = 1;
    (0..len)
        .map(|_| {
            let result = value;
            value = goldilocks_mul(value, 7);
            result
        })
        .collect()
}

fn cpu_oracle(prefixes: &[Vec<u64>], rows: usize, root: u64, powers: &[u64]) -> ClearingWords {
    let mut expected = ClearingWords(vec![vec![0; rows]; prefixes.len()]);
    for (output, prefix) in expected.0.iter_mut().zip(prefixes) {
        for ((destination, &coefficient), &power) in output.iter_mut().zip(prefix).zip(powers) {
            // Independent, simple arithmetic oracle for the fused diagonal.
            *destination = goldilocks_mul(coefficient, power);
        }
    }
    transform_goldilocks_columns_v1(
        &mut expected.0,
        root,
        Direction::Forward,
        crate::ExecutionMode::Cpu,
    )
    .unwrap();
    expected
}

fn run_arm(
    prefixes: &[Vec<u64>],
    rows: usize,
    root: u64,
    powers: &[u64],
    direct: bool,
    device_width: usize,
) -> ClearingWords {
    let views = prefixes.iter().map(Vec::as_slice).collect::<Vec<_>>();
    // Match main evaluator's batch-wide validation before any private staging.
    validate_prefixes_v1(&views, rows, root, powers).unwrap();
    let mut actual = ClearingWords(vec![vec![0; rows]; prefixes.len()]);
    if direct {
        for (input, output) in views
            .chunks(device_width)
            .zip(actual.0.chunks_mut(device_width))
        {
            let prepared =
                match prepare_prefixes_v1(input, rows, root, powers, crate::ExecutionMode::Gpu)
                    .unwrap()
                {
                    Outcome::Metal(owner) => owner,
                    Outcome::Cpu => panic!("required Metal cannot substitute CPU"),
                };
            assert!(
                prepared.allocated_payload_bytes_v1().unwrap()
                    <= prepared_goldilocks_columns_payload_bound_v1(rows, input.len()).unwrap()
            );
            let mut destinations = output.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>();
            assert_eq!(
                prepared.publish_into_v1(&mut destinations).unwrap(),
                Backend::Metal
            );
        }
    } else {
        actual
            .0
            .par_iter_mut()
            .zip(prefixes.par_iter())
            .for_each(|(output, prefix)| {
                for ((destination, &coefficient), &power) in
                    output.iter_mut().zip(prefix).zip(powers)
                {
                    *destination = prefix_product_v1(coefficient, power);
                }
            });
        for batch in actual.0.chunks_mut(device_width) {
            assert_eq!(
                transform_goldilocks_columns_v1(
                    batch,
                    root,
                    Direction::Forward,
                    crate::ExecutionMode::Gpu
                )
                .unwrap(),
                Backend::Metal
            );
        }
    }
    actual
}

#[test]
fn native_prefix_staging_matches_oracle_and_clears_all_padding_on_drop_and_unwind() {
    for (rows, lengths) in [
        (8, vec![1, 7, 8]),
        (4096, vec![17, 2049, 3864]),
        (8192, vec![4097; 8]),
    ] {
        let source = public_fixture(&lengths);
        let powers = public_powers(*lengths.iter().max().unwrap());
        let views = source.0.iter().map(Vec::as_slice).collect::<Vec<_>>();
        for unwind in [false, true] {
            let observed = ErasureObservation::begin();
            let result = std::panic::catch_unwind(|| {
                let staged = stage_validated_prefixes_v1(&views, rows, &powers).unwrap();
                for (column, prefix) in source.0.iter().enumerate() {
                    for row in 0..rows {
                        let expected = prefix
                            .get(row)
                            .map_or(0, |&value| goldilocks_mul(value, powers[row]));
                        assert_eq!(staged.word(column * rows + row), expected);
                    }
                }
                let words = rows * source.0.len();
                for index in words..staged.backing.pages.len() * METAL_BUFFER_PAGE_WORDS {
                    assert_eq!(
                        staged.backing.pages[index / METAL_BUFFER_PAGE_WORDS].words
                            [index % METAL_BUFFER_PAGE_WORDS],
                        0
                    );
                }
                assert!(!unwind, "exercise sensitive staging unwind");
            });
            assert_eq!(result.is_err(), unwind);
            assert!(
                observed.counts().0
                    >= metal_buffer_page_count(rows * lengths.len()) * METAL_BUFFER_PAGE_WORDS
            );
            assert_eq!(observed.counts().1, 0);
        }
    }
}

#[test]
fn native_prefix_required_metal_preserves_inputs_full_parity_and_wait_refusal() {
    if select_metal_device().is_none() {
        return;
    }
    for (log, lengths) in [(3, vec![1, 7, 8]), (12, vec![2048 + 1816; 8])] {
        let source = public_fixture(&lengths);
        let original = source.0.clone();
        let powers = public_powers(*lengths.iter().max().unwrap());
        let rows = 1 << log;
        let root = goldilocks_pow(7, (FIELD_MODULUS - 1) >> log);
        let expected = cpu_oracle(&source.0, rows, root, &powers);
        for width in [4, 8] {
            let actual = run_arm(&source.0, rows, root, &powers, true, width);
            assert_eq!(actual.0, expected.0);
            assert_eq!(source.0, original);
        }
        let views = source.0.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let observed = ErasureObservation::begin();
        {
            let _failure = fail_column_batch_wait_after(0);
            assert!(matches!(
                prepare_prefixes_v1(&views, rows, root, &powers, crate::ExecutionMode::Gpu),
                Err(Error::DeviceFailure(_))
            ));
        }
        {
            let _failure = fail_column_batch_wait_after(0);
            assert!(matches!(
                prepare_prefixes_v1(&views, rows, root, &powers, crate::ExecutionMode::Auto),
                Ok(Outcome::Cpu)
            ));
        }
        assert_eq!(source.0, original);
        assert!(
            observed.counts().0
                >= metal_buffer_page_count(rows * source.0.len()) * METAL_BUFFER_PAGE_WORDS
        );
        assert_eq!(observed.counts().1, 0);
    }
}

#[test]
fn native_prefix_refusal_preserves_destinations_and_full_backing_erasure() {
    struct Quarantine(Option<bool>);
    impl Drop for Quarantine {
        fn drop(&mut self) {
            TEST_QUARANTINE.with(|state| state.set(self.0));
        }
    }
    let _quarantine = Quarantine(TEST_QUARANTINE.with(|state| state.replace(Some(false))));
    let source = public_fixture(&[17, 4095]);
    let powers = public_powers(4095);
    let views = source.0.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let root = goldilocks_pow(7, (FIELD_MODULUS - 1) >> 12);
    for fault in 0..3 {
        let observed = ErasureObservation::begin();
        let mut output = PreparedOutput {
            buffer: stage_validated_prefixes_v1(&views, 4096, &powers).unwrap(),
            rows: 4096,
            columns: 2,
        };
        let mut destinations = ClearingWords(vec![vec![23; 4096]; 2]);
        let expected = match fault {
            0 => {
                destinations.0[1].pop();
                Error::InvalidShape
            }
            1 => {
                Arc::get_mut(&mut output.buffer.backing).unwrap().pages[3].words
                    [METAL_BUFFER_PAGE_WORDS - 1] = FIELD_MODULUS;
                Error::NonCanonicalOutput
            }
            _ => {
                TEST_QUARANTINE.with(|state| state.set(Some(true)));
                Error::CompletionUncertain
            }
        };
        assert_eq!(
            output.publish_into(
                &mut destinations
                    .0
                    .iter_mut()
                    .map(Vec::as_mut_slice)
                    .collect::<Vec<_>>()
            ),
            Err(expected)
        );
        assert!(
            destinations
                .0
                .iter()
                .all(|column| column.iter().all(|&word| word == 23))
        );
        drop(output);
        assert!(observed.counts().0 >= 8192);
        assert_eq!(observed.counts().1, 0);
        TEST_QUARANTINE.with(|state| state.set(Some(false)));
    }
    TEST_QUARANTINE.with(|state| state.set(Some(true)));
    assert!(matches!(
        prepare_prefixes_v1(&views, 4096, root, &powers, crate::ExecutionMode::Auto),
        Err(Error::CompletionUncertain)
    ));
    assert!(matches!(
        prepare_prefixes_v1(&views, 4096, root, &powers, crate::ExecutionMode::Gpu),
        Err(Error::CompletionUncertain)
    ));
}

#[test]
#[ignore = "common22 native-prefix complete-cost A/B, actual Metal, optimized build, run alone"]
fn required_metal_common22_native_prefix_complete_cost() {
    assert!(!cfg!(debug_assertions), "use optimized build");
    select_metal_device().expect("actual Metal required");
    let rows = 1 << 22;
    let root = goldilocks_pow(7, (FIELD_MODULUS - 1) >> 22);
    let configurations = [(false, 4), (true, 4), (false, 8), (true, 8)];
    // Every configuration occupies each position twice, with reversed adjacency.
    let orders = [
        [0, 1, 2, 3],
        [1, 2, 3, 0],
        [2, 3, 0, 1],
        [3, 0, 1, 2],
        [3, 2, 1, 0],
        [0, 3, 2, 1],
        [1, 0, 3, 2],
        [2, 1, 0, 3],
    ];
    for native_log in [15, 16, 18, 19] {
        let prefix_len = (1 << native_log) + 1816;
        let source = public_fixture(&[prefix_len; 8]);
        let powers = public_powers((1 << 19) + 1816);
        // Fixed public reference residency is present for every arm. Its construction
        // and CPU FFT are outside timing, with no oracle work between timed arms.
        let expected = cpu_oracle(&source.0, rows, root, &powers);
        for &(direct, width) in &configurations {
            let output = run_arm(&source.0, rows, root, &powers, direct, width);
            assert_eq!(output.0, expected.0);
        }
        let mut samples: [Vec<f64>; 4] = core::array::from_fn(|_| Vec::new());
        for (trial, order) in orders.iter().enumerate() {
            for &arm in order {
                let (direct, width) = configurations[arm];
                let scope = PhaseDiagnosticScope::begin();
                let start = Instant::now();
                let actual = run_arm(&source.0, rows, root, &powers, direct, width);
                // Complete output parity and canonicality are in both timing scopes.
                assert_eq!(actual.0, expected.0);
                assert!(
                    actual
                        .0
                        .par_iter()
                        .all(|column| crate::goldilocks_transform::canonical_word_slice_v1(column))
                );
                drop(actual); // Full final-output capacity erase and free are timed.
                let elapsed = start.elapsed();
                let phases = scope.finish();
                samples[arm].push(elapsed.as_secs_f64());
                eprintln!(
                    "PREFIX_COST native_log={native_log} common_log=22 prefix_len={prefix_len} mask_tail=1816 source_width=8 device_width={width} direct={direct} trial={trial} wall_s={:.9} staging_s={:.9} mapping_s={:.9} wait_s={:.9} copyback_s={:.9} backing_clear_s={:.9} gpu_s={:.9} cpu_oracle_bytes={} source_bytes={} output_bytes={} pooled_bytes={}",
                    elapsed.as_secs_f64(),
                    phases.flatten.as_secs_f64(),
                    phases.mapping.as_secs_f64(),
                    phases.ticket_wait.as_secs_f64(),
                    phases.copy_back.as_secs_f64(),
                    phases.backing_drop.as_secs_f64(),
                    phases.gpu_seconds,
                    rows * 8 * 8,
                    prefix_len * 8 * 8,
                    rows * 8 * 8,
                    rows * width * 8
                );
            }
        }
        for (arm, sample) in samples.iter_mut().enumerate() {
            sample.sort_by(f64::total_cmp);
            let median = (sample[3] + sample[4]) / 2.0;
            eprintln!(
                "PREFIX_MEDIAN native_log={native_log} direct={} device_width={} samples=8 full_cost_s={median:.9}",
                configurations[arm].0, configurations[arm].1
            );
        }
    }
}
