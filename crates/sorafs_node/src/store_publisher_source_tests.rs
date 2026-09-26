// Publisher staging remains separate from admitted storage across partial uploads and restart.

fn publisher_chunk_request(
    header: &PublisherSourceHeaderV1,
    digest: [u8; 32],
    index: u32,
    bytes: &[u8],
) -> PublisherSourceChunkRequestV1 {
    PublisherSourceChunkRequestV1 {
        provider_id: header.provider_id,
        order_id: header.order_id,
        assignment_revision: header.assignment_revision,
        manifest_digest: *header.verify().unwrap().0.digest().unwrap().as_bytes(),
        header_digest: digest,
        upload: sorafs_car::publisher::PublisherSourceUploadV1 {
            index,
            bytes: bytes.to_vec(),
        },
    }
}

#[test]
fn publisher_source_survives_restart_without_serving_before_finalized_ingest() {
    let temp = TempDir::new().unwrap();
    let now = crate::unix_now_secs();
    let provider = [0x31; 32];
    let order = [0x42; 32];
    let config = StorageConfig::builder()
        .enabled(true)
        .data_dir(canonical_temp_path(&temp).join("storage"))
        .provider_id(Some(iroha_data_model::sorafs::capacity::ProviderId::new(
            provider,
        )))
        .build();
    let payload = b"publisher bootstrap without any replica";
    let plan = single_file_plan(payload).unwrap();
    let manifest = manifest_builder_for_plan(payload, &plan)
        .pin_policy(PinPolicy {
            retention_epoch: now + 1000,
            ..PinPolicy::default()
        })
        .build()
        .unwrap();
    let header = PublisherSourceHeaderV1::new(provider, order, 1, &manifest, &plan).unwrap();
    let auth = crate::FinalizedProviderIngestAuthorizationV1::from_finalized_state(
        1,
        [9; 32],
        provider,
        order,
        *manifest.digest().unwrap().as_bytes(),
        manifest.root_cid.clone(),
        canonical_profile_handle(&manifest),
        manifest.chunk_digest_sha3_256,
        manifest.por_root,
        manifest.content_length,
    )
    .unwrap();
    let backend = StorageBackend::new(config.clone()).unwrap();
    let digest = backend
        .stage_publisher_source(&header, now + 900, now)
        .unwrap();
    assert_eq!(backend.manifest_count(), 0);
    assert_eq!(
        backend
            .ingest_staged_publisher_source(&auth, 1, &mut || Ok(()))
            .unwrap(),
        None
    );
    assert!(
        backend
            .stage_publisher_source_chunk(
                &publisher_chunk_request(&header, [0; 32], 0, payload),
                now
            )
            .is_err()
    );
    assert!(
        backend
            .stage_publisher_source_chunk(
                &PublisherSourceChunkRequestV1 {
                    assignment_revision: 2,
                    ..publisher_chunk_request(&header, digest, 0, payload)
                },
                now
            )
            .is_err()
    );
    assert!(
        backend
            .stage_publisher_source_chunk(
                &publisher_chunk_request(&header, digest, 0, payload),
                now + 901
            )
            .is_err()
    );
    assert!(
        backend
            .stage_publisher_source_chunk(
                &publisher_chunk_request(&header, digest, 0, b"substitute"),
                now
            )
            .is_err()
    );
    backend
        .stage_publisher_source_chunk(&publisher_chunk_request(&header, digest, 0, payload), now)
        .unwrap();
    assert_eq!(backend.manifest_count(), 0);
    drop(backend);
    let backend = StorageBackend::new(config).unwrap();
    assert_eq!(backend.manifest_count(), 0);
    let id = backend
        .ingest_staged_publisher_source(&auth, 1, &mut || Ok(()))
        .unwrap()
        .unwrap();
    assert_eq!(
        backend.read_payload_range(&id, 0, payload.len()).unwrap(),
        payload
    );
    assert!(
        !backend
            .root_dir
            .join(PUBLISHER_SOURCE_DIR_V1)
            .join(hex::encode(order))
            .exists()
    );
}

