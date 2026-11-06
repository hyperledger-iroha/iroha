//! Offline regression coverage for pinned native publication packages.

use super::*;

struct Fixture {
    _directory: tempfile::TempDir,
    root: PathBuf,
    package: PreparedPackage,
    plan: CarBuildPlan,
    payload: Vec<u8>,
    manifest: ManifestV1,
}

fn pinned(path: PathBuf, bytes: &[u8]) -> PinnedInput {
    fs::write(&path, bytes).expect("write pinned fixture");
    PinnedInput {
        path,
        sha256_hex: hex::encode(Sha256::digest(bytes)),
        blake3_hex: blake3::hash(bytes).to_hex().to_string(),
    }
}

fn fixture(directory: bool) -> Fixture {
    let temp = tempfile::tempdir().expect("fixture directory");
    let root = temp.path().canonicalize().expect("canonical fixture root");
    let (plan, payload) = if directory {
        CarBuildPlan::from_files(vec![
            sorafs_car::FileEntry {
                path: vec!["index.html".into()],
                data: b"<!doctype html><title>SORA CARS</title>".to_vec(),
            },
            sorafs_car::FileEntry {
                path: vec!["assets".into(), "app.js".into()],
                data: b"console.log('SORA CARS');".to_vec(),
            },
            sorafs_car::FileEntry {
                path: vec!["empty.txt".into()],
                data: Vec::new(),
            },
        ])
        .expect("native directory plan")
    } else {
        let payload = b"native single-file publication".to_vec();
        let plan = CarBuildPlan::single_file(&payload).expect("native file plan");
        (plan, payload)
    };
    let stats = sorafs_car::CarWriter::new(&plan, &payload)
        .expect("canonical CAR writer")
        .write_to(io::sink())
        .expect("canonical CAR commitments");
    let manifest = ManifestBuilder::new()
        .root_cid(stats.root_cids[0].clone())
        .dag_codec(DagCodecId(stats.dag_codec))
        .chunking_from_profile(
            plan.chunk_profile,
            sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
        )
        .chunk_digest_sha3_256(sorafs_car::compute_chunk_plan_digest_sha3(&plan.chunks))
        .por_root(compute_por_root(&payload, &plan).expect("native PoR root"))
        .content_length(plan.content_length)
        .car_digest(*stats.car_archive_digest.as_bytes())
        .car_size(stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 3,
            storage_class: StorageClass::Hot,
            retention_epoch: 1_796_573_325,
        })
        .build()
        .expect("canonical manifest");
    let files = directory.then(|| {
        plan.files
            .iter()
            .map(|file| FileEntry {
                path: file.path.clone(),
                size: file.size,
            })
            .collect::<Vec<_>>()
    });
    let package = PreparedPackage {
        schema: SCHEMA.into(),
        manifest: pinned(
            root.join("manifest.to"),
            &manifest.encode().expect("canonical manifest bytes"),
        ),
        chunk_plan: pinned(
            root.join("plan.json"),
            chunk_fetch_plan_to_string(&plan)
                .expect("native fetch plan")
                .as_bytes(),
        ),
        files: pinned(
            root.join("files.json"),
            &to_vec(&files).expect("file inventory JSON"),
        ),
        payload: pinned(root.join("payload.bin"), &payload),
    };
    Fixture {
        _directory: temp,
        root,
        package,
        plan,
        payload,
        manifest,
    }
}

fn request(root: &Path, value: &Value) -> (PathBuf, String) {
    let path = root.join("prepared.json");
    let bytes = to_vec(value).expect("request JSON");
    fs::write(&path, &bytes).expect("write request");
    (path, blake3::hash(&bytes).to_hex().to_string())
}

