//! Managed generation, custody, lifecycle selection and resource regression tests.

use super::*;
use iroha_fs::{PrivateDirectory, PublishMode};
use std::{
    net::{Ipv4Addr, TcpListener},
    path::Path,
};

fn fixture(root: &Path, name: &str) -> (ManagedStore, PrivateDirectory, PreparedLocalnet) {
    let store = ManagedStore::open(root).unwrap();
    let networks = PrivateDirectory::open(root.join("networks")).unwrap();
    let directory = networks.create_child(name).unwrap();
    let bundle = directory.create_child("generation").unwrap();
    bundle
        .write_atomic("client.toml", b"private config", PublishMode::CreateNew)
        .unwrap();
    let mut peers = Vec::new();
    for index in 0..4 {
        let config_name = format!("peer{index}.toml");
        bundle
            .write_atomic(&config_name, b"private node", PublishMode::CreateNew)
            .unwrap();
        peers.push(ManagedPeer {
            config_path: bundle.path().join(config_name),
            torii_url: format!("http://127.0.0.1:{}/", 19080 + index),
            log_name: format!("peer{index}.log"),
        });
    }
    let prepared = PreparedLocalnet {
        context: ManagedContext {
            name: name.into(),
            chain_id: "local".into(),
            network_id: "public-network-id".into(),
            account_id: "public-account-id".into(),
            dataspace_id: 0,
            dataspace_alias: "universal".into(),
            torii_url: peers[0].torii_url.clone(),
            client_config: bundle.path().join("client.toml"),
        },
        peers,
    };
    directory
        .write_atomic(
            "fixture-executable",
            b"retained binary",
            PublishMode::CreateNew,
        )
        .unwrap();
    let pin = store::pin_binary(&directory.path().join("fixture-executable")).unwrap();
    let retained = RetainedLocalnet {
        prepared: prepared.clone(),
        launcher: pin.clone(),
        daemon: pin,
        startup_timeout_ms: 30000,
    };
    directory
        .write_atomic(
            MANIFEST,
            &encode(&retained).unwrap(),
            PublishMode::CreateNew,
        )
        .unwrap();
    (store, directory, prepared)
}

#[test]
fn names_cannot_escape_the_context_root() {
    let _resources = super::native_test_guard();
    for name in [
        "",
        ".",
        "..",
        "../other",
        "foo/bar",
        "foo\\bar",
        "/tmp",
        "-bad",
        "local net",
        "日本",
    ] {
        assert!(validate_name(name).is_err(), "{name:?}");
    }
    for name in ["local", "Acme_2", "private-test"] {
        validate_name(name).unwrap();
    }
    assert!(validate_name(&"x".repeat(49)).is_err());
}

#[test]
fn port_reservations_are_live_disjoint_and_released_on_drop() {
    let _resources = super::native_test_guard();
    let ports = LocalnetPorts::reserve().unwrap();
    assert_eq!(ports.reserved_count(), 8);
    let bases = [ports.base_api, ports.base_p2p];
    assert!((i32::from(bases[0]) - i32::from(bases[1])).abs() >= 4);
    for base in bases {
        for offset in 0..4 {
            assert!(TcpListener::bind((Ipv4Addr::LOCALHOST, base + offset)).is_err());
        }
    }
    drop(ports);
    for base in bases {
        for offset in 0..4 {
            TcpListener::bind((Ipv4Addr::LOCALHOST, base + offset)).unwrap();
        }
    }
}

#[test]
fn stopped_context_selection_retains_identity_and_never_exposes_keys() {
    let _resources = super::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let (store, _, prepared) = fixture(&temporary.path().join("managed"), "local");
    let selected = store.select("local").unwrap();
    assert_eq!(selected, prepared.context);
    assert_eq!(store.prepared("local").unwrap(), prepared);
    assert_eq!(store.context(None).unwrap(), selected);
    assert_eq!(store.contexts().unwrap(), [selected.clone()]);
    let status = store.status("local").unwrap();
    assert_eq!(status.phase, ManagedPhase::Stopped);
    assert_eq!(store.down("local").unwrap(), status);
    let receipt = String::from_utf8(encode(&status).unwrap()).unwrap();
    assert!(!receipt.contains("private config"));
    assert!(!receipt.contains("private_key"));
    assert!(!receipt.contains("token"));
    let restored = ManagedStore::open(store.root()).unwrap();
    assert_eq!(restored.context(None).unwrap(), selected);
}

#[test]
fn unreadable_worker_with_held_lock_is_not_reported_stopped_or_reset() {
    let _resources = super::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let (store, directory, _) = fixture(&temporary.path().join("managed"), "local");
    let _owned = store::acquire(&directory, "runtime.lock", "local").unwrap();
    assert!(matches!(store.status("local"), Err(Error::Busy(_))));
    assert!(matches!(store.reset("local"), Err(Error::Busy(_))));
    assert!(directory.path().join(MANIFEST).exists());
}

#[test]
fn stopped_controller_cannot_leave_a_saved_ready_status() {
    let _resources = super::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let (store, directory, prepared) = fixture(&temporary.path().join("managed"), "local");
    directory
        .write_atomic(
            STATUS,
            &encode(&ManagedStatus {
                context: prepared.context,
                phase: ManagedPhase::Ready,
                running_peers: 4,
                failure: None,
            })
            .unwrap(),
            PublishMode::CreateNew,
        )
        .unwrap();
    let status = store.status("local").unwrap();
    assert_eq!(status.phase, ManagedPhase::Failed);
    assert_eq!(status.running_peers, 0);
    assert!(status.failure.unwrap().contains("unexpectedly"));
}

