//! Completed-output scan controls and production validation parity.
//!
//! Every candidate only borrows the original completed pages. There is no field
//! conversion, new private buffer, unchecked access, or change to publication.

use super::*;

const SCAN_TILE_PAGES: usize = 8;

#[derive(Clone, Copy, Debug)]
enum ScanMode {
    FlatMapAny,
    PageAny,
    PageMaximum,
    PageInvalidFold,
    ParallelPageAny,
    ParallelPageInvalidFold,
}

impl ScanMode {
    const ALL: [Self; 6] = [
        Self::FlatMapAny,
        Self::PageAny,
        Self::PageMaximum,
        Self::PageInvalidFold,
        Self::ParallelPageAny,
        Self::ParallelPageInvalidFold,
    ];

    fn parallel(self) -> bool {
        matches!(self, Self::ParallelPageAny | Self::ParallelPageInvalidFold)
    }

    fn scan(self, pages: &[MetalBufferPage], words: usize) -> bool {
        assert!(words <= pages.len() * METAL_BUFFER_PAGE_WORDS);
        match self {
            Self::FlatMapAny => !pages
                .iter()
                .flat_map(|page| page.words.iter())
                .take(words)
                .any(|&word| word >= FIELD_MODULUS),
            Self::PageAny => scan_pages::<false>(pages, words, page_any),
            Self::PageMaximum => scan_pages::<false>(pages, words, page_maximum),
            Self::PageInvalidFold => scan_pages::<false>(pages, words, page_invalid_fold),
            Self::ParallelPageAny => scan_pages::<true>(pages, words, page_any),
            Self::ParallelPageInvalidFold => scan_pages::<true>(pages, words, page_invalid_fold),
        }
    }

    fn run(self, pages: &[MetalBufferPage], words: usize, pool: &rayon::ThreadPool) -> bool {
        if self.parallel() {
            pool.install(|| self.scan(pages, words))
        } else {
            self.scan(pages, words)
        }
    }

    fn scan_columns(self, columns: &[Vec<u64>]) -> bool {
        match self {
            Self::FlatMapAny => !columns.iter().flatten().any(|&word| word >= FIELD_MODULUS),
            Self::PageAny => scan_column_tiles::<false>(columns, page_any),
            Self::PageMaximum => scan_column_tiles::<false>(columns, page_maximum),
            Self::PageInvalidFold => scan_column_tiles::<false>(columns, page_invalid_fold),
            Self::ParallelPageAny => scan_column_tiles::<true>(columns, page_any),
            Self::ParallelPageInvalidFold => scan_column_tiles::<true>(columns, page_invalid_fold),
        }
    }

    fn run_columns(self, columns: &[Vec<u64>], pool: &rayon::ThreadPool) -> bool {
        if self.parallel() {
            pool.install(|| self.scan_columns(columns))
        } else {
            self.scan_columns(columns)
        }
    }
}

fn page_any(words: &[u64]) -> bool {
    words.iter().all(|&word| word < FIELD_MODULUS)
}

fn page_maximum(words: &[u64]) -> bool {
    words.iter().copied().fold(0, u64::max) < FIELD_MODULUS
}

fn page_invalid_fold(words: &[u64]) -> bool {
    words.iter().fold(0_u64, |invalid, &word| {
        invalid | u64::from(word >= FIELD_MODULUS)
    }) == 0
}

fn scan_pages<const PARALLEL: bool>(
    pages: &[MetalBufferPage],
    words: usize,
    page: impl Fn(&[u64]) -> bool + Sync,
) -> bool {
    let full = words / METAL_BUFFER_PAGE_WORDS;
    let tail = words % METAL_BUFFER_PAGE_WORDS;
    let complete = if PARALLEL {
        pages[..full]
            .par_chunks(SCAN_TILE_PAGES)
            .all(|tile| tile.iter().all(|value| page(&value.words)))
    } else {
        pages[..full].iter().all(|value| page(&value.words))
    };
    complete && (tail == 0 || page(&pages[full].words[..tail]))
}

