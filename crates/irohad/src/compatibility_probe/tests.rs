//! `--check-storage` and `--check-config --json` probe tests.

use super::*;
use crate::config_tests::minimal_config_table;
use iroha_config::base::{WithOrigin, toml::TomlSource};
use iroha_core::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};
use iroha_data_model::block::SharedSignedBlock;
use std::{collections::BTreeMap, path::PathBuf, time::SystemTime};

fn minimal_config() -> Config {
    Config::from_toml_source(TomlSource::inline(minimal_config_table()))
        .expect("minimal config parses")
}

/// A minimal configuration whose Kura and snapshot roots live under `root`.
fn storage_config(root: &Path) -> Config {
    let mut config = minimal_config();
    config.kura.store_dir = WithOrigin::inline(root.join("kura"));
    // The minimal fixture's 4 KiB budget cannot hold Kura's fixed baseline.
    config.kura.max_disk_usage_bytes = iroha_config::base::util::Bytes(0);
    // Publish the commit marker with every fixture append, as a committed node store has it.
    config.kura.fsync_mode = iroha_config::kura::FsyncMode::Always;
    config.snapshot.store_dir = WithOrigin::inline(root.join("snapshot"));
    config
}

/// Persist original signed genesis and native certified successors into a Strict Kura.
fn populate_store(config: &Config, blocks: usize) -> Vec<SharedSignedBlock> {
    let (kura, _) = Kura::new_with_configured_lane_catalog(
        &config.kura,
        &config.nexus.lane_config,
        &config.nexus.configured_lane_catalog,
    )
    .expect("initialize Strict Kura");
    let mut chain =
        CertifiedTestChain::start(TestChainConfig::new(iroha_core::state::World::new(), 1_000))
            .expect("execute original signed genesis");
    let mut persisted: Vec<SharedSignedBlock> = Vec::new();
    for height in 1..=blocks {
        if height > 1 {
            chain.commit(Vec::new());
        }
        let block = chain.committed(height as u64).block().clone();
        kura.persist_block_immediate_for_bench(&block)
            .expect("persist fixture block");
        persisted.push(block);
    }
    drop(kura);
    persisted
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
fn check_storage_reports_a_missing_store_as_empty() {
    let root = tempfile::tempdir().expect("temporary store root");
    let report = check_storage(&storage_config(root.path())).expect("inspect missing store");
    assert_eq!(
        report,
        StorageCheckReportV1 {
            tip_height: 0,
            tip_hash: None,
            snapshot_height: None,
            prefix_hash_at_snapshot_height: None,
            snapshot_restore_dry_run: DRY_RUN_OK.to_owned(),
            snapshot_restore_error: None,
        }
    );
    assert!(report.is_ok());
    assert!(
        !root.path().join("kura").exists(),
        "the probe never creates a store"
    );
}

#[test]
fn check_storage_reads_a_fresh_store_without_blocks() {
    let root = tempfile::tempdir().expect("temporary store root");
    let config = storage_config(root.path());
    populate_store(&config, 0);
    let before = tree(root.path());
    let report = check_storage(&config).expect("inspect fresh store");
    assert_eq!(report.tip_height, 0);
    assert_eq!(report.tip_hash, None);
    assert!(report.is_ok(), "{report:?}");
    assert_eq!(
        tree(root.path()),
        before,
        "the probe never mutates the store"
    );
}

#[test]
fn check_storage_reads_a_populated_store_read_only() {
    let root = tempfile::tempdir().expect("temporary store root");
    let config = storage_config(root.path());
    let blocks = populate_store(&config, 3);
    let before = tree(root.path());
    let report = check_storage(&config).expect("inspect populated store");
    assert_eq!(report.tip_height, 3);
    assert_eq!(
        report.tip_hash,
        Some(hex_hash(blocks.last().expect("tip").hash()))
    );
    assert_eq!(report.snapshot_height, None, "no snapshot was written");
    assert!(report.is_ok(), "{report:?}");
    assert_eq!(
        tree(root.path()),
        before,
        "the probe never mutates the store"
    );
}

#[test]
fn check_storage_refuses_a_store_owned_by_a_running_node() {
    let root = tempfile::tempdir().expect("temporary store root");
    let config = storage_config(root.path());
    populate_store(&config, 0);
    let (owner, _) = Kura::new_with_configured_lane_catalog(
        &config.kura,
        &config.nexus.lane_config,
        &config.nexus.configured_lane_catalog,
    )
    .expect("a node owns the store");
    let error = check_storage(&config).expect_err("a locked store is refused");
    assert!(error.contains("cannot open Kura store"), "{error}");
    drop(owner);
    check_storage(&config).expect("the released store opens");
}

#[test]
fn check_storage_rejects_a_block_this_build_cannot_decode() {
    let root = tempfile::tempdir().expect("temporary store root");
    let config = storage_config(root.path());
    populate_store(&config, 2);
    let data = Kura::canonical_storage_path(
        &std::fs::canonicalize(root.path().join("kura")).expect("canonical store root"),
    )
    .join("blocks.data");
    // Corrupt the first (non-tip) body; opening Kura read-only checks only the durable boundary.
    let mut bytes = std::fs::read(&data).expect("read block data");
    bytes[16] ^= 0xFF;
    std::fs::write(&data, bytes).expect("corrupt block data");
    let error = check_storage(&config).expect_err("an undecodable body fails the probe");
    assert!(error.contains("block 1"), "{error}");
}

#[test]
fn run_check_storage_fails_closed_on_a_broken_snapshot() {
    let root = tempfile::tempdir().expect("temporary store root");
    let config = storage_config(root.path());
    populate_store(&config, 1);
    let snapshot = root.path().join("snapshot");
    std::fs::create_dir_all(&snapshot).expect("snapshot root");
    std::fs::write(snapshot.join("current"), b"not a digest").expect("broken current pointer");
    let report = check_storage(&config).expect("the Kura store itself opens");
    assert_eq!(report.tip_height, 1);
    assert_eq!(report.snapshot_restore_dry_run, DRY_RUN_ERROR);
    assert!(report.snapshot_restore_error.is_some());
    assert!(!report.is_ok());
    let error = run_check_storage(&config).expect_err("a failed dry run exits nonzero");
    assert!(matches!(error.current_context(), MainError::CheckStorage));
}

#[test]
fn config_compatibility_without_genesis_is_pending() {
    let config = minimal_config();
    let compatibility = config_compatibility_v1(&config, None, crate::test_build_metadata())
        .expect("compute compatibility");
    assert_eq!(compatibility.status, "pending");
    assert_eq!(compatibility.node_identity, None);
    assert_eq!(
        compatibility.diagnostic_build.source_revision,
        "local-fast-build"
    );
    assert_eq!(compatibility.protocol_version, PROTOCOL_VERSION);
    assert_eq!(compatibility.config_fingerprint, None);
    assert_eq!(compatibility.execution_policy_hash, None);
    assert_eq!(compatibility.nexus_amx_context_hash, None);
    assert_eq!(
        compatibility.wire_schema_hash,
        hex::encode(iroha_core::release_identity::wire_schema_hash())
    );
    assert_eq!(
        compatibility.gas_schedule_hash,
        hex::encode(<[u8; 32]>::from(ivm::gas::schedule_hash()))
    );
    for hash in [
        &compatibility.wire_schema_hash,
        &compatibility.nexus_policy_digest,
        &compatibility.gas_schedule_hash,
    ] {
        assert_eq!(hash.len(), 64);
        assert!(hash.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }
    // The values depend only on the build and the configuration.
    assert_eq!(
        config_compatibility_v1(&config, None, crate::test_build_metadata())
            .expect("recompute compatibility"),
        compatibility
    );
}

#[test]
fn probe_reports_roundtrip_through_norito_json() {
    let compatibility = ConfigCompatibilityV1 {
        status: "ready".to_owned(),
        node_identity: Some(NodeConfigIdentityV1 {
            node_id: iroha_model_base::peer::PeerId::new(
                iroha_crypto::KeyPair::try_from_seed(
                    vec![0x71; 32],
                    iroha_crypto::Algorithm::BlsNormal,
                )
                .expect("deterministic node identity")
                .public_key()
                .clone(),
            ),
            node_fingerprint: "99".repeat(32),
            node_config_fingerprint: "aa".repeat(32),
            initial_committee_size: 4,
            network_id: NetworkId::from_genesis_hash(marker_hash(0xBB)),
            genesis_hash: "bb".repeat(32),
        }),
        diagnostic_build: DiagnosticBuildIdentityV1 {
            version: "3.0.0".to_owned(),
            source_revision: "77".repeat(20),
            build_fingerprint: "88".repeat(32),
        },
        config_fingerprint: Some("11".repeat(32)),
        protocol_version: PROTOCOL_VERSION,
        wire_schema_hash: "22".repeat(32),
        nexus_policy_digest: "33".repeat(32),
        gas_schedule_hash: "44".repeat(32),
        execution_policy_hash: Some("55".repeat(32)),
        nexus_amx_context_hash: None,
    };
    let json = norito::json::to_json(&compatibility).expect("encode compatibility");
    assert!(json.contains("\"nexus_amx_context_hash\":null"), "{json}");
    assert_eq!(
        norito::json::from_str::<ConfigCompatibilityV1>(&json).expect("decode compatibility"),
        compatibility
    );
    let report = StorageCheckReportV1 {
        tip_height: 7,
        tip_hash: Some("66".repeat(32)),
        snapshot_height: Some(5),
        prefix_hash_at_snapshot_height: Some("77".repeat(32)),
        snapshot_restore_dry_run: DRY_RUN_OK.to_owned(),
        snapshot_restore_error: None,
    };
    let json = norito::json::to_json(&report).expect("encode storage report");
    assert_eq!(
        norito::json::from_str::<StorageCheckReportV1>(&json).expect("decode storage report"),
        report
    );
    assert!(
        norito::json::from_str::<StorageCheckReportV1>(&json.replacen(
            '{',
            "{\"unexpected\":1,",
            1
        ))
        .is_err(),
        "unknown fields are rejected"
    );
}

fn marker_hash(marker: u8) -> HashOf<BlockHeader> {
    HashOf::from_untyped_unchecked(Hash::prehashed([marker; Hash::LENGTH]))
}

/// A snapshot whose retained hashes match Kura's at its tip but not in its prefix is refused, as
/// the Strict startup refuses it; a snapshot above Kura's durable tip is also refused.
#[test]
fn restored_hashes_are_reconciled_at_every_retained_height() {
    let kura: Vec<_> = (1_u8..=5).map(marker_hash).collect();
    let kura_hash = |height: NonZeroUsize| kura.get(height.get() - 1).copied();
    reconcile_restored_hashes(kura.iter().copied(), 5, kura_hash).expect("identical history");
    reconcile_restored_hashes(kura[..3].iter().copied(), 5, kura_hash)
        .expect("a snapshot below Kura's tip");
    let mut ahead = kura.clone();
    ahead.push(marker_hash(6));
    let error = reconcile_restored_hashes(ahead.into_iter(), 5, kura_hash)
        .expect_err("snapshot state cannot exceed durable certified history");
    assert!(error.contains("height 6 exceeds"), "{error}");

    let mut tampered = kura.clone();
    tampered[1] = marker_hash(0xEE);
    assert_eq!(tampered.last(), kura.last(), "the tip still matches");
    let error = reconcile_restored_hashes(tampered.into_iter(), 5, kura_hash)
        .expect_err("a tampered prefix entry is refused");
    assert!(error.contains("height 2"), "{error}");

    let pruned = |height: NonZeroUsize| (height.get() != 3).then(|| kura[height.get() - 1]);
    let error = reconcile_restored_hashes(kura.iter().copied(), 5, pruned)
        .expect_err("a height Kura should hold is missing");
    assert!(error.contains("height 3"), "{error}");
}

#[test]
fn check_storage_error_is_localized() {
    crate::i18n::init(iroha_i18n::Language::English);
    let message = MainError::CheckStorage.to_string();
    assert!(!message.contains("error.check_storage"), "{message}");
    assert!(!message.is_empty());
}
