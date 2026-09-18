// Native pending carriers must be protected before canonical recovery can
// truncate or replace a body. Selection reads existing durable decisions only;
// it never promotes a marker, repairs a journal or retires an index record.
impl BlockStore {
    fn native_amx_startup_selected_marker_read_only(&self) -> Result<BlockStoreCommitMarker> {
        let path = self.commit_marker_path();
        let temporary_path = path.with_extension("norito.tmp");
        let stable = Self::read_bounded_commit_marker_bytes(&path)?;
        let temporary = Self::read_bounded_commit_marker_bytes(&temporary_path)?;
        if let Some(bytes) = stable.as_deref()
            && let Ok(marker) = norito::decode_canonical::<BlockStoreCommitMarker>(bytes)
            && marker.version == BlockStoreCommitMarker::VERSION
            && (marker.count == 0) != marker.tip_hash.is_none()
        {
            // The existing durable selector treats this decoded stable invariant
            // violation as an error even when a valid temporary is present.
            return Err(self.invalid_da_block_rewrite_stage(
                "Native startup stable marker has an invalid empty/tip invariant",
            ));
        }
        let valid = |bytes: &[u8]| {
            norito::decode_canonical::<BlockStoreCommitMarker>(bytes)
                .ok()
                .filter(|marker| {
                    marker.version == BlockStoreCommitMarker::VERSION
                        && (marker.count == 0) == marker.tip_hash.is_none()
                })
        };
        // The canonical marker reader promotes a valid deterministic temporary.
        // Mirror that decision without performing its promotion or residue cleanup.
        if let Some(marker) = temporary
            .as_deref()
            .and_then(valid)
            .or_else(|| stable.as_deref().and_then(valid))
        {
            return Ok(marker);
        }
        if stable.is_none() && temporary.is_none() {
            // A genuinely untouched empty prefix has no durable marker yet.
            // Missing marker metadata must never reinterpret existing history
            // as an uncommitted append and discard its body pins.
            for name in [DATA_FILE_NAME, INDEX_FILE_NAME, HASHES_FILE_NAME] {
                let path = self.path_to_blockchain.join(name);
                if Kura::regular_sidecar_metadata_for(
                    &self.path_to_blockchain,
                    &path,
                    &self.path_to_blockchain,
                )?
                .is_some_and(|metadata| metadata.file.len() != 0)
                {
                    return Err(self.invalid_da_block_rewrite_stage(
                        "pending Native publication has canonical bytes but no durable marker",
                    ));
                }
            }
            return Ok(BlockStoreCommitMarker::new(0, None));
        }
        Err(self.invalid_da_block_rewrite_stage(
            "pending Native publication has no valid selected canonical marker",
        ))
    }

    fn native_amx_startup_hash_at_read_only(
        &self,
        height: u64,
    ) -> Result<Option<HashOf<BlockHeader>>> {
        let path = self.path_to_blockchain.join(HASHES_FILE_NAME);
        let Some(before) = Kura::regular_sidecar_metadata_for(
            &self.path_to_blockchain,
            &path,
            &self.path_to_blockchain,
        )?
        else {
            return Ok(None);
        };
        let offset = height
            .checked_sub(1)
            .and_then(|index| index.checked_mul(SIZE_OF_BLOCK_HASH))
            .ok_or_else(|| {
                self.invalid_da_block_rewrite_stage("Native startup hash offset overflows")
            })?;
        let end = offset.checked_add(SIZE_OF_BLOCK_HASH).ok_or_else(|| {
            self.invalid_da_block_rewrite_stage("Native startup hash extent overflows")
        })?;
        if before.file.len() < end {
            return Ok(None);
        }
        let mut file =
            std::fs::File::open(&path).map_err(|error| Error::IO(error, path.clone()))?;
        let opened = secure_file_metadata::from_file(&file)
            .map_err(|error| Error::IO(error, path.clone()))?;
        if !Kura::sidecar_file_metadata_unchanged(&before.file, &opened) {
            return Err(self.invalid_da_block_rewrite_stage(
                "Native startup hash journal changed while opening",
            ));
        }
        let mut bytes = [0_u8; Hash::LENGTH];
        file.seek(SeekFrom::Start(offset))
            .and_then(|_| file.read_exact(&mut bytes))
            .map_err(|error| Error::IO(error, path.clone()))?;
        let opened_after = secure_file_metadata::from_file(&file)
            .map_err(|error| Error::IO(error, path.clone()))?;
        let after = Kura::regular_sidecar_metadata_for(
            &self.path_to_blockchain,
            &path,
            &self.path_to_blockchain,
        )?;
        if !Kura::sidecar_file_metadata_unchanged(&before.file, &opened_after)
            || !after
                .as_ref()
                .is_some_and(|after| Kura::stable_sidecar_file_binding_unchanged(&before, after))
            || bytes[Hash::LENGTH - 1] & 1 == 0
        {
            return Err(self.invalid_da_block_rewrite_stage(
                "Native startup hash journal changed or has an invalid hash",
            ));
        }
        Ok(Some(HashOf::from_untyped_unchecked(Hash::prehashed(bytes))))
    }