fn scan_column_tiles<const PARALLEL: bool>(
    columns: &[Vec<u64>],
    page: impl Fn(&[u64]) -> bool + Sync,
) -> bool {
    if PARALLEL {
        columns.par_iter().all(|column| {
            column
                .par_chunks(SCAN_TILE_PAGES * METAL_BUFFER_PAGE_WORDS)
                .all(&page)
        })
    } else {
        columns
            .iter()
            .all(|column| column.chunks(METAL_BUFFER_PAGE_WORDS).all(&page))
    }
}

fn scan_fixture(words: usize) -> PooledBuffer {
    let mut buffer = PooledBuffer::sensitive_zeroed(words).unwrap();
    let backing = Arc::get_mut(&mut buffer.backing).unwrap();
    for (index, word) in backing
        .pages
        .iter_mut()
        .flat_map(|page| page.words.iter_mut())
        .enumerate()
    {
        *word = if index >= words {
            // Logical padding is excluded from all six scans, even when its
            // representation would be rejected as an actual output residue.
            u64::MAX
        } else {
            [0, 1, FIELD_MODULUS - 1, 37][index % 4]
        };
    }
    buffer
}

#[test]
fn completed_output_scan_candidates_preserve_all_small_positions_and_page_tile_tail_boundaries() {
    for workers in [1, 4, 20] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        pool.broadcast(|_| {});
        for words in [
            0,
            2,
            67,
            METAL_BUFFER_PAGE_WORDS - 1,
            METAL_BUFFER_PAGE_WORDS,
            METAL_BUFFER_PAGE_WORDS + 1,
            SCAN_TILE_PAGES * METAL_BUFFER_PAGE_WORDS + 3,
        ] {
            let mut buffer = scan_fixture(words);
            let allocation = (
                buffer.backing.pages.as_ptr(),
                buffer.backing.pages.capacity(),
            );
            for mode in ScanMode::ALL {
                assert!(
                    mode.run(&buffer.backing.pages, words, &pool),
                    "{mode:?}, words={words}"
                );
            }
            assert!(completed_output_is_canonical_v1(
                &buffer.backing.pages,
                words
            ));
            let positions = if words <= 67 {
                (0..words).collect::<Vec<_>>()
            } else {
                let mut positions = vec![0, words - 1];
                for boundary in (METAL_BUFFER_PAGE_WORDS..words).step_by(METAL_BUFFER_PAGE_WORDS) {
                    positions.extend([boundary - 1, boundary]);
                    if boundary + 1 < words {
                        positions.push(boundary + 1);
                    }
                }
                positions.sort_unstable();
                positions.dedup();
                positions
            };
            for position in positions {
                let page = position / METAL_BUFFER_PAGE_WORDS;
                let offset = position % METAL_BUFFER_PAGE_WORDS;
                let original = buffer.backing.pages[page].words[offset];
                for invalid in [FIELD_MODULUS, u64::MAX] {
                    Arc::get_mut(&mut buffer.backing).unwrap().pages[page].words[offset] = invalid;
                    for mode in ScanMode::ALL {
                        assert!(
                            !mode.run(&buffer.backing.pages, words, &pool),
                            "{mode:?}, words={words}, position={position}, workers={workers}"
                        );
                    }
                    assert!(!completed_output_is_canonical_v1(
                        &buffer.backing.pages,
                        words
                    ));
                }
                Arc::get_mut(&mut buffer.backing).unwrap().pages[page].words[offset] = original;
            }
            assert_eq!(
                (
                    buffer.backing.pages.as_ptr(),
                    buffer.backing.pages.capacity()
                ),
                allocation
            );
        }
    }
}

