//! Replay checks of genuine certified copies: a clean copy, a certified result this build does not
//! reproduce, an absent body, a snapshot start on a whole-store copy and the upgrade checkpoint.

use super::super::{NativeReplaySeed, tests::fixture};
use super::*;
use crate::{
    governance::manifest::LaneManifestRegistry,
    state::upgrade_checkpoint::UpgradeCheckpointV1,
    sumeragi::test_chain::{CertifiedTestChain, Signers},
};
use iroha_data_model::prelude::*;
use iroha_test_samples::{ALICE_KEYPAIR, BOB_ID};
use std::collections::BTreeMap;

fn run<T: Send + 'static>(test: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .name("replay-check".into())
        .stack_size(32 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap_or_else(|failure| std::panic::resume_unwind(failure))
}

/// Every file under `root` with its bytes.
fn inventory(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, directory: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                visit(root, &entry.path(), files);
            } else if entry.file_type().unwrap().is_file() {
                files.insert(
                    entry.path().strip_prefix(root).unwrap().to_path_buf(),
                    std::fs::read(entry.path()).unwrap(),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}

fn index(height: u64) -> NonZeroUsize {
    NonZeroUsize::new(usize::try_from(height).unwrap()).unwrap()
}

/// Commit one genuine transfer at `time_ms`.
fn transfer(chain: &mut CertifiedTestChain, asset: &AssetId, time_ms: u64) {
    let transaction = chain.sign(
        &ALICE_KEYPAIR,
        [Transfer::asset_quantity(asset.clone(), 1_u32, BOB_ID.clone()).into()],
        time_ms - 1,
    );
    assert_eq!(chain.commit_at(time_ms, vec![transaction]), [true]);
}

/// The original configuration's empty State on a temporary store holding the chain's blocks.
fn genesis_destination(chain: &CertifiedTestChain, seed: &NativeReplaySeed) -> Arc<State> {
    let genesis = seed
        .source
        .get_block(NonZeroUsize::MIN, &seed.budget)
        .unwrap()
        .unwrap();
    let state = seed.initialize_destination(&genesis).unwrap();
    let tip = chain.kura().exact_durable_blocks_count().unwrap();
    for height in state.kura().blocks_count() + 1..=tip {
        let block = chain
            .kura()
            .get_block(NonZeroUsize::new(height).unwrap(), &seed.budget)
            .unwrap()
            .unwrap();
        state.kura().store_block(block).unwrap();
    }
    state
}

/// The State a snapshot of the original chain restores on a copy, configured with the process
/// policies the original State executed with, as startup installs them from configuration.
fn restored_destination(seed: &NativeReplaySeed, snapshot: &str, kura: &Arc<Kura>) -> Arc<State> {
    let mut state = crate::state::deserialize::KuraSeed {
        execution_budget: seed.budget.clone(),
        kura: Arc::clone(kura),
        lane_manifests: Arc::new(
            LaneManifestRegistry::empty().rebind(&seed.nexus.lane_catalog, &seed.nexus.governance),
        ),
        query_handle: crate::query::store::LiveQueryStore::start_test(),
        #[cfg(feature = "telemetry")]
        telemetry: crate::telemetry::StateTelemetry::default(),
    }
    .into_state_from_json_str_with_configured_nexus(snapshot, seed.nexus.clone())
    .expect("the copied certified successor authenticates the snapshot World");
    state.install_node_zk(seed.zk.clone()).unwrap();
    state.install_crypto_snapshot_for_isolated_replay((*seed.crypto).clone());
    state.set_oracle(seed.oracle.clone());
    state.install_node_fraud_monitoring(seed.fraud.clone());
    state.set_gov(seed.governance.clone());
    state.content = seed.content.clone();
    state.set_settlement(seed.settlement);
    Arc::from(state)
}

fn inputs(seed: &NativeReplaySeed, state: Arc<State>) -> ReplayCheckInputs {
    ReplayCheckInputs {
        source_lanes: state.kura().store_root().join("lanes"),
        state,
        genesis_account: seed.genesis_account.clone(),
        consensus_mode: seed.consensus_mode,
    }
}

fn copy(
    chain: &CertifiedTestChain,
    seed: &NativeReplaySeed,
) -> Result<Arc<Kura>, ReplayCheckError> {
    copy_store(
        chain.kura(),
        &seed.kura_config,
        &seed.nexus.lane_config,
        &seed.nexus.configured_lane_catalog,
        &mut |_| {},
    )
}

fn stored_preimage(chain: &CertifiedTestChain, height: u64) -> Vec<u8> {
    chain
        .kura()
        .get_block(index(height), &chain.state().ivm_execution_budget())
        .unwrap()
        .unwrap()
        .commit_certificate()
        .unwrap()
        .result_preimage()
        .to_vec()
}

#[test]
fn clean_copy_replays_from_signed_genesis_without_touching_the_source() {
    run(|| {
        let (mut chain, seed, asset) = fixture();
        transfer(&mut chain, &asset, 1_200);
        let source = inventory(&chain.kura().store_root());
        let destination = genesis_destination(&chain, &seed);
        let mut replayed = Vec::new();
        let report = check_replay(inputs(&seed, destination), &mut |height, tip| {
            replayed.push((height, tip));
        })
        .unwrap();
        assert_eq!(
            report,
            ReplayCheckReport {
                start: ReplayCheckStart::Genesis,
                replayed_to: 3,
                outcome: ReplayCheckOutcome::Matched,
            }
        );
        assert_eq!(replayed, [(1, 3), (2, 3), (3, 3)]);
        assert_eq!(
            inventory(&chain.kura().store_root()),
            source,
            "the original store is only read"
        );
    });
}

#[test]
fn certified_result_this_build_does_not_reproduce_names_the_first_differing_component() {
    run(|| {
        let (mut chain, seed, asset) = fixture();
        transfer(&mut chain, &asset, 1_200);
        let (header, mut body) = decode_result_preimage(&stored_preimage(&chain, 3))
            .unwrap()
            .into_parts();
        let replayed_root = body.execution.world_state_root;
        let certified_root = Hash::new(b"a World this build never produces");
        body.execution.world_state_root = certified_root;
        let preimage = CertifiedResultV1::from_parts_unvalidated(header, body)
            .preimage()
            .unwrap();
        let result = result_of_preimage(&preimage).unwrap();
        // The exact committee certifies the altered result: only re-execution can tell.
        let qc = chain.commit_qc(3, chain.committed(3).core_hash(), result, Signers::Quorum);
        chain
            .kura()
            .corrupt_commit_result_for_testing(index(3), preimage)
            .unwrap();
        chain
            .kura()
            .corrupt_commit_certificate_for_testing(
                index(3),
                Some(norito::encode_canonical(&qc).unwrap()),
            )
            .unwrap();
        let report = check_replay(
            inputs(&seed, genesis_destination(&chain, &seed)),
            &mut |_, _| {},
        )
        .unwrap();
        assert_eq!(
            report,
            ReplayCheckReport {
                start: ReplayCheckStart::Genesis,
                replayed_to: 2,
                outcome: ReplayCheckOutcome::Diverged(Box::new(ReplayDivergence {
                    height: 3,
                    first: ComponentDifference {
                        component: ResultComponent::WorldStateRoot,
                        expected: certified_root.to_string(),
                        actual: replayed_root.to_string(),
                    },
                    components: vec![ResultComponent::WorldStateRoot],
                })),
            }
        );
    });
}

#[test]
fn absent_body_is_missing_history_above_the_reproduced_prefix() {
    run(|| {
        let (mut chain, seed, asset) = fixture();
        transfer(&mut chain, &asset, 1_200);
        let destination = genesis_destination(&chain, &seed);
        destination
            .kura()
            .corrupt_canonical_body_for_testing(index(3))
            .unwrap();
        let report = check_replay(inputs(&seed, destination), &mut |_, _| {}).unwrap();
        assert_eq!(
            report,
            ReplayCheckReport {
                start: ReplayCheckStart::Genesis,
                replayed_to: 2,
                outcome: ReplayCheckOutcome::MissingHistory { height: 3 },
            }
        );
    });
}

#[test]
fn whole_store_copy_replays_from_an_authenticated_snapshot_without_touching_the_source() {
    run(|| {
        let (mut chain, seed, asset) = fixture();
        transfer(&mut chain, &asset, 1_200);
        let snapshot = crate::snapshot::CapturedStateSnapshot::capture(chain.state())
            .expect("stable snapshot cut at height three");
        transfer(&mut chain, &asset, 1_300);
        transfer(&mut chain, &asset, 1_400);
        let source = inventory(&chain.kura().store_root());
        let mut copied_bytes = 0;
        let copy = copy_store(
            chain.kura(),
            &seed.kura_config,
            &seed.nexus.lane_config,
            &seed.nexus.configured_lane_catalog,
            &mut |bytes| copied_bytes = bytes,
        )
        .unwrap();
        assert!(copy.is_temporary_staging());
        assert_ne!(copy.store_root(), chain.kura().store_root());
        assert_eq!(copy.exact_durable_blocks_count().unwrap(), 5);
        assert!(copied_bytes > 0);
        let state = restored_destination(&seed, snapshot.as_json(), &copy);
        assert_eq!(state.committed_height(), 3);
        let mut replayed = Vec::new();
        let report = check_replay(inputs(&seed, state), &mut |height, _| {
            replayed.push(height);
        })
        .unwrap();
        assert_eq!(
            report,
            ReplayCheckReport {
                start: ReplayCheckStart::Snapshot(3),
                replayed_to: 5,
                outcome: ReplayCheckOutcome::Matched,
            }
        );
        assert_eq!(replayed, [4, 5], "only blocks above the snapshot execute");
        assert_eq!(
            inventory(&chain.kura().store_root()),
            source,
            "the original store is only read"
        );
    });
}

#[cfg(unix)]
#[test]
fn copy_refuses_a_store_entry_that_is_neither_a_file_nor_a_directory() {
    run(|| {
        let (chain, seed, _) = fixture();
        let stray = chain.kura().store_root().join("stray-link");
        std::os::unix::fs::symlink("/nonexistent", &stray).unwrap();
        let error = copy(&chain, &seed).map(|_| ()).unwrap_err();
        assert!(
            matches!(&error, ReplayCheckError::Copy { path, .. } if *path == stray),
            "{error}"
        );
    });
}

#[test]
fn replay_never_starts_below_the_upgrade_checkpoint() {
    run(|| {
        let (chain, seed, _) = fixture();
        let destination = genesis_destination(&chain, &seed);
        destination
            .kura()
            .set_upgrade_checkpoint(UpgradeCheckpointV1 {
                height: 2,
                block_hash: chain.kura().get_block_hash(index(2)).unwrap(),
                signer: None,
            })
            .unwrap();
        let mut replayed = Vec::new();
        let error = check_replay(inputs(&seed, destination), &mut |height, _| {
            replayed.push(height);
        })
        .unwrap_err();
        assert!(
            matches!(
                error,
                ReplayCheckError::BelowCheckpoint {
                    start: 0,
                    checkpoint: 2
                }
            ),
            "{error}"
        );
        assert!(replayed.is_empty(), "nothing executes below the checkpoint");
    });
}

#[test]
fn differences_follow_report_order_and_fall_back_to_the_whole_result() {
    run(|| {
        let (chain, _, _) = fixture();
        let preimage = stored_preimage(&chain, 2);
        let certified = result_of_preimage(&preimage).unwrap();
        let stored = decode_result_preimage(&preimage).unwrap();
        assert!(result_differences(&stored, &stored).is_empty());
        let (header, mut body) = stored.clone().into_parts();
        body.execution.world_state_root = Hash::new(b"another World");
        body.execution.executed_block_wire_len += 1;
        let produced = CertifiedResultV1::from_parts_unvalidated(header, body);
        let divergence = ReplayDivergence::between(2, &preimage, certified, &produced);
        assert_eq!(divergence.height, 2);
        assert_eq!(
            divergence.components,
            [
                ResultComponent::TransactionResults,
                ResultComponent::WorldStateRoot
            ]
        );
        assert_eq!(
            divergence.first.component,
            ResultComponent::TransactionResults
        );
        let length = stored.commitment().execution.executed_block_wire_len;
        assert!(
            divergence
                .first
                .expected
                .ends_with(&format!("({length} bytes)"))
        );
        assert!(
            divergence
                .first
                .actual
                .ends_with(&format!("({} bytes)", length + 1))
        );
        let whole = ReplayDivergence::between(2, b"not a result preimage", certified, &produced);
        assert_eq!(whole.components, [ResultComponent::Result]);
        assert_eq!(whole.first.expected, certified.to_string());
        assert_eq!(whole.first.actual, produced.result().unwrap().to_string());
        assert_eq!(
            [
                ResultComponent::Validity,
                ResultComponent::ParentWorldStateRoot,
                ResultComponent::TransactionResults,
                ResultComponent::WorldStateRoot,
                ResultComponent::WorldStateAccumulator,
            ]
            .map(ResultComponent::name),
            [
                "validity",
                "parent_world_state_root",
                "transaction_results",
                "world_state_root",
                "world_state_accumulator",
            ]
        );
    });
}

#[test]
fn rejection_and_start_describe_themselves() {
    let rejected = ReplayDivergence::rejected(7);
    assert_eq!(rejected.height, 7);
    assert_eq!(rejected.components, [ResultComponent::Validity]);
    assert_eq!(rejected.first.expected, "valid");
    assert_eq!(rejected.first.actual, "rejected");
    assert_eq!(ReplayCheckStart::Genesis.height(), 0);
    assert_eq!(ReplayCheckStart::Snapshot(4).height(), 4);
}