#[test]
fn selection_requires_exact_paired_options_and_pinned_request_bytes() {
    let fixture = fixture(true);
    let (path, pin) = request(&fixture.root, &fixture.package.receipt());
    let selected = select(Some(path.clone()), Some(pin.clone()), None)
        .expect("paired options")
        .expect("prepared package");
    assert_eq!(selected.receipt(), fixture.package.receipt());
    assert_eq!(selected.payload_path(), fixture.package.payload.path);
    assert!(select(None, None, None).expect("no preparation").is_none());
    assert!(
        select(None, None, Some(&fixture.package.payload.path))
            .expect("ordinary payload mode")
            .is_none()
    );
    for (path, pin, payload) in [
        (Some(path.clone()), None, None),
        (None, Some(pin.clone()), None),
        (
            Some(path.clone()),
            Some(pin.clone()),
            Some(fixture.package.payload.path.as_path()),
        ),
    ] {
        assert!(select(path, pin, payload).is_err());
    }
    assert!(select(Some(path.clone()), Some("00".repeat(32)), None).is_err());
    assert!(select(Some(path.clone()), Some("invalid".into()), None).is_err());
    let mut changed = fs::read(&path).expect("request bytes");
    changed.push(b' ');
    fs::write(&path, changed).expect("change otherwise equivalent request bytes");
    assert!(select(Some(path), Some(pin), None).is_err());
}

#[test]
fn selection_rejects_unknown_schema_fields_relative_paths_and_malformed_pins() {
    let fixture = fixture(true);
    for case in 0..6 {
        let mut value = fixture.package.receipt();
        let package = value.as_object_mut().expect("package object");
        match case {
            0 => {
                package.insert(
                    "schema".into(),
                    Value::from("sorafs.prepared_deployment.v2"),
                );
            }
            1 => {
                package.insert("unreviewed".into(), Value::from(true));
            }
            _ => {
                let payload = package
                    .get_mut("payload")
                    .and_then(Value::as_object_mut)
                    .expect("payload object");
                let (field, value) = match case {
                    2 => ("unreviewed", "extra"),
                    3 => ("path", "payload.bin"),
                    4 => ("sha256_hex", "invalid"),
                    _ => ("blake3_hex", "invalid"),
                };
                payload.insert(field.into(), Value::from(value));
            }
        }
        let (path, pin) = request(&fixture.root, &value);
        assert!(select(Some(path), Some(pin), None).is_err(), "case {case}");
    }
}

#[test]
fn metadata_inputs_require_both_hashes_and_the_read_bound() {
    let fixture = fixture(true);
    for input in [
        &fixture.package.manifest,
        &fixture.package.chunk_plan,
        &fixture.package.files,
    ] {
        let original = fs::read(&input.path).expect("metadata bytes");
        assert_eq!(
            input
                .read(original.len() as u64, "fixture")
                .expect("exact bounds and pins"),
            original
        );
        assert!(input.read(original.len() as u64 - 1, "fixture").is_err());
        for change_sha in [false, true] {
            let changed = PinnedInput {
                path: input.path.clone(),
                sha256_hex: if change_sha {
                    "00".repeat(32)
                } else {
                    input.sha256_hex.clone()
                },
                blake3_hex: if change_sha {
                    input.blake3_hex.clone()
                } else {
                    "00".repeat(32)
                },
            };
            assert!(changed.read(MAX_METADATA_BYTES, "fixture").is_err());
        }
    }
}

