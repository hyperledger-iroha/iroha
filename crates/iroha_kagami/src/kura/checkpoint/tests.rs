//! Tests exercise native certified execution, not self-asserted checkpoint construction.

use super::*;
use iroha_core::{
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::sumeragi_finality::SumeragiFinalityCheckpoint;

fn options(output_dir: PathBuf) -> Args {
    Args {
        chain_id: ChainId::from("sumeragi-certified-test-chain"),
        network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed(
            [7; 32],
        ))),
        height: 5,
        max_read_bytes: 64 * 1024 * 1024,
        output_dir,
    }
}

#[test]
fn invalid_bounds_reject_before_store_or_output_access() {
    let mut args = options(PathBuf::from("/missing-checkpoint-output"));
    for height in [0, 1, MAX_HEIGHT + 1, u64::MAX] {
        args.height = height;
        let mut output = Vec::new();
        let error = run(&mut output, Path::new("/missing-checkpoint-store"), &args).unwrap_err();
        assert!(error.to_string().contains("checkpoint height"));
        assert!(output.is_empty());
    }
    args.height = 2;
    for max_read_bytes in [0, MAX_READ_BYTES + 1, u64::MAX] {
        args.max_read_bytes = max_read_bytes;
        assert!(
            args.validate()
                .unwrap_err()
                .to_string()
                .contains("max-read-bytes")
        );
    }
}

#[test]
fn oversized_frame_and_cumulative_budget_reject_before_wire_allocation() {
    let directory = tempfile::tempdir().unwrap();
    let mut store = BlockStore::new(directory.path());
    store.create_files_if_they_do_not_exist().unwrap();
    let mut index = 0_u64.to_le_bytes().to_vec();
    index.extend_from_slice(&(MAX_FINALITY_BLOCK_BYTES as u64 + 1).to_le_bytes());
    fs::write(directory.path().join("blocks.index"), &index).unwrap();
    let mut count = 0;
    assert!(
        read_block(&mut store, 1, &mut count, MAX_READ_BYTES)
            .unwrap_err()
            .to_string()
            .contains("frame bounds")
    );
    assert_eq!(count, 0);
    index.truncate(8);
    index.extend_from_slice(&100_u64.to_le_bytes());
    fs::write(directory.path().join("blocks.index"), index).unwrap();
    count = 50;
    assert!(
        read_block(&mut store, 1, &mut count, 149)
            .unwrap_err()
            .to_string()
            .contains("wire-byte budget")
    );
    assert_eq!(count, 50);
}

