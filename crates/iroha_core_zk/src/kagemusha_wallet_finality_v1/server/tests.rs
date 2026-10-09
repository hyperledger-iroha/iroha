//! Custody and small-circuit cache tests do not qualify production financial proofs.

mod cache_keys;

use std::{fs, io::Write as _, path::PathBuf};

use iroha_fs::PrivateDirectory;
use iroha_kagemusha_proof::finality::continuity::{
    checkpoint::{ProofCheckpointStore, ProofIdentity},
    tree::SourceIdentity,
};

use super::*;

fn root() -> (tempfile::TempDir, PathBuf) {
    let parent = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/qualification/server-finality-tests");
    fs::create_dir_all(&parent).unwrap();
    let temp = tempfile::Builder::new()
        .prefix("custody-")
        .tempdir_in(&parent)
        .unwrap();
    let path = temp.path().canonicalize().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    (temp, path)
}
fn name() -> String {
    format!("receipt-{}", "12".repeat(32))
}
fn limits() -> ServerFinalityLimitsV1 {
    ServerFinalityLimitsV1 {
        maximum_key_bytes: 1 << 20,
        maximum_resident_proving_key_bytes: 512 << 20,
        maximum_original_bytes: 1 << 30,
        maximum_artifacts: 4096,
        msm_bytes: 64 << 20,
        maximum_journal_entries: 32,
        maximum_journal_bytes: 8 << 20,
    }
}

fn open_journal(
    path: &std::path::Path,
    selection: &[u8],
    entries: usize,
    bytes: u64,
) -> std::io::Result<storage::Journal> {
    storage::Journal::acquire(path, selection, entries, bytes, custody::Mode::Open)
}
fn initialize_journal(
    path: &std::path::Path,
    selection: &[u8],
    entries: usize,
    bytes: u64,
) -> std::io::Result<storage::Journal> {
    storage::Journal::acquire(path, selection, entries, bytes, custody::Mode::Initialize)
}

#[test]
fn immutable_data_reopens_exactly_and_refuses_changed_installation_or_payload() {
    let (_temp, path) = root();
    let mut journal = initialize_journal(&path, b"selected signed originals", 32, 1 << 20).unwrap();
    journal.put(&name(), b"retained proof DATA").unwrap();
    journal.put(&name(), b"retained proof DATA").unwrap();
    assert!(journal.put(&name(), b"different proof DATA").is_err());
    drop(journal);
    assert!(open_journal(&path, b"another signed selection", 32, 1 << 20).is_err());
    let reopened = open_journal(&path, b"selected signed originals", 32, 1 << 20).unwrap();
    assert_eq!(
        reopened.read(&name(), 32).unwrap().unwrap(),
        b"retained proof DATA"
    );
}

#[test]
fn exclusive_journal_owner_and_unknown_or_writable_originals_fail_closed() {
    let (_temp, path) = root();
    let mut journal = initialize_journal(&path, b"selection", 32, 1 << 20).unwrap();
    assert!(open_journal(&path, b"selection", 32, 1 << 20).is_err());
    journal.put(&name(), b"original").unwrap();
    drop(journal);
    let directory = PrivateDirectory::open_exact(&path).unwrap();
    let mut unexpected = directory.create_retained_private("unexpected", 4).unwrap();
    unexpected.write_all(b"kept").unwrap();
    assert!(open_journal(&path, b"selection", 32, 1 << 20).is_err());
    assert_eq!(fs::read(path.join("unexpected")).unwrap(), b"kept");
}

#[test]
fn interrupted_partial_is_preserved_counted_and_does_not_replace_exact_destination() {
    let (_temp, path) = root();
    let mut journal = initialize_journal(&path, b"selection", 4, 1 << 20).unwrap();
    let directory = PrivateDirectory::open_exact(&path).unwrap();
    let pending_name = format!("pending-{}", "ab".repeat(16));
    let mut pending = directory
        .create_retained_private(&pending_name, 32)
        .unwrap();
    pending.write_all(b"interrupted").unwrap();
    drop(pending);
    journal.put(&name(), b"complete").unwrap();
    assert!(
        journal
            .put(&format!("receipt-{}", "34".repeat(32)), b"over capacity")
            .is_err()
    );
    drop(journal);
    let reopened = open_journal(&path, b"selection", 4, 1 << 20).unwrap();
    assert_eq!(reopened.read(&name(), 32).unwrap().unwrap(), b"complete");
    assert_eq!(fs::read(path.join(pending_name)).unwrap(), b"interrupted");
}

