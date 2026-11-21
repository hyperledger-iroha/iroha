//! Controls for the real removed-source daemon's test-owned inputs and observations.

use super::*;
use std::io::Write as _;

fn event(message: &str, field: &str, value: Value) -> String {
    let mut fields = norito::json::Map::new();
    fields.insert("message".into(), Value::String(message.into()));
    fields.insert(field.into(), value);
    let mut record = norito::json::Map::new();
    record.insert("fields".into(), Value::Object(fields));
    let target = match message {
        "Successfully loaded the state from a snapshot"
        | "Sumeragi rebuilt the state from genesis and Kura"
        | "Didn't find a state snapshot; creating an empty state" => "irohad::startup_preparation",
        "Torii bound and listening" => "iroha_torii",
        _ => "irohad",
    };
    record.insert("target".into(), target.into());
    format!(
        "{}\n",
        norito::json::to_json(&Value::Object(record)).unwrap()
    )
}

fn startup(floor: u64, tip: u64) -> Vec<String> {
    vec![
        event(
            "Successfully loaded the state from a snapshot",
            "at_height",
            floor.into(),
        ),
        event(
            "Sumeragi rebuilt the state from genesis and Kura",
            "height",
            tip.into(),
        ),
        event("startup trace", "stage", "irohad.sumeragi.started".into()),
        event("Torii bound and listening", "addr", "fixture-only".into()),
    ]
}

#[test]
fn process_observation_requires_exact_floor_replay_native_start_and_listener() {
    let mut progress = Progress::default();
    for (index, line) in startup(7, 10).iter().enumerate() {
        progress.observe(line, 7, 10).unwrap();
        assert_eq!(progress.listening, index == 3);
        assert!(!progress.stopped);
    }
    progress
        .observe(
            &event("Iroha shutdown normally", "unused", Value::Null),
            7,
            10,
        )
        .unwrap();
    assert!(progress.stopped);
}

#[test]
fn newest_checkpoint_wrong_tip_fallback_and_reordered_events_never_pass() {
    for line in [
        startup(8, 10).remove(0),
        startup(7, 10).remove(1),
        startup(7, 10).remove(2),
        startup(7, 10).remove(3),
        event(
            "Didn't find a state snapshot; creating an empty state",
            "unused",
            Value::Null,
        ),
    ] {
        assert!(Progress::default().observe(&line, 7, 10).is_err());
    }
    let mut progress = Progress::default();
    progress.observe(&startup(7, 10)[0], 7, 10).unwrap();
    assert!(progress.observe(&startup(7, 11)[1], 7, 10).is_err());
    assert!(progress.observe(&startup(7, 10)[0], 7, 10).is_err());
    assert!(!progress.listening);
}

#[test]
fn partial_log_records_and_oversized_logs_do_not_complete_startup() {
    let directory = tempfile::tempdir().unwrap();
    let mut output =
        RetainedFile::create_new_private(directory.path().join("daemon.jsonl")).unwrap();
    let mut reader = RetainedFile::open_private(directory.path().join("daemon.jsonl")).unwrap();
    let lines = startup(7, 10);
    for line in &lines[..3] {
        output.file_mut().write_all(line.as_bytes()).unwrap();
    }
    let listener = lines[3].trim_end_matches('\n');
    output.file_mut().write_all(listener.as_bytes()).unwrap();
    assert!(!read_progress(&mut reader, 7, 10).unwrap().listening);
    output.file_mut().write_all(b"\n").unwrap();
    assert!(read_progress(&mut reader, 7, 10).unwrap().listening);
    output.file().set_len(LOG_LIMIT + 1).unwrap();
    assert!(read_progress(&mut reader, 7, 10).is_err());
}

#[test]
fn checkpoint_copy_refuses_truncation_growth_and_replaced_original() {
    let mut copied = Vec::new();
    copy_exact(&mut &b"original"[..], &mut copied, 8).unwrap();
    assert_eq!(copied, b"original");
    assert_eq!(
        copy_exact(&mut &b"short"[..], &mut Vec::new(), 8)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::UnexpectedEof
    );
    assert_eq!(
        copy_exact(&mut &b"longer"[..], &mut Vec::new(), 5)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::InvalidData
    );
    let directory = tempfile::tempdir().unwrap();
    let owner = PrivateDirectory::open_or_create(directory.path()).unwrap();
    owner
        .write_atomic("source", b"original", PublishMode::CreateNew)
        .unwrap();
    assert!(
        copy_original(
            &directory.path().join("source"),
            &directory.path().join("too-large"),
            7
        )
        .is_err()
    );
    assert!(!directory.path().join("too-large").exists());
    let (original, copy) = copy_original(
        &directory.path().join("source"),
        &directory.path().join("copy"),
        8,
    )
    .unwrap();
    original.recheck().unwrap();
    copy.recheck().unwrap();
    assert_eq!(
        std::fs::read(directory.path().join("copy")).unwrap(),
        b"original"
    );
    std::fs::rename(
        directory.path().join("source"),
        directory.path().join("retired-name"),
    )
    .unwrap();
    owner
        .write_atomic("source", b"original", PublishMode::CreateNew)
        .unwrap();
    assert!(
        original.recheck().is_err(),
        "same bytes at a substituted path are insufficient"
    );
}

