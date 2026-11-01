//! `--check-replay` store handling, report and flag tests. Replay of genuine certified copies
//! (match, divergence, missing history, snapshot start, upgrade checkpoint) is covered by
//! `iroha_core`'s `sumeragi::node::replay::check` tests.

use super::*;
use crate::config_tests::minimal_config_table;
use iroha_config::base::toml::TomlSource;
use iroha_core::sumeragi::{
    node::replay_check::{ComponentDifference, ReplayDivergence, ResultComponent},
    test_chain::{CertifiedTestChain, TestChainConfig},
};
use std::{collections::BTreeMap, time::SystemTime};

/// A minimal configuration whose Kura and snapshot roots live under `root`.
fn store_config(root: &Path) -> Config {
    let mut config = Config::from_toml_source(TomlSource::inline(minimal_config_table()))
        .expect("minimal config parses");
    config.kura.store_dir = WithOrigin::inline(root.join("kura"));
    // The minimal fixture's 4 KiB budget cannot hold Kura's fixed baseline.
    config.kura.max_disk_usage_bytes = iroha_config::base::util::Bytes(0);
    // Publish the commit marker with every fixture append, as a committed node store has it.
    config.kura.fsync_mode = iroha_config::kura::FsyncMode::Always;
    config.snapshot.store_dir = WithOrigin::inline(root.join("snapshot"));
    config
}

/// Persist original signed genesis and certified successors of a chain the configuration does
/// not trust into a Strict Kura.
fn populate_store(config: &Config, blocks: u64) {
    let (kura, _) = Kura::new_with_configured_lane_catalog(
        &config.kura,
        &config.nexus.lane_config,
        &config.nexus.configured_lane_catalog,
    )
    .expect("initialize Strict Kura");
    let mut chain =
        CertifiedTestChain::start(TestChainConfig::new(iroha_core::state::World::new(), 1_000))
            .expect("execute original signed genesis");
    for height in 1..=blocks {
        if height > 1 {
            chain.commit(Vec::new());
        }
        kura.persist_block_immediate_for_bench(chain.committed(height).block())
            .expect("persist fixture block");
    }
    drop(kura);
}

/// Every file under `root` with its bytes and modification time.
fn tree(root: &Path) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime)> {
    let mut files = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries {
            let path = entry.expect("directory entry").path();
            let metadata = std::fs::symlink_metadata(&path).expect("entry metadata");
            if metadata.is_dir() {
                pending.push(path);
            } else if metadata.is_file() {
                files.insert(
                    path.clone(),
                    (
                        std::fs::read(&path).expect("read store file"),
                        metadata.modified().expect("modification time"),
                    ),
                );
            }
        }
    }
    files
}

#[test]
fn check_replay_reports_a_missing_store_as_nothing_to_replay() {
    let root = tempfile::tempdir().expect("temporary store root");
    let report = check_replay(&store_config(root.path())).expect("inspect missing store");
    assert_eq!(
        report,
        ReplayCheckReportV1 {
            tip_height: 0,
            start: ReplayStartV1 {
                height: 0,
                source: START_NONE.to_owned(),
            },
            replayed_to: 0,
            outcome: OUTCOME_MATCHED.to_owned(),
            divergence: None,
            missing_height: None,
        }
    );
    assert!(report.is_ok());
    assert!(
        !root.path().join("kura").exists(),
        "the check never creates a store"
    );
}

#[test]
fn check_replay_reads_an_empty_store_without_writing() {
    let root = tempfile::tempdir().expect("temporary store root");
    let config = store_config(root.path());
    populate_store(&config, 0);
    let before = tree(root.path());
    let report = check_replay(&config).expect("inspect empty store");
    assert_eq!(report, ReplayCheckReportV1::unstarted(0, None));
    assert_eq!(
        tree(root.path()),
        before,
        "the check never writes the store"
    );
}

#[test]
fn check_replay_refuses_a_store_owned_by_a_running_node() {
    let root = tempfile::tempdir().expect("temporary store root");
    let config = store_config(root.path());
    populate_store(&config, 0);
    let (owner, _) = Kura::new_with_configured_lane_catalog(
        &config.kura,
        &config.nexus.lane_config,
        &config.nexus.configured_lane_catalog,
    )
    .expect("a node owns the store");
    let error = check_replay(&config).expect_err("a locked store is refused");
    assert!(matches!(error.current_context(), MainError::CheckReplay));
    assert!(
        format!("{error:?}").contains("cannot open Kura store"),
        "{error:?}"
    );
    drop(owner);
    check_replay(&config).expect("the released store opens");
}