#[test]
fn live_foreground_start_is_distinguished_from_a_crashed_handoff() {
    let _resources = super::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let (store, directory, prepared) = fixture(&temporary.path().join("managed"), "local");
    directory
        .write_atomic(
            STATUS,
            &encode(&ManagedStatus {
                context: prepared.context,
                phase: ManagedPhase::Starting,
                running_peers: 0,
                failure: None,
            })
            .unwrap(),
            PublishMode::CreateNew,
        )
        .unwrap();
    let operation = store::acquire(&directory, "operation.lock", "local").unwrap();
    assert_eq!(store.status("local").unwrap().phase, ManagedPhase::Starting);
    drop(operation);
    assert_eq!(store.status("local").unwrap().phase, ManagedPhase::Failed);
}

#[test]
fn reset_deletes_only_a_stopped_named_context_and_preserves_selection_failure() {
    let _resources = super::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("managed");
    let (store, directory, _) = fixture(&root, "local");
    let (_, other, _) = fixture(&root, "other");
    store.select("local").unwrap();
    store.reset("local").unwrap();
    assert!(!directory.path().join(MANIFEST).exists());
    assert!(directory.path().join("operation.lock").exists());
    assert!(other.path().join(MANIFEST).exists());
    assert!(
        matches!(store.context(None), Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound)
    );
    assert_eq!(store.contexts().unwrap().len(), 1);
}

#[test]
fn missing_selection_is_distinct_from_a_lost_retained_generation() {
    let _resources = super::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let store = ManagedStore::open(&temporary.path().join("managed")).unwrap();
    assert!(matches!(store.context(None), Err(Error::NoSelection)));
    assert!(
        matches!(store.context(Some("unknown")), Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound)
    );
}

#[test]
fn prepared_bundle_rejects_remote_endpoints_missing_peers_and_path_escape() {
    let _resources = super::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let (_, directory, prepared) = fixture(&temporary.path().join("managed"), "local");
    store::validate_prepared("local", directory.path(), &prepared).unwrap();
    let mut changed = prepared.clone();
    changed.peers.pop();
    assert!(store::validate_prepared("local", directory.path(), &changed).is_err());
    let mut changed = prepared.clone();
    changed.context.dataspace_id = 2;
    assert!(store::validate_prepared("local", directory.path(), &changed).is_err());
    changed.context.dataspace_id = 0;
    changed.context.dataspace_alias = "foreign".into();
    assert!(store::validate_prepared("local", directory.path(), &changed).is_err());
    let mut changed = prepared.clone();
    changed.peers[0].torii_url = "https://taira.sora.org/".into();
    assert!(store::validate_prepared("local", directory.path(), &changed).is_err());
    let mut changed = prepared.clone();
    changed.peers[0].config_path = temporary.path().join("foreign.toml");
    assert!(store::validate_prepared("local", directory.path(), &changed).is_err());
    let mut changed = prepared.clone();
    changed.peers[0].config_path = directory.path().join("generation/../generation/peer0.toml");
    assert!(store::validate_prepared("local", directory.path(), &changed).is_err());
    let mut changed = prepared;
    changed.peers[0].log_name = "../outside.log".into();
    assert!(store::validate_prepared("local", directory.path(), &changed).is_err());
}

#[test]
fn lifecycle_phases_have_strict_stable_json_spelling() {
    let _resources = super::native_test_guard();
    for phase in [
        ManagedPhase::Stopped,
        ManagedPhase::Starting,
        ManagedPhase::Ready,
        ManagedPhase::Failed,
    ] {
        let bytes = encode(&phase).unwrap();
        assert_eq!(bytes, format!("\"{}\"", phase.as_str()).as_bytes());
        assert_eq!(decode::<ManagedPhase>(&bytes).unwrap(), phase);
    }
    assert!(decode::<ManagedPhase>(b"\"running\"").is_err());
}

#[test]
fn changed_binary_is_rejected_before_launch() {
    let _resources = super::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let binary = temporary.path().join("daemon");
    std::fs::write(&binary, b"candidate-one").unwrap();
    let pin = store::pin_binary(&binary).unwrap();
    store::verify_binary(&pin).unwrap();
    std::fs::write(binary, b"candidate-two").unwrap();
    assert!(store::verify_binary(&pin).is_err());
}

#[test]
fn logs_return_only_the_requested_bounded_tail() {
    let _resources = super::native_test_guard();
    use std::io::Write as _;
    let temporary = tempfile::tempdir().unwrap();
    let (store, directory, _) = fixture(&temporary.path().join("managed"), "local");
    directory
        .open_append("peer0.log")
        .unwrap()
        .write_all(b"line-one\nline-two\n")
        .unwrap();
    assert_eq!(store.logs("local", Some(0), 9).unwrap(), "line-two\n");
    assert!(store.logs("local", Some(4), 10).is_err());
    assert!(store.logs("local", None, 1024 * 1024 + 1).is_err());
    assert!(store.logs("local", None, 100).is_err());
    assert!(!directory.path().join("supervisor.log").exists());
}

#[cfg(unix)]
#[test]
fn symlinked_generation_config_is_rejected() {
    let _resources = super::native_test_guard();
    use std::os::unix::fs::symlink;
    let temporary = tempfile::tempdir().unwrap();
    let (_, directory, prepared) = fixture(&temporary.path().join("managed"), "local");
    let path = &prepared.peers[0].config_path;
    std::fs::remove_file(path).unwrap();
    symlink(&prepared.peers[1].config_path, path).unwrap();
    assert!(store::validate_prepared("local", directory.path(), &prepared).is_err());
}
