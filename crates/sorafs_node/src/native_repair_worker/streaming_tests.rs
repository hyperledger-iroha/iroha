// Real storage tests prove remote chunks are installed before the producer emits the next one.
fn repair_stream_fixture() -> (
    tempfile::TempDir,
    StorageBackend,
    StoredManifest,
    Vec<Vec<u8>>,
) {
    use sorafs_car::{CarBuildPlan, CarChunk, CarWriter, FilePlan};
    use sorafs_manifest::{DagCodecId, ManifestBuilder, PinPolicy};
    let directory = tempfile::tempdir().unwrap();
    let config = crate::config::StorageConfig::builder()
        .enabled(true)
        .data_dir(directory.path().canonicalize().unwrap().join("store"))
        .build();
    let backend = StorageBackend::new(config).unwrap();
    let size = sorafs_chunker::ChunkProfile::DEFAULT.min_size;
    let chunks: Vec<Vec<u8>> = (1..=3).map(|seed| vec![seed; size]).collect();
    let payload = chunks.concat();
    let plan = CarBuildPlan {
        chunk_profile: sorafs_chunker::ChunkProfile::DEFAULT,
        payload_digest: blake3::hash(&payload),
        content_length: payload.len() as u64,
        chunks: chunks
            .iter()
            .enumerate()
            .map(|(index, bytes)| CarChunk {
                offset: (index * size) as u64,
                length: size as u32,
                digest: *blake3::hash(bytes).as_bytes(),
            })
            .collect(),
        files: vec![FilePlan {
            path: vec!["payload".into()],
            first_chunk: 0,
            chunk_count: chunks.len(),
            size: payload.len() as u64,
        }],
    };
    let stats = CarWriter::new(&plan, &payload)
        .unwrap()
        .write_to(std::io::sink())
        .unwrap();
    let manifest = ManifestBuilder::new()
        .root_cid(stats.root_cids[0].clone())
        .dag_codec(DagCodecId(stats.dag_codec))
        .chunking_from_profile(
            plan.chunk_profile,
            sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
        )
        .chunk_digest_sha3_256(sorafs_car::compute_chunk_plan_digest_sha3(&plan.chunks))
        .por_root(sorafs_car::compute_por_root(&payload, &plan).unwrap())
        .content_length(plan.content_length)
        .car_digest(*stats.car_archive_digest.as_bytes())
        .car_size(stats.car_size)
        .pin_policy(PinPolicy::default())
        .build()
        .unwrap();
    let id = backend
        .ingest_manifest(&manifest, &plan, &mut payload.as_slice())
        .unwrap();
    let stored = backend.manifest(&id).unwrap();
    for index in 0..stored.chunk_count() {
        std::fs::write(&stored.chunk(index).unwrap().path, vec![0; size]).unwrap();
    }
    (directory, backend, stored, chunks)
}

#[derive(Debug)]
struct StreamingRepairFixture {
    chunks: Vec<Vec<u8>>,
    repeat_first: bool,
}
impl RepairOrchestrator for StreamingRepairFixture {
    fn rehydrate_missing_chunks(
        &self,
        _: &NativeRepairExecutionContextV1,
        _: &StoredManifest,
        invalid: &[&ChunkFileRecord],
        sink: &mut dyn FnMut(RepairChunkPayload) -> Result<(), RepairOrchestratorError>,
    ) -> Result<(), RepairOrchestratorError> {
        for (index, target) in invalid.iter().enumerate() {
            if index > 0 {
                assert!(
                    read_valid_chunk(invalid[index - 1]).is_some(),
                    "previous payload must be consumed before the next response"
                );
            }
            let payload = RepairChunkPayload {
                digest: target.digest,
                bytes: self.chunks[index].clone(),
                source: None,
            };
            if self.repeat_first {
                sink(payload.clone())?;
                sink(payload)?;
            } else {
                sink(payload)?;
            }
        }
        Ok(())
    }
}

#[test]
fn native_remote_repair_consumes_and_verifies_each_chunk_before_next_delivery() {
    let (_directory, backend, stored, chunks) = repair_stream_fixture();
    let mut context = evidence_context(0xa1);
    context.manifest_digest = *stored.manifest_digest();
    let source = StreamingRepairFixture {
        chunks,
        repeat_first: false,
    };
    let result = execute_storage_repair(&backend, Some(&source), &context, &|| Ok(())).unwrap();
    assert_eq!(result.invalid_before, 3);
    assert_eq!(result.rehydrated, 3);
    assert_eq!(result.invalid_after, 0);
    assert!(result.failure.is_none());
    for index in 0..stored.chunk_count() {
        assert!(read_valid_chunk(stored.chunk(index).unwrap()).is_some());
    }
}

#[test]
fn native_remote_repair_rejects_duplicate_delivery_before_terminal_success() {
    let (_directory, backend, stored, chunks) = repair_stream_fixture();
    let mut context = evidence_context(0xa1);
    context.manifest_digest = *stored.manifest_digest();
    let source = StreamingRepairFixture {
        chunks,
        repeat_first: true,
    };
    assert!(matches!(
        execute_storage_repair(&backend, Some(&source), &context, &|| Ok(())),
        Err(NativeRepairExecutionErrorV1::Orchestrator(_))
    ));
    assert!(read_valid_chunk(stored.chunk(1).unwrap()).is_none());
}

#[test]
fn native_repair_admission_bounds_metadata_and_one_chunk_without_total_payload_ceiling() {
    // Four GiB of declared chunks exercises the actual admission function without allocating or
    // writing the payload. Only the per-chunk maximum and protocol inventory are admitted.
    validate_repair_inventory(1024, std::iter::repeat_n(Some(4 * 1024 * 1024), 1024)).unwrap();
    assert!(validate_repair_inventory(NATIVE_REPAIR_MAX_CHUNKS_V1 + 1, []).is_err());
    assert!(
        validate_repair_inventory(1, [Some(sorafs_car::CHUNK_STORE_MAX_CHUNK_BYTES + 1)]).is_err()
    );
    assert!(validate_repair_inventory(1, [None]).is_err());
}

#[test]
fn repair_stops_before_the_next_write_when_current_lease_authority_changes() {
    let (_directory, backend, stored, chunks) = repair_stream_fixture();
    let mut context = evidence_context(0xa1);
    context.manifest_digest = *stored.manifest_digest();
    let source = StreamingRepairFixture {
        chunks,
        repeat_first: false,
    };
    let first = stored.chunk(0).unwrap();
    let guard = || {
        if read_valid_chunk(first).is_some() {
            Err(NativeRepairExecutionErrorV1::LeaseInvalid)
        } else {
            Ok(())
        }
    };
    assert!(execute_storage_repair(&backend, Some(&source), &context, &guard).is_err());
    assert!(read_valid_chunk(first).is_some());
    assert!(read_valid_chunk(stored.chunk(1).unwrap()).is_none());
    assert!(read_valid_chunk(stored.chunk(2).unwrap()).is_none());
}
