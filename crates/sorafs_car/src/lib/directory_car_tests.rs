// Directory CAR and chunk-fetch-plan regressions.
#[test]
fn empty_input_rejected() {
    let err = CarBuildPlan::single_file(&[]).unwrap_err();
    assert!(matches!(err, CarPlanError::EmptyInput));
}
#[test]
fn windows_directory_chunk_publication_requires_private_parent() {
    let bytes = b"payload";
    let plan = CarBuildPlan::single_file(bytes).expect("plan");
    let temporary = tempdir().expect("native fixture root");
    let refused_root = temporary.path().join("chunks");
    let mut refused = DirectoryChunkSink::new(&refused_root);
    assert!(matches!(
        refused.prepare(&plan),
        Err(ChunkStoreError::Io(error)) if error.kind() == io::ErrorKind::PermissionDenied
    ));
    assert!(!refused_root.exists());
    assert_eq!(std::fs::read_dir(temporary.path()).unwrap().count(), 0);
    drop(refused);

    let parent = iroha_fs::PrivateDirectory::open_or_create(temporary.path().join("private"))
        .expect("canonical private parent");
    let root = parent.path().join("chunks");
    let mut sink = DirectoryChunkSink::new(&root);
    sink.prepare(&plan).expect("supported native prepare");
    assert_eq!(plan.chunks.len(), 1);
    sink.write_chunk(0, &plan.chunks[0], bytes)
        .expect("original native writer");
    assert!(
        !root.exists(),
        "unpublished chunks stay in original staging"
    );
    let output = sink.finish().expect("native publication");
    assert_eq!(output.publication, DirectoryPublicationStatus::Durable);
    assert_eq!(output.total_bytes, bytes.len() as u64);
    assert_eq!(output.records.len(), 1);
    assert_eq!(
        std::fs::read(root.join(&output.records[0].file_name)).unwrap(),
        bytes
    );
    assert!(iroha_fs::PrivateDirectory::open(temporary.path()).is_err());
}
#[test]
fn chunk_fetch_specs_reflect_plan() {
    let input = sample_input();
    let plan = CarBuildPlan::single_file(&input).expect("plan");
    let specs = plan.try_chunk_fetch_specs().expect("fetch specs");
    assert_eq!(specs.len(), plan.chunks.len());
    for (idx, spec) in specs.iter().enumerate() {
        let chunk = &plan.chunks[idx];
        assert_eq!(spec.chunk_index, idx);
        assert_eq!(spec.offset, chunk.offset);
        assert_eq!(spec.length, chunk.length);
        assert_eq!(spec.digest, chunk.digest);
    }
}
#[test]
fn try_chunk_fetch_specs_rejects_invalid_plan_instead_of_returning_empty() {
    let input = sample_input();
    let mut plan = CarBuildPlan::single_file(&input).expect("plan");
    plan.chunks[0].offset = 1;
    assert!(matches!(
        plan.try_chunk_fetch_specs(),
        Err(CarPlanError::InvalidPlan(
            CarPlanValidationError::NonContiguousChunk { chunk_index: 0, .. }
        ))
    ));
}
#[test]
fn directory_car_emits_directory_root() {
    use std::io::Cursor;
    let files = vec![
        FileEntry {
            path: vec!["docs".to_string(), "index.html".to_string()],
            data: b"<html></html>".to_vec(),
        },
        FileEntry {
            path: vec!["docs".to_string(), "style.css".to_string()],
            data: b"body { background: #fff; }".to_vec(),
        },
    ];
    let (plan, payload) = CarBuildPlan::from_files(files).expect("plan");
    assert_eq!(plan.files.len(), 2);
    assert_eq!(
        plan.files[0].path,
        vec!["docs".to_string(), "index.html".to_string()]
    );
    assert_eq!(
        plan.files[1].path,
        vec!["docs".to_string(), "style.css".to_string()]
    );
    let writer = CarWriter::new(&plan, &payload).expect("writer");
    let mut buffer = Cursor::new(Vec::new());
    let stats = writer.write_to(&mut buffer).expect("write directory car");
    assert_eq!(stats.root_cids.len(), 1);
    let bytes = buffer.into_inner();
    let header_offset = PRAGMA.len() + HEADER_LEN;
    let data_offset = u64::from_le_bytes(
        bytes[PRAGMA.len() + 16..PRAGMA.len() + 24]
            .try_into()
            .unwrap(),
    );
    let data_size = u64::from_le_bytes(
        bytes[PRAGMA.len() + 24..PRAGMA.len() + 32]
            .try_into()
            .unwrap(),
    );
    assert_eq!(data_offset as usize, header_offset);
    let mut section_cid_map: BTreeMap<Vec<u8>, Vec<u8>> = BTreeMap::new();
    let mut cursor = header_offset;
    let (header_len, header_len_bytes) = decode_uleb128(&bytes[cursor..]);
    cursor += header_len_bytes + header_len as usize;
    let data_end = data_offset + data_size;
    while (cursor as u64) < data_end {
        let (section_len, len_bytes) = decode_uleb128(&bytes[cursor..]);
        cursor += len_bytes;
        let (cid, cid_len) = verifier::decode_cid(&bytes[cursor..], section_cid_map.len())
            .expect("canonical directory section CID");
        assert_eq!(cid.multihash, BLAKE3_256_MULTIHASH_CODE);
        let cid_bytes = bytes[cursor..cursor + cid_len].to_vec();
        cursor += cid_len;
        let data_len = section_len as usize - cid_len;
        let data_slice = bytes[cursor..cursor + data_len].to_vec();
        cursor += data_len;
        section_cid_map.insert(cid_bytes, data_slice);
    }
    assert_eq!(cursor as u64, data_end);
    let root_cid = stats.root_cids[0].clone();
    let root_data = section_cid_map
        .get(&root_cid)
        .expect("root directory node present");
    let (root_entries, root_size) = parse_directory_node(root_data);
    assert_eq!(root_entries.len(), 1);
    assert_eq!(root_size as usize, payload.len());
    let docs_entry = &root_entries[0];
    assert_eq!(docs_entry.name, "docs");
    assert!(matches!(docs_entry.kind, DirectoryEntryKind::Directory));
    let docs_data = section_cid_map
        .get(&docs_entry.cid)
        .expect("docs directory node present");
    let (docs_entries, docs_size) = parse_directory_node(docs_data);
    assert_eq!(docs_entries.len(), 2);
    assert_eq!(
        docs_size as usize,
        plan.files.iter().map(|f| f.size as usize).sum::<usize>()
    );
    let mut names: Vec<_> = docs_entries.iter().map(|e| e.name.as_str()).collect();
    names.sort();
    assert_eq!(names, vec!["index.html", "style.css"]);
    for entry in docs_entries {
        assert!(matches!(entry.kind, DirectoryEntryKind::File));
        let size = plan
            .files
            .iter()
            .find(|f| f.path.last().unwrap() == &entry.name)
            .map(|f| f.size)
            .expect("matching file plan");
        assert_eq!(entry.size, size);
    }
}
#[derive(Debug, Clone)]
struct DirEntryView {
    name: String,
    kind: DirectoryEntryKind,
    cid: Vec<u8>,
    size: u64,
}
fn parse_directory_node(data: &[u8]) -> (Vec<DirEntryView>, u64) {
    let mut idx = 0usize;
    let (map_len, consumed) = decode_cbor_map_len(&data[idx..]);
    assert_eq!(map_len, 4);
    idx += consumed;
    let (key_entries, consumed) = decode_cbor_text(&data[idx..]);
    assert_eq!(key_entries, "entries");
    idx += consumed;
    let (entries_len, consumed) = decode_cbor_array_len(&data[idx..]);
    idx += consumed;
    let mut entries = Vec::with_capacity(entries_len as usize);
    for _ in 0..entries_len {
        let (entry_map_len, consumed) = decode_cbor_map_len(&data[idx..]);
        assert_eq!(entry_map_len, 4);
        idx += consumed;
        let (name_key, consumed) = decode_cbor_text(&data[idx..]);
        assert_eq!(name_key, "name");
        idx += consumed;
        let (name_value, consumed) = decode_cbor_text(&data[idx..]);
        idx += consumed;
        let (cid_key, consumed) = decode_cbor_text(&data[idx..]);
        assert_eq!(cid_key, "cid");
        idx += consumed;
        let (cid_value, consumed) = decode_cbor_bytes(&data[idx..]);
        idx += consumed;
        let (kind_key, consumed) = decode_cbor_text(&data[idx..]);
        assert_eq!(kind_key, "kind");
        idx += consumed;
        let (kind_value, consumed) = decode_cbor_text(&data[idx..]);
        idx += consumed;
        let kind = match kind_value {
            "file" => DirectoryEntryKind::File,
            "dir" => DirectoryEntryKind::Directory,
            other => panic!("unexpected directory entry kind {other}"),
        };
        let (size_key, consumed) = decode_cbor_text(&data[idx..]);
        assert_eq!(size_key, "size");
        idx += consumed;
        let (size_value, consumed) = decode_cbor_uint(&data[idx..]);
        idx += consumed;
        entries.push(DirEntryView {
            name: name_value.to_owned(),
            kind,
            cid: cid_value,
            size: size_value,
        });
    }
    let (key_size, consumed) = decode_cbor_text(&data[idx..]);
    assert_eq!(key_size, "size");
    idx += consumed;
    let (dir_size, consumed) = decode_cbor_uint(&data[idx..]);
    idx += consumed;
    let (key_type, consumed) = decode_cbor_text(&data[idx..]);
    assert_eq!(key_type, "type");
    idx += consumed;
    let (type_value, consumed) = decode_cbor_text(&data[idx..]);
    assert_eq!(type_value, DIR_NODE_TYPE);
    idx += consumed;
    let (key_version, consumed) = decode_cbor_text(&data[idx..]);
    assert_eq!(key_version, "version");
    idx += consumed;
    let (version_value, consumed) = decode_cbor_uint(&data[idx..]);
    assert_eq!(version_value, DAG_NODE_VERSION);
    idx += consumed;
    assert_eq!(idx, data.len());
    (entries, dir_size)
}