    fn native_amx_publication_pins_before_storage_recovery(
        &mut self,
        store_root: &Path,
    ) -> Result<BTreeMap<u64, HashOf<BlockHeader>>> {
        let inventory = Kura::read_native_amx_publication_index_for_store(store_root)?;
        if inventory.records.is_empty() {
            return Ok(BTreeMap::new());
        }
        let marker = self.native_amx_startup_selected_marker_read_only()?;
        let stage = self.read_da_block_rewrite_stage()?;
        let selected_suffix = match stage.as_ref() {
            Some(stage) if marker == stage.old_marker => {
                Some((stage.replacement[0].height, stage.old_suffix.as_slice()))
            }
            Some(stage) if marker == stage.new_marker => {
                Some((stage.replacement[0].height, stage.replacement.as_slice()))
            }
            Some(_) => {
                return Err(self.invalid_da_block_rewrite_stage(
                    "Native startup marker selects neither durable rewrite image",
                ));
            }
            None => None,
        };
        let mut pins = inventory.pins_for_resolved_marker(&marker, |height| {
            if let Some((start, images)) = selected_suffix
                && height >= start
            {
                return Ok(images
                    .get(usize::try_from(height - start)?)
                    .filter(|image| image.height == height)
                    .map(|image| image.block_hash));
            }
            self.native_amx_startup_hash_at_read_only(height)
        })?;
        let prune_inventory = Kura::canonical_prune_intent_artifact_inventory(store_root)?;
        if let Some(prune) = prune_inventory
            .stable
            .as_ref()
            .or(prune_inventory.temporary.as_ref())
        {
            let intent = &prune.intent;
            if ![
                (intent.source_height, intent.source_tip_hash),
                (intent.target_height, intent.target_tip_hash),
            ]
            .contains(&(marker.count, marker.tip_hash))
            {
                return Err(Error::PruneIntentConflict(
                    "Native startup marker differs from pending prune endpoints".to_owned(),
                ));
            }
            for record in inventory
                .records
                .values()
                .filter(|record| record.carrier.height > intent.target_height)
            {
                let listed = intent
                    .native_amx_retirement_record_hashes
                    .binary_search(&Hash::new(record.encoded()?))
                    .is_ok();
                if !listed && pins.get(&record.carrier.height) == Some(&record.carrier.block_hash) {
                    return Err(Error::PruneIntentConflict(
                        "pending prune omits an exact selected Native publication record"
                            .to_owned(),
                    ));
                }
            }
            // Remove heights only after validating every exact selected identity;
            // a listed obsolete same-height record cannot erase another owner's pin.
            pins.retain(|height, _| *height <= intent.target_height);
        }
        Ok(pins)
    }
}

#[cfg(test)]
mod native_amx_publication_startup_pin_tests {
    use super::*;

