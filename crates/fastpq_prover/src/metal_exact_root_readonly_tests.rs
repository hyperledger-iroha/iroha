//! Complete immutable-output lifetime, prefix arithmetic and component-cost controls.

use super::*;
use crate::goldilocks_transform::{
    CompletedGoldilocksColumnsV1, GoldilocksTransformBackendV1 as Backend,
    GoldilocksTransformDirectionV1 as Direction, GoldilocksTransformErrorV1 as Error,
    PreparedGoldilocksColumnsOutcomeV1 as Outcome, prepare_goldilocks_prefix_columns_v1,
    prepared_goldilocks_columns_payload_bound_v1, transform_goldilocks_columns_v1,
};
use crate::gpu_secret::ErasureObservation;
use zeroize::Zeroize as _;

struct Quarantine(Option<bool>);
impl Quarantine {
    fn clear() -> Self {
        Self(TEST_QUARANTINE.with(|state| state.replace(Some(false))))
    }
}
impl Drop for Quarantine {
    fn drop(&mut self) {
        TEST_QUARANTINE.with(|state| state.set(self.0));
    }
}

struct Words(Vec<Vec<u64>>);
impl Drop for Words {
    fn drop(&mut self) {
        for column in &mut self.0 {
            column.resize(column.capacity(), 0);
            column.as_mut_slice().zeroize();
        }
    }
}
fn fixture(lengths: &[usize]) -> Words {
    Words(
        lengths
            .iter()
            .enumerate()
            .map(|(column, &rows)| {
                (0..rows)
                    .map(|row| {
                        if row % 7 == 0 {
                            FIELD_MODULUS - 1 - column as u64
                        } else {
                            1 + row as u64 * 31 + column as u64 * 17
                        }
                    })
                    .collect()
            })
            .collect(),
    )
}
fn powers(rows: usize) -> Vec<u64> {
    let mut value = 1;
    (0..rows)
        .map(|_| {
            let result = value;
            value = goldilocks_mul(value, 7);
            result
        })
        .collect()
}
fn oracle(source: &Words, rows: usize, root: u64, powers: &[u64]) -> Words {
    let mut result = Words(vec![vec![0; rows]; source.0.len()]);
    for (target, source) in result.0.iter_mut().zip(&source.0) {
        for ((target, &value), &power) in target.iter_mut().zip(source).zip(powers) {
            *target = goldilocks_mul(value, power); // Independent u128 remainder.
        }
    }
    transform_goldilocks_columns_v1(
        &mut result.0,
        root,
        Direction::Forward,
        crate::ExecutionMode::Cpu,
    )
    .unwrap();
    result
}

#[test]
fn readonly_mock_lifetime_shares_all_fields_and_erases_only_after_last_owner() {
    let _quarantine = Quarantine::clear();
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<CompletedGoldilocksColumnsV1>();
    assert_send_sync::<CompletedReadColumns>();
    for (rows, width) in [(2, 1), (8, 3), (4096, 8)] {
        let expected = fixture(&vec![rows; width]);
        let observed = ErasureObservation::begin();
        let output = PreparedOutput {
            buffer: PooledBuffer::from_columns(&expected.0).unwrap(),
            rows,
            columns: width,
        };
        let backing_pointer = output.buffer.backing.pages.as_ptr().cast::<u64>();
        let backing = Arc::downgrade(&output.buffer.backing);
        let completed = Arc::new(output.into_readonly().unwrap());
        assert_eq!(completed.column(0).unwrap().as_ptr(), backing_pointer);
        assert_eq!(completed.rows(), rows);
        assert_eq!(completed.columns(), width);
        assert_eq!(completed.column(width), None);
        let clones = [Arc::clone(&completed), Arc::clone(&completed)];
        expected
            .0
            .par_iter()
            .enumerate()
            .for_each(|(column, values)| {
                assert_eq!(completed.column(column).unwrap(), values);
            });
        drop(completed);
        assert!(backing.upgrade().is_some());
        assert_eq!(observed.counts().0, 0);
        drop(clones);
        assert!(backing.upgrade().is_none());
        assert!(
            observed.counts().0 >= metal_buffer_page_count(rows * width) * METAL_BUFFER_PAGE_WORDS
        );
        assert_eq!(observed.counts().1, 0);
    }
}