#[test]
fn input_column_scan_candidates_preserve_every_small_position_and_tile_boundaries() {
    for workers in [1, 4, 20] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        pool.broadcast(|_| {});
        for rows in [0, 67, SCAN_TILE_PAGES * METAL_BUFFER_PAGE_WORDS + 3] {
            let mut columns = vec![vec![FIELD_MODULUS - 1; rows]; 3];
            let positions = if rows <= 67 {
                (0..rows).collect::<Vec<_>>()
            } else {
                vec![
                    0,
                    METAL_BUFFER_PAGE_WORDS - 1,
                    METAL_BUFFER_PAGE_WORDS,
                    SCAN_TILE_PAGES * METAL_BUFFER_PAGE_WORDS - 1,
                    SCAN_TILE_PAGES * METAL_BUFFER_PAGE_WORDS,
                    rows - 1,
                ]
            };
            for mode in ScanMode::ALL {
                assert!(mode.run_columns(&columns, &pool));
            }
            for column in 0..columns.len() {
                for &position in &positions {
                    for invalid in [FIELD_MODULUS, u64::MAX] {
                        columns[column][position] = invalid;
                        for mode in ScanMode::ALL {
                            assert!(
                                !mode.run_columns(&columns, &pool),
                                "{mode:?}, rows={rows}, column={column}, position={position}"
                            );
                        }
                    }
                    columns[column][position] = FIELD_MODULUS - 1;
                }
            }
        }
    }
}

#[test]
#[ignore = "isolated completed-output scan A/B with production-helper parity"]
fn completed_output_scan_cost_preserves_real_extents_without_private_staging() {
    use std::{hint::black_box, time::Instant};

    const ROUNDS: usize = 12;
    assert!(!cfg!(debug_assertions), "use an optimized build");
    assert!(
        rayon::current_thread_index().is_none(),
        "use the external caller"
    );
    for workers in [1, 4, 20] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        pool.broadcast(|context| {
            assert_eq!(rayon::current_num_threads(), workers);
            assert_eq!(rayon::current_thread_index(), Some(context.index()));
        });
        for (log, width) in [(19, 1), (19, 4), (19, 8), (22, 4), (22, 8)] {
            let words = (1_usize << log) * width;
            let mut buffer = scan_fixture(words);
            pool.install(|| {
                assert!(completed_output_is_canonical_v1(
                    &buffer.backing.pages,
                    words
                ));
            });
            let final_page = (words - 1) / METAL_BUFFER_PAGE_WORDS;
            let final_offset = (words - 1) % METAL_BUFFER_PAGE_WORDS;
            let original = buffer.backing.pages[final_page].words[final_offset];
            Arc::get_mut(&mut buffer.backing).unwrap().pages[final_page].words[final_offset] =
                FIELD_MODULUS;
            pool.install(|| {
                assert!(!completed_output_is_canonical_v1(
                    &buffer.backing.pages,
                    words
                ));
            });
            Arc::get_mut(&mut buffer.backing).unwrap().pages[final_page].words[final_offset] =
                original;
            let allocation = (
                buffer.backing.pages.as_ptr(),
                buffer.backing.pages.capacity(),
            );
            for mode in ScanMode::ALL {
                assert!(mode.run(&buffer.backing.pages, words, &pool));
            }
            let mut samples = [[0.0_f64; ROUNDS]; ScanMode::ALL.len()];
            for round in 0..ROUNDS {
                for position in 0..ScanMode::ALL.len() {
                    let index = if round % 2 == 0 {
                        (round / 2 + position) % ScanMode::ALL.len()
                    } else {
                        (round / 2 + ScanMode::ALL.len() - 1 - position) % ScanMode::ALL.len()
                    };
                    let mode = ScanMode::ALL[index];
                    let started = Instant::now();
                    let valid = black_box(mode.run(black_box(&buffer.backing.pages), words, &pool));
                    samples[index][round] = started.elapsed().as_secs_f64();
                    assert!(valid);
                }
            }
            assert_eq!(
                (
                    buffer.backing.pages.as_ptr(),
                    buffer.backing.pages.capacity()
                ),
                allocation
            );
            for (index, mode) in ScanMode::ALL.into_iter().enumerate() {
                let mean = samples[index].iter().sum::<f64>() / ROUNDS as f64;
                samples[index].sort_by(f64::total_cmp);
                let median = (samples[index][ROUNDS / 2 - 1] + samples[index][ROUNDS / 2]) / 2.0;
                eprintln!(
                    "exact_root_canonical_scan mode={mode:?} log={log} width={width} words={words} bytes={} workers={workers} caller_context=external tile_pages={SCAN_TILE_PAGES} rounds={ROUNDS} mean_ms={:.6} median_ms={:.6} allocation=unchanged parity=all_words; includes scan and parallel pool entry; excludes fixture allocation/fill/clear and worker startup; component timing only",
                    words * mem::size_of::<u64>(),
                    mean * 1000.0,
                    median * 1000.0,
                );
            }
        }
    }
}