    fn carrier(height: u64, seed: u8) -> NativeAmxPublicationCarrier {
        NativeAmxPublicationCarrier {
            height,
            block_hash: HashOf::from_untyped_unchecked(Hash::new([seed])),
            executed_wire_hash: Hash::new([seed, 1]),
        }
    }
    fn record(
        current: NativeAmxPublicationCarrier,
        before: BlockStoreCommitMarker,
        replaced: Option<NativeAmxPublicationCarrier>,
    ) -> NativeAmxPublicationIndexRecord {
        NativeAmxPublicationIndexRecord {
            carrier: current,
            selection_marker: before,
            origin: NativeAmxPublicationIndexOriginV1::CanonicalWrite,
            replaced,
            merge_entry_hash: None,
        }
    }
    fn write_record(root: &Path, record: &NativeAmxPublicationIndexRecord) {
        let directory = root.join(NATIVE_AMX_PUBLICATION_INDEX_DIRECTORY);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join(record.file_name().unwrap()),
            record.encoded().unwrap(),
        )
        .unwrap();
    }
    fn fixture() -> (tempfile::TempDir, PathBuf, BlockStore) {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let blocks = root.join("blocks");
        std::fs::create_dir(&blocks).unwrap();
        let store = BlockStore::new(blocks);
        (directory, root, store)
    }
    fn write_marker(
        store: &BlockStore,
        marker: &BlockStoreCommitMarker,
        temporary: bool,
    ) -> Vec<u8> {
        let mut path = store.commit_marker_path();
        if temporary {
            path = path.with_extension("norito.tmp");
        }
        let bytes = norito::encode_canonical(marker).unwrap();
        std::fs::write(path, &bytes).unwrap();
        bytes
    }
    fn write_hashes(store: &BlockStore, values: &[NativeAmxPublicationCarrier]) {
        let bytes = values
            .iter()
            .flat_map(|value| value.block_hash.as_ref().iter().copied())
            .collect::<Vec<_>>();
        std::fs::write(store.path_to_blockchain.join(HASHES_FILE_NAME), bytes).unwrap();
    }

    #[test]
    fn non_tip_pending_carrier_is_pinned_and_prospective_append_is_not() {
        let (_directory, root, mut store) = fixture();
        let first = carrier(1, 1);
        let tip = carrier(2, 2);
        let future = carrier(3, 3);
        let marker = BlockStoreCommitMarker::new(2, Some(tip.block_hash));
        write_record(
            &root,
            &record(first, BlockStoreCommitMarker::new(0, None), None),
        );
        write_record(&root, &record(future, marker.clone(), None));
        let marker_bytes = write_marker(&store, &marker, false);
        write_hashes(&store, &[first, tip]);
        assert_eq!(
            store
                .native_amx_publication_pins_before_storage_recovery(&root)
                .unwrap(),
            BTreeMap::from([(1, first.block_hash)])
        );
        assert_eq!(
            std::fs::read(store.commit_marker_path()).unwrap(),
            marker_bytes
        );
        assert!(!store.path_to_blockchain.join(INDEX_FILE_NAME).exists());
        assert!(!store.path_to_blockchain.join(DATA_FILE_NAME).exists());
        assert_eq!(
            Kura::read_native_amx_publication_index_for_store(&root)
                .unwrap()
                .records
                .len(),
            2
        );
    }

    #[test]
    fn marker_temporary_selects_pins_without_promotion_or_cleanup() {
        let (_directory, root, mut store) = fixture();
        let first = carrier(1, 1);
        let next = carrier(2, 2);
        let old = BlockStoreCommitMarker::new(1, Some(first.block_hash));
        let new = BlockStoreCommitMarker::new(2, Some(next.block_hash));
        write_record(&root, &record(next, old.clone(), None));
        let stable = write_marker(&store, &old, false);
        let temporary = write_marker(&store, &new, true);
        write_hashes(&store, &[first, next]);
        assert_eq!(
            store
                .native_amx_publication_pins_before_storage_recovery(&root)
                .unwrap(),
            BTreeMap::from([(2, next.block_hash)])
        );
        assert_eq!(std::fs::read(store.commit_marker_path()).unwrap(), stable);
        assert_eq!(
            std::fs::read(store.commit_marker_path().with_extension("norito.tmp")).unwrap(),
            temporary
        );
    }

    #[test]
    fn empty_precommit_discovery_creates_no_canonical_files() {
        let (_directory, root, mut store) = fixture();
        write_record(
            &root,
            &record(carrier(1, 1), BlockStoreCommitMarker::new(0, None), None),
        );
        assert!(
            store
                .native_amx_publication_pins_before_storage_recovery(&root)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            std::fs::read_dir(&store.path_to_blockchain)
                .unwrap()
                .count(),
            0
        );
    }

    #[test]
    fn missing_marker_does_not_reinterpret_canonical_bytes_as_empty() {
        let (_directory, root, mut store) = fixture();
        let first = carrier(1, 1);
        write_record(
            &root,
            &record(first, BlockStoreCommitMarker::new(0, None), None),
        );
        write_hashes(&store, &[first]);
        assert!(
            store
                .native_amx_publication_pins_before_storage_recovery(&root)
                .is_err()
        );
        assert!(!store.commit_marker_path().exists());
        assert_eq!(
            std::fs::metadata(store.path_to_blockchain.join(HASHES_FILE_NAME))
                .unwrap()
                .len(),
            Hash::LENGTH as u64
        );
    }

    #[test]
    fn pending_prune_exempts_only_its_exact_selected_retirement_records() {
        let (_directory, root, mut store) = fixture();
        let first = carrier(1, 1);
        let next = carrier(2, 2);
        let pending = record(
            next,
            BlockStoreCommitMarker::new(1, Some(first.block_hash)),
            None,
        );
        write_record(&root, &pending);
        write_marker(
            &store,
            &BlockStoreCommitMarker::new(2, Some(next.block_hash)),
            false,
        );
        write_hashes(&store, &[first, next]);
        let mut intent = KuraPruneIntentV3 {
            version: 3,
            source_height: 2,
            source_tip_hash: Some(next.block_hash),
            target_height: 1,
            target_tip_hash: Some(first.block_hash),
            retained_merge_entries: 0,
            retained_merge_tip_hash: None,
            native_amx_retirement_record_hashes: vec![Hash::new(pending.encoded().unwrap())],
            sidecar_rewrite: KuraPruneSidecarRewriteProjectionV3::none(),
            capacity: KuraPruneCapacityAdmissionV3 {
                source_physical_bytes: 0,
                pending_canonical_bytes: 0,
                lane_publication_reserved_bytes: 0,
                certified_bundle_reserved_bytes: 0,
                autonomous_terminal_reserved_bytes: 0,
                intent_bytes: 0,
                marker_temporary_bytes: 1,
                marker_stable_growth_bytes: 0,
                admitted_peak_bytes: 0,
            },
        };
        let seal = |intent: &mut KuraPruneIntentV3| {
            intent.capacity.intent_bytes = norito::encode_canonical(intent).unwrap().len() as u64;
            intent.capacity.admitted_peak_bytes = intent
                .capacity
                .required_peak_bytes(intent.sidecar_rewrite)
                .unwrap();
            norito::encode_canonical(intent).unwrap()
        };
        let bytes = seal(&mut intent);
        let path = Kura::prune_intent_path_for(&root);
        std::fs::write(&path, &bytes).unwrap();
        assert!(
            store
                .native_amx_publication_pins_before_storage_recovery(&root)
                .unwrap()
                .is_empty()
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(
            Kura::read_native_amx_publication_index_for_store(&root)
                .unwrap()
                .records
                .len(),
            1
        );
        intent.native_amx_retirement_record_hashes.clear();
        std::fs::write(&path, seal(&mut intent)).unwrap();
        assert!(
            store
                .native_amx_publication_pins_before_storage_recovery(&root)
                .is_err()
        );
    }

    #[test]
    fn malformed_marker_or_hash_rejects_without_repair() {
        let (_directory, root, mut store) = fixture();
        let first = carrier(1, 1);
        write_record(
            &root,
            &record(first, BlockStoreCommitMarker::new(0, None), None),
        );
        std::fs::write(store.commit_marker_path(), b"malformed").unwrap();
        assert!(
            store
                .native_amx_publication_pins_before_storage_recovery(&root)
                .is_err()
        );
        assert_eq!(
            std::fs::read(store.commit_marker_path()).unwrap(),
            b"malformed"
        );
        let valid_temporary = write_marker(
            &store,
            &BlockStoreCommitMarker::new(1, Some(first.block_hash)),
            true,
        );
        let invalid_stable = BlockStoreCommitMarker {
            version: BlockStoreCommitMarker::VERSION,
            count: 1,
            tip_hash: None,
        };
        let invalid_bytes = write_marker(&store, &invalid_stable, false);
        assert!(
            store
                .native_amx_publication_pins_before_storage_recovery(&root)
                .is_err()
        );
        assert_eq!(
            std::fs::read(store.commit_marker_path()).unwrap(),
            invalid_bytes
        );
        assert_eq!(
            std::fs::read(store.commit_marker_path().with_extension("norito.tmp")).unwrap(),
            valid_temporary
        );
        std::fs::remove_file(store.commit_marker_path().with_extension("norito.tmp")).unwrap();
        write_marker(
            &store,
            &BlockStoreCommitMarker::new(1, Some(first.block_hash)),
            false,
        );
        std::fs::write(
            store.path_to_blockchain.join(HASHES_FILE_NAME),
            [0_u8; Hash::LENGTH],
        )
        .unwrap();
        assert!(
            store
                .native_amx_publication_pins_before_storage_recovery(&root)
                .is_err()
        );
        std::fs::write(
            store.path_to_blockchain.join(HASHES_FILE_NAME),
            [1_u8; Hash::LENGTH - 1],
        )
        .unwrap();
        assert!(
            store
                .native_amx_publication_pins_before_storage_recovery(&root)
                .is_err()
        );
    }

    #[test]
    fn staged_rewrite_selects_old_or_new_image_before_hash_journal_repair() {
        let (_directory, root, mut store) = fixture();
        let old = carrier(1, 1);
        let new = carrier(1, 2);
        let old_marker = BlockStoreCommitMarker::new(1, Some(old.block_hash));
        let new_marker = BlockStoreCommitMarker::new(1, Some(new.block_hash));
        write_record(
            &root,
            &record(old, BlockStoreCommitMarker::new(0, None), None),
        );
        write_record(&root, &record(new, old_marker.clone(), Some(old)));
        // Hash-only images exercise selection, not Native body authentication;
        // strict chain validation subsequently requires every selected pin's body.
        let image = |value: NativeAmxPublicationCarrier| DaBlockRewriteImageV1 {
            height: value.height,
            block_hash: value.block_hash,
            index_start: EVICTED_BLOCK_START,
            index_length: 0,
            body: None,
        };
        let stage = DaBlockRewriteStageV1 {
            format_version: DA_BLOCK_REWRITE_STAGE_VERSION,
            old_marker: old_marker.clone(),
            new_marker: new_marker.clone(),
            old_data_len: 0,
            old_index_count: 1,
            old_hash_count: 1,
            old_suffix: vec![image(old)],
            replacement: vec![image(new)],
        };
        let stage_bytes = norito::encode_canonical(&stage).unwrap();
        std::fs::write(store.da_block_rewrite_stage_path(), &stage_bytes).unwrap();
        let stable = write_marker(&store, &old_marker, false);
        write_hashes(&store, &[new]);
        assert_eq!(
            store
                .native_amx_publication_pins_before_storage_recovery(&root)
                .unwrap(),
            BTreeMap::from([(1, old.block_hash)])
        );
        write_marker(&store, &new_marker, true);
        assert_eq!(
            store
                .native_amx_publication_pins_before_storage_recovery(&root)
                .unwrap(),
            BTreeMap::from([(1, new.block_hash)])
        );
        assert_eq!(std::fs::read(store.commit_marker_path()).unwrap(), stable);
        assert_eq!(
            std::fs::read(store.da_block_rewrite_stage_path()).unwrap(),
            stage_bytes
        );
    }
}