#[test]
fn readonly_mock_refuses_entire_bad_output_and_quarantine_then_erases_on_error_or_unwind() {
    let _quarantine = Quarantine::clear();
    for fault in 0..4 {
        let observed = ErasureObservation::begin();
        let mut output = PreparedOutput {
            buffer: PooledBuffer::from_columns(&fixture(&[4096; 4]).0).unwrap(),
            rows: 4096,
            columns: 4,
        };
        let expected = match fault {
            0 => {
                Arc::get_mut(&mut output.buffer.backing).unwrap().pages[7].words[2047] =
                    FIELD_MODULUS;
                Error::NonCanonicalOutput
            }
            1 => {
                output.rows = 2048;
                Error::InvalidShape
            }
            2 => {
                output.columns = 3;
                Error::InvalidShape
            }
            _ => {
                TEST_QUARANTINE.with(|state| state.set(Some(true)));
                output.rows = 3;
                Error::CompletionUncertain
            }
        };
        assert_eq!(output.into_readonly().err(), Some(expected));
        assert!(observed.counts().0 >= 16384);
        assert_eq!(observed.counts().1, 0);
        TEST_QUARANTINE.with(|state| state.set(Some(false)));
    }
    let observed = ErasureObservation::begin();
    assert!(
        std::panic::catch_unwind(|| {
            let output = PreparedOutput {
                buffer: PooledBuffer::from_columns(&fixture(&[8; 3]).0).unwrap(),
                rows: 8,
                columns: 3,
            };
            let _completed = Arc::new(output.into_readonly().unwrap());
            panic!("exercise immutable owner unwind");
        })
        .is_err()
    );
    assert!(observed.counts().0 >= METAL_BUFFER_PAGE_WORDS);
    assert_eq!(observed.counts().1, 0);
}

#[test]
fn readonly_prefix_complete_mask_tail_and_scalar_products_are_validated() {
    let rows = 4096;
    let root = goldilocks_pow(7, (FIELD_MODULUS - 1) >> 12);
    let source = fixture(&[2048 + 1816; 8]);
    let powers = powers(2048 + 1816);
    let mut bad = fixture(&[2048 + 1816; 8]);
    *bad.0[7].last_mut().unwrap() = FIELD_MODULUS;
    for columns in [&source.0, &bad.0] {
        let views = columns.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let actual = prepare_goldilocks_prefix_columns_v1(
            &views,
            rows,
            root,
            &powers,
            crate::ExecutionMode::Cpu,
        );
        if std::ptr::eq(columns, &source.0) {
            assert!(matches!(actual, Ok(Outcome::Cpu)));
        } else {
            assert_eq!(actual.err(), Some(Error::NonCanonicalInput));
        }
    }
    for left in [0, 1, 7, u32::MAX as u64, 1 << 32, FIELD_MODULUS - 1] {
        for right in [0, 1, 7, u32::MAX as u64, 1 << 32, FIELD_MODULUS - 1] {
            assert_eq!(prefix_product_v1(left, right), goldilocks_mul(left, right));
        }
    }
    let views = source.0.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let staged = stage_validated_prefixes_v1(&views, rows, &powers).unwrap();
    for (column, source) in source.0.iter().enumerate() {
        for row in 0..rows {
            assert_eq!(
                staged.word(column * rows + row),
                source
                    .get(row)
                    .map_or(0, |&value| goldilocks_mul(value, powers[row]))
            );
        }
    }
}

