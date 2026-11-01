//! Explicit completed-success page erasure, retained ownership and lifecycle costs.

use super::*;
use crate::gpu_secret::ErasureObservation;
use rayon::prelude::*;

fn fixture_v1(pages: usize, tail: usize, sensitive: bool) -> PooledBuffer {
    assert!(pages > 0 && tail < METAL_BUFFER_PAGE_WORDS);
    let mut values = Vec::new();
    values.try_reserve_exact(pages).unwrap();
    assert_eq!(values.capacity(), pages);
    values.resize_with(pages, MetalBufferPage::zeroed);
    for (page, values) in values.iter_mut().enumerate() {
        for (word, value) in values.words.iter_mut().enumerate() {
            *value = 1 + page as u64 * 17 + word as u64 * 31;
        }
    }
    PooledBuffer {
        backing: Arc::new(PooledBufferBacking {
            pages: values,
            logical_len: pages * METAL_BUFFER_PAGE_WORDS - tail,
            sensitive,
        }),
    }
}

#[test]
fn completed_erase_preserves_full_padding_exact_capacity_and_initialized_reuse() {
    for threads in [1, 4] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                for pages in [
                    1,
                    COMPLETED_ERASE_MIN_PAGES_V1 - 1,
                    COMPLETED_ERASE_MIN_PAGES_V1 + 3,
                ] {
                    let buffer = fixture_v1(pages, 13, true);
                    let pointer = buffer.backing.pages.as_ptr();
                    let mut backing = Arc::try_unwrap(buffer.backing).ok().unwrap();
                    let observed = ErasureObservation::begin();
                    let previous_calls =
                        COMPLETED_ERASE_PARALLEL_CALLS_V1.with(|calls| calls.get());
                    backing.wipe_sensitive_pages_with_v1(true);
                    assert_eq!(
                        COMPLETED_ERASE_PARALLEL_CALLS_V1.with(|calls| calls.get())
                            - previous_calls,
                        usize::from(threads > 1 && pages >= COMPLETED_ERASE_MIN_PAGES_V1)
                    );
                    assert_eq!(observed.counts(), (pages * METAL_BUFFER_PAGE_WORDS, 0));
                    assert_eq!(backing.pages.as_ptr(), pointer);
                    assert_eq!(backing.pages.len(), pages);
                    assert_eq!(backing.pages.capacity(), pages);
                    assert!(
                        backing
                            .pages
                            .iter()
                            .all(|page| page.words.iter().all(|&word| word == 0))
                    );
                    // Reuse this exact initialized vector through an isolated
                    // pool, including padding that was nonzero before erasure.
                    let values = mem::take(&mut backing.pages);
                    let mut pool = BufferPool::default();
                    pool.recycle(values);
                    let mut values = pool.take(pages).unwrap();
                    assert_eq!(values.as_ptr(), pointer);
                    assert_eq!(values.len(), pages);
                    values.resize_with(pages, || panic!("completed pages must stay initialized"));
                    assert!(
                        values
                            .iter()
                            .all(|page| page.words.iter().all(|&word| word == 0))
                    );
                    assert_eq!(observed.counts(), (pages * METAL_BUFFER_PAGE_WORDS, 0));
                }
            });
    }
}

#[test]
fn completed_erase_retained_backing_is_untouched_until_its_original_final_drop() {
    let pages = COMPLETED_ERASE_MIN_PAGES_V1 + 1;
    let buffer = fixture_v1(pages, 7, true);
    let retained = Arc::clone(&buffer.backing);
    let observed = ErasureObservation::begin();
    buffer.erase_completed_v1();
    assert_eq!(observed.counts(), (0, 0));
    assert_eq!(Arc::strong_count(&retained), 1);
    assert!(
        retained
            .pages
            .iter()
            .all(|page| page.words.iter().all(|&word| word != 0))
    );
    drop(retained);
    assert_eq!(observed.counts(), (pages * METAL_BUFFER_PAGE_WORDS, 0));
}