#[test]
fn publisher_source_quota_and_revision_changes_discard_unadmitted_old_chunks() {
    let temp = TempDir::new().unwrap();
    let now = crate::unix_now_secs();
    let provider = [0x31; 32];
    let config = StorageConfig::builder()
        .enabled(true)
        .data_dir(canonical_temp_path(&temp).join("storage"))
        .provider_id(Some(iroha_data_model::sorafs::capacity::ProviderId::new(
            provider,
        )))
        .max_pins(1)
        .max_capacity_bytes(iroha_config::base::util::Bytes(16384))
        .build();
    let backend = StorageBackend::new(config).unwrap();
    let payload = b"bounded initial source";
    let plan = single_file_plan(payload).unwrap();
    let manifest = manifest_builder_for_plan(payload, &plan)
        .pin_policy(PinPolicy {
            retention_epoch: now + 1000,
            ..PinPolicy::default()
        })
        .build()
        .unwrap();
    let header = PublisherSourceHeaderV1::new(provider, [1; 32], 1, &manifest, &plan).unwrap();
    let digest = backend
        .stage_publisher_source(&header, now + 100, now)
        .unwrap();
    backend
        .stage_publisher_source_chunk(&publisher_chunk_request(&header, digest, 0, payload), now)
        .unwrap();
    let other = PublisherSourceHeaderV1 {
        order_id: [2; 32],
        ..header.clone()
    };
    assert!(matches!(
        backend.stage_publisher_source(&other, now + 100, now),
        Err(StorageError::PinLimitReached { .. })
    ));
    let next = PublisherSourceHeaderV1 {
        assignment_revision: 2,
        ..header.clone()
    };
    let next_digest = backend
        .stage_publisher_source(&next, now + 100, now)
        .unwrap();
    assert_ne!(next_digest, digest);
    assert!(
        !backend
            .root_dir
            .join(PUBLISHER_SOURCE_DIR_V1)
            .join(hex::encode(header.order_id))
            .join("chunk-00000000")
            .exists()
    );
    assert!(
        backend
            .stage_publisher_source(&header, now + 100, now)
            .is_err()
    );
    // An independently finalized epoch beyond the deadline releases abandoned staging.
    backend
        .stage_publisher_source(&other, now + 200, now + 101)
        .unwrap();
    assert_eq!(backend.manifest_count(), 0);
}

fn publisher_source_fixture(
    payload: &[u8],
) -> (
    TempDir,
    StorageBackend,
    PublisherSourceHeaderV1,
    crate::FinalizedProviderIngestAuthorizationV1,
) {
    let temp = TempDir::new().unwrap();
    let provider = [0x31; 32];
    let order = [0x42; 32];
    let config = StorageConfig::builder()
        .enabled(true)
        .data_dir(canonical_temp_path(&temp).join("storage"))
        .provider_id(Some(iroha_data_model::sorafs::capacity::ProviderId::new(
            provider,
        )))
        .max_pins(64)
        .build();
    let plan = single_file_plan(payload).unwrap();
    let manifest = manifest_builder_for_plan(payload, &plan)
        .pin_policy(PinPolicy {
            retention_epoch: crate::unix_now_secs() + 1000,
            ..PinPolicy::default()
        })
        .build()
        .unwrap();
    let header = PublisherSourceHeaderV1::new(provider, order, 1, &manifest, &plan).unwrap();
    let authorization = crate::FinalizedProviderIngestAuthorizationV1::from_finalized_state(
        1,
        [9; 32],
        provider,
        order,
        *manifest.digest().unwrap().as_bytes(),
        manifest.root_cid.clone(),
        canonical_profile_handle(&manifest),
        manifest.chunk_digest_sha3_256,
        manifest.por_root,
        manifest.content_length,
    )
    .unwrap();
    let backend = StorageBackend::new(config).unwrap();
    (temp, backend, header, authorization)
}