#[test]
fn malformed_checkpoint_is_never_reported_absent_or_as_proof_authority() {
    let (_temp, path) = root();
    let mut journal = initialize_journal(&path, b"selection", 32, 1 << 20).unwrap();
    let id = ProofIdentity {
        source: SourceIdentity {
            descriptor: [1; 32],
            key: [2; 32],
        },
        endpoints: [[0; 32]; 6],
    };
    assert!(journal.load(&id).unwrap().is_none());
    // The exact path is derived from the source and every endpoint, not a supplied path.
    let mut hash = Sha256::new();
    hash.update(b"kagemusha-server-finality-checkpoint-v1");
    hash.update(id.source.descriptor);
    hash.update(id.source.key);
    for endpoint in id.endpoints {
        hash.update(endpoint);
    }
    let filename = format!("node-{}", hex::encode(hash.finalize()));
    journal
        .put(&filename, b"not a canonical proof checkpoint")
        .unwrap();
    assert!(journal.load(&id).is_err());
    assert_eq!(
        journal.read(&filename, 128).unwrap().unwrap(),
        b"not a canonical proof checkpoint"
    );
}

#[test]
fn finite_limits_refuse_unbounded_resources_before_any_source_import() {
    for changed in [
        ServerFinalityLimitsV1 {
            maximum_key_bytes: 0,
            ..limits()
        },
        ServerFinalityLimitsV1 {
            maximum_original_bytes: usize::MAX,
            ..limits()
        },
        ServerFinalityLimitsV1 {
            maximum_artifacts: 0,
            ..limits()
        },
        ServerFinalityLimitsV1 {
            msm_bytes: 0,
            ..limits()
        },
        ServerFinalityLimitsV1 {
            maximum_journal_entries: 2,
            ..limits()
        },
        ServerFinalityLimitsV1 {
            maximum_journal_bytes: u64::MAX,
            ..limits()
        },
    ] {
        assert!(changed.imports().is_err());
    }
    assert!(limits().imports().is_ok());
}

#[test]
fn untrusted_pack_never_creates_server_custody_or_adopts_a_genesis() {
    use iroha_data_model::sumeragi_finality::test_fixtures::NativeFinalityFixture;
    let (_temp, path) = root();
    let fixture = NativeFinalityFixture::new_with_explicit_parameters();
    let directory = PrivateDirectory::open_exact(&path).unwrap();
    let verifiers = directory.create_child("verifiers").unwrap();
    let cache = directory.create_child("cache").unwrap();
    let journal = directory.create_child("journal").unwrap();
    assert!(
        ServerFinalityV1::open(
            &fixture.verifier(),
            InstallationV1 {
                scheme_id: [1; 32],
                manifest_digest: [2; 32]
            },
            b"unsigned pack",
            b"unsigned catalog",
            ServerFinalityStorageV1 {
                verifier_originals: &path.join("verifiers"),
                proving_cache: &path.join("cache"),
                journal: &path.join("journal")
            },
            limits(),
            ServerFinalityCancellationV1::default()
        )
        .is_err()
    );
    for directory in [verifiers, cache, journal] {
        assert!(directory.entries(1).unwrap().is_empty());
    }
}

