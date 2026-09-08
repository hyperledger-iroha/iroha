//! Real CAR-to-provider ingestion, byte retrieval, corruption, and restart qualification.
//!
//! These tests use the production CAR verifier and persistent node storage. They deliberately
//! make no claim about finalized assignment, remote source authentication, or gateway routing.
use std::{fs, io, path::Path};

use sorafs_car::{
    CarBuildPlan, CarVerifier, CarWriter, FileEntry, compute_chunk_plan_digest_sha3,
    compute_por_root,
};
use sorafs_manifest::{
    BLAKE3_256_MULTIHASH_CODE, DagCodecId, ManifestBuilder, ManifestV1, PinPolicy,
    decode_manifest_v1_canonical,
};
use sorafs_node::{NodeHandle, config::StorageConfig};

fn fixture() -> (CarBuildPlan, Vec<u8>, ManifestV1, Vec<u8>) {
    let (plan, payload) = CarBuildPlan::from_files_with_profile(
        vec![
            FileEntry {
                path: vec!["index.html".into()],
                data: "<!doctype html><html lang=\"ja\">SORA CARS 空</html>"
                    .as_bytes()
                    .to_vec(),
            },
            FileEntry {
                path: vec!["assets".into(), "road.bin".into()],
                data: (0..524_288_u32)
                    .map(|i| ((i.wrapping_mul(73) ^ (i >> 9)) & 255) as u8)
                    .collect(),
            },
            FileEntry {
                path: vec!["music".into(), "night.mid".into()],
                data: b"MThd\0\0\0\x06\0\0\0\x01\0\x60MTrk\0\0\0\x04\0\xff\x2f\0".to_vec(),
            },
        ],
        sorafs_chunker::ChunkProfile::DEFAULT,
    )
    .expect("canonical multi-file plan");
    let mut car = Vec::new();
    let stats = CarWriter::new(&plan, &payload)
        .expect("CAR writer")
        .write_to(&mut car)
        .expect("write actual CAR bytes");
    let manifest = ManifestBuilder::new()
        .root_cid(stats.root_cids[0].clone())
        .dag_codec(DagCodecId(stats.dag_codec))
        .chunking_from_profile(plan.chunk_profile, BLAKE3_256_MULTIHASH_CODE)
        .chunk_digest_sha3_256(compute_chunk_plan_digest_sha3(&plan.chunks))
        .por_root(compute_por_root(&payload, &plan).expect("PoR root"))
        .content_length(plan.content_length)
        .car_digest(*stats.car_archive_digest.as_bytes())
        .car_size(stats.car_size)
        .pin_policy(PinPolicy::default())
        .build()
        .expect("manifest");
    (plan, payload, manifest, car)
}

fn config(root: &Path) -> StorageConfig {
    StorageConfig::builder()
        .enabled(true)
        .data_dir(root.join("provider"))
        .build()
}

fn assert_readback(
    node: &NodeHandle,
    id: &str,
    manifest: &ManifestV1,
    plan: &CarBuildPlan,
    payload: &[u8],
) {
    let backend = node.storage().expect("real provider backend");
    let stored = backend.manifest(id).expect("durable manifest");
    assert_eq!(stored.manifest_cid(), manifest.root_cid);
    assert_eq!(
        stored.manifest_digest(),
        manifest.digest().expect("digest").as_bytes()
    );
    assert_eq!(stored.content_length(), payload.len() as u64);
    let mut offset = 0_usize;
    for file in &plan.files {
        let size = usize::try_from(file.size).expect("file size");
        let fetched = node
            .read_payload_range(id, offset as u64, size)
            .expect("read exact file");
        assert_eq!(
            fetched,
            payload[offset..offset + size],
            "{}",
            file.path.join("/")
        );
        offset += size;
    }
    assert_eq!(offset, payload.len());
    for chunk in &plan.chunks {
        let (record, bytes) = node
            .read_chunk_by_digest(id, &chunk.digest)
            .expect("read authenticated chunk");
        assert_eq!(record.offset, chunk.offset);
        assert_eq!(
            bytes,
            payload[chunk.offset as usize..chunk.offset as usize + chunk.length as usize]
        );
    }
    // Exercise a range across a chunk boundary, not just whole-file reads.
    if let Some(chunk) = plan.chunks.get(1) {
        let start = chunk.offset - 1;
        assert_eq!(
            node.read_payload_range(id, start, 3)
                .expect("cross-chunk range"),
            payload[start as usize..start as usize + 3]
        );
    }
    for (_, proof) in node
        .sample_por(id, 8, 0x534f5241)
        .expect("native PoR sampling")
    {
        assert!(proof.verify(stored.por_tree().root()));
    }
}