#[test]
fn publisher_source_reloads_verified_metadata_once_after_restart_for_multiple_chunks() {
    let payload = vec![0x5a; 1024 * 1024 + 1];
    let (_temp, backend, header, _) = publisher_source_fixture(&payload);
    assert!(header.chunks.len() >= 3);
    let now = crate::unix_now_secs();
    let digest = backend
        .stage_publisher_source(&header, now + 900, now)
        .unwrap();
    assert_eq!(digest, header.canonical_digest().unwrap());
    let config = backend.config.clone();
    drop(backend);
    let backend = StorageBackend::new(config).unwrap();
    for (index, chunk) in header.chunks.iter().enumerate() {
        let offset = chunk.offset as usize;
        backend
            .stage_publisher_source_chunk(
                &publisher_chunk_request(
                    &header,
                    digest,
                    index as u32,
                    &payload[offset..offset + chunk.length as usize],
                ),
                now,
            )
            .unwrap();
    }
    let first = &header.chunks[0];
    let valid = publisher_chunk_request(&header, digest, 0, &payload[..first.length as usize]);
    for wrong in [
        PublisherSourceChunkRequestV1 {
            provider_id: [0x88; 32],
            ..valid.clone()
        },
        PublisherSourceChunkRequestV1 {
            manifest_digest: [0x88; 32],
            ..valid.clone()
        },
        PublisherSourceChunkRequestV1 {
            header_digest: [0x88; 32],
            ..valid.clone()
        },
        PublisherSourceChunkRequestV1 {
            assignment_revision: 2,
            ..valid.clone()
        },
    ] {
        assert!(backend.stage_publisher_source_chunk(&wrong, now).is_err());
    }
    backend.stage_publisher_source_chunk(&valid, now).unwrap();
    assert_eq!(backend.publisher_sources.lock().unwrap().loads, 1);
    assert_eq!(backend.manifest_count(), 0);
}

#[test]
fn publisher_source_cache_rejects_replaced_metadata_before_publishing_chunk() {
    let payload = b"metadata identity belongs to the verified session";
    let (_temp, backend, header, _) = publisher_source_fixture(payload);
    let now = crate::unix_now_secs();
    let digest = backend
        .stage_publisher_source(&header, now + 900, now)
        .unwrap();
    let path = backend
        .root_dir
        .join(PUBLISHER_SOURCE_DIR_V1)
        .join(hex::encode(header.order_id));
    let metadata = path.join(PUBLISHER_SOURCE_METADATA_V1);
    let bytes = fs::read(&metadata).unwrap();
    // Even an identical replacement is a different durable file and must not reuse its old cache.
    write_atomic(&metadata, &bytes).unwrap();
    let request = publisher_chunk_request(&header, digest, 0, payload);
    assert!(backend.stage_publisher_source_chunk(&request, now).is_err());
    assert!(!path.join("chunk-00000000").exists());
    assert!(backend.publisher_sources.lock().unwrap().entries.is_empty());
    // A later request may independently decode and authenticate the replacement.
    backend.stage_publisher_source_chunk(&request, now).unwrap();
    assert_eq!(backend.publisher_sources.lock().unwrap().loads, 1);
}