#[test]
fn completed_erase_consumes_exactly_one_shared_final_owner() {
    for width in [1, 4, 8, 16] {
        let buffer = fixture_v1(COMPLETED_ERASE_MIN_PAGES_V1, 11, true);
        let weak = Arc::downgrade(&buffer.backing);
        let shared = Arc::new(buffer);
        let handles = (0..width).map(|_| Arc::clone(&shared)).collect::<Vec<_>>();
        drop(shared);
        let consumed = std::sync::atomic::AtomicUsize::new(0);
        handles.into_par_iter().for_each(|owner| {
            if let Some(buffer) = Arc::into_inner(owner) {
                consumed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                // Observe on the initiating worker, after its page tasks join.
                let observed = ErasureObservation::begin();
                buffer.erase_completed_v1();
                assert_eq!(
                    observed.counts(),
                    (COMPLETED_ERASE_MIN_PAGES_V1 * METAL_BUFFER_PAGE_WORDS, 0)
                );
            }
        });
        assert_eq!(consumed.load(std::sync::atomic::Ordering::Relaxed), 1);
        assert!(weak.upgrade().is_none());
    }
}

#[test]
fn completed_erase_unwind_keeps_sequential_cleanup_and_clears_all_pages() {
    struct ConsumeDuringUnwind(Option<PooledBuffer>);
    impl Drop for ConsumeDuringUnwind {
        fn drop(&mut self) {
            assert!(std::thread::panicking());
            self.0.take().unwrap().erase_completed_v1();
        }
    }
    let pages = COMPLETED_ERASE_MIN_PAGES_V1 + 1;
    let observed = ErasureObservation::begin();
    let previous_calls = COMPLETED_ERASE_PARALLEL_CALLS_V1.with(|calls| calls.get());
    assert!(
        std::panic::catch_unwind(|| {
            let _owner = ConsumeDuringUnwind(Some(fixture_v1(pages, 17, true)));
            panic!("exercise explicit cleanup during unwind");
        })
        .is_err()
    );
    assert_eq!(observed.counts(), (pages * METAL_BUFFER_PAGE_WORDS, 0));
    assert_eq!(
        COMPLETED_ERASE_PARALLEL_CALLS_V1.with(|calls| calls.get()),
        previous_calls
    );
}

#[test]
fn completed_erase_public_backing_keeps_original_zero_initialization_contract() {
    let mut backing = Arc::try_unwrap(fixture_v1(3, 1, false).backing)
        .ok()
        .unwrap();
    let observed = ErasureObservation::begin();
    backing.wipe_sensitive_pages_with_v1(true);
    assert_eq!(observed.counts(), (0, 0));
    assert!(
        backing
            .pages
            .iter()
            .all(|page| page.words.iter().all(|&word| word != 0))
    );
    let values = backing.take_recyclable_pages();
    assert!(values.is_empty());
    assert_eq!(values.capacity(), 3);
    let mut pool = BufferPool::default();
    pool.recycle(values);
    let mut values = pool.take(3).unwrap();
    values.resize_with(3, MetalBufferPage::zeroed);
    assert!(
        values
            .iter()
            .all(|page| page.words.iter().all(|&word| word == 0))
    );
}

#[test]
#[ignore = "complete 32/128/256 MiB pooled acquire, write, read, erase lifecycle; optimized build"]
fn completed_erase_complete_lifecycle_cost() {
    assert!(!cfg!(debug_assertions));
    for threads in [1, 4, 8] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                for bytes in [32usize << 20, 128 << 20, 256 << 20] {
                    let words = bytes / mem::size_of::<u64>();
                    let run = |parallel: bool| {
                        let start = Instant::now();
                        let mut buffer = PooledBuffer::sensitive_zeroed(words - 13).unwrap();
                        let backing = Arc::get_mut(&mut buffer.backing).unwrap();
                        assert_eq!(backing.pages.capacity() * METAL_BUFFER_PAGE_BYTES, bytes);
                        backing.pages.par_iter_mut().enumerate().for_each(|(index, page)| {
                            page.words.fill(1 + index as u64);
                        });
                        // Equal full scan in both arms, including dirty padding.
                        assert!(backing.pages.par_iter().enumerate().all(|(index, page)| {
                            page.words.iter().all(|&value| value == 1 + index as u64)
                        }));
                        let clear = Instant::now();
                        if parallel {
                            buffer.erase_completed_v1();
                        } else {
                            drop(buffer);
                        }
                        (start.elapsed().as_secs_f64(), clear.elapsed().as_secs_f64())
                    };
                    run(false);
                    run(true);
                    let mut samples: [Vec<f64>; 2] = core::array::from_fn(|_| Vec::new());
                    // Twelve observations per arm; each arm starts six pairs.
                    for round in 0..12 {
                        for position in 0..2 {
                            let arm = (round + position) % 2;
                            let (full, clear) = run(arm == 1);
                            samples[arm].push(full);
                            eprintln!("COMPLETED_ERASE_COST threads={threads} bytes={bytes} round={round} arm={arm} full_s={full:.9} erase_s={clear:.9}");
                        }
                    }
                    for (arm, values) in samples.iter_mut().enumerate() {
                        values.sort_by(f64::total_cmp);
                        let median = (values[5] + values[6]) / 2.0;
                        eprintln!("COMPLETED_ERASE_MEDIAN threads={threads} bytes={bytes} arm={arm} samples=12 full_s={median:.9}");
                    }
                }
            });
    }
}

