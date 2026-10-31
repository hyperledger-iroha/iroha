//! Transactional exact-root publication and original-allocation custody controls.

use super::*;
use crate::goldilocks_transform::{
    GoldilocksTransformBackendV1 as Backend, GoldilocksTransformDirectionV1 as Direction,
    GoldilocksTransformErrorV1 as Error, PreparedGoldilocksColumnsOutcomeV1 as Outcome,
    PreparedGoldilocksColumnsV1, prepare_goldilocks_columns_v1,
    prepared_goldilocks_columns_payload_bound_v1, prepared_goldilocks_forward_payload_bound_v1,
};
use crate::gpu_secret::ErasureObservation;

struct QuarantineScope(Option<bool>);
impl QuarantineScope {
    fn clear() -> Self {
        Self(TEST_QUARANTINE.with(|state| state.replace(Some(false))))
    }
}
impl Drop for QuarantineScope {
    fn drop(&mut self) {
        TEST_QUARANTINE.with(|state| state.set(self.0));
    }
}

fn columns(rows: usize, width: usize) -> Vec<Vec<u64>> {
    (0..width)
        .map(|column| {
            (0..rows)
                .map(|row| (row as u64 * 31 + column as u64 * 17 + 1) % FIELD_MODULUS)
                .collect()
        })
        .collect()
}

fn mock_output(rows: usize, width: usize) -> PreparedOutput {
    PreparedOutput {
        buffer: PooledBuffer::from_columns(&columns(rows, width)).unwrap(),
        rows,
        columns: width,
    }
}

#[test]
fn transactional_mock_publishes_every_word_without_moving_destinations() {
    let _quarantine = QuarantineScope::clear();
    for (rows, width) in [(2, 1), (8, 3), (4096, 8), (4096, 16)] {
        let observation = ErasureObservation::begin();
        let output = mock_output(rows, width);
        let expected = columns(rows, width);
        let bytes =
            output.backing_payload_bytes().unwrap() + mem::size_of::<PreparedGoldilocksColumnsV1>();
        assert!(bytes <= prepared_goldilocks_forward_payload_bound_v1(rows, width).unwrap());
        let cleared_words = output.buffer.backing.pages.capacity() * METAL_BUFFER_PAGE_WORDS;
        let mut target = vec![vec![19; rows]; width];
        let original_allocations = target
            .iter()
            .map(|column| (column.as_ptr(), column.capacity()))
            .collect::<Vec<_>>();
        output
            .publish_into(&mut target.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>())
            .unwrap();
        assert_eq!(target, expected);
        assert_eq!(
            target
                .iter()
                .map(|column| (column.as_ptr(), column.capacity()))
                .collect::<Vec<_>>(),
            original_allocations
        );
        drop(output);
        assert!(observation.counts().0 >= cleared_words);
        assert_eq!(observation.counts().1, 0);
    }
}

#[test]
fn transactional_mock_rejects_every_bad_shape_output_and_quarantine_before_writes() {
    let _quarantine = QuarantineScope::clear();
    for failure in 0..7 {
        let observation = ErasureObservation::begin();
        let mut output = mock_output(8, 3);
        let cleared_words = output.buffer.backing.pages.capacity() * METAL_BUFFER_PAGE_WORDS;
        let mut target = vec![vec![23; 8]; 3];
        let expected_error = match failure {
            0 => {
                target[2].pop();
                Error::InvalidShape
            }
            1 => {
                target.pop();
                Error::InvalidShape
            }
            2 => {
                output.rows = 4;
                Error::InvalidShape
            }
            3 => {
                output.columns = 4;
                Error::InvalidShape
            }
            4 => {
                Arc::get_mut(&mut output.buffer.backing).unwrap().pages[0].words[23] =
                    FIELD_MODULUS;
                Error::NonCanonicalOutput
            }
            5 => {
                Arc::get_mut(&mut output.buffer.backing)
                    .unwrap()
                    .logical_len = 23;
                Error::InvalidShape
            }
            _ => {
                TEST_QUARANTINE.with(|state| state.set(Some(true)));
                // Quarantine retains precedence over malformed output/shape.
                target[0].pop();
                Arc::get_mut(&mut output.buffer.backing).unwrap().pages[0].words[0] = FIELD_MODULUS;
                Error::CompletionUncertain
            }
        };
        let original = target.clone();
        assert_eq!(
            output.publish_into(&mut target.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>()),
            Err(expected_error)
        );
        assert_eq!(target, original);
        drop(output);
        assert!(observation.counts().0 >= cleared_words);
        assert_eq!(observation.counts().1, 0);
        TEST_QUARANTINE.with(|state| state.set(Some(false)));
    }
}