fn roundtrip(plan: &CarBuildPlan, payload: &[u8], manifest: &ManifestV1, car: &[u8]) {
    let report = CarVerifier::verify_full_car_with_plan(manifest, plan, car)
        .expect("authenticate complete CAR");
    assert_eq!(*report.chunk_store.por_tree().root(), manifest.por_root);
    assert_eq!(
        compute_chunk_plan_digest_sha3(&plan.chunks),
        manifest.chunk_digest_sha3_256
    );
    let retained = CarVerifier::verify_canonical_car_with_plan_retained(plan, car)
        .expect("retain verified CAR");
    let temp = tempfile::tempdir().expect("provider tempdir");
    let root = temp.path().canonicalize().expect("canonical provider root");
    let storage = config(&root);
    let id = {
        let node = NodeHandle::try_new(storage.clone()).expect("open provider");
        let mut reader = retained.payload_reader();
        let id = node
            .ingest_manifest(manifest, plan, &mut reader)
            .expect("ingest verified CAR payload");
        assert_eq!(io::copy(&mut reader, &mut io::sink()).expect("EOF"), 0);
        assert_readback(&node, &id, manifest, plan, payload);
        let mut duplicate = retained.payload_reader();
        assert!(
            node.ingest_manifest(manifest, plan, &mut duplicate)
                .is_err()
        );
        assert_eq!(node.storage().expect("backend").manifest_count(), 1);
        id
    };
    let reopened = NodeHandle::try_new(storage).expect("reopen persisted provider");
    assert_readback(&reopened, &id, manifest, plan, payload);
}

#[test]
fn canonical_site_car_ingests_and_survives_provider_restart() {
    let (plan, payload, manifest, car) = fixture();
    assert!(plan.chunks.len() > 1, "exercise multiple storage chunks");
    roundtrip(&plan, &payload, &manifest, &car);
}

#[test]
fn rejected_site_inputs_leave_no_manifest_and_release_ingest_reservations() {
    let (plan, payload, manifest, car) = fixture();
    let mut changed_car = car.clone();
    let at = changed_car.len() / 2;
    changed_car[at] ^= 1;
    assert!(CarVerifier::verify_full_car_with_plan(&manifest, &plan, &changed_car).is_err());
    assert!(
        CarVerifier::verify_full_car_with_plan(&manifest, &plan, &car[..car.len() - 1]).is_err()
    );
    changed_car = car.clone();
    changed_car.push(0);
    assert!(CarVerifier::verify_full_car_with_plan(&manifest, &plan, &changed_car).is_err());
    let temp = tempfile::tempdir().expect("provider tempdir");
    let node = NodeHandle::try_new(config(&temp.path().canonicalize().expect("canonical root")))
        .expect("provider");
    let mut variants = Vec::new();
    let mut wrong = manifest.clone();
    wrong.por_root[0] ^= 1;
    variants.push(wrong);
    let mut wrong = manifest.clone();
    wrong.chunk_digest_sha3_256[0] ^= 1;
    variants.push(wrong);
    let mut wrong = manifest.clone();
    wrong.car_digest[0] ^= 1;
    variants.push(wrong);
    let mut wrong = manifest.clone();
    wrong.root_cid[4] ^= 1;
    variants.push(wrong);
    for wrong in variants {
        assert!(
            node.ingest_manifest(&wrong, &plan, &mut payload.as_slice())
                .is_err()
        );
        assert_eq!(node.storage().expect("backend").manifest_count(), 0);
    }
    let mut changed_payload = payload.clone();
    changed_payload[0] ^= 1;
    assert!(
        node.ingest_manifest(&manifest, &plan, &mut changed_payload.as_slice())
            .is_err()
    );
    assert!(
        node.ingest_manifest(&manifest, &plan, &mut &payload[..payload.len() - 1])
            .is_err()
    );
    assert_eq!(node.storage().expect("backend").manifest_count(), 0);
    let id = node
        .ingest_manifest(&manifest, &plan, &mut payload.as_slice())
        .expect("valid retry after all rejected inputs");
    assert_readback(&node, &id, &manifest, &plan, &payload);
}