#[test]
fn prepared_directory_preserves_manifest_policy_layout_and_gateway_expectations() {
    let fixture = fixture(true);
    let mut artifacts = fixture.package.load().expect("prepared directory");
    assert!(matches!(&artifacts.payload, DeployPayload::Prepared(_)));
    assert_eq!(artifacts.plan, fixture.plan);
    assert_eq!(
        artifacts.manifest.encode().expect("loaded manifest"),
        fixture.manifest.encode().expect("reviewed manifest")
    );
    assert_eq!(artifacts.manifest.pin_policy, fixture.manifest.pin_policy);
    assert_eq!(artifacts.payload_kind, "prepared-directory");
    assert_eq!(
        artifacts.payload_digest_hex,
        fixture.plan.payload_digest.to_hex().to_string()
    );
    assert_eq!(
        artifacts.root_cid_hex,
        hex::encode(&fixture.manifest.root_cid)
    );
    assert_eq!(
        artifacts.manifest_digest_hex,
        fixture
            .manifest
            .digest()
            .expect("manifest digest")
            .to_hex()
            .to_string()
    );
    let mut actual = Vec::new();
    artifacts
        .payload
        .read_to_end(&mut actual)
        .expect("stream payload");
    assert_eq!(actual, fixture.payload);
    assert_eq!(
        artifacts.gateway_expectations.len(),
        fixture.plan.files.len() + 1
    );
    let alias = &artifacts.gateway_expectations[0];
    assert!(alias.path.is_none());
    let mut offset = 0;
    for (file, expected) in fixture
        .plan
        .files
        .iter()
        .zip(&artifacts.gateway_expectations[1..])
    {
        let end = offset + file.size as usize;
        assert_eq!(expected.path.as_ref(), Some(&file.path));
        assert_eq!(expected.bytes, file.size);
        assert_eq!(
            expected.blake3_hex,
            blake3::hash(&fixture.payload[offset..end])
                .to_hex()
                .to_string()
        );
        if file.path.as_slice() == ["index.html"] {
            assert_eq!(alias.bytes, expected.bytes);
            assert_eq!(alias.blake3_hex, expected.blake3_hex);
        }
        offset = end;
    }
    artifacts
        .payload
        .seek(SeekFrom::Start(1))
        .expect("random access");
    let mut byte = [0];
    artifacts
        .payload
        .read_exact(&mut byte)
        .expect("read at offset");
    assert_eq!(byte[0], fixture.payload[1]);
}

#[test]
fn raw_file_inventory_remains_a_single_file_without_a_directory_alias() {
    let fixture = fixture(false);
    let artifacts = fixture.package.load().expect("prepared single file");
    assert_eq!(artifacts.plan, fixture.plan);
    assert_eq!(artifacts.payload_kind, "prepared-file");
    assert_eq!(artifacts.gateway_expectations.len(), 1);
    let expected = &artifacts.gateway_expectations[0];
    assert!(expected.path.is_none());
    assert_eq!(expected.bytes, fixture.payload.len() as u64);
    assert_eq!(expected.blake3_hex, fixture.package.payload.blake3_hex);
}

#[test]
fn toolkit_report_preserves_the_same_reviewed_native_package() {
    let mut fixture = fixture(true);
    let mut report: Value =
        from_slice(&fs::read(&fixture.package.chunk_plan.path).expect("fetch plan bytes"))
            .expect("fetch plan JSON");
    let object = report.as_object_mut().expect("report object");
    let digest = object
        .remove("payload_digest_blake3_hex")
        .expect("payload digest");
    object.insert("schema".into(), Value::from(TOOLKIT_PACK_REPORT_SCHEMA_V1));
    object.insert("payload_digest_hex".into(), digest);
    fixture.package.chunk_plan = pinned(
        fixture.package.chunk_plan.path.clone(),
        &to_vec(&report).expect("toolkit report JSON"),
    );
    let artifacts = fixture.package.load().expect("native toolkit report");
    assert_eq!(artifacts.plan, fixture.plan);
    assert_eq!(artifacts.manifest.pin_policy, fixture.manifest.pin_policy);
}

#[test]
fn repinned_file_inventory_cannot_change_native_paths_boundaries_or_coverage() {
    for case in 0..6 {
        let mut fixture = fixture(true);
        let mut files: Value =
            from_slice(&fs::read(&fixture.package.files.path).expect("file table bytes"))
                .expect("file table JSON");
        let entries = files.as_array_mut().expect("directory file table");
        match case {
            0 => entries.reverse(),
            1 => {
                entries.pop();
            }
            _ => {
                let entry = entries[0].as_object_mut().expect("first file");
                match case {
                    2 => {
                        entry.insert(
                            "path".into(),
                            to_value(&vec!["assets", "renamed.js"]).unwrap(),
                        );
                    }
                    3 => {
                        entry.insert("path".into(), to_value(&vec!["..", "app.js"]).unwrap());
                    }
                    4 => {
                        entry.insert("size".into(), Value::from(fixture.plan.files[0].size - 1));
                    }
                    _ => {
                        entry.insert("unreviewed".into(), Value::from(true));
                    }
                }
            }
        }
        fixture.package.files = pinned(
            fixture.package.files.path.clone(),
            &to_vec(&files).expect("changed file table"),
        );
        assert!(fixture.package.load().is_err(), "file table case {case}");
    }
}