#[test]
fn publisher_source_cache_evicts_at_fixed_session_bound_and_reloads_evicted_session() {
    let payload = b"bounded cached source inventory";
    let (_temp, backend, base, _) = publisher_source_fixture(payload);
    let now = crate::unix_now_secs();
    let mut headers = Vec::new();
    for ordinal in 1..=PUBLISHER_SOURCE_CACHE_ENTRIES_V1 + 3 {
        let header = PublisherSourceHeaderV1 {
            order_id: [ordinal as u8; 32],
            ..base.clone()
        };
        backend
            .stage_publisher_source(&header, now + 900, now)
            .unwrap();
        headers.push(header);
    }
    let (evicted, loads) = {
        let cache = backend.publisher_sources.lock().unwrap();
        assert_eq!(cache.entries.len(), PUBLISHER_SOURCE_CACHE_ENTRIES_V1);
        assert_eq!(
            cache.encoded_bytes,
            cache
                .entries
                .values()
                .map(|(_, source)| source.encoded_bytes)
                .sum::<u64>()
        );
        assert!(
            cache.encoded_bytes
                <= PUBLISHER_SOURCE_CACHE_BYTES_V1.min(backend.config.max_capacity_bytes().0)
        );
        (
            headers
                .iter()
                .find(|header| !cache.entries.contains_key(&header.order_id))
                .unwrap()
                .clone(),
            cache.loads,
        )
    };
    backend
        .stage_publisher_source_chunk(
            &publisher_chunk_request(&evicted, evicted.canonical_digest().unwrap(), 0, payload),
            now,
        )
        .unwrap();
    let cache = backend.publisher_sources.lock().unwrap();
    assert_eq!(cache.loads, loads + 1);
    assert_eq!(cache.entries.len(), PUBLISHER_SOURCE_CACHE_ENTRIES_V1);
    assert!(cache.entries.contains_key(&evicted.order_id));
}

#[test]
fn publisher_source_ingest_rejects_old_revision_and_authority_loss_before_admission() {
    let payload = b"an upload never authorizes its own native completion";
    let (_temp, backend, header, authorization) = publisher_source_fixture(payload);
    let now = crate::unix_now_secs();
    let digest = backend
        .stage_publisher_source(&header, now + 900, now)
        .unwrap();
    backend
        .stage_publisher_source_chunk(&publisher_chunk_request(&header, digest, 0, payload), now)
        .unwrap();
    assert!(
        backend
            .ingest_staged_publisher_source(&authorization, 2, &mut || Ok(()))
            .is_err()
    );
    let mut calls = 0;
    let mut revoke_during_read = || {
        calls += 1;
        if calls >= 3 {
            Err(publisher_source_rejected())
        } else {
            Ok(())
        }
    };
    assert!(
        backend
            .ingest_staged_publisher_source(&authorization, 1, &mut revoke_during_read)
            .is_err()
    );
    assert!(calls >= 3);
    assert_eq!(backend.manifest_count(), 0);
    assert!(
        backend
            .root_dir
            .join(PUBLISHER_SOURCE_DIR_V1)
            .join(hex::encode(header.order_id))
            .exists()
    );
    backend
        .ingest_staged_publisher_source(&authorization, 1, &mut || Ok(()))
        .unwrap()
        .unwrap();
    assert_eq!(backend.manifest_count(), 1);
}

#[test]
fn publisher_source_reader_rechecks_buffered_reads_and_eof_with_sticky_failure() {
    let payload = b"buffered authority";
    let (_temp, backend, header, _) = publisher_source_fixture(payload);
    let now = crate::unix_now_secs();
    let digest = backend
        .stage_publisher_source(&header, now + 900, now)
        .unwrap();
    backend
        .stage_publisher_source_chunk(&publisher_chunk_request(&header, digest, 0, payload), now)
        .unwrap();
    let plan = single_file_plan(payload).unwrap();
    let path = backend
        .root_dir
        .join(PUBLISHER_SOURCE_DIR_V1)
        .join(hex::encode(header.order_id));
    for first_read in [2, payload.len()] {
        let allowed = std::cell::Cell::new(true);
        let mut authority = || {
            if allowed.get() {
                Ok(())
            } else {
                Err(publisher_source_rejected())
            }
        };
        let mut reader = PublisherSourceReaderV1 {
            path: &path,
            plan: &plan,
            index: 0,
            chunk: io::Cursor::new(Vec::new()),
            deadline: now + 900,
            current_authority: &mut authority,
            failed: false,
        };
        let mut buffer = vec![0; first_read];
        assert_eq!(reader.read(&mut buffer).unwrap(), first_read);
        assert_eq!(&buffer, &payload[..first_read]);
        allowed.set(false);
        assert!(reader.read(&mut [0; 1]).is_err());
        allowed.set(true);
        assert!(reader.read(&mut [0; 1]).is_err());
        assert!(reader.read(&mut []).is_err());
    }
}