#[test]
fn persisted_site_corruption_is_rejected_on_read_and_restart() {
    let (plan, payload, manifest, _) = fixture();
    let temp = tempfile::tempdir().expect("provider tempdir");
    let storage = config(&temp.path().canonicalize().expect("canonical root"));
    let node = NodeHandle::try_new(storage.clone()).expect("provider");
    let id = node
        .ingest_manifest(&manifest, &plan, &mut payload.as_slice())
        .expect("admit valid site");
    let backend = node.storage().expect("backend");
    let stored = backend.manifest(&id).expect("manifest");
    let chunk = stored.chunk(0).expect("stored first chunk");
    let mut bytes = fs::read(&chunk.path).expect("read persisted chunk");
    bytes[0] ^= 1;
    fs::write(&chunk.path, bytes).expect("inject same-length disk corruption");
    assert!(node.read_payload_range(&id, 0, 1).is_err());
    assert!(node.read_chunk_by_digest(&id, &chunk.digest).is_err());
    drop(stored);
    drop(backend);
    drop(node);
    assert!(
        NodeHandle::try_new(storage).is_err(),
        "restart cannot admit a corrupted persisted site"
    );
}

#[test]
#[ignore = "requires explicit local SORA CARS dist and native package paths"]
fn sora_cars_release_car_roundtrips_real_provider_storage() {
    let dist =
        std::env::var_os("SORAFS_QUALIFICATION_DIST").expect("set SORAFS_QUALIFICATION_DIST");
    let package =
        std::env::var_os("SORAFS_QUALIFICATION_PACKAGE").expect("set SORAFS_QUALIFICATION_PACKAGE");
    let package = Path::new(&package);
    let manifest_bytes =
        fs::read(package.join("sora-cars.manifest.to")).expect("read release manifest");
    assert!(manifest_bytes.len() <= sorafs_manifest::MAX_MANIFEST_ENCODED_BYTES);
    let manifest =
        decode_manifest_v1_canonical(&manifest_bytes).expect("canonical release manifest");
    let car_path = package.join("sora-cars.car");
    assert!(fs::metadata(&car_path).expect("CAR metadata").len() <= 64 * 1024 * 1024);
    let car = fs::read(car_path).expect("read release CAR");
    let descriptor = sorafs_manifest::chunker_registry::lookup(manifest.chunking.profile_id)
        .expect("registered chunker");
    let (plan, payload) =
        CarBuildPlan::from_directory_with_profile(Path::new(&dist), descriptor.profile)
            .expect("release directory plan");
    roundtrip(&plan, &payload, &manifest, &car);
    eprintln!(
        "native provider storage qualified: files={}, chunks={}, payload_bytes={}, car_bytes={}, root_cid={}; consensus and remote availability NOT qualified",
        plan.files.len(),
        plan.chunks.len(),
        payload.len(),
        car.len(),
        hex::encode(&manifest.root_cid)
    );
}