#[test]
fn repinned_chunk_plan_cannot_change_order_offsets_digests_or_payload_identity() {
    for case in 0..4 {
        let mut fixture = fixture(true);
        let mut plan: Value =
            from_slice(&fs::read(&fixture.package.chunk_plan.path).expect("plan bytes"))
                .expect("plan JSON");
        let object = plan.as_object_mut().expect("plan object");
        if case == 3 {
            object.insert(
                "payload_digest_blake3_hex".into(),
                Value::from("11".repeat(32)),
            );
        } else {
            let chunks = object
                .get_mut("chunk_fetch_specs")
                .and_then(Value::as_array_mut)
                .expect("chunk specifications");
            let chunk = chunks[0].as_object_mut().expect("first chunk");
            let (field, value) = match case {
                0 => ("chunk_index", Value::from(1_u64)),
                1 => ("offset", Value::from(1_u64)),
                _ => ("digest_blake3", Value::from("11".repeat(32))),
            };
            chunk.insert(field.into(), value);
        }
        fixture.package.chunk_plan = pinned(
            fixture.package.chunk_plan.path.clone(),
            &to_vec(&plan).expect("changed plan"),
        );
        assert!(fixture.package.load().is_err(), "chunk plan case {case}");
    }
}

#[test]
fn repinned_manifest_must_match_every_native_content_commitment() {
    for case in 0..5 {
        let mut fixture = fixture(true);
        let mut manifest = fixture.manifest.clone();
        match case {
            0 => manifest.root_cid[10] ^= 1,
            1 => manifest.car_digest[0] ^= 1,
            2 => manifest.por_root[0] ^= 1,
            3 => manifest.chunk_digest_sha3_256[0] ^= 1,
            _ => manifest.car_size += 1,
        }
        fixture.package.manifest = pinned(
            fixture.package.manifest.path.clone(),
            &manifest.encode().expect("changed canonical manifest"),
        );
        assert!(fixture.package.load().is_err(), "manifest case {case}");
    }
}

#[test]
fn payload_corruption_truncation_trailing_bytes_and_wrong_sha256_are_rejected() {
    for case in 0..4 {
        let mut fixture = fixture(true);
        let mut payload = fixture.payload.clone();
        match case {
            0 => payload[0] ^= 1,
            1 => {
                payload.pop();
            }
            2 => payload.push(0),
            _ => fixture.package.payload.sha256_hex = "00".repeat(32),
        }
        fs::write(&fixture.package.payload.path, payload).expect("changed payload");
        assert!(fixture.package.load().is_err(), "payload case {case}");
    }
}

#[cfg(unix)]
#[test]
fn verified_payload_rejects_replacement_of_its_retained_source() {
    let fixture = fixture(true);
    let mut artifacts = fixture.package.load().expect("verified retained payload");
    fs::rename(
        &fixture.package.payload.path,
        fixture.root.join("original-payload.bin"),
    )
    .expect("move original retained object");
    fs::write(&fixture.package.payload.path, &fixture.payload)
        .expect("replace with identical bytes");
    let mut bytes = [0_u8; 1];
    assert!(artifacts.payload.read_exact(&mut bytes).is_err());
}

#[test]
fn ordinary_memory_payload_preserves_read_and_seek_behavior() {
    let mut payload = DeployPayload::Memory(Cursor::new(vec![1, 2, 3]));
    payload.seek(SeekFrom::Start(1)).expect("memory seek");
    let mut bytes = [0_u8; 2];
    payload.read_exact(&mut bytes).expect("memory read");
    assert_eq!(bytes, [2, 3]);
    payload.rewind().expect("memory rewind");
    payload.read_exact(&mut bytes).expect("memory prefix");
    assert_eq!(bytes, [1, 2]);
}