#[test]
fn native_prefix_exports_checkpoint_and_ordered_next_roster_without_mutating_store() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    for _ in 2..=5 {
        chain.commit(Vec::new());
    }
    assert_eq!(chain.height(), 5);
    let directory = tempfile::tempdir().unwrap();
    let mut store = BlockStore::new(directory.path());
    store.create_files_if_they_do_not_exist().unwrap();
    for height in 1..=5 {
        store
            .append_block_to_chain(chain.committed(height).block())
            .unwrap();
    }
    drop(store);
    let names = ["blocks.index", "blocks.data", "blocks.hashes"];
    let before = names.map(|name| fs::read(directory.path().join(name)).unwrap());
    let destination = tempfile::tempdir().unwrap();
    let mut args = options(destination.path().join("verified"));
    args.network_id = chain.network_id();
    let mut output = Vec::new();
    run(&mut output, directory.path(), &args).unwrap();
    let bytes = fs::read(args.output_dir.join(CHECKPOINT_FILE)).unwrap();
    let checkpoint = SumeragiFinalityCheckpoint::decode_canonical(&bytes).unwrap();
    assert_eq!(checkpoint.height(), 5);
    assert_eq!(checkpoint.network_id(), chain.network_id());
    assert_eq!(checkpoint.block_hash(), chain.committed(5).block().hash());
    let consumer = SumeragiFinalityVerifier::from_trusted_checkpoint(
        &checkpoint,
        &args.network_id,
        args.chain_id.as_ref(),
    )
    .unwrap();
    assert_eq!(consumer.authenticated_height(), Some(5));
    assert_eq!(consumer.oldest_authenticated_height(), Some(3));
    let roster_bytes = fs::read(args.output_dir.join(ROSTER_FILE)).unwrap();
    let roster: Vec<PeerId> = norito::json::from_slice(&roster_bytes).unwrap();
    let committed = chain.committed(5);
    let ScheduledSlot::Ready(next) = &committed.commitment().schedule.next else {
        panic!("ready successor")
    };
    assert_eq!(
        roster,
        next.epoch
            .committee
            .iter()
            .map(|member| member.validator.clone())
            .collect::<Vec<_>>()
    );
    let report: norito::json::Value =
        norito::json::from_slice(&fs::read(args.output_dir.join(REPORT_FILE)).unwrap()).unwrap();
    assert_eq!(report["live_freshness_established"].as_bool(), Some(false));
    let verified_tip = consumer.verify_retained_decision(checkpoint.tip()).unwrap();
    assert_eq!(
        report["parent_core_hash_hex"].as_str(),
        Some(hex::encode(verified_tip.core_hash().0).as_str())
    );
    assert_eq!(
        report["parent_result_hash_hex"].as_str(),
        Some(hex::encode(verified_tip.result().0).as_str())
    );
    assert_eq!(
        report["next_height_configuration"]["height"].as_u64(),
        Some(6)
    );
    assert_eq!(
        report["checkpoint_blake3_hex"].as_str(),
        Some(blake3::hash(&bytes).to_hex().as_str())
    );
    assert_eq!(
        report["next_roster_blake3_hex"].as_str(),
        Some(blake3::hash(&roster_bytes).to_hex().as_str())
    );
    for (name, expected) in names.into_iter().zip(before) {
        assert_eq!(fs::read(directory.path().join(name)).unwrap(), expected);
    }
    assert!(
        run(&mut Vec::new(), directory.path(), &args).is_err(),
        "never overwrite a selected checkpoint"
    );
    args.output_dir = destination.path().join("wrong-root");
    args.network_id = options(PathBuf::new()).network_id;
    assert!(
        run(&mut Vec::new(), directory.path(), &args)
            .unwrap_err()
            .to_string()
            .contains("independently pinned")
    );
    assert!(!args.output_dir.exists());
    args.network_id = chain.network_id();
    args.chain_id = ChainId::from("wrong-chain");
    assert!(run(&mut Vec::new(), directory.path(), &args).is_err());
    assert!(!args.output_dir.exists());
    args.chain_id = ChainId::from("sumeragi-certified-test-chain");
    args.output_dir = directory.path().join("forbidden-output");
    assert!(
        run(&mut Vec::new(), directory.path(), &args)
            .unwrap_err()
            .to_string()
            .contains("inside the block store")
    );
    assert!(!args.output_dir.exists());
}

#[test]
fn tampered_native_prefix_never_publishes_checkpoint() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    chain.commit(Vec::new());
    let directory = tempfile::tempdir().unwrap();
    let mut store = BlockStore::new(directory.path());
    store.create_files_if_they_do_not_exist().unwrap();
    for height in 1..=2 {
        store
            .append_block_to_chain(chain.committed(height).block())
            .unwrap();
    }
    drop(store);
    let data = directory.path().join("blocks.data");
    let mut wire = fs::read(&data).unwrap();
    *wire.last_mut().unwrap() ^= 1;
    fs::write(&data, wire).unwrap();
    let destination = tempfile::tempdir().unwrap();
    let mut args = options(destination.path().join("rejected"));
    args.height = 2;
    args.network_id = chain.network_id();
    let mut output = Vec::new();
    assert!(run(&mut output, directory.path(), &args).is_err());
    assert!(output.is_empty());
    assert!(!args.output_dir.exists());
}
