use super::*;
use std::num::NonZeroUsize;
#[test]
fn committed_block_hashes_snapshot_reads_block_hash_journal() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query_handle);
    let first =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x41; Hash::LENGTH]));
    let second =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x42; Hash::LENGTH]));
    {
        let mut block_hashes = state.block_hashes.block();
        block_hashes.push_for_tests(first);
        block_hashes.push_for_tests(second);
        block_hashes.commit_for_tests();
    }
    assert_eq!(state.committed_block_hashes_snapshot(), vec![first, second]);
    assert!(state.committed_block_hashes_match(&[first, second]));
    assert!(!state.committed_block_hashes_match(&[first]));
    assert!(!state.committed_block_hashes_match(&[second, first]));
    let mut visited = Vec::new();
    state
        .try_for_each_committed_block_hash::<core::convert::Infallible>(|height, hash| {
            visited.push((height, hash));
            Ok(())
        })
        .expect("infallible journal visitor");
    assert_eq!(
        visited,
        vec![
            (NonZeroUsize::new(1).expect("non-zero"), first),
            (NonZeroUsize::new(2).expect("non-zero"), second),
        ]
    );
    assert_eq!(state.committed_block_hash_at_height(0), None);
    assert_eq!(state.committed_block_hash_at_height(1), Some(first));
    assert_eq!(state.committed_block_hash_at_height(2), Some(second));
    assert_eq!(state.committed_block_hash_at_height(3), None);
    assert_eq!(
        state.committed_block_height_for_hash(first),
        NonZeroUsize::new(1)
    );
    assert_eq!(
        state.committed_block_height_for_hash(second),
        NonZeroUsize::new(2)
    );
    assert_eq!(
        state.committed_block_height_for_hash(HashOf::from_untyped_unchecked(Hash::prehashed(
            [0x43; Hash::LENGTH],
        ))),
        None
    );
}