#[test]
fn transactional_mock_drop_and_unwind_erase_full_completed_backing() {
    let _quarantine = QuarantineScope::clear();
    for unwind in [false, true] {
        let observation = ErasureObservation::begin();
        let result = std::panic::catch_unwind(|| {
            let _output = mock_output(8, 3);
            assert!(!unwind, "drop an unpublished completed transform on unwind");
        });
        assert_eq!(result.is_err(), unwind);
        assert!(observation.counts().0 >= METAL_BUFFER_PAGE_WORDS);
        assert_eq!(observation.counts().1, 0);
    }
}

#[test]
fn transactional_device_preparation_preserves_inputs_until_checked_publication() {
    if select_metal_device().is_none() {
        return;
    }
    for log in [1, 8, 12] {
        for width in [1, 4, 8] {
            for direction in [Direction::Forward, Direction::Inverse] {
                check_device_case(log, width, direction);
            }
        }
    }
}

fn check_device_case(log: u32, width: usize, direction: Direction) {
    let rows = 1usize << log;
    let root = goldilocks_pow(7, (FIELD_MODULUS - 1) >> log);
    let original = columns(rows, width);
    let mut expected = original.clone();
    crate::goldilocks_transform::transform_goldilocks_columns_v1(
        &mut expected,
        root,
        direction,
        crate::ExecutionMode::Cpu,
    )
    .unwrap();
    let mut actual = original.clone();
    let allocations = actual
        .iter()
        .map(|column| (column.as_ptr(), column.capacity()))
        .collect::<Vec<_>>();
    let prepared = match prepare_goldilocks_columns_v1(
        &actual.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        root,
        direction,
        crate::ExecutionMode::Gpu,
    )
    .unwrap()
    {
        Outcome::Metal(output) => output,
        Outcome::Cpu => panic!("required Metal may not substitute CPU"),
    };
    assert_eq!(actual, original);
    assert_eq!(prepared.rows_v1(), rows);
    assert_eq!(prepared.columns_v1(), width);
    assert!(
        prepared.allocated_payload_bytes_v1().unwrap()
            <= prepared_goldilocks_columns_payload_bound_v1(rows, width).unwrap()
    );
    assert_eq!(
        prepared
            .publish_into_v1(&mut actual.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>())
            .unwrap(),
        Backend::Metal
    );
    assert_eq!(actual, expected);
    assert_eq!(
        actual
            .iter()
            .map(|column| (column.as_ptr(), column.capacity()))
            .collect::<Vec<_>>(),
        allocations
    );
}

#[test]
fn transactional_device_wait_failure_preserves_inputs() {
    if select_metal_device().is_none() {
        return;
    }
    let original = columns(256, 4);
    let views = original.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let root = goldilocks_pow(7, (FIELD_MODULUS - 1) >> 8);
    let observation = ErasureObservation::begin();
    {
        let _failure = fail_column_batch_wait_after(0);
        assert!(matches!(
            prepare_goldilocks_columns_v1(
                &views,
                root,
                Direction::Inverse,
                crate::ExecutionMode::Gpu
            ),
            Err(Error::DeviceFailure(_))
        ));
    }
    assert_eq!(original, columns(256, 4));
    assert!(observation.counts().0 >= METAL_BUFFER_PAGE_WORDS);
    assert_eq!(observation.counts().1, 0);
}

#[test]
#[ignore = "required actual Metal native19 transactional parity; run alone in release"]
fn required_metal_native19_transactional_inverse_matches_original_cpu_outputs() {
    assert!(!cfg!(debug_assertions), "use optimized build");
    select_metal_device().expect("actual Metal required");
    for width in [1, 4, 8] {
        check_device_case(19, width, Direction::Inverse);
    }
}

#[test]
fn transactional_wide_forward_refuses_last_column_faults_before_any_publication() {
    let _quarantine = QuarantineScope::clear();
    for fault in 0..3 {
        let observation = ErasureObservation::begin();
        let mut output = mock_output(8, 16);
        let mut target = vec![vec![23; 8]; 16];
        let expected = match fault {
            0 => {
                target[15].pop();
                Error::InvalidShape
            }
            1 => {
                Arc::get_mut(&mut output.buffer.backing).unwrap().pages[0].words[127] =
                    FIELD_MODULUS;
                Error::NonCanonicalOutput
            }
            _ => {
                TEST_QUARANTINE.with(|state| state.set(Some(true)));
                Error::CompletionUncertain
            }
        };
        let before = target.clone();
        assert_eq!(
            output.publish_into(&mut target.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>()),
            Err(expected)
        );
        assert_eq!(target, before);
        drop(output);
        assert!(observation.counts().0 > 0);
        assert_eq!(observation.counts().1, 0);
        TEST_QUARANTINE.with(|state| state.set(Some(false)));
    }
}