#[test]
fn signed_parent_and_child_configurations_round_trip_without_runtime_overrides() {
    run(|| {
        let parent = CertifiedTestChain::start(parent_configuration()).unwrap();
        let home = crate::sns::dataspace_id_for_sns_alias(ALIAS).unwrap();
        let child =
            CertifiedTestChain::start(child_configuration(parent.network_id(), home)).unwrap();
        for (chain, configuration) in [
            (&parent, parent_configuration()),
            (&child, child_configuration(parent.network_id(), home)),
        ] {
            let directory = tempfile::tempdir().unwrap();
            let owner = PrivateDirectory::open_or_create(directory.path()).unwrap();
            owner
                .write_atomic(
                    "genesis.nrt",
                    &chain.genesis().encode_wire().unwrap(),
                    PublishMode::CreateNew,
                )
                .unwrap();
            let p2p = iroha_primitives::addr::SocketAddr::from(std::net::SocketAddr::from((
                [127, 0, 0, 1],
                13370,
            )));
            let torii = iroha_primitives::addr::SocketAddr::from(std::net::SocketAddr::from((
                [127, 0, 0, 1],
                18080,
            )));
            let text = configuration_text(owner.path(), chain, &configuration, p2p, torii);
            owner
                .write_atomic("node.toml", text.as_bytes(), PublishMode::CreateNew)
                .unwrap();
            validate_configuration(&owner.path().join("node.toml"), chain, &configuration);
            if let Some(nexus) = configuration.nexus {
                nexus.validate_private_root_geometry(home).unwrap();
                assert_eq!(nexus.configured_dataspace_catalog.entries()[0].id, home);
                assert!(
                    !text.contains("dataspace_id ="),
                    "full-width identity is derived from its original manifest"
                );
            }
        }
    });
}

#[test]
fn another_log_target_cannot_impersonate_native_startup() {
    let mut progress = Progress::default();
    for line in startup(7, 10) {
        let mut value = norito::json::from_str::<Value>(&line).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("target".into(), "fixture.report".into());
        progress
            .observe(&norito::json::to_json(&value).unwrap(), 7, 10)
            .unwrap();
    }
    assert_eq!(progress, Progress::default());
}

#[test]
fn dataspace_lease_requires_the_original_signed_genesis_intent() {
    run(|| {
        use iroha_data_model::isi::alias_setup::EnsureAlias;
        let configuration = parent_configuration();
        assert_eq!(
            crate::sns::active_dataspace_owner_by_alias(&configuration.world.view(), ALIAS, 1_000)
                .unwrap(),
            None
        );
        let expected_owner = AccountId::new(configuration.genesis_key.public_key().clone());
        let chain = CertifiedTestChain::start(configuration).unwrap();
        assert!(
            chain
                .genesis()
                .external_transactions()
                .flat_map(|transaction| transaction.instructions().explicit_instructions())
                .any(|instruction| instruction.as_any().downcast_ref::<EnsureAlias>().is_some())
        );
        assert_eq!(
            crate::sns::active_dataspace_owner_by_alias(
                &chain.state().world_view(),
                ALIAS,
                chain.committed(1).block_time_ms()
            )
            .unwrap(),
            Some(expected_owner)
        );
        let mut missing = parent_configuration();
        missing
            .genesis_instructions
            .retain(|instruction| instruction.as_any().downcast_ref::<EnsureAlias>().is_none());
        let without_intent = CertifiedTestChain::start(missing).unwrap();
        assert_eq!(
            crate::sns::active_dataspace_owner_by_alias(
                &without_intent.state().world_view(),
                ALIAS,
                without_intent.committed(1).block_time_ms()
            )
            .unwrap(),
            None
        );
    });
}