#[test]
fn existing_only_open_never_adopts_missing_selection_or_lock() {
    let (_temp, path) = root();
    assert!(open_journal(&path, b"selected", 32, 1 << 20).is_err());
    assert_eq!(fs::read_dir(&path).unwrap().count(), 0);
    let journal = initialize_journal(&path, b"selected", 32, 1 << 20).unwrap();
    assert!(initialize_journal(&path, b"selected", 32, 1 << 20).is_err());
    fs::remove_file(path.join("selection")).unwrap();
    assert!(journal.read(&name(), 32).is_err());
    drop(journal);
    assert!(open_journal(&path, b"selected", 32, 1 << 20).is_err());
    assert!(initialize_journal(&path, b"selected", 32, 1 << 20).is_err());
    assert!(!path.join("selection").exists());
}

fn cache_record(bytes: &[u8]) -> iroha_kagemusha_proof::finality::catalog::ArtifactRecord {
    // Storage DATA only: these are never offered to any recipe qualifier or proof importer.
    iroha_kagemusha_proof::finality::catalog::ArtifactRecord {
        name: vec![1],
        lengths: [1, 1, bytes.len() as u64],
        sha256: [[1; 32], [2; 32], Sha256::digest(bytes).into()],
    }
}

#[test]
fn bounded_cache_evicts_only_selected_regenerable_data_and_reopens() {
    let (_temp, path) = root();
    let one = cache_record(b"firstkey");
    let two = cache_record(b"otherkey");
    let records = [one.clone(), two.clone()];
    let mut cache = cache::Cache::acquire(
        &path,
        b"cache selected",
        &records,
        8,
        8,
        custody::Mode::Initialize,
    )
    .unwrap();
    assert!(cache.get(&one, None).unwrap().is_none());
    cache.put(&one, b"firstkey", None).unwrap();
    cache.put(&two, b"otherkey", None).unwrap();
    assert!(cache.get(&one, None).unwrap().is_none());
    assert_eq!(cache.get(&two, None).unwrap().unwrap(), b"otherkey");
    drop(cache);
    let mut cache = cache::Cache::acquire(
        &path,
        b"cache selected",
        &records,
        8,
        8,
        custody::Mode::Open,
    )
    .unwrap();
    cache.put(&one, b"firstkey", None).unwrap();
    assert_eq!(cache.get(&one, None).unwrap().unwrap(), b"firstkey");
    assert!(cache.get(&two, None).unwrap().is_none());
    let directory = PrivateDirectory::open_exact(&path).unwrap();
    directory
        .create_retained_private("foreign", 8)
        .unwrap()
        .write_all(b"preserve")
        .unwrap();
    assert!(cache.put(&two, b"otherkey", None).is_err());
    assert_eq!(fs::read(path.join("foreign")).unwrap(), b"preserve");
    assert!(path.join(hex::encode(one.sha256[2])).exists());
}

#[test]
fn cache_cancel_is_typed_and_missing_selection_never_regenerates() {
    let (_temp, path) = root();
    let record = cache_record(b"firstkey");
    let mut cache = cache::Cache::acquire(
        &path,
        b"cache",
        &[record.clone()],
        8,
        8,
        custody::Mode::Initialize,
    )
    .unwrap();
    let token = iroha_pasta::CancellationToken::new();
    token.cancel();
    assert!(
        cache
            .put(&record, b"firstkey", Some(&token))
            .unwrap_err()
            .is_cancelled()
    );
    cache.put(&record, b"firstkey", None).unwrap();
    drop(cache);
    fs::remove_file(path.join("selection")).unwrap();
    assert!(
        cache::Cache::acquire(
            &path,
            b"cache",
            &[record.clone()],
            8,
            8,
            custody::Mode::Open
        )
        .is_err()
    );
    assert!(!path.join("selection").exists());
    assert_eq!(
        fs::read(path.join(hex::encode(record.sha256[2]))).unwrap(),
        b"firstkey"
    );
}

