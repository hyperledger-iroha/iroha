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
