//! Native publisher payload-binding and bounded HTTP-read regression tests.
use super::*;

fn site_fixture(root: &Path) -> (PathBuf, CarBuildPlan, Vec<u8>, ManifestV1) {
    let site = root.join("site");
    fs::create_dir_all(site.join("assets")).expect("create site");
    fs::write(
        site.join("index.html"),
        "<!doctype html><html lang=\"ja\">空</html>",
    )
    .expect("index");
    fs::write(site.join("assets/app.js"), "console.log('SORA CARS');").expect("script");
    let (plan, payload) = CarBuildPlan::from_directory_with_profile(&site, ChunkProfile::DEFAULT)
        .expect("directory plan");
    let manifest = manifest(&plan, &payload);
    (site, plan, payload, manifest)
}

fn manifest(plan: &CarBuildPlan, payload: &[u8]) -> ManifestV1 {
    let stats = CarStreamingWriter::new(plan)
        .write_from_reader(&mut payload.as_ref(), &mut io::sink())
        .expect("canonical CAR stats");
    ManifestBuilder::new()
        .root_cid(stats.root_cids[0].clone())
        .dag_codec(DagCodecId(stats.dag_codec))
        .chunking_profile(ChunkingProfileV1::from_descriptor(
            manifest_chunker_registry::lookup_by_handle(DEFAULT_CHUNKER_HANDLE)
                .expect("registered profile"),
        ))
        .chunk_digest_sha3_256(sorafs_car::compute_chunk_plan_digest_sha3(&plan.chunks))
        .por_root(compute_por_root(payload, plan).expect("PoR root"))
        .content_length(plan.content_length)
        .car_digest(*stats.car_archive_digest.as_bytes())
        .car_size(stats.car_size)
        .pin_policy(PinPolicy::default())
        .build()
        .expect("native manifest")
}

#[test]
fn prepared_payload_matches_the_exact_native_directory_and_file_manifest() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().canonicalize().expect("canonical root");
    let (site, _, payload, manifest) = site_fixture(&root);
    let (prepared, files, kind) =
        load_prepared_storage_payload(&site, &manifest).expect("exact directory");
    assert_eq!(prepared, payload);
    assert_eq!(files.expect("file inventory").len(), 2);
    assert_eq!(kind, "directory");
    let blob = root.join("blob.bin");
    fs::write(&blob, &payload).expect("blob");
    let plan = CarBuildPlan::single_file(&payload).expect("blob plan");
    let manifest = self::manifest(&plan, &payload);
    let (prepared, files, kind) =
        load_prepared_storage_payload(&blob, &manifest).expect("exact blob");
    assert_eq!(prepared, payload);
    assert!(files.is_none());
    assert_eq!(kind, "file");
}

#[test]
fn preparation_rejects_each_changed_manifest_commitment() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().canonicalize().expect("canonical root");
    let (site, _, _, manifest) = site_fixture(&root);
    let mut variants = Vec::new();
    let mut wrong = manifest.clone();
    wrong.content_length += 1;
    variants.push(wrong);
    let mut wrong = manifest.clone();
    wrong.chunk_digest_sha3_256[0] ^= 1;
    variants.push(wrong);
    let mut wrong = manifest.clone();
    wrong.por_root[0] ^= 1;
    variants.push(wrong);
    let mut wrong = manifest.clone();
    wrong.root_cid[4] ^= 1;
    variants.push(wrong);
    let mut wrong = manifest.clone();
    wrong.car_digest[0] ^= 1;
    variants.push(wrong);
    let mut wrong = manifest.clone();
    wrong.car_size += 1;
    variants.push(wrong);
    let mut wrong = manifest.clone();
    wrong.dag_codec.0 ^= 1;
    variants.push(wrong);
    let mut wrong = manifest.clone();
    wrong.chunking.target_size += 1;
    variants.push(wrong);
    let mut wrong = manifest.clone();
    wrong.content_length = PREPARED_STORAGE_MAX_PAYLOAD_BYTES + 1;
    variants.push(wrong);
    for (index, wrong) in variants.iter().enumerate() {
        assert!(
            load_prepared_storage_payload(&site, wrong).is_err(),
            "manifest mutation {index}"
        );
    }
}

#[test]
fn preparation_rejects_same_length_source_changes_and_renamed_paths() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().canonicalize().expect("canonical root");
    let (site, _, _, manifest) = site_fixture(&root);
    let script = site.join("assets/app.js");
    let original = fs::read(&script).expect("script");
    let mut changed = original.clone();
    changed[0] ^= 1;
    fs::write(&script, changed).expect("mutate script");
    assert!(load_prepared_storage_payload(&site, &manifest).is_err());
    fs::write(&script, original).expect("restore script");
    fs::rename(&script, site.join("assets/renamed.js")).expect("rename script");
    assert!(load_prepared_storage_payload(&site, &manifest).is_err());
}

