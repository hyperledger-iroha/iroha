//! The identical bounded relation consumes actual committed and frozen native maps.

use super::*;
use crate::test_allocations::allocations_during;
use iroha_data_model::zk::BackendTag;
use mv::{BlockMode, storage::Storage};

fn record(version: u32) -> VerifyingKeyRecord {
    VerifyingKeyRecord::new(
        version,
        "c",
        BackendTag::Stark,
        "goldilocks",
        [1; 32],
        [2; 32],
    )
}
fn id() -> VerifyingKeyId {
    VerifyingKeyId::new("b", "n")
}
fn stores() -> (
    Storage<VerifyingKeyId, VerifyingKeyRecord>,
    Storage<(String, u32), VerifyingKeyId>,
) {
    (
        [(id(), record(1))].into_iter().collect(),
        [(("c".to_owned(), 1), id())].into_iter().collect(),
    )
}

#[test]
fn both_source_forms_have_identical_exact_physical_work_and_zero_allocation() {
    let (rows, index) = stores();
    let (mirror_rows, mirror_index) = stores();
    let frozen_rows = {
        let mut block = rows.block();
        block.insert(id(), record(1));
        block.remove(VerifyingKeyId::new("z", "n"));
        block.try_detach(|_| Ok::<_, ()>(())).unwrap()
    };
    let frozen_index = {
        let mut block = index.block();
        block.insert(("c".to_owned(), 1), id());
        block.remove(("z".to_owned(), 2));
        block.try_detach(|_| Ok::<_, ()>(())).unwrap()
    };
    {
        let mut block = mirror_rows.block();
        block.insert(id(), record(1));
        block.remove(VerifyingKeyId::new("z", "n"));
        block.commit();
        let mut block = mirror_index.block();
        block.insert(("c".to_owned(), 1), id());
        block.remove(("z".to_owned(), 2));
        block.commit();
    }
    let committed_rows = mirror_rows.try_committed_view_nonblocking().unwrap();
    let committed_index = mirror_index.try_committed_view_nonblocking().unwrap();
    let rows = frozen_rows.original_images();
    let index = frozen_index.original_images();
    for (work, expected) in [(71, Err(Error::WorkLimit)), (72, Ok(()))] {
        assert_eq!(
            allocations_during(|| {
                assert_eq!(validate(&rows, &index, &mut Work::bounded(work)), expected);
                assert_eq!(
                    validate(&committed_rows, &committed_index, &mut Work::bounded(work)),
                    expected
                );
            }),
            0
        );
    }
    assert_eq!(rows.current_entries().len(), 1);
    assert_eq!(rows.undo_entries().len(), 2);
    assert_eq!(index.current_entries().len(), 1);
    assert_eq!(index.undo_entries().len(), 2);
}

#[test]
fn replacement_relation_uses_retained_preimages_even_after_target_changes() {
    let (rows, index) = stores();
    {
        let mut block = rows.block();
        block.insert(id(), record(2));
        block.commit();
        let mut block = index.block();
        block.remove(("c".to_owned(), 1));
        block.insert(("c".to_owned(), 2), id());
        block.commit();
    }
    let frozen_rows = {
        let mut block = rows.block_and_revert();
        block.insert(id(), record(3));
        block.try_detach(|_| Ok::<_, ()>(())).unwrap()
    };
    let frozen_index = {
        let mut block = index.block_and_revert();
        block.remove(("c".to_owned(), 1));
        block.insert(("c".to_owned(), 3), id());
        block.try_detach(|_| Ok::<_, ()>(())).unwrap()
    };
    let rows_identity = frozen_rows.publication_identity();
    let index_identity = frozen_index.publication_identity();
    // Real new target publications corrupt only the committed pair. They do not
    // refresh or replace either original frozen source or its replacement baseline.
    rows.block().commit();
    {
        let mut block = index.block();
        block.remove(("c".to_owned(), 2));
        block.commit();
    }
    let frozen_rows = frozen_rows.original_images();
    let frozen_index = frozen_index.original_images();
    assert_eq!(frozen_rows.mode(), BlockMode::Replace);
    assert_eq!(frozen_index.mode(), BlockMode::Replace);
    assert_eq!(frozen_rows.publication_identity(), rows_identity);
    assert_eq!(frozen_index.publication_identity(), index_identity);
    assert_eq!(frozen_rows.current_entries().next().unwrap().1.version, 3);
    assert_eq!(
        frozen_rows
            .undo_entries()
            .next()
            .unwrap()
            .1
            .as_ref()
            .unwrap()
            .version,
        1
    );
    assert_eq!(
        allocations_during(|| {
            assert_eq!(
                validate(&frozen_rows, &frozen_index, &mut Work::bounded(100_000)),
                Ok(())
            );
        }),
        0
    );
    let current_rows = rows.try_committed_view_nonblocking().unwrap();
    let current_index = index.try_committed_view_nonblocking().unwrap();
    assert_eq!(
        validate(&current_rows, &current_index, &mut Work::bounded(100_000)),
        Err(Error::Index {
            image: Image::Current,
            missing: true
        })
    );
}