#[test]
#[ignore = "isolated borrowed input-column scan A/B with production-helper parity"]
fn input_column_scan_cost_preserves_real_extents_without_private_staging() {
    use std::{hint::black_box, time::Instant};

    const ROUNDS: usize = 12;
    assert!(!cfg!(debug_assertions), "use an optimized build");
    assert!(
        rayon::current_thread_index().is_none(),
        "use the external caller"
    );
    for workers in [1, 4, 20] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        pool.broadcast(|context| {
            assert_eq!(rayon::current_num_threads(), workers);
            assert_eq!(rayon::current_thread_index(), Some(context.index()));
        });
        for (log, width) in [(19, 1), (19, 4), (19, 8), (22, 4), (22, 8)] {
            let rows = 1_usize << log;
            let words = rows * width;
            // Public fixtures stand in for original independently owned inputs.
            // The timed scan borrows these exact allocations without staging.
            let mut columns = vec![vec![FIELD_MODULUS - 1; rows]; width];
            let root = goldilocks_pow(7, (FIELD_MODULUS - 1) >> log);
            let validate = |columns: &[Vec<u64>]| {
                crate::goldilocks_transform::prepare_goldilocks_columns_v1(
                    &columns.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                    root,
                    crate::goldilocks_transform::GoldilocksTransformDirectionV1::Forward,
                    crate::ExecutionMode::Cpu,
                )
            };
            pool.install(|| assert!(validate(&columns).is_ok()));
            columns[width - 1][rows - 1] = FIELD_MODULUS;
            pool.install(|| {
                assert!(matches!(
                    validate(&columns),
                    Err(crate::goldilocks_transform::GoldilocksTransformErrorV1::NonCanonicalInput)
                ))
            });
            columns[width - 1][rows - 1] = FIELD_MODULUS - 1;
            let allocations = columns
                .iter()
                .map(|column| (column.as_ptr(), column.capacity()))
                .collect::<Vec<_>>();
            for mode in ScanMode::ALL {
                assert!(mode.run_columns(&columns, &pool));
            }
            let mut samples = [[0.0_f64; ROUNDS]; ScanMode::ALL.len()];
            for round in 0..ROUNDS {
                for position in 0..ScanMode::ALL.len() {
                    let index = if round % 2 == 0 {
                        (round / 2 + position) % ScanMode::ALL.len()
                    } else {
                        (round / 2 + ScanMode::ALL.len() - 1 - position) % ScanMode::ALL.len()
                    };
                    let mode = ScanMode::ALL[index];
                    let started = Instant::now();
                    let valid = black_box(mode.run_columns(black_box(&columns), &pool));
                    samples[index][round] = started.elapsed().as_secs_f64();
                    assert!(valid);
                }
            }
            assert_eq!(
                columns
                    .iter()
                    .map(|column| (column.as_ptr(), column.capacity()))
                    .collect::<Vec<_>>(),
                allocations
            );
            for (index, mode) in ScanMode::ALL.into_iter().enumerate() {
                let mean = samples[index].iter().sum::<f64>() / ROUNDS as f64;
                samples[index].sort_by(f64::total_cmp);
                let median = (samples[index][ROUNDS / 2 - 1] + samples[index][ROUNDS / 2]) / 2.0;
                eprintln!(
                    "exact_root_input_canonical_scan mode={mode:?} log={log} width={width} words={words} bytes={} workers={workers} caller_context=external tile_pages={SCAN_TILE_PAGES} rounds={ROUNDS} mean_ms={:.6} median_ms={:.6} allocation=unchanged parity=all_words; includes scan and parallel pool entry; excludes fixture allocation/fill/clear and worker startup; component timing only",
                    words * mem::size_of::<u64>(),
                    mean * 1000.0,
                    median * 1000.0,
                );
            }
        }
    }
}

