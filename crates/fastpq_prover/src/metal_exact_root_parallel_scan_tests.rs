//! Production completed-output parallel scans and full owner erasure at 128 MiB.

use super::*;
use crate::goldilocks_transform::{
    CANONICAL_SCAN_PARALLEL_MIN_WORDS_V1, CANONICAL_SCAN_TILE_WORDS_V1,
    GoldilocksTransformErrorV1 as Error, canonical_parallel_tests::LARGE_SCAN_FIXTURE_LOCK,
    parallel_canonical_scan_v1,
};
use crate::gpu_secret::ErasureObservation;

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

fn fixture_word(index: usize) -> u64 {
    [0, 1, FIELD_MODULUS - 1, 37][index % 4]
}

fn fixture(words: usize) -> PooledBuffer {
    // Construct real clearing host custody directly: this scan test submits no GPU work.
    let page_count = metal_buffer_page_count(words);
    let mut pages = Vec::new();
    pages.try_reserve_exact(page_count).unwrap();
    assert_eq!(pages.capacity(), page_count);
    pages.resize_with(page_count, MetalBufferPage::zeroed);
    let mut buffer = PooledBuffer {
        backing: Arc::new(PooledBufferBacking {
            pages,
            logical_len: words,
            sensitive: true,
        }),
    };
    for (index, word) in Arc::get_mut(&mut buffer.backing)
        .unwrap()
        .pages
        .iter_mut()
        .flat_map(|page| page.words.iter_mut())
        .enumerate()
    {
        *word = if index < words {
            fixture_word(index)
        } else {
            u64::MAX
        };
    }
    buffer
}

fn identity(buffer: &PooledBuffer) -> (usize, usize, usize, usize) {
    (
        buffer.backing.pages.as_ptr() as usize,
        buffer.backing.pages.len(),
        buffer.backing.pages.capacity(),
        buffer.len(),
    )
}

fn set_word(buffer: &mut PooledBuffer, index: usize, word: u64) {
    Arc::get_mut(&mut buffer.backing).unwrap().pages[index / METAL_BUFFER_PAGE_WORDS].words
        [index % METAL_BUFFER_PAGE_WORDS] = word;
}

fn assert_production_scan(buffer: &PooledBuffer, words: usize, expected: bool) {
    assert!(words <= buffer.len());
    let scalar = buffer
        .backing
        .pages
        .iter()
        .flat_map(|page| page.words.iter())
        .take(words)
        .all(|&word| word < FIELD_MODULUS);
    assert_eq!(scalar, expected);
    assert_eq!(
        completed_output_is_canonical_v1(&buffer.backing.pages, words),
        scalar
    );
}

#[test]
fn production_completed_parallel_scan_matches_scalar_and_erases_complete_owners() {
    let _fixture_guard = LARGE_SCAN_FIXTURE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(CANONICAL_SCAN_PARALLEL_MIN_WORDS_V1, 1 << 24);
    let threshold = CANONICAL_SCAN_PARALLEL_MIN_WORDS_V1;
    let tile = CANONICAL_SCAN_TILE_WORDS_V1;
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .unwrap();
    pool.install(|| {
        // Both guards are thread-local and must be created/dropped on this initiating worker.
        let _quarantine = Quarantine::clear();
        assert_eq!(rayon::current_num_threads(), 2);
        assert!(!parallel_canonical_scan_v1(threshold - 1));
        assert!(parallel_canonical_scan_v1(threshold));
        assert!(parallel_canonical_scan_v1(threshold + 3));
        {
            let observed = ErasureObservation::begin();
            let words = threshold + 3;
            let mut buffer = fixture(words);
            let allocation = identity(&buffer);
            for extent in [threshold - 1, threshold, words] {
                assert_production_scan(&buffer, extent, true);
            }
            for (position, invalid) in [
                (0, FIELD_MODULUS),
                (METAL_BUFFER_PAGE_WORDS - 1, u64::MAX),
                (METAL_BUFFER_PAGE_WORDS, FIELD_MODULUS),
                (tile - 1, u64::MAX),
                (tile, FIELD_MODULUS),
                (threshold - 1, u64::MAX),
                (threshold, FIELD_MODULUS),
                (words - 1, u64::MAX),
            ] {
                set_word(&mut buffer, position, invalid);
                assert_production_scan(&buffer, words, false);
                assert_eq!(
                    buffer.backing.pages[position / METAL_BUFFER_PAGE_WORDS].words
                        [position % METAL_BUFFER_PAGE_WORDS],
                    invalid
                );
                assert_eq!(identity(&buffer), allocation);
                set_word(&mut buffer, position, fixture_word(position));
            }
            set_word(&mut buffer, threshold - 1, FIELD_MODULUS);
            assert_production_scan(&buffer, threshold - 1, true);
            assert_production_scan(&buffer, threshold, false);
            set_word(&mut buffer, threshold - 1, fixture_word(threshold - 1));
            // All initialized padding remains untouched and excluded even on the parallel tail.
            assert!(
                buffer
                    .backing
                    .pages
                    .iter()
                    .flat_map(|page| page.words.iter())
                    .enumerate()
                    .all(|(index, &word)| word
                        == if index < words {
                            fixture_word(index)
                        } else {
                            u64::MAX
                        })
            );
            assert_eq!(identity(&buffer), allocation);
            assert_eq!(observed.counts(), (0, 0));
            let weak = Arc::downgrade(&buffer.backing);
            drop(buffer);
            assert!(weak.upgrade().is_none());
            assert_eq!(
                observed.counts(),
                (allocation.1 * METAL_BUFFER_PAGE_WORDS, 0)
            );
        }
        // Reuse the same bounded peak: each exact-shape owner is dropped before the next.
        for reject_tail in [true, false] {
            let observed = ErasureObservation::begin();
            let mut output = PreparedOutput {
                buffer: fixture(threshold),
                rows: threshold / 8,
                columns: 8,
            };
            let allocation = identity(&output.buffer);
            if reject_tail {
                set_word(&mut output.buffer, threshold - 1, FIELD_MODULUS);
            }
            let weak = Arc::downgrade(&output.buffer.backing);
            if reject_tail {
                assert_production_scan(&output.buffer, threshold, false);
                assert_eq!(
                    output.into_readonly().err(),
                    Some(Error::NonCanonicalOutput)
                );
            } else {
                assert_production_scan(&output.buffer, threshold, true);
                let completed = Arc::new(output.into_readonly().unwrap());
                assert_eq!(identity(&completed.output.buffer), allocation);
                for column in 0..8 {
                    let values = completed.column(column).unwrap();
                    assert_eq!(
                        values.as_ptr() as usize,
                        allocation.0 + column * (threshold / 8) * std::mem::size_of::<u64>()
                    );
                    assert!(
                        values.iter().enumerate().all(
                            |(row, &word)| word == fixture_word(column * (threshold / 8) + row)
                        )
                    );
                }
                assert_eq!(completed.column(8), None);
                let retained = Arc::clone(&completed);
                drop(completed);
                assert!(weak.upgrade().is_some());
                assert_eq!(observed.counts(), (0, 0));
                let final_owner = Arc::into_inner(retained).expect("one completed owner remains");
                final_owner.erase_completed_v1();
            }
            assert!(weak.upgrade().is_none());
            assert_eq!(observed.counts(), (threshold, 0));
        }
    });
}