#[test]
fn readonly_required_metal_releases_lane_before_publication_and_preserves_originals() {
    if select_metal_device().is_none() {
        return;
    }
    let rows = 4096;
    let root = goldilocks_pow(7, (FIELD_MODULUS - 1) >> 12);
    for width in [1, 4, 8] {
        let source = fixture(&vec![2048 + 1816; width]);
        let powers = powers(2048 + 1816);
        let expected = oracle(&source, rows, root, &powers);
        let views = source.0.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let observed = ErasureObservation::begin();
        let prepared = match prepare_goldilocks_prefix_columns_v1(
            &views,
            rows,
            root,
            &powers,
            crate::ExecutionMode::Gpu,
        )
        .unwrap()
        {
            Outcome::Metal(owner) => owner,
            Outcome::Cpu => panic!("required Metal"),
        };
        let completed = prepared.into_readonly_v1().unwrap();
        assert_eq!(completed.rows_v1(), rows);
        assert_eq!(completed.columns_v1(), width);
        assert!(
            completed.allocated_payload_bytes_v1().unwrap()
                <= prepared_goldilocks_columns_payload_bound_v1(rows, width).unwrap()
        );
        assert!(
            crate::backend::try_acquire_gpu_lane().is_some(),
            "immutable output must not hold the lane"
        );
        for (index, column) in expected.0.iter().enumerate() {
            assert_eq!(completed.column_v1(index).unwrap(), column);
        }
        assert!(completed.column_v1(width).is_none());
        assert_eq!(source.0, fixture(&vec![2048 + 1816; width]).0);
        drop(completed);
        assert!(observed.counts().0 >= rows * width);
        assert_eq!(observed.counts().1, 0);
        {
            let _failure = fail_column_batch_wait_after(0);
            assert!(matches!(
                prepare_goldilocks_prefix_columns_v1(
                    &views,
                    rows,
                    root,
                    &powers,
                    crate::ExecutionMode::Gpu
                ),
                Err(Error::DeviceFailure(_))
            ));
        }
        assert_eq!(source.0, fixture(&vec![2048 + 1816; width]).0);
    }
}