#[test]
#[ignore = "required Metal common22 four-column complete facade timing with full CPU parity"]
fn required_metal_common22_four_column_full_wrapper_cost_preserves_outputs() {
    use crate::goldilocks_transform::{
        GoldilocksTransformBackendV1 as Backend, GoldilocksTransformDirectionV1 as Direction,
        transform_goldilocks_columns_v1,
    };
    use std::time::Instant;

    const ROUNDS: usize = 12;
    const ROWS: usize = 1 << 22;
    const WIDTH: usize = 4;
    assert!(!cfg!(debug_assertions), "use optimized build");
    assert!(
        rayon::current_thread_index().is_none(),
        "use external caller"
    );
    select_metal_device().expect("actual Metal is required");
    let root = goldilocks_pow(7, (FIELD_MODULUS - 1) >> 22);
    let original = (0..WIDTH)
        .map(|column| {
            (0..ROWS)
                .map(|row| (31 * row + 17 * column + 1) as u64)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut expected = original.clone();
    transform_goldilocks_columns_v1(
        &mut expected,
        root,
        Direction::Forward,
        crate::ExecutionMode::Cpu,
    )
    .unwrap();
    let mut actual = original.clone();
    let pointers = actual
        .iter()
        .map(|v| (v.as_ptr(), v.capacity()))
        .collect::<Vec<_>>();
    struct StopTelemetry;
    impl Drop for StopTelemetry {
        fn drop(&mut self) {
            enable_queue_depth_stats(false);
        }
    }
    enable_queue_depth_stats(true);
    let _telemetry = StopTelemetry;
    let mut samples = [(0.0, ColumnStagingPhaseStats::default()); ROUNDS];
    for round in 0..=ROUNDS {
        for (target, source) in actual.iter_mut().zip(&original) {
            target.copy_from_slice(source);
        }
        take_column_staging_stats().unwrap();
        let started = Instant::now();
        let backend = transform_goldilocks_columns_v1(
            &mut actual,
            root,
            Direction::Forward,
            crate::ExecutionMode::Gpu,
        )
        .unwrap();
        let elapsed = started.elapsed().as_secs_f64();
        let staging = take_column_staging_stats().unwrap().fft();
        assert_eq!(backend, Backend::Metal);
        assert_eq!(staging.batches, 1);
        assert_eq!(actual, expected);
        assert_eq!(
            actual
                .iter()
                .map(|v| (v.as_ptr(), v.capacity()))
                .collect::<Vec<_>>(),
            pointers
        );
        if round != 0 {
            samples[round - 1] = (elapsed, staging);
        }
    }
    for (round, (seconds, staging)) in samples.into_iter().enumerate() {
        eprintln!(
            "exact_root_public_facade_cost log=22 width=4 rows={ROWS} workers={} caller_context=external round={round} total_ms={:.6} flatten_ms={:.6} wait_ms={:.6} timestamped_commands={} gpu_execution_ms={:.6} cpu_driver_scheduling_ms={:.6} pre_gpu_queue_ms={:.6} parity=all_words_exact pointers=unchanged timing_scope=successful-command-intervals-not-additive; includes complete public facade and private staging clear; excludes caller allocation/reset/clear and parity comparison",
            rayon::current_num_threads(),
            seconds * 1000.0,
            staging.flatten_ms,
            staging.wait_ms,
            staging.timestamped_commands,
            staging.gpu_execution_ms,
            staging.cpu_driver_scheduling_ms,
            staging.pre_gpu_queue_ms,
        );
    }
}
