//! Exact staged compaction recovers either remove-before-rename crash boundary.

use super::*;
use crate::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};

#[test]
fn exact_compaction_stage_restores_missing_live_data_or_index() {
    let mut chain =
        CertifiedTestChain::start(TestChainConfig::new(crate::state::World::new(), 1_000))
            .expect("original native genesis");
    chain.commit(Vec::new());
    let first = chain.committed(1);
    let second = chain.committed(2);
    let first_wire = first.block().encode_wire().unwrap();
    let second_wire = second.block().encode_wire().unwrap();

    for missing in [DATA_FILE_NAME, INDEX_FILE_NAME] {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let mut store = BlockStore::new(root);
        store.create_files_if_they_do_not_exist().unwrap();
        store.append_block_to_chain(first.block()).unwrap();
        store.append_block_to_chain(second.block()).unwrap();
        let marker_before = std::fs::read(root.join(COUNT_FILE_NAME)).unwrap();
        let hashes_before = std::fs::read(root.join(HASHES_FILE_NAME)).unwrap();
        let marker = store.read_commit_marker().unwrap().unwrap();
        let (marker_len, marker_digest) =
            BlockStore::eviction_file_digest(&root.join(COUNT_FILE_NAME)).unwrap();
        let (hashes_len, hashes_digest) =
            BlockStore::eviction_file_digest(&root.join(HASHES_FILE_NAME)).unwrap();
        let index_wire = [
            BlockIndex {
                start: 0,
                length: first_wire.len() as u64,
            }
            .encode(),
            BlockIndex {
                start: EVICTED_BLOCK_START,
                length: second_wire.len() as u64,
            }
            .encode(),
        ]
        .concat();
        let data_temp = store.eviction_compaction_data_path();
        let index_temp = store.eviction_compaction_index_path();
        for (path, bytes) in [(&data_temp, &first_wire), (&index_temp, &index_wire)] {
            std::fs::write(path, bytes).unwrap();
            std::fs::File::open(path).unwrap().sync_all().unwrap();
        }
        store.write_da_block_bytes(2, &second_wire).unwrap();
        let (data_len, data_digest) = BlockStore::eviction_file_digest(&data_temp).unwrap();
        let (index_len, index_digest) = BlockStore::eviction_file_digest(&index_temp).unwrap();
        let stage = EvictionCompactionStageV1 {
            format_version: EVICTION_COMPACTION_STAGE_VERSION,
            marker,
            marker_len,
            marker_digest,
            hashes_len,
            hashes_digest,
            data_temp_name: EVICTION_COMPACTION_DATA_FILE_NAME.to_owned(),
            data_len,
            data_digest,
            index_temp_name: EVICTION_COMPACTION_INDEX_FILE_NAME.to_owned(),
            index_len,
            index_digest,
            evicted: vec![EvictionCompactionEntryV1 {
                height: 2,
                block_hash: second.block_hash(),
                canonical_wire_hash: Hash::new(&second_wire),
                wire_len: second_wire.len() as u64,
            }],
        };
        store.write_eviction_compaction_stage(&stage).unwrap();
        store.drop_cached_handles();
        if missing == INDEX_FILE_NAME {
            store
                .promote_eviction_compaction_file(
                    &root.join(DATA_FILE_NAME),
                    &data_temp,
                    data_len,
                    data_digest,
                )
                .unwrap();
        }
        // Model the Windows remove-live / rename-temp interruption on every host.
        // The exact durable stage, not generic empty-file creation, owns repair.
        std::fs::remove_file(root.join(missing)).unwrap();
        sync_dir(root).unwrap();
        drop(store);

        let mut reopened = BlockStore::new(root);
        reopened
            .create_files_if_they_do_not_exist()
            .expect("exact durable stage restores its missing original live path");
        assert_eq!(reopened.read_durable_index_count().unwrap(), 2);
        assert_eq!(
            std::fs::read(root.join(DATA_FILE_NAME)).unwrap(),
            first_wire
        );
        assert_eq!(
            std::fs::read(root.join(INDEX_FILE_NAME)).unwrap(),
            index_wire
        );
        assert_eq!(
            std::fs::read(root.join(COUNT_FILE_NAME)).unwrap(),
            marker_before
        );
        assert_eq!(
            std::fs::read(root.join(HASHES_FILE_NAME)).unwrap(),
            hashes_before
        );
        assert_eq!(
            reopened.read_optional_da_cache(2).unwrap(),
            Some(second_wire.clone())
        );
        assert!(!reopened.eviction_compaction_stage_path().exists());
        assert!(!data_temp.exists());
        assert!(!index_temp.exists());
    }
}