#[test]
fn preparation_rejects_bad_manifest_and_payload_before_output_creation() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().canonicalize().expect("canonical root");
    let (site, _, _, manifest) = site_fixture(&root);
    let manifest_path = root.join("manifest.to");
    let payload_out = root.join("payload.bin");
    let files_out = root.join("files.json");
    let args = || {
        vec![
            format!("--manifest={}", manifest_path.display()),
            format!("--payload={}", site.display()),
            format!("--payload-out={}", payload_out.display()),
            format!("--files-out={}", files_out.display()),
        ]
    };
    let mut invalid = manifest.encode().expect("manifest encoding");
    invalid.push(0);
    fs::write(&manifest_path, invalid).expect("trailing manifest");
    assert!(storage_prepare(args()).is_err());
    assert!(!payload_out.exists() && !files_out.exists());
    fs::write(
        &manifest_path,
        manifest.encode().expect("manifest encoding"),
    )
    .expect("canonical manifest");
    fs::write(site.join("index.html"), "substituted site").expect("changed payload");
    assert!(storage_prepare(args()).is_err());
    assert!(!payload_out.exists() && !files_out.exists());
    File::create(&manifest_path)
        .expect("manifest file")
        .set_len(MAX_MANIFEST_ENCODED_BYTES as u64 + 1)
        .expect("oversize manifest");
    assert!(storage_prepare(args()).is_err());
    assert!(!payload_out.exists() && !files_out.exists());
}

#[test]
fn gateway_expectations_cover_every_asset_above_thirty_two_files() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().canonicalize().expect("canonical root");
    let (site, _, _, _) = site_fixture(&root);
    for index in 0..40 {
        fs::write(site.join(format!("assets/{index:02}.bin")), [index as u8]).expect("asset");
    }
    let (plan, payload) = CarBuildPlan::from_directory(&site).expect("large file inventory");
    let manifest = manifest(&plan, &payload);
    let (payload, files, _) =
        load_prepared_storage_payload(&site, &manifest).expect("exact inventory");
    let checks =
        build_gateway_expectations(files.as_deref(), &payload).expect("gateway expectations");
    assert_eq!(
        checks.len(),
        43,
        "all 42 files plus index navigation must be checked"
    );
}

fn response(headers: &str, body: Vec<u8>) -> (reqwest::blocking::Response, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback listener");
    let address = listener.local_addr().expect("listener address");
    let head = format!("HTTP/1.1 200 OK\r\nConnection: close\r\n{headers}\r\n");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("loopback accept");
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .expect("bounded server write");
        let mut request = [0; 4096];
        let _ = stream.read(&mut request);
        if stream.write_all(head.as_bytes()).is_ok() {
            let _ = stream.write_all(&body);
        }
    });
    let client = HttpClient::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .expect("bounded test client");
    let response = client
        .get(format!("http://{address}/"))
        .send()
        .expect("loopback response");
    (response, server)
}

#[test]
fn gateway_hashing_rejects_oversize_and_hashes_exact_bytes_without_whole_body_allocation() {
    let body = b"SORA CARS".to_vec();
    let (reply, server) = response("Content-Length: 9\r\n", body.clone());
    let (length, hash) = hash_gateway_response_bounded(reply, 9).expect("exact response");
    assert_eq!(length, 9);
    assert_eq!(hash, blake3_hash(&body).to_hex().to_string());
    server.join().unwrap();
    let (reply, server) = response("Content-Length: 1000000000\r\n", Vec::new());
    assert!(hash_gateway_response_bounded(reply, 9).is_err());
    server.join().unwrap();
    let (reply, server) = response("", vec![0x41; 10]);
    assert!(hash_gateway_response_bounded(reply, 9).is_err());
    server.join().unwrap();
}

#[test]
fn publish_metadata_response_is_bounded_with_or_without_content_length() {
    let (reply, server) = response("Content-Length: 1000000000\r\n", Vec::new());
    assert!(read_publish_response_bounded(reply, "test").is_err());
    server.join().unwrap();
    let (reply, server) = response("", vec![0; PUBLISH_RESPONSE_MAX_BYTES as usize + 1]);
    assert!(read_publish_response_bounded(reply, "test").is_err());
    server.join().unwrap();
    let (reply, server) = response("", b"{}".to_vec());
    assert_eq!(
        read_publish_response_bounded(reply, "test").expect("small metadata"),
        b"{}"
    );
    server.join().unwrap();
}

#[test]
fn gateway_expectations_use_verified_payload_without_rereading_changed_source_files() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().canonicalize().expect("canonical root");
    let (site, _, _, manifest) = site_fixture(&root);
    let (payload, files, _) =
        load_prepared_storage_payload(&site, &manifest).expect("exact prepared payload");
    let before =
        build_gateway_expectations(files.as_deref(), &payload).expect("original expectations");
    fs::write(site.join("index.html"), "changed after verification").expect("source changed");
    let after =
        build_gateway_expectations(files.as_deref(), &payload).expect("retained expectations");
    assert_eq!(before.len(), after.len());
    for (left, right) in before.iter().zip(after.iter()) {
        assert_eq!(left.path, right.path);
        assert_eq!(left.bytes, right.bytes);
        assert_eq!(left.blake3_hex, right.blake3_hex);
    }
    let mut invalid = files.expect("directory files");
    invalid.reverse();
    assert!(build_gateway_expectations(Some(&invalid), &payload).is_err());
}