#[test]
fn changed_cache_bytes_or_role_selection_are_refused_without_replacement() {
    let (_temp, path) = root();
    let record = cache_record(b"firstkey");
    let mut cache = cache::Cache::acquire(
        &path,
        b"cache",
        &[record.clone()],
        8,
        8,
        custody::Mode::Initialize,
    )
    .unwrap();
    cache.put(&record, b"firstkey", None).unwrap();
    drop(cache);
    assert!(open_journal(&path, b"journal", 32, 1 << 20).is_err());
    let target = path.join(hex::encode(record.sha256[2]));
    fs::remove_file(&target).unwrap();
    let directory = PrivateDirectory::open_exact(&path).unwrap();
    let mut writer = directory
        .create_retained_private(hex::encode(record.sha256[2]), 8)
        .unwrap();
    writer.write_all(b"badbytes").unwrap();
    writer.seal_read_only().unwrap();
    let mut cache = cache::Cache::acquire(
        &path,
        b"cache",
        &[record.clone()],
        8,
        8,
        custody::Mode::Open,
    )
    .unwrap();
    assert!(cache.get(&record, None).is_err());
    assert!(cache.put(&record, b"firstkey", None).is_err());
    assert_eq!(fs::read(target).unwrap(), b"badbytes");
}

#[test]
fn archive_inventory_requires_all_selected_records_and_every_original_role() {
    use crate::kagemusha_wallet_artifacts_v1::producer_inventory::{BlobV1, DirectoryOriginalsV1};
    use iroha_kagemusha_proof::finality::{
        catalog::{ArtifactSink, DirectoryCatalog},
        continuity::tree::OriginalBytes,
        native::{ArtifactId, ArtifactSource, NodeId},
    };
    let (_temp, path) = root();
    let directory = PrivateDirectory::open_exact(&path).unwrap();
    let archive_directory = directory.create_child("archive").unwrap();
    let mut archive =
        DirectoryOriginalsV1::open_existing(archive_directory.path(), 1 << 20).unwrap();
    let imports = limits().imports().unwrap();
    let mut compiler =
        DirectoryCatalog::create(path.join("metadata-only-compiler"), imports).unwrap();
    let ids = [
        ArtifactId::Source(NodeId::Genesis),
        ArtifactId::Source(NodeId::Append),
    ];
    for (index, id) in ids.iter().enumerate() {
        // Deliberately opaque DATA. Loading these bytes establishes no proof authority.
        let bytes = OriginalBytes {
            descriptor: vec![index as u8 + 1; 5],
            verifying_key: vec![index as u8 + 3; 7],
            proving_key: vec![index as u8 + 5; 11],
        };
        compiler.store(id, &bytes).unwrap();
        for original in [&bytes.descriptor, &bytes.verifying_key, &bytes.proving_key] {
            archive
                .store_original(BlobV1::of(original), original)
                .unwrap();
        }
    }
    let records: Vec<ArtifactRecord> =
        norito::decode_canonical(&compiler.inventory().unwrap()).unwrap();
    let mut source =
        artifacts::ArchiveOriginals::from_records(&records, archive_directory.path(), imports)
            .unwrap();
    assert!(source.require_complete().is_err());
    source.load(&ids[0]).unwrap();
    assert!(
        source.require_complete().is_err(),
        "a well-formed unused record must fail completeness"
    );
    source.load(&ids[1]).unwrap();
    source.require_complete().unwrap();
    for role in 0..3 {
        let mut changed = records.clone();
        changed[0].lengths[role] = 0;
        assert!(
            artifacts::ArchiveOriginals::from_records(&changed, archive_directory.path(), imports)
                .is_err()
        );
        changed[0].lengths[role] = records[0].lengths[role];
        changed[0].sha256[role] = [0; 32];
        assert!(
            artifacts::ArchiveOriginals::from_records(&changed, archive_directory.path(), imports)
                .is_err()
        );
    }
    let mut duplicate = records.clone();
    duplicate.push(records[0].clone());
    assert!(
        artifacts::ArchiveOriginals::from_records(&duplicate, archive_directory.path(), imports)
            .is_err()
    );
    assert!(
        artifacts::ArchiveOriginals::from_records(
            &records,
            archive_directory.path(),
            ImportLimits {
                maximum_artifacts: 1,
                ..imports
            }
        )
        .is_err()
    );
    assert!(
        artifacts::ArchiveOriginals::from_records(
            &records,
            archive_directory.path(),
            ImportLimits {
                maximum_original_bytes: 1,
                ..imports
            }
        )
        .is_err()
    );
    let mut unordered = records.clone();
    unordered.reverse();
    assert!(
        artifacts::ArchiveOriginals::from_records(&unordered, archive_directory.path(), imports)
            .is_err()
    );
    let mut malformed = records.clone();
    malformed[0].name.clear();
    assert!(
        artifacts::ArchiveOriginals::from_records(&malformed, archive_directory.path(), imports)
            .is_err()
    );
    let mut changed = records.clone();
    changed[0].sha256[2] = [0x7f; 32];
    let mut missing =
        artifacts::ArchiveOriginals::from_records(&changed, archive_directory.path(), imports)
            .unwrap();
    assert!(missing.load(&ids[0]).is_err() || missing.load(&ids[1]).is_err());
    assert!(missing.require_complete().is_err());
    assert!(matches!(
        missing.take_failure(),
        Some(ServerFinalityErrorV1::Storage(error)) if error.kind() == std::io::ErrorKind::NotFound
    ));
    assert!(missing.take_failure().is_none());
}