#[test]
fn check_replay_authenticates_signed_genesis_before_replay_and_never_writes() {
    let root = tempfile::tempdir().expect("temporary store root");
    let config = store_config(root.path());
    populate_store(&config, 3);
    let before = tree(root.path());
    let error = check_replay(&config).expect_err("a genesis the configuration does not trust");
    assert!(matches!(error.current_context(), MainError::CheckReplay));
    let rendered = format!("{error:?}");
    assert!(rendered.contains("genesis"), "{rendered}");
    assert_eq!(
        tree(root.path()),
        before,
        "the check never writes the store"
    );
}

#[test]
fn reports_project_the_core_verdict_and_roundtrip_through_norito_json() {
    let diverged = ReplayCheckReportV1::from_core(
        9,
        &ReplayCheckReport {
            start: ReplayCheckStart::Snapshot(5),
            replayed_to: 7,
            outcome: ReplayCheckOutcome::Diverged(Box::new(ReplayDivergence {
                height: 8,
                first: ComponentDifference {
                    component: ResultComponent::WorldStateRoot,
                    expected: "aa".repeat(32),
                    actual: "bb".repeat(32),
                },
                components: vec![ResultComponent::WorldStateRoot, ResultComponent::Schedule],
            })),
        },
    );
    assert_eq!(
        diverged,
        ReplayCheckReportV1 {
            tip_height: 9,
            start: ReplayStartV1 {
                height: 5,
                source: START_SNAPSHOT.to_owned(),
            },
            replayed_to: 7,
            outcome: OUTCOME_DIVERGED.to_owned(),
            divergence: Some(ReplayDivergenceV1 {
                height: 8,
                component: "world_state_root".to_owned(),
                expected: "aa".repeat(32),
                actual: "bb".repeat(32),
                differing_components: vec!["world_state_root".to_owned(), "schedule".to_owned()],
            }),
            missing_height: None,
        }
    );
    assert!(!diverged.is_ok());
    let summary = diverged.failure_summary();
    assert!(
        summary.contains("block 8") && summary.contains("world_state_root"),
        "{summary}"
    );
    let json = norito::json::to_json(&diverged).expect("encode replay report");
    assert_eq!(
        norito::json::from_str::<ReplayCheckReportV1>(&json).expect("decode replay report"),
        diverged
    );
    assert!(
        norito::json::from_str::<ReplayCheckReportV1>(&json.replacen('{', "{\"unexpected\":1,", 1))
            .is_err(),
        "unknown fields are rejected"
    );

    let missing = ReplayCheckReportV1::from_core(
        9,
        &ReplayCheckReport {
            start: ReplayCheckStart::Genesis,
            replayed_to: 6,
            outcome: ReplayCheckOutcome::MissingHistory { height: 7 },
        },
    );
    assert_eq!(missing.outcome, OUTCOME_MISSING_HISTORY);
    assert_eq!(missing.missing_height, Some(7));
    assert_eq!(missing.divergence, None);
    assert_eq!(
        missing.start,
        ReplayStartV1 {
            height: 0,
            source: START_GENESIS.to_owned(),
        }
    );
    assert!(!missing.is_ok());
    assert!(missing.failure_summary().contains("height 7"));

    let matched = ReplayCheckReportV1::from_core(
        9,
        &ReplayCheckReport {
            start: ReplayCheckStart::Genesis,
            replayed_to: 9,
            outcome: ReplayCheckOutcome::Matched,
        },
    );
    assert!(matched.is_ok());
    assert_eq!(matched.replayed_to, 9);

    let unstarted = ReplayCheckReportV1::unstarted(4, Some(1));
    assert_eq!(unstarted.outcome, OUTCOME_MISSING_HISTORY);
    assert_eq!(unstarted.start.source, START_NONE);
    assert!(!unstarted.is_ok());
}

#[test]
fn check_replay_flag_excludes_the_other_offline_checks() {
    let args = crate::Args::try_parse_from(["iroha3d", "--check-replay"]).expect("flag parses");
    assert!(args.startup.check_replay);
    for other in ["--check-storage", "--check-config"] {
        assert!(
            crate::Args::try_parse_from(["iroha3d", "--check-replay", other]).is_err(),
            "--check-replay with {other}"
        );
    }
}

#[test]
fn check_replay_error_is_described() {
    let message = MainError::CheckReplay.to_string();
    assert!(message.contains("replay"), "{message}");
}