#[test]
fn completed_readonly_erase_public_owner_releases_lane_then_clears_exact_output() {
    use crate::goldilocks_transform::{
        PreparedGoldilocksColumnsOutcomeV1 as Outcome, prepare_goldilocks_prefix_columns_v1,
    };
    if select_metal_device().is_none() {
        return;
    }
    let rows = 4096;
    let root = goldilocks_pow(7, (FIELD_MODULUS - 1) >> 12);
    let source = zeroize::Zeroizing::new(vec![vec![7; 4096]; 8]);
    let views = source.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let powers = vec![1; rows];
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
    assert!(crate::backend::try_acquire_gpu_lane().is_some());
    for index in 0..8 {
        let column = completed.column_v1(index).unwrap();
        assert_eq!(column[0], 7 * rows as u64);
        assert!(column[1..].iter().all(|&value| value == 0));
    }
    completed.erase_completed_v1();
    assert_eq!(observed.counts(), (rows * 8, 0));
}

#[test]
#[ignore = "actual common22 prefix FFT, full independent parity and completed erasure; optimized build"]
fn required_metal_completed_readonly_erase_complete_cost() {
    use crate::goldilocks_transform::{
        GoldilocksTransformDirectionV1 as Direction, PreparedGoldilocksColumnsOutcomeV1 as Outcome,
        prepare_goldilocks_prefix_columns_v1, transform_goldilocks_columns_v1,
    };
    assert!(!cfg!(debug_assertions));
    select_metal_device().expect("actual Metal required");
    const ROWS: usize = 1 << 22;
    const PREFIX: usize = (1 << 19) + 1816;
    let root = goldilocks_pow(7, (FIELD_MODULUS - 1) >> 22);
    let source = zeroize::Zeroizing::new(
        (0..8)
            .map(|column| {
                (0..PREFIX)
                    .map(|row| 1 + column * 17 + row as u64 * 31)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>(),
    );
    let mut power = 1;
    let powers = (0..PREFIX)
        .map(|_| {
            let old = power;
            power = goldilocks_mul(power, 7);
            old
        })
        .collect::<Vec<_>>();
    let views = source.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let mut expected = zeroize::Zeroizing::new(vec![vec![0; ROWS]; 8]);
    for (output, input) in expected.iter_mut().zip(source.iter()) {
        for ((value, &input), &power) in output.iter_mut().zip(input).zip(&powers) {
            *value = goldilocks_mul(input, power);
        }
    }
    transform_goldilocks_columns_v1(
        &mut expected,
        root,
        Direction::Forward,
        crate::ExecutionMode::Cpu,
    )
    .unwrap();
    let run = |parallel: bool| {
        let start = Instant::now();
        let prepared = match prepare_goldilocks_prefix_columns_v1(
            &views,
            ROWS,
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
        for (index, expected) in expected.iter().enumerate() {
            assert_eq!(completed.column_v1(index).unwrap(), expected);
        }
        let clear = Instant::now();
        if parallel {
            completed.erase_completed_v1();
        } else {
            drop(completed);
        }
        (start.elapsed().as_secs_f64(), clear.elapsed().as_secs_f64())
    };
    run(false);
    run(true);
    let mut samples: [Vec<f64>; 2] = core::array::from_fn(|_| Vec::new());
    for round in 0..12 {
        for position in 0..2 {
            let arm = (round + position) % 2;
            let (full, erase) = run(arm == 1);
            samples[arm].push(full);
            eprintln!(
                "COMPLETED_READONLY_ERASE_COST round={round} arm={arm} rows={ROWS} columns=8 prefix={PREFIX} full_s={full:.9} erase_s={erase:.9}"
            );
        }
    }
    for (arm, values) in samples.iter_mut().enumerate() {
        values.sort_by(f64::total_cmp);
        let median = (values[5] + values[6]) / 2.0;
        eprintln!("COMPLETED_READONLY_ERASE_MEDIAN arm={arm} samples=12 full_s={median:.9}");
    }
}