#[test]
fn offline_archive_refuses_changed_bytes_without_repairing_or_counting_them() {
    use crate::kagemusha_wallet_artifacts_v1::producer_inventory::{BlobV1, DirectoryOriginalsV1};
    use iroha_kagemusha_proof::finality::{
        catalog::{ArtifactSink, DirectoryCatalog},
        continuity::tree::OriginalBytes,
        native::{ArtifactId, ArtifactSource, NodeId},
    };
    for role in 0..3 {
        let (_temp, path) = root();
        let directory = PrivateDirectory::open_exact(&path).unwrap();
        let archive_directory = directory.create_child("archive").unwrap();
        let mut archive =
            DirectoryOriginalsV1::open_existing(archive_directory.path(), 1 << 20).unwrap();
        let imports = limits().imports().unwrap();
        let mut compiler = DirectoryCatalog::create(path.join("compiler"), imports).unwrap();
        let id = ArtifactId::Source(NodeId::Genesis);
        let bytes = OriginalBytes {
            descriptor: vec![1; 5],
            verifying_key: vec![3; 7],
            proving_key: vec![5; 11],
        };
        compiler.store(&id, &bytes).unwrap();
        for original in [&bytes.descriptor, &bytes.verifying_key, &bytes.proving_key] {
            archive
                .store_original(BlobV1::of(original), original)
                .unwrap();
        }
        let records: Vec<ArtifactRecord> =
            norito::decode_canonical(&compiler.inventory().unwrap()).unwrap();
        let record = &records[0];
        let name = hex::encode(record.sha256[role]);
        let target = archive_directory.path().join(&name);
        fs::remove_file(&target).unwrap();
        let changed = vec![0xff; record.lengths[role] as usize];
        let mut writer = archive_directory
            .create_retained_private(&name, changed.len())
            .unwrap();
        writer.write_all(&changed).unwrap();
        writer.seal_read_only().unwrap();
        let mut source =
            artifacts::ArchiveOriginals::from_records(&records, archive_directory.path(), imports)
                .unwrap();
        assert!(source.load(&id).is_err());
        assert!(source.require_complete().is_err());
        assert!(matches!(
            source.take_failure(),
            Some(ServerFinalityErrorV1::Binding)
        ));
        assert_eq!(fs::read(target).unwrap(), changed);
    }
}

#[test]
fn offline_archive_qualification_cannot_replace_missing_originals_with_metadata() {
    use iroha_data_model::sumeragi_finality::test_fixtures::NativeFinalityFixture;
    let (_temp, path) = root();
    let fixture = NativeFinalityFixture::new_with_explicit_parameters();
    assert!(
        qualify_server_archive(&fixture.verifier(), &[], &path, limits().imports().unwrap())
            .is_err()
    );
    assert_eq!(fs::read_dir(path).unwrap().count(), 0);
}