// Original padded input, direct-prefix copy publication, or readonly completed backing.
enum ArmOutput {
    Words(Words),
    Completed(Vec<CompletedGoldilocksColumnsV1>),
}
impl ArmOutput {
    /// Match successful initial coalescer cleanup: parallel independent final owners.
    fn clear_after_use(self) {
        match self {
            Self::Words(mut words) => {
                let columns = mem::take(&mut words.0);
                columns.into_par_iter().for_each(|mut column| {
                    column.resize(column.capacity(), 0);
                    column.as_mut_slice().zeroize();
                });
            }
            Self::Completed(owners) => owners.into_par_iter().for_each(drop),
        }
    }
    fn verify(&self, expected: &Words) {
        match self {
            Self::Words(values) => assert_eq!(values.0, expected.0),
            Self::Completed(owners) => {
                let mut column = 0;
                for owner in owners {
                    for index in 0..owner.columns_v1() {
                        assert_eq!(owner.column_v1(index).unwrap(), expected.0[column]);
                        column += 1;
                    }
                }
                assert_eq!(column, expected.0.len());
            }
        }
    }
}
fn arm(
    source: &Words,
    rows: usize,
    root: u64,
    powers: &[u64],
    mode: usize,
    width: usize,
) -> ArmOutput {
    let views = source.0.iter().map(Vec::as_slice).collect::<Vec<_>>();
    assert!(matches!(
        prepare_goldilocks_prefix_columns_v1(&views, rows, root, powers, crate::ExecutionMode::Cpu)
            .unwrap(),
        Outcome::Cpu
    ));
    if mode == 2 {
        let mut outputs = Vec::new();
        for input in views.chunks(width) {
            let prepared = match prepare_goldilocks_prefix_columns_v1(
                input,
                rows,
                root,
                powers,
                crate::ExecutionMode::Gpu,
            )
            .unwrap()
            {
                Outcome::Metal(owner) => owner,
                Outcome::Cpu => panic!("required Metal"),
            };
            outputs.push(prepared.into_readonly_v1().unwrap());
        }
        return ArmOutput::Completed(outputs);
    }
    let mut output = Words(vec![vec![0; rows]; source.0.len()]);
    if mode == 0 {
        output
            .0
            .par_iter_mut()
            .zip(&source.0)
            .for_each(|(output, input)| {
                for ((target, &value), &power) in output.iter_mut().zip(input).zip(powers) {
                    *target = prefix_product_v1(value, power);
                }
            });
        for batch in output.0.chunks_mut(width) {
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
    } else {
        for (input, output) in views.chunks(width).zip(output.0.chunks_mut(width)) {
            let prepared = match prepare_goldilocks_prefix_columns_v1(
                input,
                rows,
                root,
                powers,
                crate::ExecutionMode::Gpu,
            )
            .unwrap()
            {
                Outcome::Metal(owner) => owner,
                Outcome::Cpu => panic!("required Metal"),
            };
            prepared
                .publish_into_v1(&mut output.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>())
                .unwrap();
        }
    }
    ArmOutput::Words(output)
}

#[test]
#[ignore = "real common22 immutable output complete component cost, optimized build, run alone"]
fn required_metal_common22_readonly_prefix_complete_cost() {
    assert!(!cfg!(debug_assertions));
    select_metal_device().expect("actual Metal required");
    let rows = 1 << 22;
    let root = goldilocks_pow(7, (FIELD_MODULUS - 1) >> 22);
    let settings = [(0, 4), (1, 4), (2, 4), (0, 8), (1, 8), (2, 8)];
    for native_log in [15, 16, 18, 19] {
        let prefix_len = (1 << native_log) + 1816;
        let source = fixture(&[prefix_len; 8]);
        let powers = powers((1 << 19) + 1816);
        let expected = oracle(&source, rows, root, &powers);
        for &(mode, width) in &settings {
            arm(&source, rows, root, &powers, mode, width).verify(&expected);
        }
        let mut samples: [Vec<f64>; 6] = core::array::from_fn(|_| Vec::new());
        // Each setting occupies each position once in each direction.
        for reverse in [false, true] {
            for rotation in 0..6 {
                for position in 0..6 {
                    let index = if reverse {
                        (rotation + 5 - position) % 6
                    } else {
                        (rotation + position) % 6
                    };
                    let (mode, width) = settings[index];
                    let scope = PhaseDiagnosticScope::begin();
                    let start = Instant::now();
                    let output = arm(&source, rows, root, &powers, mode, width);
                    output.verify(&expected); // Same complete independent CPU parity scan.
                    let clear_start = Instant::now();
                    output.clear_after_use(); // All final allocated pages are erased in timing.
                    let final_clear = clear_start.elapsed().as_secs_f64();
                    let elapsed = start.elapsed().as_secs_f64();
                    let phases = scope.finish();
                    samples[index].push(elapsed);
                    eprintln!(
                        "READONLY_COST native_log={native_log} common_log=22 mask_tail=1816 source_width=8 device_width={width} mode={mode} reverse={reverse} rotation={rotation} full_s={elapsed:.9} staging_s={:.9} mapping_s={:.9} wait_s={:.9} copyback_s={:.9} same_thread_staging_clear_s={:.9} final_output_clear_s={final_clear:.9} gpu_s={:.9} reference_bytes={} output_bytes={} active_staging_bytes={}",
                        phases.flatten.as_secs_f64(),
                        phases.mapping.as_secs_f64(),
                        phases.ticket_wait.as_secs_f64(),
                        phases.copy_back.as_secs_f64(),
                        phases.backing_drop.as_secs_f64(),
                        phases.gpu_seconds,
                        rows * 8 * 8,
                        rows * 8 * 8,
                        if mode == 2 { 0 } else { rows * width * 8 }
                    );
                }
            }
        }
        for (index, values) in samples.iter_mut().enumerate() {
            values.sort_by(f64::total_cmp);
            let median = (values[5] + values[6]) / 2.0;
            eprintln!(
                "READONLY_MEDIAN native_log={native_log} mode={} device_width={} samples=12 full_s={median:.9}",
                settings[index].0, settings[index].1
            );
        }
    }
}
