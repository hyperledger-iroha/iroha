//! DATA custody tests do not qualify real production financial proof generation.

use std::{fs, io::Write as _, path::PathBuf};

use iroha_fs::PrivateDirectory;
use iroha_kagemusha_proof::finality::continuity::{
    checkpoint::{ProofCheckpointStore, ProofIdentity},
    tree::SourceIdentity,
};

use super::*;

fn root() -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).unwrap();
    }
    let path = temp.path().canonicalize().unwrap();
    (temp, path)
}
fn name() -> String {
    format!("receipt-{}", "12".repeat(32))
}
fn limits() -> ServerFinalityLimitsV1 {
    ServerFinalityLimitsV1 {
        maximum_key_bytes: 1 << 20,
        maximum_original_bytes: 1 << 30,
        maximum_artifacts: 4096,
        msm_bytes: 64 << 20,
        maximum_journal_entries: 32,
        maximum_journal_bytes: 8 << 20,
    }
}

#[test]
fn immutable_data_reopens_exactly_and_refuses_changed_installation_or_payload() {
    let (_temp, path) = root();
    let mut journal =
        storage::Journal::open(&path, b"selected signed originals", 32, 1 << 20).unwrap();
    journal.put(&name(), b"retained proof DATA").unwrap();
    journal.put(&name(), b"retained proof DATA").unwrap();
    assert!(journal.put(&name(), b"different proof DATA").is_err());
    drop(journal);
    assert!(storage::Journal::open(&path, b"another signed selection", 32, 1 << 20).is_err());
    let reopened =
        storage::Journal::open(&path, b"selected signed originals", 32, 1 << 20).unwrap();
    assert_eq!(
        reopened.read(&name(), 32).unwrap().unwrap(),
        b"retained proof DATA"
    );
}

#[test]
fn exclusive_journal_owner_and_unknown_or_writable_originals_fail_closed() {
    let (_temp, path) = root();
    let mut journal = storage::Journal::open(&path, b"selection", 32, 1 << 20).unwrap();
    assert!(storage::Journal::open(&path, b"selection", 32, 1 << 20).is_err());
    journal.put(&name(), b"original").unwrap();
    drop(journal);
    let directory = PrivateDirectory::open_exact(&path).unwrap();
    let mut unexpected = directory.create_retained_private("unexpected", 4).unwrap();
    unexpected.write_all(b"kept").unwrap();
    assert!(storage::Journal::open(&path, b"selection", 32, 1 << 20).is_err());
    assert_eq!(fs::read(path.join("unexpected")).unwrap(), b"kept");
}

#[test]
fn interrupted_partial_is_preserved_counted_and_does_not_replace_exact_destination() {
    let (_temp, path) = root();
    let mut journal = storage::Journal::open(&path, b"selection", 4, 1 << 20).unwrap();
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
    let reopened = storage::Journal::open(&path, b"selection", 4, 1 << 20).unwrap();
    assert_eq!(reopened.read(&name(), 32).unwrap().unwrap(), b"complete");
    assert_eq!(fs::read(path.join(pending_name)).unwrap(), b"interrupted");
}

#[test]
fn malformed_checkpoint_is_never_reported_absent_or_as_proof_authority() {
    let (_temp, path) = root();
    let mut journal = storage::Journal::open(&path, b"selection", 32, 1 << 20).unwrap();
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
    assert!(
        ServerFinalityV1::open(
            &fixture.verifier(),
            InstallationV1 {
                scheme_id: [1; 32],
                manifest_digest: [2; 32]
            },
            b"unsigned pack",
            b"unsigned catalog",
            &path,
            &path,
            limits(),
            ServerFinalityCancellationV1::default()
        )
        .is_err()
    );
    assert_eq!(fs::read_dir(path).unwrap().count(), 0);
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
        artifacts::Originals::from_records(&records, archive_directory.path(), imports).unwrap();
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
            artifacts::Originals::from_records(&changed, archive_directory.path(), imports)
                .is_err()
        );
        changed[0].lengths[role] = records[0].lengths[role];
        changed[0].sha256[role] = [0; 32];
        assert!(
            artifacts::Originals::from_records(&changed, archive_directory.path(), imports)
                .is_err()
        );
    }
    let mut duplicate = records.clone();
    duplicate.push(records[0].clone());
    assert!(
        artifacts::Originals::from_records(&duplicate, archive_directory.path(), imports).is_err()
    );
    assert!(
        artifacts::Originals::from_records(
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
        artifacts::Originals::from_records(
            &records,
            archive_directory.path(),
            ImportLimits {
                maximum_original_bytes: 1,
                ..imports
            }
        )
        .is_err()
    );
    let mut changed = records.clone();
    changed[0].sha256[2] = [0x7f; 32];
    let mut missing =
        artifacts::Originals::from_records(&changed, archive_directory.path(), imports).unwrap();
    assert!(missing.load(&ids[0]).is_err() || missing.load(&ids[1]).is_err());
    assert!(missing.require_complete().is_err());
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
